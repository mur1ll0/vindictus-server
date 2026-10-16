"""Compact exclusively synthetic oracle cases; never reads content/game state."""
import hashlib,json,pathlib
root=pathlib.Path.cwd()
path=root/'tests/fixtures/framework-lobby-costume.json'
data=json.loads(path.read_text(encoding='utf-8-sig'))
def digest(value):
 return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':')).encode()).hexdigest()
for profile in data['profiles']:
 profile['cases']=[dict(n=n,input_sha256=digest(case['rows']),expected_sha256=digest(case['expected']))for n,case in enumerate(profile['cases'])]
path.write_text(json.dumps(data,separators=(',',':')),encoding='utf-8',newline='\n')
print('Synthetic projection fixture compacted to input/output digests.')
