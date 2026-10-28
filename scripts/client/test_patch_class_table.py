import struct
import unittest

from patch_class_table import ACTIVE_CLASSES, CLASSES, NAMES, SECTION, Image, patched_bytes
from prepare_lab_client import PreparationError

BASE = 0x10000000
SPEC = {
    'pairs': ((0x10001010, '8d04c5', 0x10005000, 0), (0x10001018, '8b04c5', 0x10005004, 4)),
    'bounds': ((0x10001020, 10, 0), (0x10001024, 9, -1)),
    'names': ((0x10001030, '8b0485', 0x10006000),),
}
RELOCS = struct.pack('<IIHH', 0x1000, 12, 0x3013, 0x301b)


def image(text_patch: dict[int, bytes] | None = None) -> bytes:
    data = bytearray(0x800)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x40)
    data[0x40:0x44] = b'PE\0\0'
    struct.pack_into('<HHIIIHH', data, 0x44, 0x14c, 2, 0, 0, 0, 224, 0x2102)
    opt = 0x58
    struct.pack_into('<H', data, opt, 0x10b)
    struct.pack_into('<I', data, opt + 28, BASE)
    struct.pack_into('<II', data, opt + 32, 0x1000, 0x200)
    struct.pack_into('<I', data, opt + 56, 0x3000)
    struct.pack_into('<I', data, opt + 60, 0x400)
    struct.pack_into('<I', data, opt + 92, 16)
    struct.pack_into('<II', data, opt + 96 + 8 * 5, 0x2000, len(RELOCS))
    table = opt + 224
    struct.pack_into('<8sIIIIIIHHI', data, table, b'.text\0\0\0', 0x200, 0x1000, 0x200, 0x400, 0, 0, 0, 0, 0x60000020)
    struct.pack_into('<8sIIIIIIHHI', data, table + 40, b'.reloc\0\0', len(RELOCS), 0x2000, 0x200, 0x600, 0, 0, 0, 0,
                     0x42000040)
    sites = {0x10: bytes.fromhex('8d04c5') + struct.pack('<I', 0x10005000),
             0x18: bytes.fromhex('8b04c5') + struct.pack('<I', 0x10005004),
             0x20: bytes((0x83, 0xf8, 10)), 0x24: bytes((0x83, 0xf8, 9)),
             0x30: bytes.fromhex('8b0485') + struct.pack('<I', 0x10006000)}
    sites.update(text_patch or {})
    for offset, code in sites.items():
        data[0x400 + offset:0x400 + offset + len(code)] = code
    data[0x600:0x600 + len(RELOCS)] = RELOCS
    return bytes(data)


class PatchClassTableTest(unittest.TestCase):
    def test_accessors_point_at_the_new_section_with_the_raised_bounds(self):
        out, already = patched_bytes(image(), SPEC)
        self.assertFalse(already)
        pe = Image(out)
        section = [s for s in pe.sections() if s[0] == SECTION]
        self.assertEqual(len(section), 1)
        _, vsize, rva, _, raw = section[0]
        self.assertEqual(rva, 0x3000)
        table = BASE + rva
        self.assertEqual(out[0x410:0x417], bytes.fromhex('8d04c5') + struct.pack('<I', table))
        self.assertEqual(out[0x418:0x41f], bytes.fromhex('8b04c5') + struct.pack('<I', table + 4))
        self.assertEqual(out[0x430:0x437], bytes.fromhex('8b0485') + struct.pack('<I', table + NAMES))
        self.assertEqual(out[0x422], ACTIVE_CLASSES)
        self.assertEqual(out[0x426], ACTIVE_CLASSES - 1)
        # Delia (BaseCharacter 10): female body, its name through both tables.
        name_va, kind = struct.unpack_from('<II', out, raw + 8 * 10)
        self.assertEqual(kind, 2)
        self.assertEqual(struct.unpack_from('<I', out, raw + NAMES + 4 * 10)[0], name_va)
        at = raw + name_va - table
        self.assertEqual(out[at:out.index(b'\0', at)], b'Delia')
        self.assertEqual(Image(out).sections()[0], Image(image()).sections()[0])

    def test_relocation_directory_keeps_the_old_blocks_and_covers_the_new_pointers(self):
        out, _ = patched_bytes(image(), SPEC)
        pe = Image(out)
        rva, size = pe.directory(5)
        start = pe.offset(BASE + rva)
        blob = out[start:start + size]
        self.assertTrue(blob.startswith(RELOCS))
        page, length = struct.unpack_from('<II', blob, len(RELOCS))
        self.assertEqual((page, length, len(blob)), (0x3000, 8 + 4 * len(CLASSES), len(RELOCS) + length))
        self.assertEqual(pe.directory(4), (0, 0))

    def test_already_patched_copy_is_unchanged(self):
        out, _ = patched_bytes(image(), SPEC)
        self.assertEqual(patched_bytes(out, SPEC), (out, True))

    def test_other_build_is_refused(self):
        with self.assertRaises(PreparationError):
            patched_bytes(image({0x20: bytes((0x83, 0xf8, 12))}), SPEC)
        with self.assertRaises(PreparationError):
            patched_bytes(image({0x10: b'\x90' * 7}), SPEC)


if __name__ == '__main__':
    unittest.main()
