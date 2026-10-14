import pathlib,sqlite3,contextlib,json,hashlib
root=pathlib.Path(__file__).resolve().parents[2]; db=root.parent/'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'
def digest(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
before=digest(db)
with contextlib.closing(sqlite3.connect(db.as_uri()+'?mode=ro',uri=True)) as c:
 c.execute('PRAGMA query_only=ON');c.execute('BEGIN')
 naming=json.loads((root/'.cache/verification/character-name-profile.json').read_text(encoding='utf-8-sig'))
 tag=naming['manifest']['override_region'] or naming['manifest']['region']
 quoted='"'+tag.replace('"','""')+'"'
 flags=dict(c.execute('SELECT Feature,'+quoted+' FROM FeatureMatrix WHERE Feature IN (?,?)',('LanguageTag','jaJP')))
 if flags.get('LanguageTag')!='en-EU' or 'jaJP' not in flags or flags['jaJP'] is not None:raise ValueError('Effective language/Japanese profile is unsupported')
 raw_rows=list(c.execute('SELECT ForbiddenName,IsFullMatch,Language FROM ForbiddenNameInfo'))
 if any(type(full) is not str or full not in ('True','False') for _,full,_ in raw_rows):raise ValueError('Unsupported original forbidden flag representation')
 rows=[{'ForbiddenName':name,'IsFullMatch':full=='True','Language':lang} for name,full,lang in raw_rows]
 if len(rows)>100000 or any(not isinstance(r['ForbiddenName'],str) or not isinstance(r['Language'],str) for r in rows):raise ValueError('Unsupported original forbidden profile; values suppressed')
 if digest(db)!=before:raise ValueError('Original content changed during read')
output=root/'.cache/private-content/name-proof-input.json'
text=json.dumps({'candidate':'Murael','language_tag':'en-EU','rows':rows},ensure_ascii=True,separators=(',',':'))
if len(text)>20*1024*1024:raise ValueError('Private oracle input exceeds bound')
output.write_text(text,encoding='utf-8')
source=root/'legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterContents.cs'
code=source.read_text(encoding='utf-8-sig');start=code.index('public static bool IsForbiddenName(string name)');op=code.index('{',start);depth=1;end=op+1
while depth:
 if code[end]=='{':depth+=1
 elif code[end]=='}':depth-=1
 end+=1
method=code[start:end]
template=(root/'scripts/reverse/framework/ForbiddenCandidateOracle.cs.in').read_text(encoding='utf-8')
(root/'.cache/framework/ForbiddenCandidateOracle.cs').write_text(template.replace('__RECOVERED_METHOD__',method),encoding='utf-8')
print('Readonly private content input prepared; actual terms retained only in protected ignored directory.')
