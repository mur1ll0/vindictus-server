"""Generate the launcher icon (icons/icon.ico): a dark red rounded square with a white "V".

Our own drawing, no game asset. Run with `py -3.13 -I make_icon.py` in this folder; the output is
deterministic. 16, 32 and 48 px, 32-bit BGRA DIBs with an AND mask, which Windows 7 reads.
"""
import struct
from pathlib import Path


def pixel(x: float, y: float, size: int) -> tuple[int, int, int, int]:
    u, v = x / size, y / size
    radius = 0.18
    cx = min(max(u, radius), 1 - radius)
    cy = min(max(v, radius), 1 - radius)
    if (u - cx) ** 2 + (v - cy) ** 2 > radius ** 2:
        return (0, 0, 0, 0)
    # "V": two strokes from the top corners to the bottom centre.
    def near(ax, ay, bx, by, width):
        dx, dy = bx - ax, by - ay
        t = max(0.0, min(1.0, ((u - ax) * dx + (v - ay) * dy) / (dx * dx + dy * dy)))
        px, py = ax + t * dx, ay + t * dy
        return (u - px) ** 2 + (v - py) ** 2 <= width ** 2
    if near(0.24, 0.22, 0.5, 0.8, 0.075) or near(0.76, 0.22, 0.5, 0.8, 0.075):
        return (255, 255, 255, 255)
    shade = int(110 + 50 * (1 - v))
    return (shade, 24, 28, 255)


def image(size: int) -> bytes:
    rows = []
    for y in range(size - 1, -1, -1):  # DIBs are bottom-up
        row = bytearray()
        for x in range(size):
            samples = [pixel(x + dx, y + dy, size) for dx in (0.25, 0.75) for dy in (0.25, 0.75)]
            r, g, b, a = (sum(s[i] for s in samples) // 4 for i in range(4))
            row += bytes((b, g, r, a))
        rows.append(bytes(row))
    header = struct.pack('<IiiHHIIiiII', 40, size, size * 2, 1, 32, 0, size * size * 4, 0, 0, 0, 0)
    mask_row = ((size + 31) // 32) * 4
    return header + b''.join(rows) + bytes(mask_row * size)


def main() -> None:
    sizes = (16, 32, 48)
    images = [image(s) for s in sizes]
    out = bytearray(struct.pack('<HHH', 0, 1, len(sizes)))
    offset = 6 + 16 * len(sizes)
    for size, data in zip(sizes, images):
        out += struct.pack('<BBBBHHII', size, size, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    for data in images:
        out += data
    Path(__file__).with_name('icon.ico').write_bytes(out)


if __name__ == '__main__':
    main()
