import contextlib, json, pathlib, sqlite3, tempfile, unittest

from export_cash_shop import read_package

SCHEMA = (
    'CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);'
    'CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT,Category TEXT,ClassRestriction INT);'
    'CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE CustomizeItemInfo(Category varchar,[Order] INT,ItemClass varchar,Weight INT,IsCash INT,IsNew INT,Description varchar,CashShopType varchar,Feature varchar);'
    'CREATE TABLE CustomizePriceInfo(ItemClass varchar,Duration INT,Price INT,CashShopType varchar,Feature varchar);'
    'CREATE TABLE CustomizeCouponInfo(ItemClass varchar,Category varchar,Price INT,CashShopType varchar,Feature varchar);'
    'CREATE TABLE CustomizeColorInfo(Category varchar,[Order] INT,Color INT,Weight INT,IsCash bit,Description varchar,CashShopType varchar,Feature varchar);'
    'CREATE TABLE Cashshop_Tircoin_notsell_list(CashShopType varchar,ItemClass varchar,Amount INT,Duration INT,Price INT,Feature varchar);'
)


class CashShopExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        self.db = root / 'heroesContents.db3'
        self.config = root / 'ServiceCore.dll.config'
        self.prices = root / 'prices.json'
        self.prices.write_text(json.dumps({'default': 500, 'categories': {'HAIR': 800}}), encoding='utf-8')
        self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.executescript(SCHEMA)
            db.executemany('INSERT INTO FeatureMatrix VALUES (?,?)', [('GameCode', '42'), ('CurrentVer', '3.09.00'), ('OverrideSetting', ''), ('StoryTabInInventory', '')])
            db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?,?,?)', [('hair_a', None, 1, 2, None, None, 'HAIR', 2), ('potion', None, 99, 1, None, None, None, 2047)])
            db.execute('INSERT INTO CustomizeItemInfo VALUES (?,?,?,?,?,?,?,?,?)', ('HAIR', 1, 'hair_a', 0, 1, 0, None, 'en-EU', 'STUDY-A'))
            db.executemany('INSERT INTO CustomizePriceInfo VALUES (?,?,?,?,?)', [('hair_a', -1, 139, 'en-EU', ''), ('hair_a', 30, 69, 'en-EU', '')])
            db.commit()

    def test_categories_follow_the_catalog_rows(self):
        p = read_package(self.db, self.config, self.prices)
        self.assertEqual(p['categories'], ['HAIR', None])
        self.assertEqual((p['root_category'], p['default_price'], p['prices']), (105, 500, [{'category': 'HAIR', 'price': 800}]))
        self.assertEqual([(r['duration'], r['price']) for r in p['customize_prices']], [(-1, 139), (30, 69)])
        self.assertEqual(p['customize_items'][0]['category'], 'HAIR')
        self.assertIn('STUDY-A', [f['key'] for f in p['features']])
        self.assertEqual(p['class_restrictions'], [2, 2047])

    def test_price_table_is_bounded(self):
        self.prices.write_text(json.dumps({'default': 0, 'categories': {}}), encoding='utf-8')
        with self.assertRaises(ValueError):
            read_package(self.db, self.config, self.prices)


if __name__ == '__main__':
    unittest.main()
