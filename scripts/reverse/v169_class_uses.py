"""Where the 1.69 lab client.dll and server.dll read the player class (read only).

The class reaches native code as the networked `m_iClass` (client +0x18ec, server +0x1400; the
server sends it with log2(10)+1 = 4 bits). This script visits every direct load of that field and
every call to its getters, follows the loaded register for a few instructions and records what the
code does with it: a bound (`cmp reg, imm`), a jump table, or an unbounded table index. It also
records the per-class code anchored on the Hagie strings (the last 1.69 class). Output:
docs/evidence/client/v169-class-uses.json (local, like the rest of docs/evidence/client) and a
summary on stdout.

Usage: python -I scripts/reverse/v169_class_uses.py [client bin dir]
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

OUTPUT = ROOT / 'docs/evidence/client/v169-class-uses.json'
# Lab copies with the .hcls class tables (docs/future/new-classes.md 8.9).
EXPECTED = {'client.dll': 'a9591d3a44a2b538', 'server.dll': 'af43431e78666783'}
FIELD = {'client.dll': 0x18EC, 'server.dll': 0x1400}
REGISTERS = ['eax', 'ecx', 'edx', 'ebx', 'esp', 'ebp', 'esi', 'edi']
WINDOW = 18
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)


class Image:
    def __init__(self, path):
        pe = pefile.PE(str(path), fast_load=True)
        self.base = pe.OPTIONAL_HEADER.ImageBase
        self.data = bytes(pe.get_memory_mapped_image())
        text = next(s for s in pe.sections if s.Name.startswith(b'.text'))
        self.text = (text.VirtualAddress, text.VirtualAddress + text.Misc_VirtualSize)

    def disasm(self, va, size):
        rva = va - self.base
        return list(md.disasm(self.data[rva:rva + size], va))

    def calls_to(self, va):
        lo, hi = self.text
        out = []
        for match in re.finditer(rb'\xe8', self.data[lo:hi]):
            rva = lo + match.start()
            if rva + 5 + struct.unpack_from('<i', self.data, rva + 1)[0] == va - self.base:
                out.append(self.base + rva)
        return out

    def field_loads(self, offset):
        """`mov reg, [reg + offset]` (8B /r, disp32, no SIB)."""
        lo, hi = self.text
        out = []
        for match in re.finditer(rb'\x8b[\x80-\xbf]' + struct.pack('<I', offset), self.data[lo:hi]):
            rva = lo + match.start()
            if (self.data[rva + 1] & 7) != 4:
                out.append((self.base + rva, REGISTERS[(self.data[rva + 1] >> 3) & 7]))
        return out

    def getters(self, offset):
        """`mov eax, [ecx + offset]; ret` after padding."""
        pattern = rb'\xcc\x8b\x81' + struct.pack('<I', offset) + rb'\xc3'
        return [self.base + m.start() + 1 for m in re.finditer(pattern, self.data)]


def follow(image, va, register):
    """What the code does with `register` in the instructions after `va`."""
    tracked, uses = {register}, []
    for instruction in image.disasm(va, 0x60)[1:1 + WINDOW]:
        text, op = instruction.mnemonic, instruction.op_str
        bound = re.match(r'(\w+), (0x[0-9a-f]+|\d+)$', op)
        if text == 'cmp' and bound and bound.group(1) in tracked and int(bound.group(2), 0) < 0x40:
            uses.append(['bound', hex(instruction.address), int(bound.group(2), 0)])
        for reg, scale, table in re.findall(r'\[(\w+)\*(\d) \+ (0x[0-9a-f]+)\]', op):
            if reg in tracked:
                uses.append(['switch' if text == 'jmp' else 'table', hex(instruction.address), table, int(scale)])
        copy = re.match(r'(\w+), (\w+)$', op)
        if text in ('mov', 'movzx') and copy:
            if copy.group(2) in tracked:
                tracked.add(copy.group(1))
            elif copy.group(1) in tracked:
                tracked.discard(copy.group(1))
        if text == 'ret' or (text == 'jmp' and '[' not in op) or not tracked:
            break
    return uses


def hagie_code(image):
    """Code anchored on strings naming Hagie, with the last class-sized bound before each use."""
    out = []
    for match in re.finditer(rb'\x00([\x20-\x7e]{0,120}[Hh]agie[\x20-\x7e]{0,120})\x00', image.data):
        text = match.group(1).decode()
        pattern = struct.pack('<I', image.base + match.start(1))
        position = image.data.find(pattern, image.text[0])
        while 0 <= position < image.text[1]:
            start = image.data.rfind(b'\xcc\xcc', position - 0x400, position) + 2
            bound = None
            for instruction in image.disasm(image.base + start, position - start + 4):
                found = re.match(r'(\w+), (0x[0-9a-f]+|\d+)$', instruction.op_str)
                if instruction.mnemonic == 'cmp' and found and int(found.group(2), 0) in (8, 9, 10, 11):
                    bound = [hex(instruction.address), int(found.group(2), 0)]
            out.append({'ref': hex(image.base + position), 'string': text, 'bound': bound})
            position = image.data.find(pattern, position + 1)
    return out


def main():
    directory = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / '.cache/play-client/bin'
    report = {}
    for name, prefix in EXPECTED.items():
        path = directory / name
        # After patch_class_routing.py the measured .hcls copy is its pre-routing backup.
        backup = directory / f'{name}.{prefix}.pre-routing.bak'
        if backup.is_file():
            path = backup
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if not digest.startswith(prefix):
            raise SystemExit(f'{name}: unexpected sha256 {digest[:16]} (expected {prefix})')
        image = Image(path)
        getters = image.getters(FIELD[name])
        sites = [{'site': hex(va), 'via': 'load', 'uses': follow(image, va, register)}
                 for va, register in image.field_loads(FIELD[name])]
        sites += [{'site': hex(va), 'via': 'getter', 'uses': follow(image, va, 'eax')}
                  for getter in getters for va in image.calls_to(getter)]
        anchored = hagie_code(image)
        report[name] = {
            'sha256': digest,
            'field': hex(FIELD[name]),
            'getters': [hex(g) for g in getters],
            'reads': len(sites),
            'bounded': [s for s in sites if any(u[0] == 'bound' for u in s['uses'])],
            'indexed': [s for s in sites if any(u[0] in ('switch', 'table') for u in s['uses'])],
            'hagie_code': anchored,
        }
        bounds = sorted({u[2] for s in report[name]['bounded'] for u in s['uses'] if u[0] == 'bound'})
        print(name, digest[:16], 'getters', len(getters), 'reads', len(sites),
              'bounded', len(report[name]['bounded']), 'bound values', bounds,
              'indexed', len(report[name]['indexed']), 'hagie refs', len(anchored),
              'with class bound', sum(1 for s in anchored if s['bound']))
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(report, indent=1), encoding='utf-8')
    print('wrote', OUTPUT.relative_to(ROOT))


if __name__ == '__main__':
    main()
