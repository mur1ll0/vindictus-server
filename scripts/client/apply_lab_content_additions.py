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
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content'))
import content_edits  # noqa: E402

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


def apply(client_copy: Path, apply_changes: bool = False, additions: Path = ADDITIONS, edits: dict | None = None) -> dict:
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
        if apply_changes and current != target:
            if not backup.exists():
                with closing(sqlite3.connect(backup)) as copy:
                    db.backup(copy)
            with db:
                db.execute('DELETE FROM ShopInfo')
                db.executemany('INSERT INTO ShopInfo(' + ','.join('[%s]' % c for c in SHOP_COLUMNS) + ') VALUES(' +
                               ','.join('?' * len(SHOP_COLUMNS)) + ')', [tuple(r[c] for c in SHOP_COLUMNS) for r in target])
            plan['backup'] = str(backup)
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
