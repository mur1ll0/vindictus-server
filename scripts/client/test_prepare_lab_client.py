"""Safety and preservation tests for the copy-only client bootstrap preparer."""
import importlib.util
import sqlite3
from contextlib import contextmanager

@contextmanager
def connect(*args, **kwargs):
    """Close SQLite handles on every path, including Windows and exceptions."""
    connection = sqlite3.connect(*args, **kwargs)
    try:
        with connection:
            yield connection
    finally:
        connection.close()

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('prepare_lab_client',Path(__file__).with_name('prepare_lab_client.py'))
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.root=Path(self.temp.name)
        self.client=self.root/'lab-client'
        self.database=self.client/'sql/heroes.db3'
        self.database.parent.mkdir(parents=True)
        self.web=self.root/'lab-web'
        with connect(self.database) as c:
            c.execute('CREATE TABLE FeatureMatrix (Feature TEXT, "EN-EU-X-GM" TEXT, "ZH-TW-X-GM" TEXT, Other BLOB)')
            c.execute('INSERT INTO FeatureMatrix VALUES (?,?,?,?)',('EndPoints','http://old/endpoint','unchanged',b'\x00\x01\xff'))
            c.execute('INSERT INTO FeatureMatrix VALUES (?,?,?,?)',('NGM_disable','1','1',None))
            c.execute('CREATE TABLE Keep (id INTEGER PRIMARY KEY, value BLOB)')
            c.execute('INSERT INTO Keep VALUES (?,?)',(42,b'\x00\xff'))
    def tearDown(self):
        self.temp.cleanup()
    def call(self,apply=False,**kwargs):
        args=dict(client_copy=self.client,web_root=self.web,region='EN-EU-X-GM',base_url='http://127.0.0.1:8081',game_host='127.0.0.1',game_port=27015,apply=apply)
        args.update(kwargs)
        return module.prepare(**args)
    def rows(self,path=None):
        with connect(path or self.database) as c:
            return c.execute('SELECT * FROM FeatureMatrix ORDER BY Feature').fetchall(),c.execute('SELECT * FROM Keep').fetchall()
    def test_dry_run_does_not_write_or_create_files(self):
        original=self.database.read_bytes()
        plan=self.call()
        self.assertEqual(plan['mode'],'dry-run')
        self.assertEqual(self.database.read_bytes(),original)
        self.assertFalse(self.web.exists())
        self.assertEqual(list(self.database.parent.glob('*.bak')),[])
    def test_apply_changes_one_cell_and_keeps_complete_reversible_backup(self):
        previous=self.rows()
        plan=self.call(True)
        self.assertEqual(self.rows(Path(plan['database_backup'])),previous)
        changed,keep=self.rows()
        self.assertEqual(keep,previous[1])
        expected=[('EndPoints','http://127.0.0.1:8081/en-eu-x-gm/EndPoint.txt','unchanged',b'\x00\x01\xff'),('NGM_disable','1','1',None)]
        self.assertEqual(changed,expected)
        content=Path(plan['endpoint_file']).read_text()
        self.assertIn('"Address"',content)
        self.assertIn('"127.0.0.1" "27015"',content)
        self.assertFalse(plan['client_login_verified'])
    def test_existing_endpoint_is_backed_up(self):
        path=self.web/'en-eu-x-gm/EndPoint.txt'
        path.parent.mkdir(parents=True)
        path.write_bytes(b'old endpoint')
        plan=self.call(True)
        self.assertEqual(Path(plan['endpoint_backup']).read_bytes(),b'old endpoint')
    def test_missing_or_ambiguous_feature_is_rejected_without_changes(self):
        for duplicate in (False,True):
            with connect(self.database) as c:
                c.execute("DELETE FROM FeatureMatrix WHERE Feature='EndPoints'")
                if duplicate:
                    c.executemany('INSERT INTO FeatureMatrix (Feature) VALUES (?)',[('EndPoints',),('EndPoints',)])
            original=self.database.read_bytes()
            with self.assertRaises(module.PreparationError):self.call(True)
            self.assertEqual(self.database.read_bytes(),original)
            self.assertFalse(self.web.exists())
    def test_unknown_region_and_invalid_host_are_rejected(self):
        for kwargs in [{'region':'../../unsafe'},{'region':'NO-REGION'},{'game_host':'127.0.0.1" injected'},{'game_port':0},{'base_url':'http://user:secret@localhost:8081'}]:
            with self.assertRaises(module.PreparationError):self.call(True,**kwargs)
        self.assertFalse(self.web.exists())
    def test_original_directory_and_symlink_are_rejected(self):
        with patch.object(module,'ORIGINAL_ROOTS',(self.client,)):
            with self.assertRaises(module.PreparationError):self.call(True)
    def test_database_hard_link_to_original_is_rejected(self):
        original=self.root/'Client/sql/heroes.db3'
        original.parent.mkdir(parents=True)
        original.hardlink_to(self.database)
        with patch.object(module,'WORKSPACE',self.root),patch.object(module,'ORIGINAL_ROOTS',(self.root/'Client',)):
            with self.assertRaises(module.PreparationError):self.call(True)
        self.assertFalse(self.web.exists())

if __name__=='__main__':unittest.main(verbosity=2)
