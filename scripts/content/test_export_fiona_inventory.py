import contextlib,pathlib,sqlite3,tempfile,unittest
from export_fiona_inventory import read_package,digest
class FionaInventoryExportTests(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);root=pathlib.Path(self.tmp.name)
  self.db=root/'heroesContents.db3';self.config=root/'ServiceCore.dll.config';self.constants=root/'ItemClassConst.cs'
  self.constants.write_text('public static readonly string Gold = "synthetic_gold";public static readonly string TirCoin = "synthetic_coin";',encoding='utf-8')
  self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="ServerCode"><value>0</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:
   c.executescript('CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT);CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT,Category TEXT,"Unique" INTEGER,ExpireIn INTEGER);CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);CREATE TABLE DefaultItemInfo(CharacterClass INTEGER,ItemClass TEXT,ItemNum INTEGER,Feature TEXT);CREATE TABLE VariableStatInfo(ItemClass TEXT);CREATE TABLE DormantReward(RewardItemClass TEXT,Num INTEGER,MinLevel INTEGER,MaxLevel INTEGER,Feature TEXT);')
   c.executemany('INSERT INTO FeatureMatrix VALUES(?,?)',[('GameCode','42'),('CurrentVer','3.09.00'),('OverrideSetting',''),('StoryTabInInventory','')])
   c.execute('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?,?,?,?,?)',('synthetic',None,1,1,None,None,'TEST',2,None))
   c.executemany('INSERT INTO DefaultItemInfo VALUES(?,?,?,?)',[(1,'synthetic',1,None),(255,'synthetic',2,'flag'),(2,'other',3,None)]);c.commit()
 def test_source_order_fiona_shared_and_nullable_metadata_are_preserved_readonly(self):
  before=[digest(x)for x in (self.db,self.config,self.constants)];p=read_package(self.db,self.config,self.constants)
  self.assertEqual([x['num']for x in p['requests']],[1,2]);self.assertEqual(p['classes'][0]['unique'],2);self.assertIsNone(p['classes'][0]['expire_in']);self.assertEqual(p['gold_utf16'],'synthetic_gold'.encode('utf-16-le').hex());self.assertEqual(before,[digest(x)for x in(self.db,self.config,self.constants)])
 def test_invalid_source_numeric_values_are_not_defaulted(self):
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE ItemClassInfo SET "Unique"=?',('invalid',));c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.constants)
 def test_missing_or_unverified_source_constants_are_refused(self):
  self.constants.write_text('public static readonly string Gold = "different-profile!";',encoding='utf-8')
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.constants)
