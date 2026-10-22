"""Readonly NPC script projection: NpcTalkInfo.xml defaults, every StoryLineInfo XML and
its row, as compact element trees with XElement.Load(LoadOptions.None) text semantics."""
import argparse, contextlib, hashlib, json, pathlib, re, sqlite3, xml.etree.ElementTree as ET
import xml.etree.ElementTree
from export_item_catalog import read_package as catalog_read, digest
from export_reward_policy import feature_atoms

XML_SPACE = ' \t\r\n'

def tree(element):
    """[name, [[attr, value], ...], text, [children], tail]. XElement.Load(None) drops
    whitespace-only text nodes; XElement.Value is text + each child's Value and tail."""
    if not isinstance(element.tag, str) or '}' in element.tag:
        raise ValueError('Unsupported XML node or namespace')
    keep = lambda t: '' if not t or not t.strip(XML_SPACE) else t
    return [element.tag, [[k, v] for k, v in element.attrib.items()], keep(element.text),
            [tree(c) for c in element], keep(element.tail)]

def region_features(db, config):
    """FeatureMatrix column of the configured GameCode plus OverrideSetting, as the
    item catalog projection resolves it."""
    settings = {e.attrib['name']: e.findtext('value') for e in ET.parse(config).getroot().iter('setting') if 'name' in e.attrib}
    game_code = int(settings['GameCode'])
    columns = [r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')]; game_map = {}
    for row in db.execute("SELECT * FROM FeatureMatrix WHERE Feature='GameCode'"):
        for name, value in zip(columns, row):
            if isinstance(value, str) and re.fullmatch(r'[+-]?[0-9]+', value.strip()): game_map[int(value)] = name
    region = game_map.get(game_code, 'ko-KR').upper()
    def column(tag):
        if tag not in columns: raise ValueError('Missing configured feature column')
        return {k: ('' if v is None else v) for k, v in db.execute('SELECT Feature,"' + tag.replace('"', '""') + '" FROM FeatureMatrix')}
    features = column(region); override = features.get('OverrideSetting', '')
    if override:
        for k, v in column(override.upper()).items():
            if k not in ('GameCode', 'CurrentVer', 'OverrideSetting'): features[k] = v
    return features

def condition_atoms(node, out):
    """<feature> conditions evaluate FeatureMatrix.IsEnable(arg1)."""
    if node[0] == 'feature':
        value = node[2] if node[2] is not None else ''
        out |= feature_atoms(value.split(',')[0].strip())
    for c in node[3]: condition_atoms(c, out)

def load(path, hasher):
    data = path.read_bytes()
    if len(data) > 4 * 1024 * 1024: raise ValueError('Script file bound')
    hasher.update(len(data).to_bytes(8, 'little')); hasher.update(data)
    return tree(ET.fromstring(data))

def read_package(base, database, config):
    catalog = catalog_read(database, config)
    hasher = hashlib.sha256()
    index = base / 'NpcTalkInfo.xml'
    root = load(index, hasher)
    if root[0] != 'FileList': raise ValueError('NpcTalkInfo root')
    defaults = []
    for element in root[3]:
        if element[0] != 'Default': continue
        feature = ''
        for k, v in element[1]:
            if k == 'Feature': feature = v
        relative = pathlib.PureWindowsPath(element[2])
        path = base.joinpath(*relative.parts).with_suffix('.xml')
        if not path.resolve().is_relative_to(base.resolve()): raise ValueError('Default script outside XML root')
        defaults.append({'feature': feature, 'path': str(relative), 'tree': load(path, hasher)})
    coerced = 0
    def integer(v):
        # Declared INT: SQLite3.GetValue converts sqlite3_column_int64 to Int32, so a
        # TEXT value reads as SQLite's integer conversion of it ('' is 0).
        nonlocal coerced
        if isinstance(v, str):
            coerced += 1
            v = db.execute('SELECT CAST(? AS INTEGER)', (v,)).fetchone()[0]
        if type(v) is not int or not -(2**31) <= v < 2**31: raise ValueError('StoryLineInfo Int32')
        return v
    def boolean(v):
        if v in ('True', 'False'): return v == 'True'
        if v in (0, 1): return bool(v)
        raise ValueError('StoryLineInfo bool')
    def text(v):
        if v is not None and not isinstance(v, str): raise ValueError('StoryLineInfo text')
        return v
    storylines = []
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        for r in db.execute('Select * From [StoryLineInfo]'):
            info = {k: text(r[k]) for k in ('StoryLineID', 'StoryLineName', 'XmlPath', 'Npc', 'Category', 'Stage', 'RewardItem', 'RewardSkill', 'Episode', 'Feature')}
            info.update({k: integer(r[k]) for k in ('RewardGold', 'RewardExp', 'RewardAp', 'RewardTitle', 'RewardItemCount', 'RewardSkillRank', 'AutoGiveLevel', 'ClassRestriction', 'Season')})
            info.update({k: boolean(r[k]) for k in ('AutoProcess', 'AutoReveal')})
            entry = {'info': info, 'tree': None}
            if info['XmlPath'] is not None:
                folder = base.joinpath(*pathlib.PureWindowsPath(info['XmlPath']).parts)
                path = folder / (info['StoryLineID'] + '.xml')
                if not path.resolve().is_relative_to(base.resolve()): raise ValueError('Storyline outside XML root')
                entry['tree'] = load(path, hasher)
            storylines.append(entry)
    def short(v):
        if type(v) is not int or not -(2**15) <= v < 2**15: raise ValueError('ShopInfo Int16')
        return v
    def nullable_text(v):
        if v is not None and not isinstance(v, str): raise ValueError('ShopInfo text')
        return v
    shops = []
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        coerced_shop = 0
        for r in db.execute('Select * From [ShopInfo]'):
            row = {'ShopID': nullable_text(r['ShopID']), 'Order': short(r['Order']), 'ItemClass': nullable_text(r['ItemClass']),
                   'PriceItemClass': nullable_text(r['PriceItemClass']), 'AppearQuestID': nullable_text(r['AppearQuestID']),
                   'CharacterBind': boolean(r['CharacterBind']), 'ID': r['ID'], 'RestrictionResetTime': nullable_text(r['RestrictionResetTime']),
                   'Feature': nullable_text(r['Feature'])}
            for k in ('Count', 'PriceCount', 'RestrictionPeriodDay', 'RestrictionCount'):
                v = r[k]
                if isinstance(v, str):
                    coerced_shop += 1; v = db.execute('SELECT CAST(? AS INTEGER)', (v,)).fetchone()[0]
                if type(v) is not int or not -(2**31) <= v < 2**31: raise ValueError('ShopInfo Int32')
                row[k] = v
            if row['ID'] is not None and (type(row['ID']) is not int or not -(2**31) <= row['ID'] < 2**31): raise ValueError('ShopInfo ID')
            shops.append(row)
            if len(shops) > 100000: raise ValueError('ShopInfo bound')
    def int32(v):
        if type(v) is not int or not -(2**31) <= v < 2**31: raise ValueError('Repair Int32')
        return v
    repair = {}
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        repair['enhance'] = [[int32(r['EnhanceLevel']), int32(r['Point'])] for r in db.execute('Select * From [RepairEnhanceInfo]')]
        repair['quality'] = [[int32(r['Quality']), int32(r['Point'])] for r in db.execute('Select * From [RepairQualityInfo]')]
        repair['enchant'] = [[int32(r['EnchantLevel']), int32(r['PrefixPoint']), int32(r['SuffixPoint'])] for r in db.execute('Select * From [RepairEnchantInfo]')]
        repair['equip_class'] = [[nullable_text(r['EquipClass']), int32(r['Point'])] for r in db.execute('Select * From [RepairEquipClassInfo]')]
        # EquipItemInfo in provider order, aligned with the catalog projection's equips.
        repair['equip_item_class'] = [nullable_text(r['EquipClass']) for r in db.execute('Select * From [EquipItemInfo]')]
    if len(repair['equip_item_class']) != len(catalog['equips']): raise ValueError('EquipItemInfo differs from the catalog projection')
    def flag(v):
        if v in ('True', 'False'): return v == 'True'
        if v in (0, 1): return bool(v)
        raise ValueError('Quest bit')
    def nullable_int32(v):
        return None if v is None else int32(v)
    # QuestContents.Load tables in provider order (Select * From [Table]).
    quests = {}
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        quests['quests'] = [{'ID': nullable_text(r['ID']), 'AutoGiveLevel': int32(r['AutoGiveLevel']), 'QuestSet': int32(r['QuestSet']),
                             'ServiceCoin': int32(r['ServiceCoin']), 'MaxMemberCount': int32(r['MaxMemberCount']),
                             'MaxPlayCount': nullable_int32(r['MaxPlayCount']), 'EnableTodayQuest': nullable_text(r['EnableTodayQuest']),
                             'Season3TodayGroup': nullable_text(r['Season3TodayGroup']), 'AutoReveal': flag(r['AutoReveal']),
                             'Feature': nullable_text(r['Feature']), 'IsSeason2': flag(r['IsSeason2'])}
                            for r in db.execute('Select * From [QuestInfo]')]
        quests['goals'] = [[int32(r['ID']), nullable_text(r['QuestID']), int32(r['Relation']), int32(r['Weight'])]
                           for r in db.execute('Select * From [QuestGoalInfo]')]
        quests['constraints'] = [[nullable_text(r['QuestID']), nullable_text(r['RequiredQuestID']), int32(r['RequiredStatus'])]
                                 for r in db.execute('Select * From [QuestConstraintInfo]')]
        quests['difficulties'] = [[nullable_text(r['QuestID']), int32(r['Difficulty']), int32(r['MinLevel']), nullable_text(r['Feature'])]
                                  for r in db.execute('Select * From [QuestDifficultyInfo]')]
        quests['periods'] = [[nullable_text(r['QuestID']), nullable_text(r['DateFrom']), int32(r['Period']), nullable_int32(r['MaxPlayCount']), nullable_text(r['Feature'])]
                             for r in db.execute('Select * From [QuestPeriodInfo]')]
    # CharacterContents.Load: CharacterClassInfo (BaseCharacter 1 = Fiona) and LevelUpExpInfo,
    # both filtered by Feature at load; provider order.
    progression = {}
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        progression['class_stats'] = [[int32(r['Level']), int32(r['BaseSTR']), int32(r['BaseDEX']), int32(r['BaseINT']), int32(r['BaseWILL']), int32(r['BaseHP']), int32(r['BaseSTAMINA']), nullable_text(r['Feature'])]
                                      for r in db.execute('Select * From [CharacterClassInfo]') if r['Identifier'] == 1]
        progression['level_exp'] = [[int32(r['Level']), int32(r['RequiredExp']), nullable_text(r['Feature'])] for r in db.execute('Select * From [LevelUpExpInfo]')]
        # TitleInfo (ID, AutoGiveLevel, ClassRestriction, HideUntil, Feature) and the
        # TitleGoalInfo IDs per title, for TitleManager.GiveTitleByLevelUp.
        progression['titles'] = [[int32(r['ID']), None if r['AutoGiveLevel'] is None else int32(r['AutoGiveLevel']), int32(r['ClassRestriction']), int32(r['HideUntil']), nullable_text(r['Feature'])]
                                 for r in db.execute('Select * From [TitleInfo]')]
        progression['title_goals'] = [[int32(r['ID']), int32(r['TitleID'])] for r in db.execute('Select * From [TitleGoalInfo]')]
    # MicroPlayContents/QuestContents tables for mission entry: QuestInfo digest fields,
    # QuestGoalInfo targets and game settings, Sectors, SectorGroupInfo, the tower quests
    # (refused), Entity/EntityAttribute of the quest sectors (provider order),
    # NamedPropItemInfo entity names and QuestSettingsInfo/QuestSettingsConditionInfo.
    mission = {}
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        mission['quests'] = [[nullable_text(r['ID']), int32(r['Level']), int32(r['TimeLimit']), int32(r['DropItemCountMin']), int32(r['DropItemCountMax']),
                              flag(r['HuntingQuest']), nullable_int32(r['InitGameTime']), nullable_int32(r['SectorMoveGameTime']), int32(r['GatheringRockMax']),
                              nullable_text(r['RequiredStoryLineID']), nullable_text(r['RequiredPhase']), int32(r['MaxMemberCount']), flag(r['IsSeason2']),
                              flag(r['DisableUserShip']), nullable_text(r['RequiredItem'])]
                             for r in db.execute('Select * From [QuestInfo]')]
        mission['goals'] = [[nullable_text(r['QuestID']), int32(r['ID']), int32(r['Relation']), nullable_text(r['Target']), nullable_text(r['GameSetting']), nullable_int32(r['GameSettingArg'])]
                            for r in db.execute('Select * From [QuestGoalInfo]')]
        mission['sectors'] = [[nullable_text(r['QuestID']), int32(r['Sequence']), int32(r['SectorID']), nullable_text(r['BSPName']), int32(r['Weight'])]
                              for r in db.execute('Select * From [Sectors]')]
        mission['sector_groups'] = [[nullable_text(r['QuestID']), int32(r['FromGroup']), nullable_int32(r['FromSector']), nullable_text(r['FromTrigger']), int32(r['ToGroup']),
                                     nullable_text(r['ToSpawnPoint']), nullable_text(r['Feature'])]
                                    for r in db.execute('Select * From [SectorGroupInfo]')]
        mission['tower_quests'] = sorted({nullable_text(r['QuestID']) or '' for r in db.execute('Select * From [TowerModuleSectorInfo]')})
        wanted_sectors = {r[2] for r in mission['sectors']}
        ids = {}; order = []
        for r in db.execute('Select * From [Entity]'):
            sector = int32(r['SectorID'])
            if sector in wanted_sectors:
                ids[int32(r['ID'])] = sector; order.append(int32(r['ID']))
        attributes = {}
        for r in db.execute('Select * From [EntityAttribute]'):
            entity = int32(r['EntityID'])
            if entity in ids: attributes.setdefault(entity, []).append([nullable_text(r['Name']), nullable_text(r['Value'])])
        # Entity.ID is the HashSet member of its sector: a repeated ID keeps its first position.
        seen = set(); mission['entities'] = []
        for e in order:
            if (ids[e], e) in seen: continue
            seen.add((ids[e], e))
            if e in attributes: mission['entities'].append([ids[e], attributes[e]])
        mission['named_props'] = sorted({nullable_text(r['EntityName']) or '' for r in db.execute('Select * From [NamedPropItemInfo]')})
        mission['settings'] = [[int32(r['RowID']), nullable_int32(r['ConditionID']), nullable_text(r['Key']), nullable_text(r['Value'])]
                               for r in db.execute('Select * From [QuestSettingsInfo]')]
        mission['setting_conditions'] = [[int32(r['ConditionID']), nullable_text(r['Condition']), nullable_text(r['Value'])]
                                         for r in db.execute('Select * From [QuestSettingsConditionInfo]')]
        # MicroPlayContents.Monsters: MonsterInfo by "Class Variation" and Difficulty.
        mission['monsters'] = [[int32(r['Identifier']), nullable_text(r['Class']), nullable_text(r['Variation']), int32(r['Difficulty']), int32(r['Exp']), int32(r['HP1']), int32(r['HP_Normal_Percent'])]
                               for r in db.execute('Select * From [MonsterInfo]')]
    if len(mission['entities']) > 100000 or sum(len(e[1]) for e in mission['entities']) > 1000000: raise ValueError('Mission entity bound')
    if any(len(v) > 100000 for v in mission.values()): raise ValueError('Mission table bound')
    if any(len(v) > 10000 for v in progression.values()): raise ValueError('Progression table bound')
    if any(len(v) > 100000 for v in quests.values()): raise ValueError('Quest table bound')
    if any(q['ID'] is None for q in quests['quests']): raise ValueError('QuestInfo ID')
    wanted = {'CurrentVer', 'NewStoryLineDialog', 'FlexibleMapLoading', 'ShopPriceRatio', 'Buyback', 'BuybackPriceRatio', 'BuybackListCount', 'RestoreItem', 'RepairPriceRate',
              'GoldRate_Story', 'ExpRate_Story', 'APRate_Story', 'TodayQuest', 'TodayQuest_S2', 'TodayQuest_S3', 'TodayQuest_InitTimeAdder',
              'CafeBonusQuestPlus', 'Event_PlayCount_Unlimited', 'AddDeparture', 'AddDepartureCount', 'Event_PlayCount_Plus',
              'ChangeBasicBattleGoal', 'E3_Event_2010', 'Event_PAX_2011_enUS', 'DisableInGameCheat', 'zhCN', 'zhTW',
              'MaxLevel', 'CharacterRank', 'friend_loyalty_system', 'GuildScore'}
    for r in progression['class_stats']: wanted |= feature_atoms(r[7] or '')
    for r in progression['level_exp']: wanted |= feature_atoms(r[2] or '')
    for r in progression['titles']: wanted |= feature_atoms(r[4] or '')
    for q in quests['quests']:
        wanted |= feature_atoms(q['Feature'] or ''); wanted |= feature_atoms(q['EnableTodayQuest'] or '')
    for d in quests['difficulties']: wanted |= feature_atoms(d[3] or '')
    wanted |= {'Gathering', 'GatheringRare', 'AntiSectorSkip', 'bossevilcore_bonus', 'ExpRate_Monster', 'MDRSystem_V1', 'MDRSystem_V2',
               'MDRSystem_ActionFilter', 'DirectCollectCoin', 'AntiSectorFail', 'FatigueSystem'}
    for g in mission['sector_groups']: wanted |= feature_atoms(g[6] or '')
    for _, attrs in mission['entities']:
        for name, value in attrs:
            if (name or '').lower() == 'feature': wanted |= feature_atoms(value or '')
    for d in quests['periods']: wanted |= feature_atoms(d[4] or '')
    for row in shops: wanted |= feature_atoms(row['Feature'] or '')
    for d in defaults:
        wanted |= feature_atoms(d['feature']); condition_atoms(d['tree'], wanted)
    for s in storylines:
        wanted |= feature_atoms(s['info']['Feature'] or '')
        if s['tree'] is not None: condition_atoms(s['tree'], wanted)
    if len(wanted) > 1024 or any(not k.isascii() or not k or len(k) > 256 for k in wanted): raise ValueError('Feature subset bound')
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
        values = region_features(db, config)
    features = [{'key': k, 'value': values.get(k)} for k in sorted(wanted)]
    if any(f['value'] is not None and (not isinstance(f['value'], str) or len(f['value']) > 256) for f in features): raise ValueError('Feature value bound')
    if (digest(database), digest(config)) != (catalog['manifest']['database_sha256'], catalog['manifest']['config_sha256']):
        raise ValueError('Source changed during export')
    return {'format_version': 1, 'source': 'heroes.npc-scripts',
            'manifest': {'database_sha256': catalog['manifest']['database_sha256'], 'config_sha256': catalog['manifest']['config_sha256'],
                         'scripts_sha256': hasher.hexdigest(), 'defaults': len(defaults), 'storylines': len(storylines),
                         'coerced_integers': coerced, 'shops': len(shops), 'coerced_shop_integers': coerced_shop},
            'features': features, 'defaults': defaults, 'storylines': storylines, 'shops': shops, 'repair': repair, 'quests': quests, 'progression': progression, 'mission': mission}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--output', required=True); args = parser.parse_args()
    project = pathlib.Path(__file__).resolve().parents[2]; server = project.parent / 'ServidorVindictus/Server/zh-tw-x-gm'
    output = pathlib.Path(args.output).resolve(); allowed = (project / '.cache/private-content').resolve()
    if not output.is_relative_to(allowed): raise ValueError('Private output path outside ACL cache')
    package = read_package(server / 'Data/XML', server / 'Bin/heroesContents.db3', server / 'Bin/ServiceCore.dll.config')
    data = json.dumps(package, ensure_ascii=True, separators=(',', ':'))
    if len(data.encode('utf-8')) > 64 * 1024 * 1024: raise ValueError('Private export size bound')
    output.write_text(data, encoding='utf-8', newline='\n')
    print(json.dumps({'defaults': package['manifest']['defaults'], 'storylines': package['manifest']['storylines'], 'bytes': len(data)}))
