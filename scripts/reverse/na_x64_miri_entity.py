"""Read-only probe of the NA x64 client.dll/server.dll for the Miri player entity (Miri M5).

Same method and DLL hashes as na_x64_delia_entity.py (whose Image class it reuses): the factory
registration of `player_miri` gives the object size and the creator; the constructor's first store is
the vtable. Client: `lea rdx, "player_miri"` at 0x53db2 registers size 0x8a00 and creator 0xb19400,
which allocates and jumps to the constructor 0xb19170 (base constructor 0xabeb50, as Delia's). Server:
the factory object 0x16c8a80 (vtable 0x110ebb8) creates in 0x739bf0, an inlined constructor after the
base constructor 0x692630, storing the vtable 0x110d3e8; object size 0xc3e8.
It writes the overridden slots against the base player (Delia and Hagie alongside) and the float or
string constants that the ExtraClass entry needs (docs/future/delia-class-port.md 4.4): client slots
190/191 and 301, server slots 240/241/466, to docs/evidence/client/na-x64-miri-entity.json.

Usage: python -I scripts/reverse/na_x64_miri_entity.py [bin_x64 dir]
"""
import json
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import na_x64_delia_entity as delia  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'docs/evidence/client/na-x64-miri-entity.json'
ANCHORS = {
    'client.dll': {
        'factory_strings': ['player_miri', 'C_HeroesPlayerMiri'],
        'factory_lea': 0x53DB2,
        'object_size': 0x8A00,
        'creator': 0xB19400,
        'constructors': {'base': 0xABEB50, 'miri': 0xB19170, 'delia': 0xAF1220, 'hagie': 0xAFA570},
        'constant_slots': (190, 191, 301),
    },
    'server.dll': {
        'factory_strings': ['player_miri', 'CHeroesPlayerMiri'],
        'factory_object': 0x16C8A80,
        'object_size': 0xC3E8,
        'creator': 0x739BF0,
        'constructors': {'base': 0x692630, 'delia': 0x6E3AE0},
        'vtables': {'miri': 0x110D3E8, 'hagie': 0x10FADC0},
        'constant_slots': (240, 241, 466),
    },
}


def constant(image, rva):
    """The float an accessor returns (movss xmm0, [rip+X]) or the string it returns or passes on (lea rax/rdx)."""
    for instruction in image.body(rva, 0x40)[:8]:
        target = image.rip_target(instruction)
        if target is None:
            continue
        if instruction.mnemonic == 'movss' and instruction.op_str.startswith('xmm0'):
            return {'float': round(struct.unpack_from('<f', image.data, target)[0], 6)}
        if instruction.mnemonic == 'lea' and instruction.op_str.startswith(('rax', 'rdx')):
            text = image.cstring(target)
            if text:
                return {'string': text}
    return {'code': [f'{i.mnemonic} {i.op_str}' for i in image.body(rva, 0x40)[:6]]}


def probe(directory):
    report = {}
    for name, anchors in ANCHORS.items():
        path = directory / name
        digest = delia.sha256(path)
        if not digest.startswith(delia.EXPECTED[name]):
            raise SystemExit(f'{name}: unexpected SHA-256 {digest[:16]}')
        image = delia.Image(path)
        if min(image.data.find(t.encode() + b'\0') for t in anchors['factory_strings']) < 0:
            raise SystemExit(f'{name}: factory strings not found')
        creator = image.body(anchors['creator'], 0x40)
        if not any(re.search(rf'\b{anchors["object_size"]:#x}\b', i.op_str) for i in creator):
            raise SystemExit(f'{name}: the creator does not allocate {anchors["object_size"]:#x}')
        tables = {}
        for entity, ctor in anchors['constructors'].items():
            stores = image.ctor_vtables(ctor)
            if not stores or stores[0][0] != 0:
                raise SystemExit(f'{name}: no primary vtable in constructor {ctor:#x}')
            tables[entity] = image.vtable(stores[0][1])
        for entity, rva in anchors.get('vtables', {}).items():
            tables[entity] = image.vtable(rva)
        if len({len(t) for t in tables.values()}) != 1:
            raise SystemExit(f'{name}: vtable lengths differ')
        constants = {}
        for slot in anchors['constant_slots']:
            constants[str(slot)] = {entity: constant(image, tables[entity][slot]) for entity in ('miri', 'delia', 'hagie')
                                    if entity in tables}
        report[name] = {
            'sha256': digest,
            'image_base': hex(image.base),
            'object_size': hex(anchors['object_size']),
            'creator': hex(anchors['creator']),
            'vtable_slots': len(tables['base']),
            'miri_overrides': sum(1 for s, p in enumerate(tables['base']) if tables['miri'][s] != p),
            'constants': constants,
            'overrides': delia.overrides(image, {k: tables[k] for k in ('base', 'miri', 'delia') if k in tables}),
        }
    return report


def main():
    directory = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / '.cache/bin_x64'
    report = probe(directory)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(report, indent=1) + '\n', encoding='utf-8')
    for name, entry in report.items():
        print(name, entry['sha256'][:16], 'slots', entry['vtable_slots'], 'miri overrides', entry['miri_overrides'],
              'constants', json.dumps(entry['constants']))


if __name__ == '__main__':
    main()
