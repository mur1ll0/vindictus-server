"""recover_mss32.py and apply_delia_creation_movie.py: the client files the copy used to get by hand."""
import hashlib
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

import apply_delia_creation_movie as movie
import recover_mss32 as mss
from prepare_lab_client import WORKSPACE, PreparationError

ORIGINAL = WORKSPACE / 'Vindictus1.69EN/Vindictus EU/en-EU'
KEYSTREAM = Path(__file__).resolve().parents[2] / '.cache/play-client/hfs-keystream.bin'


class Mss32Test(unittest.TestCase):
    def test_a_volume_without_the_header_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            volume = Path(tmp) / 'v.z03'
            body = zlib.compress(b'MZ' + bytes(100))[2:-4]
            head = mss.HEADER.pack(b'PK\x03\x04', 45, 0x800, 8, 0, 0, zlib.crc32(b'x'), len(body), 102, len(mss.NAME), 0)
            volume.write_bytes(bytes(16) + head + mss.NAME + body)
            with self.assertRaises(PreparationError):
                mss.recover(volume, 16)
            with self.assertRaises(PreparationError):
                mss.recover(volume, 0)

    @unittest.skipUnless(mss.VOLUME.is_file(), 'needs Vindictus1.69EN/Vindictus.z03 (read only)')
    def test_the_archive_holds_the_good_dll(self):
        data = mss.recover()
        self.assertEqual(hashlib.sha256(data).hexdigest(), mss.SHA256)
        pe = struct.unpack_from('<I', data, 0x3C)[0]
        self.assertEqual(data[pe:pe + 4], b'PE\0\0')
        if (ORIGINAL / 'Mss32.dll').is_file():
            self.assertEqual(hashlib.sha256((ORIGINAL / 'Mss32.dll').read_bytes()).hexdigest(), mss.BROKEN_SHA256)
        with tempfile.TemporaryDirectory() as tmp:
            plan = mss.apply(Path(tmp))
            self.assertFalse((Path(tmp) / 'Mss32.dll').exists())
            self.assertTrue(plan['changed'])
            mss.apply(Path(tmp), apply_changes=True)
            self.assertFalse(mss.apply(Path(tmp), apply_changes=True)['changed'])
        with self.assertRaises(PreparationError):
            mss.apply(ORIGINAL.parent.parent, apply_changes=True)


class DeliaMovieTest(unittest.TestCase):
    def test_edits_are_same_size_and_the_source_is_checked(self):
        for at, old, new, step in movie.MOVIE_EDITS:
            self.assertEqual(len(old), len(new), step)
        with self.assertRaises(PreparationError):
            movie.edited_movie(b'GFX' + bytes(0x10000))

    @unittest.skipUnless((ORIGINAL / movie.PACK).is_file() and KEYSTREAM.is_file(),
                         'needs the 1.69 EU original and the lab keystream')
    def test_the_renamed_pack_gives_the_lab_movie_and_images(self):
        import hfs
        out = movie.files((ORIGINAL / movie.PACK).read_bytes(), hfs.load_keystream(KEYSTREAM))
        self.assertEqual(sorted(out), sorted(['scaleform/' + movie.MOVIE] + ['scaleform/' + n for n in movie.IMAGES]))
        self.assertEqual(hashlib.sha256(out['scaleform/' + movie.MOVIE]).hexdigest(), movie.MOVIE_RESULT)
        for name, sha in movie.IMAGES.items():
            loose = ORIGINAL / 'scaleform' / name
            self.assertNotEqual(hashlib.sha256(loose.read_bytes()).hexdigest(), sha)  # the loose one is older


if __name__ == '__main__':
    unittest.main()
