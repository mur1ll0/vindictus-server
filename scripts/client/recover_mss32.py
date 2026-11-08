"""Mss32.dll of the 1.69 EU client, recovered from its distribution archive into a COPY of the client.
Dry-run by default.

The Mss32.dll in Vindictus1.69EN/Vindictus EU/en-EU (dfc4e0c9...) has no MZ/PE header and fails to load
("Bad image"); the archive the copy came from (Vindictus.exe + .z01-.z03, a split ZIP) holds the good one
(docs/evidence/client/runtime-ingress.json): local header of en-EU\\Mss32.dll on disk 3 (Vindictus.z03)
at offset 1544037482, deflate 174720 -> 374272 bytes, CRC32 b67c7f6c, SHA-256 f2099aaa... The volume is
only read; the header, sizes, CRC and SHA-256 are all checked before anything is written.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

from prepare_lab_client import WORKSPACE, PreparationError, reject_original

VOLUME = WORKSPACE / 'Vindictus1.69EN/Vindictus.z03'
OFFSET = 1544037482
# The server's import (scripts/dev/import-client-base.ps1) keeps only this record of the 1.9 GB volume: the
# bytes from OFFSET on. Header, sizes, CRC and SHA-256 are checked the same way.
SLICE = WORKSPACE / 'Vindictus1.69EN/Vindictus.z03.mss32'
SLICE_BYTES = 256 * 1024
if not VOLUME.is_file() and SLICE.is_file():
    VOLUME, OFFSET = SLICE, 0
NAME = b'en-EU\\Mss32.dll'
CRC32 = 0xB67C7F6C
COMPRESSED, SIZE = 174720, 374272
SHA256 = 'f2099aaa3d7a72375f7e6dd8430ebdd3ab11240a28323642a6330d92a9cec28b'
BROKEN_SHA256 = 'dfc4e0c9bf06f460fca34c305a7dfac3952124555604ffa01693b2786b038657'
HEADER = struct.Struct('<4sHHHHHIIIHH')


def recover(volume: Path = VOLUME, offset: int = OFFSET) -> bytes:
    with volume.open('rb') as f:
        f.seek(offset)
        head = f.read(HEADER.size + len(NAME))
        sig, _, _, method, _, _, crc, csize, usize, nlen, xlen = HEADER.unpack_from(head)
        name = head[HEADER.size:HEADER.size + nlen]
        if (sig, method, crc, csize, usize, name) != (b'PK\x03\x04', 8, CRC32, COMPRESSED, SIZE, NAME):
            raise PreparationError(f'No {NAME.decode()} local header at {offset} of {volume.name}.')
        f.seek(offset + HEADER.size + nlen + xlen)
        compressed = f.read(csize)
    data = zlib.decompress(compressed, -15)
    if len(data) != SIZE or zlib.crc32(data) != CRC32 or hashlib.sha256(data).hexdigest() != SHA256 or data[:2] != b'MZ':
        raise PreparationError('The recovered Mss32.dll differs from the recorded one.')
    return data


def apply(client_copy: Path, volume: Path = VOLUME, apply_changes: bool = False) -> dict:
    target = reject_original(client_copy) / 'Mss32.dll'
    data = recover(volume)
    before = hashlib.sha256(target.read_bytes()).hexdigest() if target.is_file() else None
    if apply_changes and before != SHA256:
        target.write_bytes(data)
    return {'mode': 'apply' if apply_changes else 'dry-run', 'target': str(target), 'before_sha256': before,
            'after_sha256': SHA256, 'changed': before != SHA256}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--volume', type=Path, default=VOLUME, help='disk 3 of the 1.69 EU archive (read only)')
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(apply(args.client_copy, args.volume, args.apply)))
    except (PreparationError, OSError, zlib.error) as error:
        print(json.dumps({'applied': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
