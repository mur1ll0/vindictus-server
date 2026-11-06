"""Lab textures for the Fairy Tail collab outfits (Erza, Lucy, Wendy).

The 12 Fairy Tail models of the 1.69 EU client name materials under models/armor/fairytail that no
client has (EU 1.69, NA, Windictus 2023), so the pieces render white in the preview and as a checker
when worn (docs/content-tuning.md, collabs). This generator draws original textures for them, made
only of colour blocks and simple cloth/metal/leather patterns placed on the models' own UV islands:
  - each material's islands (connected triangles of the .vtx, vertices of the .vvd) get a role (plate,
    skirt, belt, stocking, boot...) from the island's height, side distance and UV area (SPECS);
  - the base texture is the colour itself: the items' Material1..3 are fix_red/green/bluechannel
    (127,0,0 / 0,127,0 / 0,0,127), the identity tint of the armour shaders' RGB colour mask;
  - the normal map comes from a height map (weave, grain, raised trims), its alpha is the phong mask
    (low on cloth, higher on metal), as the client's armour normal maps;
  - VTF 7.2 like the client's: DXT1 base (DXT5 with alpha for the Heroes_Armor "_metal" materials),
    DXT5 normal map, full mipmap chain and a 16x16 DXT1 thumbnail; VMT in the client's style.
The items' icons (ItemClassInfo.Icon eq_ft*), which the EU 1.69 lacks, are the official ones of the NA
client (scaleform/item_icon, read only); the outfit colours follow them.
The output is a lab asset outside git (.cache/private-assets/collabs/fairytail, with collab-assets.json);
scripts/client/apply_lab_content_additions.py packs it into the client copy.
  py -3.13 scripts/client/fairytail_textures.py [--models <hfs_extracted>/models/armor/fairytail] [--out <folder>]
                                                [--icons <NA hfsExtracted>/scaleform/item_icon]
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import random
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/blender/vindictus/formats'))
import mdl as mdl_format  # noqa: E402  (pure Python readers of the Blender add-on)
import vtx as vtx_format  # noqa: E402
import vvd as vvd_format  # noqa: E402

MODELS = ROOT.parent / 'Vindictus1.69EN/Vindictus EU/en-EU/hfs_extracted/models/armor/fairytail'
OUT = ROOT / '.cache/private-assets/collabs/fairytail'
MANIFEST = 'collab-assets.json'
FOLDER = 'models/armor/fairytail'
ICONS = Path('F:/Games/vindictus/hfsExtracted/scaleform/item_icon')   # NA client, read only
ICON_FOLDER = 'scaleform/item_icon'
DXT1, DXT5 = 13, 15
FLAG_EIGHTBITALPHA = 0x2000


class TextureError(ValueError):
    pass


# ---------------------------------------------------------------------------------------------- roles
@dataclass(frozen=True)
class Role:
    color: tuple[int, int, int]
    pattern: str                      # cloth, leather, metal
    trim: tuple[int, int, int] | None = None
    spec: int | None = None           # normal map alpha (phong mask); default by pattern
    decor: str | None = None          # flames, wrap, quilt (DECOR)


SPEC = {'cloth': 20, 'leather': 30, 'metal': 64}

# Colours after the official item icons of the NA client (scaleform/item_icon/eq_ft*.tga), the only
# picture of these outfits any client has; the drawing itself is the lab's.
SILVER = Role((206, 211, 220), 'metal', (150, 156, 168))
GOLD = Role((222, 184, 82), 'metal', (156, 118, 44))
HEART = Role((206, 32, 52), 'metal', (130, 16, 30))
ERZA_WRAP = Role((236, 236, 232), 'cloth', (196, 196, 200), decor='wrap')
ERZA_SKIRT = Role((186, 36, 44), 'cloth', (120, 18, 26), decor='flames')
ERZA_BELT = Role((232, 226, 218), 'cloth', (186, 36, 44))
ERZA_BAND = Role((44, 44, 48), 'metal', (96, 96, 104))
LUCY_TOP = Role((242, 242, 244), 'cloth', (228, 186, 62))
LUCY_SLEEVE = Role((118, 168, 228), 'cloth', (52, 92, 172))
LUCY_TRIM = Role((228, 186, 62), 'cloth', (160, 120, 30))
LUCY_SKIRT = Role((168, 128, 140), 'cloth', (110, 78, 96))
LUCY_BELT = Role((236, 236, 236), 'leather', (110, 78, 96))
STOCKING = Role((64, 62, 68), 'cloth', (40, 38, 44))
BOOT = Role((150, 100, 52), 'leather', (96, 62, 30))
BOOT_SOLE = Role((70, 46, 26), 'leather')
BOOT_CUFF = Role((176, 126, 70), 'leather', (96, 62, 30))
WENDY_DRESS = Role((40, 96, 56), 'cloth', (24, 64, 36), decor='quilt')
WENDY_COLLAR = Role((176, 214, 160), 'cloth', (228, 198, 88))
WENDY_YELLOW = Role((228, 198, 88), 'cloth', (160, 130, 40))
WENDY_SCRUNCHIE = Role((232, 150, 172), 'cloth', (190, 100, 128))
WENDY_SHOE = Role((122, 26, 36), 'leather', (80, 14, 20))


@dataclass(frozen=True)
class Spec:
    size: int
    shader: str                       # Heroes_Faceskin (cloth) or Heroes_Armor (metal)
    rules: tuple                      # ((conditions, role), ...) first match wins; last one has no conditions
    nocull: int = 0
    phong: tuple[float, float] = (0.0, 0.05)


def rules(*pairs) -> tuple:
    return tuple((dict(c), r) for c, r in pairs)


# Island conditions: z_above / z_below (centroid height in the model's height, 0..1), side_above (centroid
# side distance |x| over the model's largest |x|), area_below / area_above (UV area, fraction of the texture).
SPECS: dict[str, Spec] = {
    'pc_female_fterza_jpn_upper': Spec(512, 'Heroes_Faceskin', rules(({}, ERZA_WRAP))),
    'pc_female_fterza_jpn_lower': Spec(512, 'Heroes_Faceskin', rules(({'z_above': 0.88}, ERZA_BELT), ({}, ERZA_SKIRT)),
                                       nocull=1),
    'pc_female_fterza_jpn_hand': Spec(256, 'Heroes_Faceskin', rules(({}, ERZA_BAND)), phong=(8, 0.4)),
    'pc_female_fterza_jpn_head_metal': Spec(256, 'Heroes_Armor', rules(({}, SILVER))),
    'pc_female_ftlucy_jpn_upper': Spec(512, 'Heroes_Faceskin', rules(({'side_above': 0.35}, LUCY_SLEEVE),
                                                                     ({'area_below': 0.002}, LUCY_TRIM), ({}, LUCY_TOP)),
                                       nocull=1),
    'pc_female_ftlucy_jpn_lower': Spec(512, 'Heroes_Faceskin', rules(({'z_below': 0.7}, STOCKING), ({'z_above': 0.93}, LUCY_BELT),
                                                                     ({}, LUCY_SKIRT)),
                                       nocull=1),
    'pc_female_ftlucy_jpn_lower_metal': Spec(256, 'Heroes_Armor', rules(({}, GOLD))),
    'pc_female_ftlucy_jpn_foot': Spec(512, 'Heroes_Faceskin', rules(({'z_below': 0.06}, BOOT_SOLE), ({'z_above': 0.93}, BOOT_CUFF),
                                                                    ({}, BOOT))),
    'pc_female_ftlucy_jpn_head_metal': Spec(256, 'Heroes_Armor', rules(({'z_above': 0.5}, SILVER), ({}, HEART))),
    'pc_female_ftwendy_jpn_upper_metal': Spec(256, 'Heroes_Armor', rules(({}, GOLD))),
    'pc_female_ftwendy_jpn_upper': Spec(512, 'Heroes_Faceskin', rules(({'area_below': 0.0015}, WENDY_YELLOW),
                                                                      ({'z_above': 0.72}, WENDY_COLLAR), ({}, WENDY_DRESS)),
                                        nocull=1),
    'pc_female_ftwendy_jpn_hand': Spec(256, 'Heroes_Faceskin', rules(({}, WENDY_SCRUNCHIE)), nocull=1),
    'pc_female_ftwendy_jpn_head_metal': Spec(256, 'Heroes_Armor', rules(({'z_above': 0.5}, SILVER), ({}, HEART))),
    'pc_female_ftwendy_jpn_foot': Spec(256, 'Heroes_Faceskin', rules(({}, WENDY_SHOE))),
}


def matches(conditions: dict, f: dict) -> bool:
    tests = {'z_above': lambda v: f['z'] > v, 'z_below': lambda v: f['z'] < v, 'side_above': lambda v: f['side'] > v,
             'area_below': lambda v: f['area'] < v, 'area_above': lambda v: f['area'] > v}
    for key, value in conditions.items():
        if key not in tests:
            raise TextureError(f'Unknown island condition {key}.')
        if not tests[key](value):
            return False
    return True


def role_of(spec: Spec, features: dict) -> Role:
    for conditions, role in spec.rules:
        if matches(conditions, features):
            return role
    raise TextureError('The rules of a material must end with an unconditional role.')


# -------------------------------------------------------------------------------------------- geometry
@dataclass
class Island:
    triangles: list[tuple[tuple[float, float, float], ...]]   # (u, v, height 0..1) of each corner
    boundary: list[tuple[tuple[float, float], tuple[float, float]]]  # UV edges used by one triangle
    z: float
    side: float
    area: float


def islands_of(uvs: list[tuple], positions: list[tuple], triangles: list[tuple], z_range: tuple[float, float],
               side_max: float) -> list[Island]:
    """Connected triangles (shared vertices) of one mesh; vertices split at UV seams make UV islands."""
    parent = list(range(len(uvs)))

    def find(a: int) -> int:
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a
    for a, b, c in triangles:
        for x, y in ((a, b), (b, c)):
            rx, ry = find(x), find(y)
            if rx != ry:
                parent[rx] = ry
    groups: dict[int, list[tuple]] = {}
    for t in triangles:
        groups.setdefault(find(t[0]), []).append(t)
    z0, z1 = z_range
    out = []
    for _, tris in sorted(groups.items()):
        edges: dict[tuple[int, int], int] = {}
        area = 0.0
        verts = set()
        for a, b, c in tris:
            for e in ((a, b), (b, c), (c, a)):
                key = (min(e), max(e))
                edges[key] = edges.get(key, 0) + 1
            (ua, va), (ub, vb), (uc, vc) = uvs[a], uvs[b], uvs[c]
            area += abs((ub - ua) * (vc - va) - (uc - ua) * (vb - va)) / 2
            verts.update((a, b, c))
        z = sum(positions[i][2] for i in verts) / len(verts)
        side = abs(sum(positions[i][0] for i in verts) / len(verts))
        def height(i: int) -> float:
            return (positions[i][2] - z0) / (z1 - z0) if z1 > z0 else 0.5
        out.append(Island([tuple((*uvs[i], height(i)) for i in t) for t in tris],
                          [(uvs[a], uvs[b]) for (a, b), n in edges.items() if n == 1],
                          (z - z0) / (z1 - z0) if z1 > z0 else 0.5, side / side_max if side_max > 0 else 0.0, area))
    return out


def model_islands(models: Path, name: str) -> dict[str, list[Island]]:
    """Material name (as the model names it, lower case) -> islands of its meshes."""
    base = models / name
    mdl = mdl_format.read(base.with_suffix('.mdl').read_bytes())
    vvd = vvd_format.read(base.with_suffix('.vvd').read_bytes())
    vtx = vtx_format.read(base.with_name(name + '.dx90.vtx').read_bytes())
    if not (mdl.checksum == vvd.checksum == vtx.checksum):
        raise TextureError(f'{name}: model, vertex and optimized model checksums differ.')
    meshes = []
    for b, part in enumerate(mdl.bodyparts):
        for m, model in enumerate(part.models):
            first = model.vertex_index // 48
            for k, mesh in enumerate(model.meshes):
                verts = vvd.vertices[first + mesh.vertex_offset:first + mesh.vertex_offset + mesh.num_vertices]
                meshes.append((mesh.material, verts, vtx.triangles[b][m][k]))
    every = [v for _, verts, _ in meshes for v in verts]
    if not every:
        raise TextureError(f'{name} has no vertices.')
    z_range = (min(v.pos[2] for v in every), max(v.pos[2] for v in every))
    side_max = max(abs(v.pos[0]) for v in every)
    out: dict[str, list[Island]] = {}
    for material, verts, tris in meshes:
        texture = mdl.textures[material].lower()
        out.setdefault(texture, []).extend(islands_of([v.uv for v in verts], [v.pos for v in verts], tris, z_range, side_max))
    return out


# ---------------------------------------------------------------------------------------------- paint
def pattern_height(pattern: str, size: int, seed: int) -> Image.Image:
    """Height detail (L, 128 = flat) of a surface pattern."""
    if pattern == 'cloth':
        tile = Image.new('L', (4, 4))
        tile.putdata([150, 150, 106, 106, 150, 150, 106, 106, 106, 106, 150, 150, 106, 106, 150, 150])
        out = Image.new('L', (size, size))
        for y in range(0, size, 4):
            for x in range(0, size, 4):
                out.paste(tile, (x, y))
        return out
    rng = random.Random(seed)
    if pattern == 'metal':
        # brushed: one random value per row, stretched across
        rows = Image.new('L', (1, size))
        rows.putdata([128 + rng.randint(-6, 6) for _ in range(size)])
        return rows.resize((size, size), Image.NEAREST)
    grain = Image.new('L', (size // 2, size // 2))
    grain.putdata([128 + rng.randint(-24, 24) for _ in range(grain.width * grain.height)])
    return grain.resize((size, size), Image.BILINEAR).filter(ImageFilter.GaussianBlur(0.8))


def shaded(color: tuple[int, int, int], height: Image.Image, amount: float) -> Image.Image:
    """The colour modulated by a height pattern (128 = the colour itself)."""
    lut = [max(0, min(255, round(c * (1 + amount * (h - 128) / 128)))) for c in color for h in range(256)]
    return Image.merge('RGB', [height.point(lut[i * 256:(i + 1) * 256]) for i in range(3)])


def height_map(islands: list[Island], size: int) -> list[float]:
    """Model height (0..1) at each texel the islands' triangles cover, -1 elsewhere (barycentric over UV)."""
    z = [-1.0] * (size * size)
    for island in islands:
        for (u0, v0, h0), (u1, v1, h1), (u2, v2, h2) in island.triangles:
            x0, y0, x1, y1, x2, y2 = u0 * size, v0 * size, u1 * size, v1 * size, u2 * size, v2 * size
            det = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)
            if abs(det) < 1e-9:
                continue
            for y in range(max(0, int(min(y0, y1, y2))), min(size, int(max(y0, y1, y2)) + 1)):
                py = y + 0.5
                for x in range(max(0, int(min(x0, x1, x2))), min(size, int(max(x0, x1, x2)) + 1)):
                    px = x + 0.5
                    a = ((y1 - y2) * (px - x2) + (x2 - x1) * (py - y2)) / det
                    b = ((y2 - y0) * (px - x2) + (x0 - x2) * (py - y2)) / det
                    if a >= -0.05 and b >= -0.05 and a + b <= 1.05:
                        z[y * size + x] = a * h0 + b * h1 + (1 - a - b) * h2
    return z


FLAME = ((246, 204, 52), (236, 120, 34))


def decorate(base: Image.Image, height: Image.Image, masks: dict[str, Image.Image], z: list[float]) -> None:
    """Pattern drawn over the role colour: flames rising from the hem (Erza's skirt), wrapping bands
    (Erza's chest wrap) and quilting lines (Wendy's dress)."""
    size = base.width
    color, bump = base.load(), height.load()
    for decor, mask in masks.items():
        covered = mask.tobytes()
        for i, m in enumerate(covered):
            if not m:
                continue
            y, x = divmod(i, size)
            if decor == 'flames':
                h = z[i] if z[i] >= 0 else 1.0
                edge = 0.17 + 0.07 * abs(math.sin(x / 9.0)) + 0.025 * math.sin(x / 3.3)
                if h < edge:
                    color[x, y] = FLAME[0]
                    bump[x, y] = 150
                elif h < edge + 0.045:
                    color[x, y] = FLAME[1]
            elif decor == 'wrap' and y % 14 < 2:
                r, g, b = color[x, y]
                color[x, y] = (r * 86 // 100, g * 86 // 100, b * 86 // 100)
                bump[x, y] = 156
            elif decor == 'quilt' and ((x + y) % 26 < 2 or (x - y) % 26 < 2):
                r, g, b = color[x, y]
                color[x, y] = (r * 3 // 4, g * 3 // 4, b * 3 // 4)
                bump[x, y] = 170
            elif decor not in ('flames', 'wrap', 'quilt'):
                raise TextureError(f'Unknown decoration {decor}.')


def paint(spec: Spec, islands: list[Island], seed: int) -> tuple[Image.Image, Image.Image]:
    """(base RGBA, normal RGBA) of one material."""
    size = spec.size
    roles = [role_of(spec, f.__dict__) for f in islands]
    by_area: dict[Role, float] = {}
    for island, role in zip(islands, roles):
        by_area[role] = by_area.get(role, 0.0) + island.area
    main = max(by_area, key=by_area.get) if by_area else role_of(spec, {'z': 0.5, 'side': 0.0, 'area': 1.0})
    trim_width = max(2, size // 128)
    patterns = {p: pattern_height(p, size, seed + i) for i, p in enumerate(('cloth', 'leather', 'metal'))}
    base = shaded(main.color, patterns[main.pattern], 0.06)
    height = patterns[main.pattern].copy()
    spec_mask = Image.new('L', (size, size), main.spec if main.spec is not None else SPEC[main.pattern])
    textures = {role: shaded(role.color, patterns[role.pattern], 0.06 if role.pattern == 'cloth' else 0.1) for role in by_area}
    z = height_map(islands, size)
    # cloth and leather a little darker downwards (folds, shadow), metal even
    gradient = Image.new('L', (size, size))
    gradient.putdata([round(255 * (0.78 + 0.22 * h)) if h >= 0 else 230 for h in z])
    light = Image.new('L', (size, size), 255)
    decor: dict[str, Image.Image] = {}
    trims = []
    for island, role in zip(islands, roles):
        mask = Image.new('L', (size, size))
        draw = ImageDraw.Draw(mask)
        for tri in island.triangles:
            draw.polygon([(u * size, v * size) for u, v, _ in tri], fill=255, outline=255)
        # a little bleed around the island against seams in the mipmaps
        for a, b in island.boundary:
            draw.line([(a[0] * size, a[1] * size), (b[0] * size, b[1] * size)], fill=255, width=3)
        base.paste(textures[role], (0, 0), mask)
        height.paste(patterns[role.pattern], (0, 0), mask)
        spec_mask.paste(role.spec if role.spec is not None else SPEC[role.pattern], (0, 0), mask)
        if role.pattern != 'metal':
            light.paste(gradient, (0, 0), mask)
        if role.decor:
            decor.setdefault(role.decor, Image.new('L', (size, size))).paste(255, (0, 0), mask)
        if role.trim:
            trim = Image.new('L', (size, size))
            line = ImageDraw.Draw(trim)
            for a, b in island.boundary:
                line.line([(a[0] * size, a[1] * size), (b[0] * size, b[1] * size)], fill=255, width=trim_width * 2)
            trims.append((ImageChops.multiply(trim, mask), role.trim))
    decorate(base, height, decor, z)
    base = ImageChops.multiply(base, Image.merge('RGB', (light, light, light)))
    for trim, colour in trims:
        base.paste(colour, (0, 0), trim)
        height.paste(172, (0, 0), trim)
    height = height.filter(ImageFilter.GaussianBlur(0.6))
    return base.convert('RGBA'), normal_map(height, spec_mask)


def normal_map(height: Image.Image, spec_mask: Image.Image, strength: float = 2.0) -> Image.Image:
    """Tangent-space normal map (128,128,255 flat) of a height map; alpha = phong mask."""
    dx = height.filter(ImageFilter.Kernel((3, 3), [0, 0, 0, strength, 0, -strength, 0, 0, 0], scale=1, offset=128))
    dy = height.filter(ImageFilter.Kernel((3, 3), [0, strength, 0, 0, 0, 0, 0, -strength, 0], scale=1, offset=128))
    z = Image.new('L', height.size, 248)
    return Image.merge('RGBA', (dx, dy, z, spec_mask))


# ------------------------------------------------------------------------------------------------ VTF
def _565(c: tuple[int, int, int]) -> int:
    return ((c[0] * 31 + 127) // 255) << 11 | ((c[1] * 63 + 127) // 255) << 5 | (c[2] * 31 + 127) // 255


def _rgb(c: int) -> tuple[int, int, int]:
    return ((c >> 11) * 255 // 31, ((c >> 5) & 63) * 255 // 63, (c & 31) * 255 // 31)


def color_block(pixels: list[tuple[int, int, int]]) -> bytes:
    """DXT colour block (4-colour mode): endpoints the darkest and brightest pixel."""
    lum = [2 * r + 4 * g + b for r, g, b in pixels]
    c0, c1 = _565(pixels[lum.index(max(lum))]), _565(pixels[lum.index(min(lum))])
    if c0 < c1:
        c0, c1 = c1, c0
    if c0 == c1:
        return struct.pack('<HHI', c0, c1, 0)
    p0, p1 = _rgb(c0), _rgb(c1)
    palette = [p0, p1, tuple((2 * a + b) // 3 for a, b in zip(p0, p1)), tuple((a + 2 * b) // 3 for a, b in zip(p0, p1))]
    bits = 0
    for i, p in enumerate(pixels):
        best = min(range(4), key=lambda k: sum((x - y) ** 2 for x, y in zip(p, palette[k])))
        bits |= best << (2 * i)
    return struct.pack('<HHI', c0, c1, bits)


def alpha_block(alphas: list[int]) -> bytes:
    """DXT5 alpha block (8-level mode)."""
    a0, a1 = max(alphas), min(alphas)
    if a0 == a1:
        return bytes([a0, a1]) + bytes(6)
    table = [a0, a1] + [((7 - i) * a0 + i * a1) // 7 for i in range(1, 7)]
    bits = 0
    for i, a in enumerate(alphas):
        bits |= min(range(8), key=lambda k: abs(a - table[k])) << (3 * i)
    return bytes([a0, a1]) + bits.to_bytes(6, 'little')


def compress(image: Image.Image, fmt: int) -> bytes:
    w, h = image.size
    raw = image.convert('RGBA').tobytes()
    cache: dict[bytes, bytes] = {}
    out = bytearray()
    for by in range(0, max(h, 4), 4):
        for bx in range(0, max(w, 4), 4):
            key = b''.join(raw[((min(by + y, h - 1)) * w + min(bx + x, w - 1)) * 4:][:4] for y in range(4) for x in range(4))
            block = cache.get(key)
            if block is None:
                pixels = [tuple(key[i:i + 3]) for i in range(0, 64, 4)]
                block = color_block(pixels)
                if fmt == DXT5:
                    block = alpha_block(list(key[3::4])) + block
                cache[key] = block
            out += block
    return bytes(out)


def vtf(image: Image.Image, fmt: int, flags: int = 0) -> bytes:
    """VTF 7.2 (80-byte header) with every mipmap and a 16x16 DXT1 thumbnail, as the client's textures."""
    w, h = image.size
    if fmt not in (DXT1, DXT5) or w & (w - 1) or h & (h - 1) or w < 16 or h < 16:
        raise TextureError('Textures are DXT1/DXT5 with power-of-two sides of at least 16.')
    levels = [image.convert('RGBA')]
    while levels[-1].size != (1, 1):
        lw, lh = levels[-1].size
        levels.append(levels[-1].resize((max(1, lw // 2), max(1, lh // 2)), Image.BOX))
    rgb = levels[-1].getpixel((0, 0))
    reflectivity = [(c / 255) ** 2.2 for c in rgb[:3]]
    header = bytearray(80)
    struct.pack_into('<4sIIIHHIHH', header, 0, b'VTF\0', 7, 2, 80, w, h, flags, 1, 0)
    struct.pack_into('<3f', header, 32, *reflectivity)
    struct.pack_into('<fiBiBBH', header, 48, 1.0, fmt, len(levels), DXT1, 16, 16, 1)
    thumb = compress(image.convert('RGBA').resize((16, 16), Image.BOX), DXT1)
    return bytes(header) + thumb + b''.join(compress(level, fmt) for level in reversed(levels))


def vtf_info(data: bytes) -> dict:
    if data[:4] != b'VTF\0':
        raise TextureError('Not a VTF texture.')
    major, minor, header, w, h, flags, frames = struct.unpack_from('<IIIHHIH', data, 4)
    fmt, mips, low, lw, lh = struct.unpack_from('<iBiBB', data, 52)
    return {'version': (major, minor), 'header': header, 'width': w, 'height': h, 'flags': flags, 'frames': frames,
            'format': fmt, 'mipmaps': mips, 'thumbnail': (low, lw, lh)}


def vtf_size(fmt: int, w: int, h: int) -> int:
    block = 8 if fmt == DXT1 else 16
    total = 80 + 16 * 8
    level = 0
    while True:
        total += max(1, (max(1, w >> level) + 3) // 4) * max(1, (max(1, h >> level) + 3) // 4) * block
        if max(1, w >> level) == 1 and max(1, h >> level) == 1:
            return total
        level += 1


# ------------------------------------------------------------------------------------------------ VMT
def vmt(name: str, spec: Spec) -> str:
    texture = 'models\\armor\\fairytail\\' + name
    if spec.shader == 'Heroes_Armor':
        body = [('$envmap', 'env_cubemap'), ('$envmapcontrast', '1'), ('$envmaptint', '[0.6 0.6 0.6]'), ('$halflambert', '0'),
                ('$nocull', str(spec.nocull)), ('$nodecal', '1'), ('$alphatest', '0'), ('$translucent', '0'), ('$surface', 'metal')]
    else:
        body = [('$nocull', str(spec.nocull)), ('$nodecal', '1'), ('$alphatest', '0'), ('$translucent', '0'), ('$halflambert', '0'),
                ('$envmapmask', ''), ('$phong_enable', '1'), ('$phongexponentmap', ''), ('$phongexponent', f'{spec.phong[0]:g}'),
                ('$phongboost', f'{spec.phong[1]:g}'), ('$surface', 'metal' if spec.phong[1] >= 0.2 else 'cloth')]
    lines = [f'"{spec.shader}"', '{', '\t// Lab texture (scripts/client/fairytail_textures.py)',
             f'\t"$basetexture" "{texture}"', f'\t"$bumpmap" "{texture}_normal"', '']
    lines += [f'\t"{k}" "{v}"' for k, v in body]
    return '\r\n'.join(lines + ['}', ''])


# ----------------------------------------------------------------------------------------------- main
def model_names(models: Path) -> list[str]:
    return sorted(p.stem for p in models.glob('*.mdl'))


def icon_name(model: str) -> str:
    """ItemClassInfo.Icon of a model's items: pc_female_fterza_jpn_upper -> eq_fterza_jpn_upper."""
    if not model.startswith('pc_female_'):
        raise TextureError(f'{model} is not a female collab model.')
    return 'eq_' + model[len('pc_female_'):]


def checked_icon(data: bytes, name: str) -> bytes:
    """An item icon as the client's: uncompressed 40x40 TGA, 32 bits with 8 alpha bits."""
    if len(data) < 18 or data[2] != 2 or struct.unpack_from('<HH', data, 12) != (40, 40) or data[16] != 32             or data[17] & 0x0F != 8 or len(data) < 18 + data[0] + 40 * 40 * 4:
        raise TextureError(f'{name} is not a 40x40 32-bit TGA icon.')
    return data


def generate(models: Path, out: Path, previews: Path | None = None, icons: Path | None = ICONS) -> dict:
    """Writes the VMT/VTF of every material of the Fairy Tail models, the items' icons and the manifest;
    returns a report."""
    files: dict[str, bytes] = {}
    report = {}
    names = model_names(models)
    if not names:
        raise TextureError(f'No models in {models}.')
    for seed, name in enumerate(names):
        for material, islands in model_islands(models, name).items():
            spec = SPECS.get(material)
            if spec is None:
                raise TextureError(f'{name} names material {material}, which has no texture spec.')
            base, normal = paint(spec, islands, seed * 7)
            metal = spec.shader == 'Heroes_Armor'
            if metal:
                base.putalpha(200)    # the reflection mask of the client's _metal textures
            files[f'materials/{FOLDER}/{material}.vmt'] = vmt(material, spec).encode('latin-1')
            files[f'materials/{FOLDER}/{material}.vtf'] = vtf(base, DXT5 if metal else DXT1, FLAG_EIGHTBITALPHA if metal else 0)
            files[f'materials/{FOLDER}/{material}_normal.vtf'] = vtf(normal, DXT5, FLAG_EIGHTBITALPHA)
            roles: dict[str, int] = {}
            for island in islands:
                role = role_of(spec, island.__dict__)
                key = '#%02x%02x%02x' % role.color
                roles[key] = roles.get(key, 0) + 1
            report[material] = {'model': name, 'islands': len(islands), 'roles': roles}
            if previews:
                previews.mkdir(parents=True, exist_ok=True)
                sheet = Image.new('RGB', (spec.size * 2, spec.size))
                sheet.paste(base.convert('RGB'), (0, 0))
                sheet.paste(normal.convert('RGB'), (spec.size, 0))
                sheet.save(previews / f'{material}.png')
    missing = set(SPECS) - set(report)
    if missing:
        raise TextureError(f'The models no longer name {sorted(missing)[0]}.')
    # The EU 1.69 has no icon for these items (blank slot); the NA client has the official ones.
    for name in (names if icons else []):
        icon = icon_name(name) + '.tga'
        source = icons / icon
        if not source.is_file():
            raise TextureError(f'No item icon {source}.')
        files[f'{ICON_FOLDER}/{icon}'] = checked_icon(source.read_bytes(), icon)
    records = []
    for path, data in sorted(files.items()):
        target = out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists() or target.read_bytes() != data:
            target.write_bytes(data)
        records.append({'path': path, 'sha256': hashlib.sha256(data).hexdigest()})
    (out / MANIFEST).write_text(json.dumps({'collab': 'fairytail', 'generator': 'scripts/client/fairytail_textures.py',
                                            'files': records}, indent=1), encoding='utf-8')
    return {'materials': len(report), 'icons': sum(p.startswith(ICON_FOLDER) for p in files), 'files': len(records),
            'bytes': sum(len(d) for d in files.values()), 'detail': report}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--models', type=Path, default=MODELS)
    parser.add_argument('--out', type=Path, default=OUT)
    parser.add_argument('--previews', type=Path, help='PNG sheets (base | normal) of each material')
    parser.add_argument('--icons', type=Path, default=ICONS, help='folder with the eq_ft*.tga item icons (NA client)')
    args = parser.parse_args()
    try:
        report = generate(args.models, args.out, args.previews, args.icons)
    except (TextureError, OSError, ValueError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 1
    print(json.dumps({k: v for k, v in report.items() if k != 'detail'}))
    for material, info in report['detail'].items():
        print(f'{material}: {info["islands"]} islands {info["roles"]}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
