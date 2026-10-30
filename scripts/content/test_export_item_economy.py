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
        self.assertEqual(before, [digest(p) for p in (self.db, self.config)])
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
