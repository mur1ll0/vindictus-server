import contextlib,json,pathlib,sqlite3,tempfile,unittest
from unittest.mock import patch
import export_fiona_title_goals as subject
from export_item_catalog import digest
class TitleGoalExportTests(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=pathlib.Path(self.tmp.name);self.db=self.root/'content';self.initial=self.root/'initializer';(self.root/'src/domain').mkdir(parents=True)
  code={}
  for k in ('titles','contents'):
   p=self.root/(k+'.cs');p.write_text('synthetic '+k,encoding='utf-8');code[k]=dict(path=p.name,sha256=digest(p))
  (self.root/'src/domain/fiona_initializer_sources.json').write_text(json.dumps(code),encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:c.executescript('CREATE TABLE TitleGoalInfo(ID,TitleID);INSERT INTO TitleGoalInfo VALUES(2,17),(1,17),(3,18)');c.commit()
  self.write_initial()
 def write_initial(self):self.initial.write_text(json.dumps(dict(database_sha256=digest(self.db),config_sha256='b'*64,presets=[dict(title_id=17,class_restriction=2),dict(title_id=18,class_restriction=0),dict(title_id=19,class_restriction=2)])),encoding='utf-8')
 def run_export(self):return subject.project(self.db,self.initial,self.root)
 def test_full_selected_goal_sets_preserve_order_and_explicit_empty(self):
  before=digest(self.db);out=self.run_export();self.assertEqual(out['titles'],[dict(title_id=17,goal_ids=[2,1]),dict(title_id=19,goal_ids=[])]);self.assertEqual(digest(self.db),before)
 def test_duplicate_goal_ids_and_wrong_types_are_rejected(self):
  for value in ('2',"'not-an-id'"):
   with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('DELETE FROM TitleGoalInfo WHERE ID=1');c.execute('INSERT INTO TitleGoalInfo VALUES('+value+',17)');c.commit()
   self.write_initial()
   with self.assertRaises(ValueError):self.run_export()
 def test_source_and_initializer_mutation_are_rejected(self):
  raw=json.loads(self.initial.read_text());raw['database_sha256']='c'*64;self.initial.write_text(json.dumps(raw));
  with self.assertRaises(ValueError):self.run_export()
  self.write_initial();target=self.root/'titles.cs';target.write_text('changed',encoding='utf-8')
  with self.assertRaises(ValueError):self.run_export()
if __name__=='__main__':unittest.main()
