import contextlib,json,pathlib,sqlite3,tempfile,unittest
from unittest.mock import patch
import export_fiona_initializer as subject

class FionaInitializerExportTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
  self.root=pathlib.Path(self.temp.name);self.db=self.root/'content.db3';self.config=self.root/'config';self.state=self.root/'state.json';self.src=self.root/'source'
  self.config.write_text('synthetic configuration',encoding='utf-8')
  self.state.write_text(json.dumps(dict(format_version=1,source='study_heroes.character-initialization',presets=[dict(title_id=17,class_restriction=2)])),encoding='utf-8')
  for key,relative in subject.SOURCE_FILES.items():
   target=self.src/relative;target.parent.mkdir(parents=True,exist_ok=True)
   text='synthetic source marker'
   if key=='contents':text='new CharacterStats(value.BaseSTR, value.BaseDEX, value.BaseINT, value.BaseWILL, 100, value.BaseHP, value.BaseSTAMINA)'
   if key=='effects':text='private static HashSet<string> itemSkills = {"synthetic_skill"}; private static HashSet<string> statSkills = {};'
   target.write_text(text,encoding='utf-8')
  with contextlib.closing(sqlite3.connect(self.db))as c:
   c.executescript("""CREATE TABLE CharacterClassInfo(Identifier,Level,Feature,BaseSTR,BaseDEX,BaseINT,BaseWILL,BaseHP,BaseSTAMINA);
CREATE TABLE LevelUpExpInfo(Level,RequiredExp,Feature);CREATE TABLE TitleInfo(ID,AutoGiveLevel,ClassRestriction,Feature);
CREATE TABLE DefaultSkillInfo(CharacterClass,SkillID,Rank);CREATE TABLE SkillInfo(SkillID,ClassRestriction,UseType);
CREATE TABLE SkillRankInfo(SkillID,Rank,LevelConstraint,Feature,RequiredAP,RequireUnlock,StatusEffectType_01,StatusEffectType_02,StatusEffectType_03);
CREATE TABLE SkillConstraintInfo(SkillId,Rank,RequiredSkillId,RequiredRank);CREATE TABLE SkillArgumentInfo(SkillId,Rank,KeyWord,Value);
CREATE TABLE AttendanceEvent(AttendanceEventVer,DateFrom,DateUntil);
INSERT INTO CharacterClassInfo VALUES(1,1,NULL,1,2,3,4,5,6);INSERT INTO LevelUpExpInfo VALUES(1,81,NULL);
INSERT INTO TitleInfo VALUES(17,1,2,NULL);INSERT INTO DefaultSkillInfo VALUES(1,'synthetic_skill',2);
INSERT INTO SkillInfo VALUES('synthetic_skill',2,0);INSERT INTO SkillRankInfo VALUES('synthetic_skill',2,1,NULL,0,0,NULL,NULL,NULL);
INSERT INTO SkillRankInfo VALUES('synthetic_skill',3,1,NULL,9,0,NULL,NULL,NULL);
INSERT INTO SkillArgumentInfo VALUES('synthetic_skill',2,'synthetic_arg',7);INSERT INTO AttendanceEvent VALUES(7,'0001-01-01 00:00:00.0000001','0001-01-01 00:00:00.0000200');""");c.commit()
 def package(self):
  def reward(database,config,additional_keys):
   self.assertIn('AutoItemLogging',additional_keys);self.assertIn('APMax',additional_keys)
   self.assertIn('DisableOperationLogging',additional_keys);self.assertIn('SharedInventory',additional_keys)
   return dict(manifest=dict(database_sha256=subject.digest(database),config_sha256=subject.digest(config)),features=[])
  with patch.object(subject.reward,'read_package',side_effect=reward):return subject.read_package(self.db,self.config,self.src,self.state)
 def test_readonly_projection_hashes_provider_boolean_and_exact_ticks(self):
  paths=[self.db,self.config,self.state]+[self.src/v for v in subject.SOURCE_FILES.values()];before=[subject.digest(x)for x in paths]
  p=self.package();self.assertEqual(p['base_stats'],[1,2,3,4,5,6]);self.assertEqual(p['skills'][0]['next_rank'],dict(required_ap=9,require_unlock=False));self.assertTrue(p['skills'][0]['item_callback']);self.assertEqual(p['skills'][0]['argument_rows'],1);self.assertEqual(p['skills'][0]['arguments'],[dict(key_utf16=subject.argument_text('synthetic_arg'),value=7)]);self.assertEqual(p['attendance'][0]['date_from_ticks'],'1');self.assertEqual(p['attendance'][0]['date_until_ticks'],'200');self.assertEqual(before,[subject.digest(x)for x in paths]);self.assertEqual(p['state_sha256'],subject.digest(self.state))
 def test_invalid_provider_values_and_source_dictionary_are_refused(self):
  for value in ['False',2,None]:
   with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE SkillRankInfo SET RequireUnlock=? WHERE Rank=3',(value,));c.commit()
   with self.assertRaises(ValueError):self.package()
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE SkillRankInfo SET RequireUnlock=0');c.execute('INSERT INTO CharacterClassInfo SELECT * FROM CharacterClassInfo');c.commit()
  with self.assertRaises(ValueError):self.package()
 def test_ambiguous_preset_and_changed_implementation_are_refused(self):
  p=self.src/subject.SOURCE_FILES['contents'];p.write_text('changed',encoding='utf-8')
  with self.assertRaises(ValueError):self.package()
  self.state.write_text(json.dumps(dict(format_version=1,source='study_heroes.character-initialization',presets=[dict(title_id=1),dict(title_id=1)])),encoding='utf-8')
  with self.assertRaises(ValueError):self.package()
 def test_exact_calendar_parser_rejects_lossy_or_ambiguous_input(self):
  self.assertEqual(subject.date_ticks('0001-01-01'),'0');self.assertEqual(subject.date_ticks('9999-12-31 23:59:59.9999999'),'3155378975999999999')
  for bad in ['2026-01-01T00:00:00Z','2026-02-30', '2026-01-01 00:00:00.12345678',None]:
   with self.assertRaises(ValueError):subject.date_ticks(bad)
 def test_argument_nulls_and_keyword_bounds_are_preserved_or_refused_without_source_writes(self):
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE SkillArgumentInfo SET KeyWord=NULL,Value=NULL');c.commit()
  self.assertEqual(self.package()['skills'][0]['arguments'],[dict(key_utf16=None,value=None)])
  with contextlib.closing(sqlite3.connect(self.db))as c:c.execute('UPDATE SkillArgumentInfo SET KeyWord=?',('x'*513,));c.commit()
  with self.assertRaises(ValueError):self.package()
  self.assertEqual(subject.argument_text('\ud800'),'00d8')
if __name__=='__main__':unittest.main()
