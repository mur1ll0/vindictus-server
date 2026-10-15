import contextlib,pathlib,sqlite3,struct,tempfile,unittest
from export_item_stats import STATS,read_package,digest
class ItemStatsExportTests(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=pathlib.Path(self.tmp.name)
  self.db=self.root/'heroesContents.db3';self.config=self.root/'ServiceCore.dll.config'
  self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:
   cols=','.join('"'+n+'" INTEGER NOT NULL DEFAULT 0'for n in STATS+['MaxDurability','ArmorHP'])
   c.executescript('CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT,Weight REAL,'+cols+');CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT);CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);')
   c.executemany('INSERT INTO FeatureMatrix VALUES (?,?)',[('GameCode','42'),('CurrentVer','3.09.00'),('OverrideSetting',''),('StoryTabInInventory','')])
   c.executemany('INSERT INTO EquipItemInfo(ItemClass,Feature,Weight,STR,MaxDurability,ArmorHP)VALUES(?,?,?,?,?,?)',[('synthetic',None,1.999,2147483647,17,23),('synthetic','disabled',-1.999,-2147483648,-1,0),(None,'disabled',0,0,0,0)]);c.commit()
 def test_all_stat_columns_bits_duplicate_order_and_source_hashes_are_preserved(self):
  before=(digest(self.db),digest(self.config));p=read_package(self.db,self.config)
  self.assertEqual([r['stats'][0]for r in p['rows']],[2147483647,-2147483648,0]);self.assertEqual([r['weight_bits']for r in p['rows']],[struct.pack('>d',x).hex()for x in [1.999,-1.999,0]])
  self.assertEqual(p['rows'][0]['max_durability'],17);self.assertEqual(p['rows'][0]['armor_hp'],23);self.assertIsNone(p['rows'][2]['item_utf16']);self.assertEqual(before,(digest(self.db),digest(self.config)))
 def test_noninteger_and_out_of_range_stats_are_refused_instead_of_clamped(self):
  for value in [1.5,2147483648,-2147483649,'bad']:
   with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE EquipItemInfo SET STR=?',(value,));c.commit()
   with self.assertRaises(ValueError):read_package(self.db,self.config)
 def test_weight_null_text_are_refused_and_infinity_bits_are_retained(self):
  for value in [None,'bad']:
   with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE EquipItemInfo SET Weight=?',(value,));c.commit()
   with self.assertRaises(ValueError):read_package(self.db,self.config)
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE EquipItemInfo SET Weight=?',(float('inf'),));c.commit()
  self.assertEqual(read_package(self.db,self.config)['rows'][0]['weight_bits'],'7ff0000000000000')
