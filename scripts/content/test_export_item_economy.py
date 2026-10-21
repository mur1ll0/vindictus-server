import contextlib, sqlite3, unittest
import test_export_fiona_inventory as inventory_tests
from export_item_economy import read_package, digest

class ItemEconomyExportTests(unittest.TestCase):
    def setUp(self):
        self.helper = inventory_tests.FionaInventoryExportTests(); self.helper.setUp(); self.addCleanup(self.helper.doCleanups)
        self.db = self.helper.db; self.config = self.helper.config
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            for column in ('Price', 'SellPrice', 'RequiredLevel', 'ClassRestriction', 'UseInTown', 'Indestructible'):
                db.execute(f'ALTER TABLE ItemClassInfo ADD COLUMN {column} INTEGER DEFAULT 0')
            db.execute('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', ('synthetic', 'flag', 9, 2, None, None, 'TEST', 0, 60, 100, 25, 3, 1023, 1, 1))
            db.commit()
    def test_rows_keep_catalog_order_duplicates_and_hashes_are_readonly(self):
        before = [digest(p) for p in (self.db, self.config)]
        p = read_package(self.db, self.config)
        self.assertEqual(p['manifest']['class_rows'], 2)
        self.assertEqual([(r['unique'], r['expire_in'], r['price'], r['sell_price']) for r in p['classes']], [(2, None, 0, 0), (0, 60, 100, 25)])
        self.assertEqual((p['classes'][1]['required_level'], p['classes'][1]['class_restriction'], p['classes'][1]['use_in_town'], p['classes'][1]['indestructible'], p['format_version']), (3, 1023, 1, 1, 2))
        self.assertEqual(before, [digest(p) for p in (self.db, self.config)])
    def test_mistyped_values_are_refused(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("UPDATE ItemClassInfo SET Price='x'"); db.commit()
        with self.assertRaises(ValueError): read_package(self.db, self.config)

if __name__ == '__main__':
    unittest.main()
