"""Add entities to a Source BSP map (lump 0, the entity text) without moving any other lump.

The 1.69 lobby_beautyshop_change.bsp is the scene of the class grid (patch_class_routing CREATION_GRID): one
npc_character_creation_actor `npc_<class>` per class (csf/createcharacter_<class>.csf) that the server's
npc_character_create_select moves to the select point. It has the eleven 1.69 classes; the NA's map has the
same entity for the Miri (`npc_miri`, csf/createcharacter_miri.csf). with_creation_actors copies the donor
class's actor block with the class name changed and writes the grown entity lump at the end of the file, so
every other lump (and the absolute offsets inside the game lump) stays where it was.
"""
from __future__ import annotations

import re
import struct

HEADER_LUMPS = 8  # ident, version, then 64 lumps of (offset, length, version, fourCC)
ENTITIES = 0
ACTOR = 'npc_character_creation_actor'


class BspError(ValueError):
    pass


def entity_text(bsp: bytes) -> str:
    if bsp[:4] != b'VBSP':
        raise BspError('Not a Source BSP.')
    offset, length = struct.unpack_from('<ii', bsp, HEADER_LUMPS + 16 * ENTITIES)
    if offset <= 0 or offset + length > len(bsp):
        raise BspError('Entity lump out of the file.')
    return bsp[offset:offset + length].rstrip(b'\0').decode('latin-1')


def blocks(text: str) -> list[str]:
    return re.findall(r'\{[^{}]*\}', text)


def value(block: str, key: str) -> str | None:
    found = re.search(r'"%s"\s+"([^"]*)"' % re.escape(key), block)
    return found.group(1) if found else None


def with_creation_actors(bsp: bytes, donor: str, classes: tuple[str, ...]) -> bytes | None:
    """The map with an actor npc_<class> for each class, copied from npc_<donor> with the csf and the name
    changed; None when every class already has one. The `variation` (a scripts/npc_variations.txt entry of
    npc_character_creation_actor) stays the donor's: neither client has one for the Miri (the NA's map gives
    her npc_vella, which carries Vella's own model), and the donor's has no model, only the actor settings,
    so the look comes from the csf alone."""
    text = entity_text(bsp)
    actors = {value(b, 'targetname'): b for b in blocks(text) if value(b, 'classname') == ACTOR}
    if f'npc_{donor}' not in actors:
        raise BspError(f'The map has no {ACTOR} npc_{donor}.')
    donor_block = actors[f'npc_{donor}']
    added = [re.sub(r'("targetname"\s+)"npc_%s"' % re.escape(donor), r'\1"npc_%s"' % key,
                    donor_block.replace(f'createcharacter_{donor}.csf', f'createcharacter_{key}.csf'))
             for key in classes if f'npc_{key}' not in actors]
    if not added:
        return None
    newline = '\r\n' if '\r\n' in text else '\n'
    grown = (text.rstrip() + newline + newline.join(added) + newline).encode('latin-1') + b'\0'
    out = bytearray(bsp + b'\0' * (-len(bsp) % 4))
    struct.pack_into('<ii', out, HEADER_LUMPS + 16 * ENTITIES, len(out), len(grown))
    return bytes(out + grown)
