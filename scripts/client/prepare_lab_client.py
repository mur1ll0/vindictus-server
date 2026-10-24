"""Prepare a COPY of a legacy client's bootstrap settings. Dry-run by default.

Only FeatureMatrix.EndPoints is changed; no executable or protection is patched.
The generated endpoint describes a future game frontend, never HTTP management.
"""
from __future__ import annotations
import argparse
import ipaddress
import json
import sqlite3
from contextlib import contextmanager

@contextmanager
def connect(*args, **kwargs):
    """Close SQLite handles on every path, including Windows and exceptions."""
    connection = sqlite3.connect(*args, **kwargs)
    try:
        with connection:
            yield connection
    finally:
        connection.close()

import uuid
from pathlib import Path
from urllib.parse import urlsplit

WORKSPACE = Path(__file__).resolve().parents[3]
ORIGINAL_ROOTS = tuple(WORKSPACE / name for name in ('Client','Vindictus1.69EN','ServidorVindictus','Tools','TutorialRagezone'))

class PreparationError(ValueError):
    pass

def reject_original(path: Path) -> Path:
    resolved=path.resolve()
    if resolved == WORKSPACE or any(resolved == root.resolve() or root.resolve() in resolved.parents for root in ORIGINAL_ROOTS):
        raise PreparationError('Original project evidence is read-only; supply a separate client copy/output directory.')
    return resolved

def bootstrap_url(base: str, region: str) -> str:
    split=urlsplit(base)
    if split.scheme!='http' or not split.hostname or split.username or split.password or split.query or split.fragment or split.path not in ('','/'):
        raise PreparationError('Bootstrap base URL must be plain http://host:port without credentials, path, query or fragment.')
    try:
        split.port
    except ValueError as error:
        raise PreparationError('Invalid bootstrap port.') from error
    return base.rstrip('/')+'/'+region.lower()+'/EndPoint.txt'

def endpoint_bytes(host: str, port: int) -> bytes:
    try:
        ipaddress.IPv4Address(host)
    except ipaddress.AddressValueError as error:
        raise PreparationError('Game host must be an IPv4 address, not a command or URL.') from error
    if not 1<=port<=65535:
        raise PreparationError('Game port must be in 1..65535.')
    return ('"Address"\n{\n\t"Vindictus Laboratory"\n\t{\n\t\tplatform_server_code "1"\n\t\tlocalized_name "SERVER1"\n\t\tIP\n\t\t{\n\t\t\t"'+host+'" "'+str(port)+'"\n\t\t}\n\t}\n}\n').encode('ascii')

def prepare(client_copy: Path, web_root: Path, region: str, base_url: str, game_host: str, game_port: int, apply: bool=False) -> dict:
    client_copy=reject_original(client_copy)
    web_root=reject_original(web_root)
    database=reject_original(client_copy/'sql/heroes.db3')
    endpoint=reject_original(web_root/region.lower()/'EndPoint.txt')
    if not database.is_file():
        raise PreparationError('The client copy must contain sql/heroes.db3.')
    for source in [WORKSPACE/'Client/sql/heroes.db3',WORKSPACE/'Vindictus1.69EN/Vindictus EU/en-EU/sql/heroes.db3']:
        if source.exists() and database.samefile(source):
            raise PreparationError('The client database is the original file or a hard link to it.')
    content=endpoint_bytes(game_host,game_port)
    new_url=bootstrap_url(base_url,region)
    with connect(database.as_uri()+'?mode=ro',uri=True) as read:
        read.execute('PRAGMA query_only=ON')
        columns=[row[1] for row in read.execute('PRAGMA table_info(FeatureMatrix)')]
        if region not in columns or not region.replace('-','').isalnum() or not region.isascii():
            raise PreparationError('Requested region is not an exact FeatureMatrix column.')
        column='"'+region.replace('"','""')+'"'
        rows=read.execute('SELECT '+column+' FROM FeatureMatrix WHERE Feature=?',('EndPoints',)).fetchall()
        if len(rows)!=1:
            raise PreparationError('Exactly one FeatureMatrix.EndPoints row is required; missing/ambiguous data is not changed.')
        previous=rows[0][0]
    plan={'mode':'apply' if apply else 'dry-run','database':str(database),'region':region,'previous_bootstrap_url':previous,'new_bootstrap_url':new_url,'endpoint_file':str(endpoint),'game_host':game_host,'game_port':game_port,'game_frontend_implemented':False,'client_login_verified':False}
    if not apply:
        return plan
    if endpoint.exists() and not endpoint.is_file():
        raise PreparationError('Endpoint target must be a regular file.')
    backup=database.with_name(database.name+'.'+uuid.uuid4().hex+'.before-endpoint.bak')
    # SQLite backup includes committed WAL data rather than copying only the base file.
    with connect(database.as_uri()+'?mode=ro',uri=True) as source, connect(backup) as destination:
        source.backup(destination)
    with connect(database) as connection:
        connection.execute('BEGIN IMMEDIATE')
        current=connection.execute('SELECT '+column+' FROM FeatureMatrix WHERE Feature=?',('EndPoints',)).fetchall()
        if current!=rows:
            raise PreparationError('Database changed after planning; backup retained and update cancelled.')
        cursor=connection.execute('UPDATE FeatureMatrix SET '+column+'=? WHERE Feature=? AND '+column+' IS ?', (new_url,'EndPoints',previous))
        if cursor.rowcount!=1:
            raise PreparationError('Unexpected update count; transaction rolled back.')
        endpoint.parent.mkdir(parents=True,exist_ok=True)
        if endpoint.exists():
            previous_endpoint=endpoint.with_name(endpoint.name+'.'+uuid.uuid4().hex+'.bak')
            previous_endpoint.write_bytes(endpoint.read_bytes())
            plan['endpoint_backup']=str(previous_endpoint)
        temporary=endpoint.with_name(endpoint.name+'.'+uuid.uuid4().hex+'.tmp')
        try:
            temporary.write_bytes(content)
            temporary.replace(endpoint)
        finally:
            temporary.unlink(missing_ok=True)
    plan['database_backup']=str(backup)
    return plan

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy',type=Path,required=True)
    parser.add_argument('--web-root',type=Path,required=True)
    parser.add_argument('--region',default='EN-EU-X-GM')
    parser.add_argument('--bootstrap-base-url',default='http://127.0.0.1:8081')
    parser.add_argument('--game-host',default='127.0.0.1')
    parser.add_argument('--game-port',type=int,default=27015)
    parser.add_argument('--apply',action='store_true',help='Explicitly apply to the copy; otherwise report dry-run only.')
    args=parser.parse_args()
    try:
        print(json.dumps(prepare(args.client_copy,args.web_root,args.region,args.bootstrap_base_url,args.game_host,args.game_port,args.apply),indent=2))
    except (PreparationError,sqlite3.Error,OSError) as error:
        parser.exit(2,str(error)+'\n')
    return 0
if __name__=='__main__':
    raise SystemExit(main())
