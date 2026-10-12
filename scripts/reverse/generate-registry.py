"""Generate a version-specific Rust registration table from validated original CLR metadata."""
from pathlib import Path
import json
import uuid
import re
PROJECT = Path(__file__).resolve().parents[2]
HASH = 'be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
def main():
    x86 = json.loads((PROJECT/'docs/evidence/reverse/framework-types-x86.json').read_text(encoding='utf-8-sig'))
    x64 = json.loads((PROJECT/'docs/evidence/reverse/framework-types-x64.json').read_text(encoding='utf-8-sig'))
    if x86['source_sha256'] != HASH or x64['source_sha256'] != HASH: raise ValueError('Unexpected source identity')
    keys = lambda obj: [(row['name'],row['category'],row['guid']) for row in obj['messages']]
    if keys(x86) != keys(x64): raise ValueError('x86/x64 registry mismatch')
    rows = [row for row in x86['messages'] if row['category'] != 0]
    if len(rows) != 649 or len({row['guid'] for row in rows}) != len(rows): raise ValueError('Unexpected registry count or GUID collision')
    result = ['//! Generated from original ServiceCore.dll metadata on .NET Framework, not invented opcodes.', '//! Regenerate with scripts/reverse/generate-registry.py after exporting both platform tables.', '//! This identifies the recovered server build; matching the client still requires runtime acceptance.', 'use super::converter::ClassRegistration;', 'use uuid::Uuid;', '/// SHA256 of the exact server profile.', f'pub const SOURCE_SHA256: &str = "{HASH}";', '/// Registered nonzero categories in CLR enumeration order.', 'pub const REGISTERED_CLASSES: &[ClassRegistration] = &[']
    for row in rows:
        result.extend([f'    // {row["name"]}', f'    ClassRegistration {{ category_id: {row["category"]}, type_id: Uuid::from_u128(0x{uuid.UUID(row["guid"]).int:032x}) }},'])
    result.append('];')
    for name in ('UserLoginMessage','SyncFeatureMatrixMessage','LoginFailMessage'):
        row = next(r for r in rows if r['name'] == 'ServiceCore.EndPointNetwork.'+name)
        result.extend([f'/// Recovered {name} category for this source hash.', f'pub const {re.sub(r'(?<!^)(?=[A-Z])', '_', name).upper()}_CATEGORY: i32 = {row["category"]};'])
    login = next(r for r in rows if r['name'] == 'ServiceCore.EndPointNetwork.UserLoginMessage')
    result.extend(['/// Recovered CLR UserLogin identity for category-zero dispatch.', f'pub const USER_LOGIN_MESSAGE_TYPE_ID: Uuid = Uuid::from_u128(0x{uuid.UUID(login["guid"]).int:032x});'])
    (PROJECT/'src/protocol/server_profile.rs').write_text('\n'.join(result)+'\n',encoding='utf-8')
    print(f'Generated {len(rows)} source-identified registrations, same GUID/category in x86 and x64.')
if __name__ == '__main__': main()
