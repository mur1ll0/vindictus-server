"""Add the laboratory content rows (scripts/content/lab_content_additions.json) to a COPY of
the client's sql/heroes.db3, so the client shows what the server export adds (for example
the Ferghus shop row of Enhancement Elixir). Dry-run by default; idempotent; the original
clients are refused and a consistent backup is kept before the first change."""
from __future__ import annotations
import argparse
import json
import sqlite3
from contextlib import closing
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

ADDITIONS = Path(__file__).resolve().parents[1] / 'content/lab_content_additions.json'
SHOP_COLUMNS = ('ShopID', 'Order', 'ItemClass', 'Count', 'PriceItemClass', 'PriceCount', 'AppearQuestID',
                'CharacterBind', 'ID', 'RestrictionResetTime', 'RestrictionPeriodDay', 'RestrictionCount', 'Feature')


def missing_rows(db: sqlite3.Connection, rows: list[dict]) -> list[dict]:
    out = []
    for r in rows:
        found = db.execute('SELECT ItemClass FROM ShopInfo WHERE ShopID=? AND [Order]=?', (r['ShopID'], r['Order'])).fetchall()
        if found and found[0][0] != r['ItemClass']:
            raise PreparationError('A client ShopInfo row already uses this ShopID and Order.')
        if not found:
            out.append(r)
    return out


def apply(client_copy: Path, apply_changes: bool = False, additions: Path = ADDITIONS) -> dict:
    database = reject_original(client_copy / 'sql/heroes.db3')
    if not database.is_file():
        raise PreparationError('The client copy must contain sql/heroes.db3.')
    rows = json.loads(additions.read_text(encoding='utf-8')).get('ShopInfo', [])
    with closing(sqlite3.connect(database)) as db:
        todo = missing_rows(db, rows)
        plan = {'mode': 'apply' if apply_changes else 'dry-run', 'database': str(database),
                'shop_rows_missing': len(todo), 'shop_rows': len(rows)}
        if apply_changes and todo:
            backup = database.with_name('heroes.db3.pre-lab-additions')
            if not backup.exists():
                with closing(sqlite3.connect(backup)) as target:
                    db.backup(target)
            with db:
                db.executemany('INSERT INTO ShopInfo(' + ','.join('[%s]' % c for c in SHOP_COLUMNS) + ') VALUES(' +
                               ','.join('?' * len(SHOP_COLUMNS)) + ')', [tuple(r[c] for c in SHOP_COLUMNS) for r in todo])
            plan['backup'] = str(backup)
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(apply(args.client_copy, args.apply)))
    except PreparationError as error:
        print(json.dumps({'applied': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
