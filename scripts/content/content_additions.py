"""Laboratory items added on top of the server content (scripts/content/item_additions.json) and the
admin panel's item editor (docs/future/item-editor.md, cut 2), as the published overlay the game
writes to .cache/content-state/content-edits.json.

The server reads the exports of scripts/content/export_*.py, which `SELECT *` the server's
heroesContents.db3, cross-check row order between exports and record the database hash. The
added items therefore never touch that file: `connect` opens it read-only and puts a TEMP VIEW
in front of each item table (`main.<table>` rows, then the added rows), so every unqualified
`SELECT * FROM <table>` sees the same rows in the same order and the database hash is unchanged.
An added item copies every row of its template item in TABLES, with ItemClass replaced and the
item's 'set' columns changed; `set_id` puts it in an equipment set (SetItemInfo).

Panel edits of base items (`item_overrides`) change editable columns (EDITABLE) of every row of
the item in place: the view reads a TEMP copy of the table in its original order with those cells
changed, so row positions and counts stay. Hidden items (`hidden_items`) keep their rows (the
characters that own one keep a working item); they only leave the NPC shops (content_edits) and
the panel's give list. `removed_items` drops items of item_additions.json. The client copy gets
the same rows through `apply_client`."""
from __future__ import annotations
import hashlib
import json
import pathlib
import re
import sqlite3

ROOT = pathlib.Path(__file__).resolve().parents[2]
ADDITIONS = pathlib.Path(__file__).resolve().parent / 'item_additions.json'
EDITS = ROOT / '.cache/content-state/content-edits.json'
SERVER_DATABASE = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'
TABLES = ('ItemClassInfo', 'EquipItemInfo', 'CustomizeDefaultInfo', 'CustomizePriceInfo')
SET_TABLE = 'SetItemInfo'
CLASS_ADDITIONS = pathlib.Path(__file__).resolve().parent / 'class_additions.json'
# BaseCharacter values: 5-bit m_iClass and int ClassRestriction masks (docs/future/new-classes.md 8.13).
CLASS_LIMIT = 32
# Columns the item editor changes (the Rust panel validates the same list, src/domain/item_editor.rs).
EDITABLE = {
    'ItemClassInfo': ('Icon', 'Category', 'TradeCategory', 'TradeCategorySub', 'Rarity', 'Level', 'RequiredLevel',
                      'ClassRestriction', 'MaxStack', 'Price', 'SellPrice', 'ExpireIn'),
    'EquipItemInfo': ('CostumeType', 'MaxDurability', 'ATK', 'ATK_Speed', 'Critical', 'Balance', 'MATK', 'DEF',
                      'Res_Critical', 'STR', 'DEX', 'INT', 'WILL', 'LUCK', 'HP', 'STAMINA', 'Weight', 'Dyeable'),
}
ITEM_CLASS = re.compile(r'^[a-z0-9_]{1,64}$')
SET_ID = re.compile(r'^[A-Za-z0-9_]{1,64}$')
TEXT_LIMIT = 512
MAX_PANEL_ITEMS = 2000
# player_costume.txt entries and asset folders (.cache/private-assets/<assets>) of added items.
SECTION = re.compile(r'^[a-z_]{1,32}$')
CATEGORIES = ('male', 'female', 'giant', 'tall')
MODEL = re.compile(r'^models/[a-z0-9_/]{1,200}\.mdl$')
ASSET = re.compile(r'^[a-z0-9_]{1,64}$')


def costume_section(section: str) -> bool:
    """A player_costume.txt section (with a body category), not a weapon_<class> or shields entry."""
    return not (section.startswith('weapon_') or section == 'shields')


def _check_text(item: dict) -> None:
    text = item.get('text', {})
    if not isinstance(text, dict):
        raise ValueError('Item text must be an object')
    for field in ('name', 'desc'):
        value = text.get(field)
        if value is not None and (not isinstance(value, str) or len(value) > TEXT_LIMIT or '"' in value
                                  or any(ord(c) < 32 for c in value)):
            raise ValueError('Added item text must be a bounded string without quotes')


def _check_item(item: dict) -> None:
    for key in ('ItemClass', 'template'):
        if not isinstance(item.get(key), str) or not ITEM_CLASS.match(item[key]):
            raise ValueError(f'Added item {key} must match {ITEM_CLASS.pattern}')
    if item['ItemClass'] == item['template']:
        raise ValueError('Added item classes must differ from their template')
    for table, columns in item.get('set', {}).items():
        if table not in TABLES or not isinstance(columns, dict) or 'ItemClass' in columns:
            raise ValueError('Added item set must name item tables and must not change ItemClass')
    for costume in item.get('costumes', []):
        if not (isinstance(costume, dict) and SECTION.match(str(costume.get('section'))) and
                str(costume.get('key')).isdigit() and len(str(costume.get('key'))) <= 9 and
                (costume.get('category') in CATEGORIES or not costume_section(costume['section'])) and
                MODEL.match(str(costume.get('normal')))):
            raise ValueError('Added costume needs section, numeric key, category and a models/....mdl path')
    if 'assets' in item and not ASSET.match(str(item['assets'])):
        raise ValueError(f'Added item assets must match {ASSET.pattern}')
    if item.get('set_id') is not None and not SET_ID.match(str(item['set_id'])):
        raise ValueError(f'Added item set_id must match {SET_ID.pattern}')
    _check_text(item)


def panel_edits(edits: dict | None = None) -> dict:
    """The item parts of the published overlay: items, item_overrides, hidden_items, removed_items."""
    if edits is None:
        import content_edits
        edits = content_edits.load(EDITS)
    items = edits.get('items', [])
    overrides = edits.get('item_overrides', {})
    hidden = edits.get('hidden_items', [])
    removed = edits.get('removed_items', [])
    if not (isinstance(items, list) and isinstance(overrides, dict) and isinstance(hidden, list)
            and isinstance(removed, list)) or len(items) + len(overrides) > MAX_PANEL_ITEMS:
        raise ValueError('Panel item edits outside their bounds')
    for name in list(overrides) + hidden + removed:
        if not isinstance(name, str) or not ITEM_CLASS.match(name):
            raise ValueError(f'Panel item class must match {ITEM_CLASS.pattern}')
    for name, override in overrides.items():
        if not isinstance(override, dict):
            raise ValueError('Item override must be an object')
        for table, columns in override.get('set', {}).items():
            if table not in EDITABLE or not isinstance(columns, dict) or not set(columns) <= set(EDITABLE[table]):
                raise ValueError(f'Item override of {name} changes a column outside the editor')
        _check_text(override)
    return {'items': items, 'item_overrides': overrides, 'hidden_items': hidden, 'removed_items': removed}


def load(path: pathlib.Path = ADDITIONS, edits: dict | None = None) -> list[dict]:
    """item_additions.json items, then the panel's: a panel item replaces the file item of the same
    class, and removed_items drops file items."""
    items = json.loads(path.read_text(encoding='utf-8')).get('items', []) if path.is_file() else []
    panel = panel_edits(edits) if (edits is not None or path == ADDITIONS) else panel_edits({})
    removed = set(panel['removed_items'])
    by_class = {}
    for item in items + [{k: v for k, v in i.items()} for i in panel['items']]:
        if not isinstance(item, dict):
            raise ValueError('Added item must be an object')
        _check_item(item)
        if item['ItemClass'] in removed:
            continue
        by_class[item['ItemClass']] = item
    if len(by_class) > MAX_PANEL_ITEMS:
        raise ValueError('Too many added items')
    return list(by_class.values())


def load_overrides(edits: dict | None = None) -> dict[str, dict]:
    return panel_edits(edits)['item_overrides']


def hidden_items(edits: dict | None = None) -> list[str]:
    return panel_edits(edits)['hidden_items']


def digest(items: list[dict], overrides: dict[str, dict]) -> str:
    """Stable digest of the item content (scripts/content/refresh_item_content.py stamps it)."""
    data = json.dumps({'items': items, 'overrides': overrides}, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(data.encode()).hexdigest()


def item_rows(db: sqlite3.Connection, table: str, item: dict, source: str = 'main') -> tuple[list[str], list[tuple]]:
    """Column names and the added rows of one table: the template's rows with the item's changes.
    `source` is a schema name or a full table expression (the overridden copy of a base table)."""
    origin = source if '[' in source else f'{source}.[{table}]'
    cursor = db.execute(f'SELECT * FROM {origin} WHERE ItemClass=?', (item['template'],))
    columns = [c[0] for c in cursor.description]
    changes = {'ItemClass': item['ItemClass'], **item.get('set', {}).get(table, {})}
    unknown = set(changes) - set(columns)
    if unknown:
        raise ValueError(f'{table} has no column {sorted(unknown)[0]}')
    rows = [tuple(changes.get(c, v) for c, v in zip(columns, row)) for row in cursor]
    if table in ('ItemClassInfo', 'EquipItemInfo') and not rows:
        raise ValueError(f'Template {item["template"]} has no {table} row')
    return columns, rows


def set_rows(db: sqlite3.Connection, item: dict, schema: str = 'main') -> tuple[list[str], list[tuple]]:
    """The SetItemInfo row of an added item with a set_id (none otherwise)."""
    columns = [r[1] for r in db.execute(f'PRAGMA {schema}.table_info([{SET_TABLE}])')]
    if not item.get('set_id') or not columns:
        return columns, []
    if not db.execute(f'SELECT 1 FROM {schema}.[{SET_TABLE}] WHERE SetID=? LIMIT 1', (item['set_id'],)).fetchone():
        raise ValueError(f'Unknown set {item["set_id"]}')
    values = {'ItemClass': item['ItemClass'], 'BaseItemClass': item['ItemClass'], 'SetID': item['set_id']}
    return columns, [tuple(values.get(c) for c in columns)]


def _override_sql(columns: list[str], changes: dict, table: str, name: str) -> tuple[str, list]:
    unknown = set(changes) - set(columns)
    if unknown:
        raise ValueError(f'{table} has no column {sorted(unknown)[0]}')
    return ', '.join(f'[{c}]=?' for c in changes), list(changes.values())


def install(db: sqlite3.Connection, items: list[dict], overrides: dict[str, dict] | None = None) -> None:
    overrides = overrides or {}
    added = {item['ItemClass'] for item in items}
    for name in overrides:
        if name in added:
            raise ValueError(f'{name} is an added item; edit it, not override it')
        if not db.execute('SELECT 1 FROM main.[ItemClassInfo] WHERE ItemClass=?', (name,)).fetchone():
            raise ValueError(f'Overridden item {name} is not in the content')
    for table in TABLES:
        source = f'main.[{table}]'
        changes = {n: o['set'][table] for n, o in overrides.items() if o.get('set', {}).get(table)}
        if changes:
            # Same rows in the same order (insertion order of the copy), with the edited cells.
            db.execute(f'CREATE TEMP TABLE [_base_{table}] AS SELECT * FROM main.[{table}]')
            columns = [r[1] for r in db.execute(f'PRAGMA main.table_info([{table}])')]
            for name, values in changes.items():
                assignments, params = _override_sql(columns, values, table, name)
                db.execute(f'UPDATE temp.[_base_{table}] SET {assignments} WHERE ItemClass=?', params + [name])
            source = f'temp.[_base_{table}]'
        db.execute(f'CREATE TEMP TABLE [_added_{table}] AS SELECT * FROM main.[{table}] WHERE 0')
        for item in items:
            if db.execute(f'SELECT 1 FROM main.[{table}] WHERE ItemClass=?', (item['ItemClass'],)).fetchone():
                raise ValueError(f'{item["ItemClass"]} already exists in {table}')
            columns, rows = item_rows(db, table, item, source)
            db.executemany(f'INSERT INTO temp.[_added_{table}] VALUES({",".join("?" * len(columns))})', rows)
        db.execute(f'CREATE TEMP VIEW [{table}] AS SELECT * FROM {source} UNION ALL SELECT * FROM temp.[_added_{table}]')
    if any(item.get('set_id') for item in items):
        db.execute(f'CREATE TEMP TABLE [_added_{SET_TABLE}] AS SELECT * FROM main.[{SET_TABLE}] WHERE 0')
        for item in items:
            columns, rows = set_rows(db, item)
            db.executemany(f'INSERT INTO temp.[_added_{SET_TABLE}] VALUES({",".join("?" * len(columns))})', rows)
        db.execute(f'CREATE TEMP VIEW [{SET_TABLE}] AS SELECT * FROM main.[{SET_TABLE}] '
                   f'UNION ALL SELECT * FROM temp.[_added_{SET_TABLE}]')
    db.commit()


def load_classes(path: pathlib.Path | None = None) -> list[dict]:
    """class_additions.json: creation rows of classes outside the 1.69 CustomizeCharacter switch."""
    path = CLASS_ADDITIONS if path is None else path
    classes = json.loads(path.read_text(encoding='utf-8')).get('classes', []) if path.is_file() else []
    seen = set()
    for c in classes:
        number = c.get('character_class') if isinstance(c, dict) else None
        inner = c.get('inner', {}) if isinstance(c, dict) else {}
        if (type(number) is not int or not 10 <= number < CLASS_LIMIT or number in seen
                or not isinstance(c.get('name'), str) or not c['name'].isalpha()
                or not isinstance(inner, dict) or not ITEM_CLASS.match(str(inner.get('item')))
                or not (isinstance(inner.get('colors'), list) and len(inner['colors']) == 3
                        and all(type(v) is int for v in inner['colors']))):
            raise ValueError('Added class needs a class number 10-31, a name and an inner armor')
        for key, count in c.get('default_items', []) + c.get('default_skills', []):
            if not ITEM_CLASS.match(str(key)) or type(count) is not int or not 0 < count < 2**31:
                raise ValueError('Added class rows need a class name and a positive count')
        seen.add(number)
    return classes


def install_classes(db: sqlite3.Connection, classes: list[dict]) -> None:
    """DefaultItemInfo and DefaultSkillInfo with the added classes' rows after the original ones."""
    for table, key, rows_key in (('DefaultItemInfo', 'ItemClass', 'default_items'),
                                 ('DefaultSkillInfo', 'SkillID', 'default_skills')):
        if table in [r[0] for r in db.execute("SELECT name FROM temp.sqlite_master WHERE type='view'")]:
            raise ValueError(f'{table} already has an overlay')
        db.execute(f'CREATE TEMP TABLE [_added_{table}] AS SELECT * FROM main.[{table}] WHERE 0')
        columns = [r[1] for r in db.execute(f'PRAGMA main.table_info([{table}])')]
        identifier = db.execute(f'SELECT coalesce(max(ID), 0) FROM main.[{table}]').fetchone()[0]
        for c in classes:
            for name, count in c.get(rows_key, []):
                if db.execute(f'SELECT 1 FROM main.[{table}] WHERE CharacterClass=? AND [{key}]=?',
                              (c['character_class'], name)).fetchone():
                    raise ValueError(f'{table} already has {name} for class {c["character_class"]}')
                identifier += 1
                values = {'ID': identifier, 'CharacterClass': c['character_class'], key: name,
                          'ItemNum': count, 'Rank': count, 'Feature': None}
                db.execute(f'INSERT INTO temp.[_added_{table}] VALUES({",".join("?" * len(columns))})',
                           [values[col] for col in columns])
        db.execute(f'CREATE TEMP VIEW [{table}] AS SELECT * FROM main.[{table}] UNION ALL '
                   f'SELECT * FROM temp.[_added_{table}]')
    db.commit()


def connect(database: pathlib.Path | str) -> sqlite3.Connection:
    """Read-only connection; the server content database also sees the added and edited items
    and the added classes' creation rows."""
    path = pathlib.Path(database).resolve()
    db = sqlite3.connect(path.as_uri() + '?mode=ro', uri=True)
    if path == SERVER_DATABASE.resolve():
        items = load()
        overrides = load_overrides()
        classes = load_classes()
        try:
            if items or overrides:
                install(db, items, overrides)
            if classes:
                install_classes(db, classes)
        except Exception:
            db.close()
            raise
    return db


def apply_client(db: sqlite3.Connection, items: list[dict], overrides: dict[str, dict] | None = None,
                 original: sqlite3.Connection | None = None, previous: dict | None = None) -> int:
    """Writes the added items into a client heroes.db3 (replacing earlier copies) and the base item
    overrides in place; returns rows written. `original` is the copy taken before any lab change
    (it restores base rows whose override was undone); `previous` is what the last run wrote
    ({'added': [...], 'overridden': [...]}), so a removed item leaves the copy."""
    overrides = overrides or {}
    previous = previous or {}
    original = original or db
    written = 0
    current = {item['ItemClass'] for item in items}
    stale = [name for name in previous.get('added', []) if name not in current and ITEM_CLASS.match(name)]
    # Base items first (an added item cloned from one copies its edited cells): the original
    # cells (an undone override comes back), then the override.
    touched = sorted({*previous.get('overridden', []), *overrides} - current)
    for table in EDITABLE:
        columns = [r[1] for r in db.execute(f'PRAGMA main.table_info([{table}])')]
        editable = [c for c in EDITABLE[table] if c in columns]
        for name in touched:
            if not ITEM_CLASS.match(name):
                continue
            changes = overrides.get(name, {}).get('set', {}).get(table, {})
            if set(changes) - set(editable):
                raise ValueError(f'{table} has no column {sorted(set(changes) - set(editable))[0]}')
            for rowid, *values in original.execute(
                    f'SELECT rowid, {",".join("[%s]" % c for c in editable)} FROM main.[{table}] WHERE ItemClass=?', (name,)):
                cells = {**dict(zip(editable, values)), **changes}
                db.execute(f'UPDATE main.[{table}] SET {", ".join(f"[{c}]=?" for c in cells)} WHERE rowid=?',
                           list(cells.values()) + [rowid])
                written += 1
    for table in TABLES:
        for name in stale:
            db.execute(f'DELETE FROM main.[{table}] WHERE ItemClass=?', (name,))
        for item in items:
            db.execute(f'DELETE FROM main.[{table}] WHERE ItemClass=?', (item['ItemClass'],))
            columns, rows = item_rows(db, table, item)
            db.executemany(f'INSERT INTO main.[{table}]({",".join("[%s]" % c for c in columns)}) '
                           f'VALUES({",".join("?" * len(columns))})', rows)
            written += len(rows)
    set_columns = [r[1] for r in db.execute(f'PRAGMA main.table_info([{SET_TABLE}])')]
    if set_columns:
        for name in stale + sorted(current):
            db.execute(f'DELETE FROM main.[{SET_TABLE}] WHERE ItemClass=?', (name,))
        for item in items:
            columns, rows = set_rows(db, item)
            db.executemany(f'INSERT INTO main.[{SET_TABLE}]({",".join("[%s]" % c for c in columns)}) '
                           f'VALUES({",".join("?" * len(columns))})', rows)
            written += len(rows)
    return written


def text_entries(items: list[dict], overrides: dict[str, dict] | None = None) -> dict[str, str]:
    """Localized tokens the client looks up: HEROES_ITEM_NAME_/DESC_<ITEMCLASS upper>."""
    entries = {}
    sources = [(item['ItemClass'], item.get('text', {})) for item in items]
    sources += [(name, o.get('text', {})) for name, o in (overrides or {}).items()]
    for name, text in sources:
        if text.get('name') is not None:
            entries['HEROES_ITEM_NAME_' + name.upper()] = text['name']
        if text.get('desc') is not None:
            entries['HEROES_ITEM_DESC_' + name.upper()] = text['desc']
    return entries
