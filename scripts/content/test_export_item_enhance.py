import contextlib, pathlib, sqlite3, struct, tempfile, unittest

import content_overrides as overrides
from export_item_enhance import read_package, digest

SCHEMA = (
    'CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);'
    'CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT,EnhanceType TEXT);'
    'CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT);'
    'CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE EnhanceInfo(EnhanceType varchar,EnhanceLevel INT,MaterialClass1 varchar,MaterialNum1 INT,MaterialClass2 varchar,MaterialNum2 INT,Gold INT,SuccessRatio REAL,OnFail varchar,OnFailRecipeRatio REAL,OnFailMaterialNum1 INT,Effect INT,Feature varchar);'
    'CREATE TABLE EnhanceStatInfo(EnhanceType varchar,EnhanceLevel INT,Stat varchar,ValueType varchar,Value REAL,Base varchar);'
    'CREATE TABLE EnhanceServerInfo(EnhanceType nvarchar,EnhanceLevel INT,SuccessRatio REAL,Feature nvarchar);'
    'CREATE TABLE EnhanceBonusServerInfo(ItemClass varchar,Bonus float,EnhanceMin smallint,EnhanceMax smallint);'
    'CREATE TABLE EnhanceInsteadMaterialInfo(ItemClass varchar,UseCount INT,Slot tinyint,EnhanceMin smallint,EnhanceMax smallint);'
)


def fill(db, ratio):
    db.executescript(SCHEMA)
    db.executemany('INSERT INTO FeatureMatrix VALUES (?,?)', [('GameCode', '42'), ('CurrentVer', '3.09.00'), ('OverrideSetting', ''), ('StoryTabInInventory', ''), ('enEU', '1'), ('zhCN', '')])
    db.executemany('INSERT INTO EquipItemInfo VALUES (?,?,?)', [('sword', None, 'SWORD'), ('cloak', None, None)])
    db.executemany('INSERT INTO EnhanceInfo VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)', [
        ('SWORD', 1, 'stone', 2, None, 0, 500, ratio, 'NoPenalty', 0.0, 0, 1, 'enEU'),
        ('SWORD', 9, 'stone', 4, 'elixir', 1, 9000, ratio, 'Destroy', 0.5, 21, 3, 'enEU'),
    ])
    db.executemany('INSERT INTO EnhanceStatInfo VALUES (?,?,?,?,?,?)', [('SWORD', 1, 'ATK', 'Add', 50.0, 'ATK'), ('SWORD', 1, 'Weight', 'Multiply', -0.1, 'Weight')])
    db.executemany('INSERT INTO EnhanceServerInfo VALUES (?,?,?,?)', [('1', 1, ratio, '!zhCN'), ('9', 9, ratio, '!zhCN')])
    db.execute('INSERT INTO EnhanceBonusServerInfo VALUES (?,?,?,?)', ('gevne', 3.3, 1, 15))
    db.execute('INSERT INTO EnhanceInsteadMaterialInfo VALUES (?,?,?,?,?)', ('gevne', 1, 1, 1, 15))
    db.commit()


class ItemEnhanceExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        self.db = root / 'heroesContents.db3'
        self.original = root / 'original.db3'
        self.config = root / 'ServiceCore.dll.config'
        self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            fill(db, 1.0)
        # The original keeps text ratios ('.33') in its REAL column.
        with contextlib.closing(sqlite3.connect(self.original)) as db:
            fill(db, '.33')
        saved = (overrides.ORIGINAL_DATABASE, overrides.ORIGINAL_SHA256)
        overrides.ORIGINAL_DATABASE = self.original
        overrides.ORIGINAL_SHA256 = overrides._digest(self.original)
        overrides._cache.clear()

        def restore():
            overrides.ORIGINAL_DATABASE, overrides.ORIGINAL_SHA256 = saved
            overrides._cache.clear()
        self.addCleanup(restore)

    def test_original_ratios_become_the_single_the_server_reads(self):
        before = (digest(self.db), digest(self.config))
        p = read_package(self.db, self.config)
        single = lambda v: struct.pack('>f', v).hex()
        self.assertEqual([r['success_ratio'] for r in p['enhance']], [single(0.33)] * 2)
        self.assertEqual([r['success_ratio'] for r in p['server']], [single(0.33)] * 2)
        self.assertEqual(p['enhance'][1]['on_fail_recipe_ratio'], single(0.5))
        self.assertEqual(p['stats'][1]['value'], single(-0.1))
        self.assertEqual([e['enhance_type'] for e in p['equips']], ['SWORD'.encode('utf-16-le').hex(), None])
        self.assertEqual(p['bonus'][0]['bonus'], struct.pack('>d', 3.3).hex())
        self.assertEqual(sorted(f['key'] for f in p['features']), ['enEU', 'zhCN'])
        self.assertEqual(before, (digest(self.db), digest(self.config)))

    def test_a_null_single_is_refused(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute('UPDATE EnhanceStatInfo SET Value=NULL')
            db.commit()
        with self.assertRaises(ValueError):
            read_package(self.db, self.config)


if __name__ == '__main__':
    unittest.main()
