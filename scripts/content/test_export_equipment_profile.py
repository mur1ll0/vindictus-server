import contextlib, sqlite3, unittest
import test_export_fiona_inventory as inventory_tests
from export_equipment_profile import read_package, digest

class EquipmentProfileExportTests(unittest.TestCase):
    def setUp(self):
        self.helper=inventory_tests.FionaInventoryExportTests();self.helper.setUp();self.addCleanup(self.helper.doCleanups)
        self.db=self.helper.db;self.config=self.helper.config;self.key='synthetic'.encode('utf-16-le').hex()
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.executescript('ALTER TABLE ItemClassInfo ADD COLUMN Level INTEGER;ALTER TABLE EquipItemInfo ADD COLUMN EquipClass TEXT;ALTER TABLE EquipItemInfo ADD COLUMN CostumeType INTEGER;CREATE TABLE SetInfo(SetID TEXT);CREATE TABLE SetItemInfo(ItemClass TEXT,BaseItemClass TEXT,SetID TEXT);CREATE TABLE SetEffectInfo(SetID TEXT,SetCount INTEGER,EffectTarget TEXT,Amount INTEGER);')
            db.execute('UPDATE ItemClassInfo SET Level=7')
            db.execute('INSERT INTO EquipItemInfo VALUES(?,?,?,?)',('synthetic',None,'LONGSWORD',29))
            db.execute('INSERT INTO SetInfo VALUES(?)',('set_a',))
            db.execute('INSERT INTO SetItemInfo VALUES(?,?,?)',('synthetic','different_base_class','set_a'))
            db.executemany('INSERT INTO SetEffectInfo VALUES(?,?,?,?)',[('set_a',2,'DEF',7),('set_a',3,'DEF',11)])
            db.commit()
    def test_source_subset_metadata_exact_set_counts_and_hashes_are_readonly(self):
        before=[digest(p) for p in (self.db,self.config)]
        p=read_package(self.db,self.config,[self.key])
        self.assertEqual(p['classes'][0]['source_index'],0)
        self.assertEqual(p['classes'][0]['level'],7)
        self.assertEqual(p['equips'][0]['serial'],29)
        self.assertEqual([r['count'] for r in p['effects']],[2,3])
        self.assertEqual(len(p['sets']),1)
        self.assertEqual(before,[digest(p) for p in (self.db,self.config)])
    def test_missing_duplicated_and_unresolved_subset_data_are_refused(self):
        for keys in ([],[self.key,self.key],['4100']):
            with self.assertRaises(ValueError): read_package(self.db,self.config,keys)
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute('DELETE FROM SetInfo');db.commit()
        with self.assertRaises(ValueError): read_package(self.db,self.config,[self.key])
    def test_numeric_source_types_are_not_defaulted(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("UPDATE EquipItemInfo SET CostumeType='invalid'");db.commit()
        with self.assertRaises(ValueError): read_package(self.db,self.config,[self.key])
