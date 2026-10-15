"""Read-only conservative branch inventory, not a constructor or runtime authority.
Inspects ALL Fiona/shared default candidates before Feature selection. Any matching
raw variable/category row is counted even if disabled. Extra customization grants
are outside this report. No item names, categories or numeric stats are emitted.
"""
import contextlib,json,pathlib,sqlite3
from export_item_catalog import digest

def inspect(database,config):
 before=(digest(database),digest(config))
 with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro',uri=True))as db:
  db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
  requests=[]
  for row in db.execute('SELECT ItemClass FROM DefaultItemInfo WHERE CharacterClass IN(1,255)'):
   if len(requests)>=4096:raise ValueError('Default request bound')
   requests.append(row)
  if not requests or len(requests)>4096:raise ValueError('Default request bound')
  keys=[]
  for (value,)in requests:
   if not isinstance(value,str)or not value or len(value)>512 or not value.isascii()or any(ord(c)<32 for c in value):raise ValueError('Unverified item expression profile')
   keys.append(value.split('[')[0])
  def load(table):
   cur=db.execute('SELECT * FROM ['+table+']');columns=[c[0]for c in cur.description]
   result=[]
   for row in cur:
    if len(result)>=50000:raise ValueError('Content row bound')
    result.append(dict(zip(columns,row)))
   return result
  classes=load('ItemClassInfo');variables=load('VariableStatInfo')
  if len(classes)>50000 or len(variables)>50000:raise ValueError('Content row bound')
  known={x['ItemClass']for x in classes}
  counts=dict(default_candidates=len(keys),extended_expressions=sum('['in v for (v,)in requests),
   avatar_prefix_candidates=sum(k.startswith('avatar')for k in keys),
   raw_bracelet_matches=sum(x['ItemClass']in keys and x['Category']=='BRACELET'for x in classes),
   raw_variable_matches=sum(x['ItemClass']in keys for x in variables),
   missing_class_candidates=sum(k not in known for k in keys))
 if (digest(database),digest(config))!=before:raise ValueError('Original snapshot changed')
 return dict(format_version=1,source_database_sha256=before[0],source_config_sha256=before[1],
  originals_read_only=True,feature_filter_applied=False,counts=counts,
  all_default_candidates_avoid_generated_attribute_branches=all(v==0 for k,v in counts.items()if k!='default_candidates'),
  extra_customization_grants_inspected=False,runtime_constructor_authority=False,client_create_verified=False)
if __name__=='__main__':
 root=pathlib.Path(__file__).resolve().parents[2];base=root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
 print(json.dumps(inspect(base/'heroesContents.db3',base/'ServiceCore.dll.config'),separators=(',',':')))
