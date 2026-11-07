"""Checks a built client version (build_client_version.py report) against a reference. Read only.

  --reference <client folder>   every distributed file byte for byte (SHA-256 of both sides); the
                                differences must be explained: the item editor's test items (their own
                                packs; their rows in sql/heroes.db3; their lines in the text and scripts
                                packs, compared entry by entry; the HFS index that lists the packs) and the
                                player's files (preserved, never compared).
  --inventory <inventory.json>  the files that differ from the original are the inventory's layers up to
                                the version (.cache/verification/client-versions/inventory.json, 07/11/2026),
                                minus the test items' packs.
Prints a JSON summary; exit 0 when nothing unexplained is left.
"""
from __future__ import annotations
import argparse
import json
import re
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

import build_client_version as recipe
import hfs

TEST = re.compile('|'.join(recipe.TEST_ITEMS), re.I)
INDEX = 'hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs'
DATABASE = 'sql/heroes.db3'
COSTUME_BLOCK = re.compile(r'\t"\d+"\n\t\{\n(?:\t\t[^\n]*\n)*?\t\}')


def table_rows(path: Path) -> dict[str, list[tuple]]:
    with closing(sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
        return {t: list(db.execute(f'SELECT * FROM [{t}]')) for t in tables}


def database_diff(built: Path, reference: Path) -> dict:
    """Rows only on one side (as multisets), per table; explained when they name a test item."""
    a, b = table_rows(built), table_rows(reference)
    out = {'tables_only_built': sorted(set(a) - set(b)), 'tables_only_reference': sorted(set(b) - set(a)),
           'rows': {}, 'order_differs': [], 'unexplained': 0}
    for table in sorted(set(a) & set(b)):
        sa, sb = set(a[table]), set(b[table])
        only_a = [r for r in a[table] if r not in sb]
        only_b = [r for r in b[table] if r not in sa]
        if not only_a and not only_b:
            if sorted(a[table], key=repr) != sorted(b[table], key=repr):
                out['rows'][table] = {'unexplained': 1, 'note': 'same rows, different counts'}
                out['unexplained'] += 1
            elif a[table] != b[table]:
                out['order_differs'].append(table)
            continue
        bad = [r for r in only_a + only_b if not TEST.search(repr(r))]
        out['rows'][table] = {'only_built': len(only_a), 'only_reference': len(only_b), 'unexplained': len(bad),
                              'sample_unexplained': [repr(r)[:300] for r in bad[:5]]}
        out['unexplained'] += len(bad)
    out['unexplained'] += len(out['tables_only_built']) + len(out['tables_only_reference'])
    return out


def pack_entries(path: Path, ks: bytes) -> dict[str, bytes]:
    return {e.name: (hfs.comp_unwrap(e.data) if e.name.endswith('.comp') else e.data)
            for e in hfs.read_pack(path.read_bytes(), ks)}


def text_lines(data: bytes) -> list[str]:
    text = data.decode('utf-16') if data[:2] in (b'\xff\xfe', b'\xfe\xff') else data.decode('latin-1')
    return text.replace('\r\n', '\n').split('\n')


def pack_diff(built: Path, reference: Path, ks: bytes) -> dict:
    """Entries that differ; inside each, the lines only on one side: a test item's text line, or a line of a
    player_costume.txt block whose model names a test item, is explained."""
    a, b = pack_entries(built, ks), pack_entries(reference, ks)
    out = {'entries_only_built': sorted(set(a) - set(b)), 'entries_only_reference': sorted(set(b) - set(a)),
           'entries': {}, 'unexplained': 0}
    for name in sorted(set(a) & set(b)):
        if a[name] == b[name]:
            continue
        la, lb = text_lines(a[name]), text_lines(b[name])
        sa, sb = set(la), set(lb)
        only_a = [line for line in la if line not in sb]
        only_b = [line for line in lb if line not in sa]
        explained = set()
        for side in ('\n'.join(la), '\n'.join(lb)):
            for block in COSTUME_BLOCK.finditer(side):
                if TEST.search(block.group(0)):
                    explained.update(block.group(0).split('\n'))
        bad = [line for line in only_a + only_b if not TEST.search(line) and line not in explained]
        out['entries'][name] = {'lines_only_built': len(only_a), 'lines_only_reference': len(only_b),
                                'unexplained': len(bad), 'sample_unexplained': [line[:200] for line in bad[:5]]}
        out['unexplained'] += len(bad)
    out['unexplained'] += len(out['entries_only_built']) + len(out['entries_only_reference'])
    return out


def test_item_packs(client: Path, ks: bytes, edits: Path | None = None) -> set[str]:
    """The packs of `client` that hold only the test items' asset files: a pack name their kit folders can
    take (apply_lab_content_additions.asset_packs: the folder's next free part) whose every entry is a file
    of those kits (.cache/private-assets). Another pack of the same folder (a class's) is not one."""
    import apply_lab_content_additions as lab
    import content_additions
    import content_edits
    items = content_additions.load(edits=content_edits.load(content_edits.EDITS if edits is None else edits))
    kits: dict[str, set[str]] = {}
    for item in items:
        if item['ItemClass'] not in recipe.TEST_ITEMS or not item.get('assets'):
            continue
        root = lab.ASSETS / item['assets']
        for p in root.rglob('*'):
            if p.is_file() and p.parent != root:
                kits.setdefault(p.parent.relative_to(root).as_posix().lower(), set()).add(p.name.lower() + '.comp')
    out = set()
    for folder, names in kits.items():
        for part in range(64):
            path = f'hfs/{hfs.pack_name(folder, part)}.hfs'
            if (client / path).is_file():
                entries = {e.name.lower() for e in hfs.read_pack((client / path).read_bytes(), ks)}
                if entries and entries <= names:
                    out.add(path)
    return out


# Files the inventory gives to v1 only through the test items (its note on the scripts pack: "player_costume:
# roupas dos itens de laboratório = v1"); without them v1 keeps the original.
TEST_ITEMS_ONLY = {'hfs/763FD8F12D3D013194261D98B4AEF0FB0F6FBA1B.hfs': ('v1', "player_costume.txt entries of the test items")}


def compare_reference(report: dict, reference: Path, keystream: Path) -> dict:
    built = Path(report['out'])
    ref_tree = {p: st for p, st in recipe.tree(reference).items() if not recipe.excluded(p)}
    files = report['files']
    ks = hfs.load_keystream(keystream)
    tests = test_item_packs(reference, ks)
    # SHA-256 of the reference's files of the same size (cached by path, size and mtime like the original's).
    ref_hashes = recipe.original_hashes(reference, {p: st for p, st in ref_tree.items()
                                                    if p in files and files[p]['size'] == st.st_size})
    out = {'identical': 0, 'test_item_packs': sorted(tests), 'preserved': [], 'only_reference': {}, 'only_built': [], 'differ': {}, 'unexplained': []}
    for path in sorted(set(files) | set(ref_tree)):
        if recipe.matches(path, recipe.PRESERVE_GLOBS):
            out['preserved'].append(path)
            continue
        if path not in files:
            out['only_reference'][path] = 'test item pack' if path in tests else 'unexplained'
            if path not in tests:
                out['unexplained'].append(path)
            continue
        if path not in ref_tree:
            out['only_built'].append(path)
            out['unexplained'].append(path)
            continue
        if files[path]['size'] == ref_tree[path].st_size and files[path]['sha256'] == ref_hashes[path]:
            out['identical'] += 1
            continue
        if path == DATABASE:
            detail = database_diff(built / path, reference / path)
        elif path == INDEX:
            detail = {'explained': 'HFS index (lists the packs)', 'unexplained': 0}
        elif path.endswith('.hfs'):
            detail = pack_diff(built / path, reference / path, ks)
        else:
            detail = {'unexplained': 1}
        out['differ'][path] = detail
        if detail['unexplained']:
            out['unexplained'].append(path)
    return out


def compare_inventory(report: dict, inventory_path: Path, keystream: Path) -> dict:
    inventory = json.loads(inventory_path.read_text(encoding='utf-8'))
    lab = recipe.ROOT / inventory['lab']
    layers = {f'v{n}' for n in range(1, report['version'] + 1)}
    listed = {f['path'] for f in inventory['files']}
    expected = {f['path'] for f in inventory['files'] if set(f['layers']) & layers}
    tests = test_item_packs(lab, hfs.load_keystream(keystream)) & listed
    expected -= tests
    found = set(report['differ_from_original'])
    file_layers = {f['path']: set(f['layers']) for f in inventory['files']}
    explained = {p: why for p, (only, why) in TEST_ITEMS_ONLY.items()
                 if p in expected - found and file_layers.get(p, set()) & layers == {only}}
    return {'layers': sorted(layers), 'expected': len(expected), 'found': len(found), 'test_item_packs': sorted(tests),
            'explained_missing': explained, 'missing': sorted(expected - found - set(explained)),
            'extra': sorted(found - expected)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--reference', type=Path)
    parser.add_argument('--inventory', type=Path)
    parser.add_argument('--keystream', type=Path, default=recipe.KEYSTREAM)
    parser.add_argument('--out', type=Path, help='full result JSON')
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding='utf-8'))
    result = {'version': report['version']}
    if args.reference:
        result['reference'] = compare_reference(report, args.reference, args.keystream)
    if args.inventory:
        result['inventory'] = compare_inventory(report, args.inventory, args.keystream)
    if args.out:
        args.out.write_bytes(json.dumps(result, indent=1).encode('utf-8'))
    ref = result.get('reference', {})
    inv = result.get('inventory', {})
    summary = {'version': report['version'],
               'reference': {k: (len(v) if isinstance(v, (list, dict)) else v) for k, v in ref.items()} if ref else None,
               'unexplained': ref.get('unexplained', []),
               'differ': {p: d.get('unexplained') for p, d in ref.get('differ', {}).items()},
               'inventory': inv or None}
    print(json.dumps(summary, indent=1))
    return 1 if ref.get('unexplained') or inv.get('missing') or inv.get('extra') else 0


if __name__ == '__main__':
    sys.exit(main())
