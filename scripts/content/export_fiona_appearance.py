"""Read-only normal-Fiona appearance subset, aligned to existing catalog snapshots."""
import argparse,contextlib,json,pathlib,re,sqlite3
from export_item_catalog import read_package as catalog_read,digest
import export_reward_policy as reward
def read_package(database,config,implementation):
 catalog=catalog_read(database,config);m=catalog['manifest'];before=digest(implementation)
 source=implementation.read_text(encoding='utf-8-sig')
 section=source[source.index('public CustomizeCharacter.Result'):]if 'public CustomizeCharacter.Result'in source else source[source.index('CustomizeCharacter('):]
 match=re.search(r'case 1:\s*customizeItemRequestInfo = new CustomizeItemRequestInfo\("([^"]+)",\s*"[^"]*",\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),',section)
 if not match:raise ValueError('Fiona innerarmor source branch changed')
 def text(v):
  if v is None:return None
  if not isinstance(v,str)or len(v.encode('utf-16-le',errors='surrogatepass'))>1024:raise ValueError('Appearance text bound')
  return v.encode('utf-16-le',errors='surrogatepass').hex()
 def integer(v):
  if type(v)is not int or not -(2**31)<=v<2**31:raise ValueError('Appearance integer type/range')
  return v
 def boolean(v):
  # HeroesContentsLoader compares the provider raw string exactly to True.
  if type(v)is not str or v not in('True','False'):raise ValueError('Appearance bool profile')
  return v=='True'
 wanted={'CurrentVer','LanguageTag'}
 def feature(v):
  if v is not None and(not isinstance(v,str)or not v.isascii()or len(v)>512):raise ValueError('Appearance feature bound')
  wanted.update(reward.feature_atoms(v));return v
 def tag(v):
  if v is not None and(not isinstance(v,str)or len(v)>256):raise ValueError('Appearance CashShopType bound')
  return v
 def rows(db,table):
  c=db.execute('SELECT * FROM ['+table+']');cols=[v[0]for v in c.description];result=[]
  for row in c:
   if len(result)>=50000:raise ValueError('Appearance row bound')
   result.append(dict(zip(cols,row)))
  return result
 with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro',uri=True))as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  equips=[]
  for i,r in enumerate(rows(db,'EquipItemInfo')):
   if i>=len(catalog['equips'])or text(r['ItemClass'])!=catalog['equips'][i]['item_utf16']or r['Feature']!=catalog['equips'][i]['feature']:raise ValueError('Appearance equipment order changed')
   equips.append(dict(source_index=i,item_utf16=text(r['ItemClass']),feature=feature(r['Feature']),costume_type=integer(r['CostumeType'])))
  custom=[dict(item_utf16=text(r['ItemClass']),feature=feature(r['Feature']),cash_type=tag(r['CashShopType']),is_cash=boolean(r['IsCash']))for r in rows(db,'CustomizeItemInfo')]
  colors=[dict(category_utf16=text(r['Category']),feature=feature(r['Feature']),cash_type=tag(r['CashShopType']),is_cash=boolean(r['IsCash']),order=integer(r['Order']),color=integer(r['Color']))for r in rows(db,'CustomizeColorInfo')]
  parts=[dict(category_utf16=text(r['ItemCategory']),equip_part=integer(r['EquipPartNum']),costume_part=None if r['CostumePartNum']is None else integer(r['CostumePartNum']),decoration=r['CostumeCategory']=='Decoration')for r in rows(db,'EquipPartInfo')]
 if len(wanted)>256:raise ValueError('Appearance feature subset bound')
 with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro',uri=True))as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  columns={r[1]for r in db.execute('PRAGMA table_info(FeatureMatrix)')}
  def load(tag):
   if tag not in columns:raise ValueError('Configured feature column absent')
   return {k:v for k,v in db.execute('SELECT Feature,"'+tag.replace('"','""')+'"FROM FeatureMatrix')}
  all_features=load(m['region'])
  if m['override_region']:
   for key,value in load(m['override_region']).items():
    if key not in('GameCode','CurrentVer','OverrideSetting'):all_features[key]=value
  features=[]
  for key in sorted(wanted):
   value=all_features.get(key)
   if not key.isascii()or not key or len(key)>256 or value is not None and(not isinstance(value,str)or len(value)>256):raise ValueError('Appearance feature value profile')
   features.append(dict(key=key,value=value))

 if len(equips)!=len(catalog['equips'])or digest(database)!=m['database_sha256']or digest(config)!=m['config_sha256']or digest(implementation)!=before:raise ValueError('Appearance source snapshot changed')
 return dict(format_version=1,source='heroesContents.fiona-appearance',database_sha256=m['database_sha256'],config_sha256=m['config_sha256'],implementation_sha256=before,features=features,equips=equips,custom=custom,colors=colors,parts=parts,inner_item_utf16=text(match.group(1)),inner_colors=[int(match.group(i))for i in (2,3,4)])
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);args=parser.parse_args()
 root=pathlib.Path(__file__).resolve().parents[2];base=root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin';out=pathlib.Path(args.output).resolve()
 if not out.is_relative_to((root/'.cache/private-content').resolve()):raise ValueError('Private output outside ACL cache')
 p=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config',root/'legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemImpl.cs')
 data=json.dumps(p,ensure_ascii=True,separators=(',',':'))
 if len(data.encode())>16*1024*1024:raise ValueError('Appearance JSON bound')
 out.write_text(data,encoding='utf-8');print('Appearance snapshot:custom'+str(len(p['custom']))+';colors'+str(len(p['colors']))+';parts'+str(len(p['parts']))+';feature keys'+str(len(p['features']))+';values private;readonly')
