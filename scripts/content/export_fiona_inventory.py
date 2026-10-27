"""Read-only finite normal-Fiona inventory profile; raw data stays in private cache."""
import argparse,contextlib,json,pathlib,re,sqlite3
import content_additions
from export_item_catalog import read_package as catalog_read,digest
import export_reward_policy as reward
EXTRA=['BundleExceed','MaxDurabilityLimit','CheckEquipItemInfoVaild','AvatarSynthesis_V2','ingame_cashshop_tircoin_system']
def read_package(database,config,constants):
 catalog=catalog_read(database,config);manifest=catalog['manifest'];classes=[];requests=[];variables=[]
 before=digest(constants);source=constants.read_text(encoding='utf-8-sig')
 match=re.search(r'public static readonly string Gold\s*=\s*"([a-z0-9_]+)"',source)
 if not match:raise ValueError('Gold constant outside verified source profile')
 gold=match.group(1).encode('utf-16-le').hex()
 tir=re.search(r'public static readonly string TirCoin\s*=\s*' + chr(34) + r'([a-z0-9_]+)' + chr(34),source)
 if not tir:raise ValueError('TirCoin constant outside verified profile')
 tircoin=tir.group(1).encode('utf-16-le').hex()
 def text(v):
  if v is None:return None
  if not isinstance(v,str)or len(v.encode('utf-16-le',errors='surrogatepass'))>1024:raise ValueError('Profile text bound')
  return v.encode('utf-16-le',errors='surrogatepass').hex()
 def integer(v):
  if type(v)is not int or not -(2**31)<=v<2**31:raise ValueError('Profile integer bound')
  return v
 with contextlib.closing(content_additions.connect(database))as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  cur=db.execute('SELECT * FROM ItemClassInfo');cols=[c[0]for c in cur.description]
  for index,row in enumerate(cur):
   raw=dict(zip(cols,row))
   if index>=len(catalog['classes']):raise ValueError('Class alignment changed')
   base=catalog['classes'][index]
   if text(raw['ItemClass'])!=base['item_utf16']or raw['Feature']!=base['feature']:raise ValueError('Class order changed')
   classes.append(dict(source_index=index,item_utf16=base['item_utf16'],feature=base['feature'],category_utf16=text(raw['Category']),unique=integer(raw['Unique']),expire_in=None if raw['ExpireIn']is None else integer(raw['ExpireIn'])))
  cur=db.execute('SELECT * FROM DefaultItemInfo');cols=[c[0]for c in cur.description]
  for row in cur:
   raw=dict(zip(cols,row))
   if raw['CharacterClass']in(1,255):requests.append(dict(item_utf16=text(raw['ItemClass']),num=integer(raw['ItemNum']),feature=raw['Feature']))
  for(key,)in db.execute('SELECT ItemClass FROM VariableStatInfo'):variables.append(text(key))
  if len(classes)>50000 or len(requests)>4096 or len(variables)>50000:raise ValueError('Profile row bound')
 wanted={r['key']for r in catalog['features']}|set(EXTRA)
 for r in requests:wanted.update(reward.feature_atoms(r['feature']))
 reward.KEYS=sorted(wanted);features=reward.read_package(database,config)['features']
 if len(classes)!=len(catalog['classes'])or digest(database)!=manifest['database_sha256']or digest(config)!=manifest['config_sha256']or digest(constants)!=before:raise ValueError('Original snapshot changed')
 return dict(format_version=1,source='heroesContents.fiona-inventory',database_sha256=manifest['database_sha256'],config_sha256=manifest['config_sha256'],constants_sha256=before,gold_utf16=gold,tircoin_utf16=tircoin,features=features,classes=classes,requests=requests,variable_keys=variables)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',required=True);args=p.parse_args()
 root=pathlib.Path(__file__).resolve().parents[2];base=root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin';output=pathlib.Path(args.output).resolve()
 if not output.is_relative_to((root/'.cache/private-content').resolve()):raise ValueError('Private output outside ACL cache')
 package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config',root/'legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemClassConst.cs')
 encoded=json.dumps(package,ensure_ascii=True,separators=(',',':'))
 if len(encoded.encode())>16*1024*1024:raise ValueError('Profile JSON bound')
 output.write_text(encoded,encoding='utf-8')
 print('Fiona inventory profile:'+str(len(package['requests']))+' candidates;'+str(len(package['classes']))+' aligned classes;'+str(len(package['features']))+' feature keys;values private;source readonly')
