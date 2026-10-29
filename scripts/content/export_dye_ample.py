"""Read-only DyeAmpleSession sidecar: FeatureMatrix DyeCashCursor and the material dye.displace.

The cursor follows the catalog projection's region (GameCode column, then OverrideSetting), and the
displacement table is DistortTable's 256-byte file under the configured heroesMaterialPath. The
sidecar binds to the catalog by the database/config hashes; values stay in the ACL cache."""
import argparse,contextlib,hashlib,json,pathlib,re,sqlite3,xml.etree.ElementTree as ET
from export_item_catalog import digest


def read_package(database,config):
 before=(digest(database),digest(config))
 settings={e.attrib['name']:e.findtext('value') for e in ET.parse(config).getroot().iter('setting') if 'name' in e.attrib}
 if pathlib.PureWindowsPath(settings['heroesContentsDB3']).name!=database.name:raise ValueError('Content profile filename mismatch')
 game_code=int(settings['GameCode'])
 with contextlib.closing(sqlite3.connect('file:'+database.as_posix()+'?mode=ro',uri=True)) as db:
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
 cursor=features.get('DyeCashCursor')
 if cursor is not None and (not isinstance(cursor,str) or len(cursor)>64):raise ValueError('Cursor value type/bound')
 configured=settings.get('heroesMaterialPath')
 if not configured:raise ValueError('Missing configured material root')
 path=pathlib.PureWindowsPath(configured)
 if path.is_absolute() or path.drive:raise ValueError('Material root requires explicit relative source profile')
 root=(config.parent/pathlib.Path(*path.parts)).resolve()
 if not root.is_relative_to(config.parent.parent.parent.resolve()):raise ValueError('Material root outside original server subtree')
 source=root/'dye.displace'
 if source.is_symlink():raise ValueError('Displacement source symlink is outside verified profile')
 displace=None
 if source.exists():
  if not source.is_file() or source.stat().st_size>4096:raise ValueError('Displacement file bound/type')
  data=source.read_bytes()
  if hashlib.sha256(data).hexdigest()!=digest(source):raise ValueError('Displacement changed during read')
  displace=dict(raw_hex=data.hex(),sha256=hashlib.sha256(data).hexdigest())
 if (digest(database),digest(config))!=before:raise ValueError('Source hash changed')
 return {'format_version':1,'source':'heroesContents.dye-ample','database_sha256':before[0],'config_sha256':before[1],'region':region,'cursor':cursor,'displace':displace}


if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output',required=True)
 parser.add_argument('--server-bin',help='original server Bin directory (default: ../ServidorVindictus next to the checkout)')
 args=parser.parse_args()
 project=pathlib.Path(__file__).resolve().parents[2]
 base=pathlib.Path(args.server_bin).resolve() if args.server_bin else project.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
 output=pathlib.Path(args.output).resolve()
 if output.parent.name!='private-content' or output.parent.parent.name!='.cache':raise ValueError('Private output outside ACL cache')
 package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config')
 output.write_text(json.dumps(package,ensure_ascii=True,separators=(',',':')),encoding='utf-8',newline='\n')
 print('Dye ampoule export: cursor '+('present' if package['cursor'] is not None else 'absent')+'; displacement '+('present' if package['displace'] else 'absent')+'; source hashes unchanged; values private')
