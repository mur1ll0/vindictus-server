"""Read-only RGB material sidecar aligned to the existing catalog provider order."""
import argparse,contextlib,json,pathlib,re,sqlite3,xml.etree.ElementTree as ET
import content_additions
from export_item_catalog import read_package as read_catalog,digest
def read_package(database,config):
 catalog=read_catalog(database,config);manifest=catalog['manifest']
 settings={e.attrib['name']:e.findtext('value')for e in ET.parse(config).getroot().iter('setting')if 'name'in e.attrib}
 configured=settings.get('heroesMaterialPath')
 if not configured:raise ValueError('Missing configured material root')
 path=pathlib.PureWindowsPath(configured)
 if path.is_absolute()or path.drive:raise ValueError('Material root requires explicit relative source profile')
 root=(config.parent/pathlib.Path(*path.parts)).resolve()
 if not root.is_relative_to(config.parent.parent.parent.resolve()):raise ValueError('Material root outside original server subtree')
 rows=[];names={}
 def text(v):
  if v is None:return None
  if not isinstance(v,str)or len(v.encode('utf-16-le',errors='surrogatepass'))>512:raise ValueError('Material text bound/type')
  return v.encode('utf-16-le',errors='surrogatepass').hex()
 with contextlib.closing(content_additions.connect(database))as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  cursor=db.execute('Select * From [EquipItemInfo]');columns=[c[0]for c in cursor.description]
  for index,row in enumerate(cursor):
   if index>=len(catalog['equips']):raise ValueError('Equipment alignment changed')
   raw=dict(zip(columns,row));base=catalog['equips'][index];key=text(raw['ItemClass'])
   if key!=base['item_utf16']or raw['Feature']!=base['feature']:raise ValueError('Equipment order changed')
   materials=[]
   for column in ['Material1','Material2','Material3']:
    name=raw[column];encoded=text(name);materials.append(encoded)
    if name is None or encoded in names:continue
    if not re.fullmatch(r'[A-Za-z0-9_.-]{1,128}',name)or name in ('.','..'):raise ValueError('Unverified material path profile')
    source=root/pathlib.PureWindowsPath(name).with_suffix('.raw').name
    if source.is_symlink():raise ValueError('Material source symlink is outside verified profile')
    if source.exists():
     if not source.is_file()or source.stat().st_size>196610:raise ValueError('Material file bound/type')
     before=digest(source);data=source.read_bytes()
     import hashlib
     if hashlib.sha256(data).hexdigest()!=before or digest(source)!=before:raise ValueError('Material changed during read')
     names[encoded]=dict(name_utf16=encoded,raw_hex=data.hex(),sha256=before)
    else:names[encoded]=dict(name_utf16=encoded,raw_hex=None,sha256=None)
    if len(names)>256:raise ValueError('Material table bound')
   rows.append(dict(source_index=index,item_utf16=key,feature=raw['Feature'],materials=materials))
 if len(rows)!=len(catalog['equips'])or digest(database)!=manifest['database_sha256']or digest(config)!=manifest['config_sha256']:raise ValueError('Source snapshot changed')
 # Verify all asset hashes once more after the complete export.
 for entry in names.values():
  name=bytes.fromhex(entry['name_utf16']).decode('utf-16-le')
  source=root/pathlib.PureWindowsPath(name).with_suffix('.raw').name
  if entry['sha256']is None:
   if source.exists():raise ValueError('Missing material appeared during read')
  elif digest(source)!=entry['sha256']:raise ValueError('Material snapshot changed')
 return dict(format_version=1,source='heroesContents.item-colors',database_sha256=manifest['database_sha256'],config_sha256=manifest['config_sha256'],rows=rows,materials=list(names.values()))
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);args=parser.parse_args()
 project=pathlib.Path(__file__).resolve().parents[2];base=project.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
 output=pathlib.Path(args.output).resolve();allowed=(project/'.cache/private-content').resolve()
 if not output.is_relative_to(allowed):raise ValueError('Private output outside ACL cache')
 package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config')
 data=json.dumps(package,ensure_ascii=True,separators=(',',':'))
 if len(data.encode())>16*1024*1024:raise ValueError('Colors sidecar bound')
 output.write_text(data,encoding='utf-8',newline='\n')
 print('Colors exported:'+str(len(package['rows']))+' equipment rows;'+str(len(package['materials']))+' materials;source hashes unchanged;values private')
