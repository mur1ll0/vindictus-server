import contextlib, pathlib, sqlite3, unittest
import test_export_fiona_inventory as inventory_tests
from export_npc_scripts import read_package, digest

COLUMNS = 'StoryLineID TEXT,StoryLineName TEXT,XmlPath TEXT,Npc TEXT,Category TEXT,Stage TEXT,RewardGold INT,RewardExp INT,RewardAp INT,RewardTitle INT,RewardItem TEXT,RewardItemCount INT,RewardSkill TEXT,RewardSkillRank INT,AutoGiveLevel INT,ClassRestriction INT,AutoProcess bit,AutoReveal bit,Season INT,Episode TEXT,Feature TEXT'

class NpcScriptExportTests(unittest.TestCase):
    def setUp(self):
        self.helper = inventory_tests.FionaInventoryExportTests(); self.helper.setUp(); self.addCleanup(self.helper.doCleanups)
        self.db = self.helper.db; self.config = self.helper.config
        self.xml = pathlib.Path(self.helper.tmp.name) / 'XML'; (self.xml / 'Default').mkdir(parents=True); (self.xml / 'QuestSet01').mkdir()
        (self.xml / 'NpcTalkInfo.xml').write_text('<?xml version="1.0"?>\n<FileList>\n  <Default>Default\\Default_A</Default>\n  <Default Feature="flag">Default\\Default_B</Default>\n</FileList>', encoding='utf-8-sig')
        (self.xml / 'Default' / 'Default_A.xml').write_text('<DefaultPhase>\n  <ButtonSet id="base_buttonset"><Button caption="bye" command="endtalk"/></ButtonSet>\n  <Talk id="FERGHUS" location="t01_bs" command="greeting">\n    <Do>\n      <talk>HELLO\\nTHERE</talk>\n      <change_npc>FERGHUS, gerens</change_npc>\n    </Do>\n  </Talk>\n</DefaultPhase>', encoding='utf-8')
        (self.xml / 'Default' / 'Default_B.xml').write_text('<DefaultPhase><Talk id="ARIEL" command="greeting"><Condition><a/>text</Condition></Talk></DefaultPhase>', encoding='utf-8')
        (self.xml / 'QuestSet01' / 'GuidePlay.xml').write_text('<StoryLine><Phase id="p0"><Goal>  </Goal></Phase></StoryLine>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute(f'CREATE TABLE StoryLineInfo({COLUMNS})')
            db.execute('CREATE TABLE ShopInfo(ShopID TEXT,"Order" INT,ItemClass TEXT,Count INT,PriceItemClass TEXT,PriceCount INT,AppearQuestID TEXT,CharacterBind bit,ID INT,RestrictionResetTime TEXT,RestrictionPeriodDay INT,RestrictionCount INT,Feature TEXT)')
            db.execute('INSERT INTO ShopInfo VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)', ('A_Shop', 1, 'potion', 2, 'gold', 95, None, 'False', None, None, 0, 0, 'shop_flag'))
            db.executescript('CREATE TABLE RepairEnhanceInfo(EnhanceLevel INT,Point INT,MaxDurabilityRecoveryPoint INT);CREATE TABLE RepairQualityInfo(Quality INT,Point INT,MaxDurabilityRecoveryPoint INT);CREATE TABLE RepairEnchantInfo(EnchantLevel INT,PrefixPoint INT,SuffixPoint INT,PrefixMaxDurabilityRecoveryPoint INT,SuffixMaxDurabilityRecoveryPoint INT);CREATE TABLE RepairEquipClassInfo(EquipClass TEXT,Point INT,MaxDurabilityRecoveryPoint INT);')
            db.execute('INSERT INTO RepairQualityInfo VALUES(2,5,0)')
            db.execute('ALTER TABLE EquipItemInfo ADD COLUMN EquipClass TEXT')
            db.executescript("CREATE TABLE QuestInfo(ID TEXT,AutoGiveLevel INT,QuestSet INT,ServiceCoin INT,MaxMemberCount INT,MaxPlayCount INT,EnableTodayQuest TEXT,Season3TodayGroup TEXT,AutoReveal bit,Feature TEXT,IsSeason2 bit,Level INT,TimeLimit INT,DropItemCountMin INT,DropItemCountMax INT,HuntingQuest bit,InitGameTime INT,SectorMoveGameTime INT,GatheringRockMax INT,RequiredStoryLineID TEXT,RequiredPhase TEXT,DisableUserShip bit,RequiredItem TEXT,MinClearTime INT,FreeQuest bit);"
                             "INSERT INTO QuestInfo VALUES('quest_a',1,1,0,4,NULL,'Enable',NULL,'False','EP3','False',2,30,3,6,'False',NULL,5,1,NULL,NULL,'False',NULL,3,'True');"
                             "CREATE TABLE QuestGoalInfo(ID INT,QuestID TEXT,Relation INT,Weight INT,Target TEXT,GameSetting TEXT,GameSettingArg INT,TargetCount INT,IsPositive INT,IsParty INT,Gold INT,XP INT,MaxAP INT,MinAP INT,StepAP INT,APRegenTime INT,APRegenMax INT,Item TEXT,ItemNum INT,ItemMultiplierFeature TEXT,ItemMultiplier INT);INSERT INTO QuestGoalInfo VALUES(10,'quest_a',0,100,'kill npc_gnoll boss',NULL,NULL,1,1,1,100,0,3,1,1,60,8,'',0,'goal_flag',1);"
                             "CREATE TABLE Sectors(QuestID TEXT,Sequence INT,SectorID INT,BSPName TEXT,SubSector TEXT,Weight INT,GoldDropTotal INT,GoldDropPerProp INT);INSERT INTO Sectors VALUES('quest_a',1,7,'01h',NULL,100,400,10);"
                             "CREATE TABLE SectorGroupInfo(QuestID TEXT,FromGroup INT,FromSector INT,FromTrigger TEXT,ToGroup INT,ToSpawnPoint TEXT,Feature TEXT);INSERT INTO SectorGroupInfo VALUES('quest_a',1,NULL,'exit',2,NULL,'group_flag');"
                             "CREATE TABLE TowerModuleSectorInfo(QuestID TEXT,SectorID INT);"
                             "CREATE TABLE Entity(ID INT,SectorID INT);INSERT INTO Entity VALUES(300,7),(301,8),(302,7);"
                             "CREATE TABLE EntityAttribute(EntityID INT,Name TEXT,Value TEXT);INSERT INTO EntityAttribute VALUES(302,'classname','npc_template_maker'),(300,'classname','trigger_once'),(302,'Feature','entity_flag'),(301,'classname','x');"
                             "CREATE TABLE NamedPropItemInfo(EntityName TEXT);INSERT INTO NamedPropItemInfo VALUES('barrel'),('barrel');"
                             "CREATE TABLE QuestSettingsInfo(RowID INT,ConditionID INT,Key TEXT,Value TEXT);INSERT INTO QuestSettingsInfo VALUES(1,NULL,'TutorStamp','Basic');"
                             "CREATE TABLE QuestSettingsConditionInfo(RowID INT,ConditionID INT,Condition TEXT,Value TEXT);"
                             "CREATE TABLE MonsterInfo(Identifier INT,Class TEXT,Variation TEXT,Difficulty INT,Exp INT,HP1 INT,HP_Normal_Percent INT);INSERT INTO MonsterInfo VALUES(7,'npc_gnoll','normal',-1,30,500,100);"
                             "CREATE TABLE MonsterDropItemInfo(MonsterID INT,QuestID TEXT,Difficulty INT,ItemClassEX TEXT,Weight INT,IsAbsolute bit,IsRequiredClass bit,IsUniqueDrop bit,Feature TEXT);INSERT INTO MonsterDropItemInfo VALUES(7,NULL,1,'synthetic[PREFIX:x]',5000,'False','True','False','drop_flag');"
                             "ALTER TABLE ItemClassInfo ADD COLUMN Rarity INTEGER DEFAULT 3;ALTER TABLE ItemClassInfo ADD COLUMN ClassRestriction INTEGER DEFAULT 2047;ALTER TABLE QuestInfo ADD COLUMN PropDropTableName TEXT;CREATE TABLE PropItemWeightInfo(ID INT,DropTableName TEXT,MaterialGroup TEXT,Type TEXT,Item TEXT,Weight INT,Feature TEXT);INSERT INTO PropItemWeightInfo VALUES(1,'ruin','WOOD','INSTANT','hp 65',700,NULL);CREATE TABLE PropItemExpectationInfo(ID INT,DropTableName TEXT,MaterialGroup TEXT,DropExpectationPerSector REAL);INSERT INTO PropItemExpectationInfo VALUES(1,'ruin','WOOD',9.7);CREATE TABLE MaterialGroupInfo(ID INT,MaterialGroup TEXT,Material TEXT);INSERT INTO MaterialGroupInfo VALUES(1,'WOOD','wood');CREATE TABLE MicroPlayEffectInfo(EffectSource TEXT,EffectOperation TEXT,Condition TEXT,EffectTarget TEXT,EffectTargetAmount INT,Effect1 TEXT,Effect2 TEXT,Effect3 TEXT,Effect4 TEXT,Effect5 TEXT,Feature TEXT);INSERT INTO MicroPlayEffectInfo VALUES('phoenix_feather','PlayerRevived',NULL,'TargetDead',1,'ReviveByClient 50',NULL,NULL,'GetStatusByClient 1 x',NULL,NULL);"
                             "CREATE TABLE QuestConstraintInfo(RowID INT,QuestID TEXT,RequiredQuestID TEXT,RequiredStatus INT);"
                             "CREATE TABLE QuestDifficultyInfo(QuestID TEXT,Difficulty INT,MinLevel INT,Feature TEXT,GameMode INT,EvilCoreCount INT,GoalExpRatio INT,GoalGoldRatio INT,MaxLevel INT);INSERT INTO QuestDifficultyInfo VALUES('quest_a',1,1,NULL,1,2,100,50,9);"
                             "CREATE TABLE CharacterClassInfo(Identifier INT,Level INT,BaseSTR INT,BaseDEX INT,BaseINT INT,BaseWILL INT,BaseHP INT,BaseSTAMINA INT,Feature TEXT);INSERT INTO CharacterClassInfo VALUES(1,1,27,44,14,23,1020,130,NULL),(0,1,1,1,1,1,1,1,NULL);"
                             "CREATE TABLE TitleInfo(ID INT,AutoGiveLevel INT,ClassRestriction INT,HideUntil INT,Feature TEXT,RequiredLevel INT,AllowShare bit);INSERT INTO TitleInfo VALUES(410017,1,2047,0,NULL,3,'False');"
                             "CREATE TABLE TitleStatInfo(ID INT,TitleID INT,Stat TEXT,Amount INT);INSERT INTO TitleStatInfo VALUES(1,410017,'STR',2),(2,410017,'HP',15);"
                             "CREATE TABLE TitleGoalInfo(ID INT,TitleID INT);INSERT INTO TitleGoalInfo VALUES(1,410017);"
                             "CREATE TABLE LevelUpExpInfo(Level INT,RequiredExp INT,Feature TEXT);INSERT INTO LevelUpExpInfo VALUES(1,350,NULL),(2,1148,'EP3');"
                             "CREATE TABLE QuestPeriodInfo(RowID INT,QuestID TEXT,DateFrom TEXT,Period INT,MaxPlayCount INT,Feature TEXT);INSERT INTO QuestPeriodInfo VALUES(1,'quest_a','2010-07-21 00:00:00.000',24,5,'enEU');")
            db.execute('INSERT INTO StoryLineInfo VALUES(' + ','.join('?' * 21) + ')', ('GuidePlay', 'n', 'QuestSet01', None, 'MainStream', None, 0, 0, 0, 0, None, '', None, 0, 1, 1023, 'False', 'True', 1, None, None))
            db.commit()
    def test_trees_keep_order_tails_and_source_integer_reads(self):
        before = [digest(p) for p in (self.db, self.config)]
        p = read_package(self.xml, self.db, self.config)
        self.assertEqual([(d['feature'], d['path']) for d in p['defaults']], [('', 'Default\\Default_A'), ('flag', 'Default\\Default_B')])
        talk = p['defaults'][0]['tree'][3][1]
        self.assertEqual((talk[0], talk[1]), ('Talk', [['id', 'FERGHUS'], ['location', 't01_bs'], ['command', 'greeting']]))
        self.assertEqual(talk[3][0][3][0][2], 'HELLO\\nTHERE')
        condition = p['defaults'][1]['tree'][3][0][3][0]
        self.assertEqual((condition[3][0][4], p['storylines'][0]['tree'][3][0][3][0][2]), ('text', ''))
        info = p['storylines'][0]['info']
        self.assertEqual((info['RewardItemCount'], info['AutoReveal'], info['AutoProcess'], p['manifest']['coerced_integers']), (0, True, False, 1))
        shop = p['shops'][0]
        self.assertEqual((shop['ShopID'], shop['Order'], shop['PriceCount'], shop['CharacterBind'], p['manifest']['shops']), ('A_Shop', 1, 95, False, 1))
        self.assertIn('shop_flag', [f['key'] for f in p['features']])
        self.assertEqual((p['repair']['quality'], len(p['repair']['equip_item_class'])), ([[2, 5]], 0))
        m = p['mission']
        self.assertEqual(m['quests'], [['quest_a', 2, 30, 3, 6, False, None, 5, 1, None, None, 4, False, False, None]])
        self.assertEqual(m['goals'], [['quest_a', 10, 0, 'kill npc_gnoll boss', None, None]])
        self.assertEqual(m['goal_infos'], [[10, 1, 1, 1, 100, 0, 3, 1, 1, 60, 8, '', 0, 'goal_flag', 1, 100]])
        self.assertEqual((m['difficulty_rewards'], m['quest_rewards']), ([['quest_a', 1, 100, 50, 9, None]], [['quest_a', 3, True]]))
        self.assertTrue({'goal_flag', 'ExpRate_Combat', 'MicroPlayFinishTime'} <= {f['key'] for f in p['features']})
        # Only entities of the quest sectors, in Entity order with attribute order.
        self.assertEqual(m['entities'], [[7, [['classname', 'trigger_once']]], [7, [['classname', 'npc_template_maker'], ['Feature', 'entity_flag']]]])
        self.assertEqual((m['named_props'], m['tower_quests'], m['settings']), (['barrel'], [], [[1, None, 'TutorStamp', 'Basic']]))
        self.assertEqual(m['monsters'], [[7, 'npc_gnoll', 'normal', -1, 30, 500, 100]])
        self.assertEqual(m['monster_drops'], [[7, None, 1, 'synthetic[PREFIX:x]', 5000, False, True, False, 'drop_flag']])
        self.assertEqual(m['effects'], [['phoenix_feather', 'PlayerRevived', 'TargetDead', 1, ['ReviveByClient 50', None, None, 'GetStatusByClient 1 x', None]]])
        self.assertEqual((m['sectors'][0][5:], m['prop_expectations'], m['material_groups']), ([400, 10], [['ruin', 'WOOD', 9]], [[1, 'WOOD']]))
        # The dropped class (attributes stripped) carries its rarity and class restriction.
        self.assertEqual([r[:3] for r in m['drop_classes']], [['synthetic', 3, 2047]])
        self.assertIn('drop_flag', [f['key'] for f in p['features']])
        keys = [f['key'] for f in p['features']]
        self.assertTrue({'group_flag', 'entity_flag', 'Gathering', 'GatheringRare'} <= set(keys))
        self.assertEqual(before, [digest(p) for p in (self.db, self.config)])
        g = p['progression']
        self.assertEqual((g['title_stats'], g['title_sharing']), ([[410017, 'STR', 2], [410017, 'HP', 15]], [[410017, 3, False]]))
        self.assertIn('AccountTitleStatRatio', keys)
    def test_missing_storyline_xml_is_refused(self):
        (self.xml / 'QuestSet01' / 'GuidePlay.xml').unlink()
        with self.assertRaises(OSError): read_package(self.xml, self.db, self.config)

if __name__ == '__main__':
    unittest.main()
