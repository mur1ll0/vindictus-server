"""Read-only probe of the NA x64 client.dll/server.dll for the Delia player entity.

Targeted reading only (no whole-binary analysis): the DLL hashes pin the build,
the recovered anchors (factory, constructor, vtable) are re-verified, and the
vtable slots that the Delia and Hagie entities override against the base player
are written to docs/evidence/client/na-x64-delia-entity.json.

Usage: python -I scripts/reverse/na_x64_delia_entity.py [bin_x64 dir]
"""
import hashlib
import json
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/client/.deps'))
import capstone  # noqa: E402
import pefile  # noqa: E402

OUTPUT = ROOT / 'docs/evidence/client/na-x64-delia-entity.json'
EXPECTED = {
    'client.dll': '0be9bfa584761111',
    'server.dll': '9f674dbab6f31b0d',
}
# RVA anchors recovered in this step. Factory strings, constructors and vtables are
# re-checked below; cvar objects and defaults are copied from the registration code.
ANCHORS = {
    'client.dll': {
        'factory_strings': ['player_delia', 'C_HeroesPlayerDelia'],
        'object_size': 0x8AE0,
        'constructors': {'base': 0xABEB50, 'delia': 0xAF1220, 'hagie': 0xAFA570},
        'helper_vtables': {'ragdoll_launch': 0x1AF6ED0, 'special_hold_skill': 0x1AF6EF0},
        'cvars': {
            'cl_delia_debug': (0x22F1EF0, '0'),
            'delia_guard_atten_reset_always': (0x22F1F60, '1'),
            'delia_run_direct_stabilize_count': (0x22F2190, '5'),
            'plr_poseparam_run_yaw_return_speed': (0x22F1FD0, '55.0'),
            'plr_poseparam_run_yaw_speed': (0x22F2040, '180.0'),
            'plr_poseparam_sprint_yaw_return_speed': (0x22F20B0, '20.0'),
            'plr_poseparam_sprint_yaw_speed': (0x22F2120, '40.0'),
        },
    },
    'server.dll': {
        'factory_strings': ['player_delia', 'CHeroesPlayerDelia'],
        'object_size': 0xAF98,
        'constructors': {'base': 0x692630, 'delia': 0x6E3AE0},
        'vtables': {'hagie': 0x10FADC0},
    },
}


def sha256(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


class Image:
    def __init__(self, path):
        self.pe = pefile.PE(str(path), fast_load=True)
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.data = self.pe.get_memory_mapped_image()
        self.sections = {s.Name.rstrip(b'\0').decode(): (s.VirtualAddress, s.Misc_VirtualSize) for s in self.pe.sections}
        # VMProtect moved the exception directory out of .pdata; use the data directory.
        directory = self.pe.OPTIONAL_HEADER.DATA_DIRECTORY[3]
        table = self.data[directory.VirtualAddress:directory.VirtualAddress + directory.Size]
        self.functions = {}
        for offset in range(0, len(table) - 11, 12):
            begin, end = struct.unpack_from('<II', table, offset)
            if 0 < end - begin < 0x100000:
                self.functions.setdefault(begin, end)
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)

    def u64(self, rva):
        return struct.unpack_from('<Q', self.data, rva)[0]

    def cstring(self, rva, limit=96):
        raw = bytes(self.data[rva:rva + limit])
        end = raw.find(b'\0')
        if end > 0 and all(32 <= c < 127 for c in raw[:end]):
            return raw[:end].decode()
        return None

    def is_code(self, rva):
        return any(v <= rva < v + size for name, (v, size) in self.sections.items() if name in ('.text', '.vmp0'))

    def vtable(self, rva):
        slots = []
        while True:
            pointer = self.u64(rva + 8 * len(slots)) - self.base
            if not (0 < pointer < 0x80000000 and self.is_code(pointer)):
                return slots
            slots.append(pointer)

    def body(self, rva, fallback=0x200):
        end = self.functions.get(rva)
        code = bytes(self.data[rva:end if end else rva + fallback])
        out = []
        for instruction in self.md.disasm(code, self.base + rva):
            if end is None and instruction.mnemonic == 'int3':
                break
            out.append(instruction)
        return out

    def rip_target(self, instruction):
        match = re.search(r'rip \+ (0x[0-9a-f]+)', instruction.op_str)
        if match:
            return instruction.address + instruction.size + int(match.group(1), 16) - self.base
        return None

    def ctor_vtables(self, rva):
        """(this offset, vtable rva) stores in a constructor body."""
        this, registers, stores = {'rcx'}, {}, []
        for instruction in self.body(rva):
            text = instruction.op_str
            alias = re.match(r'(\w+), rcx$', text)
            if instruction.mnemonic == 'mov' and alias and instruction.address - self.base < rva + 0x60:
                this.add(alias.group(1))
            target = self.rip_target(instruction)
            if instruction.mnemonic == 'lea' and target is not None:
                registers[text.split(',')[0]] = target
            store = re.match(r'qword ptr \[(\w+)(?: \+ (0x[0-9a-f]+))?\], (\w+)$', text)
            if instruction.mnemonic == 'mov' and store and store.group(1) in this and store.group(3) in registers:
                stores.append((int(store.group(2) or '0', 16), registers[store.group(3)]))
        return stores

    def summary(self, rva):
        strings, calls = [], 0
        for instruction in self.body(rva, 0x80):
            if instruction.mnemonic == 'call':
                calls += 1
            target = self.rip_target(instruction)
            text = self.cstring(target) if target is not None else None
            if text and len(text) > 2:
                strings.append(text)
        end = self.functions.get(rva)
        return {'rva': hex(rva), 'size': (end - rva) if end else None, 'calls': calls, 'strings': strings}


def overrides(image, tables):
    base = tables['base']
    result = []
    for slot, pointer in enumerate(base):
        changed = {name: table[slot] for name, table in tables.items() if name != 'base' and table[slot] != pointer}
        if changed:
            row = {'slot': slot, 'base': hex(pointer)}
            for name, target in changed.items():
                row[name] = image.summary(target)
            result.append(row)
    return result


def probe(directory):
    report = {}
    for name, anchors in ANCHORS.items():
        path = directory / name
        digest = sha256(path)
        if not digest.startswith(EXPECTED[name]):
            raise SystemExit(f'{name}: unexpected SHA-256 {digest[:16]}')
        image = Image(path)
        found = {text: image.data.find(text.encode() + b'\0') for text in anchors['factory_strings']}
        if min(found.values()) < 0:
            raise SystemExit(f'{name}: factory strings not found')
        tables = {}
        for entity, ctor in anchors['constructors'].items():
            stores = image.ctor_vtables(ctor)
            if not stores or stores[0][0] != 0:
                raise SystemExit(f'{name}: no primary vtable in constructor {ctor:#x}')
            tables[entity] = image.vtable(stores[0][1])
        for entity, rva in anchors.get('vtables', {}).items():
            tables[entity] = image.vtable(rva)
        if len({len(table) for table in tables.values()}) != 1:
            raise SystemExit(f'{name}: vtable lengths differ')
        entry = {
            'sha256': digest,
            'image_base': hex(image.base),
            'object_size': hex(anchors['object_size']),
            'vtable_slots': len(tables['base']),
            'delia_constructor_vtables': [[hex(o), hex(v)] for o, v in image.ctor_vtables(anchors['constructors']['delia'])],
            'overrides': overrides(image, tables),
        }
        if 'helper_vtables' in anchors:
            entry['helper_vtables'] = {k: [hex(x) for x in image.vtable(v)] for k, v in anchors['helper_vtables'].items()}
        if 'cvars' in anchors:
            entry['cvars'] = {}
            for cvar, (rva, default) in anchors['cvars'].items():
                entry['cvars'][cvar] = {'object': hex(rva), 'default': default}
        report[name] = entry
    return report


def main():
    directory = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / '.cache/bin_x64'
    report = probe(directory)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(report, indent=1) + '\n', encoding='utf-8')
    for name, entry in report.items():
        print(name, entry['sha256'][:16], 'slots', entry['vtable_slots'], 'overridden', len(entry['overrides']))


if __name__ == '__main__':
    main()
