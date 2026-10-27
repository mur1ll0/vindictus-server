"""Laboratory items added on top of the server content (scripts/content/item_additions.json).

The server reads the exports of scripts/content/export_*.py, which `SELECT *` the server's
heroesContents.db3, cross-check row order between exports and record the database hash. The
added items therefore never touch that file: `connect` opens it read-only and puts a TEMP VIEW
in front of each item table (`main.<table>` rows, then the added rows), so every unqualified
`SELECT * FROM <table>` sees the same rows in the same order and the database hash is unchanged.
An added item copies every row of its template item in TABLES, with ItemClass replaced and the
item's 'set' columns changed. The client copy gets the same rows through `apply_client`."""
from __future__ import annotations
import json
import pathlib
import re
import sqlite3

ROOT = pathlib.Path(__file__).resolve().parents[2]
ADDITIONS = pathlib.Path(__file__).resolve().parent / 'item_additions.json'
SERVER_DATABASE = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'
TABLES = ('ItemClassInfo', 'EquipItemInfo', 'CustomizeDefaultInfo', 'CustomizePriceInfo')
ITEM_CLASS = re.compile(r'^[a-z0-9_]{1,64}$')
TEXT_LIMIT = 512
# player_costume.txt entries and asset folders (.cache/private-assets/<assets>) of added items.
SECTION = re.compile(r'^[a-z_]{1,32}$')
CATEGORIES = ('male', 'female', 'giant', 'tall')
MODEL = re.compile(r'^models/[a-z0-9_/]{1,200}\.mdl$')
ASSET = re.compile(r'^[a-z0-9_]{1,64}$')


def load(path: pathlib.Path = ADDITIONS) -> list[dict]:
    if not path.is_file():
        return []
    items = json.loads(path.read_text(encoding='utf-8')).get('items', [])
    seen = set()
    for item in items:
        for key in ('ItemClass', 'template'):
            if not isinstance(item.get(key), str) or not ITEM_CLASS.match(item[key]):
                raise ValueError(f'Added item {key} must match {ITEM_CLASS.pattern}')
        if item['ItemClass'] in seen or item['ItemClass'] == item['template']:
            raise ValueError('Added item classes must be unique and differ from their template')
        seen.add(item['ItemClass'])
        for table, columns in item.get('set', {}).items():
            if table not in TABLES or not isinstance(columns, dict) or 'ItemClass' in columns:
                raise ValueError('Added item set must name item tables and must not change ItemClass')
        for costume in item.get('costumes', []):
            if not (isinstance(costume, dict) and SECTION.match(str(costume.get('section'))) and
                    str(costume.get('key')).isdigit() and len(str(costume.get('key'))) <= 9 and
                    costume.get('category') in CATEGORIES and MODEL.match(str(costume.get('normal')))):
                raise ValueError('Added costume needs section, numeric key, category and a models/....mdl path')
        if 'assets' in item and not ASSET.match(str(item['assets'])):
            raise ValueError(f'Added item assets must match {ASSET.pattern}')
        for field in ('name', 'desc'):
            text = item.get('text', {}).get(field)
            if text is not None and (not isinstance(text, str) or len(text) > TEXT_LIMIT or '"' in text):
                raise ValueError('Added item text must be a bounded string without quotes')
    return items


def item_rows(db: sqlite3.Connection, table: str, item: dict, schema: str = 'main') -> tuple[list[str], list[tuple]]:
    """Column names and the added rows of one table: the template's rows with the item's changes."""
    cursor = db.execute(f'SELECT * FROM {schema}.[{table}] WHERE ItemClass=?', (item['template'],))
    columns = [c[0] for c in cursor.description]
    changes = {'ItemClass': item['ItemClass'], **item.get('set', {}).get(table, {})}
    unknown = set(changes) - set(columns)
    if unknown:
        raise ValueError(f'{table} has no column {sorted(unknown)[0]}')
    rows = [tuple(changes.get(c, v) for c, v in zip(columns, row)) for row in cursor]
    if table in ('ItemClassInfo', 'EquipItemInfo') and not rows:
        raise ValueError(f'Template {item["template"]} has no {table} row')
    return columns, rows


def install(db: sqlite3.Connection, items: list[dict]) -> None:
    for table in TABLES:
        db.execute(f'CREATE TEMP TABLE [_added_{table}] AS SELECT * FROM main.[{table}] WHERE 0')
        for item in items:
            if db.execute(f'SELECT 1 FROM main.[{table}] WHERE ItemClass=?', (item['ItemClass'],)).fetchone():
                raise ValueError(f'{item["ItemClass"]} already exists in {table}')
            columns, rows = item_rows(db, table, item)
            db.executemany(f'INSERT INTO temp.[_added_{table}] VALUES({",".join("?" * len(columns))})', rows)
        db.execute(f'CREATE TEMP VIEW [{table}] AS SELECT * FROM main.[{table}] UNION ALL SELECT * FROM temp.[_added_{table}]')
    db.commit()


def connect(database: pathlib.Path | str) -> sqlite3.Connection:
    """Read-only connection; the server content database also sees the added items."""
    path = pathlib.Path(database).resolve()
    db = sqlite3.connect(path.as_uri() + '?mode=ro', uri=True)
    if path == SERVER_DATABASE.resolve():
        items = load()
        if items:
            try:
                install(db, items)
            except Exception:
                db.close()
                raise
    return db


def apply_client(db: sqlite3.Connection, items: list[dict]) -> int:
    """Writes the added items into a client heroes.db3 (replacing earlier copies); returns rows written."""
    written = 0
    for table in TABLES:
        for item in items:
            db.execute(f'DELETE FROM main.[{table}] WHERE ItemClass=?', (item['ItemClass'],))
            columns, rows = item_rows(db, table, item)
            db.executemany(f'INSERT INTO main.[{table}]({",".join("[%s]" % c for c in columns)}) '
                           f'VALUES({",".join("?" * len(columns))})', rows)
            written += len(rows)
    return written


def text_entries(items: list[dict]) -> dict[str, str]:
    """Localized tokens the client looks up: HEROES_ITEM_NAME_/DESC_<ITEMCLASS upper>."""
    entries = {}
    for item in items:
        text = item.get('text', {})
        if text.get('name') is not None:
            entries['HEROES_ITEM_NAME_' + item['ItemClass'].upper()] = text['name']
        if text.get('desc') is not None:
            entries['HEROES_ITEM_DESC_' + item['ItemClass'].upper()] = text['desc']
    return entries
