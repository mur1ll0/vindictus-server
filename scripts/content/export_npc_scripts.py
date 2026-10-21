"""Readonly NPC script projection: NpcTalkInfo.xml defaults, every StoryLineInfo XML and
its row, as compact element trees with XElement.Load(LoadOptions.None) text semantics."""
import argparse, contextlib, hashlib, json, pathlib, re, sqlite3, xml.etree.ElementTree as ET
import xml.etree.ElementTree
from export_item_catalog import read_package as catalog_read, digest
from export_reward_policy import feature_atoms

XML_SPACE = ' \t\r\n'

def tree(element):
    """[name, [[attr, value], ...], text, [children], tail]. XElement.Load(None) drops
    whitespace-only text nodes; XElement.Value is text + each child's Value and tail."""
    if not isinstance(element.tag, str) or '}' in element.tag:
        raise ValueError('Unsupported XML node or namespace')
    keep = lambda t: '' if not t or not t.strip(XML_SPACE) else t
    return [element.tag, [[k, v] for k, v in element.attrib.items()], keep(element.text),
            [tree(c) for c in element], keep(element.tail)]

def region_features(db, config):
    """FeatureMatrix column of the configured GameCode plus OverrideSetting, as the
    item catalog projection resolves it."""
    settings = {e.attrib['name']: e.findtext('value') for e in ET.parse(config).getroot().iter('setting') if 'name' in e.attrib}
    game_code = int(settings['GameCode'])
    columns = [r[1] for r in db.execute('PRAGMA table_info(FeatureMatrix)')]; game_map = {}
    for row in db.execute("SELECT * FROM FeatureMatrix WHERE Feature='GameCode'"):
        for name, value in zip(columns, row):
            if isinstance(value, str) and re.fullmatch(r'[+-]?[0-9]+', value.strip()): game_map[int(value)] = name
    region = game_map.get(game_code, 'ko-KR').upper()
    def column(tag):
        if tag not in columns: raise ValueError('Missing configured feature column')
        return {k: ('' if v is None else v) for k, v in db.execute('SELECT Feature,"' + tag.replace('"', '""') + '" FROM FeatureMatrix')}
    features = column(region); override = features.get('OverrideSetting', '')
    if override:
        for k, v in column(override.upper()).items():
            if k not in ('GameCode', 'CurrentVer', 'OverrideSetting'): features[k] = v
    return features

def condition_atoms(node, out):
    """<feature> conditions evaluate FeatureMatrix.IsEnable(arg1)."""
    if node[0] == 'feature':
        value = node[2] if node[2] is not None else ''
        out |= feature_atoms(value.split(',')[0].strip())
    for c in node[3]: condition_atoms(c, out)

def load(path, hasher):
    data = path.read_bytes()
    if len(data) > 4 * 1024 * 1024: raise ValueError('Script file bound')
    hasher.update(len(data).to_bytes(8, 'little')); hasher.update(data)
    return tree(ET.fromstring(data))

def read_package(base, database, config):
    catalog = catalog_read(database, config)
    hasher = hashlib.sha256()
    index = base / 'NpcTalkInfo.xml'
    root = load(index, hasher)
    if root[0] != 'FileList': raise ValueError('NpcTalkInfo root')
    defaults = []
    for element in root[3]:
        if element[0] != 'Default': continue
        feature = ''
        for k, v in element[1]:
            if k == 'Feature': feature = v
        relative = pathlib.PureWindowsPath(element[2])
        path = base.joinpath(*relative.parts).with_suffix('.xml')
        if not path.resolve().is_relative_to(base.resolve()): raise ValueError('Default script outside XML root')
        defaults.append({'feature': feature, 'path': str(relative), 'tree': load(path, hasher)})
    coerced = 0
    def integer(v):
        # Declared INT: SQLite3.GetValue converts sqlite3_column_int64 to Int32, so a
        # TEXT value reads as SQLite's integer conversion of it ('' is 0).
        nonlocal coerced
        if isinstance(v, str):
            coerced += 1
            v = db.execute('SELECT CAST(? AS INTEGER)', (v,)).fetchone()[0]
        if type(v) is not int or not -(2**31) <= v < 2**31: raise ValueError('StoryLineInfo Int32')
        return v
    def boolean(v):
        if v in ('True', 'False'): return v == 'True'
        if v in (0, 1): return bool(v)
        raise ValueError('StoryLineInfo bool')
    def text(v):
        if v is not None and not isinstance(v, str): raise ValueError('StoryLineInfo text')
        return v
    storylines = []
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN'); db.row_factory = sqlite3.Row
        for r in db.execute('Select * From [StoryLineInfo]'):
            info = {k: text(r[k]) for k in ('StoryLineID', 'StoryLineName', 'XmlPath', 'Npc', 'Category', 'Stage', 'RewardItem', 'RewardSkill', 'Episode', 'Feature')}
            info.update({k: integer(r[k]) for k in ('RewardGold', 'RewardExp', 'RewardAp', 'RewardTitle', 'RewardItemCount', 'RewardSkillRank', 'AutoGiveLevel', 'ClassRestriction', 'Season')})
            info.update({k: boolean(r[k]) for k in ('AutoProcess', 'AutoReveal')})
            entry = {'info': info, 'tree': None}
            if info['XmlPath'] is not None:
                folder = base.joinpath(*pathlib.PureWindowsPath(info['XmlPath']).parts)
                path = folder / (info['StoryLineID'] + '.xml')
                if not path.resolve().is_relative_to(base.resolve()): raise ValueError('Storyline outside XML root')
                entry['tree'] = load(path, hasher)
            storylines.append(entry)
    wanted = {'CurrentVer', 'NewStoryLineDialog', 'FlexibleMapLoading'}
    for d in defaults:
        wanted |= feature_atoms(d['feature']); condition_atoms(d['tree'], wanted)
    for s in storylines:
        wanted |= feature_atoms(s['info']['Feature'] or '')
        if s['tree'] is not None: condition_atoms(s['tree'], wanted)
    if len(wanted) > 512 or any(not k.isascii() or not k or len(k) > 256 for k in wanted): raise ValueError('Feature subset bound')
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
        values = region_features(db, config)
    features = [{'key': k, 'value': values.get(k)} for k in sorted(wanted)]
    if any(f['value'] is not None and (not isinstance(f['value'], str) or len(f['value']) > 256) for f in features): raise ValueError('Feature value bound')
    if (digest(database), digest(config)) != (catalog['manifest']['database_sha256'], catalog['manifest']['config_sha256']):
        raise ValueError('Source changed during export')
    return {'format_version': 1, 'source': 'heroes.npc-scripts',
            'manifest': {'database_sha256': catalog['manifest']['database_sha256'], 'config_sha256': catalog['manifest']['config_sha256'],
                         'scripts_sha256': hasher.hexdigest(), 'defaults': len(defaults), 'storylines': len(storylines),
                         'coerced_integers': coerced},
            'features': features, 'defaults': defaults, 'storylines': storylines}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--output', required=True); args = parser.parse_args()
    project = pathlib.Path(__file__).resolve().parents[2]; server = project.parent / 'ServidorVindictus/Server/zh-tw-x-gm'
    output = pathlib.Path(args.output).resolve(); allowed = (project / '.cache/private-content').resolve()
    if not output.is_relative_to(allowed): raise ValueError('Private output path outside ACL cache')
    package = read_package(server / 'Data/XML', server / 'Bin/heroesContents.db3', server / 'Bin/ServiceCore.dll.config')
    data = json.dumps(package, ensure_ascii=True, separators=(',', ':'))
    if len(data.encode('utf-8')) > 64 * 1024 * 1024: raise ValueError('Private export size bound')
    output.write_text(data, encoding='utf-8', newline='\n')
    print(json.dumps({'defaults': package['manifest']['defaults'], 'storylines': package['manifest']['storylines'], 'bytes': len(data)}))
