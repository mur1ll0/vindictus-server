"""Readonly ItemClassInfo economy columns bound by row position to the item catalog projection."""
import argparse, contextlib, json, pathlib, sqlite3
from export_item_catalog import read_package as catalog_read, digest

COLUMNS = ('Unique', 'ExpireIn', 'Price', 'SellPrice', 'RequiredLevel', 'ClassRestriction', 'UseInTown')

def read_package(database, config):
    catalog = catalog_read(database, config)
    def text(v):
        if v is None: return None
        if not isinstance(v, str): raise ValueError('Economy text type')
        b = v.encode('utf-16-le', errors='surrogatepass')
        if len(b) > 1024: raise ValueError('Economy text bound')
        return b.hex()
    def integer(v, nullable=False):
        if v is None and nullable: return None
        if type(v) is not int or not -(2**31) <= v < 2**31:
            raise ValueError('Economy Int32 type/range')
        return v
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        # Same SELECT * provider order as the catalog projection; rows bind by position.
        rows = []
        for r in db.execute('Select * From [ItemClassInfo]'):
            rows.append(dict(item_utf16=text(r['ItemClass']), category_utf16=text(r['Category']),
                             unique=integer(r['Unique']), expire_in=integer(r['ExpireIn'], True),
                             price=integer(r['Price']), sell_price=integer(r['SellPrice']),
                             required_level=integer(r['RequiredLevel']),
                             class_restriction=integer(r['ClassRestriction']),
                             use_in_town=integer(r['UseInTown'])))
            if len(rows) > 50000: raise ValueError('Economy row bound')
    if [r['item_utf16'] for r in rows] != [c['item_utf16'] for c in catalog['classes']]:
        raise ValueError('Economy rows differ from the catalog projection')
    m = catalog['manifest']
    return {'format_version': 1, 'source': 'heroesContents.item-economy',
            'manifest': dict(database_sha256=m['database_sha256'], config_sha256=m['config_sha256'], class_rows=len(rows)),
            'classes': rows}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--output', required=True); args = parser.parse_args()
    project = pathlib.Path(__file__).resolve().parents[2]; base = project.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    output = pathlib.Path(args.output).resolve(); allowed = (project/'.cache/private-content').resolve()
    if not output.is_relative_to(allowed): raise ValueError('Private output path outside ACL cache')
    data = json.dumps(read_package(base/'heroesContents.db3', base/'ServiceCore.dll.config'), ensure_ascii=True, separators=(',', ':'))
    if len(data.encode('utf-8')) > 16*1024*1024: raise ValueError('Private export size bound')
    output.write_text(data, encoding='utf-8', newline='\n')
    print(json.dumps({'classes': data.count('"item_utf16"'), 'bytes': len(data)}))
