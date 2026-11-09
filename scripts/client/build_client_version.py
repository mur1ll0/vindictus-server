"""Recipe of the client versions the launcher distributes (docs/future/game-launcher.md 4.7).

    py -3.13 scripts/client/build_client_version.py --version 1|2|3 --out .cache/client-versions/v<N>

Builds, in a NEW folder, version N from the 1.69 EU original (read only) and the code of this commit:
  v1 = 1.69 EU + everything that is not a new class: the good Mss32.dll (recover_mss32.py), the ProudNet
       IOCP fix of engine.dll, the cash shop list limit and the launcher login of client.dll, the
       Messenger's nmcogame.dll (tools/nmcogame), the EndPoint, the original content values, the server's
       story goals, the B1-B5 content, the collabs and the published panel edits (heroes.db3 and packs);
  v2 = v1 + the Delia (class 10): .hcls/.hrte, the engine item byte, her creation movie and town motion;
  v3 = v2 + the Miri (class 11, NA assets staged in .cache/private-assets/classes/miri).
The item editor's test items (TEST_ITEMS) stay out of every version; their costume numbers stay taken.

Steps, in the order of docs/client-dll-patches.md: copy the original minus EXCLUDED (never the player's
generated files), DLL chains, Mss32.dll, nmcogame.dll, heroes.db3 (EndPoint, original values, story goals),
the Delia movie (v2+), the content (apply_lab_content_additions.apply with the class layer), then the
build's own leftovers (backups, lab-backup/, reports) are removed: the folder holds only what is
distributed. The report (<out>/../v<N>.report.json by default) has every file's SHA-256 and size, which
files differ from the original, the PRESERVE marks and each step's result. The DLL hashes are checked
against the chain's known values; the output is deterministic for the same original, inputs and commit.
"""
from __future__ import annotations
import argparse
import contextlib
import fnmatch
import hashlib
import ipaddress
import json
import os
import shutil
import sqlite3
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import class_layers
from prepare_lab_client import WORKSPACE, PreparationError, reject_original

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = Path(__file__).resolve().parent
ORIGINAL = WORKSPACE / 'Vindictus1.69EN/Vindictus EU/en-EU'
KEYSTREAM = ROOT / '.cache/play-client/hfs-keystream.bin'   # read only; derived once (hfs.py keystream)
STATE_NAME = 'client-build.json'   # lab-backup/<this> marks a tree built with keep_state
# sql/ files of the content step (apply_lab_content_additions): the database before it and what it wrote.
DATABASE_BACKUP, DATABASE_ITEMS = 'heroes.db3.pre-lab-additions', 'heroes.db3.lab-items.json'
KEYSTREAM_SHA256 ='8af3e7e346c5a22d2c2f8584234ef16183fd6421547841fb114120ab7e23ab5b'
NMCOGAME = ROOT / 'tools/nmcogame/target/i686-win7-windows-msvc/release/nmcogame.dll'  # tools/nmcogame/build.ps1
HASH_CACHE = ROOT / '.cache/client-versions/original-sha256.json'
# Lab endpoint (prepare_lab_client.py defaults: the frontend of scripts/dev/start-lab-frontend.ps1).
ENDPOINT = {'region': 'EN-EU-X-GM', 'base_url': 'http://127.0.0.1:8081', 'game_host': '127.0.0.1', 'game_port': 27015}
# Item editor test items (operator decision of 07/11/2026): never distributed.
TEST_ITEMS = ('fiona_inner_lab', 'fiona_inner_fringe', 'fiona_inner_zero', 'evy_inner_zero')

# Exclusions (game-launcher.md 4.3 item 6, D-L3 and the additions of 07/11/2026): folders anywhere in the
# tree, then globs over the relative path (or the file name when the glob has no '/').
# Not BlackCipher: Vindictus.exe imports NGClient.aes, which refuses to start the game without BlackCipher/*.aes
# (LV of 08/11/2026); its logs already fall under *.log.
EXCLUDED_DIRS = ('hfs_extracted', 'lab-backup', 'passport', 'downloads', 'DownloadLists', '__pycache__')
EXCLUDED_GLOBS = ('l0-*', '*.bak', '*.pre-*.bak', '*.log', 'npp_*_std*.log', 'fmod.log', '*.nexon', '*.partial',
                  'NMService.exe', '_eu_AdBalloonExt.exe', 'VindictusLauncher.exe', 'bcdedit.exe', 'start*.bat',
                  'maps/graphs/*.ain', 'demoheader.tmp', 'hfs-keystream.bin', 'cfg/config_pendingkey_concmd.cfg',
                  'bin/class-routing.json', 'sql/heroes.db3.pre-*', 'sql/heroes.db3.lab-items.json', 'nmcogame.ini',
                  'vindictus-launcher.*', 'bin/config_material.txt')
# Player files: installed when missing, never checked nor overwritten (manifest `preservar`).
PRESERVE_GLOBS = ('SAVE/*', 'cfg/config.cfg', 'cfg/config_env.cfg', 'cfg/config_ex.cfg', 'cfg/language.cfg',
                  'cfg/config_*_ex.cfg')

# SHA-256 (first 16 hex digits) of each DLL after its chain, per version (docs/client-dll-patches.md).
DLL_RESULTS = {
    1: {'client.dll': '57ea793688bcf97f', 'server.dll': 'd7727e182f6cdec1', 'engine.dll': '60c3f4aaf3da6b0a'},
    2: {'client.dll': 'bb6852295cff9642', 'server.dll': '651527214e9356a1', 'engine.dll': 'c4e97a77bd8b2c28'},
    3: {'client.dll': '59b60e7a1fd5cc74', 'server.dll': '5f0429365ee0ccd1', 'engine.dll': 'df6c70d2d66f52ed'},
}


def matches(path: str, globs: tuple[str, ...]) -> bool:
    name = path.rsplit('/', 1)[-1]
    return any(fnmatch.fnmatchcase(path.lower(), g.lower()) or ('/' not in g and fnmatch.fnmatchcase(name.lower(), g.lower()))
               for g in globs)


def excluded(path: str) -> bool:
    return any(part in EXCLUDED_DIRS for part in path.split('/')[:-1]) or matches(path, EXCLUDED_GLOBS)


def tree(base: Path) -> dict[str, os.stat_result]:
    """Relative path -> stat of every file outside the excluded folders."""
    out = {}
    for folder, dirs, files in os.walk(base):
        dirs[:] = sorted(d for d in dirs if d not in EXCLUDED_DIRS)
        rel = Path(folder).relative_to(base).as_posix()
        for name in sorted(files):
            path = name if rel == '.' else f'{rel}/{name}'
            out[path] = os.stat(Path(folder) / name)
    return out


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(8 << 20), b''):
            h.update(block)
    return h.hexdigest()


def original_hashes(original: Path, files: dict[str, os.stat_result]) -> dict[str, str]:
    """SHA-256 of the original's files, cached by path, size and mtime (the original never changes)."""
    cache = json.loads(HASH_CACHE.read_text(encoding='utf-8')) if HASH_CACHE.is_file() else {}
    known = cache.get(str(original), {})
    out, fresh = {}, False
    for path, st in files.items():
        entry = known.get(path)
        if entry and entry[0] == st.st_size and entry[1] == st.st_mtime_ns:
            out[path] = entry[2]
        else:
            out[path] = sha256_file(original / path)
            known[path] = [st.st_size, st.st_mtime_ns, out[path]]
            fresh = True
    if fresh:
        cache[str(original)] = known
        HASH_CACHE.parent.mkdir(parents=True, exist_ok=True)
        HASH_CACHE.write_bytes(json.dumps(cache).encode('utf-8'))
    return out


def git_state() -> dict:
    def git(*args: str) -> str:
        try:
            return subprocess.run(['git', *args], cwd=ROOT, capture_output=True, text=True, check=False).stdout.strip()
        except OSError:  # the client builder image has no git: the image records the commit
            return os.environ.get('VINDICTUS_RECIPE_COMMIT', '') if args[0] == 'rev-parse' else ''
    return {'commit': git('rev-parse', 'HEAD'),
            'dirty_recipe_files': git('status', '--porcelain', '--', 'scripts/client', 'scripts/content').splitlines()}


def run_script(name: str, *args: str) -> dict:
    """A script that only has a command line (prints one JSON object)."""
    done = subprocess.run([sys.executable, '-s', '-E', str(SCRIPTS / name), *args], capture_output=True, text=True,
                          check=False, cwd=ROOT)
    try:
        result = json.loads(done.stdout.strip().splitlines()[-1])
    except (IndexError, json.JSONDecodeError):
        result = {'stdout': done.stdout[-2000:]}
    if done.returncode != 0:
        raise PreparationError(f'{name} failed ({done.returncode}): {done.stderr[-2000:] or result}')
    return result


def dll_sha(out: Path, name: str) -> str:
    return sha256_file(out / 'bin' / name)


def patch_dlls(out: Path, version: int, classes: tuple[str, ...]) -> dict:
    import patch_class_routing
    import patch_class_table
    import patch_client_launcher_login
    import patch_engine_class_flags
    import patch_engine_iocp_sign
    import patch_shop_list_limit
    steps = {'client.dll': [], 'server.dll': [], 'engine.dll': []}

    def step(dll: str, name: str, result: dict) -> None:
        steps[dll].append({'script': name, **{k: v for k, v in result.items()
                                             if k in ('before_sha256', 'after_sha256', 'already_patched', 'active_classes', 'classes')}})

    step('client.dll', 'patch_shop_list_limit.py', patch_shop_list_limit.patch(out, True))
    if classes:
        plan = patch_class_table.patch(out, True, classes)
        for d in plan['dlls']:
            step(Path(d['dll']).name, 'patch_class_table.py', {**d, 'active_classes': plan['active_classes']})
        plan = patch_class_routing.patch(out, True, classes=classes)
        for d in plan['dlls']:
            step(Path(d['dll']).name, 'patch_class_routing.py', {**d, 'classes': plan['classes']})
    step('client.dll', 'patch_client_launcher_login.py', patch_client_launcher_login.patch(out, True))
    if classes:
        step('engine.dll', 'patch_engine_class_flags.py', patch_engine_class_flags.patch(out, True, classes))
    step('engine.dll', 'patch_engine_iocp_sign.py', patch_engine_iocp_sign.patch(out, True))
    for dll, expected in DLL_RESULTS[version].items():
        found = dll_sha(out, dll)
        steps[dll].append({'result_sha256': found})
        if expected and not found.startswith(expected):
            raise PreparationError(f'v{version} {dll} is {found[:16]}, expected {expected}.')
    return steps


@contextlib.contextmanager
def base_costume(original: Path, keystream: bytes, work: Path):
    """content_additions.BASE_COSTUME from the original scripts pack instead of the lab's extraction."""
    import content_additions
    import hfs
    pack = original / 'hfs' / (hfs.pack_name('scripts') + '.hfs')
    entry = next(e for e in hfs.read_pack(pack.read_bytes(), keystream) if e.name.lower() == 'player_costume.txt.comp')
    path = work / 'player_costume.txt'
    path.write_bytes(hfs.comp_unwrap(entry.data))
    saved = content_additions.BASE_COSTUME
    content_additions.BASE_COSTUME = path
    try:
        yield hashlib.sha256(path.read_bytes()).hexdigest()
    finally:
        content_additions.BASE_COSTUME = saved


def build(version: int, out: Path, original: Path = ORIGINAL, keystream: Path = KEYSTREAM, edits: Path | None = None,
          nmcogame: Path = NMCOGAME, keep_test_items: bool = False, report: Path | None = None,
          link_from: Path | None = None, endpoint_host: str = '127.0.0.1', keep_state: bool = False) -> dict:
    """keep_state: the server's client tree (game-launcher.md 4.10): the backups and lab-backup/ stay so
    refresh() can apply the content again; no per-file report (the client builder hashes the tree)."""
    sys.path.insert(0, str(ROOT / 'scripts/content'))
    import apply_delia_creation_movie
    import content_edits
    import prepare_lab_client
    import recover_mss32
    classes = class_layers.VERSIONS[version]
    host = str(ipaddress.IPv4Address(endpoint_host))  # the bootstrap the client asks for its server list
    endpoint = {**ENDPOINT, 'base_url': f'http://{host}:8081', 'game_host': host}
    out = reject_original(out)
    if out == (ROOT / '.cache/play-client').resolve() or (out.exists() and any(out.iterdir())):
        raise PreparationError(f'{out} must be a new or empty folder (never the lab copy).')
    for path, what in ((original / 'bin/client.dll', 'the 1.69 EU original'), (keystream, 'the pack keystream'),
                       (nmcogame, 'nmcogame.dll (tools/nmcogame/build.ps1)')):
        if not path.is_file():
            raise PreparationError(f'Missing {what}: {path}.')
    if sha256_file(keystream) != KEYSTREAM_SHA256:
        raise PreparationError('The keystream differs from the recorded one.')
    edits = content_edits.EDITS if edits is None else edits
    out.mkdir(parents=True, exist_ok=True)
    started = time.time()
    result = {'version': version, 'classes': list(classes), 'out': str(out), 'original': str(original),
              'endpoint_host': host,
              'git': git_state(), 'test_items_excluded': [] if keep_test_items else list(TEST_ITEMS), 'steps': {}}

    # 1. Copy the original minus the exclusions.
    source = tree(original)
    copied = {p: st for p, st in source.items() if not excluded(p)}
    for path in copied:
        target = out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(original / path, target)
    after_copy = {p: (s.st_size, s.st_mtime_ns) for p, s in tree(out).items()}
    result['steps']['copy'] = {'files': len(copied), 'bytes': sum(s.st_size for s in copied.values()),
                               'excluded': sorted(set(source) - set(copied))}

    with tempfile.TemporaryDirectory(prefix=f'client-v{version}-', dir=out.parent) as tmp:
        work = Path(tmp)
        # 2. DLL chains.
        result['steps']['dlls'] = patch_dlls(out, version, classes)
        # 3. Mss32.dll and nmcogame.dll.
        result['steps']['mss32'] = recover_mss32.apply(out, apply_changes=True)
        shutil.copyfile(nmcogame, out / 'nmcogame.dll')
        result['steps']['nmcogame'] = {'source': str(nmcogame), 'sha256': sha256_file(out / 'nmcogame.dll')}
        # 4. heroes.db3: EndPoint (its EndPoint.txt goes to a throwaway web root), original values, story goals.
        result['steps']['endpoint'] = {k: v for k, v in prepare_lab_client.prepare(
            out, work / 'web', endpoint['region'], endpoint['base_url'], endpoint['game_host'], endpoint['game_port'],
            True).items() if k in ('previous_bootstrap_url', 'new_bootstrap_url')}
        result['steps']['original_content'] = run_script('apply_original_content.py', '--client-copy', str(out), '--apply')
        result['steps']['story_goals'] = run_script('align_story_goals.py', '--client-copy', str(out), '--apply')
        # 5. The Delia's customization movie (v2+), before the Miri's sliders go on top of it.
        if 'delia' in classes:
            result['steps']['delia_movie'] = apply_delia_creation_movie.apply(out, keystream, True)
        # 6. Content of the layer: shops, panel edits, items, collabs, classes, packs and index.
        result['steps']['content'] = apply_content(out, classes, original, keystream, edits, keep_test_items, work)

    if keep_state:
        # The server's client tree (client builder, game-launcher.md 4.10): the backups stay, so the content
        # step can run again over it (refresh); the distributed files are those outside the exclusions.
        (out / 'lab-backup').mkdir(exist_ok=True)
        (out / 'lab-backup' / STATE_NAME).write_bytes(json.dumps(
            {'version': version, 'keep_test_items': keep_test_items, 'commit': result['git']['commit']}).encode('utf-8'))
    removed = [] if keep_state else prune(out)
    result['steps']['pruned'] = sorted(removed)
    if keep_state:
        result['totals'] = {'seconds': round(time.time() - started)}
        report = out.parent / f'{out.name}.report.json' if report is None else report
        report.write_bytes(json.dumps(result, indent=1).encode('utf-8'))
        return result

    # 8. Report: every file, which ones differ from the original, the player's files.
    return finish_report(result, out, original, source, copied, after_copy, started, report, link_from)


def apply_content(out: Path, classes: tuple[str, ...], original: Path, keystream: Path, edits: Path,
                  keep_test_items: bool, work: Path, sql_dir: Path | None = None) -> dict:
    """Step 6: shops, panel edits, items, collabs, classes, packs and index (idempotent: the packs and the
    database come back from the backups the first run kept, then the edits go on top). `sql_dir`: a copy
    of the tree's sql/ to work on (refresh)."""
    import apply_lab_content_additions
    import content_edits
    import hfs
    ks = hfs.load_keystream(keystream)
    with base_costume(original, ks, work) as costume_sha:
        plan = apply_lab_content_additions.apply(
            out, True, edits=content_edits.load(edits), keystream=keystream, classes=classes,
            exclude_items=() if keep_test_items else TEST_ITEMS, sql_dir=sql_dir)
    return {'base_costume_sha256': costume_sha, 'edits': str(edits),
            'edits_sha256': sha256_file(edits) if edits.is_file() else None, **plan}


def refresh(out: Path, original: Path = ORIGINAL, keystream: Path = KEYSTREAM, edits: Path | None = None) -> dict:
    """Runs only the content step again over a tree built with keep_state (the server's client tree): the
    published panel edits change heroes.db3, the item packs and the index without rebuilding the client."""
    sys.path.insert(0, str(ROOT / 'scripts/content'))
    import content_edits
    out = reject_original(out)
    marker = out / 'lab-backup' / STATE_NAME
    if not marker.is_file():
        raise PreparationError(f'{out} was not built with --keep-state (no lab-backup/{STATE_NAME}).')
    state = json.loads(marker.read_text(encoding='utf-8'))
    version = int(state['version'])
    classes = class_layers.VERSIONS[version]
    for path, what in ((original / 'bin/client.dll', 'the 1.69 EU original'), (keystream, 'the pack keystream')):
        if not path.is_file():
            raise PreparationError(f'Missing {what}: {path}.')
    if sha256_file(keystream) != KEYSTREAM_SHA256:
        raise PreparationError('The keystream differs from the recorded one.')
    edits = content_edits.EDITS if edits is None else edits
    started = time.time()
    database = out / 'sql/heroes.db3'
    # The database step runs on a local copy (the system's temporary folder: the container's own tmpfs in
    # the client builder) and starts again from the build's own backup, the database before the content
    # step. Only a different content goes back into the tree: SQLite lays the pages out by its version (the
    # image's 3.40 and the host's 3.50 write different bytes for the same rows), so the tree keeps the
    # published bytes when the content did not change and the draft gets no heroes.db3.
    with tempfile.TemporaryDirectory(prefix=f'client-refresh-v{version}-') as tmp:
        sql = Path(tmp) / 'sql'
        sql.mkdir()
        for name in (DATABASE_BACKUP, DATABASE_ITEMS):
            if (out / 'sql' / name).is_file():
                shutil.copyfile(out / 'sql' / name, sql / name)
        shutil.copyfile(sql / DATABASE_BACKUP if (sql / DATABASE_BACKUP).is_file() else database, sql / 'heroes.db3')
        (Path(tmp) / 'work').mkdir()
        content = apply_content(out, classes, original, keystream, edits, bool(state.get('keep_test_items')),
                                Path(tmp) / 'work', sql_dir=sql)
        content['database'] = 'written' if install_database(sql / 'heroes.db3', database) else 'same content, kept'
        for name in (DATABASE_BACKUP, DATABASE_ITEMS):
            if (sql / name).is_file() and not (name == DATABASE_BACKUP and (out / 'sql' / name).is_file()):
                shutil.copyfile(sql / name, out / 'sql' / name)
    return {'version': version, 'classes': list(classes), 'out': str(out), 'git': git_state(),
            'steps': {'content': content}, 'totals': {'seconds': round(time.time() - started)}}


def database_digest(path: Path) -> str:
    """SHA-256 of what the client reads from a SQLite database: the schema (without root pages), the
    user_version, application_id and encoding, and every table's rows in rowid order with their rowids and
    value types. The page layout and the header's counters and SQLite version are left out."""
    h = hashlib.sha256()
    with contextlib.closing(sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.text_factory = lambda raw: ('text', raw)  # text and blobs apart, and text that is not UTF-8 as is
        for pragma in ('user_version', 'application_id', 'encoding'):
            h.update(repr((pragma, db.execute(f'PRAGMA {pragma}').fetchone())).encode())
        for row in db.execute('SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name'):
            h.update(repr(('schema', row)).encode())
        tables = [row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
        for name in tables:
            quoted = '"' + name[1].decode('utf-8', 'surrogateescape').replace('"', '""') + '"'
            h.update(repr(('table', name)).encode())
            try:
                rows = db.execute(f'SELECT rowid, * FROM {quoted} ORDER BY rowid')
            except sqlite3.OperationalError:  # WITHOUT ROWID: a scan follows the primary key
                rows = db.execute(f'SELECT * FROM {quoted}')
            for row in rows:
                h.update(repr(row).encode())
    return h.hexdigest()


def install_database(built: Path, target: Path) -> bool:
    """The content step's database into the tree, unless the tree's has the same content (database_digest):
    then the tree keeps its bytes. True when written."""
    if target.is_file() and database_digest(built) == database_digest(target):
        return False
    partial = target.with_name(target.name + '.partial')
    shutil.copyfile(built, partial)
    os.replace(partial, target)
    return True


def prune(out: Path) -> list[str]:
    """Step 7: the build's leftovers leave the folder (backups, lab-backup/, reports)."""
    removed = []
    for folder, dirs, files in os.walk(out, topdown=False):
        rel = Path(folder).relative_to(out).as_posix()
        for name in files:
            path = name if rel == '.' else f'{rel}/{name}'
            if excluded(path):
                (out / path).unlink()
                removed.append(path)
        if rel != '.' and Path(folder).name in EXCLUDED_DIRS:
            shutil.rmtree(folder)
            removed.append(rel + '/')
    return removed


def finish_report(result: dict, out: Path, original: Path, source: dict, copied: dict, after_copy: dict,
                  started: float, report: Path | None, link_from: Path | None) -> dict:
    """Step 8: every file, which ones differ from the original, the player's files."""
    hashes = original_hashes(original, {p: st for p, st in source.items() if p in copied})
    files, differ = {}, {}
    for path, st in sorted(tree(out).items()):
        untouched = path in copied and after_copy.get(path) == (st.st_size, st.st_mtime_ns)
        sha = hashes[path] if untouched else sha256_file(out / path)
        files[path] = {'sha256': sha, 'size': st.st_size, **({'preserve': True} if matches(path, PRESERVE_GLOBS) else {})}
        if path not in copied:
            differ[path] = 'new'
        elif sha != hashes[path]:
            differ[path] = 'changed'
    result['differ_from_original'] = differ
    result['files'] = files
    result['totals'] = {'files': len(files), 'bytes': sum(f['size'] for f in files.values()),
                        'differ': len(differ), 'seconds': round(time.time() - started)}
    if link_from is not None:
        result['linked'] = link_identical(out, files, link_from)
    report = out.parent / f'{out.name}.report.json' if report is None else report
    report.write_bytes(json.dumps(result, indent=1).encode('utf-8'))
    return result


def link_identical(out: Path, files: dict, previous: Path) -> dict:
    """Disk space: every finished file equal to the previous version's (its report: same SHA-256 and size)
    becomes a hard link to it, and both names turn read only, so a later write fails instead of changing
    two versions. Only after the build: no step writes into a linked file."""
    previous = reject_original(previous)
    other = json.loads((previous.parent / f'{previous.name}.report.json').read_text(encoding='utf-8'))['files']
    linked, saved = 0, 0
    for path, info in files.items():
        theirs = other.get(path)
        source, target = previous / path, out / path
        if not theirs or theirs['sha256'] != info['sha256'] or theirs['size'] != info['size']:
            continue
        if not source.is_file() or source.stat().st_size != info['size'] or os.path.samefile(source, target):
            continue
        os.chmod(source, stat.S_IREAD)
        target.unlink()
        os.link(source, target)
        linked += 1
        saved += info['size']
    return {'previous': str(previous), 'files': linked, 'bytes': saved}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--version', type=int, choices=sorted(class_layers.VERSIONS),
                        help='layer to build (required unless --refresh)')
    parser.add_argument('--out', type=Path, required=True, help='new folder (e.g. .cache/client-versions/v3)')
    parser.add_argument('--keep-state', action='store_true',
                        help="the server's client tree: keep the backups so --refresh can apply the content again")
    parser.add_argument('--refresh', action='store_true',
                        help='only the content step again, over a tree built with --keep-state')
    parser.add_argument('--original', type=Path, default=ORIGINAL, help='1.69 EU client (read only)')
    parser.add_argument('--keystream', type=Path, default=KEYSTREAM)
    parser.add_argument('--edits', type=Path, help='published panel edits (default: content_edits.EDITS)')
    parser.add_argument('--nmcogame', type=Path, default=NMCOGAME)
    parser.add_argument('--keep-test-items', action='store_true',
                        help='keep the item editor test items (only to compare with the lab copy)')
    parser.add_argument('--report', type=Path, help='report JSON (default: <out>.report.json next to the folder)')
    parser.add_argument('--endpoint-host', default='127.0.0.1',
                        help='IPv4 of the server for other computers (compose PUBLIC_HOST); the client reads its server list there')
    parser.add_argument('--link-from', type=Path,
                        help='an earlier version folder (with its report): identical files become read-only hard links')
    args = parser.parse_args()
    if args.version is None and not args.refresh:
        parser.error('--version is required unless --refresh')
    try:
        if args.refresh:
            result = refresh(args.out, args.original, args.keystream, args.edits)
        else:
            result = build(args.version, args.out, args.original, args.keystream, args.edits, args.nmcogame,
                           args.keep_test_items, args.report, args.link_from, args.endpoint_host, args.keep_state)
    except (PreparationError, ValueError, OSError) as error:
        print(json.dumps({'built': False, 'error': str(error)}))
        return 2
    print(json.dumps({'built': True, 'version': result['version'], 'totals': result['totals'],
                      'differ_from_original': result.get('differ_from_original', {})}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
