"""Read-only equipment stats sidecar; same snapshot and row order as the catalog projection."""
import argparse,contextlib,json,pathlib,sqlite3,struct
from export_item_catalog import read_package as read_catalog,digest
STATS=['STR','DEX','INT','LUCK','WILL','HP','STAMINA','ATK','DEF','MATK','ATK_Speed','Critical','Balance','Res_Critical','PVP_ATK','PVP_MATK','PVP_DEF','TOWN_SPEED']
def read_package(database,config):
 catalog=read_catalog(database,config);manifest=catalog['manifest'];rows=[]
 def integer(v):
  if type(v)is not int or not -(2**31)<=v<2**31:raise ValueError('Equipment stat integer type/range')
  return v
 with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro',uri=True))as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  cursor=db.execute('Select * From [EquipItemInfo]');cols=[c[0]for c in cursor.description]
  if not set(STATS+['MaxDurability','Weight','ArmorHP']).issubset(cols):raise ValueError('Missing equipment stat columns')
  for index,row in enumerate(cursor):
   if index>=len(catalog['equips']):raise ValueError('Catalog alignment changed')
   raw=dict(zip(cols,row));projection=catalog['equips'][index]
   key=None if raw['ItemClass']is None else raw['ItemClass'].encode('utf-16-le',errors='surrogatepass').hex()
   if key!=projection['item_utf16']or raw['Feature']!=projection['feature']:raise ValueError('Catalog order changed')
   weight=raw['Weight']
   if type(weight)not in(int,float):raise ValueError('Equipment weight is not a source double')
   rows.append(dict(source_index=index,item_utf16=key,feature=raw['Feature'],stats=[integer(raw[n])for n in STATS],max_durability=integer(raw['MaxDurability']),weight_bits=struct.pack('>d',float(weight)).hex(),armor_hp=integer(raw['ArmorHP'])))
 if len(rows)!=len(catalog['equips'])or digest(database)!=manifest['database_sha256']or digest(config)!=manifest['config_sha256']:raise ValueError('Source snapshot changed')
 return dict(format_version=1,source='heroesContents.item-stats',database_sha256=manifest['database_sha256'],config_sha256=manifest['config_sha256'],rows=rows)
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);args=parser.parse_args()
 project=pathlib.Path(__file__).resolve().parents[2];base=project.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
 output=pathlib.Path(args.output).resolve();allowed=(project/'.cache/private-content').resolve()
 if not output.is_relative_to(allowed):raise ValueError('Private output outside ACL cache')
 package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config');data=json.dumps(package,ensure_ascii=True,separators=(',',':'))
 if len(data.encode())>16*1024*1024:raise ValueError('Stats sidecar bound exceeded')
 output.write_text(data,encoding='utf-8',newline='\n');print('Equipment stats exported:'+str(len(package['rows']))+' rows;21 stat inputs;source hashes unchanged;values private')
