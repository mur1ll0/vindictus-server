"""Readonly, source-bound normal-equipment metadata for an explicit class subset."""
import argparse, contextlib, json, pathlib, sqlite3
from export_item_catalog import read_package as catalog_read, digest

ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'src/domain/equipped_gear_sources.json'

def read_package(database, config, selected):
    if not 1 <= len(selected) <= 4096 or len(set(selected)) != len(selected):
        raise ValueError('Equipment subset is empty, duplicated or oversized')
    normalized = {bytes.fromhex(s).hex() for s in selected}
    if len(normalized) != len(selected): raise ValueError('Duplicate encoded equipment class')
    selected = normalized
    if any(len(bytes.fromhex(s)) > 1024 or len(bytes.fromhex(s)) % 2 for s in selected):
        raise ValueError('Equipment class UTF16 bound')
    package = catalog_read(database, config)
    sources = json.loads(MANIFEST.read_text(encoding='utf-8-sig'))
    for row in sources.values():
        if digest(ROOT / row['path']) != row['sha256']:
            raise ValueError('Equipment implementation source changed')
    def text(v):
        if v is None: return None
        if not isinstance(v, str): raise ValueError('Equipment text type')
        b = v.encode('utf-16-le', errors='surrogatepass')
        if len(b) > 1024: raise ValueError('Equipment text bound')
        return b.hex()
    def integer(v):
        if type(v) is not int or not -(2**31) <= v < 2**31:
            raise ValueError('Equipment Int32 type/range')
        return v
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro', uri=True)) as db:
        db.row_factory = sqlite3.Row
        classes=[]; equips=[]
        for i, row in enumerate(db.execute('SELECT * FROM ItemClassInfo')):
            if text(row['ItemClass']) in selected:
                classes.append(dict(source_index=i, item_utf16=text(row['ItemClass']), feature=row['Feature'], category_utf16=text(row['Category']), level=integer(row['Level'])))
        for i, row in enumerate(db.execute('SELECT * FROM EquipItemInfo')):
            if text(row['ItemClass']) in selected:
                equips.append(dict(source_index=i, item_utf16=text(row['ItemClass']), feature=row['Feature'], equip_utf16=text(row['EquipClass']), serial=integer(row['CostumeType'])))
        if {c['item_utf16'] for c in classes} != selected or {e['item_utf16'] for e in equips} != selected:
            raise ValueError('Selected equipment missing from source tables')
        sets=[]; wanted=set()
        definitions={text(row['SetID']) for row in db.execute('SELECT * FROM SetInfo')}
        for row in db.execute('SELECT * FROM SetItemInfo'):
            if text(row['ItemClass']) in selected:
                key=text(row['SetID'])
                if key is None or key not in definitions: raise ValueError('Unresolved equipment set')
                sets.append(dict(item_utf16=text(row['ItemClass']),set_utf16=key));wanted.add(key)
        effects=[dict(set_utf16=text(r['SetID']),count=integer(r['SetCount']),target_utf16=text(r['EffectTarget']),amount=integer(r['Amount'])) for r in db.execute('SELECT * FROM SetEffectInfo') if text(r['SetID']) in wanted]
    if digest(database) != package['manifest']['database_sha256'] or digest(config) != package['manifest']['config_sha256']:
        raise ValueError('Equipment source changed during export')
    return dict(format_version=1,source='heroesContents.normal-equipment',database_sha256=package['manifest']['database_sha256'],config_sha256=package['manifest']['config_sha256'],implementations={k:r['sha256'] for k,r in sources.items()},classes=classes,equips=equips,sets=sets,effects=effects)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);parser.add_argument('--classes-file',required=True);args=parser.parse_args()
    out=pathlib.Path(args.output).resolve();private=(ROOT/'.cache/private-content').resolve()
    source=pathlib.Path(args.classes_file).resolve()
    if out.parent != private or source.parent != private: raise ValueError('Use prepared private content directory')
    selected=[r['class_hex'] for r in json.loads(source.read_text(encoding='utf-8-sig'))]
    base=ROOT.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config',selected)
    with out.open('x',encoding='utf-8',newline='\n') as stream: json.dump(package,stream,separators=(',',':'),ensure_ascii=True);stream.write('\n')
    print('Private readonly equipment profile exported; values omitted.')

if __name__ == '__main__': main()
