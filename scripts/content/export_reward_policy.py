"""Read-only, bounded export of the recovered dormant reward content subset."""
import contextlib, argparse, hashlib, json, pathlib, re, sqlite3, xml.etree.ElementTree as ET
KEYS = ['CurrentVer','OverrideSetting','DormantAccount','DormantAccount_ver2','DormantAccountInterval','DormantNewAccountInterval','friend_loyalty_system','friend_loyalty_return_interval','NGM_disable','PassportImmigrator','NotifyBanReason','IPLockTime','SecondPassword','EssentialSecondPassword','SecondPasswordClear','SecondPassword_UsingSHA','SecondPassword_IP_Limit','MaxCharacterSlot','UserCareSystem','DisableInGameCheat','DisableCharacterSlotCheat','PurchaseSlot','PremiumSlot','MaxCharacterSlotWithPurchase','CombatRejoinSystem']
COLUMNS = ['RewardItemClass','Num','MinLevel','MaxLevel','Feature']
# Framework String.Trim whitespace; split before trim and retain literal one-part expressions.
WHITESPACE='\u0009\u000a\u000b\u000c\u000d\u0020\u0085\u00a0\u1680'+''.join(chr(i) for i in range(0x2000,0x200b))+'\u2028\u2029\u202f\u205f\u3000'
def feature_atoms(expression, depth=0):
    if depth>32:raise ValueError('Feature depth bound exceeded')
    if not expression:return set()
    for separator in ('||','&&'):
        parts=[p for p in expression.split(separator) if p]
        if len(parts)>=2:return set().union(*(feature_atoms(p,depth+1) for p in parts))
    expression=expression.strip(WHITESPACE)
    if not expression:raise ValueError('Whitespace-only feature is invalid in recovered parser')
    if expression.startswith('!'):return feature_atoms(expression[1:],depth+1)
    return {expression}

def digest(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream,'sha256').hexdigest()
def read_package(database, config):
    source_hash=digest(database); config_hash=digest(config)
    settings={e.attrib['name']:e.findtext('value') for e in ET.parse(config).getroot().iter('setting') if 'name' in e.attrib}
    game_code=int(settings['GameCode']); server_code=int(settings['ServerCode'])
    if any(not -(2**31)<=value<2**31 for value in (game_code,server_code)):raise ValueError('Invalid configured integer')
    if pathlib.PureWindowsPath(settings['heroesContentsDB3']).name != database.name: raise ValueError('Configured content filename differs')
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri()+'?mode=ro',uri=True)) as conn:
        conn.execute('PRAGMA query_only=ON');conn.execute('BEGIN')
        columns=[r[1] for r in conn.execute('PRAGMA table_info(FeatureMatrix)')]
        game_map={}
        for row in conn.execute("SELECT * FROM FeatureMatrix WHERE Feature='GameCode'"):
            for name,value in zip(columns,row):
                if isinstance(value,str) and re.fullmatch(r'[+-]?[0-9]+',value.strip()):
                    code=int(value)
                    if -(2**31)<=code<2**31: game_map[code]=name
        region=game_map.get(game_code,'ko-KR').upper()
        def load(tag):
            if tag not in columns: raise ValueError('Feature region column missing')
            quoted='"'+tag.replace('"','""')+'"'
            result={}
            for key,value in conn.execute('SELECT Feature,'+quoted+' FROM FeatureMatrix'):
                if not isinstance(key,str) or (value is not None and not isinstance(value,str)):raise ValueError('Unsupported feature value type')
                result[key]='' if value is None else value
            return result
        features=load(region); override=features.get('OverrideSetting','')
        if override:
            for key,value in load(override.upper()).items():
                if key not in ('GameCode','CurrentVer','OverrideSetting'): features[key]=value
        if [r[1] for r in conn.execute('PRAGMA table_info(DormantReward)')] != COLUMNS: raise ValueError('Reward schema changed')
        rewards=[]; wanted=set(KEYS)
        for item,num,minimum,maximum,feature in conn.execute('SELECT * FROM DormantReward'):
            if not isinstance(item,str) or len(item.encode('utf-16-le'))>100:raise ValueError('Invalid item class')
            if any(type(x) is not int or not -(2**31)<=x<2**31 for x in (num,minimum,maximum)):raise ValueError('Invalid integer reward field')
            if feature is not None:
                if not isinstance(feature,str) or len(feature.encode('utf-16-le'))>100:raise ValueError('Invalid reward feature')
                wanted.update(feature_atoms(feature))
            rewards.append(dict(item_utf16=item.encode('utf-16-le').hex().upper(),num=num,min_level=minimum,max_level=maximum,feature=feature))
            if len(rewards)>1000:raise ValueError('Reward row bound exceeded')
        if len(wanted)>128:raise ValueError('Feature subset bound exceeded')
    if digest(database)!=source_hash or digest(config)!=config_hash:raise ValueError('Reference files changed during read')
    package=dict(format_version=1,source='heroesContents.dormant-reward-policy',manifest=dict(database_sha256=source_hash,config_sha256=config_hash,region=region,override_region=override.upper() if override else None,game_code=game_code,server_code=server_code,catalog_rows=len(rewards)),features=[dict(key=k,value=features[k] if k in features else None) for k in sorted(wanted)],rewards=rewards)
    return package
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);args=parser.parse_args()
    base=pathlib.Path(__file__).resolve().parents[3]/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    data=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config')
    output=pathlib.Path(args.output).resolve(); allowed=pathlib.Path(__file__).resolve().parents[2]/'.cache/private-content'
    if not output.is_relative_to(allowed.resolve()):raise ValueError('Private output must stay under .cache/private-content')
    text=json.dumps(data,ensure_ascii=True,separators=(',',':'))
    if len(text)>512*1024:raise ValueError('Export size bound exceeded')
    output.write_text(text,encoding='utf-8')
    print('Reward export: rows='+str(len(data['rewards']))+'; region='+data['manifest']['region']+'; reference hashes unchanged; raw content remains private.')
