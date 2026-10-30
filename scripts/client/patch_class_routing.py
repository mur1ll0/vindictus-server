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
from patch_engine_class_flags import DELIA_FLAG  # noqa: E402
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
    shape_of: str = 'lynn'          # older class whose body-shape slot it borrows (SHAPE_CLASS)
    weapons: tuple[str, ...] = ()   # weapon entities of the class (CLASS_WEAPONS)
    server_constants: tuple[float, ...] = (0.9, 1.1, 100.0)  # server slots 221, 222, 414

    @property
    def name(self) -> str:
        return self.key.capitalize()


EXTRA_CLASSES = (
    # The NA gives Delia weapon_bastardsword, an entity the 1.69 lacks; the lab uses the closest
    # existing one, Hurk's weapon_greatsword (one large sword, sheathed on the back).
    # Server constants: NA Delia slots 240/241 (0.88/0.96, as the client) and 466 (92.0).
    ExtraClass(10, 'delia', 'hagie', 'female', 'csf/bodyshape_delia_default.fig', (0.88, 0.96), 3,
               weapons=('weapon_greatsword',), server_constants=(0.88, 0.96, 92.0)),
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
        (0x384A3308, 9, False),   # customization "sample outfits": CsfChanged.create_/trial_<class>1-4
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
        # Player spawn matches cl_playerclass against these keys (0x4055a320, bound in
        # SERVER_CLASS_BOUNDS) and sets class 0 when none matches: a Delia created by the class grid
        # (no town character data yet) spawned as Lethita in the customization (C3).
        (0x40CFA1B0, '{key}'),
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
# Server class bounds: (VA, original bytes, replacement from LAST). The spawn's key loop
# (`cmp esi, 0xa`) and CHeroesPlayer::SetClass (0x4055a190: `cmp eax, 9` ... `mov ebx, 9`, the cap).
SERVER_CLASS_BOUNDS = {'server.dll': (
    (0x4055A336, '83fe0a', lambda last: bytes((0x83, 0xFE, last + 1))),
    (0x4055A194, '83f809', lambda last: bytes((0x83, 0xF8, last))),
    (0x4055A19E, 'bb09000000', lambda last: b'\xbb' + struct.pack('<I', last)),
)}
# The client entity every extra class gets (the server falls back to "player", CHeroesPlayer)
# and the methods its own class entity overrides in the NA:
# * slot 264 returns the body figure (csf/bodyshape_<class>_default.fig). Hagie's (0x3870d1d0)
#   returns hers unless BodyShapeModify is on and the game state is not 0x1f; the base returns
#   none, and Delia's body was deformed from the belly to the thighs at character select (D8v);
# * slots 177/178 return two float constants (base 0.9/1.1, Hagie 0.91/0.99, NA Delia 0.88/0.96).
# Each slot of that vtable points at a dispatcher on m_iClass (+0x18ec): an extra class gets
# Hagie's figure gate with its own figure and its constants; the others run the original.
# The server's CHeroesPlayer (m_iClass +0x1400) has the same pair at 221/222 and a third NA
# override at 414 (NA 466: base 100, Lynn 89, Delia 92). Entries: (class, m_iClass offset,
# figure slot or None, constant slots, figure gate or None, ExtraClass field with the values).
ENTITY_SLOTS = {'client.dll': ('C_HeroesPlayer', 0x18EC, 264, (177, 178), 0x3870D1D0, 'constants'),
                'server.dll': ('CHeroesPlayer', 0x1400, None, (221, 222, 414), None, 'server_constants')}
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
# "Is this class allowed" accessors over the item's per-class bools (+0x198, ten entries, filled
# by engine.dll; Delia's is DELIA_FLAG, see patch_engine_class_flags.py): a `switch` on classes 0-9
# returning flags[class], so class 10 got false ("Cannot be equipped", D8e). Each head jumps to a
# stub that returns flags[class] for classes 0-9, DELIA_FLAG for class 10 and keeps the default
# (false) above. Entries: (VA of
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
# Delia's flag (DELIA_FLAG) and naming #GameUI_Delia; a Delia that is not allowed is skipped instead of
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
# Body-shape manager (csf/bodyshape_min/max_<class>.fig, the .bsh sliders): the two per-entity
# entry points (0x38760650 loads, 0x387606b0 applies) take the class from the entity and skip
# above 9 (`cmp reg, 9; ja`), because the manager keeps four groups of 10 per-class maps and flags
# at +0x460. Character select is the only screen that blends body shapes, and Delia's lower-body
# skin rendered white there (D12-D15). Experiment (D15): an extra class borrows the slot of an
# older class (`shape_of`) instead of skipping; the manager's own storage is unchanged.
# Entries: (VA of `cmp reg, 9; ja`, its bytes, register).
SHAPE_CLASS = {'client.dll': ((0x3876066F, '83fe097733', 'esi'), (0x387606DE, '83fb090f87dc020000', 'ebx'))}
# Costume default tables indexed [slot][10] without a bound (`table[slot*10 + class]`): the body
# layer default (server 0x40815490 over the table at 0x40f6494c, all -1) and its sibling, and the
# client copies. Class 10 read the dword after the table, a pointer: the selection NPC
# (0x40411e60) networked it as m_bodyLayerList[0] and the client drew a white body decal over the
# hips and thighs (D12-D16). An extra class now reads its `clone_of` column.
CLASS_DEFAULTS = {'client.dll': (0x3887CE30, 0x3887CF30), 'server.dll': (0x40815390, 0x40815490)}
CLASS_DEFAULT_HEAD = bytes.fromhex('8b4424048b5424088d04808d04428b0481c20800')
# Per-class weapon entity lists (a CUtlVector<const char*> per class, 0x14 bytes, filled at load
# by client 0x3888e4d0 / server 0x408256a0; NA: Delia weapon_bastardsword, Miri weapon_draker).
# Every reader clamps `cmp R, 0xa; jl; xor R, R; lea X, [R + R*4]` before `[X*4 + table]`, so
# class 10 got Lethita's list and a Bastard Sword showed as dual swords (D14). An extra class now
# gets its own row in .hrte (`weapons`), reached by setting X so that X*4 + table is that row.
CLASS_WEAPONS = {'client.dll': 0x38FBC6C0, 'server.dll': 0x40F68510}
WEAPON_SLOTS = 5
# Places that load m_iClass and then compare it with each older class: an extra class is loaded
# as its `clone_of` class there. Server 0x4053b560 sets the stamina restoration rate (48 for
# classes 0, 1, 3-9, 12 for Evy, 0 for anything else): Delia regenerated almost nothing (D18).
# Entries: (VA of `mov reg, [base + 0x1400]`, its bytes).
CLASS_ALIAS = {'server.dll': ((0x4053B5BE, '8b8600140000'),   # stamina restoration rate
                              (0x40539AEC, '8b8600140000'))}  # per-class timing value (switch of 10, 0 above)
# The client's player spawn (0x38705f30) switches on the class to hand its weapon entity name to
# vtable +0x55c (`push ebx; push name; jmp 0x38705fb4`); above 9 it skipped that. An extra class
# passes its first `weapons` entry. (VA of `cmp eax, 9; ja`, its bytes, the join.)
CLIENT_WEAPON_SWITCH = {'client.dll': (0x38705F62, '83f8097759', 0x38705FB4)}
# Action-state conditions (E3): named flags in a 121-bit set per animation state; four functions
# per DLL bound the id by `cmp ecx, 0x79`, and the registration (client 0x387f33b0, server
# 0x407f5470) fills name->id and id->name maps with ids 0-120. Delia's scripts use five names
# the 1.69 lacks (NA ids 140-144); they get ids 121-125 (a 121-bit set already spans 128 bits),
# appended at the registration's epilogue. Entries: (epilogue `pop edi; mov al, 1; pop esi;
# mov esp, ebp`, name map, its insert, id map, its insert, the four bounds).
CONDITION_NAMES = ('As_Cond_Can_Armor_Purge', 'As_Cond_Delia_SPSkill3_Activated', 'AS_Cond_Delia_Special_Hold_Skill',
                   'AS_Cond_Delia_Guard', 'AS_Cond_Delia_Guard_Hit')
FIRST_NEW_CONDITION = 121
CONDITIONS = {
    'client.dll': (0x387F4E79, 0x38FAB1D8, 0x387F29C0, 0x38FAB1F4, 0x387F2A30,
                   (0x387DE3FB, 0x387DE44B, 0x387DE55B, 0x387DE5BB)),
    'server.dll': (0x407F6F39, 0x40F5D430, 0x407F4800, 0x40F5D44C, 0x407F4870,
                   (0x407E63CB, 0x407E641B, 0x407E647B, 0x407E64DB)),
}
CONDITION_TAIL = bytes.fromhex('5fb0015e8be5')  # pop edi; mov al, 1; pop esi; mov esp, ebp
# Guard (E3). The guard state is the base player's "class state" pair, slots 618 (set; the
# action-state event ENABLE_ENDURE_MODE calls it with 1) and 619 (get); the base does nothing
# and returns false, so Delia's guard animation never blocked (D19). Fiona keeps the state in a
# cell of her own object and blocks in slot 555 (0x40575a50: unblockable flag 0x8000000 at
# info+0x3c, damage zeroed, reaction, then the base 555). For an extra class: 618/619 keep the
# state in a 16-entry (entity, state) table in .hrte, 555 raises AS_Cond_Delia_Guard and
# AS_Cond_Delia_Guard_Hit (NA: on each blocked hit) and runs Fiona's routine, which only uses
# base fields; ending the guard lowers both conditions.
# (set slot, get slot, damage slot, guarded-damage routine, forced-off test, condition object,
#  raise, lower)
GUARD = {'server.dll': (618, 619, 555, 0x40575A50, 0x4060FE00, 0x405380A0, 0x407E6470, 0x407E64D0)}
GUARD_ENTRIES = 16
# Guard, client side (plan 8.15 item 1). A hit on the local player is decided by the client: the
# server's guarded-damage routine honours a client hit (info+0x3c & 0x10000000) only when the
# client set bit 2 of info+0x78, so the server state alone never blocked Delia's hits (live test
# 29/10/2026: damage with the server state forced to 1). Fiona's client slot 367 (0x3870cef0)
# copies the hit, reads slot 430 and, when the hit is guardable (two routines false), sets bit 2,
# clears byte +0x79, zeroes the damage (+0x30, +0x48), raises As_Cond_Do_PerfectEndurable (0x4d)
# under As_Cond_Can_PerfectEndurable (0x4c) and calls the base 367 with the copy; her fields
# (+0x4ba0..+0x4bac) are past the base object and are not reused. ENABLE/DISABLE_ENDURE_MODE
# (0x38899046) call slot 429 on the client too. For an extra class: 429/430 keep a state table,
# 367 is Fiona's decision plus AS_Cond_Delia_Guard/_Hit (124/125, NA: on each guarded hit), and
# the end of the guard lowers both. Conditions live on [this+0x437c] (+0x140 set). Fiona also sets
# the base byte +0x4b3d (HIT_IMPACT_SKIP) on a guarded hit: the base reaction (0x386f32f7) then
# skips the hit impact and its sound (0x388db3c0) once and clears it (D22: Delia blocked the damage
# but still played the hit sound).
# (set slot, get slot, damage slot, hit copy, unguardable(info, commands), unguardable(info),
#  raise, test, lower, allocator global)
CLIENT_GUARD = {'client.dll': (429, 430, 367, 0x38707FB0, 0x38888400, 0x38888180,
                               0x387DE550, 0x387DE580, 0x387DE5B0, 0x38ADA3C4)}
HIT_IMPACT_SKIP = 0x4B3D
# Per-frame class conditions (plan 8.15 item 2). NA server 0x6e4d30 (slot 517 = 1.69 460) raises
# As_Cond_Can_Armor_Purge (SP5's gate) while armour is equipped in slots 4-8 and the purge is not
# blocked, and As_Cond_Delia_SPSkill3_Activated while skill_delia_sp3 is off cooldown (SP1/SP3
# `_one` follow-ups); the NA client mirrors both from the network. In 1.69 Kalok raises his own in
# the client think (slot 334, after the base 0x387065a0) and the server think (460). For an extra
# class both thinks run the base and then raise Can_Armor_Purge (lab approximation: armour is
# always worn; the purge block is not ported). SP3_Activated stays off: the 1.69 cooldown store of
# SP skills was not found (NA: 32 ids at +0x18, last use at +0x228, 0x29bf70). SET_GUARD_LATCH_ATTEN
# is not used by any Delia content (1.69 or NA) and is not ported.
# (think slot, condition object getter or None for [this+COMMANDS], raise)
CLASS_CONDITIONS = {'client.dll': (334, None, 0x387DE550), 'server.dll': (460, 0x405380A0, 0x407E6470)}
# Delia's status effects and on-hit commands (plan 8.15 items 3-4), server only. The 1.69
# heroes.db3 and scripts/status_effect.txt already define her effects, but server.dll registers
# no factory for five of their types, so those effects did nothing. Registered after
# stamina_adder_on_attack (its registrar's `ret`), effect objects as the 1.69 builds them (base
# ctor, 7-slot vtable: 1 load(flag), 2 tick, 3 remove(flag), 4 on hit(info, victim), 5 on damage
# taken(info, victim)), behaviour from the NA classes (server x64 factories 0xad89f0, 0xad8900,
# 0xad89a0, 0xad8880, 0xad8950):
# * stamina_adder_on_attack_time (adder, validCount, effectName): stamina_adder_on_attack limited
#   to validCount hits (the effect is a client visual, not ported);
# * delia_restore_health (reduce_ratio, restore_time; Wild Star's last stage): takes the ratio of
#   current HP (at least 1 HP is kept) into a reserve that the think gives back at
#   reserve/restore_time per second; a hit taken that is neither guarded nor unblockable forfeits
#   the rest and removes skill_delia_strong_attack_week (NA 0x6e60e0, 0x6e7054, 0x6e6320);
# * skill_delia_strong_attack (delay_time, immune_count; Sharpness 1-3): after delay_time each hit
#   taken that is neither guarded nor unblockable uses one count; below zero the buff drops one
#   level (NA slot 6 0xad8210, decay in slot 3); the UPDATE/REMOVE_CHARGE_SMASH_EFFECT client
#   visuals are not ported;
# * damage_modify_to_action (candidate_actions, damage_rates): a hit whose attack name (info+0x60)
#   is a candidate is multiplied; the first rate applies to every candidate (all Delia rows use
#   one rate twice);
# * restore_equipment_def (add_def): adds armour HP like armor_hp_adder (0x40530da0); the reset
#   on removal and the purge flag are not ported.
# The NA commands ENABLE/DISABLE_STACK_CHARGE_SMASH_STATUS_EFFECT (+0xad40) and
# ENABLE/DISABLE_ADD_STRONG_ATTACK_STATUS_EFFECT (+0xad41) (NA 0x6e52c9..0x6e5422) are taken at the
# entry of the AE_CUSTOM_MOTION dispatcher (0x40838f20) for an extra class; on a hit (slot 70,
# NA 0x6e58c0 overriding slot 73) the first adds skill_delia_strong_attack one level up to 3 and
# clears itself, the second adds skill_delia_strong_attack_hit when it is absent or at level 1.
# Decay and forfeit are carried out by the think, never inside an effect callback.
# PURGE_ALL_ARMOR (Rose Crush, delia_SP5 frame 125 in the 1.69 pc_delia_motion_ex.txt) is unknown
# to the 1.69 dispatcher. NA (0x6e5168) breaks armour slots 4-8 through slot 628 (0x6e77a0 ->
# 0x6c4f20(slot, broken 1, notify 0): the costume state only, no model rebuild), zeroes their armour
# HP and sets the purge flag (+0xaf48). The 1.69 armour break (0x40530ce0 -> 0x40565b00) always
# notifies: the notify path (0x405465d0) rebuilds the costume model (CoStUmE_<parts>.mdl, which is
# what shows the broken parts in 1.69) and sets it at once. Run from inside the animation event
# dispatch, that frees the model the dispatch is still reading (D23: server 0x40383bab,
# CStudioHdr::pSeqdesc); the 1.69's own breaks run from damage, outside it. A purge also takes the
# armour off the body, leaving the inner wear (operator, D23: the original shows only the inner
# armour after Rose Crush, while a break by a hit shows the broken look): the costume's worn part
# of the slot (+0x94[slot], networked, copied into the model name by 0x4081be60) becomes -1 and
# the part is kept in the entry (+0x1c + 4 * (slot - 4), part + 1). When the slot stops being broken
# (a repair; every un-break goes through 0x40547480(slot, 0, notify)), the entry of 0x40547480
# puts the kept part back (when the slot is still hidden) and asks the think for a rebuild
# (operator, D23: the repair kit fixed the armour but it did not come back). Where the armour system
# is off (the game state check 0x40740a10 on [0x40d6f5e4] false, e.g. the ship: every slot reads as
# empty to 0x40814620, so neither the purge nor the repair kit touches the armour), the purge only
# hides the worn parts, and the player's repair (0x40530cf0, called by the repair kit) puts the kept
# parts back first: a look only, as the operator asked (D23). Both then run the costume recompute
# (0x4081be60: worn +0x94 -> resolved +0x19c, which names the model); a real break or repair runs
# it inside 0x4081c170, but the look-only path did not (D23 on the ship, memory: worn parts -1 and
# the resolved parts and the rebuilt CoStUmE_ name still with them). For an extra class
# each of slots 4-8 that has armour
# (0x40814620 false) gets HP 0 (0x408144d0), its part hidden and the broken state without notify
# (0x40547480(slot, 1, 0)), the armour status update (0x4054e160) runs once, and the rebuild is left
# to the think (entry +0x18), after the base think has finished its animation events. The
# purge flag and NA's slot-4 look (0x6bd370) are not ported. The command is server only (NA
# client.dll has no PURGE string), so a town area without the game server never purges.
DELIA_EFFECTS = {'server.dll': {
    'register': 0x404FD6D0, 'hook': 0x40A087D4, 'new': 0x401D3890, 'base_ctor': 0x404F6290,
    'dtor': 0x4050C9D0, 'base_slots': (0x404F57D0, 0x404F57E0, 0x404F57F0, 0x404F5800, 0x404F5810),
    'owner': 0x404F58E0, 'player_owner': 0x404F5930, 'arg': 0x404F6AC0, 'arg_text': 0x404F5C50,
    'ftol': 0x40993AC0, 'globals': 0x40D6EE48, 'stamina_load': 0x4050C820, 'stamina_hit': 0x4050C9F0,
    'health_set': 0x4002AA90, 'armor_add': 0x40530DA0, 'hit_name': 0x408BCCA0, 'level': 0x404FDF40,
    'strncmp': 0x408B7A50, 'events': 0x40838F20, 'hit_slot': 70, 'armor': 0x148C, 'armor_none': 0x40814620,
    'armor_hp_set': 0x408144D0, 'armor_state': 0x40547480, 'armor_update': 0x4054E160, 'costume': 0x405465D0,
    'clothes': 0x156C, 'worn_part': 0x94, 'armor_on': 0x40740A10, 'game_state': 0x40D6F5E4,
    'repair': 0x40530CF0, 'costume_parts': 0x4081BE60}}
DELIA_TABLE_ENTRIES, DELIA_ENTRY = 16, 0x40
# entry: +0 entity, +4 reserved HP, +8 restore per second, +0xc fraction, +0x10 last restore time,
# +0x14 stack-charge-smash flag, +0x15 add-strong-attack flag, +0x16 decay due, +0x17 forfeit due,
# +0x18 costume rebuild due (PURGE_ALL_ARMOR), +0x1c..+0x2f worn part + 1 of purged slots 4-8
DELIA_KEPT_PART = 0x1C - 4 * 4  # + 4 * slot
ARMOR_STATE_HEAD = bytes.fromhex('8b442408568bf1')  # mov eax, [esp+8]; push esi; mov esi, ecx
REPAIR_HEAD = bytes.fromhex('d944240451')  # fld dword [esp+4]; push ecx
EVENTS_HEAD = bytes.fromhex('515356578b7c2414')  # push ecx/ebx/esi/edi; mov edi, [esp+0x14]
REGISTRAR_TAIL = bytes.fromhex('c3cccccccc')
PLAYER_SLOTS = {'add': 0x380, 'find': 0x388, 'remove': 0x390, 'npc': 0x160, 'changed': 0x230,
                'guarding': 0x9AC}
PLAYER_FIELDS = {'max_health': 0x80, 'health': 0x84, 'health_cell': 0x90, 'effects': 0x2878, 'dead': 0x9C,
                 'commands': 0xC8}
# client 0x384ac450 rebuilds an item's class mask from the per-class flags (+0x198..+0x1a1, ten
# bits); 0x384af6e0 tests two items' masks against each other, so a Delia-only item never
# matched. Its tail (Hagie's bit and `ret`) jumps to a stub that also adds bit 10 (DELIA_FLAG).
# (VA of the tail `cmp byte [ecx+0x1a1], 0`, its bytes)
ITEM_MASK = {'client.dll': (0x384AC4C1, bytes.fromhex('80b9a101000000' '7405' '0500020000' 'c3'))}
# The client's item record copy (0x384e5d38, beside the `iteminfo addr` log) copies UseInQuest
# (+0x1cc) as a byte; copying the dword also carries DELIA_FLAG (+0x1cd; +0x1ce/+0x1cf are padding).
# (VA, movzx ecx, byte [edi+0x1cc]; mov [esi+0x1cc], cl -> mov ecx, [edi+0x1cc]; nop; mov [esi+0x1cc], ecx)
ITEM_COPY = {'client.dll': (0x384E5DBB, bytes.fromhex('0fb68fcc010000' '888ecc010000'),
                            bytes.fromhex('8b8fcc010000' '90' '898ecc010000'))}
# Animation event frame keys (HEROES_CUSTOM_ANIM_EVENT): the 1.69 parser knows $ref_group$,
# $endframe_finally$, $0_finally$ and $-1_finally$; any other $...$ key goes to the frame
# expression and an unknown one drops the event. NA adds `$immediately$` (frame 0 with a flag),
# which Delia's `spskill_activate_states` uses to open AttackSP01/02_begin/04/05.
# (site: `mov ecx, [esp+0xa0]` before the expression call; strcmp)
FRAME_KEYS = {'client.dll': (0x387FFFD3, 0x38A5DFC6), 'server.dll': (0x40800963, 0x40994755)}
FRAME_KEY_SITE = bytes.fromhex('8b8c24a0000000')
FRAME_KEY_DONE = 0x20  # the parser's join after a known key (frame in esi)
PERFECT_ENDURE = (0x4C, 0x4D)  # As_Cond_Can_PerfectEndurable, As_Cond_Do_PerfectEndurable (1.69 ids)
COMMANDS = 0x437C  # the client player's command/condition object
# Diagnostic only (D8k, operator's choice): the server builds `player_<cl_playerclass>` and, when
# no such factory exists, pushes "player" (CHeroesPlayer, the base of every class) at 0x40371314.
# --fallback-entity swaps that string for an existing class factory, so a class without its own
# entity (only Delia) gets that class's entity. Off by default; not the Delia entity design.
FALLBACK_PUSH = 0x40371314
FALLBACK_ENTITIES = ('player_fiona',)
# Character creation by the class grid (S3_CharacterCreation, NewCharacterDialogS3), following the
# NA's CharacterCreation_UI_V3 flow (MIGRATION.md, C3). The 1.69 grid movie already lists the 11
# classes and the 1.69 lobby_beautyshop_change.bsp already has the NA scene: one
# npc_character_creation_actor `npc_<class>` per class (csf/createcharacter_<class>.csf) and the
# point_teleport entities npc_character_create_select/_nonselect. What the 1.69 lacks is code:
# * client: the S3 grid opens in that map (`changemap lobby_beautyshop_change`, NA 0x380921b0)
#   instead of s3_game_create_character, whose actors stop at Arisha; once the map is up the movie's
#   OnStartCreation runs (NA 0x38bdf4a0) and the dialog answers `npc_character_create` plus
#   OnReadyCreation (NA 0x389e06f0); `Select<Class>` (Delia included) sends
#   `npc_character_create_select <class>` instead of the old map's `force_start_touch <class>`;
#   GameStart goes straight to CharacterCreationState (11, NA 0x38bdf610) instead of the prologue
#   (8), which enters with `changemap lobby_beautyshop_body` (NA 0x38bd8600) and, on that map,
#   turns the creation mode and the customization dialog on (NA 0x38bd8e80);
# * server: the two console commands (NA 0x406db0d0, 0x406db3f0). The NA hides the actors with
#   EF_NODRAW and plays ACT_CHARACTER_CREATION_SELECTED_S3 through a client command the 1.69 lacks;
#   here the selected actor is teleported to the select point and the others to the non-select
#   point (both from the map), and the actor keeps its own animation (approximation).
CREATION_GRID = {
    'client.dll': {
        'features': 0x38CD2F90,          # feature matrix object (vt+0x18: is the feature on)
        'engine': 0x38CD2F0C,            # engine client (vt+0x188 ClientCmd, vt+0x1c unrestricted)
        'ui': 0x385F7EF0,                # Scaleform UI manager getter (vt+0x10 invoke)
        'strcmp': 0x38A5DFC6,
        's3_name': 0x38B7AFC0,           # "S3_CharacterCreation"
        'map_operand': (0x388E8F49, 'b85800c138'),        # mov eax, "map_background s3_game_create_character"
        'start_camera': (0x385239D4, '68acb0b738'),       # push "cam_select_createcharacter_camera"
        'start_invoke': (0x38523A02, '6864b0b738'),       # push "NewCharacterDialogS3.StartCreation"
        'handler': (0x3890AB90, '81ec00020000' '5657' '8bbc2414020000'),  # S3 FSCommand handler head
        'handler_rest': 0x3890AE35,      # its GameStart test (esi = dialog, edi = argument)
        'transition': (0x3852376D, 'b808000000'),         # NewCharacterSelection_Start -> state 8
        'enter': (0x38524B00, '81ec00020000'),            # CharacterCreationState enter
        'leave_command': (0x38524BAB, '68b8b2b738'),      # its leave: push "sv_create_character_mode 0"
        'creation_vtable': 0x38B79BC8,   # CharacterCreationState; slot 4 runs once the map is up
        # UI start-up registers the grid dialog's FSCommand handler only when S3_CharacterCreation
        # is already on, and then skips the V4 bar's. The FeatureMatrix arrives later, at login, so
        # the grid opened without a handler (D6, and the first C3 run: buttons did nothing).
        # Both handlers are registered now; the runtime feature test picks the dialog.
        'register_test': (0x3858B7D0, '745e'),            # je V2 branch (S3 off)
        'register_failed': (0x3858B81E, '0f8ce9000000'),  # jl next feature (S3 insert failed)
        'register_done': (0x3858B82B, 'e9c9000000'),      # jmp to the shared tail
        'register_v2': 0x3858B830,       # the CharacterCreation_UI_V2 test (V4 / V1 handler)
        'register_tail': bytes.fromhex('3bc3' '7410' '8b0d') + b'%s' + bytes.fromhex('8b09' '8b11' '50' '8b4214' 'ffd0'),
        'register_list': 0x38ADA3C4,
        # The prologue (state 8) gives the local player the chosen class: state machine 0x3851e5f0,
        # its class 0x38519540, cl_playerclass/cl_playermodel 0x38594860 (called at 0x38526d0c).
        # The grid skips the prologue (NA V3), so the creation enter does it (else the customization
        # shows the default Lethita).
        'machine': 0x3851E5F0, 'machine_class': 0x38519540, 'player_class': 0x38594860,
        'fire': 0x385194D0,              # machine->Fire(event, arguments)
        # Customization Back (NA 0x389ac61c: sv_prologue_mode 0, CharacterBodyCreation_Back): the
        # 1.69 CharacterCreationState (11) only has Create/OK/Failed in the transition table, so the
        # table (state, event, handler) is copied with one more entry back to the class grid (7).
        'transitions': (0x3851D6D0, '688d000000', 0x3851D6D5, '68c85cca38'),  # push count; push table
        'grid_state': 7,
        # The customization dialog's FSCommand handler; its movie shows Button_Back only with
        # CharacterCreation_UI_V3, which also hides the name field the 1.69 C++ still reads, so the
        # handler shows the button itself on OnShow (UI manager vt+0x18, set boolean).
        'dialog': (0x384AF6E0, '81ecdc030000' '5657' '8bbc24f0030000'),
        'dialog_rest': 0x384AF6EF,
        # The customization panel fill (0x384a5870: loadDefaultValues, outfits, arrageAll) runs after
        # the movie's own onShow; the UI manager call right after arrageAll becomes a call into a stub
        # that shows the button, then goes to the getter (C3-11).
        'filled': (0x384A5B1E, 'e8cd231500'),
    },
    'server.dll': {
        'init': (0x409FA70D, '68405fa240'),  # force_start_touch's static initializer: push atexit dtor
        'init_rest': 0x409FA712,         # call atexit; pop ecx; ret
        'ctor': 0x408BC430,              # ConCommand(name, callback, help, flags, completion)
        'flags': 0x40,                   # as force_start_touch, which the same client path reaches
        'engine': 0x40D6EE50,            # vt+0xd0 Cmd_Argc, vt+0xd4 Cmd_Argv
        'entities': 0x40D2A0F8,          # gEntList
        'find': 0x401523E0,              # FindEntityByName(start, name, searching, activator, caller, filter)
        'teleport': 0x208,               # CBaseEntity::Teleport(origin, angles, velocity) slot
        # The actor's S3 activity (0x4040e380 under S3_CharacterCreation) is registered by name at
        # these two pushes. ACT_CHARACTER_CREATION_S3 poses were made for the old map's props (Kay
        # leans, Lynn stands on nothing) and Hagie's and Delia's town motions lack it (T-pose); all
        # eleven have the NA's ACT_CHARACTER_CREATION_SELECTED_S3, so the slot takes that name.
        'activity': ((0x4040F034, '68f00fb340'), (0x4040F03F, '68f00fb340')),
        # point_teleport origin/angles in lobby_beautyshop_change.bsp (NA reads the same entities).
        'select': ((-64.0, 0.0, 0.0), (0.0, 180.0, 0.0)),
        'nonselect': ((-16.0, 276.0, 152.0), (0.0, 0.0, 0.0)),
    },
}
CREATION_STATE = 11  # CharacterCreationState in the 1.69 state factory (0x3851B990: id - 1 cases)


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
    """One FLAG_CHECKS stub: cmp reg, DELIA; ja default; al = DELIA_FLAG or flags[reg]; ret pop."""
    reg = {'eax': 0, 'ecx': 1}[register]
    item = 1 if base == 'ecx' else 0
    layout.pad()
    stub = layout.here()
    layout.body += bytes((0x83, 0xF8 | reg, DELIA))
    layout.body += b'\x0f\x87' + struct.pack('<i', default - (layout.here() + 6))
    if base != 'ecx':
        layout.body += b'\x8b\x44\x24\x04'  # mov eax, [esp + 4]
    delia = layout.forward(b'\x0f\x84')
    layout.body += bytes((0x8A, 0x84, (reg << 3) | item)) + struct.pack('<I', 0x198)  # mov al, [item + reg + 0x198]
    layout.body += b'\xc2' + struct.pack('<H', pop)
    layout.land(delia)
    layout.body += bytes((0x8A, 0x80 | item)) + struct.pack('<I', DELIA_FLAG)  # mov al, [item + DELIA_FLAG]
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
                struct.pack_into('<I', raw, at, DELIA_FLAG)
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


def shape_class(layout: Layout, edits: list, va: int, old: str, register: str) -> str:
    """SHAPE_CLASS stub: an extra class becomes its `shape_of` class, then the original bound."""
    reg = {'eax': 0, 'ecx': 1, 'edx': 2, 'ebx': 3, 'esi': 6, 'edi': 7}[register]
    replaced = bytes.fromhex(old)
    ins = list(md.disasm(replaced, va))
    if len(ins) != 2 or ins[1].mnemonic != 'ja':
        raise PreparationError(f'Unexpected body-shape bound at {va:#x}; nothing is patched.')
    skip = int(ins[1].op_str, 16)
    layout.pad()
    stub = layout.here()
    for extra in EXTRA_CLASSES:
        layout.body += bytes((0x83, 0xF8 | reg, extra.index))
        other = layout.forward(b'\x0f\x85')
        layout.body += bytes((0xB8 | reg,)) + struct.pack('<I', OLD_CLASSES.index(extra.shape_of))
        layout.land(other)
    layout.body += replaced[:3]
    layout.to(b'\x0f\x87', skip)
    layout.to(b'\xe9', va + len(replaced))
    edits.append((va, replaced, b'\xe9' + struct.pack('<i', stub - (va + 5)) + b'\x90' * (len(replaced) - 5)))
    return f'{va:#x} -> {stub:#x}'


def class_default(image: Image, layout: Layout, edits: list, va: int) -> str:
    """CLASS_DEFAULTS stub: table[slot*10 + class] with an extra class read as its clone_of."""
    o = image.offset(va)
    if bytes(image.data[o:o + len(CLASS_DEFAULT_HEAD)]) != CLASS_DEFAULT_HEAD:
        raise PreparationError(f'Unexpected default table accessor at {va:#x}; nothing is patched.')
    layout.pad()
    stub = layout.here()
    layout.body += CLASS_DEFAULT_HEAD[:8]  # mov eax, [esp + 4] (slot); mov edx, [esp + 8] (class)
    for extra in EXTRA_CLASSES:
        layout.body += bytes((0x83, 0xFA, extra.index))
        other = layout.forward(bytes((0x0F, 0x85)))
        layout.body += bytes((0xBA,)) + struct.pack('<I', OLD_CLASSES.index(extra.clone_of))
        layout.land(other)
    layout.body += CLASS_DEFAULT_HEAD[8:]
    edits.append((va, CLASS_DEFAULT_HEAD[:8], bytes((0xE9,)) + struct.pack('<i', stub - (va + 5)) + bytes((0x90,)) * 3))
    return f'{va:#x} -> {stub:#x}'


def class_weapons(image: Image, layout: Layout, edits: list, relocs: set[int], table: int) -> dict:
    """CLASS_WEAPONS: a weapon list row per extra class and a stub at each clamped reader."""
    layout.pad()
    # Rows hold exactly WEAPON_SLOTS names, the unused ones empty, as the loader fills them: the
    # server's 0x40825890 walks all five without reading the size (one entry crashed it, D18).
    if any(len(extra.weapons) > WEAPON_SLOTS for extra in EXTRA_CLASSES):
        raise PreparationError(f'At most {WEAPON_SLOTS} weapon entities per class.')
    names = {extra.index: [layout.string(w) for w in extra.weapons] + [layout.string('')] * (WEAPON_SLOTS - len(extra.weapons))
             for extra in EXTRA_CLASSES}
    layout.pad()
    rows = {}
    for extra in EXTRA_CLASSES:
        items = layout.here()
        for name in names[extra.index]:
            layout.pointer(name)
        layout.pad()
        rows[extra.index] = layout.here()
        # CUtlVector: memory, allocation count, grow size (-1: external buffer, never freed), size, elements
        layout.pointer(items)
        layout.body += struct.pack('<iii', len(names[extra.index]), -1, len(names[extra.index]))
        layout.pointer(items)
        layout.pad()
    readers = []
    for at in sorted(relocs):
        if not image.text_contains(at) or struct.unpack_from('<I', image.data, image.offset(at))[0] != table:
            continue
        for start in range(at - 24, at):
            code = bytes(image.data[image.offset(start):image.offset(start) + 10])
            ins = list(md.disasm(code, start))
            if len(ins) == 4 and ins[0].mnemonic == 'cmp' and ins[0].op_str.endswith(', 0xa') and ins[0].size == 3                     and ins[1].mnemonic == 'jl' and ins[1].size == 2 and ins[2].mnemonic == 'xor' and ins[3].mnemonic == 'lea'                     and ins[3].size == 3 and not any(start <= r < start + 10 for r in relocs):
                readers.append((start, ins[3].op_str.split(',')[0], code))
                break
    if len(readers) != 5:
        raise PreparationError(f'Expected 5 class weapon readers of {table:#x}, found {len(readers)}; nothing is patched.')
    regs = {'eax': 0, 'ecx': 1, 'edx': 2, 'ebx': 3, 'esi': 6, 'edi': 7}
    stubs = []
    for start, target, code in readers:
        index = code[1] & 7  # register compared in `cmp R, 0xa`
        layout.pad()
        stub = layout.here()
        found = []
        for extra in EXTRA_CLASSES:
            layout.body += bytes((0x83, 0xF8 | index, extra.index))
            other = layout.forward(bytes((0x0F, 0x85)))
            layout.body += bytes((0xB8 | regs[target],)) + struct.pack('<I', ((rows[extra.index] - table) // 4) & 0xFFFFFFFF)
            found.append(layout.forward(bytes((0xE9,))))
            layout.land(other)
        layout.body += code
        for at_ in found:
            layout.land(at_)
        layout.to(bytes((0xE9,)), start + 10)
        edits.append((start, code, bytes((0xE9,)) + struct.pack('<i', stub - (start + 5)) + bytes((0x90,)) * 5))
        stubs.append(f'{start:#x} -> {stub:#x}')
    return {'rows': {i: hex(a) for i, a in rows.items()}, 'readers': stubs}


def class_alias(image: Image, layout: Layout, edits: list, relocs: set[int], va: int, old: str) -> str:
    """CLASS_ALIAS stub: load m_iClass, turn an extra class into its clone_of, continue."""
    replaced = bytes.fromhex(old)
    ins = list(md.disasm(replaced, va))
    if len(ins) != 1 or ins[0].mnemonic != 'mov' or any(va <= r < va + len(replaced) for r in relocs):
        raise PreparationError(f'Unexpected class load at {va:#x}; nothing is patched.')
    reg = {'eax': 0, 'ecx': 1, 'edx': 2, 'ebx': 3, 'esi': 6, 'edi': 7}[ins[0].op_str.split(',')[0]]
    layout.pad()
    stub = layout.here()
    layout.body += replaced
    for extra in EXTRA_CLASSES:
        layout.body += bytes((0x83, 0xF8 | reg, extra.index))
        other = layout.forward(bytes((0x0F, 0x85)))
        layout.body += bytes((0xB8 | reg,)) + struct.pack('<I', OLD_CLASSES.index(extra.clone_of))
        layout.land(other)
    layout.to(bytes((0xE9,)), va + len(replaced))
    edits.append((va, replaced, bytes((0xE9,)) + struct.pack('<i', stub - (va + 5)) + bytes((0x90,)) * (len(replaced) - 5)))
    return f'{va:#x} -> {stub:#x}'


def client_weapon_switch(image: Image, layout: Layout, edits: list, va: int, old: str, join: int) -> str:
    """CLIENT_WEAPON_SWITCH stub: `push ebx; push <first weapon>; jmp join` for an extra class."""
    replaced = bytes.fromhex(old)
    ins = list(md.disasm(replaced, va))
    if len(ins) != 2 or f'{ins[0].mnemonic} {ins[0].op_str}' != 'cmp eax, 9' or ins[1].mnemonic != 'ja':
        raise PreparationError(f'Unexpected class switch at {va:#x}; nothing is patched.')
    default = int(ins[1].op_str, 16)
    layout.pad()
    names = {extra.index: layout.string(extra.weapons[0]) for extra in EXTRA_CLASSES if extra.weapons}
    layout.pad()
    stub = layout.here()
    for index, name in names.items():
        layout.body += bytes((0x83, 0xF8, index))
        other = layout.forward(bytes((0x0F, 0x85)))
        layout.body += bytes((0x53, 0x68))  # push ebx; push name
        layout.pointer(name)
        layout.to(bytes((0xE9,)), join)
        layout.land(other)
    layout.body += replaced[:3]
    layout.to(bytes((0x0F, 0x87)), default)
    layout.to(bytes((0xE9,)), va + 5)
    edits.append((va, replaced, bytes((0xE9,)) + struct.pack('<i', stub - (va + 5))))
    return f'{va:#x} -> {stub:#x}'


def conditions(layout: Layout, edits: list, tail: int, name_map: int, name_insert: int, id_map: int,
               id_insert: int, bounds: tuple) -> dict:
    """CONDITIONS: ids 121-125 for Delia's condition names, registered after the 1.69 list."""
    limit = FIRST_NEW_CONDITION + len(CONDITION_NAMES)
    for va in bounds:
        edits.append((va, bytes.fromhex('83f979'), bytes((0x83, 0xF9, limit))))
    layout.pad()
    names = [layout.string(n) for n in CONDITION_NAMES]
    layout.pad()
    stub = layout.here()
    layout.body += bytes((0x60, 0x83, 0xEC, 0x08))  # pushad; sub esp, 8
    for i, name in enumerate(names):
        for first, second, table, insert in ((name, None, name_map, name_insert), (None, name, id_map, id_insert)):
            # the pair {name, id} for the name map, {id, name} for the id map, at [esp]
            layout.body += bytes((0xC7, 0x04, 0x24))  # mov dword ptr [esp], imm32
            if first is not None:
                layout.pointer(first)
            else:
                layout.body += struct.pack('<I', FIRST_NEW_CONDITION + i)
            layout.body += bytes((0xC7, 0x44, 0x24, 0x04))  # mov dword ptr [esp + 4], imm32
            if second is not None:
                layout.pointer(second)
            else:
                layout.body += struct.pack('<I', FIRST_NEW_CONDITION + i)
            layout.body += bytes((0x8D, 0x04, 0x24, 0x50, 0xB9))  # lea eax, [esp]; push eax; mov ecx, map
            layout.pointer(table)
            layout.to(bytes((0xE8,)), insert)
    layout.body += bytes((0x83, 0xC4, 0x08, 0x61))  # add esp, 8; popad
    layout.body += CONDITION_TAIL
    layout.to(bytes((0xE9,)), tail + len(CONDITION_TAIL))
    edits.append((tail, CONDITION_TAIL, bytes((0xE9,)) + struct.pack('<i', stub - (tail + 5)) + bytes((0x90,))))
    return {'ids': {n: FIRST_NEW_CONDITION + i for i, n in enumerate(CONDITION_NAMES)},
            'registration': f'{tail:#x} -> {stub:#x}', 'bounds': [f'{va:#x}: < {limit}' for va in bounds]}


def guard_state(layout: Layout, forced_off: int | None, ended) -> tuple[int, int, int]:
    """A 16-entry (entity, state) table in .hrte with get(this) -> al and set(this, state) (ret 4).
    `forced_off` (or None) is a routine whose true result forces the state to 0; `ended` emits the
    code run (ecx = this, esi/edi free) when the state is set to 0."""
    layout.pad()
    states = layout.here()
    layout.body += bytes(8 * GUARD_ENTRIES)

    # get(this): al = state != 0 for this entity
    layout.pad()
    get = layout.here()
    layout.body += bytes((0xB8,))
    layout.pointer(states)
    layout.body += bytes((0xBA,)) + struct.pack('<I', GUARD_ENTRIES)
    loop = layout.here()
    layout.body += bytes((0x39, 0x08))  # cmp [eax], ecx
    found = layout.forward(bytes((0x0F, 0x84)))
    layout.body += bytes((0x83, 0xC0, 0x08, 0x4A))  # add eax, 8; dec edx
    layout.to(bytes((0x0F, 0x85)), loop)
    layout.body += bytes((0x32, 0xC0, 0xC3))  # xor al, al; ret
    layout.land(found)
    layout.body += bytes((0x83, 0x78, 0x04, 0x00, 0x0F, 0x95, 0xC0, 0xC3))  # cmp [eax+4], 0; setne al; ret

    # set(this, state): the state is the argument's low byte
    layout.pad()
    put = layout.here()
    if forced_off is not None:
        layout.body += bytes((0x51,))  # push ecx
        layout.to(bytes((0xE8,)), forced_off)
        layout.body += bytes((0x59, 0x0F, 0xB6, 0x54, 0x24, 0x04, 0x84, 0xC0))  # pop ecx; movzx edx, byte [esp+4]; test al, al
        keep = layout.forward(bytes((0x0F, 0x84)))
        layout.body += bytes((0x33, 0xD2))  # xor edx, edx
        layout.land(keep)
    else:
        layout.body += bytes((0x0F, 0xB6, 0x54, 0x24, 0x04))  # movzx edx, byte [esp+4]
    layout.body += bytes((0x56, 0x57, 0xBE))  # push esi; push edi; mov esi, states
    layout.pointer(states)
    layout.body += bytes((0x33, 0xFF, 0xB8)) + struct.pack('<I', GUARD_ENTRIES)  # xor edi, edi; mov eax, n
    loop = layout.here()
    layout.body += bytes((0x39, 0x0E))  # cmp [esi], ecx
    hit = layout.forward(bytes((0x0F, 0x84)))
    layout.body += bytes((0x83, 0x3E, 0x00))  # cmp dword [esi], 0
    used = layout.forward(bytes((0x0F, 0x85)))
    layout.body += bytes((0x85, 0xFF))  # test edi, edi
    taken = layout.forward(bytes((0x0F, 0x85)))
    layout.body += bytes((0x89, 0xF7))  # mov edi, esi: first free entry
    layout.land(used)
    layout.land(taken)
    layout.body += bytes((0x83, 0xC6, 0x08, 0x48))  # add esi, 8; dec eax
    layout.to(bytes((0x0F, 0x85)), loop)
    layout.body += bytes((0x85, 0xFF))  # test edi, edi: no free entry -> reuse the first
    free = layout.forward(bytes((0x0F, 0x85)))
    layout.body += bytes((0xBF,))
    layout.pointer(states)
    layout.land(free)
    layout.body += bytes((0x89, 0xFE, 0x89, 0x0E))  # mov esi, edi; mov [esi], ecx
    layout.land(hit)
    layout.body += bytes((0x89, 0x56, 0x04, 0x85, 0xD2))  # mov [esi+4], edx; test edx, edx
    done = layout.forward(bytes((0x0F, 0x85)))
    ended()
    layout.land(done)
    layout.body += bytes((0x5F, 0x5E, 0xC2, 0x04, 0x00))  # pop edi; pop esi; ret 4
    return states, get, put


def class_dispatch(image: Image, layout: Layout, edits: list, relocs: set[int], name: str, klass: int,
                   slot: int, body: int) -> str:
    """Point vtable `slot` of `name` at a stub that jumps to `body` for an extra class (m_iClass at
    +klass) and to the original method otherwise."""
    at = vtable(image, name) + 4 * slot
    if at not in relocs:
        raise PreparationError(f'{name} slot {slot} is not relocated; nothing is patched.')
    original = struct.unpack_from('<I', image.data, image.offset(at))[0]
    layout.pad()
    stub = layout.here()
    layout.body += bytes((0x8B, 0x81)) + struct.pack('<I', klass)
    for extra in EXTRA_CLASSES:
        layout.body += bytes((0x83, 0xF8, extra.index))
        other = layout.forward(bytes((0x0F, 0x85)))
        layout.to(bytes((0xE9,)), body)
        layout.land(other)
    layout.to(bytes((0xE9,)), original)
    edits.append((at, struct.pack('<I', original), struct.pack('<I', stub)))
    return f'slot {slot} {original:#x} -> {stub:#x}'


def guard(image: Image, layout: Layout, edits: list, relocs: set[int], set_slot: int, get_slot: int,
          damage_slot: int, guarded: int, forced_off: int, conditions_of: int, raise_: int, lower: int,
          forfeit: int | None = None) -> dict:
    """GUARD: guard state table, state set/get and the guarded damage path for extra classes; a hit that
    is neither guarded nor unblockable calls `forfeit(this)` (Delia's reserved HP) when given."""
    name, klass = ENTITY_SLOTS['server.dll'][0], ENTITY_SLOTS['server.dll'][1]
    guard_id = FIRST_NEW_CONDITION + CONDITION_NAMES.index('AS_Cond_Delia_Guard')
    hit_id = FIRST_NEW_CONDITION + CONDITION_NAMES.index('AS_Cond_Delia_Guard_Hit')

    def call(target: int) -> None:
        layout.to(bytes((0xE8,)), target)

    def ended() -> None:
        # guard ended: lower both conditions (ecx = this)
        call(conditions_of)
        layout.body += bytes((0x85, 0xC0))
        none = layout.forward(bytes((0x0F, 0x84)))
        layout.body += bytes((0x89, 0xC7, 0x68)) + struct.pack('<I', guard_id) + bytes((0x89, 0xF9))
        call(lower)
        layout.body += bytes((0x68,)) + struct.pack('<I', hit_id) + bytes((0x89, 0xF9))
        call(lower)
        layout.land(none)

    states, get, put = guard_state(layout, forced_off, ended)

    # damage(this, info)
    layout.pad()
    damage = layout.here()
    layout.body += bytes((0x56, 0x8B, 0xF1, 0x8B, 0x44, 0x24, 0x08))  # push esi; mov esi, ecx; mov eax, [esp+8]
    # a hit reported by the client (0x10000000) was guarded there when bit 2 of +0x78 is set
    layout.body += bytes((0xF7, 0x40, 0x3C)) + struct.pack('<I', 0x10000000)  # test [eax+0x3c], client hit
    server_side = layout.forward(bytes((0x0F, 0x84)))
    layout.body += bytes((0xF6, 0x40, 0x78, 0x02))  # test byte [eax+0x78], 2
    plain0 = layout.forward(bytes((0x0F, 0x84)))
    blocked = layout.forward(bytes((0xE9,)))
    layout.land(server_side)
    layout.body += bytes((0xF7, 0x40, 0x3C)) + struct.pack('<I', 0x8000000)  # test [eax+0x3c], unblockable
    plain = layout.forward(bytes((0x0F, 0x85)))
    layout.body += bytes((0x8B, 0xCE))  # mov ecx, esi
    call(get)
    layout.body += bytes((0x84, 0xC0))
    plain2 = layout.forward(bytes((0x0F, 0x84)))
    layout.land(blocked)
    layout.body += bytes((0x8B, 0xCE))
    call(conditions_of)
    layout.body += bytes((0x85, 0xC0))
    plain3 = layout.forward(bytes((0x0F, 0x84)))
    layout.body += bytes((0x50, 0x68)) + struct.pack('<I', guard_id) + bytes((0x8B, 0x4C, 0x24, 0x04))
    call(raise_)  # push eax; push guard; mov ecx, [esp+4]; raise
    layout.body += bytes((0x58, 0x68)) + struct.pack('<I', hit_id) + bytes((0x8B, 0xC8))
    call(raise_)  # pop eax; push hit; mov ecx, eax; raise
    final = layout.forward(bytes((0xE9,)))
    layout.land(plain0)
    layout.land(plain2)
    if forfeit is not None:
        layout.body += bytes((0x8B, 0x44, 0x24, 0x08))  # mov eax, [esp+8]
        layout.body += bytes((0xF7, 0x40, 0x3C)) + struct.pack('<I', 0x8000000)
        kept = layout.forward(bytes((0x0F, 0x85)))
        layout.body += bytes((0x8B, 0xCE))
        call(forfeit)
        layout.land(kept)
    for at in (final, plain, plain3):
        layout.land(at)
    layout.body += bytes((0xFF, 0x74, 0x24, 0x08, 0x8B, 0xCE))  # push [esp+8]; mov ecx, esi
    call(guarded)
    layout.body += bytes((0x5E, 0xC2, 0x04, 0x00))  # pop esi; ret 4

    def dispatch(slot: int, body: int) -> str:
        return class_dispatch(image, layout, edits, relocs, name, klass, slot, body)

    return {'states': hex(states), 'get': hex(get), 'set': hex(put), 'damage': hex(damage),
            'slots': [dispatch(set_slot, put), dispatch(get_slot, get), dispatch(damage_slot, damage)]}


def client_guard(image: Image, layout: Layout, edits: list, relocs: set[int], set_slot: int, get_slot: int,
                 damage_slot: int, copy: int, unguardable: int, unguardable_info: int, raise_: int, test: int,
                 lower: int, allocator: int) -> dict:
    """CLIENT_GUARD: the client guard state and the client's guarded-hit decision for extra classes."""
    name, klass = ENTITY_SLOTS['client.dll'][0], ENTITY_SLOTS['client.dll'][1]
    base = struct.unpack_from('<I', image.data, image.offset(vtable(image, name) + 4 * damage_slot))[0]
    guard_id = FIRST_NEW_CONDITION + CONDITION_NAMES.index('AS_Cond_Delia_Guard')
    hit_id = FIRST_NEW_CONDITION + CONDITION_NAMES.index('AS_Cond_Delia_Guard_Hit')

    def call(target: int) -> None:
        layout.to(bytes((0xE8,)), target)

    def ended() -> None:
        # guard ended: lower both conditions on [this+COMMANDS]
        layout.body += bytes((0x8B, 0x81)) + struct.pack('<I', COMMANDS) + bytes((0x85, 0xC0))
        none = layout.forward(bytes((0x0F, 0x84)))
        for cond in (guard_id, hit_id):
            layout.body += bytes((0x89, 0xC7, 0x68)) + struct.pack('<I', cond) + bytes((0x89, 0xF9))
            call(lower)  # mov edi, eax; push id; mov ecx, edi; lower
            layout.body += bytes((0x89, 0xF8))  # mov eax, edi
        layout.land(none)

    states, get, put = guard_state(layout, None, ended)

    # damage(this, info), the frame of Fiona's 0x3870cef0: copy at [esp+0xc] after push esi/edi
    layout.pad()
    damage = layout.here()
    layout.body += bytes((0x81, 0xEC, 0x90, 0, 0, 0, 0x56, 0x57))  # sub esp, 0x90; push esi; push edi
    layout.body += bytes((0x8B, 0xBC, 0x24, 0x9C, 0, 0, 0, 0x8B, 0xF1))  # mov edi, [esp+0x9c]; mov esi, ecx
    layout.body += bytes((0x57, 0x8D, 0x4C, 0x24, 0x10))  # push edi; lea ecx, [esp+0x10]
    call(copy)
    layout.body += bytes((0x8B, 0xCE))
    call(get)
    layout.body += bytes((0x84, 0xC0))
    skips = [layout.forward(bytes((0x0F, 0x84)))]
    layout.body += bytes((0x8B, 0x86)) + struct.pack('<I', COMMANDS) + bytes((0x50, 0x57))  # push [esi+cmd]; push edi
    call(unguardable)
    layout.body += bytes((0x83, 0xC4, 0x08, 0x84, 0xC0))
    skips.append(layout.forward(bytes((0x0F, 0x85))))
    layout.body += bytes((0x57,))
    call(unguardable_info)
    layout.body += bytes((0x83, 0xC4, 0x04, 0x84, 0xC0))
    skips.append(layout.forward(bytes((0x0F, 0x85))))
    layout.body += bytes((0x83, 0x8C, 0x24, 0x84, 0, 0, 0, 0x02))  # or dword [esp+0x84], 2: guarded
    layout.body += bytes((0xC6, 0x86)) + struct.pack('<I', HIT_IMPACT_SKIP) + b'\x01'  # no impact sound
    layout.body += bytes((0xC6, 0x84, 0x24, 0x85, 0, 0, 0, 0x00))  # mov byte [esp+0x85], 0
    layout.body += bytes((0xD9, 0xEE, 0xD9, 0x54, 0x24, 0x3C, 0xD9, 0x5C, 0x24, 0x54))  # damage = 0
    layout.body += bytes((0x8B, 0x8E)) + struct.pack('<I', COMMANDS) + bytes((0x85, 0xC9))
    skips.append(layout.forward(bytes((0x0F, 0x84))))
    layout.body += bytes((0x51, 0x68)) + struct.pack('<I', guard_id)  # push ecx; push guard
    call(raise_)
    layout.body += bytes((0x8B, 0x0C, 0x24, 0x68)) + struct.pack('<I', hit_id)  # mov ecx, [esp]; push hit
    call(raise_)
    layout.body += bytes((0x8B, 0x0C, 0x24, 0x68)) + struct.pack('<I', PERFECT_ENDURE[0])
    call(test)
    layout.body += bytes((0x59, 0x84, 0xC0))  # pop ecx; test al, al
    skips.append(layout.forward(bytes((0x0F, 0x84))))
    layout.body += bytes((0x68,)) + struct.pack('<I', PERFECT_ENDURE[1])
    call(raise_)
    for at in skips:
        layout.land(at)
    layout.body += bytes((0x8D, 0x54, 0x24, 0x0C, 0x52, 0x8B, 0xCE))  # lea edx, [esp+0xc]; push edx; mov ecx, esi
    call(base)
    # free the copy's vector (+0x60) as Fiona's routine does
    layout.body += bytes((0x83, 0x7C, 0x24, 0x74, 0x00, 0x8B, 0xF0))  # cmp dword [esp+0x74], 0; mov esi, eax
    done = layout.forward(bytes((0x0F, 0x8C)))
    layout.body += bytes((0x8B, 0x44, 0x24, 0x6C, 0x85, 0xC0))
    done2 = layout.forward(bytes((0x0F, 0x84)))
    layout.body += bytes((0x8B, 0x0D))
    layout.pointer(allocator)
    layout.body += bytes((0x8B, 0x09, 0x8B, 0x11, 0x50, 0x8B, 0x42, 0x14, 0xFF, 0xD0))
    layout.land(done)
    layout.land(done2)
    layout.body += bytes((0x5F, 0x8B, 0xC6, 0x5E, 0x81, 0xC4, 0x90, 0, 0, 0, 0xC2, 0x04, 0x00))

    def dispatch(slot: int, body: int) -> str:
        return class_dispatch(image, layout, edits, relocs, name, klass, slot, body)

    return {'states': hex(states), 'get': hex(get), 'set': hex(put), 'damage': hex(damage), 'base': hex(base),
            'slots': [dispatch(set_slot, put), dispatch(get_slot, get), dispatch(damage_slot, damage)]}


def delia_effects(image: Image, layout: Layout, edits: list, relocs: set[int], k: dict) -> dict:
    """DELIA_EFFECTS: Delia's status-effect types, her on-hit commands, the per-entity state and its think."""
    entity, klass = ENTITY_SLOTS['server.dll'][0], ENTITY_SLOTS['server.dll'][1]
    f, s = PLAYER_FIELDS, PLAYER_SLOTS
    L = layout

    def b(code: str) -> None:
        L.body += bytes.fromhex(code)

    def d32(v: int) -> str:
        return struct.pack('<I', v & 0xFFFFFFFF).hex()

    def call(va: int) -> None:
        L.to(b'\xe8', va)

    def ptr(va: int) -> None:
        L.pointer(va)

    def push_ptr(va: int) -> None:
        b('68')
        ptr(va)

    def fwd(op: str) -> int:
        return L.forward(bytes.fromhex(op))

    def arg_float(name: int) -> None:
        # ST0 = argument `name` of the effect in esi (default 0)
        b('d9ee 51 d91c24')  # fldz; push ecx; fstp [esp]
        push_ptr(name)
        b('8bce')
        call(k['arg'])

    names = {n: L.string(n) for n in (
        'stamina_adder_on_attack_time', 'delia_restore_health', 'skill_delia_strong_attack', 'damage_modify_to_action',
        'restore_equipment_def', 'validCount', 'reduce_ratio', 'restore_time', 'delay_time', 'immune_count',
        'candidate_actions', 'damage_rates', 'add_def', 'skill_delia_strong_attack_week', 'skill_delia_strong_attack_hit',
        'ENABLE_STACK_CHARGE_SMASH_STATUS_EFFECT', 'DISABLE_STACK_CHARGE_SMASH_STATUS_EFFECT',
        'ENABLE_ADD_STRONG_ATTACK_STATUS_EFFECT', 'DISABLE_ADD_STRONG_ATTACK_STATUS_EFFECT', 'PURGE_ALL_ARMOR', '')}
    L.pad()
    registrar = L.here()
    L.body += bytes(4)
    table = L.here()
    L.body += bytes(DELIA_TABLE_ENTRIES * DELIA_ENTRY)

    # find(ecx = entity) -> eax = entry or 0; find_or_add -> a cleared entry when missing
    L.pad()
    find = L.here()
    b('b8'); ptr(table)
    b('ba' + d32(DELIA_TABLE_ENTRIES))
    loop = L.here()
    b('3908'); hit = fwd('0f84')  # cmp [eax], ecx
    b('83c0' + bytes((DELIA_ENTRY,)).hex() + '4a')  # add eax, size; dec edx
    L.to(b'\x0f\x85', loop)
    b('33c0')
    L.land(hit)
    b('c3')
    L.pad()
    find_or_add = L.here()
    call(find)
    b('85c0'); found = fwd('0f85')
    b('b8'); ptr(table)
    b('ba' + d32(DELIA_TABLE_ENTRIES))
    loop = L.here()
    b('833800'); free = fwd('0f84')  # cmp dword [eax], 0
    b('83c0' + bytes((DELIA_ENTRY,)).hex() + '4a')
    L.to(b'\x0f\x85', loop)
    b('b8'); ptr(table)  # full: reuse the first entry
    L.land(free)
    b('57 51 50 8bf8 33c0 b9' + d32(DELIA_ENTRY // 4) + 'f3ab 58 59 5f')  # clear the entry
    b('8908')  # mov [eax], ecx
    L.land(found)
    b('c3')

    # set_health(ecx = entity, edx = value): the protected cell, then the networked copy
    L.pad()
    set_health = L.here()
    b('56 57 8bf1 8bfa 57 8d8e' + d32(f['health_cell']))
    call(k['health_set'])
    b('39be' + d32(f['health'])); same = fwd('0f84')
    b('8d86' + d32(f['health']) + '50 8bce 8b16 ff92' + d32(s['changed']))
    b('89be' + d32(f['health']))
    L.land(same)
    b('5f 5e c3')

    # status effects of a player (ecx), name in edx: level (0 when absent), remove, add (level in eax)
    L.pad()
    level_of = L.here()
    b('56 8bf1 8b06 6aff 52 8bce ff90' + d32(s['find']))  # find(name, -1)
    b('85c0'); none = fwd('0f8c')
    b('50 8b8e' + d32(f['effects']))
    call(k['level'])
    b('85c0'); ok = fwd('0f8d')
    L.land(none)
    b('33c0')
    L.land(ok)
    b('5e c3')
    L.pad()
    remove_named = L.here()
    b('56 8bf1 8b06 6aff 52 8bce ff90' + d32(s['find']))
    b('85c0'); none = fwd('0f8c')
    b('8b16 6a01 50 8bce ff92' + d32(s['remove']))
    L.land(none)
    b('5e c3')
    L.pad()
    add_named = L.here()
    b('6a00 6a00 51 50 52 8b01 ff90' + d32(s['add']))  # add(name, level, source = player, 0, 0)
    b('c3')

    # forfeit(ecx = player): a hit taken drops the reserve; the think removes the debuff
    L.pad()
    forfeit = L.here()
    call(find)
    b('85c0'); out = fwd('0f84')
    b('837804 00'); out2 = fwd('0f8e')
    b('c7400400000000 c6401701')
    L.land(out); L.land(out2)
    b('c3')

    # tick(ecx = player): deferred removals, then the reserve flows back into health
    L.pad()
    tick = L.here()
    b('56 57 8bf9')
    call(find)
    b('85c0'); out = fwd('0f84')
    b('8bf0')
    b('807e1700'); nf = fwd('0f84')
    b('c6461700 8bcf ba'); ptr(names['skill_delia_strong_attack_week'])
    call(remove_named)
    L.land(nf)
    b('807e1800'); nr = fwd('0f84')
    b('c6461800 8bcf')  # costume rebuild after a purge
    call(k['costume'])
    L.land(nr)
    b('807e1600'); nd = fwd('0f84')
    b('c6461600 8b07 6aff 68'); ptr(names['skill_delia_strong_attack'])
    b('8bcf ff90' + d32(s['find']))  # index of the Sharpness buff
    b('85c0'); nd2 = fwd('0f8c')
    b('50 50 8b8f' + d32(f['effects']))
    call(k['level'])
    b('5a 50 8b07 6a01 52 8bcf ff90' + d32(s['remove']))  # pop idx; keep level; remove(idx, 1)
    b('58 83f801'); nd3 = fwd('0f8e')
    b('48 8bcf ba'); ptr(names['skill_delia_strong_attack'])
    call(add_named)
    L.land(nd); L.land(nd2); L.land(nd3)
    b('837e0400'); out2 = fwd('0f8e')
    b('83bf' + d32(f['health']) + '00'); dead = fwd('0f8e')  # no restore once dead
    b('a1'); ptr(k['globals'])
    b('d9400c d86610 d84e08 d8460c d9c0')  # dt * rate + fraction; duplicate
    call(k['ftol'])
    b('50 db0424 dee9 d95e0c 58')  # fraction = amount - int(amount)
    b('8b0d'); ptr(k['globals'])
    b('8b490c 894e10')  # last = now
    b('85c0'); out3 = fwd('0f8e')
    b('3b4604'); fits = fwd('0f8e')
    b('8b4604')
    L.land(fits)
    b('294604')  # reserve -= amount
    b('8b97' + d32(f['health']) + '03d0 3b97' + d32(f['max_health'])); low = fwd('0f8e')
    b('8b97' + d32(f['max_health']))
    L.land(low)
    b('8bcf')
    call(set_health)
    L.land(out); L.land(out2); L.land(out3); L.land(dead)
    b('5f 5e c3')

    # effect classes
    def vtable_of(slots: list[int]) -> int:
        L.pad()
        at = L.here()
        for v in slots:
            ptr(v)
        return at

    def factory(size: int, vt_at: int) -> int:
        L.pad()
        at = L.here()
        b('68' + d32(size))
        call(k['new'])
        b('83c404 85c0'); r = fwd('0f84')
        b('56 8bf0 8bce')
        call(k['base_ctor'])
        b('c706'); ptr(vt_at)
        b('57 8dbe98000000 33c0 b9' + d32((size - 0x98) // 4) + 'f3ab 5f 8bc6 5e')
        L.land(r)
        b('c3')
        return at

    base1, base2, base3, base4, base5, base6 = 0, *k['base_slots']
    # stamina_adder_on_attack_time: +0x98 adder (stamina_adder_on_attack's load), +0x9c hits left
    L.pad()
    sat_load = L.here()
    b('56 8bf1 ff742408 8bce')
    call(k['stamina_load'])
    arg_float(names['validCount'])
    call(k['ftol'])
    b('89869c000000 5e c20400')
    L.pad()
    sat_hit = L.here()
    b('56 8bf1 83be9c00000000'); out = fwd('0f8e')
    b('8b44240c 85c0'); out2 = fwd('0f84')
    b('8b10 8bc8 ff92' + d32(s['npc']) + '85c0'); out3 = fwd('0f84')
    b('8b44240c 80b8' + d32(f['dead']) + '00'); out4 = fwd('0f85')
    b('ff74240c ff74240c 8bce')
    call(k['stamina_hit'])
    b('ff8e9c000000')
    for at in (out, out2, out3, out4):
        L.land(at)
    b('5e c20800')

    # delia_restore_health
    L.pad()
    drh_load = L.here()
    b('56 57 8bf1 807c240c00'); out = fwd('0f85')
    b('6a01 8bce')
    call(k['owner'])
    b('8bf8 85ff'); out2 = fwd('0f84')
    arg_float(names['reduce_ratio'])
    b('db87' + d32(f['health']) + 'dec9')  # * health
    call(k['ftol'])
    b('8b8f' + d32(f['health']) + '3bc1'); keep = fwd('0f8c')
    b('8d41ff')  # at least 1 HP stays
    L.land(keep)
    b('85c0'); out3 = fwd('0f8e')
    b('50 8bcf')
    call(find_or_add)
    b('8b1424 015004 50')  # reserve += amount; keep entry
    arg_float(names['restore_time'])
    b('58 db4004 def1 d95808')  # rate = reserve / restore_time
    b('8b0d'); ptr(k['globals'])
    b('8b490c 894810 c7400c00000000')
    b('5a 8b8f' + d32(f['health']) + '2bca 8bd1 8bcf')
    call(set_health)
    for at in (out, out2, out3):
        L.land(at)
    b('5f 5e c20400')

    # skill_delia_strong_attack: +0x98 time the count starts, +0x9c hits it takes
    L.pad()
    sda_load = L.here()
    b('56 8bf1')
    arg_float(names['delay_time'])
    b('a1'); ptr(k['globals'])
    b('d8400c d99e98000000')
    arg_float(names['immune_count'])
    call(k['ftol'])
    b('89869c000000 5e c20400')
    L.pad()
    sda_hurt = L.here()
    b('56 57 8bf1 6a01 8bce')
    call(k['owner'])
    b('8bf8 3b442410'); out = fwd('0f85')  # the owner is the one hit
    b('8b44240c 83783000'); out2 = fwd('0f84')  # no damage
    b('f7403c00000008'); out3 = fwd('0f85')  # unblockable
    b('f7403c00000010'); server = fwd('0f84')
    b('f6407802'); out4 = fwd('0f85')  # guarded on the client
    checked = fwd('e9')
    L.land(server)
    b('8b17 8bcf ff92' + d32(s['guarding']) + '84c0'); out5 = fwd('0f85')
    L.land(checked)
    b('a1'); ptr(k['globals'])
    b('d9400c d89e98000000 dfe0 f6c441'); out6 = fwd('0f85')  # still within delay_time
    b('ff8e9c000000'); out7 = fwd('0f89')
    b('8bcf')
    call(find_or_add)
    b('c6401601')  # decay due
    for at in (out, out2, out3, out4, out5, out6, out7):
        L.land(at)
    b('5f 5e c20800')

    # damage_modify_to_action: +0x98 rate, +0x9c candidate names (0x60 bytes)
    L.pad()
    dma_load = L.here()
    b('56 57 8bf1 68'); ptr(names[''])
    push_ptr(names['candidate_actions'])
    b('8bce')
    call(k['arg_text'])
    b('8dbe9c000000 b95f000000 8bd0 85d2'); done = fwd('0f84')
    copy = L.here()
    b('8a02 8807 84c0'); done2 = fwd('0f84')
    b('42 47 49')
    L.to(b'\x0f\x85', copy)
    b('c60700')
    L.land(done); L.land(done2)
    arg_float(names['damage_rates'])
    b('d99e98000000 5f 5e c20400')
    L.pad()
    dma_hit = L.here()
    b('56 57 53 8bf1 8b442410 85c0'); out = fwd('0f84')
    b('8d4860')
    call(k['hit_name'])
    b('85c0'); out2 = fwd('0f84')
    b('8bd8 8dbe9c000000')
    token = L.here()
    b('803f00'); out3 = fwd('0f84')
    b('803f20'); start = fwd('0f85')
    b('47')
    L.to(b'\xe9', token)
    L.land(start)
    b('8bcb 8bd7')
    same = L.here()
    b('8a01 84c0'); ended = fwd('0f84')
    b('3a02'); differ = fwd('0f85')
    b('41 42')
    L.to(b'\xe9', same)
    L.land(ended)
    b('8a02 84c0'); match = fwd('0f84')
    b('3c20'); match2 = fwd('0f84')
    L.land(differ)
    skip = L.here()
    b('8a07 84c0'); out4 = fwd('0f84')
    b('3c20')
    L.to(b'\x0f\x84', token)
    b('47')
    L.to(b'\xe9', skip)
    L.land(match); L.land(match2)
    b('8b442410 d98698000000 d84830 d95830')
    for at in (out, out2, out3, out4):
        L.land(at)
    b('5b 5f 5e c20800')

    # restore_equipment_def: armour HP + add_def, as armor_hp_adder
    L.pad()
    red_load = L.here()
    b('807c240400'); r = fwd('0f85')
    b('56 8bf1 6a01')
    call(k['player_owner'])
    b('85c0'); o = fwd('0f84')
    b('50')
    arg_float(names['add_def'])
    b('59 51 d91c24')
    call(k['armor_add'])
    L.land(o)
    b('5e')
    L.land(r)
    b('c20400')

    types = [
        ('stamina_adder_on_attack_time', 0xA0, [k['dtor'], sat_load, base2, base3, sat_hit, base5, base6]),
        ('delia_restore_health', 0x98, [k['dtor'], drh_load, base2, base3, base4, base5, base6]),
        ('skill_delia_strong_attack', 0xA0, [k['dtor'], sda_load, base2, base3, base4, sda_hurt, base6]),
        ('damage_modify_to_action', 0x100, [k['dtor'], dma_load, base2, base3, dma_hit, base5, base6]),
        ('restore_equipment_def', 0x98, [k['dtor'], red_load, base2, base3, base4, base5, base6]),
    ]
    factories = {}
    for name, size, slots in types:
        factories[name] = factory(size, vtable_of(slots))

    # registration, after stamina_adder_on_attack's (its registrar ends `ret; int3...`)
    L.pad()
    register = L.here()
    for name, _, _ in types:
        push_ptr(factories[name])
        push_ptr(names[name])
        b('b9'); ptr(registrar)
        call(k['register'])
    b('c3')
    edits.append((k['hook'], REGISTRAR_TAIL, b'\xe9' + struct.pack('<i', register - (k['hook'] + 5))))

    # AE_CUSTOM_MOTION commands at the dispatcher entry (ecx = event holder, [esp+4] = command)
    commands = [('ENABLE_STACK_CHARGE_SMASH_STATUS_EFFECT', 0x14, 1), ('DISABLE_STACK_CHARGE_SMASH_STATUS_EFFECT', 0x14, 0),
                ('ENABLE_ADD_STRONG_ATTACK_STATUS_EFFECT', 0x15, 1), ('DISABLE_ADD_STRONG_ATTACK_STATUS_EFFECT', 0x15, 0)]
    L.pad()
    events = L.here()
    b('51')
    hits = []
    for text in [c[0] for c in commands] + ['PURGE_ALL_ARMOR']:
        b('6a' + bytes((len(text) + 1,)).hex())
        push_ptr(names[text])
        b('ff742410')
        call(k['strncmp'])
        b('83c40c 85c0')
        hits.append(fwd('0f84'))
    purge = hits.pop()
    b('59')
    L.body += EVENTS_HEAD
    L.to(b'\xe9', k['events'] + len(EVENTS_HEAD))
    for at, (_, field, value) in zip(hits, commands):
        L.land(at)
        b('59 8b81' + d32(f['commands']) + '85c0'); no = fwd('0f84')
        b('83b8' + d32(klass) + bytes((DELIA,)).hex()); no2 = fwd('0f8c')
        b('8bc8')
        call(find_or_add)
        b('c640' + bytes((field, value)).hex())
        L.land(no); L.land(no2)
        b('c20400')
    # PURGE_ALL_ARMOR: armour slots 4-8 broken without the costume rebuild
    L.land(purge)
    b('59 8b81' + d32(f['commands']) + '85c0'); no = fwd('0f84')
    b('83b8' + d32(klass) + bytes((DELIA,)).hex()); no2 = fwd('0f8c')
    b('56 57 53 8bf8 8b8f' + d32(k['armor']) + '85c9'); none = fwd('0f84')  # edi = entity; armour container
    b('8bcf')
    call(find_or_add)
    b('8bd8 be04000000')  # ebx = entry; esi = slot 4
    worn = k['clothes'] + k['worn_part']

    def hide() -> None:
        # edi = entity, esi = slot, ebx = entry: keep the worn part + 1, then -1
        b('83bcb7' + d32(worn) + 'ff'); hidden = fwd('0f84')  # worn part already none
        b('8b84b7' + d32(worn) + '40 8944b3' + bytes((DELIA_KEPT_PART,)).hex())  # keep part + 1
        b('8d8f' + d32(k['clothes']) + '8d94b7' + d32(worn) + '52 8b01 ff5004')  # changed(&part)
        b('c784b7' + d32(worn) + 'ffffffff')  # part = -1
        L.land(hidden)

    def armor_on() -> None:
        # al = the armour system is on (the game state check 0x40814620 makes)
        b('8b0d'); ptr(k['game_state'])
        call(k['armor_on'])

    loop = L.here()
    b('8b8f' + d32(k['armor']) + '8b4904 56')  # ecx = armour HP; push slot
    call(k['armor_none'])
    b('84c0'); off = fwd('0f85')
    b('8b8f' + d32(k['armor']) + '8b4904 6a00 56')  # HP(slot) = 0.0
    call(k['armor_hp_set'])
    hide()
    b('6a00 6a01 56 8bcf')  # broken(slot, 1, no notify)
    call(k['armor_state'])
    skip = fwd('e9')
    L.land(off)  # no armour here, or the armour system is off: the look only
    armor_on()
    b('84c0'); skip2 = fwd('0f85')
    hide()
    L.land(skip); L.land(skip2)
    b('46 83fe09')
    L.to(b'\x0f\x8c', loop)
    b('8d8f' + d32(k['clothes']))  # worn -> resolved parts
    call(k['costume_parts'])
    b('8bcf')
    call(k['armor_update'])
    b('c6431801')  # rebuild the costume in the think
    L.land(none)
    b('5b 5f 5e')
    L.land(no); L.land(no2)
    b('c20400')

    # armour state(slot, broken, notify) entry: an un-broken purged slot gets its part back
    L.pad()
    restore = L.here()
    b('807c240800'); o1 = fwd('0f85')  # broken: original
    b('83b9' + d32(klass) + bytes((DELIA,)).hex()); o2 = fwd('0f8c')
    b('8b442404 83f804'); o3 = fwd('0f8c')
    b('83f808'); o4 = fwd('0f8f')
    b('56 57 53 51 8bf0')  # esi = slot
    call(find)
    b('59 85c0'); d1 = fwd('0f84')
    b('8bd8 8b7cb3' + bytes((DELIA_KEPT_PART,)).hex() + ' 85ff'); d2 = fwd('0f84')  # edi = part + 1
    b('c744b3' + bytes((DELIA_KEPT_PART,)).hex() + '00000000 4f')
    b('83bcb1' + d32(worn) + 'ff'); d3 = fwd('0f85')  # the costume was rebuilt meanwhile
    b('51 8d94b1' + d32(worn) + '52 8d89' + d32(k['clothes']) + '8b01 ff5004 59')  # changed(&part)
    b('89bcb1' + d32(worn))  # part back
    b('c6431801')  # rebuild in the think
    for at in (d1, d2, d3):
        L.land(at)
    b('5b 5f 5e')
    for at in (o1, o2, o3, o4):
        L.land(at)
    L.body += ARMOR_STATE_HEAD
    L.to(b'\xe9', k['armor_state'] + len(ARMOR_STATE_HEAD))
    edits.append((k['armor_state'], ARMOR_STATE_HEAD,
                  b'\xe9' + struct.pack('<i', restore - (k['armor_state'] + 5)) + b'\x90' * 2))

    # player repair(rate) entry: with the armour system off, the kept parts come back (look only)
    L.pad()
    repair = L.here()
    b('83b9' + d32(klass) + bytes((DELIA,)).hex()); r1 = fwd('0f8c')
    b('51')
    armor_on()
    b('59 84c0'); r2 = fwd('0f85')  # armour on: the un-break path restores
    b('56 57 53 51')
    call(find)
    b('59 85c0'); r3 = fwd('0f84')
    b('8bd8 be04000000')
    rl = L.here()
    b('8b7cb3' + bytes((DELIA_KEPT_PART,)).hex() + ' 85ff'); rn = fwd('0f84')
    b('c744b3' + bytes((DELIA_KEPT_PART,)).hex() + '00000000 4f')
    b('83bcb1' + d32(worn) + 'ff'); rn2 = fwd('0f85')
    b('51 8d94b1' + d32(worn) + '52 8d89' + d32(k['clothes']) + '8b01 ff5004 59')  # changed(&part)
    b('89bcb1' + d32(worn) + ' c6431801')  # part back; rebuild in the think
    L.land(rn); L.land(rn2)
    b('46 83fe09')
    L.to(b'\x0f\x8c', rl)
    b('51 8d89' + d32(k['clothes']))  # worn -> resolved parts
    call(k['costume_parts'])
    b('59')
    L.land(r3)
    b('5b 5f 5e')
    L.land(r1); L.land(r2)
    L.body += REPAIR_HEAD
    L.to(b'\xe9', k['repair'] + len(REPAIR_HEAD))
    edits.append((k['repair'], REPAIR_HEAD, b'\xe9' + struct.pack('<i', repair - (k['repair'] + 5))))
    edits.append((k['events'], EVENTS_HEAD, b'\xe9' + struct.pack('<i', events - (k['events'] + 5)) + b'\x90' * 3))

    # on hit (slot 70): the original, then the armed flags add Delia's buffs
    original = struct.unpack_from('<I', image.data, image.offset(vtable(image, entity) + 4 * k['hit_slot']))[0]
    L.pad()
    on_hit = L.here()
    b('56 57 8bf1 ff742410 ff742410 8bce')
    call(original)
    b('8b7c2410 85ff'); out = fwd('0f84')
    b('8b17 8bcf ff92' + d32(s['npc']) + '85c0'); out2 = fwd('0f84')
    b('80bf' + d32(f['dead']) + '00'); out3 = fwd('0f85')
    b('8bce')
    call(find)
    b('85c0'); out4 = fwd('0f84')
    b('8bf8 807f1400'); s2 = fwd('0f84')
    b('c6471400 8bce ba'); ptr(names['skill_delia_strong_attack'])
    call(level_of)
    b('40 83f803'); cap = fwd('0f8e')
    b('b803000000')
    L.land(cap)
    b('8bce ba'); ptr(names['skill_delia_strong_attack'])
    call(add_named)
    L.land(s2)
    b('807f1500'); out5 = fwd('0f84')
    b('8bce ba'); ptr(names['skill_delia_strong_attack_hit'])
    call(level_of)
    b('83f801'); out6 = fwd('0f8f')
    b('b801000000 8bce ba'); ptr(names['skill_delia_strong_attack_hit'])
    call(add_named)
    for at in (out, out2, out3, out4, out5, out6):
        L.land(at)
    b('5f 5e c20800')

    return {'table': hex(table), 'tick': hex(tick), 'forfeit': hex(forfeit), 'register': hex(register),
            'events': hex(events), 'restore': hex(restore), 'repair': hex(repair),
            'factories': {n: hex(v) for n, v in factories.items()},
            'hit': class_dispatch(image, layout, edits, relocs, entity, klass, k['hit_slot'], on_hit),
            'on_hit': hex(on_hit)}


def class_conditions(image: Image, layout: Layout, edits: list, relocs: set[int], name: str, think: int,
                     conditions_of: int | None, raise_: int, tick: int | None = None) -> dict:
    """CLASS_CONDITIONS: the player think raises the extra class's standing conditions, then runs
    `tick(this)` when given."""
    entity, klass = ENTITY_SLOTS[name][0], ENTITY_SLOTS[name][1]
    base = struct.unpack_from('<I', image.data, image.offset(vtable(image, entity) + 4 * think))[0]
    purge = FIRST_NEW_CONDITION + CONDITION_NAMES.index('As_Cond_Can_Armor_Purge')
    layout.pad()
    body = layout.here()
    layout.body += bytes((0x56, 0x8B, 0xF1))  # push esi; mov esi, ecx
    layout.to(bytes((0xE8,)), base)
    if conditions_of is None:
        layout.body += bytes((0x8B, 0x8E)) + struct.pack('<I', COMMANDS)  # mov ecx, [esi + commands]
    else:
        layout.body += bytes((0x8B, 0xCE))
        layout.to(bytes((0xE8,)), conditions_of)
        layout.body += bytes((0x8B, 0xC8))  # mov ecx, eax
    layout.body += bytes((0x85, 0xC9))
    none = layout.forward(bytes((0x0F, 0x84)))
    layout.body += bytes((0x68,)) + struct.pack('<I', purge)
    layout.to(bytes((0xE8,)), raise_)
    layout.land(none)
    if tick is not None:
        layout.body += bytes((0x8B, 0xCE))
        layout.to(bytes((0xE8,)), tick)
    layout.body += bytes((0x5E, 0xC3))  # pop esi; ret
    return {'think': hex(body), 'base': hex(base),
            'slot': class_dispatch(image, layout, edits, relocs, entity, klass, think, body)}


def item_mask(layout: Layout, edits: list, va: int, old: bytes) -> str:
    """ITEM_MASK: the item class mask also carries bit 10 (DELIA_FLAG)."""
    layout.pad()
    stub = layout.here()
    layout.body += old[:-1]  # Hagie's bit as before, without the `ret`
    layout.body += bytes((0x80, 0xB9)) + struct.pack('<I', DELIA_FLAG) + bytes.fromhex('00' '7405' '0500040000' 'c3')  # + Delia's bit
    edits.append((va, old, bytes((0xE9,)) + struct.pack('<i', stub - (va + 5)) + bytes((0xCC,)) * (len(old) - 5)))
    return f'{va:#x} -> {stub:#x}'


def immediately(layout: Layout, edits: list, va: int, strcmp: int) -> str:
    """FRAME_KEYS: `$immediately$` is frame 0, as NA reads it (without NA's flag)."""
    key = layout.string('$immediately$')
    layout.pad()
    stub = layout.here()
    layout.body += bytes((0x68,))
    layout.pointer(key)
    layout.body += bytes((0x55,))  # push ebp (the key text)
    layout.to(bytes((0xE8,)), strcmp)
    layout.body += bytes.fromhex('83c408' '85c0')  # add esp, 8; test eax, eax
    other = layout.forward(bytes((0x0F, 0x85)))
    layout.body += bytes((0x33, 0xF6))  # xor esi, esi
    layout.to(bytes((0xE9,)), va + FRAME_KEY_DONE)
    layout.land(other)
    layout.body += FRAME_KEY_SITE
    layout.to(bytes((0xE9,)), va + len(FRAME_KEY_SITE))
    edits.append((va, FRAME_KEY_SITE, bytes((0xE9,)) + struct.pack('<i', stub - (va + 5)) + bytes((0xCC,)) * 2))
    return f'{va:#x} -> {stub:#x}'


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


def creation_names() -> list[tuple[int, str]]:
    """(m_iClass, key) of every class the grid can create: the 1.69 ten, then EXTRA_CLASSES."""
    return list(enumerate(OLD_CLASSES)) + [(extra.index, extra.key) for extra in EXTRA_CLASSES]


def creation_client(image: Image, layout: Layout, edits: list, relocs: set[int], k: dict) -> dict:
    """CREATION_GRID, client: the S3 grid in lobby_beautyshop_change, the dialog commands and the
    CharacterCreationState of the NA's CharacterCreation_UI_V3 flow, all behind S3_CharacterCreation."""
    names = creation_names()
    layout.pad()
    text = {t: layout.string(t) for t in (
        'changemap lobby_beautyshop_change', 'gameui_hide;cc_minimap_hide;npc_character_create;force_start_touch trigger_cam_midview',
        'OnStartCreation', 'npc_character_create;force_start_touch trigger_cam_midview',
        'NewCharacterDialogS3.OnReadyCreation', 'sv_prologue_mode 1', 'scaleform_fade_out 1.5',
        'changemap lobby_beautyshop_body', 'cam_select_beautyshop_camera', 'scaleform_fade_in 1.5',
        'cc_show_create_character_dialog', 'hhud_should_draw 0', 'sv_create_character_mode 0;hhud_should_draw 1')}
    select = {i: (layout.string(f'Select{key.capitalize()}'), layout.string(f'npc_character_create_select {i}'))
              for i, key in names}

    def feature() -> None:  # al = S3_CharacterCreation is on
        layout.body += b'\x8b\x0d'
        layout.pointer(k['features'])
        layout.body += b'\x8b\x01' + b'\x68'
        layout.pointer(k['s3_name'])
        layout.body += b'\xff\x50\x18' + b'\x84\xc0'

    def command(slot: int, string: int) -> None:  # engine->ClientCmd(string), slot 0x188 or 0x1c
        layout.body += b'\x8b\x0d'
        layout.pointer(k['engine'])
        layout.body += b'\x8b\x11' + b'\x68'
        layout.pointer(string)
        layout.body += b'\xff\x92' + struct.pack('<I', slot)

    def invoke(string: int) -> None:  # the UI manager calls a movie function without arguments
        layout.to(b'\xe8', k['ui'])
        layout.body += b'\x8b\x10' + b'\x6a\x00' * 3 + b'\x8b\xc8' + b'\x68'
        layout.pointer(string)
        layout.body += b'\xff\x52\x10'

    def is_argument(string: int) -> int:  # strcmp(edi, string); a forward jne to fill in
        layout.body += b'\x68'
        layout.pointer(string)
        layout.body += b'\x57'
        layout.to(b'\xe8', k['strcmp'])
        layout.body += b'\x83\xc4\x08' + b'\x85\xc0'
        return layout.forward(b'\x0f\x85')

    def operand(site: tuple[int, str], string: int) -> None:
        va, old = site
        if va + 1 not in relocs:
            raise PreparationError(f'No relocation at {va + 1:#x}; nothing is patched.')
        old_bytes = bytes.fromhex(old)
        edits.append((va, old_bytes, old_bytes[:1] + struct.pack('<I', string)))

    report = {}
    # Register the grid handler, then fall through to the V2 test, which registers the V4 one.
    va, old = k['register_test']
    edits.append((va, bytes.fromhex(old), b'\x90\x90'))
    va, old = k['register_failed']
    edits.append((va, bytes.fromhex(old), b'\x0f\x8c' + struct.pack('<i', k['register_v2'] - (va + 6))))
    va, old = k['register_done']
    layout.pad()
    stub = layout.here()
    head, tail = k['register_tail'].split(b'%s')
    layout.body += head
    layout.pointer(k['register_list'])
    layout.body += tail
    layout.to(b'\xe9', k['register_v2'])
    edits.append((va, bytes.fromhex(old), b'\xe9' + struct.pack('<i', stub - (va + 5))))
    report['register'] = f'{va:#x} -> {stub:#x}'
    operand(k['map_operand'], text['changemap lobby_beautyshop_change'])
    # Map ready: the scene commands and OnReadyCreation (which shows the first class and its video)
    # straight from here. The movie also calls OnStartCreation when it opens, before the map loads;
    # answering that played Lethita's video during the loading, then again once loaded (C3-11).
    operand(k['start_camera'], text['gameui_hide;cc_minimap_hide;npc_character_create;force_start_touch trigger_cam_midview'])
    operand(k['start_invoke'], text['NewCharacterDialogS3.OnReadyCreation'])
    # CharacterCreationState leave (as NA 0x38bd8c50): the overhead HUD back on with the creation mode off.
    operand(k['leave_command'], text['sv_create_character_mode 0;hhud_should_draw 1'])

    # The grid dialog's FSCommand handler: OnStartCreation and Select<Class> here, the rest (GameStart,
    # Back) in the original body with the same frame (esi = dialog, edi = argument).
    handler, head = k['handler'][0], bytes.fromhex(k['handler'][1])
    layout.pad()
    stub = layout.here()
    layout.body += head + b'\x8b\xf1'  # ...; mov esi, ecx
    done = []
    other = is_argument(text['OnStartCreation'])  # answered by the map-ready slot above
    done.append(layout.forward(b'\xe9'))
    layout.land(other)
    for i, (choose, scene) in select.items():
        other = is_argument(choose)
        layout.body += b'\x8b\x46\x20' + b'\xc7\x00' + struct.pack('<I', i) + b'\xc6\x40\x09\x01'
        command(0x188, scene)
        done.append(layout.forward(b'\xe9'))
        layout.land(other)
    layout.to(b'\xe9', k['handler_rest'])
    for at in done:
        layout.land(at)
    layout.body += b'\x5f\x5e' + b'\x81\xc4\x00\x02\x00\x00' + b'\xc2\x0c\x00'
    edits.append((handler, head[:6], b'\xe9' + struct.pack('<i', stub - (handler + 5)) + b'\x90'))
    report['handler'] = f'{handler:#x} -> {stub:#x}'

    # NewCharacterSelection_Start: CharacterCreationState instead of the prologue.
    va, old = k['transition']
    layout.pad()
    stub = layout.here()
    feature()
    layout.body += b'\xb8\x08\x00\x00\x00' + b'\x74\x05' + b'\xb8' + struct.pack('<I', CREATION_STATE) + b'\xc3'
    edits.append((va, bytes.fromhex(old), b'\xe9' + struct.pack('<i', stub - (va + 5))))
    report['transition'] = f'{va:#x} -> {stub:#x}'

    # CharacterCreationState enter: with the grid, only the change to the customization map.
    va, old = k['enter']
    layout.pad()
    stub = layout.here()
    feature()
    grid = layout.forward(b'\x0f\x85')
    body = layout.here()
    layout.body += bytes.fromhex(old)
    layout.to(b'\xe9', va + len(bytes.fromhex(old)))
    layout.land(grid)
    layout.to(b'\xe8', k['machine'])
    layout.body += b'\x8b\xc8'
    layout.to(b'\xe8', k['machine_class'])
    layout.body += b'\x50'
    layout.to(b'\xe8', k['player_class'])
    layout.body += b'\x83\xc4\x04'
    # NA 0x38bd8683: the overhead HUD is off in the customization ("Lv. 0" above the model otherwise).
    for t in ('sv_prologue_mode 1', 'scaleform_fade_out 1.5', 'changemap lobby_beautyshop_body', 'hhud_should_draw 0'):
        command(0x1C, text[t])
    layout.body += b'\xc3'
    edits.append((va, bytes.fromhex(old), b'\xe9' + struct.pack('<i', stub - (va + 5)) + b'\x90'))
    report['enter'] = f'{va:#x} -> {stub:#x}'

    # Its slot 4 (map ready): the original enter body (creation mode, costume), then the camera and
    # the customization dialog that create_character.bsp's trigger_create_character used to open.
    table = vtable(image, 'CharacterCreationState')
    if table != k['creation_vtable'] or table + 0x10 not in relocs:
        raise PreparationError(f'Unexpected CharacterCreationState vtable {table:#x}; nothing is patched.')
    slot = struct.unpack_from('<I', image.data, image.offset(table + 0x10))[0]
    layout.pad()
    stub = layout.here()
    feature()
    layout.body += b'\x0f\x84' + struct.pack('<i', slot - (layout.here() + 6))
    layout.to(b'\xe8', body)
    for t in ('cam_select_beautyshop_camera', 'scaleform_fade_in 1.5', 'cc_show_create_character_dialog'):
        command(0x1C, text[t])
    layout.body += b'\xc3'
    edits.append((table + 0x10, struct.pack('<I', slot), struct.pack('<I', stub)))
    report['map_ready'] = f'{table:#x}[4] {slot:#x} -> {stub:#x}'

    # Customization Back: a CharacterCreationState -> class grid transition in a copy of the table.
    back_event, prologue_off = layout.string('CharacterCreation_Back'), layout.string('sv_prologue_mode 0')
    back_label = layout.string('Back')
    show_label, button = layout.string('OnShow'), layout.string('CreateCharacterDialog.Button_Back._visible')
    shown = layout.string('1')
    name_color, white = layout.string('CreateCharacterDialog.Text_Name.textColor'), layout.string('16777215')  # "true" converts to NaN, false, for a display property (C3-9)
    layout.pad()
    back = layout.here()
    command(0x1C, prologue_off)
    layout.body += b'\xb8' + struct.pack('<I', k['grid_state']) + b'\xc3'
    count_va, count_old, table_va, table_old = k['transitions']
    count = struct.unpack_from('<I', bytes.fromhex(count_old), 1)[0]
    old_table = struct.unpack_from('<I', bytes.fromhex(table_old), 1)[0]
    if table_va + 1 not in relocs:
        raise PreparationError(f'No relocation at {table_va + 1:#x}; nothing is patched.')
    layout.pad()
    copy = layout.here()
    for i in range(count):
        state, name, handler = struct.unpack_from('<III', image.data, image.offset(old_table + 12 * i))
        layout.body += struct.pack('<I', state)
        layout.pointer(name)
        layout.pointer(handler)
    layout.body += struct.pack('<I', CREATION_STATE)
    layout.pointer(back_event)
    layout.pointer(back)
    edits.append((count_va, bytes.fromhex(count_old), b'\x68' + struct.pack('<I', count + 1)))
    edits.append((table_va, bytes.fromhex(table_old), b'\x68' + struct.pack('<I', copy)))
    report['transitions'] = f'{old_table:#x}[{count}] -> {copy:#x}[{count + 1}]'

    # The dialog: Back fires the event; OnShow (grid only) shows the button, then the original runs.
    va, old = k['dialog']
    head = bytes.fromhex(old)
    layout.pad()
    stub = layout.here()
    layout.body += head + b'\x8b\xf1'
    other = is_argument(back_label)
    layout.body += b'\x6a\x00' + b'\x68'
    layout.pointer(back_event)
    layout.to(b'\xe8', k['machine'])
    layout.body += b'\x8b\xc8'
    layout.to(b'\xe8', k['fire'])
    layout.body += b'\x5f\x5e' + b'\x81\xc4' + struct.pack('<I', 0x3DC) + b'\xc2\x0c\x00'
    layout.land(other)
    # The movie's own onShow hides the button after the OnShow command; it ends with
    # selectHairTab(), whose OnTabSelected sends `SelectTab.<from>.<to>`: show it then too.
    # cmp [edi], "Sele"; jne; cmp [edi+4], "ctTa"; jne; cmp byte [edi+8], 'b'; je show
    layout.body += b'\x81\x3f' + b'Sele' + b'\x75\x13' + b'\x81\x7f\x04' + b'ctTa' + b'\x75\x0a'
    layout.body += b'\x80\x7f\x08' + b'b'
    tab = layout.forward(b'\x0f\x84')
    original = is_argument(show_label)
    layout.land(tab)
    feature()
    hidden = layout.forward(b'\x0f\x84')
    # UI manager vt+0x1c sets a variable from TEXT (the 1.69 passes the result of vt+0x14, a string:
    # 0x3828d24a, 0x3826dc35); the boolean entry vt+0x18 left the button hidden (C3-7). The 1.69
    # layout already places Button_Back between Button_BasicShape (x 7) and Button_Complete (x 204).
    layout.to(b'\xe8', k['ui'])
    layout.body += b'\x8b\x10' + b'\x68'
    layout.pointer(shown)
    layout.body += b'\x8b\xc8' + b'\x68'
    layout.pointer(button)
    layout.body += b'\xff\x52\x1c'
    layout.land(original)
    layout.land(hidden)
    # The original continues with `mov esi, ecx` (0x384af6f5): ecx must be the dialog again, after
    # the strcmp and feature calls above (Create read a null [esi+0x20] otherwise, C3-15).
    layout.body += b'\x8b\xce'  # mov ecx, esi
    layout.to(b'\xe9', k['dialog_rest'])
    edits.append((va, head[:6], b'\xe9' + struct.pack('<i', stub - (va + 5)) + b'\x90'))
    report['dialog'] = f'{va:#x} -> {stub:#x}'

    va, old = k['filled']
    layout.pad()
    stub = layout.here()
    feature()
    skip = layout.forward(b'\x0f\x84')
    layout.to(b'\xe8', k['ui'])
    layout.body += b'\x8b\x10' + b'\x68'
    layout.pointer(shown)
    layout.body += b'\x8b\xc8' + b'\x68'
    layout.pointer(button)
    layout.body += b'\xff\x52\x1c'
    layout.to(b'\xe8', k['ui'])
    layout.body += b'\x8b\x10' + b'\x68'
    layout.pointer(white)
    layout.body += b'\x8b\xc8' + b'\x68'
    layout.pointer(name_color)
    layout.body += b'\xff\x52\x1c'
    layout.land(skip)
    layout.to(b'\xe9', k['ui'])
    edits.append((va, bytes.fromhex(old), b'\xe8' + struct.pack('<i', stub - (va + 5))))
    report['filled'] = f'{va:#x} -> {stub:#x}'
    return report


def creation_server(layout: Layout, edits: list, k: dict) -> dict:
    """CREATION_GRID, server: `npc_character_create` (every actor to the non-select point) and
    `npc_character_create_select <class>` (that actor to the select point, the others away)."""
    names = creation_names()
    layout.pad()
    create, choose = layout.string('npc_character_create'), layout.string('npc_character_create_select')
    selected = layout.string('ACT_CHARACTER_CREATION_SELECTED_S3')
    for va, old in k['activity']:
        edits.append((va, bytes.fromhex(old), b'\x68' + struct.pack('<I', selected)))
    actors = [(i, layout.string(f'npc_{key}')) for i, key in names]
    layout.pad()
    places = {}
    for place in ('select', 'nonselect'):
        origin, angles = k[place]
        places[place] = (layout.here(), layout.here() + 12)
        layout.body += struct.pack('<3f', *origin) + struct.pack('<3f', *angles)
    layout.pad()
    objects = (layout.here(), layout.here() + 0x80)  # two ConCommand objects (the section is writable)
    layout.body += bytes(0x100)

    layout.pad()
    find = layout.here()  # find(name): gEntList.FindEntityByName(NULL, name, NULL, NULL, NULL, NULL)
    layout.body += b'\x6a\x00' * 4 + b'\xff\x74\x24\x14' + b'\x6a\x00' + b'\xb9'
    layout.pointer(k['entities'])
    layout.to(b'\xe8', k['find'])
    layout.body += b'\xc2\x04\x00'
    layout.pad()
    teleport = layout.here()  # teleport(entity, origin, angles): entity->Teleport(origin, angles, NULL)
    layout.body += (b'\x8b\x4c\x24\x04' + b'\x8b\x01' + b'\x6a\x00' + b'\xff\x74\x24\x10' * 2
                    + b'\xff\x90' + struct.pack('<I', k['teleport']) + b'\xc2\x0c\x00')

    def move(place: str) -> None:  # teleport(eax, place)
        layout.body += b'\x68'
        layout.pointer(places[place][1])
        layout.body += b'\x68'
        layout.pointer(places[place][0])
        layout.body += b'\x50'
        layout.to(b'\xe8', teleport)

    def each_actor(body) -> None:
        for i, name in actors:
            layout.body += b'\x68'
            layout.pointer(name)
            layout.to(b'\xe8', find)
            layout.body += b'\x85\xc0'
            skip = layout.forward(b'\x0f\x84')
            body(i)
            layout.land(skip)

    layout.pad()
    create_callback = layout.here()
    each_actor(lambda i: move('nonselect'))
    layout.body += b'\xc3'

    layout.pad()
    choose_callback = layout.here()
    layout.body += b'\x56\x53' + b'\x8b\x0d'
    layout.pointer(k['engine'])
    layout.body += b'\x8b\x01' + b'\xff\x90\xd0\x00\x00\x00' + b'\x83\xf8\x02'
    out = layout.forward(b'\x0f\x8c')
    layout.body += b'\x8b\x0d'
    layout.pointer(k['engine'])
    layout.body += b'\x8b\x01' + b'\x6a\x01' + b'\xff\x90\xd4\x00\x00\x00' + b'\x8b\xf0' + b'\x33\xdb'
    digit = layout.here()  # ebx = the decimal class argument
    layout.body += b'\x0f\xb6\x06' + b'\x83\xe8\x30' + b'\x83\xf8\x09'
    parsed = layout.forward(b'\x0f\x87')
    layout.body += b'\x6b\xdb\x0a' + b'\x03\xd8' + b'\x46'
    layout.to(b'\xe9', digit)
    layout.land(parsed)

    def place_actor(i: int) -> None:
        layout.body += b'\x83\xfb' + bytes((i,))
        away = layout.forward(b'\x0f\x85')
        move('select')
        moved = layout.forward(b'\xe9')
        layout.land(away)
        move('nonselect')
        layout.land(moved)
    each_actor(place_actor)
    layout.land(out)
    layout.body += b'\x5b\x5e' + b'\xc3'

    # Registered from force_start_touch's static initializer, before ConVar_Register links them.
    va, old = k['init']
    layout.pad()
    stub = layout.here()
    for name, callback, obj in ((create, create_callback, objects[0]), (choose, choose_callback, objects[1])):
        layout.body += b'\x6a\x00' + b'\x6a' + bytes((k['flags'],))
        for value in (name, callback, name):
            layout.body += b'\x68'
            layout.pointer(value)
        layout.body += b'\xb9'
        layout.pointer(obj)
        layout.to(b'\xe8', k['ctor'])
    old_bytes = bytes.fromhex(old)
    layout.body += b'\x68'
    layout.pointer(struct.unpack_from('<I', old_bytes, 1)[0])
    layout.to(b'\xe9', k['init_rest'])
    edits.append((va, old_bytes, b'\xe9' + struct.pack('<i', stub - (va + 5))))
    return {'init': f'{va:#x} -> {stub:#x}', 'npc_character_create': hex(create_callback),
            'npc_character_create_select': hex(choose_callback), 'commands': [hex(o) for o in objects]}


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
                 figure_slot: int | None, constant_slots: tuple[int, ...], figure_gate: int | None,
                 values: str) -> dict:
    """ENTITY_SLOTS: per-class dispatchers in the shared player entity's vtable."""
    table = vtable(image, name)
    layout.pad()
    figures = {extra.index: layout.string(extra.figure) for extra in EXTRA_CLASSES}
    layout.pad()
    constants = {}
    for extra in EXTRA_CLASSES:
        constants[extra.index] = layout.here()
        floats = getattr(extra, values)
        if len(floats) != len(constant_slots):
            raise PreparationError(f'{extra.key}: {values} needs {len(constant_slots)} values.')
        layout.body += struct.pack(f'<{len(floats)}f', *floats)

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

    return {'vtable': hex(table), 'figure': dispatch(figure_slot, figure) if figure_slot is not None else None,
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
    for va, old, new in SERVER_CLASS_BOUNDS.get(name, ()):
        edits.append((va, bytes.fromhex(old), new(LAST)))
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
    report['shape_class'] = [shape_class(layout, edits, *entry) for entry in SHAPE_CLASS.get(name, ())]
    report['class_defaults'] = [class_default(image, layout, edits, va) for va in CLASS_DEFAULTS.get(name, ())]
    if name in CLASS_WEAPONS:
        report['class_weapons'] = class_weapons(image, layout, edits, relocs, CLASS_WEAPONS[name])
    report['class_alias'] = [class_alias(image, layout, edits, relocs, *entry) for entry in CLASS_ALIAS.get(name, ())]
    if name in CLIENT_WEAPON_SWITCH:
        report['client_weapon_switch'] = client_weapon_switch(image, layout, edits, *CLIENT_WEAPON_SWITCH[name])
    if name in CONDITIONS:
        report['conditions'] = conditions(layout, edits, *CONDITIONS[name])
    effects = None
    if name in DELIA_EFFECTS:
        effects = delia_effects(image, layout, edits, relocs, DELIA_EFFECTS[name])
        report['delia_effects'] = effects
    if name in GUARD:
        report['guard'] = guard(image, layout, edits, relocs, *GUARD[name],
                                forfeit=int(effects['forfeit'], 16) if effects else None)
    if name in CLIENT_GUARD:
        report['guard'] = client_guard(image, layout, edits, relocs, *CLIENT_GUARD[name])
    if name in ITEM_MASK:
        report['item_mask'] = item_mask(layout, edits, *ITEM_MASK[name])
    if name in ITEM_COPY:
        va, old, new = ITEM_COPY[name]
        edits.append((va, old, new))
        report['item_copy'] = f'{va:#x}: UseInQuest dword (+{DELIA_FLAG:#x})'
    if name in FRAME_KEYS:
        report['frame_keys'] = immediately(layout, edits, *FRAME_KEYS[name])
    if name in CLASS_CONDITIONS:
        report['class_conditions'] = class_conditions(image, layout, edits, relocs, name, *CLASS_CONDITIONS[name],
                                                      tick=int(effects['tick'], 16) if effects else None)
    if name in ENTITY_SLOTS:
        report['entity_slots'] = entity_slots(image, layout, edits, relocs, *ENTITY_SLOTS[name])
    if name == 'client.dll':
        report['creation_grid'] = creation_client(image, layout, edits, relocs, CREATION_GRID[name])
    elif name == 'server.dll':
        report['creation_grid'] = creation_server(layout, edits, CREATION_GRID[name])
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
