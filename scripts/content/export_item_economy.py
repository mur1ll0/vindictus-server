"""Readonly ItemClassInfo economy columns bound by row position to the item catalog projection."""
import argparse, contextlib, datetime, json, pathlib, sqlite3
import content_additions
from export_item_catalog import read_package as catalog_read, digest
import content_overrides as overrides

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
    with contextlib.closing(content_additions.connect(database)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        # Same SELECT * provider order as the catalog projection; rows bind by position.
        rows = []
        # ItemContents.setInfoByItemClass: SetItemInfo rows by ItemClass, each SetInfo of its
        # SetID (GetSetInfo is null without a SetInfo row); the first one decides IsSetItem.
        set_ids = {r['SetID'] for r in db.execute('Select SetID From [SetInfo]')}
        first_set = {}
        for r in db.execute('Select ItemClass, SetID From [SetItemInfo]'):
            first_set.setdefault(r['ItemClass'], r['SetID'] if r['SetID'] in set_ids else None)
        def plain(v):
            if v is None: return None
            if not isinstance(v, str) or len(v) > 256: raise ValueError('Economy sort text')
            return v
        for r in overrides.select(db, 'ItemClassInfo'):
            rows.append(dict(item_utf16=text(r['ItemClass']), category_utf16=text(r['Category']),
                             unique=integer(r['Unique']), expire_in=integer(r['ExpireIn'], True),
                             price=integer(r['Price']), sell_price=integer(r['SellPrice']),
                             required_level=integer(r['RequiredLevel']),
                             class_restriction=integer(r['ClassRestriction']),
                             use_in_town=integer(r['UseInTown']), indestructible=integer(r['Indestructible']),
                             # ItemClassInfoConstraintTrait columns (IsConstraintSatisfied).
                             level_upper_bound=integer(r['LevelUpperBound']), cafe_only=integer(r['IsCafeOnly']),
                             required_skill_utf16=text(r['RequiredSkill']), required_skill_rank=integer(r['RequiredSkillRank']),
                             required_enhance=integer(r['RequiredEnhanceLevel']), required_enchant=integer(r['RequiredEnchantLevel']),
                             # ItemOwner.MakeConsumablesInfo (battle inventory of mission entry).
                             quick_slot_max_stack=integer(r['QuickSlotMaxStack']), draft=integer(r['Draft']),
                             # ItemOwner.ItemComparer (SortInventory) and GetInventoryFilterType.
                             sort_order=plain(r['SortOrder']), level=integer(r['Level']),
                             inventory_filter=plain(r['InventoryFilterName']), set_id=plain(first_set.get(r['ItemClass']))))
            if len(rows) > 50000: raise ValueError('Economy row bound')
        # ItemContents.HotSpringPotionInfoDic (ToDictionary by ItemClass: a repeated class throws).
        hot_spring = []
        for r in db.execute('Select * From [HotSpringPotionInfo]'):
            if any(h['item_class'] == r['ItemClass'] for h in hot_spring): raise ValueError('Repeated hot spring potion')
            hot_spring.append(dict(item_class=plain(r['ItemClass']), status_effect=plain(r['StatusEffect']),
                                   level=integer(r['Level']), bath_time_sec=integer(r['BathTimeSec']),
                                   overlap_min_sec=integer(r['OverlapMinSec']), all_announce=integer(r['AllAnnounce'])))
            if len(hot_spring) > 1000: raise ValueError('Hot spring row bound')
        # AlchemistBoxInfo box classes: their inner consumables are not migrated (refused).
        boxes = sorted({text(r['BoxItemClass']) or '' for r in db.execute('Select * From [AlchemistBoxInfo]')})
        pets = read_pets(db, integer, plain)
        if pets is not None:
            pets['features'] = pet_features(db, catalog['manifest'])
    if [r['item_utf16'] for r in rows] != [c['item_utf16'] for c in catalog['classes']]:
        raise ValueError('Economy rows differ from the catalog projection')
    m = catalog['manifest']
    package = {'format_version': 4, 'source': 'heroesContents.item-economy', 'alchemist_boxes': boxes,
               'manifest': dict(database_sha256=m['database_sha256'], config_sha256=m['config_sha256'], class_rows=len(rows)),
               'classes': rows, 'hot_spring': hot_spring}
    if pets is not None:
        package['pets'] = pets
    return package

def dotnet_ticks(text):
    """SQLite DATETIME text ('2012-02-13 17:00:00.000') as DateTime ticks (the source compares
    it with DateTime.UtcNow, so the value is UTC)."""
    if not isinstance(text, str): raise ValueError('Pet period date type')
    moment = datetime.datetime.strptime(text, '%Y-%m-%d %H:%M:%S.%f')
    delta = moment - datetime.datetime(1, 1, 1)
    return (delta.days * 86400 + delta.seconds) * 10_000_000 + delta.microseconds * 10

PET_FEATURES = ('Pet', 'Pet_food_V2', 'PetPurchaseLimit', 'PetMaxLevel', 'Pet_ExpBattle_Point',
                'NamingRuleMaxBytes')

def pet_features(db, manifest):
    """FeatureMatrix values of the pet system for the catalog's region (and its override),
    as FeatureMatrix.GetInteger/GetFloat read them."""
    columns = {r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')}
    def load(tag):
        if tag not in columns: raise ValueError('Missing configured feature column')
        return {r[0]: r[1] for r in db.execute('SELECT Feature,"' + tag.replace('"', '""') + '" FROM FeatureMatrix')}
    values = load(manifest['region'])
    if manifest.get('override_region'):
        for k, v in load(manifest['override_region']).items():
            if k not in ('GameCode', 'CurrentVer', 'OverrideSetting'): values[k] = v
    out = []
    for key in PET_FEATURES:
        value = values.get(key)
        if value is not None and (not isinstance(value, str) or len(value) > 256): raise ValueError('Pet feature value')
        out.append(dict(key=key, value=value))
    return out

def read_pets(db, integer, plain):
    """ItemContents pet dictionaries (PetInfo, PetSkillInfo, PetStatusBalanceInfo,
    DefaultPetSkillInfo, PetActiveTimePeriodInfo with its Feature, PetFeedInfo) and the
    ExtraData of the pet_ items (pet type, skill, feed amount). None without PetInfo."""
    tables = {r[0] for r in db.execute("Select name From sqlite_master Where type='table'")}
    if 'PetInfo' not in tables:
        return None
    def unique(rows, key, what):
        if len({key(r) for r in rows}) != len(rows): raise ValueError('Repeated ' + what)
        if len(rows) > 10000: raise ValueError(what + ' row bound')
        return rows
    infos = unique([dict(pet_id=integer(r['PetID']), default_level=integer(r['DefaultLevel']),
                         stat_type=integer(r['StatBalanceType']), max_active_time=integer(r['MaxActiveTime']),
                         item_class=plain(r['ItemClass']), max_hunger=integer(r['MaxHunger']),
                         hunger_time=integer(r['HungerTime']), hunger_decrease=integer(r['HungerDecrease']))
                    for r in db.execute('Select * From [PetInfo]')], lambda r: r['pet_id'], 'PetInfo')
    balance = unique([dict(stat_type=integer(r['StatType']), level=integer(r['Level']),
                           required_exp=integer(r['RequiredExp']), max_exp=integer(r['MaxExp']), hp=integer(r['HP']),
                           res_damage=integer(r['ResDamage']), hp_recovery=integer(r['HPRecovery']),
                           def_break=integer(r['DefBreak']), atk_balance=integer(r['AtkBalance']), atk=integer(r['Atk']),
                           defense=integer(r['Def']), critical=integer(r['Critical']), res_critical=integer(r['ResCritical']))
                      for r in db.execute('Select * From [PetStatusBalanceInfo]')],
                     lambda r: (r['stat_type'], r['level']), 'PetStatusBalanceInfo')
    skills = unique([dict(skill_id=integer(r['SkillID']), cool_time_type=integer(r['CoolTimeType'], True))
                     for r in db.execute('Select * From [PetSkillInfo]')], lambda r: r['skill_id'], 'PetSkillInfo')
    defaults = [dict(pet_id=integer(r['PetID']), slot_order=integer(r['SlotOrder']), skill_id=integer(r['PetSkillID']),
                     open_level=integer(r['OpenLevel']))
                for r in db.execute('Select * From [DefaultPetSkillInfo]')] if 'DefaultPetSkillInfo' in tables else []
    if len(defaults) > 10000: raise ValueError('DefaultPetSkillInfo row bound')
    periods = [dict(pet_id=integer(r['PetID']), date_from_ticks=dotnet_ticks(r['DateFrom']), period=integer(r['Period']),
                    feature=plain(r['Feature']))
               for r in db.execute('Select * From [PetActiveTimePeriodInfo]')] if 'PetActiveTimePeriodInfo' in tables else []
    if len(periods) > 10000: raise ValueError('PetActiveTimePeriodInfo row bound')
    feeds = unique([dict(feed_type=integer(r['FeedType']), item_class=plain(r['ItemClass']))
                    for r in db.execute('Select * From [PetFeedInfo]')], lambda r: r['feed_type'], 'PetFeedInfo')
    if any(not 0 <= f['feed_type'] <= 255 for f in feeds): raise ValueError('PetFeedInfo byte range')
    items = []
    for r in db.execute('Select ItemClass, ExtraData From [ItemClassInfo]'):
        if isinstance(r['ItemClass'], str) and r['ItemClass'].startswith('pet_'):
            row = dict(item_class=plain(r['ItemClass']), extra_data=plain(r['ExtraData']))
            # Repeated ItemClassInfo rows are kept by the catalog; they must agree here.
            if row not in items:
                items.append(row)
    unique(items, lambda r: r['item_class'], 'pet item')
    return dict(infos=infos, balance=balance, skills=skills, default_skills=defaults, periods=periods,
                feeds=feeds, items=items)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--output', required=True); args = parser.parse_args()
    project = pathlib.Path(__file__).resolve().parents[2]; base = project.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    output = pathlib.Path(args.output).resolve(); allowed = (project/'.cache/private-content').resolve()
    if not output.is_relative_to(allowed): raise ValueError('Private output path outside ACL cache')
    data = json.dumps(read_package(base/'heroesContents.db3', base/'ServiceCore.dll.config'), ensure_ascii=True, separators=(',', ':'))
    if len(data.encode('utf-8')) > 16*1024*1024: raise ValueError('Private export size bound')
    output.write_text(data, encoding='utf-8', newline='\n')
    print(json.dumps({'classes': data.count('"item_utf16"'), 'bytes': len(data)}))
