"""Synthetic SQLite only; checks provenance, order, overrides and type refusal."""
import contextlib,pathlib,sqlite3,tempfile,unittest
from export_item_catalog import read_package,digest
class CatalogExportTests(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=pathlib.Path(self.tmp.name)
  self.db=self.root/'heroesContents.db3';self.config=self.root/'ServiceCore.dll.config'
  self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db)) as db:
   db.executescript('CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT,"STUDY-B" TEXT);CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT);CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT);CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT,ExtraData TEXT);')
   db.executemany('INSERT INTO FeatureMatrix VALUES (?,?,?)',[('GameCode','42','43'),('CurrentVer','3.09.00','9'),('OverrideSetting','',''),('StoryTabInInventory','','1'),('on','1.1',''),('BraceletMaxSlotCount','4','0')])
   db.executemany('INSERT INTO EquipItemInfo VALUES (?,?)',[('study','on'),('study','off'),('STUDY',None)])
   db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?)',[('study','on',1,5,None,''),('study','off',2,2,'off',None)])
   db.execute('INSERT INTO ItemClassExtraInfo VALUES (?,?,?)',('study','on','1=100'));db.commit()
 def test_projection_preserves_select_star_order_duplicates_null_empty_and_original_hashes(self):
  before=(digest(self.db),digest(self.config));p=read_package(self.db,self.config)
  self.assertEqual([r['feature'] for r in p['equips']],['on','off',None]);self.assertEqual([r['max_stack'] for r in p['classes']],[1,2])
  self.assertIsNone(p['classes'][0]['bind']);self.assertEqual(p['classes'][0]['trade_on_bind'],'');self.assertEqual(before,(digest(self.db),digest(self.config)))
 def test_override_keeps_source_current_version_and_selects_other_feature_values(self):
  with contextlib.closing(sqlite3.connect(self.db)) as db:db.execute('UPDATE FeatureMatrix SET "STUDY-A"=? WHERE Feature=?',('STUDY-B','OverrideSetting'));db.commit()
  p=read_package(self.db,self.config);features={f['key']:f['value'] for f in p['features']}
  self.assertEqual(features['CurrentVer'],'3.09.00');self.assertEqual(features['StoryTabInInventory'],'1');self.assertEqual(p['manifest']['override_region'],'STUDY-B')
 def test_source_byte_storage_range_is_not_clamped(self):
  with contextlib.closing(sqlite3.connect(self.db)) as db:db.execute('UPDATE ItemClassInfo SET Storage=256');db.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config)
 def test_unverified_feature_encoding_is_rejected_without_exporting_content(self):
  with contextlib.closing(sqlite3.connect(self.db)) as db:db.execute('UPDATE EquipItemInfo SET Feature=?',('ç',));db.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config)
