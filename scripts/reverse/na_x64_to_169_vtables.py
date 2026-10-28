"""Map the NA x64 player vtable slots onto the 1.69 (32-bit) player vtables.

Read-only and targeted: only the player vtables and the bodies of the methods
they point to are read. Both builds share ten classes; for every slot the set
of classes that override it, the strings and float constants of the overrides
and of the base method form a signature, and a monotone alignment (vtables keep
declaration order) pairs the slots. Writes
docs/evidence/client/na-x64-to-169-vtables.json (local, like the rest of that
folder) and prints the Delia slots with their 1.69 counterpart.

Usage: python -I scripts/reverse/na_x64_to_169_vtables.py [bin_x64 dir] [1.69 bin dir]
"""
import json
import re
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from na_x64_delia_entity import ROOT, Image, sha256  # noqa: E402

import capstone  # noqa: E402
import pefile  # noqa: E402

OUTPUT = ROOT / 'docs/evidence/client/na-x64-to-169-vtables.json'
CLASSES = ['Lethita', 'Fiona', 'Evy', 'Kalok', 'Kay', 'Vella', 'Hurk', 'Lynn', 'Arisha', 'Hagie']
EXPECTED_169 = {'client.dll': 'b284ba719047556b', 'server.dll': 'd7727e182f6cdec1'}
BUILDS = {
    # dll: (x64 base vtable, slot returning the class name, class prefix)
    'client.dll': (0x1AF1340, 13, 'C_HeroesPlayer'),
    'server.dll': (0x10E89E8, 11, 'CHeroesPlayer'),
}
DELIA_SLOTS = {
    'client.dll': [0, 13, 15, 16, 17, 29, 31, 83, 190, 191, 286, 301, 330, 331, 349, 351, 380, 416, 419,
                   455, 470, 476, 485, 487, 489, 516],
    'server.dll': [0, 9, 10, 11, 12, 13, 28, 29, 73, 78, 137, 240, 241, 334, 390, 434, 435, 466, 469, 504,
                   517, 615, 624, 627, 628, 631, 657, 677, 694, 697, 699, 725, 729, 730, 731, 736, 748],
}
# A class vtable overriding more slots than this is a different object sharing the method.
MAX_OVERRIDES = 120


def text_of(data, rva):
    raw = bytes(data[rva:rva + 96])
    end = raw.find(b'\0')
    if end > 2 and all(32 <= c < 127 for c in raw[:end]):
        return raw[:end].decode()
    return None


class Image32:
    def __init__(self, path):
        self.pe = pefile.PE(str(path), fast_load=True)
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.data = self.pe.get_memory_mapped_image()
        self.bytes = bytes(self.data)
        text = next(s for s in self.pe.sections if s.Name.startswith(b'.text'))
        self.text = (text.VirtualAddress, text.Misc_VirtualSize)
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)

    def u32(self, rva):
        return struct.unpack_from('<I', self.data, rva)[0]

    def is_code(self, rva):
        return self.text[0] <= rva < self.text[0] + self.text[1]

    def vtable(self, rva):
        slots = []
        while True:
            pointer = self.u32(rva + 4 * len(slots)) - self.base
            if not (0 < pointer < 0x10000000 and self.is_code(pointer)):
                return slots
            slots.append(pointer)

    def dwords(self, value):
        pattern, found, index = struct.pack('<I', value), [], -1
        while True:
            index = self.bytes.find(pattern, index + 1)
            if index < 0:
                return found
            found.append(index)

    def rtti_vtable(self, mangled):
        """Primary vtable through the RTTI complete object locator."""
        name = f'.?AV{mangled}@@'.encode() + b'\0'
        offset = self.bytes.find(name)
        if offset < 0:
            return None
        descriptor = self.base + offset - 8
        for locator in self.dwords(descriptor):
            locator -= 12
            if self.u32(locator) != 0 or self.u32(locator + 4) != 0:
                continue
            for reference in self.dwords(self.base + locator):
                if self.is_code(self.u32(reference + 4) - self.base):
                    return reference + 4
        return None

    def signature(self, rva):
        strings, floats, end = [], [], 0
        for instruction in self.md.disasm(self.bytes[rva:rva + 0x1800], self.base + rva):
            if instruction.mnemonic == 'int3':
                break
            for match in re.finditer(r'0x([0-9a-f]{8})', instruction.op_str):
                target = int(match.group(1), 16) - self.base
                if not 0 < target < len(self.bytes) - 4:
                    continue
                text = text_of(self.data, target)
                if text:
                    strings.append(text)
                elif 'ss' in instruction.mnemonic or 'fld' in instruction.mnemonic:
                    floats.append(round(struct.unpack_from('<f', self.data, target)[0], 4))
            if instruction.mnemonic.startswith('j') and instruction.op_str.startswith('0x'):
                end = max(end, int(instruction.op_str, 16))
            if instruction.mnemonic == 'ret' and instruction.address >= end:
                break
            if instruction.mnemonic == 'jmp' and instruction.address >= end and not instruction.op_str.startswith('0x'):
                break
        return tuple(sorted(set(strings))), tuple(sorted(set(floats)))


def signature64(image, rva):
    strings, floats, end = [], [], 0
    stop = image.functions.get(rva, rva + 0x2000)
    for instruction in image.md.disasm(bytes(image.data[rva:stop]), image.base + rva):
        if instruction.mnemonic == 'int3':
            break
        target = image.rip_target(instruction)
        if target is not None:
            text = image.cstring(target)
            if text and len(text) > 2:
                strings.append(text)
            elif 'ss' in instruction.mnemonic:
                floats.append(round(struct.unpack_from('<f', image.data, target)[0], 4))
        if instruction.mnemonic.startswith('j') and instruction.op_str.startswith('0x'):
            end = max(end, int(instruction.op_str, 16))
        if instruction.mnemonic == 'ret' and instruction.address >= end:
            break
        if instruction.mnemonic == 'jmp' and instruction.address >= end and not instruction.op_str.startswith('0x'):
            break
    return tuple(sorted(set(strings))), tuple(sorted(set(floats)))


def x64_class_vtables(image, base_vtable, name_slot, prefix):
    """Class vtables found through their `lea rax, "<prefix><Class>"; ret` method."""
    base = image.vtable(base_vtable)
    data = bytes(image.data)
    occurrences = {}
    for cls in CLASSES:
        needle, index, found = (prefix + cls).encode() + b'\0', -1, []
        while True:
            index = data.find(needle, index + 1)
            if index < 0:
                break
            if data[index - 1:index] == b'\0':
                found.append(index)
        occurrences[cls] = found
    refs = rip_references(image, [o for found in occurrences.values() for o in found])
    tables = {}
    for cls, found in occurrences.items():
        for occurrence in found:
            for ref in refs[occurrence]:
                method = ref - 3
                if data[method:method + 3] != b'\x48\x8d\x05' or data[method + 7:method + 8] != b'\xc3':
                    continue
                pattern, index = struct.pack('<Q', image.base + method), -1
                while True:
                    index = data.find(pattern, index + 1)
                    if index < 0:
                        break
                    candidate = index - 8 * name_slot
                    slots = image.vtable(candidate)
                    if len(slots) == len(base) and candidate != base_vtable:
                        changed = sum(1 for a, b in zip(slots, base) if a != b)
                        if changed < MAX_OVERRIDES:
                            tables[cls] = candidate
    return tables


def rip_references(image, targets):
    start, size = image.sections['.text']
    code = bytes(image.data[start:start + size])
    wanted, found = set(targets), {t: [] for t in targets}
    for shift in range(4):
        values = struct.unpack_from('<%di' % ((len(code) - shift - 4) // 4), code, shift)
        next_ip = start + shift + 4
        for index, displacement in enumerate(values):
            target = displacement + next_ip + 4 * index
            if target in wanted:
                found[target].append(next_ip - 4 + 4 * index)
    return found


def features(tables, base, signature):
    cache = {}

    def sig(rva):
        if rva not in cache:
            cache[rva] = signature(rva)
        return cache[rva]

    masks = [set() for _ in base]
    overrides = [{} for _ in base]
    for cls, table in tables.items():
        for slot, pointer in enumerate(table):
            if pointer != base[slot]:
                masks[slot].add(cls)
                overrides[slot][cls] = sig(pointer)
    return masks, overrides, [sig(pointer) for pointer in base]


def score(old, new, i, j):
    masks_a, over_a, base_a = old
    masks_b, over_b, base_b = new
    value = 0.0
    union = masks_a[i] | masks_b[j]
    if union:
        common = masks_a[i] & masks_b[j]
        value += 3.0 * len(common) / len(union) - 1.0
        for cls in common:
            a, b = over_a[i][cls], over_b[j][cls]
            if a == b and (a[0] or a[1]):
                value += 2.0
            elif set(a[0]) & set(b[0]):
                value += 1.0
    if base_a[i][0] or base_a[i][1]:
        if base_a[i] == base_b[j]:
            value += 3.0
        elif set(base_a[i][0]) & set(base_b[j][0]):
            value += 1.5
        elif base_b[j][0] or base_b[j][1]:
            value -= 0.3
    return value


def align(old, new):
    rows, cols = len(old[0]), len(new[0])
    skip_old, skip_new = -0.6, -0.02
    table = [[0.0] * (cols + 1) for _ in range(rows + 1)]
    move = [[0] * (cols + 1) for _ in range(rows + 1)]
    for i in range(1, rows + 1):
        table[i][0], move[i][0] = table[i - 1][0] + skip_old, 1
    for j in range(1, cols + 1):
        table[0][j], move[0][j] = table[0][j - 1] + skip_new, 2
    for i in range(1, rows + 1):
        for j in range(1, cols + 1):
            best, step = table[i - 1][j - 1] + score(old, new, i - 1, j - 1), 0
            if table[i - 1][j] + skip_old > best:
                best, step = table[i - 1][j] + skip_old, 1
            if table[i][j - 1] + skip_new > best:
                best, step = table[i][j - 1] + skip_new, 2
            table[i][j], move[i][j] = best, step
    pairs, i, j = {}, rows, cols
    while i > 0 and j > 0:
        if move[i][j] == 0:
            pairs[j - 1] = (i - 1, round(score(old, new, i - 1, j - 1), 2))
            i, j = i - 1, j - 1
        elif move[i][j] == 1:
            i -= 1
        else:
            j -= 1
    return pairs


def confidence(value):
    if value is None:
        return 'NA-only'
    if value >= 3.0:
        return 'high'
    if value >= 1.5:
        return 'medium'
    return 'low'


def main():
    x64_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / '.cache/bin_x64'
    old_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT.parent / 'Vindictus1.69EN/Vindictus EU/en-EU/bin'
    report = {}
    for dll, (base_vtable, name_slot, prefix) in BUILDS.items():
        digest = sha256(old_dir / dll)
        if not digest.startswith(EXPECTED_169[dll]):
            raise SystemExit(f'1.69 {dll}: unexpected SHA-256 {digest[:16]}')
        new = Image(x64_dir / dll)
        old = Image32(old_dir / dll)
        new_tables = x64_class_vtables(new, base_vtable, name_slot, prefix)
        old_base = old.rtti_vtable(prefix)
        old_tables = {cls: old.rtti_vtable(prefix + cls) for cls in CLASSES}
        common = [cls for cls in CLASSES if cls in new_tables and old_tables.get(cls)]
        old_features = features({c: old.vtable(old_tables[c]) for c in common}, old.vtable(old_base), old.signature)
        new_features = features({c: new.vtable(new_tables[c]) for c in common}, new.vtable(base_vtable),
                                lambda rva, image=new: signature64(image, rva))
        pairs = align(old_features, new_features)
        delia = []
        for slot in DELIA_SLOTS[dll]:
            old_slot, value = pairs.get(slot, (None, None))
            delia.append({'x64': slot, '1.69': old_slot, 'score': value, 'confidence': confidence(value)})
        report[dll] = {
            'x64_sha256': sha256(x64_dir / dll),
            '169_sha256': digest,
            'classes': common,
            'x64_slots': len(new_features[0]),
            '169_slots': len(old_features[0]),
            'mapped': len(pairs),
            'delia': delia,
            'x64_to_169': {str(k): v for k, v in sorted(pairs.items())},
        }
        print(dll, 'classes', len(common), 'mapped', len(pairs), 'of', len(old_features[0]))
        for row in delia:
            print(f"  x64 {row['x64']:>3} -> 1.69 {str(row['1.69']):>4}  {row['confidence']}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(report, indent=1) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
