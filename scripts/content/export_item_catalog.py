"""Read-only SELECT-* catalog projection; raw content stays in an ACL-protected cache."""
import argparse,contextlib,hashlib,json,pathlib,re,sqlite3,xml.etree.ElementTree as ET
import content_additions
from export_reward_policy import feature_atoms
import content_overrides as overrides

def digest(path):
 with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def read_package(database,config):
 before=(digest(database),digest(config))
 settings={e.attrib['name']:e.findtext('value') for e in ET.parse(config).getroot().iter('setting') if 'name' in e.attrib}
 if pathlib.PureWindowsPath(settings['heroesContentsDB3']).name!=database.name:raise ValueError('Content profile filename mismatch')
 game_code=int(settings['GameCode'])
 if not -(2**31)<=game_code<2**31:raise ValueError('GameCode outside source integer')
 with contextlib.closing(content_additions.connect(database)) as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  columns=[r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')];game_map={}
  for row in db.execute("SELECT * FROM FeatureMatrix WHERE Feature='GameCode'"):
   for name,value in zip(columns,row):
    if isinstance(value,str) and re.fullmatch(r'[+-]?[0-9]+',value.strip()):game_map[int(value)]=name
  region=game_map.get(game_code,'ko-KR').upper()
  def load(tag):
   if tag not in columns:raise ValueError('Missing configured feature column')
   return {k:('' if v is None else v) for k,v in db.execute('SELECT Feature,"'+tag.replace('"','""')+'" FROM FeatureMatrix')}
  features=load(region);override=features.get('OverrideSetting','')
  if override:
   for k,v in load(override.upper()).items():
    if k not in ('GameCode','CurrentVer','OverrideSetting'):features[k]=v
  wanted={'CurrentVer','StoryTabInInventory','BraceletMaxSlotCount','Macro_EnchantTradeRestriction'}
  def text(value):
   if value is not None and (not isinstance(value,str) or len(value.encode('utf-16-le',errors='surrogatepass'))>1024):raise ValueError('Text projection bound/type')
   return None if value is None else value.encode('utf-16-le',errors='surrogatepass').hex()
  def expression(value):
   if value is not None and (not isinstance(value,str) or not value.isascii() or len(value)>512):raise ValueError('Feature expression bound/profile')
   wanted.update(feature_atoms(value));return value
  def rows(table,classes=False):
   # Same SELECT * as HeroesContentsLoader, with no invented ORDER BY or deduplication.
   # ItemClassInfo MaxStack comes from the original database (content_overrides).
   if table in overrides.COLUMNS:source=overrides.select(db,table)
   else:
    cursor=db.execute('Select * From ['+table+']');cs=[c[0] for c in cursor.description];source=(dict(zip(cs,row)) for row in cursor)
   result=[]
   for raw in source:
    entry={'item_utf16':text(raw['ItemClass']),'feature':expression(raw['Feature'])}
    if classes:
     maximum=raw['MaxStack'];storage=raw['Storage']
     if type(maximum) is not int or not -(2**31)<=maximum<2**31 or type(storage) is not int or not 0<=storage<=255:raise ValueError('Projected integer type/range')
     entry.update(max_stack=maximum,storage=storage,bind=expression(raw['Bind']),trade_on_bind=expression(raw['TradeOnBind']))
    result.append(entry)
    if len(result)>50000:raise ValueError('Catalog row bound')
   return result
  equips=rows('EquipItemInfo');classes=rows('ItemClassInfo',True);extras=rows('ItemClassExtraInfo')
  if len(wanted)>128:raise ValueError('Feature subset bound')
  selected=[]
  for key in sorted(wanted):
   value=features.get(key)
   if not key.isascii() or not key or len(key)>256 or (value is not None and (not isinstance(value,str) or len(value)>256)):raise ValueError('Feature value type/bound')
   selected.append({'key':key,'value':value})
 if (digest(database),digest(config))!=before:raise ValueError('Source hash changed')
 return {'format_version':1,'source':'heroesContents.item-catalog-projection','manifest':dict(database_sha256=before[0],config_sha256=before[1],region=region,override_region=override.upper() if override else None,equip_rows=len(equips),class_rows=len(classes),extra_rows=len(extras)),'features':selected,'equips':equips,'classes':classes,'extras':extras}
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);args=parser.parse_args()
 project=pathlib.Path(__file__).resolve().parents[2];base=project.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
 output=pathlib.Path(args.output).resolve();allowed=(project/'.cache/private-content').resolve()
 if not output.is_relative_to(allowed):raise ValueError('Private output path outside ACL cache')
 package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config');data=json.dumps(package,ensure_ascii=True,separators=(',',':'))
 if len(data.encode('utf-8'))>16*1024*1024:raise ValueError('Private export size bound')
 output.write_text(data,encoding='utf-8',newline='\n')
 print('Catalog export: equips='+str(len(package['equips']))+'; classes='+str(len(package['classes']))+'; extras='+str(len(package['extras']))+'; features='+str(len(package['features']))+'; source hashes unchanged; data private')
