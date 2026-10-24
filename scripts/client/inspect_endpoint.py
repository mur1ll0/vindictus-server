"""Generate reproducible static endpoint evidence; never execute a legacy program."""
from pathlib import Path
import hashlib, json, re, sqlite3, subprocess, sys
ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'vindictus-server/docs/evidence/client'
sys.path.insert(0, str(Path(__file__).parent / '.deps'))
import capstone
import pefile

def digest(data): return hashlib.sha256(data).hexdigest()
def main():
    engine = ROOT / 'Client/bin/engine.dll'
    data = engine.read_bytes()
    pe = pefile.PE(data=data)
    base = pe.OPTIONAL_HEADER.ImageBase
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    ranges = [(0x420d5e50,158), (0x420d5ef0,19), (0x4204d273,38), (0x423242d6,33), (0x423243a1,162), (0x423244a3,151), (0x42323ba3,277), (0x426e45a0,26), (0x426e45d0,26), (0x426e4630,29), (0x42323ec0,10), (0x4231a630,143)]
    asm = []
    for addr, size in ranges:
        offset = pe.get_offset_from_rva(addr-base)
        asm.append(f'\nVA {addr:#x}, file offset {offset:#x}, {size} bytes')
        asm.extend(f'{i.address:#010x}  {i.bytes.hex():24} {i.mnemonic:8} {i.op_str}' for i in md.disasm(data[offset:offset+size],addr))
    imports = {}
    for descriptor in pe.DIRECTORY_ENTRY_IMPORT:
        for imp in descriptor.imports:
            if imp.address in [0x426ee5c4,0x426ee5d0,0x426ee5d8]:
                imports[hex(imp.address)] = {'dll': descriptor.dll.decode(), 'name': imp.name.decode() if imp.name else str(imp.ordinal)}
    key = bytes((((i//23 - (i//53)*53 + i*18+99)&255) ^ (i%11)) for i in range(2048))
    cfg = []
    for name in ['config_env.cfg','config_ex.cfg']:
        raw = (ROOT/'Client/cfg'/name).read_bytes()
        decoded = bytes(x ^ key[i%2048] for i,x in enumerate(raw))
        assert decoded.startswith(b'startconfig\n')
        assert bytes(x ^ key[i%2048] for i,x in enumerate(decoded)) == raw
        lines = decoded.decode('utf-8').splitlines()
        cfg.append({'path':f'Client/cfg/{name}', 'sha256':digest(raw), 'bytes':len(raw), 'decoded_prefix':'startconfig', 'reencode_exact':True, 'endpoint_candidates':[{'line':i,'text':line} for i,line in enumerate(lines,1) if re.search(r'^(serverhost|serverport|connectserver|server_name_archive)\b',line)]})
    databases = []
    for directory in ['Client','Vindictus1.69EN/Vindictus EU/en-EU']:
        for relative in ['sql/heroes.db3','hfs_extracted/sqlite/heroes.db3']:
            p=ROOT/directory/relative
            with sqlite3.connect(p.resolve().as_uri()+'?mode=ro',uri=True) as c:
                c.execute('PRAGMA query_only=ON')
                rows=c.execute('SELECT Feature,"EN-EU-X-GM","ZH-TW-X-GM" FROM FeatureMatrix WHERE Feature IN (\'EndPoints\',\'DetourEndPoint\',\'NGM_disable\',\'GameCode\') ORDER BY Feature').fetchall()
            databases.append({'path':str(p.relative_to(ROOT)), 'sha256':digest(p.read_bytes()), 'columns':['Feature','EN-EU-X-GM','ZH-TW-X-GM'], 'rows':rows})
    disk=ROOT/'Vindictus1.69EN/VinServer/VinServer 2008 x64.vmdk'
    archive_path=r'xampp\htdocs\en-eu-x-gm\EndPoint.txt'
    sevenzip=Path(r'C:\Program Files\7-Zip\7z.exe')
    result=subprocess.run([str(sevenzip),'e','-so',str(disk),archive_path],capture_output=True,check=True)
    assert len(result.stdout)==149 and b'192.168.50.227' in result.stdout and b'27015' in result.stdout
    OUT.mkdir(parents=True,exist_ok=True)
    (OUT/'EndPoint.legacy.txt').write_bytes(result.stdout)
    (OUT/'engine-endpoint.asm.txt').write_text('\n'.join(asm)+'\n',encoding='utf-8')
    with disk.open('rb') as file: header=file.read(65536)
    evidence={'analysis':'static only; runtime acceptance unverified','engine':{'path':str(engine.relative_to(ROOT)),'sha256':digest(data),'image_base':hex(base),'imports':imports,'sections':[s.Name.decode().rstrip(chr(0)) for s in pe.sections]},'cfg_decode':cfg,'databases':databases,'legacy_endpoint':{'disk':str(disk.relative_to(ROOT)),'disk_bytes':disk.stat().st_size,'disk_first_65536_bytes_sha256':digest(header),'archive_path':archive_path,'bytes':149,'sha256':digest(result.stdout),'extractor':str(sevenzip),'extractor_sha256':digest(sevenzip.read_bytes())},'tool_versions':{'capstone':capstone.__version__,'pefile':pefile.__version__}}
    (OUT/'endpoint-static.json').write_text(json.dumps(evidence,indent=2),encoding='utf-8')
    print(json.dumps({'result':'static evidence generated','imports':imports,'endpoint_bytes':149,'cfg_roundtrip':[x['reencode_exact'] for x in cfg],'runtime_accepted':False},indent=2))
if __name__=='__main__':main()
