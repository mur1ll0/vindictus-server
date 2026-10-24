"""Read-only enhance contents: EquipItemInfo.EnhanceType aligned with the catalog's
equipment rows, EnhanceInfo/EnhanceStatInfo/EnhanceServerInfo/EnhanceBonusServerInfo/
EnhanceInsteadMaterialInfo in provider order, and the region values of their features.
SuccessRatio comes from the original content (content_overrides). REAL columns are
exported as the Single the server's System.Data.SQLite reads (REAL -> DbType.Single;
text cells are converted by SQLite itself)."""
import argparse, contextlib, json, pathlib, sqlite3, struct
import content_overrides as overrides
from export_item_catalog import read_package as catalog_read, digest
from export_npc_scripts import region_features
from export_reward_policy import feature_atoms

ROOT = pathlib.Path(__file__).resolve().parents[2]


def text(v):
    if v is None:
        return None
    if not isinstance(v, str):
        raise ValueError('Enhance text type')
    b = v.encode('utf-16-le', errors='surrogatepass')
    if len(b) > 1024:
        raise ValueError('Enhance text bound')
    return b.hex()


def integer(v, low=-(2**31), high=2**31):
    if type(v) is not int or not low <= v < high:
        raise ValueError('Enhance integer type/range')
    return v


def double(v):
    if type(v) not in (int, float):
        raise ValueError('Enhance Double type')
    return struct.pack('>d', float(v)).hex()


def read_package(database, config):
    package = catalog_read(database, config)
    manifest = package['manifest']
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        db.row_factory = sqlite3.Row

        def single(v):
            # sqlite3_column_double (SQLite converts a text cell), then (float).
            if isinstance(v, str):
                v = db.execute('SELECT CAST(? AS REAL)', (v,)).fetchone()[0]
            if type(v) not in (int, float):
                raise ValueError('Enhance Single type')
            return struct.pack('>f', float(v)).hex()

        equips = []
        for index, r in enumerate(db.execute('Select * From [EquipItemInfo]')):
            if index >= len(package['equips']):
                raise ValueError('Catalog alignment changed')
            key = text(r['ItemClass'])
            if key != package['equips'][index]['item_utf16'] or r['Feature'] != package['equips'][index]['feature']:
                raise ValueError('Catalog order changed')
            equips.append(dict(source_index=index, item_utf16=key, feature=r['Feature'],
                               enhance_type=text(r['EnhanceType'])))
        if len(equips) != len(package['equips']):
            raise ValueError('Catalog alignment changed')
        enhance = [dict(source_index=i, enhance_type=text(r['EnhanceType']), level=integer(r['EnhanceLevel']),
                        material1=text(r['MaterialClass1']), num1=integer(r['MaterialNum1']),
                        material2=text(r['MaterialClass2']), num2=integer(r['MaterialNum2']),
                        gold=integer(r['Gold']), success_ratio=single(r['SuccessRatio']), on_fail=text(r['OnFail']),
                        on_fail_recipe_ratio=single(r['OnFailRecipeRatio']),
                        on_fail_material_num1=integer(r['OnFailMaterialNum1']), effect=integer(r['Effect']),
                        feature=r['Feature'])
                   for i, r in enumerate(overrides.select(db, 'EnhanceInfo'))]
        stats = [dict(source_index=i, enhance_type=text(r['EnhanceType']), level=integer(r['EnhanceLevel']),
                      stat=text(r['Stat']), value_type=text(r['ValueType']), value=single(r['Value']),
                      base=text(r['Base']))
                 for i, r in enumerate(db.execute('Select * From [EnhanceStatInfo]'))]
        server = [dict(source_index=i, enhance_type=text(r['EnhanceType']), level=integer(r['EnhanceLevel']),
                       success_ratio=single(r['SuccessRatio']), feature=r['Feature'])
                  for i, r in enumerate(overrides.select(db, 'EnhanceServerInfo'))]
        bonus = [dict(source_index=i, item_utf16=text(r['ItemClass']), bonus=double(r['Bonus']),
                      min=integer(r['EnhanceMin'], -(2**15), 2**15), max=integer(r['EnhanceMax'], -(2**15), 2**15))
                 for i, r in enumerate(db.execute('Select * From [EnhanceBonusServerInfo]'))]
        instead = [dict(source_index=i, item_utf16=text(r['ItemClass']), use_count=integer(r['UseCount']),
                        slot=integer(r['Slot'], 0, 256), min=integer(r['EnhanceMin'], -(2**15), 2**15),
                        max=integer(r['EnhanceMax'], -(2**15), 2**15))
                   for i, r in enumerate(db.execute('Select * From [EnhanceInsteadMaterialInfo]'))]
        values = region_features(db, config)
    if any(r['feature'] is not None and not isinstance(r['feature'], str) for r in enhance + server):
        raise ValueError('Enhance feature type')
    atoms = set()
    for r in enhance + server:
        atoms |= feature_atoms(r['feature'] or '')
    if len(atoms) > 256 or any(not k.isascii() or not k or len(k) > 256 for k in atoms):
        raise ValueError('Enhance feature subset bound')
    features = [dict(key=k, value=values.get(k)) for k in sorted(atoms)]
    if any(f['value'] is not None and (not isinstance(f['value'], str) or len(f['value']) > 256) for f in features):
        raise ValueError('Enhance feature value bound')
    if digest(database) != manifest['database_sha256'] or digest(config) != manifest['config_sha256']:
        raise ValueError('Enhance source changed during export')
    return dict(format_version=1, source='heroesContents.item-enhance', database_sha256=manifest['database_sha256'],
                config_sha256=manifest['config_sha256'], equips=equips, enhance=enhance, stats=stats,
                server=server, bonus=bonus, instead=instead, features=features)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    base = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    output = pathlib.Path(args.output).resolve()
    if output.parent != (ROOT / '.cache/private-content').resolve():
        raise ValueError('Private output outside ACL cache')
    package = read_package(base / 'heroesContents.db3', base / 'ServiceCore.dll.config')
    data = json.dumps(package, ensure_ascii=True, separators=(',', ':'))
    if len(data.encode()) > 16 * 1024 * 1024:
        raise ValueError('Enhance profile bound exceeded')
    output.write_text(data, encoding='utf-8', newline='\n')
    print('Enhance contents exported: %d equipment, %d enhance, %d stat, %d server rows; values private'
          % (len(package['equips']), len(package['enhance']), len(package['stats']), len(package['server'])))


if __name__ == '__main__':
    main()
