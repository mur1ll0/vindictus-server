"""player_costume.txt entry numbers of an imported NA class that the laboratory items already use.

The NA numbers a class's entries itself (Miri's inner armors are 9000-9xxx and 90000-90468), and the
item editor gives laboratory items free numbers from 9100 (src/domain/item_editor.rs). The 1.69 file
and the class do not collide (docs/future/miri-class-port.md 2.5), but laboratory items made before
the class arrived can: Miri's 9100 and 9102 against fiona_inner_zero and evy_inner_zero. Laboratory
items are operator data and keep their numbers; the class's colliding entries move to the first
numbers no one uses, the same way on the client (merged player_costume.txt) and on the server
(EquipItemInfo.CostumeType of the class's items), because both compute it from the same files.
"""
from __future__ import annotations
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'client'))
import kv_merge  # noqa: E402

FIRST_FREE = 9000
# ItemClassInfo.Category -> player_costume.txt section, for the equipment the class's items wear.
SECTIONS = {'INNERARMOR': 'inner_armor', 'HAIR': 'hair', 'FACE': 'face', 'HELM': 'head', 'TUNIC': 'upper_body',
            'PANTS': 'lower_body', 'GLOVES': 'hand', 'BOOTS': 'foot'}


# The 1.69 entity an imported class's weapon type uses (patch_class_routing.py EXTRA_CLASSES `weapons`):
# the 1.69 has no weapon_draker, and a weapon entity takes its model from its own script's
# SubTypeInfoOverride entry named by the item's EquipItemInfo.CostumeType. The class's NA subtypes
# move there as WEAPON_SUBTYPE_BASE + n. CostumeType also picks the entity in the class's weapon
# list (patch_class_routing.py CLASS_WEAPONS; server 0x40823590): 10000-19999 slot 0, 20000-29999
# slot 1, anything else CostumeType / 64 capped at 4. The Miri has slot 0 only, so her subtypes go to
# 10100 + n (up to 11133), past the 1.69 greatsword's 10001-10009. 20000 + n (slot 1) and 5000 + n
# (slot 4) left her with no weapon at all (MI5, 30/10).
LAB_WEAPON_ENTITY = {'draker': 'weapon_greatsword'}
WEAPON_SUBTYPE_BASE = 10100
# XE move of a class the NA released without one (operator rule of 31/10/2026; .agents/skills/new-class-port,
# "Pulo, ataques no ar e XE"): the 1.69 donor's XE skill under the class's name (class_imports.donor_xe_rows)
# and the donor's SuperPower state, with the class's skill and weapon entity (stage_na_class DONOR_STATES).
XE_DONOR = 'delia'
XE_SKILL = 'skill_{klass}_xe_move'
DONOR_WEAPON_ENTITY = {'delia': 'weapon_bastardsword'}


def weapon_subtype(weapon: str, costume_type: int) -> int:
    """The CostumeType an imported weapon item gets (its NA subtype moved into the lab entity)."""
    return WEAPON_SUBTYPE_BASE + costume_type if weapon in LAB_WEAPON_ENTITY else costume_type


# The Dragon Knight look's weapon (the NA's csf/metamorphosis/miri_dragon_knight.csf: weapon_draker
# sub-type 1033) has the black trails (miri_*_night; the normal ones are blue). The lab look keeps the held
# weapon: a .csf weapon block gives the player a new weapon entity (server 0x40539f60, vtable +0x808), which
# only the class default outfits ask for and the metamorphosis' restore does not undo. So every sub-type
# also carries the Dragon Knight trails at KNIGHT_TRAIL_OFFSET + n, the indices stage_na_class gives the
# transformed sequences (the 1.69 reads trail indices up to 10: weapon_dualblade.txt).
KNIGHT_WEAPON_SUBTYPE = {'draker': '1033'}
KNIGHT_TRAIL_OFFSET = 5


def kv_block_text(script: bytes, node, key: str | None = None) -> str:
    """A block's source text without its comment lines, under another key when given."""
    lines = [line.rstrip() for line in script[node.start:node.end].decode('latin-1').splitlines()
             if line.strip() and not line.strip().startswith('//')]
    if key is not None and lines:
        lines[0] = re.sub(r'^(\s*)"?[^"\s{]+"?', lambda m: f'{m.group(1)}"{key}"', lines[0], count=1)
    return '\r\n'.join(lines)


def block_child(node, key: str):
    return next((c for c in node.children if c.block and c.key.lower() == key), None) if node else None


def weapon_costumes(script: bytes, weapon: str) -> list[dict]:
    """The NA weapon script's SubTypeInfoOverride models as entries of the lab entity's script
    (section, key, normal, trails), for apply_lab_content_additions.with_script_entries.

    An entry takes the weapon's own TrailData (the Draker's fire, miri_trail_dragon_*) when the NA
    subtype has none: in the lab entity's script the top-level trails are the 1.69 greatsword's, which
    the Miri's AE_WEAPON_ENABLE_TRAIL_POINT indices (up to 4, along the chain) drew instead (C4 MI21).
    The Dragon Knight sub-type's trails follow at KNIGHT_TRAIL_OFFSET + n."""
    entity = LAB_WEAPON_ENTITY.get(weapon)
    if entity is None:
        return []
    root = kv_merge.parse(script)
    default = block_child(next((n for n in root.children if n.block), None), 'traildata')
    found, stack = None, [root]
    while stack and found is None:
        node = stack.pop()
        for child in node.children:
            if child.block and child.key.lower() == 'subtypeinfooverride':
                found = child
                break
            if child.block:
                stack.append(child)
    knight = block_child(block_child(found, KNIGHT_WEAPON_SUBTYPE.get(weapon, '')), 'traildata')
    out = []
    for entry in found.children if found else []:
        model = next((leaf.value for leaf in entry.children if leaf.key.lower() == 'playermodel'), None)
        if entry.block and re.fullmatch(r'\d{1,4}', entry.key) and model:
            costume = {'section': entity, 'key': str(weapon_subtype(weapon, int(entry.key))), 'normal': model}
            own = block_child(entry, 'traildata') or default
            parts = [kv_block_text(script, c) for c in own.children if c.block] if own else []
            parts += [kv_block_text(script, c, str(KNIGHT_TRAIL_OFFSET + int(c.key)))
                      for c in (knight.children if knight else []) if c.block and c.key.isdigit()]
            if parts:
                costume['trails'] = '"TrailData"\r\n{\r\n' + '\r\n'.join(parts) + '\r\n}'
            out.append(costume)
    return out


def section_keys(text: bytes) -> dict[str, set[str]]:
    return {s.key.lower(): {c.key for c in s.children if c.block} for s in kv_merge.parse(text).children if s.block}


def costume_remap(source: bytes, base: bytes, taken: dict[str, set[str]], needles: tuple[str, ...]) -> dict[tuple[str, str], str]:
    """(section, NA number) -> new number for the class's NA entries that `taken` (laboratory items'
    numbers by section) already uses."""
    wanted = tuple(n.lower().encode('latin-1') for n in needles)
    used = section_keys(source)
    for section, keys in section_keys(base).items():
        used.setdefault(section, set()).update(keys)
    remap = {}
    for node in kv_merge.parse(source).children:
        section = node.key.lower()
        lab = taken.get(section, set())
        if not node.block or not lab:
            continue
        for entry in node.children:
            if entry.block and entry.key in lab and kv_merge.mentions(source, entry, wanted):
                numbers = used.setdefault(section, set()) | lab
                new = next(str(k) for k in range(FIRST_FREE, 10**9) if str(k) not in numbers)
                used[section].add(new)
                remap[(section, entry.key)] = new
    return remap


def rename(source: bytes, remap: dict[tuple[str, str], str]) -> bytes:
    """The NA file with each remapped entry's number (and its "label", which repeats it) changed."""
    if not remap:
        return source
    edits = []
    for node in kv_merge.parse(source).children:
        for entry in node.children if node.block else []:
            new = remap.get((node.key.lower(), entry.key))
            if new is None or not entry.block:
                continue
            key_end = source.index(b'"', entry.start + 1) + 1 if source[entry.start:entry.start + 1] == b'"' else \
                entry.start + len(entry.key)
            edits.append((entry.start, key_end, b'"%s"' % new.encode()))
            for leaf in entry.children:
                if leaf.key.lower() == 'label' and leaf.value == entry.key:
                    value_at = source.rindex(entry.key.encode(), leaf.start, leaf.end)
                    edits.append((value_at, value_at + len(entry.key), new.encode()))
    out = bytearray(source)
    for start, end, text in sorted(edits, reverse=True):
        out[start:end] = text
    return bytes(out)


def lab_taken(items: list[dict]) -> dict[str, set[str]]:
    """Numbers the laboratory items' costumes use, by section (content_additions items)."""
    taken: dict[str, set[str]] = {}
    for item in items:
        for costume in item.get('costumes', []):
            if re.fullmatch(r'[a-z_]{1,32}', str(costume.get('section', ''))):
                taken.setdefault(costume['section'], set()).add(str(costume.get('key')))
    return taken
