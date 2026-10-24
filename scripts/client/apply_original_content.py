"""Apply the original gameplay values to a COPY of the 1.69 EN client database. Dry-run by default.

The 1.69 EN client ships the same modified content as the server (every quest drops 20
items, every title goal needs one action, stacks of 1000, tripled story EXP). The client
reads these columns for its own display (title goal counters, quest board, stack size),
so the lab copy takes the same original values the server exports use
(scripts/content/content_overrides.py), here from the TutorialRagezone client database.
Only the listed columns change; a SQLite backup of the copy is written first.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content'))
from content_overrides import COLUMNS  # noqa: E402
from prepare_lab_client import PreparationError, connect, reject_original  # noqa: E402

WORKSPACE = Path(__file__).resolve().parents[3]
ORIGINAL_CLIENT_DATABASE = WORKSPACE / 'TutorialRagezone/DB/client/sql/heroes.db3'
ORIGINAL_CLIENT_SHA256 = 'c22ec7fe22f948250fc0e7dacf82acfdfe547141b03a0a6f10a1bda3cc436188'


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def original_values(table: str) -> dict:
    keys, columns = COLUMNS[table]
    with connect(ORIGINAL_CLIENT_DATABASE.resolve().as_uri() + '?mode=ro', uri=True) as db:
        db.row_factory = sqlite3.Row
        values = {}
        for r in db.execute('Select * From [' + table + ']'):
            key = tuple(r[k] for k in keys)
            if key in values:
                raise PreparationError('Original client key repeated in ' + table)
            values[key] = {c: r[c] for c in columns}
    return values


def plan(copy_db: Path) -> dict:
    """Per table, the (rowid, column, old, new) changes the copy needs."""
    changes = {}
    with connect(copy_db.resolve().as_uri() + '?mode=ro', uri=True) as db:
        db.row_factory = sqlite3.Row
        tables = {r[0] for r in db.execute("select name from sqlite_master where type='table'")}
        for table, (keys, columns) in COLUMNS.items():
            if table not in tables:
                continue
            original = original_values(table)
            seen = set()
            rows = []
            for r in db.execute('Select rowid AS rid, * From [' + table + ']'):
                key = tuple(r[k] for k in keys)
                if key in seen:
                    raise PreparationError('Client copy key repeated in ' + table)
                seen.add(key)
                for c in columns:
                    new = original.get(key, {}).get(c, r[c])
                    if new != r[c]:
                        rows.append((r['rid'], c, r[c], new))
            changes[table] = rows
    return changes


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--client-copy', required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    root = reject_original(Path(args.client_copy))
    copy_db = root / 'sql' / 'heroes.db3'
    if not copy_db.is_file():
        raise PreparationError('Client copy has no sql/heroes.db3')
    if digest(ORIGINAL_CLIENT_DATABASE) != ORIGINAL_CLIENT_SHA256:
        raise PreparationError('Original client database changed')
    changes = plan(copy_db)
    summary = {t: len(rows) for t, rows in changes.items()}
    if args.apply and any(summary.values()):
        backup = copy_db.with_name('heroes.db3.pre-original-content')
        if not backup.exists():
            with connect(copy_db) as source, connect(backup) as target:
                source.backup(target)
        with connect(copy_db) as db:
            for table, rows in changes.items():
                for rid, column, _, new in rows:
                    db.execute('UPDATE [' + table + '] SET [' + column + ']=? WHERE rowid=?', (new, rid))
    print(json.dumps({'applied': bool(args.apply), 'changes': summary}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
