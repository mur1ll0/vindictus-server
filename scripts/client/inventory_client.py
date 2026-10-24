"""Read-only comparison of legacy clients. Python standard library only."""
import hashlib
import json
import re
import sqlite3
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
OUTPUT = ROOT / 'vindictus-server/docs/evidence/client'
CLIENTS = [ROOT / 'Client', ROOT / 'Vindictus1.69EN/Vindictus EU/en-EU']
def sha256(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()
def describe_db(path):
    with sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True) as connection:
        connection.execute('PRAGMA query_only=ON')
        tables = connection.execute("SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name").fetchall()
        interesting = []
        for name, schema in tables:
            if re.search(r'class|character|version|server|connect|login|account|config', name, re.I):
                count = connection.execute('SELECT COUNT(*) FROM "' + name.replace('"', '""') + '"').fetchone()[0]
                interesting.append({'name': name, 'schema': schema, 'rows': count})
        feature_matrix = []
        table_names = {table[0] for table in tables}
        if 'FeatureMatrix' in table_names:
            feature_matrix = connection.execute('SELECT Feature,"EN-EU-X-GM","ZH-TW-X-GM" FROM FeatureMatrix WHERE Feature IN (\'NGM_disable\',\'GameCode\') ORDER BY Feature').fetchall()
        class_identifiers = []
        if 'CharacterClassInfo' in table_names:
            class_identifiers = [row[0] for row in connection.execute('SELECT DISTINCT Identifier FROM CharacterClassInfo ORDER BY Identifier')]
        return {'path': str(path.relative_to(ROOT)), 'sha256': sha256(path), 'tables': len(tables), 'interesting_tables': interesting, 'feature_matrix_columns': ['Feature', 'EN-EU-X-GM', 'ZH-TW-X-GM'], 'feature_matrix_rows': feature_matrix, 'class_identifiers': class_identifiers}
def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    records = []
    for client in CLIENTS:
        files, strings = [], []
        for relative in ['vindictus.exe','bin/client.dll','bin/server.dll','bip/client.dll','bip/server.dll','start.bat','CurrentInfo.txt','cfg/config_env.cfg','cfg/config_ex.cfg']:
            path = client / relative
            if not path.exists():
                continue
            files.append({'path':relative, 'bytes':path.stat().st_size, 'sha256':sha256(path)})
            if path.suffix.lower() in ('.exe', '.dll'):
                data = path.read_bytes()
                for pattern, encoding in [(rb'[ -~]{4,}', 'ascii'), (rb'(?:[ -~]\x00){4,}', 'utf-16le')]:
                    for match in re.finditer(pattern, data):
                        value = match.group().decode(encoding)
                        if re.search(r'(^[-+]?(ip|port|server|connect|passport)$)|(^[-+]?(ip|port|server|connect)[ =])|server_(ip|addr|port)|login_server|frontend|connect to|127\.0\.0\.1|192\.168\.', value, re.I) and len(value) < 500:
                            strings.append({'file':relative, 'offset':hex(match.start()), 'encoding':encoding, 'value':value})
        records.append({'root':str(client.relative_to(ROOT)), 'files':files, 'endpoint_related_binary_strings':strings})
    databases = [describe_db(client / 'sql/heroes.db3') for client in CLIENTS]
    databases.append(describe_db(ROOT / 'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'))
    (OUTPUT / 'inventory.json').write_text(json.dumps({'clients':records, 'databases':databases}, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'output':str(OUTPUT / 'inventory.json'), 'clients':[{'root':c['root'], 'files':len(c['files']), 'strings':len(c['endpoint_related_binary_strings'])} for c in records], 'databases':[{'path':db['path'], 'tables':db['tables'], 'interesting':[t['name'] for t in db['interesting_tables']]} for db in databases]}, indent=2))
if __name__ == '__main__':
    main()
