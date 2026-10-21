import contextlib, sqlite3, unittest
import test_export_fiona_inventory as inventory_tests
from export_skill_profile import read_package, digest

class SkillProfileExportTests(unittest.TestCase):
    def setUp(self):
        self.helper=inventory_tests.FionaInventoryExportTests();self.helper.setUp();self.addCleanup(self.helper.doCleanups)
        self.db=self.helper.db;self.config=self.helper.config;self.key='skill_synthetic'.encode('utf-16-le').hex()
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.executescript('CREATE TABLE SkillInfo(SkillID TEXT,ClassRestriction INTEGER,UseType INTEGER);CREATE TABLE SkillRankInfo(SkillID TEXT,Rank INTEGER,LevelConstraint INTEGER,RequiredAP INTEGER,RequireUnlock INTEGER,Feature TEXT);CREATE TABLE SkillConstraintInfo(SkillId TEXT,Rank INTEGER,RequiredSkillId TEXT,RequiredRank INTEGER);CREATE TABLE SkillArgumentInfo(SkillId TEXT,Rank INTEGER,KeyWord TEXT,Value INTEGER);')
            db.execute('INSERT INTO SkillInfo VALUES(?,?,?)',('skill_synthetic',2,1))
            db.executemany('INSERT INTO SkillRankInfo VALUES(?,?,?,?,?,?)',[('skill_synthetic',1,1,0,0,None),('skill_synthetic',2,3,30,0,None),('skill_synthetic',2,4,40,1,'feature_b'),('skill_other',2,1,5,0,None)])
            db.execute('INSERT INTO SkillConstraintInfo VALUES(?,?,?,?)',('skill_synthetic',2,'skill_other',1))
            db.executemany('INSERT INTO SkillArgumentInfo VALUES(?,?,?,?)',[('skill_synthetic',1,'bonus',2),('skill_synthetic',2,'bonus',None),('skill_other',1,'bonus',9)])
            db.commit()
    def test_rows_keep_source_order_duplicates_and_hashes_are_readonly(self):
        before=[digest(p) for p in (self.db,self.config)]
        p=read_package(self.db,self.config,[self.key])
        self.assertEqual([r['rank'] for r in p['ranks']],[1,2,2])
        self.assertEqual(p['ranks'][2]['require_unlock'],True)
        self.assertEqual(p['ranks'][2]['feature'],'feature_b')
        self.assertEqual(p['constraints'][0]['required_rank'],1)
        self.assertEqual(p['skills'][0]['class_restriction'],2)
        self.assertEqual((p['format_version'],p['skills'][0]['use_type']),(3,1))
        self.assertEqual([(a['rank'],a['value']) for a in p['arguments']],[(1,2),(2,None)])
        self.assertEqual(before,[digest(p) for p in (self.db,self.config)])
    def test_missing_duplicated_and_mistyped_rows_are_refused(self):
        for keys in ([],[self.key,self.key],['4100']):
            with self.assertRaises(ValueError): read_package(self.db,self.config,keys)
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute("UPDATE SkillRankInfo SET RequiredAP='x'");db.commit()
        with self.assertRaises(ValueError): read_package(self.db,self.config,[self.key])
