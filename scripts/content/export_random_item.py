"""Read-only random item contents: RandomItem, RandomItemRecipeClass and RandomItemRecipe
(RandomItemContents) in provider order, RandomItemV2 and RandomItemV2Recipe
(RandomItemContentsV2, dispatched before RandomItemContents), ItemClassInfo.Level aligned with the catalog's class rows, and the
region values of the features they read. Float cells are exported as the Double that
System.Data.SQLite reads (text cells are converted by SQLite itself). The tables are the
same in the original content."""
import argparse, contextlib, json, pathlib, sqlite3, struct
from export_item_catalog import read_package as catalog_read, digest
from export_npc_scripts import region_features
from export_reward_policy import feature_atoms

ROOT = pathlib.Path(__file__).resolve().parents[2]


def text(v, limit=4096):
    if v is None:
        return None
    if not isinstance(v, str):
        raise ValueError('RandomItem text type')
    b = v.encode('utf-16-le', errors='surrogatepass')
    if len(b) > limit:
        raise ValueError('RandomItem text bound')
    return b.hex()


def integer(v, low=-(2**31), high=2**31, nullable=False):
    if v is None and nullable:
        return None
    if type(v) is not int or not low <= v < high:
        raise ValueError('RandomItem integer type/range')
    return v


def read_package(database, config):
    package = catalog_read(database, config)
    manifest = package['manifest']
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        db.row_factory = sqlite3.Row

        def double(v, nullable=False):
            if v is None and nullable:
                return None
            if isinstance(v, str):
                v = db.execute('SELECT CAST(? AS REAL)', (v,)).fetchone()[0]
            if type(v) not in (int, float):
                raise ValueError('RandomItem Double type')
            return struct.pack('>d', float(v)).hex()

        levels = []
        for index, r in enumerate(db.execute('Select * From [ItemClassInfo]')):
            if index >= len(package['classes']):
                raise ValueError('Catalog alignment changed')
            key = text(r['ItemClass'])
            if key != package['classes'][index]['item_utf16'] or r['Feature'] != package['classes'][index]['feature']:
                raise ValueError('Catalog order changed')
            levels.append(integer(r['Level']))
        if len(levels) != len(package['classes']):
            raise ValueError('Catalog alignment changed')
        boxes = [dict(id=integer(r['ID']), server_code=integer(r['ServerCode'], nullable=True),
                      item_utf16=text(r['ItemClass']), key_utf16=text(r['KeyItemClass']),
                      visual=integer(r['Visual'], nullable=True), feature=r['Feature'])
                 for r in db.execute('Select * From [RandomItem]')]
        classes = []
        for r in db.execute('Select * From [RandomItemRecipeClass]'):
            # HeroesContentsLoader: a text cell in the Int32 ItemNumber throws in SetValue; the
            # row is still yielded with ItemNumber 0 and the later properties unset.
            broken = isinstance(r['ItemNumber'], str)
            classes.append(dict(id=integer(r['RandomItemID']), name_utf16=text(r['ClassName']),
                                expected=integer(r['ExpectedAppearanceCount'], nullable=True),
                                probability=double(r['Probability']),
                                number=0 if broken else integer(r['ItemNumber']),
                                rule=None if broken else text(r['AdditionalRule'])))
        recipes = [[integer(r['RandomItemID']), text(r['RecipeClassName']),
                    integer(r['CharacterType'], nullable=True), text(r['ItemClass']), integer(r['Number']),
                    double(r['Probability']), integer(r['Color1'], nullable=True),
                    integer(r['Color2'], nullable=True), integer(r['Color3'], nullable=True),
                    text(r['NotifyMessage']), integer(r['IsVariable']), double(r['HaveItem']),
                    double(r['DontHaveItem']), r['Feature']]
                   for r in db.execute('Select * From [RandomItemRecipe]')]
        def boolean(v):
            # Convert.ChangeType of the bit cells ('True'/'False' text in the content).
            if isinstance(v, str) and v.lower() in ('true', 'false'):
                return v.lower() == 'true'
            if type(v) is int and v in (0, 1):
                return v == 1
            raise ValueError('RandomItem Boolean type')

        v2 = [dict(item_utf16=text(r['ItemClass']), key_utf16=text(r['KeyItemClass']),
                   random_item_utf16=text(r['RandomItemID']), show_pop_up=boolean(r['ShowPopUp']),
                   titicore=boolean(r['IsTitiCore']), cool_time=integer(r['CoolTime'], nullable=True),
                   exclusive_utf16=text(r['ExclusiveID']), feature=r['Feature'])
              for r in db.execute('Select * From [RandomItemV2]')]
        # RandomItemID, ItemClass, Count, RecipeType, Probability, RareItemID, Cond_Character,
        # Cond_Item, Cond_Feature, Color1-3, CharacterBind, ExpireBySec, Announce, Message.
        v2_recipes = [[text(r['RandomItemID']), text(r['ItemClass']), integer(r['Count']),
                       integer(r['RecipeType'], 0, 256), double(r['Probability']),
                       integer(r['RareItemID'], nullable=True), text(r['Cond_Character']),
                       text(r['Cond_Item']), r['Cond_Feature'], integer(r['Color1']),
                       integer(r['Color2']), integer(r['Color3']), boolean(r['CharacterBind']),
                       integer(r['ExpireBySec'], nullable=True), text(r['Announce']), text(r['Message'])]
                      for r in db.execute('Select * From [RandomItemV2Recipe]')]
        values = region_features(db, config)
    featured = ([b['feature'] for b in boxes] + [r[13] for r in recipes] + [v['feature'] for v in v2]
                + [r[8] for r in v2_recipes])
    if any(f is not None and not isinstance(f, str) for f in featured):
        raise ValueError('RandomItem feature type')
    # Values read by UseInventoryItemProcessor and RandomItemManager.
    atoms = {'RandomItemV2', 'RandomItemBundle', 'RandomItem_FeverSystem', 'Titicore',
             'VariableProbability_RandomItem', 'VariableItem_ConstraintCheck', 'GameCode'}
    for f in featured:
        atoms |= feature_atoms(f or '')
    if len(atoms) > 512 or any(not k.isascii() or not k or len(k) > 256 for k in atoms):
        raise ValueError('RandomItem feature subset bound')
    features = [dict(key=k, value=values.get(k)) for k in sorted(atoms)]
    if digest(database) != manifest['database_sha256'] or digest(config) != manifest['config_sha256']:
        raise ValueError('RandomItem source changed during export')
    return dict(format_version=2, source='heroesContents.random-item', database_sha256=manifest['database_sha256'],
                config_sha256=manifest['config_sha256'], levels=levels, boxes=boxes, classes=classes,
                recipes=recipes, v2_boxes=v2, v2_recipes=v2_recipes, features=features)


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
    if len(data.encode()) > 48 * 1024 * 1024:
        raise ValueError('RandomItem profile bound exceeded')
    output.write_text(data, encoding='utf-8', newline='\n')
    print('RandomItem contents exported: %d boxes, %d classes, %d recipes; values private'
          % (len(package['boxes']), len(package['classes']), len(package['recipes'])))


if __name__ == '__main__':
    main()
