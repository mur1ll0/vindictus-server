import contextlib, pathlib, sqlite3, struct, tempfile, unittest

from export_random_item import read_package

SCHEMA = (
    'CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);'
    'CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT,[Level] INT);'
    'CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE RandomItem(ID INT,ServerCode INT,Description nchar,ItemClass nvarchar,KeyItemClass nvarchar,Visual INT,BaitProbability float,Feature nvarchar,UseFeverPoint INT,FeverPoint INT,RareItemCount INT,NormalItemCount INT);'
    'CREATE TABLE RandomItemRecipeClass(RandomItemID INT,ClassName nvarchar,ExpectedAppearanceCount INT,Probability float,ItemNumber INT,AdditionalRule varchar);'
    'CREATE TABLE RandomItemRecipe(RandomItemID INT,RecipeClassName nvarchar,CharacterType INT,ItemClass nvarchar,Number INT,Probability float,Color1 INT,Color2 INT,Color3 INT,NotifyMessage nvarchar,IsVariable INT,HaveItem float,DontHaveItem float,Feature nvarchar);'
    'CREATE TABLE RandomItemV2(ItemClass nvarchar,KeyItemClass nvarchar,Feature nvarchar);'
)


class RandomItemExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        self.db = root / 'heroesContents.db3'
        self.config = root / 'ServiceCore.dll.config'
        self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.executescript(SCHEMA)
            db.executemany('INSERT INTO FeatureMatrix VALUES (?,?)', [('GameCode', '42'), ('CurrentVer', '3.09.00'), ('OverrideSetting', ''), ('StoryTabInInventory', ''), ('RandomItemBundle', '1'), ('enEU', '1')])
            db.execute('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?,?)', ('gift_box', None, 1, 1, None, None, 10))
            db.execute('INSERT INTO RandomItem VALUES (?,?,?,?,?,?,?,?,?,?,?,?)', (11, None, None, 'gift_box', None, 1, None, None, 0, 0, -1, -1))
            db.executemany('INSERT INTO RandomItemRecipeClass VALUES (?,?,?,?,?,?)', [(11, 'gift_box', None, 1.0, 4, None), (12, 'broken', 1, '.0001', '', None)])
            db.execute('INSERT INTO RandomItemRecipe VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)', (11, 'gift_box', None, 'potion', 3, '.33', None, None, None, '0', 0, 0.0, 0.0, 'enEU'))
            db.execute('INSERT INTO RandomItemV2 VALUES (?,?,?)', ('other_box', None, None))
            db.commit()

    def test_tables_follow_the_loader(self):
        p = read_package(self.db, self.config)
        self.assertEqual(p['levels'], [10])
        self.assertEqual(p['boxes'][0]['visual'], 1)
        # The '.33' text cell is the Double SQLite converts; the text ItemNumber row keeps 0.
        self.assertEqual(p['recipes'][0][5], struct.pack('>d', 0.33).hex())
        self.assertEqual((p['classes'][1]['number'], p['classes'][1]['rule']), (0, None))
        self.assertEqual(p['v2_items'][0]['item_utf16'], 'other_box'.encode('utf-16-le').hex())
        self.assertIn('enEU', [f['key'] for f in p['features']])


if __name__ == '__main__':
    unittest.main()
