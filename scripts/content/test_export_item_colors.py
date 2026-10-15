import contextlib,pathlib,sqlite3,tempfile,unittest
from export_item_colors import read_package,digest
class ItemColorsExportTests(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);root=pathlib.Path(self.tmp.name)
  folder=root/'server'/'Bin';folder.mkdir(parents=True);self.materials=root/'server'/'Material';self.materials.mkdir()
  self.db=folder/'heroesContents.db3';self.config=folder/'ServiceCore.dll.config'
  self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting><setting name="heroesMaterialPath"><value>..\\Material</value></setting></configuration>',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:
   c.executescript('CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT,Material1 TEXT,Material2 TEXT,Material3 TEXT);CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT);CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);')
   c.executemany('INSERT INTO FeatureMatrix VALUES(?,?)',[('GameCode','42'),('CurrentVer','3.09.00'),('OverrideSetting',''),('StoryTabInInventory','')])
   c.executemany('INSERT INTO EquipItemInfo VALUES(?,?,?,?,?)',[('synthetic',None,'first',None,'missing'),('synthetic','disabled','empty','first',None),(None,'disabled',None,None,None)]);c.commit()
  (self.materials/'first.raw').write_bytes(bytes([1,2,3,255]));(self.materials/'empty.raw').write_bytes(b'')
 def test_provider_order_nulls_missing_empty_trailing_bytes_and_hashes_are_preserved(self):
  before=(digest(self.db),digest(self.config),digest(self.materials/'first.raw'));p=read_package(self.db,self.config)
  self.assertEqual(len(p['rows']),3);self.assertEqual([r['source_index']for r in p['rows']],[0,1,2]);self.assertEqual(p['rows'][1]['feature'],'disabled');self.assertIsNone(p['rows'][2]['item_utf16'])
  self.assertEqual([m['raw_hex']for m in p['materials']],['010203ff',None,''])
  self.assertEqual(before,(digest(self.db),digest(self.config),digest(self.materials/'first.raw')))
 def test_material_path_and_size_outside_verified_profile_are_refused(self):
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE EquipItemInfo SET Material1=?',('../escape',));c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config)
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE EquipItemInfo SET Material1=?',('first',));c.commit()
  (self.materials/'first.raw').write_bytes(bytes(196611))
  with self.assertRaises(ValueError):read_package(self.db,self.config)
 def test_missing_config_and_outside_root_are_refused(self):
  text=self.config.read_text();self.config.write_text(text.replace('..\\Material','../../../../outside'))
  with self.assertRaises(ValueError):read_package(self.db,self.config)
  self.config.write_text(text.replace('heroesMaterialPath','unused'))
  with self.assertRaises(ValueError):read_package(self.db,self.config)
