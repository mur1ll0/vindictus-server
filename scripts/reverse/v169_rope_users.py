"""Where the 1.69 lab client.dll creates cables (read only; Miri M7f).

The NA draws the Draker's chain as a cable from the player's `anim_attachment_LH_Chain1` to the
weapon's `chain_01` (na_x64_draker_weapon.py). The 1.69 has no Draker, but its client already makes
chain cables for the Vella's dual blades (`C_HeroesPlayerVella`, `m_hChainL`/`m_hChainR`,
`cable/heroes_metalchain_01.vmt`) and for mounts (`LINK_ROPE`). This probe finds the absolute
references to given strings (x86 code embeds the address: push imm32, mov reg, imm32), and lists the
code around each one, so the cable creation call and its arguments can be read without disassembling
the whole binary.

Usage: python -I scripts/reverse/v169_rope_users.py [--dll client.dll] [--around N] text ... [--at VA ...]
"""
import argparse
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import v169_class_uses as v169  # noqa: E402

BACKUPS = {'client.dll': 'client.dll.0a0b866dcc0a1852.pre-routing.bak', 'server.dll': 'server.dll.c85a56d5b845810f.pre-routing.bak'}


def string_vas(image, text):
    needle = text.encode() + b'\0'
    out, at = [], 0
    while True:
        at = image.data.find(needle, at)
        if at < 0:
            return out
        if at == 0 or image.data[at - 1] == 0:
            out.append(image.base + at)
        at += 1


def references(image, va):
    lo, hi = image.text
    return [image.base + lo + m.start() for m in re.finditer(re.escape(struct.pack('<I', va)), image.data[lo:hi])]


def listing(image, va, before, after):
    """Instructions from a function start found by walking back to int3/ret padding, up to after."""
    rva = va - image.base
    start = rva - before
    pad = image.data.rfind(b'\xcc\xcc', start, rva)
    if pad >= 0:
        start = pad + 2
        while image.data[start] == 0xCC:
            start += 1
    lines = []
    for i in image.disasm(image.base + start, rva - start + after):
        text = f'{i.address:#x} {i.mnemonic} {i.op_str}'
        for word in re.findall(r'0x[0-9a-f]{8}', i.op_str):
            target = int(word, 16)
            if image.base <= target < image.base + len(image.data):
                s = v169_string(image, target)
                if s:
                    text += f'    ; "{s}"'
        lines.append(text)
    return lines


def v169_string(image, va):
    rva = va - image.base
    raw = image.data[rva:rva + 96]
    end = raw.find(b'\0')
    return raw[:end].decode() if end > 3 and all(32 <= c < 127 for c in raw[:end]) else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--dll', default='client.dll')
    parser.add_argument('--around', type=lambda x: int(x, 0), default=0x80)
    parser.add_argument('--at', nargs='*', default=[], type=lambda x: int(x, 16))
    parser.add_argument('texts', nargs='*')
    args = parser.parse_args()
    image = v169.Image(ROOT / '.cache/play-client/bin' / BACKUPS[args.dll])
    for text in args.texts:
        for va in string_vas(image, text):
            refs = references(image, va)
            print(f'== "{text}" at {va:#x}: {len(refs)} references {[hex(r) for r in refs[:8]]}')
            for ref in refs[:4]:
                print('\n'.join(listing(image, ref, 0x400, args.around)))
                print('--')
    for va in args.at:
        print(f'== {va:#x}')
        print('\n'.join(listing(image, va, 0x10, args.around)))


if __name__ == '__main__':
    main()
