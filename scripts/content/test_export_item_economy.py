import contextlib, sqlite3, unittest
import test_export_fiona_inventory as inventory_tests
from export_item_economy import read_package, digest

class ItemEconomyExportTests(unittest.TestCase):
    def setUp(self):
        self.helper = inventory_tests.FionaInventoryExportTests(); self.helper.setUp(); self.addCleanup(self.helper.doCleanups)
        self.db = self.helper.db; self.config = self.helper.config
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            for column in ('Price', 'SellPrice', 'RequiredLevel', 'ClassRestriction', 'UseInTown', 'Indestructible', 'LevelUpperBound', 'IsCafeOnly', 'RequiredSkillRank', 'RequiredEnhanceLevel', 'RequiredEnchantLevel'):
                db.execute(f'ALTER TABLE ItemClassInfo ADD COLUMN {column} INTEGER DEFAULT 0')
            db.execute('ALTER TABLE ItemClassInfo ADD COLUMN RequiredSkill TEXT')
            db.execute('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', ('synthetic', 'flag', 9, 2, None, None, 'TEST', 0, 60, 100, 25, 3, 1023, 1, 1, 40, 2, 5, 0, 0, 'skill_x'))
            db.execute('ALTER TABLE ItemClassInfo ADD COLUMN QuickSlotMaxStack INTEGER DEFAULT 0')
            db.execute('ALTER TABLE ItemClassInfo ADD COLUMN Draft INTEGER DEFAULT 0')
            db.execute("UPDATE ItemClassInfo SET QuickSlotMaxStack=10,Draft=1 WHERE ItemClass='synthetic'")
            db.execute('CREATE TABLE AlchemistBoxInfo(BoxItemClass TEXT,Attribute TEXT,Slot1 TEXT)')
            db.execute("INSERT INTO AlchemistBoxInfo VALUES('box_b','a','x'),('box_a',NULL,NULL)")
            for column, kind in (('SortOrder', 'TEXT'), ('Level', 'INTEGER DEFAULT 0'), ('InventoryFilterName', 'TEXT')):
                db.execute(f'ALTER TABLE ItemClassInfo ADD COLUMN {column} {kind}')
            db.execute("UPDATE ItemClassInfo SET SortOrder='100.02',Level=7,InventoryFilterName='GOODS' WHERE ItemClass='synthetic'")
            db.execute('CREATE TABLE SetInfo(SetID TEXT)')
            db.execute('CREATE TABLE SetItemInfo(ItemClass TEXT,BaseItemClass TEXT,SetID TEXT)')
            db.execute("INSERT INTO SetInfo VALUES('set_a')")
            db.execute("INSERT INTO SetItemInfo VALUES('synthetic',NULL,'set_a'),('synthetic',NULL,'set_b')")
            db.execute('CREATE TABLE HotSpringPotionInfo(ItemClass TEXT,StatusEffect TEXT,Level INT,UntilEffectSec INT,BathTimeSec INT,OverlapMinSec INT,AllAnnounce INT,ShowName INT)')
            db.execute("INSERT INTO HotSpringPotionInfo VALUES('hotspring_powder_rose','hotspring_powder_rose',1,30,1800,60,0,1)")
            columns = {r[1] for r in db.execute('PRAGMA table_info(ItemClassInfo)')}
            for column in ('TradeRestirction', 'Bind', 'TradeOnBind'):
                if column not in columns:
                    db.execute(f'ALTER TABLE ItemClassInfo ADD COLUMN {column} TEXT')
            db.execute("UPDATE ItemClassInfo SET TradeRestirction='ACCOUNTONLY',Bind='Disable',TradeOnBind='S3EP1' WHERE ItemClass='synthetic'")
            for column in ('TradeCategory', 'TradeCategorySub'):
                if column not in columns:
                    db.execute(f'ALTER TABLE ItemClassInfo ADD COLUMN {column} TEXT')
            db.execute("UPDATE ItemClassInfo SET TradeCategory='MATERIAL',TradeCategorySub='MATERIAL_ORE' WHERE ItemClass='synthetic'")
            db.execute('CREATE TABLE ItemTradeLimitInfo(ItemClass TEXT,MaxLimitValue INT)')
            db.execute("INSERT INTO ItemTradeLimitInfo VALUES('phoenix_feather',700),('empty_bottle',NULL)")
            db.execute('CREATE TABLE MailTypeInfo(MailType INT,Description TEXT,Sender TEXT,System TEXT,ItemAttached TEXT,Charged TEXT,Replyable TEXT,Returnable TEXT,Expire_in INT,Arrive_in INT,Fee INT,FeeForItem INT,Fee_Express INT,FeeForItem_Express INT)')
            db.execute("INSERT INTO MailTypeInfo VALUES(9,'Heroes_MailType_DungeonItem','SYSTEM','True','True','False','False','False',10080,0,0,0,0,0),(2,'Heroes_MailType_Item',NULL,'False','True','False','True','True',10080,60,100,2,200,5)")
            db.execute('CREATE TABLE GuildLevelUpInfo(Level INT,RequiredExp INT,LevelUpRewardItemClass TEXT,Feature TEXT)')
            db.execute("INSERT INTO GuildLevelUpInfo VALUES(2,130,'capsule_ap[VALUE:100]','GuildLevel'),(1,100,'capsule_ap[VALUE:100]','GuildLevel'),(1,9,NULL,'Disable')")
            db.execute('CREATE TABLE GuildLevelBonusInfo(Level INT,LevelBonus TEXT,Arg INT,Feature TEXT)')
            db.execute("INSERT INTO GuildLevelBonusInfo VALUES(1,'BattleExp',1,NULL)")
            db.execute('CREATE TABLE RankDescription(RankID TEXT,CategoryID INT,EventID INT,ScoreType TEXT,SortType TEXT,ScoreBasisType TEXT,ScoreResultType TEXT,Feature TEXT,ScoreLimit INT,IsGuild INT,InGameBannerPeriodType INT,InGameBannerPosition INT)')
            db.execute("INSERT INTO RankDescription VALUES('SoloTimeAttack_quest_a',2,20001,'Time','ASCENDING','Basis','ABSOLUTE','EP1',90000,0,0,0),('GamePlay_complete_battle',5,50001,'Count','DESCENDING','Basis','RELATIVE',NULL,NULL,0,1,1)")
            db.commit()
    def test_rows_keep_catalog_order_duplicates_and_hashes_are_readonly(self):
        before = [digest(p) for p in (self.db, self.config)]
        p = read_package(self.db, self.config)
        self.assertEqual(p['manifest']['class_rows'], 2)
        self.assertEqual([(r['unique'], r['expire_in'], r['price'], r['sell_price']) for r in p['classes']], [(2, None, 0, 0), (0, 60, 100, 25)])
        self.assertEqual((p['classes'][1]['required_level'], p['classes'][1]['class_restriction'], p['classes'][1]['use_in_town'], p['classes'][1]['indestructible'], p['format_version']), (3, 1023, 1, 1, 4))
        self.assertEqual((p['classes'][1]['quick_slot_max_stack'], p['classes'][1]['draft'], p['classes'][0]['quick_slot_max_stack']), (10, 1, 10))
        self.assertEqual(p['alchemist_boxes'], ['box_a'.encode('utf-16-le').hex(), 'box_b'.encode('utf-16-le').hex()])
        self.assertEqual((p['classes'][1]['level_upper_bound'], p['classes'][1]['cafe_only'], p['classes'][1]['required_skill_rank'], p['classes'][1]['required_skill_utf16']), (40, 2, 5, 'skill_x'.encode('utf-16-le').hex()))
        c = p['classes'][1]
        self.assertEqual((c['sort_order'], c['level'], c['inventory_filter'], c['set_id']), ('100.02', 7, 'GOODS', 'set_a'))
        self.assertEqual(p['hot_spring'], [dict(item_class='hotspring_powder_rose', status_effect='hotspring_powder_rose', level=1,
                                                bath_time_sec=1800, overlap_min_sec=60, all_announce=0)])
        self.assertEqual(c['trade_restriction'], 'ACCOUNTONLY')
        self.assertEqual([(m['mail_type'], m['sender'], m['system'], m['returnable'], m['arrive_in'], m['fee_for_item_express']) for m in p['mail_types']],
                         [(2, None, False, True, 60, 5), (9, 'SYSTEM', True, False, 0, 0)])
        # Guild rows keep their Feature for the server policy (repeated levels included).
        self.assertEqual([(g['level'], g['required_exp'], g['reward'], g['feature']) for g in p['guild_levels']],
                         [(1, 100, 'capsule_ap[VALUE:100]', 'GuildLevel'), (1, 9, None, 'Disable'), (2, 130, 'capsule_ap[VALUE:100]', 'GuildLevel')])
        self.assertEqual(p['guild_level_bonuses'], [dict(level=1, bonus='BattleExp', arg=1, feature=None)])
        self.assertEqual([(r['rank_id'], r['event_id'], r['score_type'], r['sort_type'], r['score_limit']) for r in p['rank_descriptions']],
                         [('SoloTimeAttack_quest_a', 20001, 'Time', 'ASCENDING', 90000), ('GamePlay_complete_battle', 50001, 'Count', 'DESCENDING', None)])
        self.assertEqual(before, [digest(p) for p in (self.db, self.config)])
    def test_marketplace_categories_limits_and_region_features(self):
        p = read_package(self.db, self.config)
        self.assertEqual((p['classes'][1]['trade_category'], p['classes'][1]['trade_category_sub']), ('MATERIAL', 'MATERIAL_ORE'))
        m = p['marketplace']
        self.assertEqual(m['trade_limits'], [dict(item_class='phoenix_feather', max_limit_value=700), dict(item_class='empty_bottle', max_limit_value=None)])
        self.assertEqual([f['key'] for f in m['features']][:2], ['CurrentVer', 'MoneyLimitToTradeItem'])
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("INSERT INTO ItemTradeLimitInfo VALUES('phoenix_feather',1)"); db.commit()
        with self.assertRaises(ValueError): read_package(self.db, self.config)
    def test_events_are_region_switches_used_by_content(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            columns = [r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')]
            for key in ('Event_xmas_2012', 'Event_xmas_kokr_2012', 'Event_Rate_2011', 'Event_unused', 'BoostEvent'):
                db.execute('INSERT INTO FeatureMatrix(Feature) VALUES(?)', (key,))
            db.execute('UPDATE FeatureMatrix SET ' + ','.join('"%s"=?' % c for c in columns[1:]) + " WHERE Feature='Event_Rate_2011'", ['100'] * (len(columns) - 1))
            db.execute('CREATE TABLE ShopInfo(ShopID TEXT,ItemClass TEXT,Count INT,PriceItemClass TEXT,PriceCount INT,Feature TEXT)')
            db.execute("INSERT INTO ShopInfo VALUES('a','xmas_hat',1,'gold',500,'Event_xmas_2012'),('b','xmas_box','2','coupon_xmas',3,'Event_xmas_2012&&!Event_xmas_kokr_2012'),('c','x',1,'gold',1,'Event_Rate_2011')")
            # A master switch gates its whole table (PeriodicEventScheduler's BoostEvent).
            db.execute('CREATE TABLE PeriodicEventInfo(Identifier INT,RewardType TEXT,RewardArg TEXT,Amount INT,Possibility INT,AnnounceLocalizedKey TEXT,Feature TEXT)')
            db.execute("INSERT INTO PeriodicEventInfo VALUES(1,'Gold',NULL,1000,49000,NULL,'disable')")
            db.execute('CREATE TABLE EventTemplate(Name TEXT,Feature TEXT,StartScript TEXT,EndScript TEXT)')
            db.execute("INSERT INTO EventTemplate VALUES('#exp2x','ExpRate_Combat[200];ExpRate_Monster[200]','monster_refresh;quest_refresh;','monster_refresh;')")
            db.commit()
        package = read_package(self.db, self.config)
        self.assertEqual(package['event_templates'], [dict(name='#exp2x', feature='ExpRate_Combat[200];ExpRate_Monster[200]',
                                                           start_script='monster_refresh;quest_refresh;', end_script='monster_refresh;')])
        events = package['events']
        empty = {'shops': [], 'coupons': [], 'drops': [], 'quests': [], 'stories': [], 'titles': [], 'periodic': [], 'goals': []}
        self.assertEqual(events, [dict(key='BoostEvent', value=None, uses={'PeriodicEventInfo': 1}, detail={**empty, 'periodic': [['Gold', None, 1000, 49000]]}),
                                  dict(key='Event_xmas_2012', value=None, uses={'ShopInfo': 2},
                                       detail={**empty, 'shops': [['a', 'xmas_hat', 1, 'gold', 500], ['b', 'xmas_box', 2, 'coupon_xmas', 3]]})])
    def test_mail_type_bits_must_be_text_booleans(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("UPDATE MailTypeInfo SET System='1' WHERE MailType=9"); db.commit()
        with self.assertRaises(ValueError): read_package(self.db, self.config)
    def test_pet_tables_are_exported_when_present(self):
        self.assertNotIn('pets', read_package(self.db, self.config))
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute('ALTER TABLE ItemClassInfo ADD COLUMN ExtraData TEXT')
            db.execute("UPDATE ItemClassInfo SET ExtraData='518' WHERE ItemClass='synthetic'")
            db.execute('CREATE TABLE PetInfo(PetID INT,DefaultLevel INT,StatBalanceType INT,MaxActiveTime INT,DeactiveTime INT,ItemClass TEXT,MaxHunger INT,HungerTime INT,HungerDecrease INT,Feature TEXT)')
            db.execute("INSERT INTO PetInfo VALUES(518,1,1,86400,0,'pet_create_x',200,36,1,'Pet')")
            db.execute('CREATE TABLE PetStatusBalanceInfo(StatType INT,Level INT,RequiredExp INT,MaxExp INT,HP INT,ResDamage INT,HPRecovery INT,DefBreak INT,AtkBalance INT,Atk INT,Def INT,Critical INT,ResCritical INT,Feature TEXT)')
            db.execute('INSERT INTO PetStatusBalanceInfo VALUES(1,1,88,88,624,25,2,70,80,740,152,2,10,NULL)')
            db.execute('CREATE TABLE PetSkillInfo(SkillID INT,CoolTimeType INT)')
            db.execute('INSERT INTO PetSkillInfo VALUES(1,0)')
            db.execute('CREATE TABLE DefaultPetSkillInfo(PetID INT,SlotOrder INT,PetSkillID INT,OpenLevel INT)')
            db.execute('INSERT INTO DefaultPetSkillInfo VALUES(518,1,1,1)')
            db.execute('CREATE TABLE PetActiveTimePeriodInfo(PetID INT,DateFrom TEXT,Period INT,Feature TEXT)')
            db.execute("INSERT INTO PetActiveTimePeriodInfo VALUES(518,'2012-02-13 17:00:00.000',24,'enUS')")
            db.execute('CREATE TABLE PetFeedInfo(FeedType INT,ItemClass TEXT)')
            db.execute("INSERT INTO PetFeedInfo VALUES(1,'pet_feed_make')")
            db.commit()
        pets = read_package(self.db, self.config)['pets']
        self.assertEqual(pets['infos'], [dict(pet_id=518, default_level=1, stat_type=1, max_active_time=86400, item_class='pet_create_x',
                                              max_hunger=200, hunger_time=36, hunger_decrease=1)])
        self.assertEqual((pets['balance'][0]['required_exp'], pets['balance'][0]['defense']), (88, 152))
        self.assertEqual(pets['periods'], [dict(pet_id=518, date_from_ticks=634647492000000000, period=24, feature='enUS')])
        self.assertEqual(pets['items'], [])
        self.assertEqual((pets['skills'], pets['feeds']), ([dict(skill_id=1, cool_time_type=0)], [dict(feed_type=1, item_class='pet_feed_make')]))
    def test_mistyped_values_are_refused(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("UPDATE ItemClassInfo SET Price='x'"); db.commit()
        with self.assertRaises(ValueError): read_package(self.db, self.config)

if __name__ == '__main__':
    unittest.main()
