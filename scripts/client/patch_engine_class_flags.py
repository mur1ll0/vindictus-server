"""Let items allow the extra classes (10 Delia, 11 Miri) in a COPY of engine.dll. Dry-run by default.

engine.dll reads ItemClassInfo.ClassRestriction (an int mask) and unpacks it into one bool per
class at item +0x198 (setter 0x42245110, its only caller 0x422be9f8). The record has room for ten
classes only (+0x198..+0x1a1): the loader writes Indestructible to +0x1a2 right after the setter
(0x422bea4a), so every item denies the extra classes and the client shows their items as "Cannot
be equipped" and red (client tests D8c, D22). Each extra class gets a padding byte after UseInQuest
(+0x1cc), which nothing in engine.dll or server.dll reads or writes: CLASS_FLAGS (Delia +0x1cd,
Miri +0x1ce; patch_class_routing.py reads them).
The setter's tail (bits 8 and 9, 0x42245170..0x4224518f) has no room for more than one extra bit,
so it becomes a jump to a stub in a new executable section (.hflg) that sets bits 8 and 9 as the
original does and each extra class's byte by setne (0 or 1), then returns; the function's early exit
(`je` at 0x42245116, mask 0) moves to a `ret 4` kept in the tail. The stub holds no absolute address
(no relocation); the jump is relative. Always applied from the original engine.dll (the
*.pre-classflags.bak backup when the copy is already patched).
"""
from __future__ import annotations
import argparse
import json
import os
import struct
import sys
from pathlib import Path

import class_layers
from patch_class_table import Image, align, digest
from prepare_lab_client import PreparationError, reject_original

EXPECTED = 'a3add9742f7fa03c'
TAIL = 0x42245170
# test edx,0x100; je; mov [ecx+0x1a0],al; test edx,0x200; je; mov [ecx+0x1a1],al; ret 4; int3
OLD_TAIL = bytes.fromhex('f7c20001000074068881a0010000f7c20002000074068881a1010000c20400cc')
# Item byte of each extra class (padding after UseInQuest +0x1cc), by class number.
CLASS_FLAGS = {10: 0x1CD, 11: 0x1CE}
DELIA_FLAG = CLASS_FLAGS[10]


def flags_of(classes: tuple[str, ...] = class_layers.LAYERS) -> dict[int, int]:
    """CLASS_FLAGS of a class layer (class_layers): v2 sets the Delia's byte only."""
    wanted = class_layers.indices(class_layers.checked(classes, allow_none=False))
    return {number: flag for number, flag in CLASS_FLAGS.items() if number in wanted}
EXIT = 0x42245116
OLD_EXIT = bytes.fromhex('7474')  # je 0x4224518c (mask 0)
SECTION = b'.hflg\0\0\0'
CODE = 0x60000020  # code, execute, read


def stub(flags: dict[int, int] = CLASS_FLAGS) -> bytes:
    """Bits 8 and 9 as the original tail, then each extra class's byte; ret 4. al is 1 (0x42245118)."""
    code = bytes.fromhex('f7c200010000' '7406' '8881a0010000'    # bit 8 -> [ecx+0x1a0]
                         'f7c200020000' '7406' '8881a1010000')   # bit 9 -> [ecx+0x1a1]
    for number, flag in sorted(flags.items()):
        code += bytes.fromhex('f7c2') + struct.pack('<I', 1 << number)  # test edx, 1 << class
        code += bytes.fromhex('0f9581') + struct.pack('<I', flag)       # setne [ecx + flag]
    return code + bytes.fromhex('c20400')


def new_tail(stub_va: int) -> bytes:
    """jmp stub; ret 4 (the early exit's target); int3 padding."""
    jump = b'\xe9' + struct.pack('<i', stub_va - (TAIL + 5))
    return (jump + bytes.fromhex('c20400')).ljust(len(OLD_TAIL), b'\xcc')


def plan(data: bytes, flags: dict[int, int] = CLASS_FLAGS) -> tuple[bytes, dict]:
    image = Image(data)
    if any(s[0] == SECTION for s in image.sections()):
        raise PreparationError('engine.dll already has the .hflg section; start from the original.')
    for va, old in ((TAIL, OLD_TAIL), (EXIT, OLD_EXIT)):
        o = image.offset(va)
        if bytes(image.data[o:o + len(old)]) != old:
            raise PreparationError(f'Unexpected bytes at {va:#x}; nothing is patched.')
    if image.table + 40 * (image.count + 1) > min(s[4] for s in image.sections()):
        raise PreparationError('No room for another section header.')
    body = stub(flags)
    rva = align(max(v + max(vs, rs) for _, vs, v, rs, _ in image.sections()), image.section_align)
    raw = align(len(image.data), image.file_align)
    image.data += b'\0' * (raw - len(image.data)) + body + b'\0' * (align(len(body), image.file_align) - len(body))
    struct.pack_into('<8sIIIIIIHHI', image.data, image.table + 40 * image.count, SECTION, len(body), rva,
                     align(len(body), image.file_align), raw, 0, 0, 0, 0, CODE)
    image.count += 1
    struct.pack_into('<H', image.data, image.pe + 6, image.count)
    struct.pack_into('<I', image.data, image.opt + 56, align(rva + len(body), image.section_align))
    struct.pack_into('<I', image.data, image.opt + 64, 0)  # checksum
    image.set_directory(4, 0, 0)  # the signature no longer covers the image
    stub_va = image.base + rva
    ret = TAIL + 5
    tail = new_tail(stub_va)
    new_exit = bytes((0x74, ret - (EXIT + 2)))
    for va, new in ((TAIL, tail), (EXIT, new_exit)):
        o = image.offset(va)
        image.data[o:o + len(new)] = new
    return bytes(image.data), {
        'tail': hex(TAIL), 'stub': hex(stub_va), 'ret': hex(ret), 'exit': f'{EXIT:#x} -> {ret:#x}',
        'flags': '+0x198..+0x1a1 (classes 0-9), ' + ', '.join(f'{f:#x} (class {c})' for c, f in sorted(flags.items()))}


def original(dll: Path) -> bytes:
    """The unpatched engine.dll: the copy itself, or its backup once patched."""
    data = dll.read_bytes()
    if digest(data).startswith(EXPECTED):
        return data
    backup = dll.with_name(f'engine.dll.{EXPECTED}.pre-classflags.bak')
    if backup.is_file() and digest(backup.read_bytes()).startswith(EXPECTED):
        return backup.read_bytes()
    raise PreparationError(f'engine.dll: expected {EXPECTED}… or its backup, found {digest(data)[:16]}.')


def patch(client_copy: Path, apply: bool = False, classes: tuple[str, ...] = class_layers.LAYERS) -> dict:
    dll = reject_original(client_copy / 'bin' / 'engine.dll')
    current = dll.read_bytes()
    data = original(dll)
    image, report = plan(data, flags_of(classes))
    backup = dll.with_name(f'engine.dll.{digest(data)[:16]}.pre-classflags.bak')
    result = {'mode': 'apply' if apply else 'dry-run', 'dll': str(dll), 'before_sha256': digest(current),
              'original_sha256': digest(data), 'after_sha256': digest(image), 'backup': str(backup), **report}
    if apply:
        if not backup.exists():
            backup.write_bytes(data)
        partial = dll.with_suffix('.partial')
        partial.write_bytes(image)
        os.replace(partial, dll)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    class_layers.add_argument(parser, allow_none=False)
    args = parser.parse_args()
    try:
        print(json.dumps(patch(args.client_copy, args.apply, args.classes), indent=1))
    except (PreparationError, ValueError) as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
