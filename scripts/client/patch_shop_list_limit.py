"""Raise the in-game cash shop ShopList download limit in a COPY of client.dll. Dry-run by default.

InGameCashShopDialog_V2 downloads ingame_cashshop_shopListURL with `push 0x100000`
(1 MiB) at VA 0x38479247 and 0x38479306 (file offsets 0x478647 and 0x478706 of the
1.69 EN client.dll, SHA-256 b284ba71...). A larger response never reaches the parser.
Only those two immediates change (to 16 MiB); the original client stays read-only and
the unpatched copy is kept next to the patched file.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

SOURCE_SHA256 = 'b284ba719047556b8468ff58b82d93578de6a7b46a11e33709b4dc42f1bebb02'
OFFSETS = (0x478647, 0x478706)
ORIGINAL = bytes.fromhex('6800001000')  # push 0x100000
PATCHED = bytes.fromhex('6800000001')  # push 0x1000000


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def patched_bytes(data: bytes, source_sha256: str = SOURCE_SHA256) -> tuple[bytes, bool]:
    """Return the patched image and whether it was already patched."""
    sites = [data[o:o + len(ORIGINAL)] for o in OFFSETS]
    if all(s == PATCHED for s in sites):
        return data, True
    if digest(data) != source_sha256 or any(s != ORIGINAL for s in sites):
        raise PreparationError('client.dll is not the recovered 1.69 EN build; nothing is patched.')
    image = bytearray(data)
    for o in OFFSETS:
        image[o:o + len(PATCHED)] = PATCHED
    return bytes(image), False


def patch(client_copy: Path, apply: bool = False) -> dict:
    dll = reject_original(client_copy / 'bin/client.dll')
    if not dll.is_file():
        raise PreparationError('The client copy must contain bin/client.dll.')
    data = dll.read_bytes()
    image, already = patched_bytes(data)
    backup = dll.with_name('client.dll.' + SOURCE_SHA256[:16] + '.bak')
    plan = {'mode': 'apply' if apply else 'dry-run', 'client_dll': str(dll), 'already_patched': already,
            'before_sha256': digest(data), 'after_sha256': digest(image), 'limit_bytes': 0x1000000,
            'backup': str(backup)}
    if apply and not already:
        if not backup.exists():
            backup.write_bytes(data)
        partial = dll.with_suffix('.partial')
        partial.write_bytes(image)
        os.replace(partial, dll)
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(patch(args.client_copy, args.apply)))
    except PreparationError as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
