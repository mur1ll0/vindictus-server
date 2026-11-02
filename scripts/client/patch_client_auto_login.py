"""Let client.dll log in with the passport the launcher hands over, without the login dialog, in a COPY.

Dry-run by default.

When the title's Start Game is clicked, the main-menu step 0x385232f0 picks what comes next. With
`-mantis` it returns state 0x29 (the Nexon Launcher path). Otherwise, only when the FeatureMatrix
event modes `E3_Event_2010` or `Event_PAX_2011_enUS` are on, it reads the passport: with
`NGM_disable` from the `event_account` ConVar (0x38e5d9c8, registered at 0x38aa5670, empty by
default; the login dialog also writes the typed ID there, 0x3852245d), without it from the NMCO
manager. A non-empty passport goes straight to the login interface (`[+8](passport)`) and the step
returns 2, skipping `LoginDialog_V2`; an empty one, or a failed call, opens the dialog.

The event modes change about 66 other places in client.dll (and engine/server), so they stay off.
This patch only removes their gate at 0x385233fe (`je 0x385234e6`, both modes off → dialog) with a
6-byte NOP. Effect: an empty `event_account` still opens the login dialog as before; a launcher that
starts the game with `+event_account <ticket>` gets Start Game → character selection.
"""
from __future__ import annotations
import argparse
import json
import os
import sys
from pathlib import Path

from patch_class_table import Image, digest
from prepare_lab_client import PreparationError, reject_original

SITE = 0x385233fe
# je 0x385234e6 (neither event mode: show LoginDialog_V2)
ORIGINAL = bytes.fromhex('0f84e2000000')
# nop word ptr [eax + eax] (fall through to the event_account passport read)
PATCHED = bytes.fromhex('660f1f440000')
DIALOG = 0x385234e6
PASSPORT_READ = 0x38523404


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
    dll = reject_original(client_copy / 'bin' / 'client.dll')
    if not dll.is_file():
        raise PreparationError('The client copy must contain bin/client.dll.')
    data = dll.read_bytes()
    image, already = plan(data)
    backup = dll.with_name(f'client.dll.{digest(data)[:16]}.pre-autologin.bak')
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
