"""Route the classes the 1.69 lacks (EXTRA_CLASSES: Delia, 10) to their files in COPIES of client.dll
and server.dll. Dry-run by default. Every per-class patch reads EXTRA_CLASSES, so a later class is
one more table entry; only the item class flags (FLAG_CHECKS, TOOLTIPS) stop at class 10, the last
of engine.dll's 11 item flags.

docs/future/new-classes.md 8.13 lists where the 1.69 DLLs stop at nine classes. Without this
patch class 10 falls to the default case of the per-class switches, is the pet in the switches
that have one, and reads past five 10-entry tables of the server. The patch follows the NA layout:

* each switch below gets a jump table in a new `.hrte` section. Cases 0-9 are unchanged; case 10
  runs a copy of the Hagie case with every Hagie string replaced by the Delia one (the 1.69
  content has all of them); in the switches with a pet case, the pet moves to PET (28, as in the
  NA) and 11-27 take the default case;
* the five string tables of the server get 32 entries (0-9 copied, 10 Delia, 11-31 repeat entry 0
  so a later class never reads a null), and every relocated reference to them is redirected;
* `m_iClass` is sent with log2(PET)+1 = 5 bits instead of log2(10)+1 = 4;
* the client's local-player setup accepts classes below PET instead of below 10;
* SkillActionStateMatchTable gets an 11th per-class map for Delia (the section is writable for it).

Requires the `.hcls` patch (patch_class_table.py), whose relocation directory moves again into
`.hrte`, with one more block for the new pointers. The unpatched copy is kept next to each file.
"""
from __future__ import annotations
import argparse
import json
import os
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

from patch_class_table import Image, align, digest  # noqa: E402
from prepare_lab_client import PreparationError, reject_original  # noqa: E402

SECTION = b'.hrte\0\0\0'
PET = 28
TABLE_SIZE = 32
# Classes 0-9 of the 1.69, in m_iClass order.
OLD_CLASSES = ('lethita', 'fiona', 'evy', 'kalok', 'kay', 'vella', 'hurk', 'lynn', 'arisha', 'hagie')


@dataclass(frozen=True)
class ExtraClass:
    """A class the 1.69 DLLs do not know. Every per-class patch below reads this table, so a new
    class (Miri next) is one more entry plus its content files."""
    index: int                      # m_iClass, contiguous from 10 and below PET
    key: str                        # lowercase name in the scripts and paths ("delia")
    clone_of: str                   # older class whose switch cases are copied with names renamed
    body: str                       # skeleton folder under models/player/_anim
    figure: str                     # body figure (client entity slot 264)
    constants: tuple[float, float]  # client entity slots 177/178 (NA values)
    faces: int                      # models/player/<key>/pc_<key>_faceNN.mdl

    @property
    def name(self) -> str:
        return self.key.capitalize()


EXTRA_CLASSES = (
    ExtraClass(10, 'delia', 'hagie', 'female', 'csf/bodyshape_delia_default.fig', (0.88, 0.96), 3),
)
# Class 10 is the only extra class with an item "usable by" flag (engine.dll keeps bool[11] per
# item; patch_engine_class_flags.py), so FLAG_CHECKS and TOOLTIPS stay on her.
DELIA = 10
LAST = max(c.index for c in EXTRA_CLASSES)
EXPECTED = {'client.dll': 'a9591d3a44a2b538', 'server.dll': 'af43431e78666783'}
# Switches: (VA of `cmp reg, imm8`, original bound, whether case 10 is the pet).
SWITCHES = {
    'client.dll': (
        (0x387ED842, 9, False),   # scripts/actionstate/player_action_state_<class>[_ex].txt
        (0x3880401A, 10, True),   # scripts/expression/expression_table_<class>
        (0x38861C22, 10, True),   # scripts/skill/<class>_skill_keyword_table.txt
        (0x387A3AE4, 9, False),   # lowercase class name (csf/bodyshape_*_%s.fig and others)
    ),
    'server.dll': (
        (0x407EFDBD, 9, False),   # action state scripts
        (0x40804ACA, 10, True),   # expression tables
        (0x407CEB22, 10, True),   # skill keyword tables
    ),
}
# Server string tables indexed by m_iClass without a bound: (VA, entry template or None), filled
# per extra class from {key}, {Name} and {body}.
TABLES = {
    'server.dll': (
        (0x40CF9E64, 'csf/create_{key}.csf'),
        (0x40CF9E3C, 'csf/prologue_{key}.csf'),
        (0x40CF968C, 'Player.{Name}.'),
        (0x40CFA0B0, 'csf/costume_default_{key}.csf'),
        (0x40CFA0D8, None),  # PvP costume: Hagie's entry is null too
        # Animation models read by [class*4 + table] at 0x4040ff6c (town), 0x4040ffa9 (battle) and
        # 0x4040ff91 (ExtremeMode). Class 10 read the next table (a male skeleton) and, in
        # ExtremeMode, a non-pointer; D8c froze on the emote.
        (0x40CF1F80, 'models/player/_anim/{body}/{key}/pc_{key}_town_motion.mdl'),
        (0x40CF1FA8, 'models/player/_anim/{body}/{key}/pc_{key}_battle_motion.mdl'),
        (0x40CF1FD0, 'models/player/_anim/{body}/{key}/pc_{key}_battle_motion_ex.mdl'),
    ),
    # Class name and initial read through 0x388e6ac0/0x388e6ae0 (bounded below by CLASS_STRING_BOUNDS);
    # the skill window passes the name to SkillDialog.setClassInfo, whose movie has Delia's frames
    # (Weapon_1st_Delia). Class 10 got an empty name and every skill icon landed on one spot (D12).
    'client.dll': (
        (0x38CB6D24, '{Name}'),
        (0x38CB6D4C, '{Initial}'),
    ),
}
# `cmp eax, 9` of the two client accessors above (`ja` to an empty string); the bound becomes LAST.
CLASS_STRING_BOUNDS = {'client.dll': (0x388E6AC4, 0x388E6AE4)}
# The client entity every extra class gets (the server falls back to "player", CHeroesPlayer)
# and the methods its own class entity overrides in the NA:
# * slot 264 returns the body figure (csf/bodyshape_<class>_default.fig). Hagie's (0x3870d1d0)
#   returns hers unless BodyShapeModify is on and the game state is not 0x1f; the base returns
#   none, and Delia's body was deformed from the belly to the thighs at character select (D8v);
# * slots 177/178 return two float constants (base 0.9/1.1, Hagie 0.91/0.99, NA Delia 0.88/0.96).
# Each slot of that vtable points at a dispatcher on m_iClass (+0x18ec): an extra class gets
# Hagie's figure gate with its own figure and its constants; the others run the original.
ENTITY_SLOTS = {'client.dll': ('C_HeroesPlayer', 0x18EC, 264, (177, 178), 0x3870D1D0)}
# `push 0xa` whose log2 sizes the m_iClass SendProp (0x405368c3..0x405368c7).
BITS = {'server.dll': 0x405368C5}
# `cmp eax, 0xa; jge` that skips the whole local-player setup (C_HeroesPlayer vtable +0xac,
# 0x387033b0), which builds the command object at +0x437c that the movement code reads every
# frame. Class 10 crashed at 0x387da6b0 in client test D8. The bound becomes PET.
GATES = {'client.dll': (0x387033D7,)}
# SkillActionStateMatchTable keeps one action-state map (a 0x1c-byte CUtlMap) per class at
# this + class*0x1c + 0x20 for classes 0-9, with the action-name map right after (+0x138), so
# class 10 read that map and crashed (D8b: emote, Mercenary Outpost). Delia gets an 11th map in
# .hrte: (GetSkillNamesByActionStateName index, MatchingActionStateSkill map pick, its class-loop
# `cmp ecx, 0xa`, the maps' less function set by the constructor).
MATCH = {
    'client.dll': (0x3885FB93, 0x38860993, 0x38860C1D, 0x38022280),
    'server.dll': (0x407CCBA3, 0x407CD8A3, 0x407CDB2D, 0x400CEB80),
}
# "Is this class allowed" accessors over the item's per-class bools (+0x198, 11 entries, filled
# by engine.dll; see patch_engine_class_flags.py): a `switch` on classes 0-9 returning
# flags[class], so class 10 got false ("Cannot be equipped", D8e). Each head jumps to a stub that
# returns flags[class] for classes 0-10 and keeps the default (false) above. Entries: (VA of
# `cmp reg, 9`, original bytes replaced by the jump, class register, item base: 'ecx' (this) or
# 'stack' ([esp + 4]), default VA, bytes popped by `ret`). The rest of the old head, including its
# relocated jump-table address, stays in place as dead code.
FLAG_CHECKS = {
    'client.dll': (
        (0x382D4C34, '83f8097761', 'eax', 'ecx', 0x382D4C9A, 4),
        (0x388F9C86, '83f9090f87', 'ecx', 'stack', 0x388F9D15, 8),
    ),
    'server.dll': (
        (0x402682E8, '83f9090f87', 'ecx', 'stack', 0x40268377, 4),
    ),
}
# Item tooltip "usable by" lists (one block per class, Lethita..Hagie, then "#..._CharacterClass0"
# = "Cannot be equipped" when no class matched): (Hagie block start, end, join VA, bytes the
# jump to the stub replaces at the join). The stub runs a copy of the Hagie block reading
# flag 10 (+0x1a2) and naming #GameUI_Delia; a Delia that is not allowed is skipped instead of
# clearing the "every class" marker, so the lists of the ten older classes do not change.
TOOLTIPS = {
    'client.dll': (
        (0x38410C19, 0x38410C55, 0x38410C5C, '33c033d266899424b4010000'),
        (0x385B09D8, 0x385B0A12, 0x385B0A18, '8bbc24c4130000'),
    ),
}
# The motion-model precache (registered at 0x40a09bd3) precaches only the ten town motions in
# town and clamps the class to 9 elsewhere, so Delia's motion model was never precached and the
# model failed at spawn (invisible, frozen emote; D8d-D8f). Its entry (`sub esp,0x54; push esi;
# push edi`) jumps to a stub that first precaches her three motion models with PrecacheModel.
PRECACHE = {'server.dll': (0x4054DE00, '83ec545657', 0x40114940)}
# Costume manager (scripts/player_costume.txt): per-class columns of 10 (default and default_ex
# part per slot at +0xf0/+0x2d0, colours from +0x4b0, colour flags from +0xa50, town at +0xbb8,
# event files at +0xbe0/+0xc80). The loader (client 0x38882e30) matches a section name against
# classes 0-9 only, so the "delia" sections (base delia_base.mdl, face, motion) were read as part
# entries and every class-10 read landed in the next column: no body (D8b-D8g). Delia gets a
# shadow costume object in .hrte: the loader stores her sections in its class-0 column, and each
# accessor called with class 10 reads the shadow as class 0. The other classes' columns stay put.
# Entries: name-match bound (`cmp esi, 9`), class block start, next-section point, accessors
# (VA, original bytes moved to the stub, stack offset of the class argument at entry).
COSTUME = {
    'client.dll': (0x38882FE0, 0x3888305B, 0x38883525, (
        (0x3887CBC0, '568b742408', 8),                 # default part (slot, class)
        (0x3887CC60, '8b4424048b8481b80b0000', 4),     # town (class)
        (0x3887CC70, '56578b7c240c', 4),               # event file (class)
        (0x3887CCC0, '8b54240456', 0xC),               # colour (slot, index, class)
        (0x3887CD10, '8b5424048b442408', 0xC),         # colour flag (slot, index, class)
    )),
    'server.dll': (0x4081A900, 0x4081A97B, 0x4081AE45, (
        (0x40815120, '568b742408', 8),
        (0x408151C0, '8b4424048b8481b80b0000', 4),
        (0x408151D0, '56578b7c240c', 4),
        (0x40815220, '8b54240456', 0xC),
        (0x40815270, '8b5424048b442408', 0xC),
    )),
}
# The server costume object's "event file for class" method (0x4081bae0) starts with
# `cmp esi, 9; ja` and reads the manager's event_file_name(_xe) column inline (the manager is a
# global, so `class*16 + 0x40f64698`/`0x40f64738`). Class 10 got no motion event file, the
# networked path (+0x3f0, client +0x410) stayed empty and the town body was never built
# (D8l/D8m memory). Class 10 now reads her paths from the shadow's class-0 column.
# (VA of `cmp esi, 9; ja`, its bytes, ExtremeMode test, continuation after the address, skip.)
EVENT_FILE = {'server.dll': (0x4081BB0F, '83fe097778', 0x408B3A70, 0x4081BB31, 0x4081BB8C, 0x4081BB14)}
# Class name -> class index (lethita..hagie, else -1), used by scripts/translated_spskill.txt,
# whose "delia" section was dropped: Delia had no SP skills (the eight -1 values at server
# +0x3418 in D8m-D8r were her SP skill slots). The entry (push esi; mov esi, [esp+8]) jumps to a
# stub that returns DELIA for "delia" (the DLL's own case-insensitive compare) and runs the
# original otherwise. (entry VA, its 5 bytes, compare routine.)
CLASS_NAME = {
    'client.dll': (0x389145B0, '568b742408', 0x38A5DFC6),
    'server.dll': (0x40252880, '568b742408', 0x40994755),
}
# The server player's costume entry (0x405465d0) starts with `cmp eax, 9; ja` on m_iClass and,
# above 9, skips the costume build (0x40546400, which names the model "CoStUmE_<parts>...mdl")
# and leaves the generic lethita_base.mdl: Delia's body was never built (D8u: Fiona's server
# model name is CoStUmE_1/0_10/..., Delia's models/player/lethita/lethita_base.mdl).
COSTUME_GATE = {'server.dll': 0x405465DE}
COSTUME_BLOCK = bytes.fromhex('8b4424148d4c9b1e')  # mov eax, [esp + 0x14]; lea ecx, [ebx + ebx*4 + 0x1e]
COSTUME_SIZE = 0x1000  # above the last per-class field (+0xc80 + 0x10)
# Skill class checks (IsAvailableClass over SkillInfo.ClassRestriction): `cmp ecx, 9; ja` before
# `1 << class`, so every skill of a class above 9 was skipped and Delia's skill window stayed empty
# (D8v-D11). The helper at 0x3835df60 and the skill dialog's list (0x38372c00) inline the same
# test, as does 0x385a5ed0 (`cmp eax, 9`, a scan of `cmp reg, 9; ja` before `shl reg, cl`, D13); the
# bound becomes the last extra class (the mask is read as 16 bits, enough for class 15).
CLASS_BITS = {'client.dll': (0x3835DF6E, 0x38372D03, 0x385A6763)}
# Diagnostic only (D8k, operator's choice): the server builds `player_<cl_playerclass>` and, when
# no such factory exists, pushes "player" (CHeroesPlayer, the base of every class) at 0x40371314.
# --fallback-entity swaps that string for an existing class factory, so a class without its own
# entity (only Delia) gets that class's entity. Off by default; not the Delia entity design.
FALLBACK_PUSH = 0x40371314
FALLBACK_ENTITIES = ('player_fiona',)


def filled(template: str | None, extra: ExtraClass) -> str | None:
    return template.format(key=extra.key, Name=extra.name, Initial=extra.name[0], body=extra.body) if template else None


def precache_models(extra: ExtraClass) -> list[str]:
    """Motion models, then the base body and faces (player_costume.txt "base" and "face"): Delia's
    town entity had model index -1 while a Fiona in the same entity had 13 (D8l memory diff)."""
    motions = [filled(text, extra) for _, text in TABLES['server.dll'] if text and '_motion' in text]
    return motions + [f'models/player/{extra.key}/{extra.key}_base.mdl'] + [
        f'models/player/{extra.key}/pc_{extra.key}_face{n:02d}.mdl' for n in range(1, extra.faces + 1)]


def check_classes(classes: tuple[ExtraClass, ...]) -> None:
    """The stubs assume contiguous indices from 10 below the pet, and class 10 first (FLAG_CHECKS)."""
    if [c.index for c in classes] != list(range(10, 10 + len(classes))) or classes[-1].index >= PET:
        raise PreparationError('Extra classes must be numbered 10, 11, ... below the pet.')
    if len({c.key for c in classes}) != len(classes) or any(c.clone_of not in OLD_CLASSES for c in classes):
        raise PreparationError('Extra classes need distinct keys and an older class to clone.')


DELIA_PRECACHE = precache_models(EXTRA_CLASSES[0])
MATCH_LOOKUP = bytes.fromhex('0fbf4424248d0cc5000000002bc88d1c8e8d5424108d7b20')
MATCH_PICK = bytes.fromhex('8b6c241083c5fc')
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
md.detail = False


class Layout:
    """The .hrte body being built: bytes, absolute pointers (relocations) and strings."""

    def __init__(self, va: int):
        self.va, self.body, self.pointers, self.strings = va, bytearray(), [], {}

    def here(self) -> int:
        return self.va + len(self.body)

    def pad(self) -> None:
        self.body += b'\xcc' * (align(len(self.body), 16) - len(self.body))

    def string(self, text: str) -> int:
        if text not in self.strings:
            self.strings[text] = self.here()
            self.body += text.encode('ascii') + b'\0'
        return self.strings[text]

    def pointer(self, value: int) -> None:
        self.pointers.append(len(self.body))
        self.body += struct.pack('<I', value)

    def to(self, opcode: bytes, target: int) -> None:
        """A jump or call (opcode bytes then rel32) to a known address."""
        self.body += opcode + struct.pack('<i', target - (self.here() + len(opcode) + 4))

    def forward(self, opcode: bytes) -> int:
        """A jump to a later point of this stub; land() fills it in."""
        self.body += opcode + bytes(4)
        return len(self.body) - 4

    def land(self, at: int) -> None:
        struct.pack_into('<i', self.body, at, len(self.body) - (at + 4))


def relocations(image: Image) -> set[int]:
    """VAs of every relocated dword (IMAGE_REL_BASED_HIGHLOW)."""
    rva, size = image.directory(5)
    start = image.offset(image.base + rva)
    out, at = set(), start
    while at < start + size:
        page, length = struct.unpack_from('<II', image.data, at)
        if length < 8:
            break
        for i in range(8, length, 2):
            entry = struct.unpack_from('<H', image.data, at + i)[0]
            if entry >> 12 == 3:
                out.add(image.base + page + (entry & 0xFFF))
        at += length
    return out


def read_string(image: Image, va: int) -> str | None:
    try:
        o = image.offset(va)
    except PreparationError:
        return None
    end = image.data.find(b'\0', o, o + 200)
    raw = bytes(image.data[o:end])
    return raw.decode('ascii') if end > o and all(32 <= c < 127 for c in raw) else None


def rename(text: str, old: str, new: str) -> str:
    """Replace a class name keeping its case (hagie/Hagie/HAGIE)."""
    return re.sub(old, lambda m: new if m.group().islower() else new.upper() if m.group().isupper()
                  else new.capitalize(), text, flags=re.IGNORECASE)


def delia(text: str) -> str:
    return rename(text, 'hagie', 'delia')


def switch_parts(image: Image, va: int, bound: int) -> tuple[str, int, int, int]:
    """(register, default VA, jump table VA, VA of the table displacement) of
    `cmp reg, bound; ja default; jmp [reg*4 + table]`."""
    code = bytes(image.data[image.offset(va):image.offset(va) + 20])
    ins = list(md.disasm(code, va))[:3]
    if len(ins) < 3 or ins[0].mnemonic != 'cmp' or ins[0].op_str.split(', ')[1] not in (hex(bound), str(bound)) \
            or ins[0].size != 3 or ins[1].mnemonic != 'ja':
        raise PreparationError(f'Unexpected switch head at {va:#x}; nothing is patched.')
    found = re.match(r'dword ptr \[(\w+)\*4 \+ (0x[0-9a-f]+)\]', ins[2].op_str)
    register = ins[0].op_str.split(', ')[0]
    if ins[2].mnemonic != 'jmp' or not found or found.group(1) != register:
        raise PreparationError(f'Unexpected jump at {va:#x}; nothing is patched.')
    if code[ins[2].address - va:ins[2].address - va + 2] != bytes((0xFF, 0x24)) or ins[2].size != 7:
        raise PreparationError(f'Unexpected jump encoding at {ins[2].address:#x}; nothing is patched.')
    return register, int(ins[1].op_str, 16), int(found.group(2), 16), ins[2].address + 3


def clone_case(image: Image, layout: Layout, start: int, end: int, relocs: set[int],
               old: str = 'hagie', new: str = 'delia') -> list[str]:
    """Copy the case [start, end) of class `old` into the layout with the strings of class `new`
    (already in the layout); returns the strings."""
    code = bytes(image.data[image.offset(start):image.offset(end)])
    replaced, last, inner, placed = [], None, [], {}
    for ins in md.disasm(code, start):
        placed[ins.address] = len(layout.body)
        raw = bytes(code[ins.address - start:ins.address - start + ins.size])
        target = int(ins.op_str, 16) if re.fullmatch(r'0x[0-9a-f]+', ins.op_str) else None
        if ins.mnemonic == 'jmp' and target is not None:
            layout.body += b'\xe9' + struct.pack('<i', target - (layout.here() + 5))
        elif ins.mnemonic.startswith('j') and target is not None and ins.mnemonic != 'jmp':
            if not start <= target < end:
                opcode = raw[0] - 0x70 if raw[0] in range(0x70, 0x80) else raw[1] - 0x80
                layout.body += bytes((0x0F, 0x80 + opcode)) + struct.pack('<i', target - (layout.here() + 6))
            else:
                # Forward branch inside the case: patched once the target is copied.
                layout.body += bytes((0x0F, 0x80 + (raw[0] - 0x70 if raw[0] in range(0x70, 0x80) else raw[1] - 0x80)))
                inner.append((len(layout.body), target))
                layout.body += bytes(4)
        elif ins.mnemonic == 'call' and target is not None:
            layout.body += b'\xe8' + struct.pack('<i', target - (layout.here() + 5))
        else:
            fields = [i for i in range(ins.size - 3) if ins.address + i in relocs]
            out = bytearray(raw)
            for i in fields:
                value = struct.unpack_from('<I', raw, i)[0]
                text = read_string(image, value)
                if text and old in text.lower():
                    struct.pack_into('<I', out, i, layout.strings[rename(text, old, new)])
                    replaced.append(rename(text, old, new))
            base = len(layout.body)
            layout.body += out
            for i in fields:
                layout.pointers.append(base + i)
        last = ins
    if last is None or not (last.mnemonic in ('jmp', 'ret')):
        layout.body += b'\xe9' + struct.pack('<i', end - (layout.here() + 5))
    for at, target in inner:
        if target not in placed:
            raise PreparationError(f'Branch to {target:#x} lands inside an instruction; nothing is patched.')
        struct.pack_into('<i', layout.body, at, placed[target] - (at + 4))
    return replaced


def match_table(image: Image, layout: Layout, edits: list, relocs: set[int],
                lookup: int, pick: int, loop: int, less: int) -> dict:
    """An action-state map per extra class and the two stubs that reach them (MATCH)."""
    for va, raw in ((lookup, MATCH_LOOKUP), (pick, MATCH_PICK)):
        if any(va - 3 <= r < va + len(raw) for r in relocs):
            raise PreparationError(f'Relocated dword inside {va:#x}; nothing is patched.')
    if b'\xbd' + struct.pack('<I', less) not in image.data:
        raise PreparationError(f'No `mov ebp, {less:#x}` (the maps\' less function); nothing is patched.')
    # The maps, as the constructor leaves each class map: less function, empty CUtlMemory,
    # root and first free 0xffff, no element.
    layout.pad()
    maps = {}
    for extra in EXTRA_CLASSES:
        maps[extra.index] = layout.here()
        layout.pointer(less)
        layout.body += struct.pack('<IIIHHHHI', 0, 0, 0, 0xFFFF, 0, 0xFFFF, 0, 0)

    # GetSkillNamesByActionStateName: an extra class uses its map (edi) and base (ebx = map - 0x20,
    # read again as ebx + 0x24); any other class >= 10 the first extra map; lower classes keep the
    # original computation.
    layout.pad()
    lookup_stub = layout.here()
    layout.body += MATCH_LOOKUP[:5]
    found = []
    for index, address in maps.items():
        layout.body += b'\x83\xf8' + bytes((index,))
        other = layout.forward(b'\x0f\x85')
        layout.body += b'\xbf'
        layout.pointer(address)
        found.append(layout.forward(b'\xe9'))
        layout.land(other)
    layout.body += b'\x83\xf8\x0a'
    low = layout.forward(b'\x0f\x8c')
    layout.body += b'\xbf'
    layout.pointer(maps[EXTRA_CLASSES[0].index])
    for at in found:
        layout.land(at)
    layout.body += b'\x8d\x5f\xe0'
    done = layout.forward(b'\xe9')
    layout.land(low)
    layout.body += MATCH_LOOKUP[5:17] + MATCH_LOOKUP[21:24]
    layout.land(done)
    layout.body += MATCH_LOOKUP[17:21]  # lea edx, [esp + 0x10], for every class
    layout.to(b'\xe9', lookup + len(MATCH_LOOKUP))
    edits.append((lookup, MATCH_LOOKUP,
                  b'\xe9' + struct.pack('<i', lookup_stub - (lookup + 5)) + b'\x90' * (len(MATCH_LOOKUP) - 5)))

    # MatchingActionStateSkill: the loop's class is at [esp + 0x28]; an extra class fills its map (ebp).
    # The loop also keeps &map->m_Elements at [esp + 0x10] and reads it again after an insert
    # (client 0x38860b4a, server 0x407cda5a; D8c crashed there), so an extra class points it at its
    # map too; the `add [esp + 0x10], 0x1c` that follows is overwritten by the next extra class.
    layout.pad()
    pick_stub = layout.here()
    found = []
    for index, address in maps.items():
        layout.body += b'\x83\x7c\x24\x28' + bytes((index,))
        other = layout.forward(b'\x0f\x85')
        layout.body += b'\xbd'
        layout.pointer(address)
        layout.body += b'\xc7\x44\x24\x10'
        layout.pointer(address + 4)
        found.append(layout.forward(b'\xe9'))
        layout.land(other)
    layout.body += MATCH_PICK
    for at in found:
        layout.land(at)
    layout.to(b'\xe9', pick + len(MATCH_PICK))
    edits.append((pick, MATCH_PICK, b'\xe9' + struct.pack('<i', pick_stub - (pick + 5)) + b'\x90' * 2))
    edits.append((loop, b'\x83\xf9\x0a', b'\x83\xf9' + bytes((LAST + 1,))))
    return {'map': hex(maps[EXTRA_CLASSES[0].index]), 'maps': {i: hex(a) for i, a in maps.items()},
            'lookup': f'{lookup:#x} -> {lookup_stub:#x}', 'pick': f'{pick:#x} -> {pick_stub:#x}',
            'loop': f'{loop:#x} classes 0-{LAST}'}


def flag_check(layout: Layout, edits: list, va: int, old: str, register: str, base: str,
               default: int, pop: int) -> str:
    """One FLAG_CHECKS stub: cmp reg, DELIA; ja default; al = flags[reg]; ret pop."""
    reg = {'eax': 0, 'ecx': 1}[register]
    layout.pad()
    stub = layout.here()
    layout.body += bytes((0x83, 0xF8 | reg, DELIA))
    layout.body += b'\x0f\x87' + struct.pack('<i', default - (layout.here() + 6))
    if base == 'ecx':
        layout.body += bytes((0x8A, 0x84, (reg << 3) | 1)) + struct.pack('<I', 0x198)  # mov al, [ecx + reg + 0x198]
    else:
        layout.body += b'\x8b\x44\x24\x04'  # mov eax, [esp + 4]
        layout.body += bytes((0x8A, 0x84, (reg << 3) | 0)) + struct.pack('<I', 0x198)  # mov al, [eax + reg + 0x198]
    layout.body += b'\xc2' + struct.pack('<H', pop)
    edits.append((va, bytes.fromhex(old), b'\xe9' + struct.pack('<i', stub - (va + 5))))
    return f'{va:#x} -> {stub:#x}'


def tooltip(image: Image, layout: Layout, edits: list, relocs: set[int],
            start: int, end: int, join: int, old: str) -> str:
    """One TOOLTIPS stub: the Hagie block for flag 10 and #GameUI_<class 10>, then the join."""
    replaced = bytes.fromhex(old)
    if any(join - 3 <= r < join + len(replaced) for r in relocs):
        raise PreparationError(f'Relocated dword inside the join {join:#x}; nothing is patched.')
    layout.pad()
    name = layout.string(f'#GameUI_{EXTRA_CLASSES[0].name}')
    layout.pad()
    stub = layout.here()
    skips, flags, named = [], 0, False
    for ins in md.disasm(bytes(image.data[image.offset(start):image.offset(end)]), start):
        raw = bytearray(image.data[image.offset(ins.address):image.offset(ins.address) + ins.size])
        if ins.mnemonic == 'je':
            layout.body += b'\x0f\x84'
            skips.append(len(layout.body))
            layout.body += bytes(4)
        elif ins.mnemonic == 'call' and raw[0] == 0xE8:
            layout.body += b'\xe8' + struct.pack('<i', int(ins.op_str, 16) - (layout.here() + 5))
        elif ins.mnemonic == 'push' and ins.address + 1 in relocs:
            if read_string(image, struct.unpack_from('<I', raw, 1)[0]) != '#GameUI_Hagie':
                raise PreparationError(f'Unexpected string at {ins.address:#x}; nothing is patched.')
            layout.body += b'\x68'
            layout.pointer(name)
            named = True
        else:
            if any(ins.address <= r < ins.address + ins.size for r in relocs):
                raise PreparationError(f'Unexpected relocation at {ins.address:#x}; nothing is patched.')
            at = raw.find(struct.pack('<I', 0x1A1))
            if ins.mnemonic == 'cmp' and at > 0:
                struct.pack_into('<I', raw, at, 0x1A2)
                flags += 1
            layout.body += raw
    if flags != 2 or not named or len(skips) != 2:
        raise PreparationError(f'Unexpected Hagie block at {start:#x}; nothing is patched.')
    tail = len(layout.body)
    for at in skips:
        struct.pack_into('<i', layout.body, at, tail - (at + 4))
    layout.body += replaced + b'\xe9' + struct.pack('<i', join + len(replaced) - (layout.here() + len(replaced) + 5))
    edits.append((join, replaced, b'\xe9' + struct.pack('<i', stub - (join + 5)) + b'\x90' * (len(replaced) - 5)))
    return f'{join:#x} -> {stub:#x}'


def precache(layout: Layout, edits: list, entry: int, old: str, precache_model: int) -> str:
    """The PRECACHE stub: PrecacheModel(path, 0) for each model of the extra classes, then the entry."""
    models = [text for extra in EXTRA_CLASSES for text in precache_models(extra)]
    layout.pad()
    addresses = [layout.string(text) for text in models]  # strings before the code
    layout.pad()
    stub = layout.here()
    for address in addresses:
        layout.body += b'\x6a\x00\x68'
        layout.pointer(address)
        layout.body += b'\xe8' + struct.pack('<i', precache_model - (layout.here() + 5)) + b'\x83\xc4\x08'
    replaced = bytes.fromhex(old)
    layout.body += replaced + b'\xe9' + struct.pack('<i', entry + len(replaced) - (layout.here() + len(replaced) + 5))
    edits.append((entry, replaced, b'\xe9' + struct.pack('<i', stub - (entry + 5))))
    return f'{entry:#x} -> {stub:#x} ({len(models)} models)'


def costume(image: Image, layout: Layout, edits: list, relocs: set[int],
            bound: int, block: int, after: int, accessors: tuple) -> dict:
    """COSTUME: a shadow costume object per extra class, the loader hooks and the accessor stubs."""
    for va, size in [(bound, 3), (block, len(COSTUME_BLOCK)), (after, 7)] + [(v, len(bytes.fromhex(o))) for v, o, _ in accessors]:
        if any(va - 3 <= r < va + size for r in relocs):
            raise PreparationError(f'Relocated dword inside {va:#x}; nothing is patched.')
    o = image.offset(after)
    if bytes(image.data[o:o + 3]) != b'\x8b\xcf\xe8':
        raise PreparationError(f'Unexpected bytes at {after:#x}; nothing is patched.')
    call = after + 7 + struct.unpack_from('<i', image.data, o + 3)[0]

    # The shadows, as the loader's initialisation leaves a column: default and default_ex parts and
    # town -1, colours and flags 0, empty strings (zeroed CUtlString). Then the saved real object.
    layout.pad()
    shadows = {}
    for extra in EXTRA_CLASSES:
        shadows[extra.index] = layout.here()
        body = bytearray(COSTUME_SIZE)
        for part in range(12):
            struct.pack_into('<i', body, 0xF0 + part * 40, -1)
            struct.pack_into('<i', body, 0x2D0 + part * 40, -1)
        struct.pack_into('<i', body, 0xBB8, -1)
        layout.body += body
    real = layout.here()
    layout.body += bytes(4)

    # Loader: a section named like an extra class runs the class block on its shadow as class 0.
    layout.pad()
    block_stub = layout.here()
    found = []
    for index, shadow in shadows.items():
        layout.body += b'\x83\xfe' + bytes((index,))
        other = layout.forward(b'\x0f\x85')
        layout.body += b'\x8b\x44\x24\x14' + b'\xa3'
        layout.pointer(real)
        layout.body += b'\xc7\x44\x24\x14'
        layout.pointer(shadow)
        layout.body += b'\x33\xf6'
        found.append(layout.forward(b'\xe9'))
        layout.land(other)
    for at in found:
        layout.land(at)
    layout.body += COSTUME_BLOCK
    layout.to(b'\xe9', block + len(COSTUME_BLOCK))
    edits.append((block, COSTUME_BLOCK, b'\xe9' + struct.pack('<i', block_stub - (block + 5)) + b'\x90' * 3))
    edits.append((bound, b'\x83\xfe\x09', b'\x83\xfe' + bytes((LAST,))))

    # Next section: put the real object back once a shadow was used.
    layout.pad()
    after_stub = layout.here()
    found = []
    for shadow in shadows.values():
        layout.body += b'\x81\x7c\x24\x14'
        layout.pointer(shadow)
        found.append(layout.forward(b'\x0f\x84'))
    keep = layout.forward(b'\xe9')
    for at in found:
        layout.land(at)
    layout.body += b'\x50' + b'\xa1'
    layout.pointer(real)
    layout.body += b'\x89\x44\x24\x18' + b'\x58'
    layout.land(keep)
    layout.body += b'\x8b\xcf'
    layout.to(b'\xe8', call)
    layout.to(b'\xe9', after + 7)
    edits.append((after, bytes(image.data[o:o + 7]), b'\xe9' + struct.pack('<i', after_stub - (after + 5)) + b'\x90' * 2))

    # Accessors: an extra class reads its shadow as class 0.
    stubs = []
    for va, old, at in accessors:
        replaced = bytes.fromhex(old)
        layout.pad()
        stub = layout.here()
        found = []
        for index, shadow in shadows.items():
            layout.body += b'\x83\x7c\x24' + bytes((at, index))
            other = layout.forward(b'\x0f\x85')
            layout.body += b'\xc7\x44\x24' + bytes((at,)) + bytes(4) + b'\xb9'
            layout.pointer(shadow)
            found.append(layout.forward(b'\xe9'))
            layout.land(other)
        for at_ in found:
            layout.land(at_)
        layout.body += replaced
        layout.to(b'\xe9', va + len(replaced))
        edits.append((va, replaced, b'\xe9' + struct.pack('<i', stub - (va + 5)) + b'\x90' * (len(replaced) - 5)))
        stubs.append(f'{va:#x} -> {stub:#x}')
    return {'shadow': hex(shadows[EXTRA_CLASSES[0].index]), 'shadows': {i: hex(a) for i, a in shadows.items()},
            'real': hex(real), 'block': f'{block:#x} -> {block_stub:#x}', 'next': f'{after:#x} -> {after_stub:#x}',
            'accessors': stubs}


def fallback_entity(image: Image, edits: list, factory: str) -> str:
    """Point the server's missing-factory fallback at an existing class factory (diagnostic)."""
    if factory not in FALLBACK_ENTITIES:
        raise PreparationError(f'Fallback entity must be one of {FALLBACK_ENTITIES}.')
    o = image.offset(FALLBACK_PUSH)
    old = bytes(image.data[o:o + 5])
    if old[0] != 0x68 or read_string(image, struct.unpack_from('<I', old, 1)[0]) != 'player':
        raise PreparationError(f'Unexpected fallback at {FALLBACK_PUSH:#x}; nothing is patched.')
    found = image.data.find(factory.encode('ascii') + b'\0')
    for section, _, vaddr, rsize, raw in image.sections():
        if raw <= found < raw + rsize and image.data[found - 1] == 0:
            address = image.base + vaddr + found - raw
            break
    else:
        raise PreparationError(f'No {factory!r} string in the image.')
    edits.append((FALLBACK_PUSH, old, b'h' + struct.pack('<I', address)))
    return f'{FALLBACK_PUSH:#x}: "player" -> "{factory}" ({address:#x})'


def class_name(layout: Layout, edits: list, entry: int, old: str, compare: int) -> str:
    """CLASS_NAME stub: an extra class key -> its index, anything else -> the original lookup."""
    layout.pad()
    names = {extra.index: layout.string(extra.key) for extra in EXTRA_CLASSES}
    layout.pad()
    stub = layout.here()
    layout.body += b'\x8b\x44\x24\x04' + b'\x85\xc0'  # mov eax, [esp + 4]; test eax, eax; je orig
    null = layout.forward(b'\x0f\x84')
    for index, name in names.items():
        layout.body += b'\x68'
        layout.pointer(name)
        layout.body += b'\xff\x74\x24\x08'  # push [esp + 8] (the name argument)
        layout.to(b'\xe8', compare)
        layout.body += b'\x83\xc4\x08' + b'\x85\xc0' + b'\x75\x06' + b'\xb8' + struct.pack('<I', index) + b'\xc3'
    layout.land(null)
    replaced = bytes.fromhex(old)
    layout.body += replaced
    layout.to(b'\xe9', entry + len(replaced))
    edits.append((entry, replaced, b'\xe9' + struct.pack('<i', stub - (entry + 5))))
    return f'{entry:#x} -> {stub:#x}'


def event_file(layout: Layout, edits: list, shadows: dict, va: int, old: str, extreme: int,
               resume: int, skip: int, keep: int) -> str:
    """EVENT_FILE stub: an extra class takes its event path from its shadow costume column."""
    layout.pad()
    stub = layout.here()
    for index, shadow in shadows.items():
        layout.body += b'\x83\xfe' + bytes((index,))
        other = layout.forward(b'\x0f\x85')
        layout.body += b'\x57'
        layout.to(b'\xe8', extreme)
        layout.body += b'\x84\xc0' + b'\x74\x07' + b'\xb9'
        layout.pointer(shadow + 0xC80)  # event_file_name_xe, column 0
        layout.body += b'\xeb\x05' + b'\xb9'
        layout.pointer(shadow + 0xBE0)  # event_file_name, column 0
        layout.to(b'\xe9', resume)
        layout.land(other)
    # Other classes: the original `cmp esi, 9; ja skip`, then back to `push edi`.
    layout.body += b'\x83\xfe\x09' + b'\x0f\x87' + struct.pack('<i', skip - (layout.here() + 3 + 6))
    layout.body += b'\xe9' + struct.pack('<i', keep - (layout.here() + 5))
    edits.append((va, bytes.fromhex(old), b'\xe9' + struct.pack('<i', stub - (va + 5))))
    return f'{va:#x} -> {stub:#x}'


def file_va(image: Image, at: int) -> int:
    for _, _, vaddr, rsize, raw in image.sections():
        if raw <= at < raw + rsize:
            return image.base + vaddr + at - raw
    raise PreparationError(f'File offset {at:#x} is outside every section.')


def vtable(image: Image, name: str) -> int:
    """The primary vtable of class `name`, found through its RTTI (type descriptor, then the
    complete object locator at offset 0 that the vtable's -4 entry points at)."""
    data = bytes(image.data)
    at = data.find(b'\0.?AV' + name.encode('ascii') + b'@@\0')
    if at < 0:
        raise PreparationError(f'No RTTI for {name}; nothing is patched.')
    descriptor = file_va(image, at + 1) - 8
    for found in re.finditer(re.escape(struct.pack('<I', descriptor)), data):
        locator = file_va(image, found.start()) - 12
        if struct.unpack_from('<II', data, image.offset(locator)) == (0, 0):
            for ref in re.finditer(re.escape(struct.pack('<I', locator)), data):
                return file_va(image, ref.start()) + 4
    raise PreparationError(f'No primary vtable for {name}; nothing is patched.')


def entity_slots(image: Image, layout: Layout, edits: list, relocs: set[int], name: str, klass: int,
                 figure_slot: int, constant_slots: tuple[int, int], figure_gate: int) -> dict:
    """ENTITY_SLOTS: per-class dispatchers in the shared player entity's vtable."""
    table = vtable(image, name)
    layout.pad()
    figures = {extra.index: layout.string(extra.figure) for extra in EXTRA_CLASSES}
    layout.pad()
    constants = {}
    for extra in EXTRA_CLASSES:
        constants[extra.index] = layout.here()
        layout.body += struct.pack('<ff', *extra.constants)

    def dispatch(slot: int, body) -> str:
        at = table + 4 * slot
        if at not in relocs:
            raise PreparationError(f'{name} slot {slot} is not relocated; nothing is patched.')
        original = struct.unpack_from('<I', image.data, image.offset(at))[0]
        layout.pad()
        stub = layout.here()
        layout.body += b'\x8b\x81' + struct.pack('<I', klass)  # mov eax, [ecx + m_iClass]
        for extra in EXTRA_CLASSES:
            layout.body += b'\x83\xf8' + bytes((extra.index,))
            other = layout.forward(b'\x0f\x85')
            body(extra)
            layout.land(other)
        layout.to(b'\xe9', original)
        edits.append((at, struct.pack('<I', original), struct.pack('<I', stub)))
        return f'slot {slot} {original:#x} -> {stub:#x}'

    def figure(extra: ExtraClass) -> None:
        # The gate returns null when the figure is off; otherwise the class's own figure.
        layout.to(b'\xe8', figure_gate)
        layout.body += b'\x85\xc0' + b'\x74\x05' + b'\xb8'
        layout.pointer(figures[extra.index])
        layout.body += b'\xc3'

    def constant(index: int):
        def body(extra: ExtraClass) -> None:
            layout.body += b'\xd9\x05'  # fld dword [constant]
            layout.pointer(constants[extra.index] + 4 * index)
            layout.body += b'\xc3'
        return body

    return {'vtable': hex(table), 'figure': dispatch(figure_slot, figure),
            'constants': [dispatch(slot, constant(i)) for i, slot in enumerate(constant_slots)]}


def plan_dll(name: str, data: bytes, fallback: str | None = None) -> tuple[bytes, dict]:
    check_classes(EXTRA_CLASSES)
    image = Image(data)
    if any(s[0] == SECTION for s in image.sections()):
        raise PreparationError(f'{name} already has .hrte; restore the pre-routing backup to patch again.')
    if not any(s[0] == b'.hcls\0\0\0' for s in image.sections()):
        raise PreparationError(f'{name} needs the .hcls patch first (patch_class_table.py).')
    relocs = relocations(image)
    rva = align(max(v + max(vs, rs) for _, vs, v, rs, _ in image.sections()), image.section_align)
    layout = Layout(image.base + rva)
    edits, report = [], {'switches': [], 'tables': [], 'bits': None}

    # Strings first, so clone_case can refer to them by address.
    wanted, heads = {}, {}
    for va, bound, _ in SWITCHES.get(name, ()):
        _, default, table, _ = switch_parts(image, va, bound)
        cases = {struct.unpack_from('<I', image.data, image.offset(table + 4 * i))[0] for i in range(bound + 1)}
        heads[va] = (default, table)
        for extra in EXTRA_CLASSES:
            source = OLD_CLASSES.index(extra.clone_of)
            case = struct.unpack_from('<I', image.data, image.offset(table + 4 * source))[0]
            # The copied case ends at the next case, the default or the join the other cases jump
            # to (the last case usually falls through into it).
            first = min(cases)
            joins = {int(i.op_str, 16) for i in md.disasm(bytes(image.data[image.offset(first):image.offset(case)]), first)
                     if i.mnemonic == 'jmp' and re.fullmatch(r'0x[0-9a-f]+', i.op_str)}
            wanted[va, extra.index] = (case, min(a for a in cases | joins | {default} if a > case))
    texts = set()
    for extra in EXTRA_CLASSES:
        for va, _, _ in SWITCHES.get(name, ()):
            case, end = wanted[va, extra.index]
            for a in range(case, end - 3):
                text = read_string(image, struct.unpack_from('<I', image.data, image.offset(a))[0]) if a in relocs else None
                if text and extra.clone_of in text.lower():
                    texts.add(rename(text, extra.clone_of, extra.key))
        texts |= {filled(text, extra) for _, text in TABLES.get(name, ()) if text}
    for text in sorted(texts):
        layout.string(text)
    layout.pad()

    for va, bound, pet in SWITCHES.get(name, ()):
        register, _, _, disp = switch_parts(image, va, bound)
        default, table = heads[va]
        stubs, strings = {}, []
        for extra in EXTRA_CLASSES:
            case, end = wanted[va, extra.index]
            stubs[extra.index] = layout.here()
            strings += clone_case(image, layout, case, end, relocs, extra.clone_of, extra.key)
            layout.pad()
        entries = [struct.unpack_from('<I', image.data, image.offset(table + 4 * i))[0] for i in range(bound + 1)]
        count = PET + 1 if pet else LAST + 1
        new_table = layout.here()
        for i in range(count):
            if i < 10:
                layout.pointer(entries[i])
            elif i in stubs:
                layout.pointer(stubs[i])
            elif pet and i == PET:
                layout.pointer(entries[10])
            else:
                layout.pointer(default)
        edits.append((va + 2, bytes((bound,)), bytes((count - 1,))))
        edits.append((disp, struct.pack('<I', table), struct.pack('<I', new_table)))
        report['switches'].append({'cmp': hex(va), 'register': register, 'bound': f'{bound} -> {count - 1}',
                                   'pet': PET if pet else None, 'delia_case': hex(stubs[DELIA]),
                                   'cases': {i: hex(a) for i, a in stubs.items()}, 'strings': strings})

    extras = {extra.index: extra for extra in EXTRA_CLASSES}
    for va, text in TABLES.get(name, ()):
        entries = [struct.unpack_from('<I', image.data, image.offset(va + 4 * i))[0] for i in range(10)]
        new_table = layout.here()
        for i in range(TABLE_SIZE):
            value = entries[i] if i < 10 else (
                (layout.strings[filled(text, extras[i])] if text else 0) if i in extras else entries[0])
            if value:
                layout.pointer(value)
            else:
                layout.body += b'\0\0\0\0'
        users = []
        for at in sorted(relocs):
            if image.text_contains(at):
                value = struct.unpack_from('<I', image.data, image.offset(at))[0]
                if va <= value < va + 40:
                    edits.append((at, struct.pack('<I', value), struct.pack('<I', new_table + value - va)))
                    users.append(hex(at))
        if not users:
            raise PreparationError(f'No reference to the table at {va:#x}; nothing is patched.')
        report['tables'].append({'table': hex(va), 'moved_to': hex(new_table), 'delia': filled(text, extras[DELIA]),
                                 'references': users})

    for va in CLASS_STRING_BOUNDS.get(name, ()):
        edits.append((va, b'\x83\xf8\x09', b'\x83\xf8' + bytes((LAST,))))
    if name in BITS:
        edits.append((BITS[name], b'\x6a\x0a', bytes((0x6A, PET))))
        report['bits'] = f'm_iClass nBits log2(10)+1 -> log2({PET})+1'
    report['gates'] = [hex(va) for va in GATES.get(name, ())]
    for va in GATES.get(name, ()):
        edits.append((va, b'\x83\xf8\x0a', bytes((0x83, 0xF8, PET))))
    if name in MATCH:
        report['match_table'] = match_table(image, layout, edits, relocs, *MATCH[name])
    report['flag_checks'] = [flag_check(layout, edits, *entry) for entry in FLAG_CHECKS.get(name, ())]
    report['tooltips'] = [tooltip(image, layout, edits, relocs, *entry) for entry in TOOLTIPS.get(name, ())]
    if name in PRECACHE:
        report['precache'] = precache(layout, edits, *PRECACHE[name])
    if name in COSTUME:
        report['costume'] = costume(image, layout, edits, relocs, *COSTUME[name])
    if name in COSTUME_GATE:
        edits.append((COSTUME_GATE[name], b'\x83\xf8\x09', b'\x83\xf8' + bytes((LAST,))))
        report['costume_gate'] = f'{COSTUME_GATE[name]:#x}: classes 0-{LAST}'
    if name in CLASS_NAME:
        report['class_name'] = class_name(layout, edits, *CLASS_NAME[name])
    for va in CLASS_BITS.get(name, ()):
        head = bytes(image.data[image.offset(va):image.offset(va) + 3])
        if head[0] != 0x83 or head[1] & 0xF8 != 0xF8 or head[2] != 9:
            raise PreparationError(f'Unexpected class check at {va:#x}; nothing is patched.')
        edits.append((va, head, head[:2] + bytes((LAST,))))
    if name in CLASS_BITS:
        report['class_bits'] = [f'{va:#x}: classes 0-{LAST}' for va in CLASS_BITS[name]]
    if name in EVENT_FILE:
        shadows = {int(i): int(a, 16) for i, a in report['costume']['shadows'].items()}
        report['event_file'] = event_file(layout, edits, shadows, *EVENT_FILE[name])
    if name in ENTITY_SLOTS:
        report['entity_slots'] = entity_slots(image, layout, edits, relocs, *ENTITY_SLOTS[name])
    if fallback and name == 'server.dll':
        report['fallback_entity'] = fallback_entity(image, edits, fallback)

    for va, old, new in edits:
        o = image.offset(va)
        if bytes(image.data[o:o + len(old)]) != old:
            raise PreparationError(f'Unexpected bytes at {va:#x}; nothing is patched.')
    if image.table + 40 * (image.count + 1) > min(s[4] for s in image.sections()):
        raise PreparationError('No room for another section header.')

    # Relocation directory: the current one, then a block per 4 KiB page of new pointers.
    reloc_rva, reloc_size = image.directory(5)
    old_relocs = bytes(image.data[image.offset(image.base + reloc_rva):image.offset(image.base + reloc_rva) + reloc_size])
    layout.pad()
    reloc_at = len(layout.body)
    pages = {}
    for p in layout.pointers:
        pages.setdefault((rva + p) & ~0xFFF, []).append((rva + p) & 0xFFF)
    blocks = b''
    for page, offsets in sorted(pages.items()):
        # HIGHLOW entries; an odd count ends with an ABSOLUTE (type 0) entry to keep blocks 4-aligned.
        entries = [0x3000 | o for o in sorted(offsets)] + ([0] if len(offsets) % 2 else [])
        blocks += struct.pack('<II', page, 8 + 2 * len(entries)) + b''.join(struct.pack('<H', e) for e in entries)
    layout.body += old_relocs + blocks
    body = bytes(layout.body)

    raw = align(len(image.data), image.file_align)
    image.data += b'\0' * (raw - len(image.data)) + body + b'\0' * (align(len(body), image.file_align) - len(body))
    struct.pack_into('<8sIIIIIIHHI', image.data, image.table + 40 * image.count, SECTION, len(body), rva,
                     align(len(body), image.file_align), raw, 0, 0, 0, 0, 0xE0000020)  # code, read, execute, write (Delia's map)
    image.count += 1
    struct.pack_into('<H', image.data, image.pe + 6, image.count)
    struct.pack_into('<I', image.data, image.opt + 56, align(rva + len(body), image.section_align))
    struct.pack_into('<I', image.data, image.opt + 64, 0)
    image.set_directory(5, rva + reloc_at, len(old_relocs) + len(blocks))
    for va, old, new in edits:
        o = image.offset(va)
        image.data[o:o + len(new)] = new
    report['section'] = {'va': hex(image.base + rva), 'size': len(body), 'pointers': len(layout.pointers)}
    return bytes(image.data), report


def _text_contains(self: Image, va: int) -> bool:
    for name, vsize, vaddr, _, _ in self.sections():
        if name.startswith(b'.text') and vaddr <= va - self.base < vaddr + vsize:
            return True
    return False


Image.text_contains = _text_contains


def patch(client_copy: Path, apply: bool = False, fallback: str | None = None) -> dict:
    plan = {'mode': 'apply' if apply else 'dry-run', 'pet': PET, 'dlls': []}
    for name, prefix in EXPECTED.items():
        dll = reject_original(client_copy / 'bin' / name)
        data = dll.read_bytes()
        if not digest(data).startswith(prefix):
            raise PreparationError(f'{name}: expected the .hcls copy {prefix}…, found {digest(data)[:16]}.')
        image, report = plan_dll(name, data, fallback)
        backup = dll.with_name(f'{name}.{digest(data)[:16]}.pre-routing.bak')
        plan['dlls'].append({'dll': str(dll), 'before_sha256': digest(data), 'after_sha256': digest(image),
                             'backup': str(backup), **report})
        if apply:
            if not backup.exists():
                backup.write_bytes(data)
            partial = dll.with_suffix('.partial')
            partial.write_bytes(image)
            os.replace(partial, dll)
    if apply:
        # Addresses of the applied stubs and data, read by scripts/dev/lab_memory.py.
        (client_copy / 'bin' / 'class-routing.json').write_text(json.dumps(plan, indent=1), encoding='utf-8')
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--fallback-entity', choices=FALLBACK_ENTITIES,
                        help='diagnostic: entity for a class without its own (default: the base player)')
    args = parser.parse_args()
    try:
        print(json.dumps(patch(args.client_copy, args.apply, args.fallback_entity), indent=1))
    except PreparationError as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
