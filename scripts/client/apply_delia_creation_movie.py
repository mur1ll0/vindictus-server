"""The Delia's customization movie (class layer v2, scripts/client/class_layers.py) in a COPY of the client.
Dry-run by default.

The client reads the loose scaleform/ files before the packs. The 1.69's loose CreateCharacterDialog.gfx
(2016, 1a51f689...) has no Back button nor the CharacterCreation_UI_V3 branch the Delia's creation flow
uses (docs/history/session-log.md, C3-12..C3-19); the newer movie, and the three images of it that also
exist loose with older content, are in the 1.69's renamed pack RENOMEADo_9FB527B3... (not read by the
game). This script copies them out of that pack into scaleform/ and edits the movie as the C3 steps did:

- C3-14, V3 branch of onShow (same-size swaps): 0xe425 false -> true (name field, its text and Check
  shown), 0xe45b true -> false (t_Notic hidden), 0xe464 `push r0` -> `push true` (Button_Back shown);
- C3-15, Text_Name (DefineEditText) colour: 0xf381 RGB cccccc -> ffffff and the initial HTML `color`
  attribute at 0xf3ca "#cccccc" -> "#ffffff";
- C3-17, onKillFocus: the constant 6710886 (0x666666) at 0xd680 -> 16777215 (0xffffff).

Each source and the result are checked by SHA-256. In v3, apply_lab_content_additions.apply_loose_fixes
then adds the Miri's slider aliases on top of this movie.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import sys
from pathlib import Path

import hfs
from prepare_lab_client import PreparationError, reject_original

PACK = 'hfs/RENOMEADo_9FB527B373DCBD159552ADD1710487A724237AF9.hfs'
MOVIE = 'CreateCharacterDialog.gfx'
MOVIE_SOURCE = 'd9f2dd66a109ed1821d682781322a4360e9f0cf3b4dfad8eb500403134ab5665'
MOVIE_RESULT = 'ac5543b072761a3f648f5ba98d6c9316d331173dfb2d52343835425a662c59ec'
# (offset, original bytes, new bytes, step)
MOVIE_EDITS = (
    (0xD680, '666666', 'ffffff', 'C3-17 onKillFocus colour constant'),
    (0xE425, '00', '01', 'C3-14 name field shown'),
    (0xE45B, '01', '00', 'C3-14 t_Notic hidden'),
    (0xE464, '0400', '0501', 'C3-14 Button_Back shown (push true)'),
    (0xF381, 'cccccc', 'ffffff', 'C3-15 Text_Name colour'),
    (0xF3CA, '636363636363', '666666666666', 'C3-15 Text_Name initial HTML colour'),
)
# Images copied as they are in the pack (C3-13, C3-19).
IMAGES = {
    'CreateCharacterDialog_I30.dds': '04084564e97b0e1b7d3b7e5d1948a9523ba46ff215afef18a414e0d9d6670d2f',
    'CreateCharacterDialog_I5.dds': '04642050b7b59467d4fde6d1bd4da8d0c790f4bbc36d7f047dd2dc2e49064fde',
    'CreateCharacterDialog_I89.dds': '9a73326ff1e02975ea2afe609c6f79f29ad07ac9a3f328485d20ec8864858a81',
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def edited_movie(data: bytes) -> bytes:
    if digest(data) != MOVIE_SOURCE:
        raise PreparationError(f'{MOVIE} of the renamed pack is {digest(data)[:16]}, expected {MOVIE_SOURCE[:16]}.')
    out = bytearray(data)
    for at, old, new, _ in MOVIE_EDITS:
        old, new = bytes.fromhex(old), bytes.fromhex(new)
        if out[at:at + len(old)] != old or len(old) != len(new):
            raise PreparationError(f'Unexpected bytes at {at:#x} of {MOVIE}.')
        out[at:at + len(new)] = new
    if digest(bytes(out)) != MOVIE_RESULT:
        raise PreparationError(f'The edited {MOVIE} is {digest(bytes(out))[:16]}, expected {MOVIE_RESULT[:16]}.')
    return bytes(out)


def files(pack: bytes, keystream: bytes) -> dict[str, bytes]:
    """scaleform/<name> -> bytes: the edited movie and the three images, from the renamed pack."""
    entries = {}
    for e in hfs.read_pack(pack, keystream):
        name = e.name[:-5] if e.name.lower().endswith('.comp') else e.name
        entries[name.lower()] = hfs.comp_unwrap(e.data) if e.name.lower().endswith('.comp') else e.data
    out = {}
    for name, sha in IMAGES.items():
        data = entries.get(name.lower())
        if data is None or digest(data) != sha:
            raise PreparationError(f'{name} of the renamed pack is missing or differs.')
        out['scaleform/' + name] = data
    if MOVIE.lower() not in entries:
        raise PreparationError(f'The renamed pack has no {MOVIE}.')
    out['scaleform/' + MOVIE] = edited_movie(entries[MOVIE.lower()])
    return out


def apply(client_copy: Path, keystream: Path, apply_changes: bool = False) -> dict:
    client_copy = reject_original(client_copy)
    pack = client_copy / PACK
    if not pack.is_file():
        raise PreparationError(f'The client copy must contain {PACK}.')
    wanted = files(pack.read_bytes(), hfs.load_keystream(keystream))
    plan = {'mode': 'apply' if apply_changes else 'dry-run', 'files': {}}
    for path, data in sorted(wanted.items()):
        target = client_copy / path
        before = digest(target.read_bytes()) if target.is_file() else None
        plan['files'][path] = {'before_sha256': before, 'after_sha256': digest(data)}
        if apply_changes and before != digest(data):
            target.write_bytes(data)
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--keystream', type=Path, required=True, help='4096-byte pack keystream (scripts/client/hfs.py)')
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(apply(args.client_copy, args.keystream, args.apply), indent=1))
    except (PreparationError, hfs.HfsError, OSError) as error:
        print(json.dumps({'applied': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
