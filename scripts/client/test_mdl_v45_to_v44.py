import struct
import unittest
from pathlib import Path

import mdl_v45_to_v44 as conv

ROOT = Path(__file__).resolve().parents[2]
NA_PAIR = Path('F:/Games/vindictus/hfsExtracted/models/player/delia/pc_delia_hair_crystal_rose_04.mdl')
LAB_PAIR = ROOT / '.cache/play-client/hfs_extracted/models/player/delia/pc_delia_hair_crystal_rose_04.mdl'
SEQ = 24  # stride of the synthetic sequence/animation descriptions


def synthetic_v45() -> bytes:
    """Header + 260-byte path block + 2 sequences, 2 animations, a cdtexture table and its string."""
    body_at = conv.HEADER + conv.BLOCK
    seq_at, anim_at = body_at, body_at + 2 * SEQ
    cd_at = anim_at + 2 * SEQ
    text_at = cd_at + 4
    data = bytearray(text_at + 8)
    data[:4] = conv.MAGIC
    struct.pack_into('<i', data, 4, conv.VERSION_FROM)
    struct.pack_into('<i', data, conv.LENGTH_AT, len(data))
    path = b'player\\miri\\pc_miri_test.mdl'
    data[conv.HEADER:conv.HEADER + len(path)] = path
    for name, count, index in (('localseq', 2, seq_at), ('localanim', 2, anim_at), ('cdtexture', 1, cd_at)):
        count_at, index_at = conv.COUNT_INDEX[name]
        struct.pack_into('<i', data, count_at, count)
        struct.pack_into('<i', data, index_at, index)
    for k in range(2):
        struct.pack_into('<i', data, seq_at + k * SEQ, -(seq_at + k * SEQ))
        struct.pack_into('<i', data, seq_at + k * SEQ + 4, 1234)  # relative field, unchanged
        struct.pack_into('<i', data, anim_at + k * SEQ, -(anim_at + k * SEQ))
    struct.pack_into('<i', data, cd_at, text_at)
    data[text_at:text_at + 5] = b'miri\0'
    struct.pack_into('<i', data, conv.COUNT_INDEX['surfaceprop'][1], text_at)
    data[388:392] = b'SCAL'
    return bytes(data)


def i32(data: bytes, at: int) -> int:
    return struct.unpack_from('<i', data, at)[0]


class ConvertTest(unittest.TestCase):
    def test_block_is_removed_and_offsets_move_back(self):
        source = synthetic_v45()
        out = conv.convert(source)
        self.assertEqual(len(out), len(source) - conv.BLOCK)
        self.assertEqual(i32(out, 4), conv.VERSION_TO)
        self.assertEqual(i32(out, conv.LENGTH_AT), len(out))
        seq_at = i32(out, conv.COUNT_INDEX['localseq'][1])
        self.assertEqual(seq_at, conv.HEADER)
        for k in range(2):
            self.assertEqual(i32(out, seq_at + k * SEQ), -(seq_at + k * SEQ))
            self.assertEqual(i32(out, seq_at + k * SEQ + 4), 1234)
        anim_at = i32(out, conv.COUNT_INDEX['localanim'][1])
        self.assertEqual(i32(out, anim_at + SEQ), -(anim_at + SEQ))
        cd_at = i32(out, conv.COUNT_INDEX['cdtexture'][1])
        self.assertEqual(out[i32(out, cd_at):i32(out, cd_at) + 4], b'miri')
        surface = i32(out, conv.COUNT_INDEX['surfaceprop'][1])
        self.assertEqual(out[surface:surface + 4], b'miri')
        self.assertEqual(out[388:392], b'SCAL')
        self.assertEqual(out[:4], source[:4])

    def test_rejects_other_versions_and_bad_blocks(self):
        source = bytearray(synthetic_v45())
        struct.pack_into('<i', source, 4, 44)
        with self.assertRaises(conv.ConversionError):
            conv.convert(bytes(source))
        source = bytearray(synthetic_v45())
        source[conv.HEADER] = 0
        with self.assertRaises(conv.ConversionError):
            conv.convert(bytes(source))
        source = bytearray(synthetic_v45())
        struct.pack_into('<i', source, conv.LENGTH_AT, len(source) + 1)
        with self.assertRaises(conv.ConversionError):
            conv.convert(bytes(source))

    def test_rejects_inconsistent_descriptions(self):
        source = bytearray(synthetic_v45())
        seq_at = i32(source, conv.COUNT_INDEX['localseq'][1])
        struct.pack_into('<i', source, seq_at + SEQ, 7)
        with self.assertRaises(conv.ConversionError):
            conv.convert(bytes(source))


@unittest.skipUnless(NA_PAIR.is_file() and LAB_PAIR.is_file(), 'needs the NA extraction and the lab copy')
class MeasuredPairTest(unittest.TestCase):
    def test_header_matches_the_169_model(self):
        out = conv.convert(NA_PAIR.read_bytes())
        lab = LAB_PAIR.read_bytes()
        self.assertEqual(len(out), len(lab))
        for count_at, index_at in conv.COUNT_INDEX.values():
            self.assertEqual(i32(out, index_at), i32(lab, index_at))
            if count_at is not None:
                self.assertEqual(i32(out, count_at), i32(lab, count_at))
        self.assertEqual(out[368:408], lab[368:408])


if __name__ == '__main__':
    unittest.main()
