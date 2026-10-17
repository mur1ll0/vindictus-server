import contextlib,sqlite3,unittest
import test_export_fiona_inventory as inventory_tests
from export_fiona_appearance import read_package,digest,source_manifest
class FionaAppearanceExportTests(unittest.TestCase):
 def setUp(self):
  self.helper=inventory_tests.FionaInventoryExportTests();self.helper.setUp();self.addCleanup(self.helper.doCleanups)
  self.sources={k:v['sha256']for k,v in source_manifest().items()}
  self.db=self.helper.db;self.config=self.helper.config;self.impl=self.db.parent/'ItemImpl.cs'
  self.impl.write_text('public CustomizeCharacter.Result CustomizeCharacter(){switch(x){case 1: customizeItemRequestInfo = new CustomizeItemRequestInfo("synthetic", "ignored", 1, 2, 3, 0);}}',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:
   c.executescript('ALTER TABLE EquipItemInfo ADD COLUMN CostumeType INTEGER;CREATE TABLE CustomizeItemInfo(ItemClass TEXT,Feature TEXT,CashShopType TEXT,IsCash TEXT);CREATE TABLE CustomizeColorInfo(Category TEXT,Feature TEXT,CashShopType TEXT,IsCash TEXT,"Order" INTEGER,Color INTEGER);CREATE TABLE EquipPartInfo(ItemCategory TEXT,EquipPartNum INTEGER,CostumePartNum INTEGER,CostumeCategory TEXT,IsConsumable BIT);')
   c.execute('INSERT INTO CustomizeItemInfo VALUES(?,?,?,?)',('synthetic','flag',None,'False'))
   c.execute('INSERT INTO CustomizeColorInfo VALUES(?,?,?,?,?,?)',('skin',None,None,'True',7,10))
   c.execute('INSERT INTO EquipPartInfo VALUES(?,?,?,?,?)',('TEST',1,None,'Decoration',0));c.commit()
 def test_source_text_booleans_nullable_fields_and_hashes_are_preserved(self):
  before=[digest(x)for x in(self.db,self.config,self.impl)];p=read_package(self.db,self.config,self.impl,self.sources)
  self.assertFalse(p['custom'][0]['is_cash']);self.assertTrue(p['colors'][0]['is_cash']);self.assertIsNone(p['parts'][0]['costume_part']);self.assertTrue(p['parts'][0]['decoration']);self.assertEqual(p['inner_colors'],[1,2,3]);self.assertIn('flag',[r['key']for r in p['features']]);self.assertEqual(before,[digest(x)for x in(self.db,self.config,self.impl)])
 def test_unmeasured_boolean_profile_is_refused(self):
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute("UPDATE CustomizeItemInfo SET IsCash='true'");c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl,self.sources)
 def test_changed_class_branch_and_invalid_integer_are_refused(self):
  self.impl.write_text('CustomizeCharacter(){case 2:}',encoding='utf-8')
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl,self.sources)
  self.impl.write_text('CustomizeCharacter(){case 1: customizeItemRequestInfo = new CustomizeItemRequestInfo("synthetic", "ignored", 1, 2, 3, 0);}',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute("UPDATE CustomizeColorInfo SET Color='bad'");c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl,self.sources)

 def test_initial_gear_consumable_boolean_and_sources_are_required(self):
  p=read_package(self.db,self.config,self.impl,self.sources)
  self.assertFalse(p['parts'][0]['consumable']);self.assertEqual(p['gear_sources'],self.sources)
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute("UPDATE EquipPartInfo SET IsConsumable=1");c.commit()
  self.assertTrue(read_package(self.db,self.config,self.impl,self.sources)['parts'][0]['consumable'])
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute("UPDATE EquipPartInfo SET IsConsumable=2");c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl,self.sources)
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl,{})
