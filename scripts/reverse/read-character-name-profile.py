"""Read only the effective naming feature values; never export forbidden names."""
import contextlib, json, pathlib, sqlite3, sys
from datetime import datetime, timezone
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/content'))
from export_reward_policy import read_package, digest
base=ROOT.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
db=base/'heroesContents.db3'; config=base/'ServiceCore.dll.config'
manifest=read_package(db,config)['manifest']
tag=manifest['override_region'] or manifest['region']
keys=['NamingRule','NamingRuleMaxBytes','NamingRuleRepeatingCharCheckCount','ChangedCharacterName_ProtectPeriod']
with contextlib.closing(sqlite3.connect(db.resolve().as_uri()+'?mode=ro',uri=True)) as conn:
 conn.execute('PRAGMA query_only=ON'); conn.execute('BEGIN')
 columns=[r[1] for r in conn.execute('PRAGMA table_info(FeatureMatrix)')]
 if tag not in columns: raise ValueError('Recovered region column missing')
 quoted='"'+tag.replace('"','""')+'"'
 rows=list(conn.execute('SELECT Feature,'+quoted+' FROM FeatureMatrix WHERE Feature IN (?,?,?,?)',keys))
 if len(rows)!=4 or len(dict(rows))!=4 or any(not isinstance(v,str) for _,v in rows): raise ValueError('Naming feature profile differs')
features=dict(rows)
assert features['NamingRule']=='^[a-zA-Z]{2,}_?[a-zA-Z]{2,}$'
assert [features[k] for k in keys[1:]]==['20','-1','30']
if digest(db)!=manifest['database_sha256'] or digest(config)!=manifest['config_sha256']:raise ValueError('Reference source changed during read')
output=ROOT/'.cache/verification/character-name-profile.json'
output.write_text(json.dumps({'verified_at_utc':datetime.now(timezone.utc).isoformat(),'source_read_only':True,'manifest':manifest,'features':features,'forbidden_terms_exported':False},indent=2)+'\n',encoding='utf-8')
print('Naming profile:4effective declarations; original hashes unchanged; no forbidden terms exported.')
