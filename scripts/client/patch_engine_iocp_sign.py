"""Stop engine.dll's ProudNet client from dropping I/O completions above 2 GB in a COPY. Dry-run by default.

The client's completion getter (0x425fd2e0, GetQueuedCompletionStatus through the function table
of 0x425bd900) tests the OVERLAPPED pointer as a signed int: `cmp eax, ebx; jge 0x425fd3d3` at
0x425fd3a2. Only a non-negative pointer is matched against the socket's receive (+0xc0), send
(+0x120), +0x60 and +0x1c0 OVERLAPPEDs; a negative one becomes a custom event (type 0). The lab
Vindictus.exe is large-address-aware, so once the low 2 GB are used up (about 2 GB private memory
since the Miri content) a new CTcpLayer_C can land above 0x80000000. Its first receive completion,
the relay's connection hint, is then dropped: the client never sends its public key, closes the
TCP connection 10 s later and waits 60 s (connection) plus 100 s (NetClient::Disconnect, the
receive still counted as pending) before giving up on the ship (C4 MI5, B2 RL1).

The `jge` becomes `jmp`: every completion goes through the pointer comparisons. A value that
matches none of them still becomes type 0 with the same fields (the getter sets them before the
test, and the non-matching path never dereferences the key), so custom events keep working.
"""
from __future__ import annotations
import argparse
import json
import os
import sys
from pathlib import Path

from patch_class_table import Image, digest
from prepare_lab_client import PreparationError, reject_original

SITE = 0x425fd3a2
# cmp eax, ebx (ebx = 0); jge 0x425fd3d3 (match the socket's OVERLAPPEDs)
ORIGINAL = bytes.fromhex('3bc37d2d')
# cmp eax, ebx; jmp 0x425fd3d3
PATCHED = bytes.fromhex('3bc3eb2d')
MATCH = 0x425fd3d3


def plan(data: bytes) -> tuple[bytes, bool]:
    """Return the patched image and whether it was already patched."""
    image = Image(data)
    o = image.offset(SITE)
    site = bytes(image.data[o:o + len(ORIGINAL)])
    if site == PATCHED:
        return data, True
    if site != ORIGINAL:
        raise PreparationError(f'Unexpected bytes at {SITE:#x}; nothing is patched.')
    image.data[o:o + len(PATCHED)] = PATCHED
    return bytes(image.data), False


def patch(client_copy: Path, apply: bool = False) -> dict:
    dll = reject_original(client_copy / 'bin' / 'engine.dll')
    if not dll.is_file():
        raise PreparationError('The client copy must contain bin/engine.dll.')
    data = dll.read_bytes()
    image, already = plan(data)
    backup = dll.with_name(f'engine.dll.{digest(data)[:16]}.pre-iocpsign.bak')
    result = {'mode': 'apply' if apply else 'dry-run', 'dll': str(dll), 'already_patched': already,
              'before_sha256': digest(data), 'after_sha256': digest(image), 'site': hex(SITE),
              'backup': str(backup)}
    if apply and not already:
        if not backup.exists():
            backup.write_bytes(data)
        partial = dll.with_suffix('.partial')
        partial.write_bytes(image)
        os.replace(partial, dll)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(patch(args.client_copy, args.apply), indent=1))
    except PreparationError as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
