"""Readonly, source-bound normal first-Fiona initialization projection. Values stay private."""
import argparse,contextlib,datetime,hashlib,json,pathlib,re,sqlite3
from export_item_catalog import digest
import export_reward_policy as reward
SOURCE_FILES={
 'contents':'PlayerService.CharacterServiceCore/CharacterContents.cs',
 'skills':'PlayerService.CharacterServiceCore/SkillManager.cs',
 'effects':'PlayerService.CharacterServiceCore/SkillEffect.cs',
 'stats':'PlayerService.CharacterServiceCore/StatManager.cs',
 'ap':'PlayerService.CharacterServiceCore/APManager.cs',
 'titles':'PlayerService.CharacterServiceCore/TitleManager.cs',
 'loader':'PlayerService.CharacterServiceCore.Functions/CharacterLoader.cs',
 'query':'PlayerService.CharacterServiceCore/CharacterDataContext.cs',
}
KEYS=('SharedInventory','DisableOperationLogging','AutoItemLogging','ProcessAfterDeleteItem','InGameGuild','GuildScore','RandomMission','Attendance_Newbie','AttendanceEvent_Newbie','AttendanceAdditionalPeriod_Newbie','MissionCompletionCountCleanTime','MissionCompletionCountCleanPeriod','AccountTitleStatRatio','CharacterTransferSystem','CharacterExceptSystem','AddTitleFromPreset','DeleteCharacterDelay','DisableCharacterCreation','ServerTrouble','Event_newbie_character_preset_item','PremiumCharacterEventItem')
ENABLE_KEYS=('SharedInventory','DisableOperationLogging','AutoItemLogging','ProcessAfterDeleteItem','InGameGuild','GuildScore','RandomMission','Attendance_Newbie','CharacterTransferSystem','CharacterExceptSystem','AddTitleFromPreset','DisableCharacterCreation','ServerTrouble','DisableLogging','CashShop','DormantAccount','NexonGameSecurity')
def integer(v):
 if type(v)is not int or not -(2**31)<=v<2**31:raise ValueError('Initializer Int32 profile')
 return v
def text(v):
 if not isinstance(v,str):raise ValueError('Initializer text profile')
 b=v.encode('utf-16-le',errors='surrogatepass')
 if len(b)>100:raise ValueError('Initializer text bound')
 return b.hex()
def argument_text(v):
 if not isinstance(v,str):raise ValueError('Skill argument key type')
 b=v.encode('utf-16-le',errors='surrogatepass')
 if len(b)>1024:raise ValueError('Skill argument key bound')
 return b.hex()

def boolean(v):
 # Original BIT provider returns System.Boolean and GetString throws, measured x86/x64.
 if type(v)is not int or v not in(0,1):raise ValueError('Initializer provider BIT profile')
 return v==1
def date_ticks(value):
 if not isinstance(value,str):raise ValueError('Initializer date provider profile')
 m=re.fullmatch(r'(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,7}))?)?',value)
 if not m:raise ValueError('Initializer date text shape')
 year,month,day=(int(m[i])for i in(1,2,3));hour,minute,second=(int(m[i]or 0)for i in(4,5,6))
 dt=datetime.datetime(year,month,day,hour,minute,second)
 return str((dt.toordinal()-1)*864000000000+(hour*3600+minute*60+second)*10000000+int((m[7]or'').ljust(7,'0')))
def read_package(database,config,source_dir,state_file):
 initial_db=digest(database);initial_config=digest(config);initial_state=digest(state_file)
 if state_file.stat().st_size>64*1024*1024:raise ValueError('Character state snapshot bound')
 state=json.loads(state_file.read_text(encoding='utf-8-sig'))
 if state.get('format_version')!=1 or state.get('source')!='study_heroes.character-initialization':raise ValueError('Character state source binding')
 presets=state.get('presets')
 if type(presets)is not list or len(presets)>100000:raise ValueError('Preset source bound')
 ids=[integer(r['title_id'])for r in presets]
 preset_ids=set(ids)
 if len(ids)!=len(set(ids)):raise ValueError('Preset dictionary ambiguous')
 files={k:source_dir/v for k,v in SOURCE_FILES.items()};before={k:digest(v)for k,v in files.items()}
 contents=files['contents'].read_text(encoding='utf-8-sig');effects=files['effects'].read_text(encoding='utf-8-sig')
 if not re.search(r'new CharacterStats\(value.BaseSTR, value.BaseDEX, value.BaseINT, value.BaseWILL, 100, value.BaseHP, value.BaseSTAMINA\)',contents):raise ValueError('Source base-stat constructor changed')
 def group(marker):
  start=effects.index(marker);end=effects.index(';',start)
  return set(re.findall(r'"([^"\r\n]+)"',effects[start:end]))
 item_skills=group('private static HashSet<string> itemSkills');stat_skills=group('private static HashSet<string> statSkills')
 wanted=set(KEYS)|set(ENABLE_KEYS);enabled_keys=set(ENABLE_KEYS);rows=[]
 def feature(v):
  if v is not None and(not isinstance(v,str)or not v.isascii()or len(v)>512):raise ValueError('Initializer feature bound')
  atoms=reward.feature_atoms(v);wanted.update(atoms);enabled_keys.update(atoms);return v
 with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro',uri=True))as db:
  db.row_factory=sqlite3.Row;db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  classes=list(db.execute('SELECT * FROM CharacterClassInfo'));class_keys=[(r['Identifier'],r['Level'])for r in classes]
  if len(class_keys)!=len(set(class_keys)):raise ValueError('Class dictionary ambiguous')
  selected=[r for r in classes if r['Identifier']==1 and r['Level']==1]
  if len(selected)!=1 or selected[0]['Feature']is not None:raise ValueError('First-Fiona class source changed')
  r=selected[0];basis=[integer(r[k])for k in('BaseSTR','BaseDEX','BaseINT','BaseWILL','BaseHP','BaseSTAMINA')]
  levels=list(db.execute('SELECT * FROM LevelUpExpInfo'));level_keys=[r['Level']for r in levels]
  if len(level_keys)!=len(set(level_keys)):raise ValueError('Level dictionary ambiguous')
  levels=[dict(level=integer(r['Level']),required_exp=integer(r['RequiredExp']),feature=feature(r['Feature']))for r in levels if r['Level']==1]
  titles=list(db.execute('SELECT ID,AutoGiveLevel,ClassRestriction,Feature FROM TitleInfo'))
  if len(titles)!=len(set(r['ID']for r in titles)):raise ValueError('Title dictionary ambiguous')
  titles=[dict(title_id=integer(r['ID']),auto_give_level=None if r['AutoGiveLevel']is None else integer(r['AutoGiveLevel']),class_restriction=integer(r['ClassRestriction']),feature=feature(r['Feature']))for r in titles if r['AutoGiveLevel']==1 or r['ID']in preset_ids]
  if len(titles)>4096:raise ValueError('Title projection bound')
  for default in db.execute('SELECT * FROM DefaultSkillInfo WHERE CharacterClass IN(1,255)'):
   key=default['SkillID'];rank=integer(default['Rank']);infos=list(db.execute('SELECT ClassRestriction,UseType FROM SkillInfo WHERE SkillID=?',(key,)))
   if len(infos)!=1:raise ValueError('SkillInfo lookup ambiguous')
   ranks=list(db.execute('SELECT * FROM SkillRankInfo WHERE SkillID=? AND Rank IN(?,?)',(key,rank,rank+1)))
   keys=[r['Rank']for r in ranks]
   if len(keys)!=len(set(keys)):raise ValueError('SkillRank lookup ambiguous')
   current=[r for r in ranks if r['Rank']==rank]
   if len(current)!=1:raise ValueError('Default skill rank missing')
   r=current[0];nxt=next((x for x in ranks if x['Rank']==rank+1),None)
   constraints=list(db.execute('SELECT RequiredSkillId,RequiredRank FROM SkillConstraintInfo WHERE SkillId=? AND Rank=?',(key,rank)))
   arguments=[dict(a)for a in db.execute('SELECT * FROM SkillArgumentInfo WHERE SkillId=? AND Rank=?',(key,rank))]
   arg_hash=hashlib.sha256(json.dumps(arguments,ensure_ascii=True,separators=(',',':')).encode()).hexdigest()
   rows.append(dict(id_utf16=text(key),rank=rank,class_restriction=integer(infos[0]['ClassRestriction']),use_type=integer(infos[0]['UseType']),level_constraint=integer(r['LevelConstraint']),rank_feature=feature(r['Feature']),constraints=[dict(id_utf16=text(c['RequiredSkillId']),rank=integer(c['RequiredRank']))for c in constraints],next_rank=None if nxt is None else dict(required_ap=integer(nxt['RequiredAP']),require_unlock=boolean(nxt['RequireUnlock'])),status_effect_types=[r['StatusEffectType_0'+str(n)]for n in(1,2,3)],item_callback=key in item_skills,stat_callback=key in stat_skills,meditation=key=='skill_meditation',argument_rows=len(arguments),argument_sha256=arg_hash,arguments=[dict(key_utf16=None if a["KeyWord"] is None else argument_text(a["KeyWord"]),value=None if a["Value"] is None else integer(a["Value"]))for a in arguments]))
  if len(rows)>256:raise ValueError('Default skill count bound')
  attendance=[dict(version=integer(r['AttendanceEventVer']),date_from_ticks=date_ticks(r['DateFrom']),date_until_ticks=date_ticks(r['DateUntil']))for r in db.execute('SELECT AttendanceEventVer,DateFrom,DateUntil FROM AttendanceEvent')]
  if len(attendance)>4096 or len(attendance)!=len(set(r['version']for r in attendance)):raise ValueError('Attendance dictionary profile')
 package=reward.read_package(database,config,additional_keys=wanted)
 if package['manifest']['database_sha256']!=initial_db or package['manifest']['config_sha256']!=initial_config or digest(state_file)!=initial_state:raise ValueError('Initializer source snapshot changed during read')
 if before!={k:digest(v)for k,v in files.items()}:raise ValueError('Initializer implementation changed during read')
 return dict(format_version=1,source='heroesContents.first-fiona-initializer',database_sha256=package['manifest']['database_sha256'],config_sha256=package['manifest']['config_sha256'],state_sha256=digest(state_file),implementations=before,features=package['features'],enabled_keys=sorted(enabled_keys),base_stats=basis,levels=levels,presets=presets,title_info=titles,skills=rows,attendance=attendance)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',required=True);p.add_argument('--state',required=True);args=p.parse_args()
 root=pathlib.Path.cwd();out=pathlib.Path(args.output).resolve();private=(root/'.cache/private-content').resolve()
 if not out.is_relative_to(private):raise ValueError('Initializer output outside private ACL cache')
 state=pathlib.Path(args.state).resolve()
 if not state.is_relative_to((root/'.cache/private-character-state').resolve()):raise ValueError('State snapshot outside private ACL cache')
 base=root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin';src=root/'legacy/decompiled/PlayerService-f23d8b9df77c'
 data=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config',src,state)
 raw=json.dumps(data,ensure_ascii=True,separators=(',',':'))
 if len(raw.encode())>16*1024*1024:raise ValueError('Initializer JSON bound')
 out.write_text(raw,encoding='utf-8',newline='\n')
 print('Readonly first-Fiona profile:stats6;skills'+str(len(data['skills']))+';preset candidates'+str(len(data['title_info']))+';features'+str(len(data['features']))+';raw values private;runtime integration pending.')
