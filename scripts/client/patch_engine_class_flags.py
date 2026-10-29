"""Let items allow class 10 (Delia) in a COPY of engine.dll. Dry-run by default.

engine.dll reads ItemClassInfo.ClassRestriction (an int mask) and unpacks it into one bool per
class at item +0x198 (setter 0x42245110, its only caller 0x422be9f8). The record has room for ten
classes only (+0x198..+0x1a1): the loader writes Indestructible to +0x1a2 right after the setter
(0x422bea4a), so every item denies class 10 and the client shows Delia's items as "Cannot be
equipped" and red (client tests D8c, D22). Delia's flag goes to DELIA_FLAG, the padding byte after
UseInQuest (+0x1cc), which nothing in engine.dll or server.dll reads or writes. The setter's tail
(bits 8 and 9, 0x42245170..0x4224518f) is rewritten in place to also store bit 10 there; the
function's early exit (`je` at 0x42245116) moves to the new `ret 4`. No new section, no relocated
dword touched.
"""
from __future__ import annotations
import argparse
import json
import os
import sys
from pathlib import Path

from patch_class_table import Image, digest
from prepare_lab_client import PreparationError, reject_original

EXPECTED = 'a3add9742f7fa03c'
TAIL = 0x42245170
# test edx,0x100; je; mov [ecx+0x1a0],al; test edx,0x200; je; mov [ecx+0x1a1],al; ret 4; int3
OLD_TAIL = bytes.fromhex('f7c20001000074068881a0010000f7c20002000074068881a1010000c20400cc')
DELIA_FLAG = 0x1CD  # item byte for class 10 (padding after UseInQuest)
# al is 1 here (0x42245118). add ecx,0x1a0; bits 8, 9 of edx (dh) to [ecx], [ecx+1]; bit 10 to
# [ecx + DELIA_FLAG - 0x1a0] by setne (0 or 1); ret 4 at TAIL + 29.
NEW_TAIL = bytes.fromhex('81c1a0010000' 'f6c601' '7402' '8801' 'f6c602' '7403' '884101'
                         'f6c604' '0f9541' f'{DELIA_FLAG - 0x1A0:02x}' '90' 'c20400')
EXIT = 0x42245116
OLD_EXIT = bytes.fromhex('7474')  # je 0x4224518c (mask 0)


def plan(data: bytes) -> tuple[bytes, dict]:
    image = Image(data)
    ret = TAIL + len(NEW_TAIL) - 3
    new_exit = bytes((0x74, ret - (EXIT + 2)))
    for va, old in ((TAIL, OLD_TAIL), (EXIT, OLD_EXIT)):
        o = image.offset(va)
        if bytes(image.data[o:o + len(old)]) != old:
            raise PreparationError(f'Unexpected bytes at {va:#x}; nothing is patched.')
    if len(NEW_TAIL) != len(OLD_TAIL):
        raise PreparationError('The new tail must fit the old one exactly.')
    for va, new in ((TAIL, NEW_TAIL), (EXIT, new_exit)):
        o = image.offset(va)
        image.data[o:o + len(new)] = new
    return bytes(image.data), {'tail': hex(TAIL), 'ret': hex(ret), 'exit': f'{EXIT:#x} -> {ret:#x}',
                               'flags': f'+0x198..+0x1a1 (classes 0-9), {DELIA_FLAG:#x} (class 10)'}


def patch(client_copy: Path, apply: bool = False) -> dict:
    dll = reject_original(client_copy / 'bin' / 'engine.dll')
    data = dll.read_bytes()
    if not digest(data).startswith(EXPECTED):
        raise PreparationError(f'engine.dll: expected {EXPECTED}…, found {digest(data)[:16]}.')
    image, report = plan(data)
    backup = dll.with_name(f'engine.dll.{digest(data)[:16]}.pre-classflags.bak')
    result = {'mode': 'apply' if apply else 'dry-run', 'dll': str(dll), 'before_sha256': digest(data),
              'after_sha256': digest(image), 'backup': str(backup), **report}
    if apply:
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
