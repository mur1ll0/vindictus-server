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
        self.assertEqual(before, [digest(p) for p in (self.db, self.config)])
    def test_mistyped_values_are_refused(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("UPDATE ItemClassInfo SET Price='x'"); db.commit()
        with self.assertRaises(ValueError): read_package(self.db, self.config)

if __name__ == '__main__':
    unittest.main()
