"""Add the laboratory content rows (scripts/content/lab_content_additions.json) and the published
admin panel edits (.cache/private-content/content-edits.json, B4.4) to a COPY of the client's
sql/heroes.db3, so the client shows the NPC shops the server sells (for example the Ferghus row
of Enhancement Elixir, or a panel price change). ShopInfo is rebuilt from the backup of the
original table each time, so a removed or changed row is undone when its edit is. Dry-run by
default; idempotent; the original clients are refused and a consistent backup is kept before
the first change."""
from __future__ import annotations
import argparse
import json
import re
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content'))
import content_additions  # noqa: E402
import content_edits  # noqa: E402
import hfs  # noqa: E402

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
    """The localization file (UTF-16 LE with BOM) with the entries first in its Tokens block."""
    decoded = text.decode('utf-16')
    tokens = decoded.index('"Tokens"')
    opening = decoded.index('{\r\n', tokens) + 3
    lines = ''.join('\t\t"%s"\t"%s"\r\n' % (key, value) for key, value in sorted(entries.items()))
    return (decoded[:opening] + lines + decoded[opening:]).encode('utf-16')


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
    for item in items:
        if not item.get('assets'):
            continue
        root = (assets / item['assets']).resolve()
        if not root.is_relative_to(assets.resolve()) or not root.is_dir():
            raise PreparationError(f'Missing asset folder {item["assets"]}.')
        for path in sorted(p for p in root.rglob('*') if p.is_file()):
            folder = path.parent.relative_to(root).as_posix().lower()
            folders.setdefault(folder, []).append(hfs.Entry(path.name.lower() + '.comp', hfs.comp_wrap(path.read_bytes()), hfs.DEFAULT_ATTR))
    packs = {}
    for folder, entries in folders.items():
        part = 0
        while hfs.pack_name(folder, part) in index_packs:
            part += 1
        packs[hfs.pack_name(folder, part)] = entries
    return packs


def apply_hfs(client_copy: Path, items: list[dict], keystream: Path, apply_changes: bool, assets: Path = ASSETS) -> dict:
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
    originals = {path: backup_dir / path.name for path in [index, scripts] + [p for p, _ in texts]}
    previously_added = json.loads(added_list.read_text(encoding='utf-8')) if added_list.exists() else []
    entries = content_additions.text_entries(items)
    costumes = [c for item in items for c in item.get('costumes', [])]
    if not items and not any(b.exists() for b in originals.values()) and not previously_added:
        return {'hfs': 'unchanged: no added items'}
    if apply_changes:
        backup_dir.mkdir(exist_ok=True)
        for path, backup in originals.items():
            if not backup.exists():
                backup.write_bytes(path.read_bytes())
    source = {path: (backup if backup.exists() else path).read_bytes() for path, backup in originals.items()}
    wanted = dict(source)
    rebuilt = []
    if entries:
        for path, language in texts:
            pack = [hfs.Entry(e.name, with_tokens(e.data, entries) if e.name == f'heroes_text_{language}.txt' else e.data, e.attr)
                    for e in hfs.read_pack(source[path], ks)]
            wanted[path] = hfs.build_pack(pack, ks)
            rebuilt.append((path.name, wanted[path]))
    if costumes:
        pack = [hfs.Entry(e.name, hfs.comp_wrap(with_costumes(hfs.comp_unwrap(e.data), costumes))
                          if e.name == 'player_costume.txt.comp' else e.data, e.attr) for e in hfs.read_pack(source[scripts], ks)]
        wanted[scripts] = hfs.build_pack(pack, ks)
        rebuilt.append((scripts.name, wanted[scripts]))
    plain = hfs.decode_index(source[index])
    new_packs = {}
    for name, pack in asset_packs(items, set(hfs.index_packs(plain)), assets).items():
        path = client_copy / 'hfs' / (name + '.hfs')
        new_packs[path] = hfs.build_pack(pack, ks)
        rebuilt.append((path.name, new_packs[path]))
    if rebuilt:
        plain, _ = hfs.sync(plain, rebuilt, ks)
        wanted[index] = hfs.encode_index(plain, source[index][:hfs.INDEX_HEAD])
    wanted.update(new_packs)
    changed = [p for p, data in wanted.items() if not p.exists() or p.read_bytes() != data]
    stale = [client_copy / 'hfs' / name for name in previously_added if client_copy / 'hfs' / name not in new_packs]
    if apply_changes:
        for path in changed:
            path.write_bytes(wanted[path])
        for path in stale:
            path.unlink(missing_ok=True)
        added_list.write_text(json.dumps(sorted(p.name for p in new_packs)), encoding='utf-8')
    return {'hfs': 'applied' if items else 'restored', 'text_tokens': len(entries), 'costumes': len(costumes),
            'new_packs': sorted(p.name for p in new_packs), 'hfs_files_changed': len(changed), 'stale_packs': len(stale)}


def apply(client_copy: Path, apply_changes: bool = False, additions: Path = ADDITIONS, edits: dict | None = None,
          items: list[dict] | None = None, keystream: Path | None = None) -> dict:
    database = reject_original(client_copy / 'sql/heroes.db3')
    if not database.is_file():
        raise PreparationError('The client copy must contain sql/heroes.db3.')
    rows = json.loads(additions.read_text(encoding='utf-8')).get('ShopInfo', [])
    edits = content_edits.EMPTY if edits is None else edits
    backup = database.with_name('heroes.db3.pre-lab-additions')
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
        items = content_additions.load() if items is None else items
        plan['items'] = len(items)
        if apply_changes and items and not backup.exists():
            with closing(sqlite3.connect(backup)) as copy:
                db.backup(copy)
        if apply_changes:
            # B4.5 tuning cells; already-applied values change nothing.
            with db:
                plan['tuning_cells'] = content_edits.apply_client_tuning(db, edits)
            # Item editor cut 1: added items replace their earlier copies.
            with db:
                plan['item_rows'] = content_additions.apply_client(db, items)
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
    plan.update(apply_hfs(client_copy, items, keystream, apply_changes))
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
    except PreparationError as error:
        print(json.dumps({'applied': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
