"""Read-only per-class initial contents for character creation; values stay in the private cache.

For each BaseCharacter of the CustomizeCharacter switch (0-9): the DefaultItemInfo requests the
inventory delivers (CharacterClass == class or 255, table order), the INNERARMOR request of the
switch and the CharacterClassInfo rows GetCharacterStats reads. FeatureMatrix values of every
request feature travel with the file so they are evaluated in the loaded region.
"""
import argparse,contextlib,json,pathlib,re,sqlite3
import content_additions
from export_item_catalog import digest
import export_reward_policy as reward
CLASSES=range(10)
def text(v):
 if v is None:return None
 if not isinstance(v,str)or len(v.encode('utf-16-le',errors='surrogatepass'))>1024:raise ValueError('Class text bound')
 return v.encode('utf-16-le',errors='surrogatepass').hex()
def integer(v):
 if type(v)is not int or not -(2**31)<=v<2**31:raise ValueError('Class integer bound')
 return v
def inners(implementation):
 source=implementation.read_text(encoding='utf-8-sig')
 section=source[source.index('CustomizeCharacter('):]
 found={}
 for m in re.finditer(r'case (\d+):\s*customizeItemRequestInfo = new CustomizeItemRequestInfo\("([^"]+)",\s*"INNERARMOR",\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),',section):
  c=int(m.group(1))
  if c in found:break
  found[c]=dict(inner_item_utf16=text(m.group(2)),inner_colors=[int(m.group(i))for i in(3,4,5)])
 if sorted(found)!=list(CLASSES):raise ValueError('CustomizeCharacter switch changed')
 return found
def read_package(database,config,implementation):
 before=(digest(database),digest(config),digest(implementation))
 inner=inners(implementation);wanted=set();classes=[]
 with contextlib.closing(content_additions.connect(database))as db:
  db.row_factory=sqlite3.Row;db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  defaults=list(db.execute('SELECT CharacterClass,ItemClass,ItemNum,Feature FROM DefaultItemInfo'))
  stats=list(db.execute('SELECT Identifier,Level,Feature,BaseSTR,BaseDEX,BaseINT,BaseWILL,BaseHP,BaseSTAMINA FROM CharacterClassInfo'))
  for c in CLASSES:
   requests=[]
   for r in defaults:
    if r['CharacterClass']in(c,255):
     f=r['Feature']
     if f is not None and(not isinstance(f,str)or not f.isascii()or len(f)>512):raise ValueError('Request feature bound')
     wanted.update(reward.feature_atoms(f));requests.append(dict(item_utf16=text(r['ItemClass']),num=integer(r['ItemNum']),feature=f))
   rows=[r for r in stats if r['Identifier']==c]
   levels=[r['Level']for r in rows]
   if not rows or len(levels)!=len(set(levels))or any(r['Feature']is not None for r in rows):raise ValueError('CharacterClassInfo profile changed')
   if len(requests)>4096 or len(rows)>4096:raise ValueError('Class row bound')
   classes.append(dict(character_class=c,requests=requests,**inner[c],class_stats=[[integer(r[k])for k in('Level','BaseSTR','BaseDEX','BaseINT','BaseWILL','BaseHP','BaseSTAMINA')]for r in sorted(rows,key=lambda r:r['Level'])]))
 package=reward.read_package(database,config,additional_keys=sorted(wanted))
 if before!=(digest(database),digest(config),digest(implementation))or package['manifest']['database_sha256']!=before[0]:raise ValueError('Class source changed during export')
 return dict(format_version=1,source='heroesContents.class-initial',database_sha256=before[0],config_sha256=package['manifest']['config_sha256'],implementation_sha256=before[2],features=package['features'],classes=classes)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',required=True);args=p.parse_args()
 root=pathlib.Path(__file__).resolve().parents[2];out=pathlib.Path(args.output).resolve()
 if not out.is_relative_to((root/'.cache/private-content').resolve()):raise ValueError('Private output outside ACL cache')
 base=root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
 data=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config',root/'legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemImpl.cs')
 raw=json.dumps(data,ensure_ascii=True,separators=(',',':'))
 if len(raw.encode())>4*1024*1024:raise ValueError('Class JSON bound')
 out.write_text(raw,encoding='utf-8',newline='\n')
 print('Class initial contents:classes'+str(len(data['classes']))+';requests'+str(sum(len(c['requests'])for c in data['classes']))+';features'+str(len(data['features']))+';values private;readonly')
