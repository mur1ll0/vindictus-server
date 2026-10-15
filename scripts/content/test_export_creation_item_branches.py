import contextlib,json,pathlib,sqlite3,tempfile,unittest
from inspect_creation_item_branches import inspect

class CreationItemBranchesTests(unittest.TestCase):
 def prepare(self,root):
  database=root/'content.db3';config=root/'ServiceCore.dll.config';config.write_text('synthetic',encoding='utf-8')
  with contextlib.closing(sqlite3.connect(database))as db,db:
   db.executescript('CREATE TABLE DefaultItemInfo(ItemClass TEXT,CharacterClass INTEGER);CREATE TABLE ItemClassInfo(ItemClass TEXT,Category TEXT,Feature TEXT);CREATE TABLE VariableStatInfo(ItemClass TEXT,VariableStat TEXT,Feature TEXT);')
  return database,config
 def test_plain_fiona_and_shared_candidates_are_conservative_read_only_evidence(self):
  with tempfile.TemporaryDirectory()as temp:
   database,config=self.prepare(pathlib.Path(temp))
   with contextlib.closing(sqlite3.connect(database))as db,db:
    db.executemany('INSERT INTO DefaultItemInfo VALUES(?,?)',[('synthetic_one',1),('synthetic_two',255),('avatar_other',2)])
    db.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?)',[('synthetic_one','WEAPON',None),('synthetic_two','CLOTH',None)])
   before=database.read_bytes();result=inspect(database,config)
   self.assertEqual(result['counts'],dict(default_candidates=2,extended_expressions=0,avatar_prefix_candidates=0,raw_bracelet_matches=0,raw_variable_matches=0,missing_class_candidates=0))
   self.assertTrue(result['all_default_candidates_avoid_generated_attribute_branches'])
   self.assertFalse(result['runtime_constructor_authority']);self.assertFalse(result['extra_customization_grants_inspected'])
   self.assertEqual(database.read_bytes(),before)
   self.assertNotIn('synthetic_one',json.dumps(result))
 def test_disabled_raw_rows_extended_missing_and_avatar_candidates_are_never_silently_skipped(self):
  with tempfile.TemporaryDirectory()as temp:
   database,config=self.prepare(pathlib.Path(temp))
   with contextlib.closing(sqlite3.connect(database))as db,db:
    db.executemany('INSERT INTO DefaultItemInfo VALUES(?,?)',[('avatar_synthetic[A:1]',1),('missing',255)])
    db.execute('INSERT INTO ItemClassInfo VALUES(?,?,?)',('avatar_synthetic','BRACELET','Disabled'))
    db.execute('INSERT INTO VariableStatInfo VALUES(?,?,?)',('avatar_synthetic','STR(-10%|10%)','Disabled'))
   result=inspect(database,config)
   self.assertEqual(result['counts'],dict(default_candidates=2,extended_expressions=1,avatar_prefix_candidates=1,raw_bracelet_matches=1,raw_variable_matches=1,missing_class_candidates=1))
   self.assertFalse(result['all_default_candidates_avoid_generated_attribute_branches'])
 def test_empty_and_unverified_expression_profiles_fail_explicitly(self):
  with tempfile.TemporaryDirectory()as temp:
   database,config=self.prepare(pathlib.Path(temp))
   with self.assertRaises(ValueError):inspect(database,config)
   with contextlib.closing(sqlite3.connect(database))as db,db:db.execute('INSERT INTO DefaultItemInfo VALUES(?,1)',('bad\x00name',))
   with self.assertRaises(ValueError):inspect(database,config)

if __name__=='__main__':unittest.main()
