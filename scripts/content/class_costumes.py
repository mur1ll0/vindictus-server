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
# move there as WEAPON_SUBTYPE_BASE + n, above the 1.69 entries and the item editor's.
LAB_WEAPON_ENTITY = {'draker': 'weapon_greatsword'}
WEAPON_SUBTYPE_BASE = 20000


def weapon_subtype(weapon: str, costume_type: int) -> int:
    """The CostumeType an imported weapon item gets (its NA subtype moved into the lab entity)."""
    return WEAPON_SUBTYPE_BASE + costume_type if weapon in LAB_WEAPON_ENTITY else costume_type


def weapon_costumes(script: bytes, weapon: str) -> list[dict]:
    """The NA weapon script's SubTypeInfoOverride models as entries of the lab entity's script
    (section, key, normal), for apply_lab_content_additions.with_script_entries."""
    entity = LAB_WEAPON_ENTITY.get(weapon)
    if entity is None:
        return []
    found, stack = None, [kv_merge.parse(script)]
    while stack and found is None:
        node = stack.pop()
        for child in node.children:
            if child.block and child.key.lower() == 'subtypeinfooverride':
                found = child
                break
            if child.block:
                stack.append(child)
    out = []
    for entry in found.children if found else []:
        model = next((leaf.value for leaf in entry.children if leaf.key.lower() == 'playermodel'), None)
        if entry.block and re.fullmatch(r'\d{1,4}', entry.key) and model:
            out.append({'section': entity, 'key': str(weapon_subtype(weapon, int(entry.key))), 'normal': model})
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
