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
    'CREATE TABLE RandomItemV2(ItemClass nvarchar,KeyItemClass nvarchar,RandomItemID nvarchar,ShowPopUp bit,IsTitiCore bit,CoolTime INT,ExclusiveID nvarchar,Feature nvarchar);'
    'CREATE TABLE RandomItemV2Recipe(RandomItemID nvarchar,ItemClass nvarchar,Count INT,RecipeType tinyint,Probability float,RareItemID INT,Cond_Character nvarchar,Cond_Item nvarchar,Cond_Feature nvarchar,Color1 INT,Color2 INT,Color3 INT,CharacterBind bit,ExpireBySec INT,Announce TEXT,Message TEXT);'
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
            db.execute('INSERT INTO RandomItemV2 VALUES (?,?,?,?,?,?,?,?)', ('other_box', None, 'other', 'True', 'False', None, None, None))
            db.execute('INSERT INTO RandomItemV2Recipe VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', ('other', 'potion', 2, 0, 1.0, None, '1', None, 'STUDY-A', -1, -1, -1, 'True', None, None, 'm'))
            db.commit()

    def test_tables_follow_the_loader(self):
        p = read_package(self.db, self.config)
        self.assertEqual(p['levels'], [10])
        self.assertEqual(p['boxes'][0]['visual'], 1)
        # The '.33' text cell is the Double SQLite converts; the text ItemNumber row keeps 0.
        self.assertEqual(p['recipes'][0][5], struct.pack('>d', 0.33).hex())
        self.assertEqual((p['classes'][1]['number'], p['classes'][1]['rule']), (0, None))
        v2 = p['v2_boxes'][0]
        self.assertEqual((v2['item_utf16'], v2['show_pop_up'], v2['titicore']), ('other_box'.encode('utf-16-le').hex(), True, False))
        r = p['v2_recipes'][0]
        self.assertEqual((r[2], r[3], r[6], r[8], r[12]), (2, 0, '1'.encode('utf-16-le').hex(), 'STUDY-A', True))
        self.assertIn('STUDY-A', [f['key'] for f in p['features']])
        self.assertIn('enEU', [f['key'] for f in p['features']])

    def test_na_packages_fill_missing_boxes(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?,?)', [
                ('ap_box', None, 1, 1, None, None, 1), ('capsule_ap5000', None, 1, 1, None, None, 1),
                ('gacha_box', None, 1, 1, None, None, 1), ('featured_box', None, 1, 1, None, None, 1),
                ('potion', None, 99, 1, None, None, 1)])
            db.commit()
        na = pathlib.Path(self.tmp.name) / 'na.db3'
        with contextlib.closing(sqlite3.connect(na)) as db:
            db.executescript(
                'CREATE TABLE RandomItemV2_Client(ItemClass TEXT,RandomItemID TEXT,Feature TEXT);'
                'CREATE TABLE RandomItemV2Recipe_Client(RecipeClassName TEXT,ItemClass TEXT,Count INT,RecipeType INT,'
                'Color1 INT,Color2 INT,Color3 INT,Color4 INT,Color5 INT,Color6 INT,CharacterBind TEXT,ExpireByMin INT,'
                'Cond_Character TEXT,Feature TEXT);')
            db.executemany('INSERT INTO RandomItemV2_Client VALUES (?,?,?)', [
                ('ap_box', 'ap_box', None), ('gacha_box', 'gacha_box', None), ('featured_box', 'featured_box', 'X'),
                ('other_box', 'other_box', None), ('unknown_box', 'unknown_box', None)])
            row = lambda rid, cls, t: (rid, cls, 1, t, -1, -1, -1, -1, -1, -1, 'False', None, None, None)
            db.executemany('INSERT INTO RandomItemV2Recipe_Client VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)', [
                row('ap_box', 'capsule_ap5000', 0), row('gacha_box', 'potion', 1),
                row('featured_box', 'potion', 0), row('other_box', 'potion', 0), row('unknown_box', 'potion', 0)])
            db.commit()
        p = read_package(self.db, self.config, na)
        h = lambda s: s.encode('utf-16-le').hex()
        self.assertEqual([b['item_utf16'] for b in p['v2_boxes']], [h('other_box'), h('ap_box')])
        added = p['v2_boxes'][1]
        self.assertEqual((added['random_item_utf16'], added['show_pop_up'], added['feature']), (h('ap_box'), True, None))
        r = p['v2_recipes'][-1]
        self.assertEqual((r[0], r[1], r[2], r[3], r[4], r[12], r[15]),
                         (h('ap_box'), h('capsule_ap5000'), 1, 0, struct.pack('>d', 1.0).hex(), False,
                          h('GameUI_Heroes_RandomItem_GetItem')))
        self.assertEqual(len(p['v2_recipes']), 2)


if __name__ == '__main__':
    unittest.main()
