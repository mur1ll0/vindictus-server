"""Add the laboratory content rows (scripts/content/lab_content_additions.json) and the published
admin panel edits (.cache/private-content/content-edits.json, B4.4) to a COPY of the client's
sql/heroes.db3, so the client shows the NPC shops the server sells (for example the Ferghus row
of Enhancement Elixir, or a panel price change). ShopInfo is rebuilt from the backup of the
original table each time, so a removed or changed row is undone when its edit is. Dry-run by
default; idempotent; the original clients are refused and a consistent backup is kept before
the first change."""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import sqlite3
import struct
import sys
from contextlib import closing
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content'))
import class_costumes  # noqa: E402
import class_imports  # noqa: E402
import content_additions  # noqa: E402
import content_edits  # noqa: E402
import hfs  # noqa: E402
import kv_merge  # noqa: E402
import mdl_anim  # noqa: E402

ADDITIONS = Path(__file__).resolve().parents[1] / 'content/lab_content_additions.json'
SHOP_COLUMNS = ('ShopID', 'Order', 'ItemClass', 'Count', 'PriceItemClass', 'PriceCount', 'AppearQuestID',
                'CharacterBind', 'ID', 'RestrictionResetTime', 'RestrictionPeriodDay', 'RestrictionCount', 'Feature')
SELECT = 'SELECT ' + ','.join('[%s]' % c for c in SHOP_COLUMNS) + ' FROM ShopInfo ORDER BY rowid'


def rows_of(db: sqlite3.Connection) -> list[dict]:
    return [dict(zip(SHOP_COLUMNS, r)) for r in db.execute(SELECT)]


def missing_rows(db: sqlite3.Connection, rows: list[dict]) -> list[dict]:
    out = []
    for r in rows:
        found = db.execute('SELECT ItemClass FROM ShopInfo WHERE ShopID=? AND [Order]=?', (r['ShopID'], r['Order'])).fetchall()
        if found and found[0][0] != r['ItemClass']:
            raise PreparationError('A client ShopInfo row already uses this ShopID and Order.')
        if not found:
            out.append(r)
    return out


def target_rows(original: list[dict], additions: list[dict], edits: dict) -> list[dict]:
    keys = {(r['ShopID'], r['Order']): r for r in original}
    for r in additions:
        found = keys.get((r['ShopID'], r['Order']))
        if found is not None and found['ItemClass'] != r['ItemClass']:
            raise PreparationError('A client ShopInfo row already uses this ShopID and Order.')
    rows = original + [r for r in additions if (r['ShopID'], r['Order']) not in keys]
    rows = content_edits.apply_shop_rows(rows, edits)
    # The client stores CharacterBind as the text 'True'/'False'.
    return [{**r, 'CharacterBind': ('True' if r['CharacterBind'] else 'False') if isinstance(r['CharacterBind'], bool)
             else r['CharacterBind']} for r in rows]


TEXT_FOLDERS = ('resource/localized_text/english', 'resource/localized_text/english_eu')
SCRIPTS_FOLDER = 'scripts'
INDEX = 'hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs'
ASSETS = Path(__file__).resolve().parents[2] / '.cache/private-assets'


def with_tokens(text: bytes, entries: dict[str, str]) -> bytes:
    """The localization file (UTF-16 LE with BOM) with the entries first in its Tokens block; an
    existing line of the same key (a base item the panel renamed) is dropped."""
    decoded = text.decode('utf-16')
    if entries:
        existing = re.compile(r'^[ \t]*"(?:%s)"[ \t]+"[^\r\n]*"\r?\n' % '|'.join(map(re.escape, entries)), re.M | re.I)
        decoded = existing.sub('', decoded)
    tokens = decoded.index('"Tokens"')
    opening = decoded.index('{\r\n', tokens) + 3
    lines = ''.join('\t\t"%s"\t"%s"\r\n' % (key, value) for key, value in sorted(entries.items()))
    return (decoded[:opening] + lines + decoded[opening:]).encode('utf-16')


def kv_blocks(text: bytes) -> dict[tuple[str, ...], tuple[int, int, set[str]]]:
    """Every KeyValues block of a client script by its key path (lower case): the offsets of its
    braces and the keys of its child blocks. Quoted or bare tokens, `//` comments."""
    blocks, stack, pending, at = {}, [], None, 0
    while at < len(text):
        c = text[at:at + 1]
        if c in b' \t\r\n':
            at += 1
        elif text.startswith(b'//', at):
            end = text.find(b'\n', at)
            at = len(text) if end < 0 else end
        elif c == b'{':
            path = tuple(p for p, _, _ in stack) + ((pending or '').lower(),)
            if stack:
                stack[-1][2].add((pending or '').lower())
            stack.append(((pending or '').lower(), at, set()))
            pending = None
            at += 1
        elif c == b'}':
            if stack:
                name, opening, children = stack.pop()
                blocks.setdefault(tuple(p for p, _, _ in stack) + (name,), (opening, at, children))
            pending = None
            at += 1
        else:
            if c == b'"':
                end = text.find(b'"', at + 1)
                end = len(text) if end < 0 else end
                token, at = text[at + 1:end], end + 1
            else:
                end = at
                while end < len(text) and text[end:end + 1] not in b' \t\r\n{}"':
                    end += 1
                token, at = text[at:end], end
            # A key followed by a value resets; a key followed by '{' names the block; a
            # condition such as [$WIN32] changes nothing.
            if not token.startswith(b'['):
                pending = None if pending is not None else token.decode('latin-1')
    return blocks


# Entry blocks the client reads models from (docs/future/item-editor.md, section 3.3).
SCRIPT_ENTRIES = {'weapon': (('weapondata', 'subtypeinfooverride'), ('viewmodel', 'playermodel')),
                  'shields': (('fiona',), ('normal', 'broken'))}


def script_of(section: str) -> str:
    """The scripts pack file of a costume entry."""
    if section.startswith('weapon_'):
        return section + '.txt.comp'
    if section == 'shields':
        return 'shields.txt.comp'
    return 'player_costume.txt.comp'


def with_script_entries(text: bytes, costumes: list[dict], kind: str) -> bytes:
    """A weapon script or shields.txt with an entry per costume (key: CostumeType) in its model block."""
    path, fields = SCRIPT_ENTRIES[kind]
    for costume in costumes:
        block = kv_blocks(text).get(path)
        if block is None:
            raise PreparationError(f'The {costume["section"]} script has no {"/".join(path)} block.')
        opening, closing, children = block
        if costume['key'] in children:
            raise PreparationError(f'The {costume["section"]} script already has entry {costume["key"]}.')
        lines = ''.join('\t\t\t"%s"\t"%s"\r\n' % (f, costume['normal']) for f in fields)
        entry = ('\r\n\t\t"%s"\r\n\t\t{\r\n%s\t\t}\r\n\t' % (costume['key'], lines)).encode('ascii')
        text = text[:closing] + entry + text[closing:]
    return text


def with_costumes(text: bytes, costumes: list[dict]) -> bytes:
    """player_costume.txt with the entries appended to their sections (each: section, key, category, normal)."""
    for costume in costumes:
        found = re.search(rb'(?:^|\n)"%s"\r?\n\{' % re.escape(costume['section'].encode()), text)
        if found is None:
            raise PreparationError(f'player_costume.txt has no {costume["section"]} section.')
        start = found.start()
        end = re.compile(rb'\n"[^"\r\n]+"\r?\n\{').search(text, start + 1)
        end = end.start() if end else len(text)
        closing = text.rindex(b'}', start, end)
        if re.search(rb'\n\t"%s"\r?\n\t\{' % costume['key'].encode(), text[start:end]):
            raise PreparationError(f'player_costume.txt already has {costume["section"]} {costume["key"]}.')
        block = ('\r\n\t"%s"\r\n\t{\r\n\t\t"category"\t"%s"\r\n\t\t"label"\t"%s"\r\n\t\t"normal"\t"%s"\r\n\t}\r\n'
                 % (costume['key'], costume['category'], costume['key'], costume['normal'])).encode('ascii')
        text = text[:closing] + block + text[closing:]
    return text


def asset_packs(items: list[dict], index_packs: set[str], assets: Path) -> dict[str, list]:
    """New packs for the items' asset files, by pack name: a folder the index already knows gets its next
    part, a new folder part 0. Files go in as .comp, as the game stores models and materials."""
    folders: dict[str, list] = {}
    seen: dict[str, str] = {}
    for item in items:
        if not item.get('assets'):
            continue
        root = (assets / item['assets']).resolve()
        if not root.is_relative_to(assets.resolve()) or not root.is_dir():
            raise PreparationError(f'Missing asset folder {item["assets"]}.')
        # Game files only: a kit (scripts/client/model_kit.py) also has kit.json at its root.
        for path in sorted(p for p in root.rglob('*') if p.is_file() and p.parent != root):
            game_path = path.relative_to(root).as_posix().lower()
            if seen.setdefault(game_path, item['ItemClass']) != item['ItemClass']:
                raise PreparationError(f'{game_path} is in the assets of {seen[game_path]} and {item["ItemClass"]}.')
            folder = path.parent.relative_to(root).as_posix().lower()
            folders.setdefault(folder, []).append(hfs.Entry(path.name.lower() + '.comp', hfs.comp_wrap(path.read_bytes()), hfs.DEFAULT_ATTR))
    packs = {}
    for folder, entries in folders.items():
        part = 0
        while hfs.pack_name(folder, part) in index_packs:
            part += 1
        packs[hfs.pack_name(folder, part)] = entries
    return packs


CLASS_MANIFEST = 'class-assets.json'
CLASS_PREFIXES = ('models/', 'materials/', 'effects/', 'expressions/', 'csf/', 'scaleform/', 'sound/', 'scripts/')
# Plain bytes per new pack part: few parts per folder (the 1.69 has at most 14) and packs the size of
# the largest 1.69 ones (docs/future/miri-class-port.md 2.5).
CLASS_PART_LIMIT = 192 * 2**20
CLASS_PACKS = 'class-packs.json'
GAME_PATH = re.compile(r'^[a-z0-9_./ -]{1,240}$')


def class_asset_files(assets: Path = ASSETS) -> list[tuple[str, Path, str]]:
    """(game path, staged file, SHA-256) of every NA class staged by scripts/client/stage_na_class.py."""
    files: dict[str, tuple[Path, str]] = {}
    for manifest in sorted((assets / 'classes').glob('*/' + CLASS_MANIFEST)):
        root = manifest.parent.resolve()
        for record in json.loads(manifest.read_text(encoding='utf-8')).get('files', []):
            path = str(record.get('path', ''))
            if (not GAME_PATH.match(path) or not path.startswith(CLASS_PREFIXES) or '..' in path.split('/')
                    or not re.fullmatch(r'[0-9a-f]{64}', str(record.get('sha256', '')))):
                raise PreparationError(f'Bad class asset entry in {manifest}.')
            file = (root / path).resolve()
            if not file.is_relative_to(root) or not file.is_file():
                raise PreparationError(f'Missing staged class asset {path}.')
            if files.setdefault(path, (file, record['sha256']))[0] != file:
                raise PreparationError(f'{path} is staged by two classes.')
    return sorted((path, file, sha) for path, (file, sha) in files.items())


def class_weapon_costumes(assets: Path = ASSETS, imports: Path = content_additions.CLASS_IMPORTS) -> list[dict]:
    """Model entries of each staged imported class's weapon script, renumbered into the lab entity's
    script (class_costumes.LAB_WEAPON_ENTITY), the same numbers class_imports gives its items."""
    out = []
    for path in class_imports.load_imports(imports):
        info = class_imports.read_meta(path)
        script = assets / 'classes' / info['name'] / 'scripts' / f'weapon_{info["weapon"]}.txt'
        if (assets / 'classes' / info['name'] / CLASS_MANIFEST).is_file() and script.is_file():
            out += class_costumes.weapon_costumes(script.read_bytes(), info['weapon'])
    return out


def class_merges(assets: Path = ASSETS) -> list[tuple[str, Path, tuple[str, ...]]]:
    """(game path, NA copy, class names) of each shared 1.69 script a staged class adds lines to."""
    merges = []
    for manifest in sorted((assets / 'classes').glob('*/' + CLASS_MANIFEST)):
        root = manifest.parent.resolve()
        data = json.loads(manifest.read_text(encoding='utf-8'))
        for record in data.get('merge', []):
            path = str(record.get('path', ''))
            needles = tuple(str(n) for n in record.get('needles', []))
            file = (root / 'merge' / path).resolve()
            if (not GAME_PATH.match(path) or '..' in path.split('/') or not needles
                    or not all(re.fullmatch(r'[a-z]{2,16}', n) for n in needles)
                    or not file.is_relative_to(root) or not file.is_file()):
                raise PreparationError(f'Bad class merge entry in {manifest}.')
            merges.append((path, file, needles))
    return merges


def original_pack(plain: bytes, path: str, client_copy: Path) -> Path:
    """The 1.69 pack holding a game path, from the original index."""
    names = hfs.index_packs(plain)
    folder, file_name = path.rsplit('/', 1)
    parts = {hfs.pack_name(folder, part) for part in range(64)}
    key = hfs.name_key(file_name)
    found = sorted({names[struct.unpack_from('<H', e, 6)[0]] for e in hfs.index_entries(plain) if e[:6] == key} & parts)
    if len(found) != 1:
        raise PreparationError(f'{path} is not in exactly one pack of the client.')
    return client_copy / 'hfs' / (found[0] + '.hfs')


def merge_packs(plain: bytes, merges: list, client_copy: Path) -> dict[Path, dict[str, list]]:
    """The 1.69 pack holding each merged script, from the original index: pack path -> file -> merges."""
    packs: dict[Path, dict[str, list]] = {}
    for path, file, needles in merges:
        pack = original_pack(plain, path, client_copy)
        packs.setdefault(pack, {}).setdefault(path.rsplit('/', 1)[1], []).append((file, needles))
    return packs


# 1.69 files rewritten inside their own pack (rebuilt from the backup) for an imported class's look: the
# Delia's town jump carried the weapon into her hand, as the Miri's did (mdl_anim.town_jump_on_back,
# C4 MI9), in a combat-pose jump: the Fiona's free-hands jump replaces it (C4 MI11). A fix takes the file
# and a reader of other original files; returning None leaves the file as it is.
ORIGINAL_FIXES = {'models/player/_anim/female/delia/pc_delia_town_motion.mdl':
                  lambda data, read: mdl_anim.town_jump_on_back(data, read(mdl_anim.TOWN_JUMP_DONOR))}


def fixed_packs(plain: bytes, client_copy: Path, fixes: dict = ORIGINAL_FIXES) -> dict[Path, dict[str, object]]:
    """Original pack -> lower-case file name -> fix, for the 1.69 files a staged class needs changed."""
    packs: dict[Path, dict[str, object]] = {}
    for path, fix in fixes.items():
        packs.setdefault(original_pack(plain, path, client_copy), {})[path.rsplit('/', 1)[1].lower()] = fix
    return packs


# Class scripts in a folder the 1.69 already has go into the 1.69's own (last) pack of that folder: the
# client did not read them from a new part of the folder (C4 MI6: no key of the Miri's action-state,
# expression or keyword scripts reached memory, while her new folders and the merged scripts load).
# The Draker's chain material (M7f) joins the 1.69's cable pack the same way, for the same doubt.
INJECTED_PREFIXES = ('scripts/', 'materials/cable/')


def injected_packs(plain: bytes, files: list[tuple[str, Path, str]],
                   client_copy: Path) -> dict[Path, list[tuple[str, Path, str]]]:
    """Original pack (the folder's last part in the original index) -> class files (game path, file,
    SHA-256) added to it."""
    names = set(hfs.index_packs(plain))
    out: dict[Path, list[tuple[str, Path, str]]] = {}
    for path, file, sha in files:
        if not path.startswith(INJECTED_PREFIXES):
            continue
        folder = path.rsplit('/', 1)[0]
        parts = [part for part in range(64) if hfs.pack_name(folder, part) in names]
        if parts:
            pack = client_copy / 'hfs' / (hfs.pack_name(folder, max(parts)) + '.hfs')
            out.setdefault(pack, []).append((path, file, sha))
    return out


def class_pack_plan(files: list[tuple[str, Path, str]], taken: set[str]) -> list[tuple[str, str, list[tuple[str, Path, str]]]]:
    """New packs for the staged class files: per folder, the parts after the ones the index has, each up
    to CLASS_PART_LIMIT plain bytes."""
    folders: dict[str, list] = {}
    for path, file, sha in files:
        folder, name = path.rsplit('/', 1)
        folders.setdefault(folder, []).append((name, file, sha))
    plan = []
    for folder, entries in sorted(folders.items()):
        part = 0
        current, size = [], 0
        for entry in entries:
            length = entry[1].stat().st_size
            if current and size + length > CLASS_PART_LIMIT:
                while hfs.pack_name(folder, part) in taken:
                    part += 1
                plan.append((hfs.pack_name(folder, part), folder, current))
                part += 1
                current, size = [], 0
            current.append(entry)
            size += length
        while hfs.pack_name(folder, part) in taken:
            part += 1
        plan.append((hfs.pack_name(folder, part), folder, current))
    return plan


def class_entry(folder: str, name: str, file: Path) -> tuple[str, object, int]:
    """Sounds are stored plain, the rest as .comp, as the 1.69 packs do."""
    if folder == 'sound' or folder.startswith('sound/'):
        return name, file.read_bytes, hfs.DEFAULT_ATTR
    return name + '.comp', lambda: hfs.comp_wrap(file.read_bytes(), 6), hfs.DEFAULT_ATTR


def class_packs(client_copy: Path, plan: list, ks: bytes, apply_changes: bool) -> tuple[list, int]:
    """(pack name, pack_files list) of each class pack and how many were written. A pack is rebuilt only
    when its file list or a file's SHA-256 changed (class-packs.json in lab-backup)."""
    cache_path = client_copy / 'lab-backup' / CLASS_PACKS
    cache = json.loads(cache_path.read_text(encoding='utf-8')) if cache_path.exists() else {}
    listed, built, fresh = [], 0, {}
    for name, folder, entries in plan:
        key = hashlib.sha256(json.dumps([(n, sha) for n, _, sha in entries]).encode()).hexdigest()
        path = client_copy / 'hfs' / (name + '.hfs')
        known = cache.get(name, {})
        if known.get('key') == key and path.is_file() and path.stat().st_size == known.get('size'):
            files = [tuple(f) for f in known['files']]
        elif apply_changes:
            files = hfs.write_pack(path, [class_entry(folder, n, f) for n, f, _ in entries], ks)
            built += 1
        else:
            built += 1
            continue
        fresh[name] = {'key': key, 'size': path.stat().st_size, 'files': files}
        listed.append((name, files))
    if apply_changes:
        cache_path.parent.mkdir(exist_ok=True)
        cache_path.write_text(json.dumps(fresh), encoding='utf-8')
    return listed, built


def apply_hfs(client_copy: Path, items: list[dict], keystream: Path, apply_changes: bool, assets: Path = ASSETS,
              overrides: dict | None = None) -> dict:
    """Rebuilds the English text packs (item text) and the scripts pack (player_costume.txt entries) from their
    backups, adds packs for the items' asset files and points the HFS index at them (docs/future/
    game-launcher.md, L0-e to L0-g). Without items, the original packs and index come back."""
    if not keystream.is_file():
        return {'hfs': 'skipped: no keystream', 'keystream': str(keystream)}
    ks = hfs.load_keystream(keystream)
    backup_dir = client_copy / 'lab-backup'
    added_list = backup_dir / 'added-packs.json'
    index = client_copy / INDEX
    texts = [(client_copy / 'hfs' / (hfs.pack_name(f) + '.hfs'), f.rsplit('/', 1)[1]) for f in TEXT_FOLDERS]
    scripts = client_copy / 'hfs' / (hfs.pack_name(SCRIPTS_FOLDER) + '.hfs')
    merges = class_merges(assets)
    original_index = backup_dir / index.name if (backup_dir / index.name).exists() else index
    merged = merge_packs(hfs.decode_index(original_index.read_bytes()), merges, client_copy) if merges else {}
    class_files = class_asset_files(assets)
    injected = injected_packs(hfs.decode_index(original_index.read_bytes()), class_files, client_copy)
    injected_paths = {path for pack in injected.values() for path, _, _ in pack}
    fixed = fixed_packs(hfs.decode_index(original_index.read_bytes()), client_copy) if class_files else {}
    class_files = [f for f in class_files if f[0] not in injected_paths]
    # Every pack with a backup comes back to it unless rebuilt below (a merge that was undone).
    restored = [client_copy / 'hfs' / p.name for p in backup_dir.glob('*.hfs') if p.name != index.name]
    originals = {path: backup_dir / path.name
                 for path in [index, scripts] + [p for p, _ in texts] + sorted(merged) + sorted(injected) + sorted(fixed) + restored}
    previously_added = json.loads(added_list.read_text(encoding='utf-8')) if added_list.exists() else []
    entries = content_additions.text_entries(items, overrides)
    costumes = [c for item in items for c in item.get('costumes', [])]
    costumes += class_weapon_costumes(assets)
    class_tokens = class_imports.class_text(assets / 'classes', class_imports.load_imports(content_additions.CLASS_IMPORTS))
    if (not items and not entries and not class_files and not merges and not class_tokens
            and not any(b.exists() for b in originals.values()) and not previously_added):
        return {'hfs': 'unchanged: no added items'}
    if apply_changes:
        backup_dir.mkdir(exist_ok=True)
        for path, backup in originals.items():
            if not backup.exists():
                backup.write_bytes(path.read_bytes())
    source = {path: (backup if backup.exists() else path).read_bytes() for path, backup in originals.items()}
    wanted = dict(source)
    rebuilt = []
    if entries or class_tokens:
        for path, language in texts:
            pack = []
            for e in hfs.read_pack(source[path], ks):
                if e.name == f'heroes_text_{language}.txt':
                    # Class tokens only where the 1.69 has no line; the item editor's replace theirs.
                    present = {k.lower() for k, _ in class_imports.TOKEN_LINE.findall(e.data.decode('utf-16'))}
                    added = {k: v for k, v in class_tokens.items() if k.lower() not in present}
                    e = hfs.Entry(e.name, with_tokens(e.data, {**added, **entries}), e.attr)
                pack.append(e)
            wanted[path] = hfs.build_pack(pack, ks)
            rebuilt.append((path.name, wanted[path]))
    by_file: dict[str, list[dict]] = {}
    for costume in costumes:
        by_file.setdefault(script_of(costume['section']), []).append(costume)
    merge_report = {'files': 0, 'blocks': 0, 'lines': 0, 'conflicts': [], 'renumbered': {}}
    # Class merges first (NA lines of the class), then the item editor's entries on top.
    fix_report = []
    original_plain = hfs.decode_index(original_index.read_bytes())

    def read_original(game_path: str) -> bytes:
        """An original 1.69 file, from its pack's backup when the copy has one."""
        pack = original_pack(original_plain, game_path, client_copy)
        raw = (backup_dir / pack.name).read_bytes() if (backup_dir / pack.name).exists() else pack.read_bytes()
        name = game_path.rsplit('/', 1)[1].lower()
        for e in hfs.read_pack(raw, ks):
            if e.name.lower() in (name, name + '.comp'):
                return hfs.comp_unwrap(e.data) if e.name.lower().endswith('.comp') else e.data
        raise PreparationError(f'{pack.name} has no {name}.')

    for path in sorted(set(merged) | set(injected) | set(fixed) | ({scripts} if by_file else set())):
        pending = dict(merged.get(path, {}))
        fixes = dict(fixed.get(path, {}))
        pack = []
        for e in hfs.read_pack(source[path], ks):
            comp = e.name.endswith('.comp')
            file_name = e.name[:-5] if comp else e.name
            sources = pending.pop(file_name, [])
            rows = by_file.pop(e.name, None) if path == scripts else None
            fix = fixes.pop(file_name.lower(), None)
            if fix:
                data = fix(hfs.comp_unwrap(e.data) if comp else e.data, read_original)
                if data is not None:
                    e = hfs.Entry(e.name, hfs.comp_wrap(data) if comp else data, e.attr)
                    fix_report.append(file_name)
            if sources or rows:
                plain = hfs.comp_unwrap(e.data) if comp else e.data
                for file, needles in sources:
                    na = file.read_bytes()
                    if file_name == 'player_costume.txt':
                        remap = class_costumes.costume_remap(na, plain, class_costumes.lab_taken(items), needles)
                        na = class_costumes.rename(na, remap)
                        merge_report['renumbered'] = {f'{s}/{k}': v for (s, k), v in sorted(remap.items())}
                    plain, report = kv_merge.merge(plain, na, needles)
                    merge_report['files'] += 1
                    merge_report['blocks'] += report['blocks']
                    merge_report['lines'] += report['lines']
                    merge_report['conflicts'] += [f'{file_name}:{c}' for c in report['conflicts']]
                if rows and e.name == 'player_costume.txt.comp':
                    plain = with_costumes(plain, rows)
                elif rows:
                    plain = with_script_entries(plain, rows, 'shields' if e.name == 'shields.txt.comp' else 'weapon')
                e = hfs.Entry(e.name, hfs.comp_wrap(plain) if comp else plain, e.attr)
            pack.append(e)
        if pending or fixes:
            raise PreparationError(f'{path.name} has no {sorted(pending or fixes)[0]}.')
        present = {e.name.lower() for e in pack}
        for game_path, file, _ in injected.get(path, []):
            folder, name = game_path.rsplit('/', 1)
            entry_name, data, attr = class_entry(folder, name, file)
            if entry_name.lower() in present:
                raise PreparationError(f'{path.name} already has {entry_name}.')
            pack.append(hfs.Entry(entry_name, data(), attr))
        wanted[path] = hfs.build_pack(pack, ks)
        rebuilt.append((path.name, wanted[path]))
    if by_file:
        raise PreparationError(f'The scripts pack has no {sorted(by_file)[0]}.')
    plain = hfs.decode_index(source[index])
    # NA classes first (gigabytes, written pack by pack), then the item editor's packs after them.
    plan = class_pack_plan(class_files, set(hfs.index_packs(plain)))
    listed, class_built = class_packs(client_copy, plan, ks, apply_changes)
    taken = set(hfs.index_packs(plain)) | {name for name, _, _ in plan}
    new_packs = {}
    for name, pack in asset_packs(items, taken, assets).items():
        path = client_copy / 'hfs' / (name + '.hfs')
        new_packs[path] = hfs.build_pack(pack, ks)
        rebuilt.append((path.name, new_packs[path]))
    if rebuilt or listed:
        plain, _ = hfs.sync_listed(plain, [(name, hfs.pack_files(raw, ks)) for name, raw in rebuilt] + listed)
        wanted[index] = hfs.encode_index(plain, source[index][:hfs.INDEX_HEAD])
    wanted.update(new_packs)
    changed = [p for p, data in wanted.items() if not p.exists() or p.read_bytes() != data]
    added = sorted([p.name for p in new_packs] + [name + '.hfs' for name, _, _ in plan])
    stale = [client_copy / 'hfs' / name for name in previously_added if name not in added]
    if apply_changes:
        for path in changed:
            path.write_bytes(wanted[path])
        for path in stale:
            path.unlink(missing_ok=True)
        added_list.write_text(json.dumps(added), encoding='utf-8')
    return {'hfs': 'applied' if items or entries or class_files or merges or class_tokens else 'restored', 'text_tokens': len(entries),
            'class_tokens': len(class_tokens),
            'costumes': len(costumes), 'new_packs': sorted(p.name for p in new_packs), 'class_files': len(class_files),
            'class_packs': len(plan), 'class_injected': len(injected_paths), 'class_packs_built': class_built, 'class_merges': merge_report, 'original_fixes': fix_report,
            'hfs_files_changed': len(changed), 'stale_packs': len(stale)}


def apply(client_copy: Path, apply_changes: bool = False, additions: Path = ADDITIONS, edits: dict | None = None,
          items: list[dict] | None = None, keystream: Path | None = None) -> dict:
    database = reject_original(client_copy / 'sql/heroes.db3')
    if not database.is_file():
        raise PreparationError('The client copy must contain sql/heroes.db3.')
    rows = json.loads(additions.read_text(encoding='utf-8')).get('ShopInfo', [])
    edits = content_edits.EMPTY if edits is None else edits
    backup = database.with_name('heroes.db3.pre-lab-additions')
    # What the last run wrote for the item editor (added classes and overridden base classes).
    written = database.with_name('heroes.db3.lab-items.json')
    with closing(sqlite3.connect(database)) as db:
        current = rows_of(db)
        if backup.exists():
            with closing(sqlite3.connect(backup.resolve().as_uri() + '?mode=ro', uri=True)) as original_db:
                original = rows_of(original_db)
        else:
            original = current
        target = target_rows(original, rows, edits)
        plan = {'mode': 'apply' if apply_changes else 'dry-run', 'database': str(database),
                'shop_rows_missing': len(missing_rows(db, rows)), 'shop_rows': len(rows),
                'edits_version': edits.get('version', 0), 'changed': current != target}
        items = content_additions.load(edits=edits) if items is None else items
        overrides = content_additions.load_overrides(edits)
        previous = json.loads(written.read_text(encoding='utf-8')) if written.exists() else {}
        plan['items'] = len(items)
        plan['item_overrides'] = len(overrides)
        imports = class_imports.load_imports(content_additions.CLASS_IMPORTS)
        plan['class_imports'] = [p.stem for p in imports]
        if apply_changes and (items or overrides or imports) and not backup.exists():
            with closing(sqlite3.connect(backup)) as copy:
                db.backup(copy)
        if apply_changes:
            # B4.5 tuning cells; already-applied values change nothing.
            with db:
                plan['tuning_cells'] = content_edits.apply_client_tuning(db, edits)
            # Item editor: added items replace their earlier copies, removed ones leave, and base
            # items take their overrides over the original cells.
            original = sqlite3.connect(backup.resolve().as_uri() + '?mode=ro', uri=True) if backup.exists() else None
            try:
                with db:
                    # Imported NA classes first (their tables come back to the original rows), then the
                    # item editor's rows on top.
                    if imports or previous.get('imported'):
                        plan['class_rows'] = class_imports.apply_client(db, original, imports,
                                                                        content_additions.class_remap(imports, items))
                    plan['item_rows'] = content_additions.apply_client(db, items, overrides, original, previous)
            finally:
                if original is not None:
                    original.close()
            written.write_text(json.dumps({'added': sorted(i['ItemClass'] for i in items),
                                           'overridden': sorted(overrides),
                                           'imported': [p.stem for p in imports]}), encoding='utf-8')
        if apply_changes and current != target:
            if not backup.exists():
                with closing(sqlite3.connect(backup)) as copy:
                    db.backup(copy)
            with db:
                db.execute('DELETE FROM ShopInfo')
                db.executemany('INSERT INTO ShopInfo(' + ','.join('[%s]' % c for c in SHOP_COLUMNS) + ') VALUES(' +
                               ','.join('?' * len(SHOP_COLUMNS)) + ')', [tuple(r[c] for c in SHOP_COLUMNS) for r in target])
            plan['backup'] = str(backup)
    keystream = client_copy / 'hfs-keystream.bin' if keystream is None else keystream
    plan.update(apply_hfs(client_copy, items, keystream, apply_changes, overrides=overrides))
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--edits', type=Path, default=content_edits.EDITS,
                        help='published panel edits (content-edits-export); missing = none')
    args = parser.parse_args()
    try:
        print(json.dumps(apply(args.client_copy, args.apply, edits=content_edits.load(args.edits))))
    except (PreparationError, ValueError) as error:
        print(json.dumps({'applied': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
