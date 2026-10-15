import contextlib,sqlite3,unittest
import test_export_fiona_inventory as inventory_tests
from export_fiona_appearance import read_package,digest
class FionaAppearanceExportTests(unittest.TestCase):
 def setUp(self):
  self.helper=inventory_tests.FionaInventoryExportTests();self.helper.setUp();self.addCleanup(self.helper.doCleanups)
  self.db=self.helper.db;self.config=self.helper.config;self.impl=self.db.parent/'ItemImpl.cs'
  self.impl.write_text('public CustomizeCharacter.Result CustomizeCharacter(){switch(x){case 1: customizeItemRequestInfo = new CustomizeItemRequestInfo("synthetic", "ignored", 1, 2, 3, 0);}}',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:
   c.executescript('ALTER TABLE EquipItemInfo ADD COLUMN CostumeType INTEGER;CREATE TABLE CustomizeItemInfo(ItemClass TEXT,Feature TEXT,CashShopType TEXT,IsCash TEXT);CREATE TABLE CustomizeColorInfo(Category TEXT,Feature TEXT,CashShopType TEXT,IsCash TEXT,"Order" INTEGER,Color INTEGER);CREATE TABLE EquipPartInfo(ItemCategory TEXT,EquipPartNum INTEGER,CostumePartNum INTEGER,CostumeCategory TEXT);')
   c.execute('INSERT INTO CustomizeItemInfo VALUES(?,?,?,?)',('synthetic','flag',None,'False'))
   c.execute('INSERT INTO CustomizeColorInfo VALUES(?,?,?,?,?,?)',('skin',None,None,'True',7,10))
   c.execute('INSERT INTO EquipPartInfo VALUES(?,?,?,?)',('TEST',1,None,'Decoration'));c.commit()
 def test_source_text_booleans_nullable_fields_and_hashes_are_preserved(self):
  before=[digest(x)for x in(self.db,self.config,self.impl)];p=read_package(self.db,self.config,self.impl)
  self.assertFalse(p['custom'][0]['is_cash']);self.assertTrue(p['colors'][0]['is_cash']);self.assertIsNone(p['parts'][0]['costume_part']);self.assertTrue(p['parts'][0]['decoration']);self.assertEqual(p['inner_colors'],[1,2,3]);self.assertIn('flag',[r['key']for r in p['features']]);self.assertEqual(before,[digest(x)for x in(self.db,self.config,self.impl)])
 def test_unmeasured_boolean_profile_is_refused(self):
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute("UPDATE CustomizeItemInfo SET IsCash='true'");c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl)
 def test_changed_class_branch_and_invalid_integer_are_refused(self):
  self.impl.write_text('CustomizeCharacter(){case 2:}',encoding='utf-8')
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl)
  self.impl.write_text('CustomizeCharacter(){case 1: customizeItemRequestInfo = new CustomizeItemRequestInfo("synthetic", "ignored", 1, 2, 3, 0);}',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute("UPDATE CustomizeColorInfo SET Color='bad'");c.commit()
  with self.assertRaises(ValueError):read_package(self.db,self.config,self.impl)
