"""Readonly ItemClassInfo economy columns bound by row position to the item catalog projection."""
import argparse, contextlib, datetime, json, pathlib, re, sqlite3
import collabs
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
                             inventory_filter=plain(r['InventoryFilterName']), set_id=plain(first_set.get(r['ItemClass'])),
                             # Item.IsMailableTo (TradeRestirction: null is tradable).
                             trade_restriction=plain(r['TradeRestirction']),
                             # TradeItemDataInfo.SetDataFromItemClassInfo (AddTradeItem).
                             trade_category=plain(r['TradeCategory']),
                             trade_category_sub=plain(r['TradeCategorySub'])))
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
        # ItemContents.mailTypeDict (MailTypeInfo ToDictionary by MailType: a repeated type throws).
        def flag(v):
            if v not in ('True', 'False'): raise ValueError('MailTypeInfo bit text')
            return v == 'True'
        mail_types = []
        for r in db.execute('Select * From [MailTypeInfo] Order By MailType'):
            row = dict(mail_type=integer(r['MailType']), description=plain(r['Description']), sender=plain(r['Sender']),
                       system=flag(r['System']), item_attached=flag(r['ItemAttached']), charged=flag(r['Charged']),
                       replyable=flag(r['Replyable']), returnable=flag(r['Returnable']), expire_in=integer(r['Expire_in']),
                       arrive_in=integer(r['Arrive_in']), fee=integer(r['Fee']), fee_for_item=integer(r['FeeForItem']),
                       fee_express=integer(r['Fee_Express']), fee_for_item_express=integer(r['FeeForItem_Express']))
            if not 0 <= row['mail_type'] <= 255 or any(m['mail_type'] == row['mail_type'] for m in mail_types):
                raise ValueError('MailTypeInfo type')
            mail_types.append(row)
            if len(mail_types) > 256: raise ValueError('MailTypeInfo row bound')
        # GuildContents.GuildLevelUpInfoDic and CharacterContents GuildLevelBonusInfo: raw rows
        # with their Feature, which the server's FeatureMatrix policy evaluates.
        tables = {r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        guild_levels = []
        for r in (db.execute('Select * From [GuildLevelUpInfo] Order By Level') if 'GuildLevelUpInfo' in tables else ()):
            guild_levels.append(dict(level=integer(r['Level']), required_exp=integer(r['RequiredExp']),
                                     reward=plain(r['LevelUpRewardItemClass']), feature=plain(r['Feature'])))
            if len(guild_levels) > 1000: raise ValueError('Guild level row bound')
        guild_bonuses = []
        for r in (db.execute('Select * From [GuildLevelBonusInfo] Order By Level') if 'GuildLevelBonusInfo' in tables else ()):
            guild_bonuses.append(dict(level=integer(r['Level']), bonus=plain(r['LevelBonus']),
                                      arg=integer(r['Arg']), feature=plain(r['Feature'])))
            if len(guild_bonuses) > 1000: raise ValueError('Guild bonus row bound')
        # RankContents.RankDescription (RankService): RankID → EventID with its score rules;
        # the Feature is evaluated by the server's policy.
        ranks = []
        for r in (db.execute('Select * From [RankDescription] Order By EventID') if 'RankDescription' in tables else ()):
            if any(x['rank_id'] == r['RankID'] for x in ranks): raise ValueError('Repeated RankID')
            ranks.append(dict(rank_id=plain(r['RankID']), event_id=integer(r['EventID']), score_type=plain(r['ScoreType']),
                              sort_type=plain(r['SortType']), score_result_type=plain(r['ScoreResultType']),
                              feature=plain(r['Feature']), score_limit=integer(r['ScoreLimit'], True),
                              is_guild=integer(r['IsGuild'], True)))
            if len(ranks) > 5000: raise ValueError('Rank description row bound')
        marketplace = read_marketplace(db, integer, plain, catalog['manifest'])
        events = read_events(db, catalog['manifest'], npc_xml_root(database))
        # AdminContents.EventTemplate (ToDictionary by Name: a repeated name throws).
        templates = []
        def script(v):
            if v is None: return None
            if not isinstance(v, str) or len(v) > 4000: raise ValueError('Event template text')
            return v
        if 'EventTemplate' in {r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}:
            for r in db.execute('Select * From [EventTemplate]'):
                if any(t['name'] == r['Name'] for t in templates): raise ValueError('Repeated event template')
                templates.append(dict(name=plain(r['Name']), feature=script(r['Feature']), start_script=script(r['StartScript']), end_script=script(r['EndScript'])))
                if len(templates) > 2000: raise ValueError('Event template bound')
        pets = read_pets(db, integer, plain)
        if pets is not None:
            pets['features'] = pet_features(db, catalog['manifest'])
    if [r['item_utf16'] for r in rows] != [c['item_utf16'] for c in catalog['classes']]:
        raise ValueError('Economy rows differ from the catalog projection')
    m = catalog['manifest']
    package = {'format_version': 4, 'source': 'heroesContents.item-economy', 'alchemist_boxes': boxes,
               'manifest': dict(database_sha256=m['database_sha256'], config_sha256=m['config_sha256'], class_rows=len(rows)),
               'classes': rows, 'hot_spring': hot_spring, 'mail_types': mail_types,
               'marketplace': marketplace, 'events': events, 'event_templates': templates,
               'event_packs': collabs.check_packs({e['key'] for e in events}),
               'guild_levels': guild_levels, 'guild_level_bonuses': guild_bonuses,
               'rank_descriptions': ranks}
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

MARKETPLACE_FEATURES = ('CurrentVer', 'MoneyLimitToTradeItem', 'TradeRegisterFee', 'UseTradePriceLimit',
                        'UseTradePriceLimitPercentage', 'PremiumTradeTicket', 'ItemPriceInfoInItemSlot',
                        'ItemPriceInfoInService', 'LevelLimitToTradeItem')

def region_features(db, manifest, keys, what):
    """FeatureMatrix values of `keys` for the catalog's region (and its override), as
    FeatureMatrix.GetString reads them."""
    columns = {r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')}
    def load(tag):
        if tag not in columns: raise ValueError('Missing configured feature column')
        return {r[0]: r[1] for r in db.execute('SELECT Feature,"' + tag.replace('"', '""') + '" FROM FeatureMatrix')}
    values = load(manifest['region'])
    if manifest.get('override_region'):
        for k, v in load(manifest['override_region']).items():
            if k not in ('GameCode', 'CurrentVer', 'OverrideSetting'): values[k] = v
    out = []
    for key in keys:
        value = values.get(key)
        if value is not None and (not isinstance(value, str) or len(value) > 256): raise ValueError(what + ' feature value')
        out.append(dict(key=key, value=value))
    return out

def read_marketplace(db, integer, plain, manifest):
    """The TradeSystem's FeatureMatrix values and ItemContents.itemTradeLimitInfo
    (ItemTradeLimitInfo ToDictionary by ItemClass: a repeated class throws)."""
    limits = []
    for r in db.execute('Select * From [ItemTradeLimitInfo]'):
        if any(l['item_class'] == r['ItemClass'] for l in limits): raise ValueError('Repeated ItemTradeLimitInfo')
        limits.append(dict(item_class=plain(r['ItemClass']), max_limit_value=integer(r['MaxLimitValue'], True)))
        if len(limits) > 10000: raise ValueError('ItemTradeLimitInfo row bound')
    return dict(features=region_features(db, manifest, MARKETPLACE_FEATURES, 'Marketplace'), trade_limits=limits)

# Region tokens of the other services: an event key carrying one is not ours.
OTHER_REGIONS = {'kokr', 'jajp', 'zhcn', 'zhtw', 'thth', 'kr', 'ko', 'jp', 'ja', 'cn', 'zh', 'tw', 'th',
                 'cntw', 'korea', 'japan', 'china', 'taiwan', 'thai'}
# An event switch is off (empty) or a version / "1"; integers and paths are event parameters.
SWITCH = re.compile(r'^(1|[0-9]+\.[0-9]+(\.[0-9]+)*)$')

# FeatureMatrix keys without the Event prefix that switch a whole event system on
# (PeriodicEventScheduler.ItemOwnerEvent, AllUserGoalEventScheduler).
MASTER_SWITCHES = {'BoostEvent': 'PeriodicEventInfo', 'AllUserGoalEvent': 'AllUserGoalInfo'}

def npc_xml_root(database):
    """The server's NPC scripts (Data/XML next to Bin/heroesContents.db3), or None."""
    root = pathlib.Path(database).resolve().parent.parent / 'Data/XML'
    return root if (root / 'NpcTalkInfo.xml').is_file() else None

def npc_feature_uses(xml_root):
    """FeatureMatrix keys the NPC scripts read: NpcTalkInfo defaults by their Feature (an NPC stays
    in town only while its default talk is enabled) and <feature> button conditions of the
    defaults, with the NPC (id, location) of each default."""
    from export_npc_scripts import load, condition_atoms
    from export_reward_policy import feature_atoms
    import hashlib
    hasher = hashlib.sha256()
    root = load(xml_root / 'NpcTalkInfo.xml', hasher)
    if root[0] != 'FileList': raise ValueError('NpcTalkInfo root')
    defaults, conditions, npcs = {}, {}, {}
    for element in root[3]:
        if element[0] != 'Default': continue
        feature = next((v for k, v in element[1] if k == 'Feature'), '')
        relative = pathlib.PureWindowsPath(element[2])
        path = xml_root.joinpath(*relative.parts).with_suffix('.xml')
        if not path.resolve().is_relative_to(xml_root.resolve()): raise ValueError('Default script outside XML root')
        tree = load(path, hasher)
        talk = next((c for c in tree[3] if c[0] == 'Talk'), None)
        npc = [dict(talk[1]).get('id'), dict(talk[1]).get('location')] if talk else None
        try:
            atoms = feature_atoms(feature) if feature else set()
        except ValueError:
            atoms = set()
        for atom in atoms:
            defaults[atom] = defaults.get(atom, 0) + 1
            if npc and npc[0] and npc not in npcs.setdefault(atom, []): npcs[atom].append(npc)
        found = []
        def walk(node):
            if node[0] == 'Condition':
                out = set(); condition_atoms(node, out); found.extend(out)
            else:
                for c in node[3]: walk(c)
        walk(tree)
        for atom in found:
            conditions[atom] = conditions.get(atom, 0) + 1
    return defaults, conditions, npcs

def read_events(db, manifest, xml_root=None):
    """The FeatureMatrix event switches of the region (keys starting with Event, without another
    region's token) that some content row or NPC script references, with the rows per table. The
    collab keys (collabs.py) are listed with their region (`adapted`) even from another service."""
    from export_reward_policy import feature_atoms
    columns = {r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')}
    def load(tag):
        if tag not in columns: raise ValueError('Missing configured feature column')
        return {r[0]: r[1] for r in db.execute('SELECT Feature,"' + tag.replace('"', '""') + '" FROM FeatureMatrix')}
    values = load(manifest['region'])
    if manifest.get('override_region'):
        for k, v in load(manifest['override_region']).items():
            if k not in ('GameCode', 'CurrentVer', 'OverrideSetting'): values[k] = v
    keys = {}
    for key, value in values.items():
        if key in MASTER_SWITCHES:
            keys[key] = value or None
            continue
        if not isinstance(key, str) or not key.lower().startswith('event') or not key.isascii() or len(key) > 256:
            continue
        if OTHER_REGIONS & set(key.lower().split('_')) and not collabs.collab(key):
            continue
        if value not in (None, '') and not (isinstance(value, str) and SWITCH.match(value)):
            continue
        keys[key] = value or None
    uses = {k: {} for k in keys}
    tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
    for table in tables:
        if table == 'FeatureMatrix': continue
        if 'Feature' not in {r[1] for r in db.execute('PRAGMA table_info("' + table.replace('"', '""') + '")')}:
            continue
        for (feature,) in db.execute('SELECT Feature FROM "' + table.replace('"', '""') + '" WHERE Feature IS NOT NULL'):
            if not isinstance(feature, str) or len(feature) > 512: continue
            try:
                atoms = feature_atoms(feature)
            except ValueError:
                continue
            for atom in atoms:
                if atom in uses:
                    uses[atom][table] = uses[atom].get(table, 0) + 1
    # A master switch gates every row of its table, whatever their own Feature.
    for key, table in MASTER_SWITCHES.items():
        if key in uses and table in tables:
            uses[key][table] = db.execute('SELECT count(*) FROM "' + table + '"').fetchone()[0]
    npcs = {}
    if xml_root is not None:
        defaults, conditions, npcs = npc_feature_uses(xml_root)
        for table, counts in (('NpcTalkInfo', defaults), ('NpcCondition', conditions)):
            for atom, n in counts.items():
                if atom in uses: uses[atom][table] = n
    listed = [k for k in sorted(keys) if uses[k]]
    details = event_details(db, listed, tables)
    out = []
    for k in listed:
        entry = dict(key=k, value=keys[k], uses=uses[k], detail=details[k])
        if npcs.get(k): entry['detail']['npcs'] = npcs[k][:DETAIL_ROWS]
        if collabs.collab(k):
            entry['collab'] = collabs.collab(k)
            if collabs.adapted(k): entry['adapted'] = collabs.adapted(k)
        out.append(entry)
    if len(out) > 1000 or any(len(e['uses']) > 64 for e in out): raise ValueError('Event bound')
    return out

# Rows per section of an event's description in the panel.
DETAIL_ROWS = 60

def event_details(db, keys, tables):
    """What each event switch brings, for the panel's description: the rows of the tables
    the server reads (or will read) whose Feature names the key, in provider order."""
    from export_reward_policy import feature_atoms
    def atoms(feature):
        if not isinstance(feature, str) or len(feature) > 512: return set()
        try:
            return feature_atoms(feature)
        except ValueError:
            return set()
    out = {k: {'shops': [], 'coupons': [], 'drops': [], 'quests': [], 'stories': [], 'titles': [], 'periodic': [], 'goals': []} for k in keys}
    def n(v):
        # A number column; SQLite may keep a numeric text.
        if isinstance(v, int): return v
        try:
            return int(str(v).strip())
        except (TypeError, ValueError):
            return None
    def add(feature, section, row, master=None):
        for k in (atoms(feature) | ({master} if master else set())):
            if k in out and len(out[k][section]) < DETAIL_ROWS: out[k][section].append(row)
    if 'ShopInfo' in tables:
        for r in db.execute('Select * From [ShopInfo]'):
            add(r['Feature'], 'shops', [r['ShopID'], r['ItemClass'], n(r['Count']), r['PriceItemClass'], n(r['PriceCount'])])
    if 'CouponShopItemInfo' in tables and 'CouponShopVersionInfo' in tables:
        versions = {r['ShopVersion']: (r['PriceItemClass'], r['Feature']) for r in db.execute('Select * From [CouponShopVersionInfo]')}
        for r in db.execute('Select * From [CouponShopItemInfo]'):
            price, version_feature = versions.get(r['ShopVersion'], (None, None))
            row = [n(r['ShopVersion']), r['RewardItemClass'], n(r['RewardItemCount']), price, n(r['PriceItemCount'])]
            for k in atoms(r['Feature']) | atoms(version_feature):
                if k in out and len(out[k]['coupons']) < DETAIL_ROWS: out[k]['coupons'].append(row)
    monsters = {}
    if 'MonsterInfo' in tables:
        for r in db.execute('Select Identifier, Class, Variation From [MonsterInfo]'):
            monsters.setdefault(r['Identifier'], ' '.join(x for x in (r['Class'], r['Variation']) if x))
    if 'EventDropItemInfo' in tables:
        for r in db.execute('Select * From [EventDropItemInfo]'):
            add(r['Feature'], 'drops', [monsters.get(r['MonsterID']) if r['MonsterID'] is not None else None, r['ItemClass'], n(r['Probability']), r['Condition']])
    if 'QuestInfo' in tables:
        for r in db.execute('Select ID, Feature From [QuestInfo]'):
            add(r['Feature'], 'quests', r['ID'])
    if 'StoryLineInfo' in tables:
        for r in db.execute('Select * From [StoryLineInfo]'):
            add(r['Feature'], 'stories', [r['StoryLineID'], r['Npc'], n(r['RewardExp']), n(r['RewardGold']), n(r['RewardAp']), r['RewardItem'], n(r['RewardItemCount']), n(r['RewardTitle'])])
    if 'TitleInfo' in tables:
        for r in db.execute('Select ID, Feature From [TitleInfo]'):
            add(r['Feature'], 'titles', n(r['ID']))
    if 'PeriodicEventInfo' in tables:
        for r in db.execute('Select * From [PeriodicEventInfo]'):
            add(r['Feature'], 'periodic', [r['RewardType'], r['RewardArg'], n(r['Amount']), n(r['Possibility'])], master='BoostEvent')
    if 'AllUserGoalInfo' in tables:
        for r in db.execute('Select * From [AllUserGoalInfo]'):
            add(r['Feature'], 'goals', [r['GoalName'], r['Goal'], n(r['GoalCount']), r['RewardItemClass'], n(r['RewardNum']), r['StartTime'], r['EndTime']], master='AllUserGoalEvent')
    return out

PET_FEATURES = ('Pet', 'Pet_food_V2', 'PetPurchaseLimit', 'PetMaxLevel', 'Pet_ExpBattle_Point',
                'NamingRuleMaxBytes')

def pet_features(db, manifest):
    """FeatureMatrix values of the pet system for the catalog's region (and its override),
    as FeatureMatrix.GetInteger/GetFloat read them."""
    return region_features(db, manifest, PET_FEATURES, 'Pet')

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
