"""Readonly, source-bound skill rank/constraint metadata for an explicit skill subset."""
import argparse, contextlib, json, pathlib, sqlite3
import content_additions
from export_item_catalog import read_package as catalog_read, digest
from export_npc_scripts import region_features
from export_reward_policy import feature_atoms

ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'src/domain/skill_list_sources.json'

def read_package(database, config, selected):
    if not 1 <= len(selected) <= 4096 or len(set(selected)) != len(selected):
        raise ValueError('Skill subset is empty, duplicated or oversized')
    selected = {bytes.fromhex(s).hex() for s in selected}
    if any(len(bytes.fromhex(s)) > 100 or len(bytes.fromhex(s)) % 2 for s in selected):
        raise ValueError('Skill ID UTF16 bound')
    package = catalog_read(database, config)
    sources = json.loads(MANIFEST.read_text(encoding='utf-8-sig'))
    for row in sources.values():
        if digest(ROOT / row['path']) != row['sha256']:
            raise ValueError('Skill implementation source changed')
    def text(v):
        if v is None: return None
        if not isinstance(v, str): raise ValueError('Skill text type')
        b = v.encode('utf-16-le', errors='surrogatepass')
        if len(b) > 1024: raise ValueError('Skill text bound')
        return b.hex()
    def integer(v):
        if type(v) is not int or not -(2**31) <= v < 2**31:
            raise ValueError('Skill Int32 type/range')
        return v
    def boolean(v):
        # HeroesContentsLoader reads bool columns as real booleans or as True/False text.
        if v in (0, 1): return bool(v)
        if v in ('True', 'False'): return v == 'True'
        raise ValueError('Skill bool type')
    with contextlib.closing(content_additions.connect(database)) as db:
        db.row_factory = sqlite3.Row
        # GetTable reads the provider row order; ToDouble*Dictionary keeps the last
        # duplicate, so every selected row is exported in that order.
        skills=[dict(source_index=i, skill_utf16=text(r['SkillID']), class_restriction=integer(r['ClassRestriction']), use_type=integer(r['UseType']), price=integer(r['Price']))
                for i, r in enumerate(db.execute('SELECT * FROM SkillInfo')) if text(r['SkillID']) in selected]
        ranks=[dict(source_index=i, skill_utf16=text(r['SkillID']), rank=integer(r['Rank']), level=integer(r['LevelConstraint']),
                    required_ap=integer(r['RequiredAP']), require_unlock=boolean(r['RequireUnlock']), feature=r['Feature'])
               for i, r in enumerate(db.execute('SELECT * FROM SkillRankInfo')) if text(r['SkillID']) in selected]
        constraints=[dict(source_index=i, skill_utf16=text(r['SkillId']), rank=integer(r['Rank']),
                          required_skill_utf16=text(r['RequiredSkillId']), required_rank=integer(r['RequiredRank']))
                     for i, r in enumerate(db.execute('SELECT * FROM SkillConstraintInfo')) if text(r['SkillId']) in selected]
        # SkillArgumentInfo.Value is Int32?; a null value throws in UpdateSkillArgument.
        arguments=[dict(source_index=i, skill_utf16=text(r['SkillId']), rank=integer(r['Rank']), key_utf16=text(r['KeyWord']),
                        value=None if r['Value'] is None else integer(r['Value']))
                   for i, r in enumerate(db.execute('SELECT * FROM SkillArgumentInfo')) if text(r['SkillId']) in selected]
    if {s['skill_utf16'] for s in skills} != selected or {r['skill_utf16'] for r in ranks} != selected:
        raise ValueError('Selected skill missing from source tables')
    if any(r['feature'] is not None and not isinstance(r['feature'], str) for r in ranks):
        raise ValueError('Skill feature type')
    # The catalog's feature subset omits skill features: export their region values.
    atoms = set()
    for r in ranks: atoms |= feature_atoms(r['feature'] or '')
    if len(atoms) > 256 or any(not k.isascii() or not k or len(k) > 256 for k in atoms): raise ValueError('Skill feature subset bound')
    with contextlib.closing(content_additions.connect(database)) as db:
        values = region_features(db, config)
    features = [dict(key=k, value=values.get(k)) for k in sorted(atoms)]
    if any(f['value'] is not None and (not isinstance(f['value'], str) or len(f['value']) > 256) for f in features):
        raise ValueError('Skill feature value bound')
    if digest(database) != package['manifest']['database_sha256'] or digest(config) != package['manifest']['config_sha256']:
        raise ValueError('Skill source changed during export')
    return dict(format_version=4, source='heroesContents.skill-ranks', database_sha256=package['manifest']['database_sha256'],
                config_sha256=package['manifest']['config_sha256'], implementations={k:r['sha256'] for k,r in sources.items()},
                skills=skills, ranks=ranks, constraints=constraints, arguments=arguments, features=features)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',required=True);parser.add_argument('--skills-file',required=True);args=parser.parse_args()
    out=pathlib.Path(args.output).resolve();private=(ROOT/'.cache/private-content').resolve()
    source=pathlib.Path(args.skills_file).resolve()
    if out.parent != private or source.parent != private: raise ValueError('Use prepared private content directory')
    selected=[r['skill_hex'] for r in json.loads(source.read_text(encoding='utf-8-sig'))]
    base=ROOT.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    package=read_package(base/'heroesContents.db3',base/'ServiceCore.dll.config',selected)
    with out.open('x',encoding='utf-8',newline='\n') as stream: json.dump(package,stream,separators=(',',':'),ensure_ascii=True);stream.write('\n')
    print('Private readonly skill profile exported; values omitted.')

if __name__ == '__main__': main()
