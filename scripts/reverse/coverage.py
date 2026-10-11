"""Summarize recovered source coverage without equating decompilation to migration."""
from pathlib import Path
from datetime import datetime, timezone
import json
import re

PROJECT = Path(__file__).resolve().parents[2]
def main():
    inventory = json.loads((PROJECT / 'legacy/inventory.json').read_text(encoding='utf-8-sig'))
    grouped = {}
    for row in inventory['files']:
        grouped.setdefault(row['sha256'], []).append(row)
    rows = []
    for folder in sorted((PROJECT / 'legacy/decompiled').iterdir()):
        if not folder.is_dir():
            continue
        marker_path = folder / '.decompiled.json'
        marker = json.loads(marker_path.read_text(encoding='utf-8-sig')) if marker_path.exists() else {}
        sources = list(folder.rglob('*.cs'))
        line_count = 0
        imports = set()
        for source in sources:
            text = source.read_text(encoding='utf-8-sig', errors='replace')
            line_count += len(text.splitlines())
            imports.update(re.findall(r'\[DllImport\("([^"\n]+)', text))
        digest = marker.get('sha256')
        migrated_parts = ['Packet framing', 'ISAAC generator', 'CryptoTransform + bounded stream receiver'] if folder.name == 'Core-8942a0c26a9a' else []
        if folder.name == 'ServiceCore-be65d59d2385':
            migrated_parts = ['UserLoginMessage payload', 'Heroes ISAAC + CryptoTransformHeroes transport', 'TypeConverter plain-type codec +649 source-identified registrations (no session/authentication)', 'SyncFeatureMatrix + Heroes bootstrap sequence', 'LoginFailMessage codec']
        if folder.name == 'FrontendServiceCore-6be475728091':
            migrated_parts = ['initial TCP registration/feature sequence and diagnostic login refusal only; full states/services pending', 'restricted laboratory identity expressions (not authentication)']
        if folder.name == 'LoginServiceCore-1600fb4b593d':
            migrated_parts = ['new-user insert candidate and CLR defaults only; UserLogin persistence/callbacks pending']
        rows.append({'assembly': folder.name, 'source_sha256': digest, 'original_paths': [r['path'] for r in grouped.get(digest, [])], 'decompilation_status': marker.get('status', 'unknown'), 'csharp_files': len(sources), 'source_lines': line_count, 'native_imports': sorted(imports), 'migration_status': 'partial' if migrated_parts else 'pending', 'ported_parts': migrated_parts, 'game_parity_verified': False})
    result = {'generated_at_utc': datetime.now(timezone.utc).isoformat(), 'definition': 'Source files include third-party libraries, generated wrappers and admin tools. Counts are inventory, not percent migration.', 'migration_complete': False, 'assemblies': rows, 'source_files': sum(r['csharp_files'] for r in rows), 'source_lines': sum(r['source_lines'] for r in rows)}
    output = PROJECT / 'docs/evidence/reverse/coverage.json'
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(f"Coverage inventory: {len(rows)} assemblies, {result['source_files']} C# files; full migration remains pending.")
if __name__ == '__main__':
    main()
