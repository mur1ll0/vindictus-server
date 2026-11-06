import hashlib
import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

import fairytail_textures as ft


def decode_color(block: bytes) -> list[tuple[int, int, int]]:
    c0, c1, bits = struct.unpack('<HHI', block)
    p0, p1 = ft._rgb(c0), ft._rgb(c1)
    palette = [p0, p1, tuple((2 * a + b) // 3 for a, b in zip(p0, p1)), tuple((a + 2 * b) // 3 for a, b in zip(p0, p1))]
    return [palette[(bits >> (2 * i)) & 3] for i in range(16)]


def decode_alpha(block: bytes) -> list[int]:
    a0, a1 = block[0], block[1]
    table = [a0, a1] + [((7 - i) * a0 + i * a1) // 7 for i in range(1, 7)]
    bits = int.from_bytes(block[2:8], 'little')
    return [table[(bits >> (3 * i)) & 7] for i in range(16)]


def square(z: float) -> tuple[list, list, list]:
    """A UV quad (two triangles) and a separate triangle: two islands."""
    uvs = [(0.0, 0.0), (0.5, 0.0), (0.5, 0.5), (0.0, 0.5), (0.75, 0.75), (1.0, 0.75), (1.0, 1.0)]
    positions = [(0.0, 0.0, z), (1.0, 0.0, z), (1.0, 0.0, z), (0.0, 0.0, z), (4.0, 0.0, 0.0), (4.0, 0.0, 0.0), (4.0, 0.0, 0.0)]
    return uvs, positions, [(0, 1, 2), (0, 2, 3), (4, 5, 6)]


class VtfTest(unittest.TestCase):
    def test_header_mipmaps_and_thumbnail_match_the_clients_textures(self):
        image = Image.new('RGBA', (64, 32), (200, 100, 50, 255))
        for fmt, flags in ((ft.DXT1, 0), (ft.DXT5, ft.FLAG_EIGHTBITALPHA)):
            data = ft.vtf(image, fmt, flags)
            info = ft.vtf_info(data)
            self.assertEqual((info['version'], info['header'], info['width'], info['height'], info['frames']), ((7, 2), 80, 64, 32, 1))
            self.assertEqual((info['format'], info['flags'], info['mipmaps'], info['thumbnail']), (fmt, flags, 7, (ft.DXT1, 16, 16)))
            self.assertEqual(len(data), ft.vtf_size(fmt, 64, 32))
            # the full resolution image is the last level; a flat colour stays that colour
            top = data[-(64 // 4) * (32 // 4) * (8 if fmt == ft.DXT1 else 16):]
            first = top[:8] if fmt == ft.DXT1 else top[8:16]
            self.assertTrue(all(abs(a - b) <= 8 for p in decode_color(first) for a, b in zip(p, (200, 100, 50))))
        with self.assertRaises(ft.TextureError):
            ft.vtf(Image.new('RGBA', (48, 32)), ft.DXT1)

    def test_blocks_keep_two_colours_and_alpha_levels(self):
        pixels = [(255, 0, 0)] * 8 + [(0, 0, 255)] * 8
        decoded = decode_color(ft.color_block(pixels))
        self.assertTrue(all(abs(a - b) <= 8 for p, q in zip(decoded, pixels) for a, b in zip(p, q)))
        alphas = [20] * 8 + [64] * 8
        self.assertEqual(decode_alpha(ft.alpha_block(alphas)), alphas)
        self.assertEqual(decode_alpha(ft.alpha_block([200] * 16)), [200] * 16)


class IslandTest(unittest.TestCase):
    def test_islands_have_uv_area_boundary_and_normalised_place(self):
        uvs, positions, triangles = square(10.0)
        islands = ft.islands_of(uvs, positions, triangles, (0.0, 10.0), 4.0)
        self.assertEqual(len(islands), 2)
        quad, tri = sorted(islands, key=lambda i: -i.area)
        self.assertAlmostEqual(quad.area, 0.25)
        self.assertEqual(len(quad.boundary), 4)          # the diagonal is shared
        self.assertAlmostEqual(quad.z, 1.0)
        self.assertAlmostEqual(tri.side, 1.0)
        self.assertAlmostEqual(tri.area, 0.03125)

    def test_rules_pick_the_first_matching_role(self):
        spec = ft.SPECS['pc_female_ftlucy_jpn_lower']
        self.assertEqual(ft.role_of(spec, {'z': 0.2, 'side': 0, 'area': 0.1}), ft.STOCKING)
        self.assertEqual(ft.role_of(spec, {'z': 0.95, 'side': 0, 'area': 0.1}), ft.LUCY_BELT)
        self.assertEqual(ft.role_of(spec, {'z': 0.8, 'side': 0, 'area': 0.1}), ft.LUCY_SKIRT)
        with self.assertRaises(ft.TextureError):
            ft.role_of(ft.Spec(16, 'Heroes_Faceskin', ft.rules(({'z_above': 0.5}, ft.GOLD))), {'z': 0.1, 'side': 0, 'area': 0})
        with self.assertRaises(ft.TextureError):
            ft.matches({'height': 1}, {'z': 0, 'side': 0, 'area': 0})

    def test_every_spec_ends_with_an_unconditional_role(self):
        self.assertEqual(len(ft.SPECS), 14)
        for name, spec in ft.SPECS.items():
            self.assertEqual(spec.rules[-1][0], {}, name)
            self.assertEqual(spec.shader, 'Heroes_Armor' if name.endswith('_metal') else 'Heroes_Faceskin', name)


class PaintTest(unittest.TestCase):
    def test_islands_get_their_role_colour_trim_and_phong_mask(self):
        uvs, positions, triangles = square(10.0)
        islands = ft.islands_of(uvs, positions, triangles, (0.0, 10.0), 4.0)
        spec = ft.Spec(128, 'Heroes_Faceskin', ft.rules(({'area_below': 0.1}, ft.GOLD), ({}, ft.ERZA_SKIRT)))
        base, normal = ft.paint(spec, islands, 1)
        self.assertEqual((base.size, normal.size, base.mode, normal.mode), ((128, 128), (128, 128), 'RGBA', 'RGBA'))
        inside = base.getpixel((32, 32))[:3]                   # quad centre: skirt cloth
        self.assertTrue(all(abs(a - b) <= 12 for a, b in zip(inside, ft.ERZA_SKIRT.color)))
        self.assertEqual(normal.getpixel((32, 32))[3], ft.SPEC['cloth'])
        corner = base.getpixel((117, 107))[:3]                   # inside the small triangle: gold metal
        self.assertTrue(all(abs(a - b) <= 24 for a, b in zip(corner, ft.GOLD.color)), corner)
        self.assertEqual(normal.getpixel((117, 107))[3], ft.SPEC['metal'])
        self.assertEqual(base.getpixel((1, 32))[:3], ft.ERZA_SKIRT.trim)   # on the quad's edge
        flat = ft.normal_map(Image.new('L', (8, 8), 128), Image.new('L', (8, 8), 20)).getpixel((4, 4))
        self.assertEqual(flat, (128, 128, 248, 20))

    def test_vmt_names_the_textures_in_the_clients_style(self):
        cloth = ft.vmt('pc_female_ftlucy_jpn_upper', ft.SPECS['pc_female_ftlucy_jpn_upper'])
        self.assertTrue(cloth.startswith('"Heroes_Faceskin"\r\n{'))
        self.assertIn('"$basetexture" "models\\armor\\fairytail\\pc_female_ftlucy_jpn_upper"', cloth)
        self.assertIn('"$bumpmap" "models\\armor\\fairytail\\pc_female_ftlucy_jpn_upper_normal"', cloth)
        self.assertIn('"$nocull" "1"', cloth)
        metal = ft.vmt('pc_female_fterza_jpn_head_metal', ft.SPECS['pc_female_fterza_jpn_head_metal'])
        self.assertIn('"Heroes_Armor"', metal)
        self.assertIn('"$envmap" "env_cubemap"', metal)


def tga(size: int = 40) -> bytes:
    return bytes([0, 0, 2]) + bytes(9) + struct.pack('<HH', size, size) + bytes([32, 8]) + bytes(size * size * 4)


class DecorTest(unittest.TestCase):
    def test_flames_rise_from_the_hem_and_quilting_draws_lines(self):
        size = 64
        base = Image.new('RGB', (size, size), ft.ERZA_SKIRT.color)
        height = Image.new('L', (size, size), 128)
        z = [0.0 if i // size >= 32 else 1.0 for i in range(size * size)]   # lower half is the hem
        ft.decorate(base, height, {'flames': Image.new('L', (size, size), 255)}, z)
        self.assertEqual(base.getpixel((10, 60)), ft.FLAME[0])
        self.assertEqual(base.getpixel((10, 5)), ft.ERZA_SKIRT.color)
        quilt = Image.new('RGB', (size, size), (100, 100, 100))
        ft.decorate(quilt, height, {'quilt': Image.new('L', (size, size), 255)}, z)
        self.assertEqual((quilt.getpixel((0, 0)), quilt.getpixel((5, 0))), ((75, 75, 75), (100, 100, 100)))
        with self.assertRaises(ft.TextureError):
            ft.decorate(quilt, height, {'stars': Image.new('L', (size, size), 255)}, z)

    def test_height_map_interpolates_the_corners(self):
        island = ft.Island([((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 1.0))], [], 0.5, 0.0, 0.5)
        z = ft.height_map([island], 8)
        self.assertAlmostEqual(z[0], 0.0625)
        self.assertAlmostEqual(z[6 * 8 + 0], 0.8125)
        self.assertEqual(z[7 * 8 + 7], -1.0)


class GenerateTest(unittest.TestCase):
    def test_generate_writes_every_material_the_icons_and_a_manifest_and_is_stable(self):
        uvs, positions, triangles = square(10.0)
        islands = ft.islands_of(uvs, positions, triangles, (0.0, 10.0), 4.0)
        specs = {'pc_female_x_upper': ft.Spec(16, 'Heroes_Faceskin', ft.rules(({}, ft.LUCY_TOP))),
                 'pc_female_x_upper_metal': ft.Spec(16, 'Heroes_Armor', ft.rules(({}, ft.GOLD)))}
        both = {'pc_female_x_upper': islands, 'pc_female_x_upper_metal': islands}
        with tempfile.TemporaryDirectory() as temp:
            models, out, icons = Path(temp) / 'models', Path(temp) / 'out', Path(temp) / 'icons'
            models.mkdir()
            icons.mkdir()
            (models / 'pc_female_x_upper.mdl').write_bytes(b'')
            with mock.patch.object(ft, 'SPECS', specs), mock.patch.object(ft, 'model_islands', lambda m, n: both):
                with self.assertRaises(ft.TextureError):
                    ft.generate(models, out, icons=icons)                  # no icon
                (icons / 'eq_x_upper.tga').write_bytes(tga(32))
                with self.assertRaises(ft.TextureError):
                    ft.generate(models, out, icons=icons)                  # not a 40x40 icon
                (icons / 'eq_x_upper.tga').write_bytes(tga())
                report = ft.generate(models, out, icons=icons)
                first = {p: p.read_bytes() for p in out.rglob('*') if p.is_file()}
                again = ft.generate(models, out, icons=icons)
                with mock.patch.object(ft, 'model_islands', lambda m, n: {'pc_female_x_upper': islands, 'pc_x_other': islands}):
                    with self.assertRaises(ft.TextureError):
                        ft.generate(models, out, icons=icons)
            self.assertEqual((report['materials'], report['icons'], report['files']), (2, 1, 7))
            self.assertEqual(again['bytes'], report['bytes'])
            manifest = json.loads((out / ft.MANIFEST).read_text(encoding='utf-8'))
            paths = [r['path'] for r in manifest['files']]
            self.assertEqual(paths, sorted([f'materials/models/armor/fairytail/{n}' for n in (
                'pc_female_x_upper.vmt', 'pc_female_x_upper.vtf', 'pc_female_x_upper_normal.vtf',
                'pc_female_x_upper_metal.vmt', 'pc_female_x_upper_metal.vtf', 'pc_female_x_upper_metal_normal.vtf')]
                + ['scaleform/item_icon/eq_x_upper.tga']))
            for record in manifest['files']:
                self.assertEqual(hashlib.sha256((out / record['path']).read_bytes()).hexdigest(), record['sha256'])
            self.assertEqual((out / 'scaleform/item_icon/eq_x_upper.tga').read_bytes(), tga())
            folder = out / 'materials/models/armor/fairytail'
            self.assertEqual(ft.vtf_info((folder / 'pc_female_x_upper_metal.vtf').read_bytes())['format'], ft.DXT5)
            self.assertEqual(ft.vtf_info((folder / 'pc_female_x_upper.vtf').read_bytes())['format'], ft.DXT1)
            self.assertEqual(first, {p: p.read_bytes() for p in out.rglob('*') if p.is_file()})

    def test_icon_names_follow_the_models(self):
        self.assertEqual(ft.icon_name('pc_female_fterza_jpn_upper'), 'eq_fterza_jpn_upper')
        with self.assertRaises(ft.TextureError):
            ft.icon_name('pc_male_x_upper')


if __name__ == '__main__':
    unittest.main()
