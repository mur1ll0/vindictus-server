"""Read-only random item contents: RandomItem, RandomItemRecipeClass and RandomItemRecipe
(RandomItemContents) in provider order, RandomItemV2 and RandomItemV2Recipe
(RandomItemContentsV2, dispatched before RandomItemContents), ItemClassInfo.Level aligned with the catalog's class rows, and the
region values of the features they read. Float cells are exported as the Double that
System.Data.SQLite reads (text cells are converted by SQLite itself). The tables are the
same in the original content."""
import argparse, contextlib, json, pathlib, sqlite3, struct
import content_additions
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


# The server's own RandomItemV2 package rows (RecipeType 0 only): 2657 of 2685 recipes roll
# Probability 1.0, 2616 send Message GameUI_Heroes_RandomItem_GetItem and 324 of 367 boxes
# show the pop-up. The NA client tables keep no Probability/Message/ShowPopUp columns, so an
# imported package takes these values.
NA_PACKAGE_PROBABILITY = 1.0
NA_PACKAGE_MESSAGE = 'GameUI_Heroes_RandomItem_GetItem'


def na_packages(na_path, package, boxes, v2, v2_recipes, boolean):
    """RandomItemV2 boxes of the NA client database (RandomItemV2_Client and
    RandomItemV2Recipe_Client) for catalog classes the server content has no box for. Only
    deterministic packages are taken: every recipe RecipeType 0 with a catalog item, a positive
    Count, no Feature, no ExpireByMin and no '#'/'@'/'$' class; boxes with a Feature, a
    RandomItemID the server already uses or any other recipe are skipped."""
    catalog = {c['item_utf16'] for c in package['classes']}
    taken = {b['item_utf16'] for b in boxes} | {b['item_utf16'] for b in v2}
    used_ids = {r[0] for r in v2_recipes} | {b['random_item_utf16'] for b in v2}
    with contextlib.closing(sqlite3.connect(pathlib.Path(na_path).resolve().as_uri() + '?mode=ro', uri=True)) as na:
        na.execute('PRAGMA query_only=ON')
        na.row_factory = sqlite3.Row
        recipes = {}
        for r in na.execute('Select * From [RandomItemV2Recipe_Client]'):
            recipes.setdefault(r['RecipeClassName'], []).append(r)
        added_boxes, added_recipes = [], []
        for b in na.execute('Select * From [RandomItemV2_Client]'):
            item, rid = text(b['ItemClass']), text(b['RandomItemID'])
            rows = recipes.get(b['RandomItemID'], [])
            if (b['Feature'] is not None or item not in catalog or item in taken or rid in used_ids
                    or not rows):
                continue
            if not all(type(r['RecipeType']) is int and r['RecipeType'] == 0 and r['Feature'] is None
                       and r['ExpireByMin'] is None and type(r['Count']) is int and r['Count'] > 0
                       and isinstance(r['ItemClass'], str) and not r['ItemClass'].startswith(('#', '@', '$'))
                       and text(r['ItemClass']) in catalog for r in rows):
                continue
            taken.add(item)
            used_ids.add(rid)
            added_boxes.append(dict(item_utf16=item, key_utf16=None, random_item_utf16=rid, show_pop_up=True,
                                    titicore=False, cool_time=None, exclusive_utf16=None, feature=None))
            for r in rows:
                added_recipes.append([rid, text(r['ItemClass']), integer(r['Count']), 0,
                                      struct.pack('>d', NA_PACKAGE_PROBABILITY).hex(), None,
                                      text(r['Cond_Character']), None, None, integer(r['Color1']),
                                      integer(r['Color2']), integer(r['Color3']), boolean(r['CharacterBind']),
                                      None, None, text(NA_PACKAGE_MESSAGE)])
    return added_boxes, added_recipes


def read_package(database, config, na=None):
    package = catalog_read(database, config)
    manifest = package['manifest']
    with contextlib.closing(content_additions.connect(database)) as db:
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
        if na is not None:
            added_boxes, added_recipes = na_packages(na, package, boxes, v2, v2_recipes, boolean)
            v2 += added_boxes
            v2_recipes += added_recipes
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
    na = ROOT / '.cache/na-db/heroes-plain.db3'
    package = read_package(base / 'heroesContents.db3', base / 'ServiceCore.dll.config',
                           na if na.is_file() else None)
    data = json.dumps(package, ensure_ascii=True, separators=(',', ':'))
    if len(data.encode()) > 48 * 1024 * 1024:
        raise ValueError('RandomItem profile bound exceeded')
    output.write_text(data, encoding='utf-8', newline='\n')
    print('RandomItem contents exported: %d boxes, %d classes, %d recipes; values private'
          % (len(package['boxes']), len(package['classes']), len(package['recipes'])))


if __name__ == '__main__':
    main()
