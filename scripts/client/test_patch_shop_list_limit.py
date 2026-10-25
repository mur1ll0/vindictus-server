import hashlib
import unittest

from patch_shop_list_limit import OFFSETS, ORIGINAL, PATCHED, patched_bytes
from prepare_lab_client import PreparationError


def image(site: bytes) -> bytes:
    data = bytearray(0x478800)
    for o in OFFSETS:
        data[o:o + len(site)] = site
    return bytes(data)


class PatchShopListLimitTest(unittest.TestCase):
    def test_patches_both_download_limits(self):
        data = image(ORIGINAL)
        out, already = patched_bytes(data, hashlib.sha256(data).hexdigest())
        self.assertFalse(already)
        self.assertEqual([out[o:o + 5] for o in OFFSETS], [PATCHED, PATCHED])
        self.assertEqual(sum(a != b for a, b in zip(data, out)), 4)

    def test_already_patched_copy_is_unchanged(self):
        data = image(PATCHED)
        self.assertEqual(patched_bytes(data, 'unused'), (data, True))

    def test_other_build_is_refused(self):
        data = image(ORIGINAL)
        with self.assertRaises(PreparationError):
            patched_bytes(data, '0' * 64)
        with self.assertRaises(PreparationError):
            patched_bytes(image(b'\x90' * 5), hashlib.sha256(image(b'\x90' * 5)).hexdigest())


if __name__ == '__main__':
    unittest.main()
