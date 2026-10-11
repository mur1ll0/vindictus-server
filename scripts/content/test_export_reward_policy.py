"""Temporary synthetic SQLite fixtures; never open the original game database."""
import contextlib, hashlib, pathlib, sqlite3, tempfile, unittest
from export_reward_policy import read_package

class RewardExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=pathlib.Path(self.tmp.name)
        self.db=self.root/'heroesContents.db3'
        self.config=self.root/'ServiceCore.dll.config'
        self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="ServerCode"><value>1</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>')
        with contextlib.closing(sqlite3.connect(self.db)) as c, c:
            c.execute('CREATE TABLE FeatureMatrix(Feature TEXT,"EN-EU-X-GM" TEXT,"ZH-TW-X-GM" TEXT)')
            c.executemany('INSERT INTO FeatureMatrix VALUES(?,?,?)', [('GameCode','42','43'),('CurrentVer','3.09.00','10'),('OverrideSetting',None,None),('a','1.1',None),('zhTW',None,'1')])
            c.execute('CREATE TABLE DormantReward(RewardItemClass TEXT,Num INTEGER,MinLevel INTEGER,MaxLevel INTEGER,Feature TEXT)')
            c.executemany('INSERT INTO DormantReward VALUES(?,?,?,?,?)',[('synthetic',2,0,0,None),('second',-1,-1,20,'a'),('synthetic',2,0,0,None)])
    def test_configuration_game_code_selects_region_independent_of_folder(self):
        package=read_package(self.db,self.config)
        self.assertEqual(package['manifest']['region'],'EN-EU-X-GM')
        self.assertEqual(dict((v['key'],v['value']) for v in package['features'])['CurrentVer'],'3.09.00')
    def test_order_duplicates_null_and_signed_values_are_preserved(self):
        package=read_package(self.db,self.config)
        rows=package['rewards']
        self.assertEqual(rows[0],rows[2]);self.assertIsNone(rows[0]['feature'])
        self.assertEqual(rows[1]['num'],-1);self.assertEqual(rows[1]['min_level'],-1)
    def test_override_excludes_current_version_game_code_and_override_setting(self):
        with contextlib.closing(sqlite3.connect(self.db)) as c, c:
            c.execute('UPDATE FeatureMatrix SET "EN-EU-X-GM"=? WHERE Feature=?',('ZH-TW-X-GM','OverrideSetting'))
        package=read_package(self.db,self.config)
        values=dict((v['key'],v['value']) for v in package['features'])
        self.assertEqual(values['CurrentVer'],'3.09.00');self.assertEqual(values['a'],'')
        self.assertEqual(package['manifest']['override_region'],'ZH-TW-X-GM')
    def test_read_only_export_leaves_bytes_unchanged(self):
        before=[hashlib.sha256(v.read_bytes()).digest() for v in (self.db,self.config)]
        read_package(self.db,self.config)
        self.assertEqual(before,[hashlib.sha256(v.read_bytes()).digest() for v in (self.db,self.config)])
        self.assertEqual(set(self.root.iterdir()),{self.config,self.db})
    def test_invalid_sqlite_types_and_integer_overflow_are_rejected(self):
        for value in ('invalid',2147483648,1.5):
            with contextlib.closing(sqlite3.connect(self.db)) as c, c:c.execute('UPDATE DormantReward SET Num=?',(value,))
            with self.assertRaises(ValueError):read_package(self.db,self.config)
    def test_schema_filename_and_row_limits_fail_closed(self):
        with contextlib.closing(sqlite3.connect(self.db)) as c, c:
            c.executemany('INSERT INTO DormantReward VALUES(?,?,?,?,?)',[('synthetic',1,0,0,None)]*1000)
        with self.assertRaises(ValueError):read_package(self.db,self.config)
        with contextlib.closing(sqlite3.connect(self.db)) as c, c:c.execute('DELETE FROM DormantReward');c.execute('ALTER TABLE DormantReward ADD COLUMN Unknown INTEGER')
        with self.assertRaises(ValueError):read_package(self.db,self.config)
        self.config.write_text(self.config.read_text().replace('heroesContents.db3','unexpected.db3'))
        with self.assertRaises(ValueError):read_package(self.db,self.config)
    def test_duplicate_feature_keys_follow_source_last_assignment(self):
        with contextlib.closing(sqlite3.connect(self.db)) as c, c:c.execute('INSERT INTO FeatureMatrix VALUES(?,?,?)',('a','3.1',None))
        package=read_package(self.db,self.config)
        self.assertEqual(dict((v['key'],v['value']) for v in package['features'])['a'],'3.1')

    def test_single_split_part_is_literal_feature_key(self):
        with contextlib.closing(sqlite3.connect(self.db)) as c,c:
            c.execute('INSERT INTO FeatureMatrix VALUES(?,?,?)',('single||','1',None))
            c.execute('UPDATE DormantReward SET Feature=?',('single||',))
        package=read_package(self.db,self.config)
        values=dict((v['key'],v['value']) for v in package['features'])
        self.assertEqual(values['single||'],'1');self.assertNotIn('single',values)

if __name__=='__main__':unittest.main()