import io
import struct
import unittest

import gfx_patch as g


def tag(code, body):
    return struct.pack('<HI', code << 6 | 0x3F, len(body)) + body


def movie():
    """A GFx movie with one atlas (id 1), a sub-image (2), a shape (3) filled with it, and a class sprite (4)
    whose 'Delia' frame places the shape and shows two frames."""
    atlas = tag(1009, struct.pack('<HHHHH', 1, 9, 14, 1024, 512) + b'\0' + bytes((7,)) + b'atl.dds')
    sub = tag(1008, struct.pack('<6H', 2, 1, 382, 145, 483, 199))
    shape = tag(2, struct.pack('<H', 3) + bytes(8) + b'\x01\x43' + struct.pack('<H', 2) + bytes(6))
    frame = (tag(43, b'Delia\0') + tag(26, bytes((0x06,)) + struct.pack('<HH', 1, 3)) + tag(1, b'') + tag(1, b''))
    lethita = tag(43, b'Lethita\0') + tag(26, bytes((0x06,)) + struct.pack('<HH', 1, 3)) + tag(1, b'')
    sprite = tag(39, struct.pack('<HH', 4, 3) + lethita + frame + tag(0, b''))
    body = bytes((0x08, 0x00)) + struct.pack('<HH', 0, 1) + atlas + sub + shape + sprite + tag(0, b'')
    return b'GFX\x09' + struct.pack('<I', 8 + len(body)) + body


class GfxPatchTest(unittest.TestCase):
    def test_a_class_frame_copies_the_donor_onto_a_new_atlas(self):
        src = movie()
        out = g.with_class_frame(src, 'Miri', 'portrait.dds')
        self.assertEqual(struct.unpack_from('<I', out, 4)[0], len(out))
        top = list(g._tags(out, 8 + 2 + 4, len(out)))
        atlases = [out[b:b + l] for c, _, b, l in top if c == 1009]
        self.assertEqual(len(atlases), 2)
        self.assertEqual(struct.unpack_from('<HHHHH', atlases[1]), (5, 9, 14, *g.ATLAS_SIZE))
        self.assertTrue(atlases[1].endswith(bytes((12,)) + b'portrait.dds'))
        subs = [struct.unpack_from('<6H', out, b) for c, _, b, l in top if c == 1008]
        self.assertEqual(subs[1], (6, 5, 0, 0, *g.PORTRAIT_SIZE))
        shapes = [out[b:b + l] for c, _, b, l in top if c == 2]
        self.assertEqual(struct.unpack_from('<H', shapes[1])[0], 7)
        self.assertEqual(struct.unpack_from('<H', shapes[1], 12)[0], 6)
        sprite = next((b, l) for c, _, b, l in top if c == 39)
        self.assertEqual(struct.unpack_from('<HH', out, sprite[0]), (4, 5))  # two more frames
        inner = list(g._tags(out, sprite[0] + 4, sprite[0] + sprite[1]))
        labels = [out[b:out.index(b'\0', b)] for c, _, b, l in inner if c == 43]
        self.assertEqual(labels, [b'Lethita', b'Delia', b'Miri'])
        placed = [struct.unpack_from('<H', out, b + 3)[0] for c, _, b, l in inner if c == 26]
        self.assertEqual(placed, [3, 3, 7])
        self.assertEqual(inner[-1][0], 0)
        self.assertIsNone(g.with_class_frame(out, 'Miri', 'portrait.dds'))  # already there
        with self.assertRaises(g.GfxError):
            g.with_class_frame(src, 'Miri', 'portrait.dds', donor='Kay')

    def test_the_portrait_is_a_dxt5_atlas(self):
        from PIL import Image
        bust = Image.new('RGBA', (95, 95), (200, 100, 50, 255))
        raw = io.BytesIO()
        bust.save(raw, 'PNG')
        dds = g.class_portrait_dds(raw.getvalue())
        self.assertEqual(dds[84:88], b'DXT5')
        atlas = Image.open(io.BytesIO(dds)).convert('RGBA')
        self.assertEqual(atlas.size, g.ATLAS_SIZE)
        self.assertEqual(atlas.getpixel((g.PORTRAIT_SIZE[0] // 2, 10))[3], 255)  # the bust, centred
        self.assertEqual(atlas.getpixel((g.ATLAS_SIZE[0] - 1, g.ATLAS_SIZE[1] - 1))[3], 0)


if __name__ == '__main__':
    unittest.main()
