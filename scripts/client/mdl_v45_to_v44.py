"""Convert NA studio models (IDST version 45) to the version 44 layout the 1.69 engine ships.

Measured on pc_delia_hair_crystal_rose_04.mdl (NA v45 against 1.69 v44, docs/future/miri-class-port.md
2.2): both share the 408-byte studiohdr_t; version 45 inserts a 260-byte block (MAX_PATH, the model's
long path, zero padded) right after it, so every absolute offset of the header is 260 larger. The
conversion drops the block, moves those offsets back and fixes the few places that hold offsets from
the start of the file instead of from their own structure:
  - header fields up to bonetablebynameindex (bones, sequences, textures, strings...), when they point
    past the block;
  - the cdtexture and local node name tables (arrays of absolute string offsets);
  - baseptr of every sequence and animation description (negative offset back to the header).
The .vvd/.vtx/.phy files keep their versions (the same as 1.69) and the model checksum is untouched.

  mdl_v45_to_v44.py <in.mdl> <out.mdl>
"""
from __future__ import annotations
import argparse
import struct
import sys
from pathlib import Path

HEADER = 408
BLOCK = 260
MAGIC = b'IDST'
VERSION_FROM, VERSION_TO = 45, 44
LENGTH_AT = 76

# studiohdr_t field offsets (Source 2006/2007 layout, shared by both versions).
COUNT_INDEX = {  # name: (count offset or None, index offset)
    'bone': (156, 160), 'bonecontroller': (164, 168), 'hitboxset': (172, 176), 'localanim': (180, 184),
    'localseq': (188, 192), 'texture': (204, 208), 'cdtexture': (212, 216), 'skin': (None, 228),
    'bodypart': (232, 236), 'localattachment': (240, 244), 'localnode': (None, 252), 'localnodename': (248, 256),
    'flexdesc': (260, 264), 'flexcontroller': (268, 272), 'flexrule': (276, 280), 'ikchain': (284, 288),
    'mouth': (292, 296), 'localposeparam': (300, 304), 'surfaceprop': (None, 308), 'keyvalue': (None, 312),
    'localikautoplaylock': (320, 324), 'includemodel': (336, 340), 'szanimblockname': (None, 348),
    'animblock': (352, 356), 'bonetablebyname': (None, 364),
}
# From +368 the Vindictus header has its own fields (a 'SCAL' tag at +388, 1.0f at +392), equal in both
# versions of the measured pairs: they are not offsets and stay as they are.
SEQUENCE_STRIDES = range(16, 1025, 4)


class ConversionError(ValueError):
    pass


def i32(data: bytes | bytearray, at: int) -> int:
    return struct.unpack_from('<i', data, at)[0]


def put(data: bytearray, at: int, value: int) -> None:
    struct.pack_into('<i', data, at, value)


def is_v45(data: bytes) -> bool:
    return len(data) >= HEADER + BLOCK and data[:4] == MAGIC and i32(data, 4) == VERSION_FROM


def baseptr_stride(data: bytes | bytearray, index: int, count: int) -> int:
    """Size of a sequence/animation description, found from their baseptr (-offset of each entry). The
    descriptions end before the next section the header points at."""
    if count < 2:
        return 0
    following = [i32(data, at) for _count, at in COUNT_INDEX.values() if i32(data, at) > index]
    limit = min(following, default=len(data)) - index
    for stride in SEQUENCE_STRIDES:
        if stride * count > limit:
            break
        if all(i32(data, index + k * stride) == -(index + k * stride) for k in range(count)):
            return stride
    raise ConversionError('Sequence or animation descriptions without a consistent baseptr.')


def convert(data: bytes) -> bytes:
    if not is_v45(data):
        raise ConversionError('Not an IDST version 45 model.')
    if i32(data, LENGTH_AT) != len(data):
        raise ConversionError('The header length does not match the file size.')
    block = data[HEADER:HEADER + BLOCK]
    end = block.find(b'\0')
    if end <= 0 or any(block[end:]) or any(c < 0x20 or c > 0x7e for c in block[:end]):
        raise ConversionError('The version 45 block is not a zero-padded model path.')
    old = bytearray(data)
    # The descriptions must already be consistent in the source (baseptr == -offset).
    strides = {}
    for name in ('localseq', 'localanim'):
        count_at, index_at = COUNT_INDEX[name]
        count, index = i32(old, count_at), i32(old, index_at)
        if count:
            if index < HEADER + BLOCK or i32(old, index) != -index:
                raise ConversionError(f'{name} descriptions do not start with their baseptr.')
            strides[name] = baseptr_stride(old, index, count)
    out = bytearray(old[:HEADER] + old[HEADER + BLOCK:])
    put(out, 4, VERSION_TO)
    put(out, LENGTH_AT, len(out))
    for name, (count_at, index_at) in COUNT_INDEX.items():
        value = i32(old, index_at)
        if value >= HEADER + BLOCK:
            put(out, index_at, value - BLOCK)
        elif HEADER < value < HEADER + BLOCK or (value == HEADER and count_at is not None and i32(old, count_at)):
            raise ConversionError(f'{name} points into the version 45 block.')
    for name in ('cdtexture', 'localnodename'):
        count_at, index_at = COUNT_INDEX[name]
        count, table = i32(out, count_at), i32(out, index_at)
        for k in range(count if table else 0):
            at = table + 4 * k
            value = i32(out, at)
            if value >= HEADER + BLOCK:
                put(out, at, value - BLOCK)
    for name in ('localseq', 'localanim'):
        count_at, index_at = COUNT_INDEX[name]
        count, index = i32(out, count_at), i32(out, index_at)
        for k in range(count):
            at = index + k * strides.get(name, 0)
            put(out, at, -at)
    return bytes(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('source', type=Path)
    parser.add_argument('target', type=Path)
    args = parser.parse_args()
    try:
        args.target.write_bytes(convert(args.source.read_bytes()))
    except (OSError, ConversionError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
