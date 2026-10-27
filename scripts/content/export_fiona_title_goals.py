"""Readonly title-goal constructor projection;raw values remain in protected cache."""
import argparse,contextlib,hashlib,json,pathlib,sqlite3
import content_additions
from export_item_catalog import digest

def project(database,initializer,source_root):
 before=digest(database);init_bytes=initializer.read_bytes();initial=json.loads(init_bytes.decode('utf-8-sig'))
 code=json.loads((source_root/'src/domain/fiona_initializer_sources.json').read_text(encoding='utf-8-sig'))
 sources={k:source_root/code[k]['path']for k in ('titles','contents')}
 hashes={k:digest(p)for k,p in sources.items()}
 if before!=initial['database_sha256'] or any(hashes[k]!=code[k]['sha256'] for k in sources):raise ValueError('Title goal source mismatch')
 ids=sorted(r['title_id']for r in initial['presets']if r['class_restriction']&2)
 if len(ids)>4096 or len(ids)!=len(set(ids)):raise ValueError('Title preset scope')
 selected=set(ids);goals={i:[]for i in ids};seen=set()
 with contextlib.closing(content_additions.connect(database))as db:
  db.row_factory=sqlite3.Row;db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  for r in db.execute('SELECT ID,TitleID FROM TitleGoalInfo'):
   if type(r['ID'])is not int or type(r['TitleID'])is not int:raise ValueError('Title goal integer shape')
   if not -2147483648<=r['ID']<=2147483647 or not -2147483648<=r['TitleID']<=2147483647:raise ValueError('Title goal range')
   if r['ID']in seen:raise ValueError('Duplicate title goal ID')
   seen.add(r['ID'])
   if r['TitleID']in selected:goals[r['TitleID']].append(r['ID'])
 if sum(map(len,goals.values()))>16384 or any(len(v)>1024 for v in goals.values()):raise ValueError('Title goal count')
 if digest(database)!=before or initializer.read_bytes()!=init_bytes or any(digest(sources[k])!=hashes[k]for k in sources):raise ValueError('Title goal source changed')
 return dict(format_version=1,source='heroesContents.initial-title-goals',database_sha256=before,config_sha256=initial['config_sha256'],initializer_sha256=hashlib.sha256(init_bytes).hexdigest(),title_manager_sha256=hashes['titles'],contents_sha256=hashes['contents'],titles=[dict(title_id=i,goal_ids=goals[i])for i in ids])
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--initializer',default='.cache/private-content/fiona-initializer.json');p.add_argument('--output',default='.cache/private-content/fiona-title-goals.json');a=p.parse_args();root=pathlib.Path.cwd();private=(root/'.cache/private-content').resolve();out=pathlib.Path(a.output).resolve();init=pathlib.Path(a.initializer).resolve()
 if not private.is_dir()or not out.is_relative_to(private)or not init.is_relative_to(private):raise ValueError('Title goal files require existing private ACL cache')
 data=project(root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3',init,root);out.write_text(json.dumps(data,separators=(',',':')),encoding='utf-8',newline='\n')
 print('Readonly title goals:titles'+str(len(data['titles']))+';goals'+str(sum(len(t['goal_ids'])for t in data['titles']))+';raw values private;no services executed.')
