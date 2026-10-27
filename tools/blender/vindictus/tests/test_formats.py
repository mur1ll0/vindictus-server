"""Format tests; run with Blender's Python (it ships numpy):
  "<Blender 5.1>/5.1/python/bin/python.exe" -m unittest discover -s tools/blender/vindictus/tests -t tools/blender/vindictus
Game files are never versioned: the kit round trip runs only when the laboratory kit exists."""
import random
import unittest
from pathlib import Path

import numpy as np

from formats import kit, vtf, vtx, vvd

LAB_KIT = Path(__file__).resolve().parents[4] / '.cache/private-assets/kits/fiona_inner_fringe'


class VvdTest(unittest.TestCase):
    def test_write_then_read_keeps_vertices(self):
        vertices = [vvd.Vertex((0.75, 0.25, 0.0), (3, 7, 0), 2, (1.0, 2.0, 3.0), (0.0, 0.0, 1.0), (0.5, 0.25)),
                    vvd.Vertex((1.0, 0.0, 0.0), (0, 0, 0), 1, (-1.0, 0.0, 9.0), (1.0, 0.0, 0.0), (0.0, 1.0))]
        tangents = [(1.0, 0.0, 0.0, 1.0), (0.0, 1.0, 0.0, -1.0)]
        back = vvd.read(vvd.write(1234, 3, vertices, tangents))
        self.assertEqual((back.checksum, back.num_lods, back.vertices, back.tangents), (1234, 3, vertices, tangents))


class VtxTest(unittest.TestCase):
    def test_palettes_respect_the_bone_limit_and_triangles_survive(self):
        rng = random.Random(5)
        count = 300
        bones = [tuple(rng.sample(range(70), 3)) for _ in range(count)]
        triangles = [tuple(rng.sample(range(count), 3)) for _ in range(400)]
        for palette, _ in vtx.strips_of(triangles, bones, 53):
            self.assertLessEqual(len(palette), 53)
        data = vtx.write(99, 2, 53, [[triangles]], [[bones]], [[[3] * count]])
        back = vtx.read(data)
        self.assertEqual((back.checksum, back.num_lods, back.max_bones_per_strip), (99, 2, 53))
        self.assertEqual(back.triangles[0][0][0], triangles)


class VtfTest(unittest.TestCase):
    def test_bgra_round_trip_and_mipmaps(self):
        image = np.random.default_rng(1).integers(0, 256, (32, 64, 4), dtype=np.uint8)
        data = vtf.write(image)
        back, info = vtf.read(data)
        self.assertTrue(np.array_equal(back, image))
        self.assertEqual((info['width'], info['height'], info['mipmaps'], info['format']), (64, 32, 7, vtf.BGRA8888))

    def test_dxt1_block_decodes_its_two_colors(self):
        # c0 = pure red (0xF800), c1 = pure blue (0x001F), every pixel index 0 except the first row index 1.
        block = bytes([0x00, 0xF8, 0x1F, 0x00, 0b01010101, 0, 0, 0])
        image = vtf.decode(vtf.DXT1, 4, 4, block)
        self.assertEqual(tuple(image[1, 0]), (255, 0, 0, 255))
        self.assertEqual(tuple(image[0, 0]), (0, 0, 255, 255))


class ZipTest(unittest.TestCase):
    def test_kit_zip_extracts_inside_its_folder_only(self):
        import tempfile
        import zipfile
        with tempfile.TemporaryDirectory() as tmp:
            good = Path(tmp) / 'good.zip'
            with zipfile.ZipFile(good, 'w') as z:
                z.writestr('kit.json', '{"model": "models/a.mdl"}')
                z.writestr('Models/A.mdl', b'IDST')
            root = kit.extract_zip(good, Path(tmp) / 'out')
            self.assertEqual((root / 'models/a.mdl').read_bytes(), b'IDST')
            for name, content in (('bad.zip', {'models/a.mdl': b''}), ('evil.zip', {'kit.json': b'{}', '../x.txt': b''})):
                path = Path(tmp) / name
                with zipfile.ZipFile(path, 'w') as z:
                    for entry, data in content.items():
                        z.writestr(entry, data)
                with self.assertRaises(kit.KitError):
                    kit.extract_zip(path, Path(tmp) / name.replace('.zip', ''))
            self.assertFalse((Path(tmp) / 'x.txt').exists())

    def test_exported_folder_zips_stored_with_forward_slashes(self):
        import tempfile
        import zipfile
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp) / 'kit'
            (folder / 'models/player').mkdir(parents=True)
            (folder / 'kit.json').write_text('{"model": "models/player/a.mdl"}')
            (folder / 'models/player/a.mdl').write_bytes(b'IDST')
            kit.zip_folder(folder, Path(tmp) / 'a.zip')
            with zipfile.ZipFile(Path(tmp) / 'a.zip') as z:
                self.assertEqual(sorted(i.filename for i in z.infolist()), ['kit.json', 'models/player/a.mdl'])
                self.assertTrue(all(i.compress_type == zipfile.ZIP_STORED for i in z.infolist()))
            back = kit.extract_zip(Path(tmp) / 'a.zip', Path(tmp) / 'back')
            self.assertEqual((back / 'models/player/a.mdl').read_bytes(), b'IDST')


@unittest.skipUnless((LAB_KIT / 'kit.json').is_file(), 'laboratory kit not present')
class KitRoundTripTest(unittest.TestCase):
    def test_saved_kit_reads_back_the_same_geometry(self):
        import tempfile
        source = kit.load(LAB_KIT)
        with tempfile.TemporaryDirectory() as out:
            kit.save(source, Path(out), 'models/player/fiona/round_trip.mdl', source.models)
            back = kit.load(Path(out))
        self.assertEqual(back.mdl.name, 'player\\fiona\\round_trip.mdl')
        for a, b in zip(source.models[0], back.models[0]):
            self.assertEqual((a.positions, a.uvs, a.triangles), (b.positions, b.uvs, b.triangles))


if __name__ == '__main__':
    unittest.main()
