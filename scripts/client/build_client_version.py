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
import json
import os
import shutil
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
KEYSTREAM_SHA256 = '8af3e7e346c5a22d2c2f8584234ef16183fd6421547841fb114120ab7e23ab5b'
NMCOGAME = ROOT / 'tools/nmcogame/target/i686-win7-windows-msvc/release/nmcogame.dll'  # tools/nmcogame/build.ps1
HASH_CACHE = ROOT / '.cache/client-versions/original-sha256.json'
# Lab endpoint (prepare_lab_client.py defaults: the frontend of scripts/dev/start-lab-frontend.ps1).
ENDPOINT = {'region': 'EN-EU-X-GM', 'base_url': 'http://127.0.0.1:8081', 'game_host': '127.0.0.1', 'game_port': 27015}
# Item editor test items (operator decision of 07/11/2026): never distributed.
TEST_ITEMS = ('fiona_inner_lab', 'fiona_inner_fringe', 'fiona_inner_zero', 'evy_inner_zero')

# Exclusions (game-launcher.md 4.3 item 6, D-L3 and the additions of 07/11/2026): folders anywhere in the
# tree, then globs over the relative path (or the file name when the glob has no '/').
EXCLUDED_DIRS = ('hfs_extracted', 'lab-backup', 'passport', 'BlackCipher', 'downloads', 'DownloadLists', '__pycache__')
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
        return subprocess.run(['git', *args], cwd=ROOT, capture_output=True, text=True, check=False).stdout.strip()
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
          link_from: Path | None = None) -> dict:
    sys.path.insert(0, str(ROOT / 'scripts/content'))
    import apply_delia_creation_movie
    import apply_lab_content_additions
    import content_edits
    import hfs
    import prepare_lab_client
    import recover_mss32
    classes = class_layers.VERSIONS[version]
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
            out, work / 'web', ENDPOINT['region'], ENDPOINT['base_url'], ENDPOINT['game_host'], ENDPOINT['game_port'],
            True).items() if k in ('previous_bootstrap_url', 'new_bootstrap_url')}
        result['steps']['original_content'] = run_script('apply_original_content.py', '--client-copy', str(out), '--apply')
        result['steps']['story_goals'] = run_script('align_story_goals.py', '--client-copy', str(out), '--apply')
        # 5. The Delia's customization movie (v2+), before the Miri's sliders go on top of it.
        if 'delia' in classes:
            result['steps']['delia_movie'] = apply_delia_creation_movie.apply(out, keystream, True)
        # 6. Content of the layer: shops, panel edits, items, collabs, classes, packs and index.
        ks = hfs.load_keystream(keystream)
        with base_costume(original, ks, work) as costume_sha:
            plan = apply_lab_content_additions.apply(
                out, True, edits=content_edits.load(edits), keystream=keystream, classes=classes,
                exclude_items=() if keep_test_items else TEST_ITEMS)
        result['steps']['content'] = {'base_costume_sha256': costume_sha, 'edits': str(edits),
                                      'edits_sha256': sha256_file(edits) if edits.is_file() else None, **plan}

    # 7. The build's leftovers leave the folder (backups, lab-backup/, reports).
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
    result['steps']['pruned'] = sorted(removed)

    # 8. Report: every file, which ones differ from the original, the player's files.
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
    parser.add_argument('--version', type=int, choices=sorted(class_layers.VERSIONS), required=True)
    parser.add_argument('--out', type=Path, required=True, help='new folder (e.g. .cache/client-versions/v3)')
    parser.add_argument('--original', type=Path, default=ORIGINAL, help='1.69 EU client (read only)')
    parser.add_argument('--keystream', type=Path, default=KEYSTREAM)
    parser.add_argument('--edits', type=Path, help='published panel edits (default: content_edits.EDITS)')
    parser.add_argument('--nmcogame', type=Path, default=NMCOGAME)
    parser.add_argument('--keep-test-items', action='store_true',
                        help='keep the item editor test items (only to compare with the lab copy)')
    parser.add_argument('--report', type=Path, help='report JSON (default: <out>.report.json next to the folder)')
    parser.add_argument('--link-from', type=Path,
                        help='an earlier version folder (with its report): identical files become read-only hard links')
    args = parser.parse_args()
    try:
        result = build(args.version, args.out, args.original, args.keystream, args.edits, args.nmcogame,
                       args.keep_test_items, args.report, args.link_from)
    except (PreparationError, ValueError, OSError) as error:
        print(json.dumps({'built': False, 'error': str(error)}))
        return 2
    print(json.dumps({'built': True, 'version': result['version'], 'totals': result['totals'],
                      'differ_from_original': result['differ_from_original']}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
