"""Valve texture (.vtf 7.x): the high resolution image of the first frame as RGBA8 rows (top row first),
and a writer of uncompressed BGRA8888 textures with a full mipmap chain. Needs numpy (Blender ships it)."""
from __future__ import annotations
import struct
import numpy as np

RGBA8888, ABGR8888, RGB888, BGR888, I8, IA88, A8, BGRA8888, DXT1, DXT3, DXT5, BGRX8888 = 0, 1, 2, 3, 5, 6, 8, 12, 13, 14, 15, 16
BYTES = {RGBA8888: 4, ABGR8888: 4, RGB888: 3, BGR888: 3, I8: 1, IA88: 2, A8: 1, BGRA8888: 4, BGRX8888: 4}
BLOCK = {DXT1: 8, DXT3: 16, DXT5: 16}
FLAG_EIGHTBITALPHA = 0x2000


class VtfError(ValueError):
    pass


def level_size(fmt: int, w: int, h: int) -> int:
    if fmt in BLOCK:
        return max(1, (w + 3) // 4) * max(1, (h + 3) // 4) * BLOCK[fmt]
    if fmt in BYTES:
        return w * h * BYTES[fmt]
    raise VtfError(f'Unsupported VTF image format {fmt}.')


def _colors565(c: np.ndarray) -> np.ndarray:
    r = ((c >> 11) & 31) * 255 // 31
    g = ((c >> 5) & 63) * 255 // 63
    b = (c & 31) * 255 // 31
    return np.stack([r, g, b], -1).astype(np.int32)


def _dxt_color(blocks: np.ndarray, dxt1: bool) -> tuple[np.ndarray, np.ndarray]:
    c0 = blocks[:, 0].astype(np.uint32) | (blocks[:, 1].astype(np.uint32) << 8)
    c1 = blocks[:, 2].astype(np.uint32) | (blocks[:, 3].astype(np.uint32) << 8)
    p0, p1 = _colors565(c0), _colors565(c1)
    four = (c0 > c1) | (not dxt1)
    p2 = np.where(four[:, None], (2 * p0 + p1) // 3, (p0 + p1) // 2)
    p3 = np.where(four[:, None], (p0 + 2 * p1) // 3, 0)
    palette = np.stack([p0, p1, p2, p3], 1)                        # n,4,3
    alpha_black = dxt1 & ~four
    bits = blocks[:, 4:8].astype(np.uint32)
    code = bits[:, 0] | (bits[:, 1] << 8) | (bits[:, 2] << 16) | (bits[:, 3] << 24)
    idx = np.stack([(code >> (2 * i)) & 3 for i in range(16)], 1)  # n,16
    rgb = np.take_along_axis(palette, idx[:, :, None].astype(np.int64), 1)
    alpha = np.where(alpha_black[:, None] & (idx == 3), 0, 255)
    return rgb, alpha


def _dxt5_alpha(blocks: np.ndarray) -> np.ndarray:
    a0, a1 = blocks[:, 0].astype(np.int32), blocks[:, 1].astype(np.int32)
    table = [a0, a1]
    eight = a0 > a1
    for i in range(1, 7):
        table.append(np.where(eight, ((7 - i) * a0 + i * a1) // 7, ((5 - i) * a0 + i * a1) // 5 if i < 5 else 0))
    table[6] = np.where(eight, table[6], 0)
    table[7] = np.where(eight, table[7], 255)
    table = np.stack(table, 1)
    bits = np.zeros(len(blocks), dtype=np.uint64)
    for i in range(6):
        bits |= blocks[:, 2 + i].astype(np.uint64) << np.uint64(8 * i)
    idx = np.stack([(bits >> np.uint64(3 * i)) & np.uint64(7) for i in range(16)], 1).astype(np.int64)
    return np.take_along_axis(table, idx, 1)


def decode(fmt: int, w: int, h: int, data: bytes) -> np.ndarray:
    if fmt in BLOCK:
        bw, bh = max(1, (w + 3) // 4), max(1, (h + 3) // 4)
        raw = np.frombuffer(data, np.uint8)[:bw * bh * BLOCK[fmt]].reshape(bw * bh, BLOCK[fmt])
        if fmt == DXT1:
            rgb, alpha = _dxt_color(raw, True)
        else:
            rgb, _ = _dxt_color(raw[:, 8:], False)
            if fmt == DXT5:
                alpha = _dxt5_alpha(raw[:, :8])
            else:
                nib = np.unpackbits(raw[:, :8], axis=1, bitorder='little').reshape(-1, 16, 4)
                alpha = (nib * np.array([1, 2, 4, 8])).sum(-1) * 17
        px = np.concatenate([rgb, alpha[:, :, None]], -1).reshape(bh, bw, 4, 4, 4).transpose(0, 2, 1, 3, 4)
        return px.reshape(bh * 4, bw * 4, 4)[:h, :w].astype(np.uint8)
    n = BYTES[fmt]
    a = np.frombuffer(data, np.uint8)[:w * h * n].reshape(h, w, n)
    order = {RGBA8888: [0, 1, 2, 3], ABGR8888: [3, 2, 1, 0], BGRA8888: [2, 1, 0, 3], BGRX8888: [2, 1, 0]}
    if fmt in (RGB888, BGR888):
        rgb = a if fmt == RGB888 else a[:, :, ::-1]
        return np.concatenate([rgb, np.full((h, w, 1), 255, np.uint8)], -1)
    if fmt in (I8, A8, IA88):
        i = a[:, :, 0]
        alpha = a[:, :, 1] if fmt == IA88 else (i if fmt == A8 else np.full_like(i, 255))
        lum = np.full_like(i, 255) if fmt == A8 else i
        return np.stack([lum, lum, lum, alpha], -1)
    rgba = a[:, :, order[fmt]]
    if fmt == BGRX8888:
        rgba = np.concatenate([rgba, np.full((h, w, 1), 255, np.uint8)], -1)
    return np.ascontiguousarray(rgba)


def read(data: bytes) -> tuple[np.ndarray, dict]:
    if data[:4] != b'VTF\0':
        raise VtfError('Not a VTF texture.')
    major, minor, header_size, w, h, flags, frames = struct.unpack_from('<IIIHHIH', data, 4)
    fmt, mips, low_fmt, low_w, low_h = struct.unpack_from('<iBiBB', data, 52)
    if major != 7 or minor > 2:
        raise VtfError(f'Unsupported VTF version {major}.{minor}.')
    at = header_size + (level_size(low_fmt, low_w, low_h) if low_fmt >= 0 and low_w and low_h else 0)
    for level in range(mips - 1, 0, -1):
        at += level_size(fmt, max(1, w >> level), max(1, h >> level)) * frames
    image = decode(fmt, w, h, data[at:at + level_size(fmt, w, h)])
    return image, {'format': fmt, 'flags': flags, 'width': w, 'height': h, 'mipmaps': mips}


def write(rgba: np.ndarray, flags: int = 0) -> bytes:
    """Uncompressed BGRA8888 with all mipmaps (smallest first), no low resolution thumbnail."""
    h, w = rgba.shape[:2]
    if w & (w - 1) or h & (h - 1):
        raise VtfError('VTF sides must be powers of two.')
    levels = [rgba.astype(np.float32)]
    while levels[-1].shape[0] > 1 or levels[-1].shape[1] > 1:
        a = levels[-1]
        hh, ww = max(1, a.shape[0] // 2), max(1, a.shape[1] // 2)
        a = a[:hh * 2 if a.shape[0] > 1 else 1, :ww * 2 if a.shape[1] > 1 else 1]
        a = a.reshape(hh, a.shape[0] // hh, ww, a.shape[1] // ww, 4).mean((1, 3))
        levels.append(a)
    alpha = bool((rgba[:, :, 3] < 255).any())
    flags = (flags & ~0x3000) | (FLAG_EIGHTBITALPHA if alpha else 0)
    header = bytearray(80)
    struct.pack_into('<4sIIIHHIHH', header, 0, b'VTF\0', 7, 2, 80, w, h, flags, 1, 0)
    struct.pack_into('<3f', header, 32, 0.5, 0.5, 0.5)
    struct.pack_into('<fiBiBBH', header, 48, 1.0, BGRA8888, len(levels), -1, 0, 0, 1)
    body = b''.join(np.clip(np.rint(level), 0, 255).astype(np.uint8)[:, :, [2, 1, 0, 3]].tobytes() for level in reversed(levels))
    return bytes(header) + body
