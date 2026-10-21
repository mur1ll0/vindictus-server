"""Readonly NPC script projection: NpcTalkInfo.xml defaults, every StoryLineInfo XML and
its row, as compact element trees with XElement.Load(LoadOptions.None) text semantics."""
import argparse, contextlib, hashlib, json, pathlib, sqlite3, xml.etree.ElementTree as ET
from export_item_catalog import read_package as catalog_read, digest

XML_SPACE = ' \t\r\n'

def tree(element):
    """[name, [[attr, value], ...], text, [children], tail]. XElement.Load(None) drops
    whitespace-only text nodes; XElement.Value is text + each child's Value and tail."""
    if not isinstance(element.tag, str) or '}' in element.tag:
        raise ValueError('Unsupported XML node or namespace')
    keep = lambda t: '' if not t or not t.strip(XML_SPACE) else t
    return [element.tag, [[k, v] for k, v in element.attrib.items()], keep(element.text),
            [tree(c) for c in element], keep(element.tail)]

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
    if (digest(database), digest(config)) != (catalog['manifest']['database_sha256'], catalog['manifest']['config_sha256']):
        raise ValueError('Source changed during export')
    return {'format_version': 1, 'source': 'heroes.npc-scripts',
            'manifest': {'database_sha256': catalog['manifest']['database_sha256'], 'config_sha256': catalog['manifest']['config_sha256'],
                         'scripts_sha256': hasher.hexdigest(), 'defaults': len(defaults), 'storylines': len(storylines),
                         'coerced_integers': coerced},
            'defaults': defaults, 'storylines': storylines}

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
