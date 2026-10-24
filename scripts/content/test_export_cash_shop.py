import contextlib, json, pathlib, sqlite3, tempfile, unittest

from export_cash_shop import read_package

SCHEMA = (
    'CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);'
    'CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT,Category TEXT);'
    'CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);'
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
            db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?,?)', [('hair_a', None, 1, 2, None, None, 'HAIR'), ('potion', None, 99, 1, None, None, None)])
            db.commit()

    def test_categories_follow_the_catalog_rows(self):
        p = read_package(self.db, self.config, self.prices)
        self.assertEqual(p['categories'], ['HAIR', None])
        self.assertEqual((p['root_category'], p['default_price'], p['prices']), (105, 500, [{'category': 'HAIR', 'price': 800}]))

    def test_price_table_is_bounded(self):
        self.prices.write_text(json.dumps({'default': 0, 'categories': {}}), encoding='utf-8')
        with self.assertRaises(ValueError):
            read_package(self.db, self.config, self.prices)


if __name__ == '__main__':
    unittest.main()
