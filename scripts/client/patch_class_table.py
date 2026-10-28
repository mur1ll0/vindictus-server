"""Extend the BaseCharacter class tables of COPIES of client.dll and server.dll. Dry-run by default.

The 1.69 EN client knows ten classes through fixed tables (docs/future/new-classes.md 8.2):
client.dll pairs {name, body type} at 0x38cafd50 read by 0x3879ec50/0x3879ec60 (`cmp eax, 0xa`);
server.dll the same pairs at 0x40d0cdf8 (0x40755b40/0x40755b50) and a name list at 0x40d140dc
read by 0x408b3980 (`cmp eax, 9`). The NA client keeps the same pair layout with 28 classes
(client.dll 0x3958ad38), the source of CLASSES below.

The patch appends a `.hcls` section with room for MAX_CLASSES entries, points the accessor
instructions at it and raises their bounds to ACTIVE_CLASSES. The relocation directory moves
into the new section with one more block for its absolute pointers. The original client stays
read-only and the unpatched copy is kept next to each patched file.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import struct
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

SECTION = b'.hcls\0\0\0'
MAX_CLASSES = 32
# BaseCharacter order and body type of the NA client table (1 male, 2 female, 3 giant, 4 tall).
CLASSES = (('Lethita', 1), ('Fiona', 2), ('Evy', 2), ('Kalok', 3), ('Kay', 1), ('Vella', 2), ('Hurk', 4),
           ('Lynn', 2), ('Arisha', 2), ('Hagie', 1), ('Delia', 2), ('Miri', 2), ('Grimden', 4), ('Miul', 2),
           ('Bel', 2), ('Lethor', 2), ('Kael', 1), ('Tessa', 2), ('Danah', 2), ('Letty', 2), ('Latiya', 2),
           ('Czern', 2), ('Achel', 1), ('Sou', 1), ('Sanyaa', 2), ('Neamhain', 2), ('Calia', 2), ('Lumielle', 2))
# Classes the accessors accept: the ten of the 1.69 client and Delia.
ACTIVE_CLASSES = 11
PAIRS, NAMES, STRINGS = 0, 0x100, 0x180

# Per DLL: (VA, opcode prefix, original table VA, field) for pair loads, (VA, original bound,
# bound minus count) for `cmp eax, imm8`, and (VA, opcode prefix, original VA) for name loads.
SPECS = {
    'client.dll': {
        'pairs': ((0x3879ec54, '8d04c5', 0x38cafd50, 0), (0x3879ec6d, '8b04c5', 0x38cafd54, 4)),
        'bounds': ((0x3879ec68, 10, 0),),
        'names': (),
    },
    'server.dll': {
        'pairs': ((0x40755b44, '8d04c5', 0x40d0cdf8, 0), (0x40755b5d, '8b04c5', 0x40d0cdfc, 4)),
        'bounds': ((0x40755b58, 10, 0), (0x408b3984, 9, -1)),
        'names': ((0x408b3989, '8b0485', 0x40d140dc),),
    },
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def align(value: int, unit: int) -> int:
    return (value + unit - 1) // unit * unit


class Image:
    """The few PE32 fields the patch reads and writes."""

    def __init__(self, data: bytes):
        self.data = bytearray(data)
        if data[:2] != b'MZ':
            raise PreparationError('Not a PE image.')
        self.pe = struct.unpack_from('<I', data, 0x3c)[0]
        if data[self.pe:self.pe + 4] != b'PE\0\0' or struct.unpack_from('<H', data, self.pe + 24)[0] != 0x10b:
            raise PreparationError('Not a PE32 image.')
        self.opt = self.pe + 24
        self.count = struct.unpack_from('<H', data, self.pe + 6)[0]
        self.table = self.opt + struct.unpack_from('<H', data, self.pe + 20)[0]
        self.base = struct.unpack_from('<I', data, self.opt + 28)[0]
        self.section_align, self.file_align = struct.unpack_from('<II', data, self.opt + 32)

    def sections(self) -> list[tuple[bytes, int, int, int, int]]:
        return [struct.unpack_from('<8sIIII', self.data, self.table + 40 * i) for i in range(self.count)]

    def offset(self, va: int) -> int:
        rva = va - self.base
        for _, vsize, vaddr, rsize, raw in self.sections():
            if vaddr <= rva < vaddr + min(vsize or rsize, rsize):
                return raw + rva - vaddr
        raise PreparationError(f'VA {va:#x} is outside the image.')

    def directory(self, index: int) -> tuple[int, int]:
        return struct.unpack_from('<II', self.data, self.opt + 96 + 8 * index)

    def set_directory(self, index: int, rva: int, size: int) -> None:
        struct.pack_into('<II', self.data, self.opt + 96 + 8 * index, rva, size)


def section_body(base: int, rva: int, relocs: bytes) -> tuple[bytes, int]:
    """Pairs, name pointers, strings and the moved relocation directory; returns its offset."""
    body = bytearray(STRINGS)
    for i, (name, kind) in enumerate(CLASSES):
        at = base + rva + len(body)
        struct.pack_into('<II', body, PAIRS + 8 * i, at, kind)
        struct.pack_into('<I', body, NAMES + 4 * i, at)
        body += name.encode('ascii') + b'\0'
    body += b'\0' * (align(len(body), 4) - len(body))
    reloc_at = len(body)
    entries = [PAIRS + 8 * i for i in range(len(CLASSES))] + [NAMES + 4 * i for i in range(len(CLASSES))]
    block = struct.pack('<II', rva, 8 + 2 * len(entries)) + b''.join(struct.pack('<H', 0x3000 | e) for e in entries)
    body += relocs + block
    return bytes(body), reloc_at


def patched_bytes(data: bytes, spec: dict) -> tuple[bytes, bool]:
    """Return the patched image and whether it was already patched."""
    image = Image(data)
    existing = [s for s in image.sections() if s[0] == SECTION]
    sites = []
    for va, prefix, old, field in spec['pairs']:
        sites.append((va, bytes.fromhex(prefix) + struct.pack('<I', old), field, 'pair', prefix))
    for va, prefix, old in spec['names']:
        sites.append((va, bytes.fromhex(prefix) + struct.pack('<I', old), NAMES, 'name', prefix))
    if existing:
        _, _, vaddr, _, _ = existing[0]
        table = image.base + vaddr
        expected = [bytes.fromhex(p) + struct.pack('<I', table + f) for _, _, f, _, p in sites]
        expected += [bytes((0x83, 0xf8, ACTIVE_CLASSES + delta)) for _, _, delta in spec['bounds']]
        found = [bytes(image.data[image.offset(va):image.offset(va) + 7]) for va, *_ in sites]
        found += [bytes(image.data[image.offset(va):image.offset(va) + 3]) for va, _, _ in spec['bounds']]
        if found != expected:
            raise PreparationError('The .hcls section exists but the accessors differ; restore the backup.')
        return data, True
    for va, original, *_ in sites:
        if image.data[image.offset(va):image.offset(va) + 7] != original:
            raise PreparationError(f'Unexpected bytes at {va:#x}; nothing is patched.')
    for va, bound, _ in spec['bounds']:
        if image.data[image.offset(va):image.offset(va) + 3] != bytes((0x83, 0xf8, bound)):
            raise PreparationError(f'Unexpected bound at {va:#x}; nothing is patched.')
    if image.table + 40 * (image.count + 1) > min(s[4] for s in image.sections()):
        raise PreparationError('No room for another section header.')
    rva = align(max(v + max(vs, rs) for _, vs, v, rs, _ in image.sections()), image.section_align)
    reloc_rva, reloc_size = image.directory(5)
    relocs = bytes(image.data[image.offset(image.base + reloc_rva):image.offset(image.base + reloc_rva) + reloc_size])
    body, reloc_at = section_body(image.base, rva, relocs)
    raw = align(len(image.data), image.file_align)
    image.data += b'\0' * (raw - len(image.data)) + body + b'\0' * (align(len(body), image.file_align) - len(body))
    struct.pack_into('<8sIIIIIIHHI', image.data, image.table + 40 * image.count, SECTION, len(body), rva,
                     align(len(body), image.file_align), raw, 0, 0, 0, 0, 0x40000040)
    image.count += 1
    struct.pack_into('<H', image.data, image.pe + 6, image.count)
    struct.pack_into('<I', image.data, image.opt + 56, align(rva + len(body), image.section_align))
    struct.pack_into('<I', image.data, image.opt + 64, 0)  # checksum
    image.set_directory(4, 0, 0)  # the signature no longer covers the image
    image.set_directory(5, rva + reloc_at, len(relocs) + 8 + 4 * len(CLASSES))
    table = image.base + rva
    for va, original, field, _, prefix in sites:
        o = image.offset(va)
        image.data[o:o + 7] = bytes.fromhex(prefix) + struct.pack('<I', table + field)
    for va, _, delta in spec['bounds']:
        image.data[image.offset(va) + 2] = ACTIVE_CLASSES + delta
    return bytes(image.data), False


def patch(client_copy: Path, apply: bool = False) -> dict:
    plan = {'mode': 'apply' if apply else 'dry-run', 'active_classes': ACTIVE_CLASSES, 'dlls': []}
    for name, spec in SPECS.items():
        dll = reject_original(client_copy / 'bin' / name)
        if not dll.is_file():
            raise PreparationError(f'The client copy must contain bin/{name}.')
        data = dll.read_bytes()
        image, already = patched_bytes(data, spec)
        backup = dll.with_name(f'{name}.{digest(data)[:16]}.pre-hcls.bak')
        plan['dlls'].append({'dll': str(dll), 'already_patched': already, 'before_sha256': digest(data),
                             'after_sha256': digest(image), 'backup': None if already else str(backup)})
        if apply and not already:
            if not backup.exists():
                backup.write_bytes(data)
            partial = dll.with_suffix('.partial')
            partial.write_bytes(image)
            os.replace(partial, dll)
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(patch(args.client_copy, args.apply)))
    except PreparationError as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
