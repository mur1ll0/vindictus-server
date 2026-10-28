"""Route class 10 (Delia) to her files in COPIES of client.dll and server.dll. Dry-run by default.

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
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

from patch_class_table import Image, align, digest  # noqa: E402
from prepare_lab_client import PreparationError, reject_original  # noqa: E402

SECTION = b'.hrte\0\0\0'
PET = 28
DELIA = 10
TABLE_SIZE = 32
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
# Server string tables indexed by m_iClass without a bound: (VA, Delia entry or None).
TABLES = {
    'server.dll': (
        (0x40CF9E64, 'csf/create_delia.csf'),
        (0x40CF9E3C, 'csf/prologue_delia.csf'),
        (0x40CF968C, 'Player.Delia.'),
        (0x40CFA0B0, 'csf/costume_default_delia.csf'),
        (0x40CFA0D8, None),  # PvP costume: Hagie's entry is null too
        # Animation models read by [class*4 + table] at 0x4040ff6c (town), 0x4040ffa9 (battle) and
        # 0x4040ff91 (ExtremeMode). Class 10 read the next table (a male skeleton) and, in
        # ExtremeMode, a non-pointer; D8c froze on the emote.
        (0x40CF1F80, 'models/player/_anim/female/delia/pc_delia_town_motion.mdl'),
        (0x40CF1FA8, 'models/player/_anim/female/delia/pc_delia_battle_motion.mdl'),
        (0x40CF1FD0, 'models/player/_anim/female/delia/pc_delia_battle_motion_ex.mdl'),
    ),
}
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
DELIA_MOTIONS = [text for va, text in TABLES['server.dll'] if text and '_motion' in text]
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


def delia(text: str) -> str:
    return re.sub('hagie', lambda m: 'delia' if m.group() == 'hagie' else 'Delia' if m.group() == 'Hagie'
                  else 'DELIA', text, flags=re.IGNORECASE)


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


def clone_case(image: Image, layout: Layout, start: int, end: int, relocs: set[int]) -> list[str]:
    """Copy the Hagie case [start, end) into the layout with Delia strings; returns the strings."""
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
                if text and 'hagie' in text.lower():
                    struct.pack_into('<I', out, i, layout.strings[delia(text)])
                    replaced.append(delia(text))
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
    """Delia's action-state map and the two stubs that reach it (MATCH)."""
    for va, raw in ((lookup, MATCH_LOOKUP), (pick, MATCH_PICK)):
        if any(va - 3 <= r < va + len(raw) for r in relocs):
            raise PreparationError(f'Relocated dword inside {va:#x}; nothing is patched.')
    if b'\xbd' + struct.pack('<I', less) not in image.data:
        raise PreparationError(f'No `mov ebp, {less:#x}` (the maps\' less function); nothing is patched.')
    # The map, as the constructor leaves each class map: less function, empty CUtlMemory,
    # root and first free 0xffff, no element.
    layout.pad()
    delia_map = layout.here()
    layout.pointer(less)
    layout.body += struct.pack('<IIIHHHHI', 0, 0, 0, 0xFFFF, 0, 0xFFFF, 0, 0)

    # GetSkillNamesByActionStateName: classes >= 10 use Delia's map (edi) and its base (ebx = map - 0x20,
    # read again as ebx + 0x24); lower classes keep the original computation.
    layout.pad()
    lookup_stub = layout.here()
    layout.body += MATCH_LOOKUP[:5] + b'\x83\xf8\x0a' + b'\x7c\x0a' + b'\xbf'
    layout.pointer(delia_map)
    layout.body += b'\x8d\x5f\xe0' + b'\xeb\x0f' + MATCH_LOOKUP[5:17] + MATCH_LOOKUP[21:24] + MATCH_LOOKUP[17:21]
    layout.body += b'\xe9' + struct.pack('<i', lookup + len(MATCH_LOOKUP) - (layout.here() + 5))
    edits.append((lookup, MATCH_LOOKUP,
                  b'\xe9' + struct.pack('<i', lookup_stub - (lookup + 5)) + b'\x90' * (len(MATCH_LOOKUP) - 5)))

    # MatchingActionStateSkill: the loop's class is at [esp + 0x28]; class 10 fills Delia's map (ebp).
    # The loop also keeps &map->m_Elements at [esp + 0x10] and reads it again after an insert
    # (client 0x38860b4a, server 0x407cda5a; D8c crashed there), so class 10 points it at her map
    # too. The loop ends after class 10, so the `add [esp + 0x10], 0x1c` that follows is harmless.
    layout.pad()
    pick_stub = layout.here()
    layout.body += b'\x83\x7c\x24\x28\x0a' + b'\x7c\x0f' + b'\xbd'
    layout.pointer(delia_map)
    layout.body += b'\xc7\x44\x24\x10'
    layout.pointer(delia_map + 4)
    layout.body += b'\xeb\x07' + MATCH_PICK
    layout.body += b'\xe9' + struct.pack('<i', pick + len(MATCH_PICK) - (layout.here() + 5))
    edits.append((pick, MATCH_PICK, b'\xe9' + struct.pack('<i', pick_stub - (pick + 5)) + b'\x90' * 2))
    edits.append((loop, b'\x83\xf9\x0a', b'\x83\xf9' + bytes((DELIA + 1,))))
    return {'map': hex(delia_map), 'lookup': f'{lookup:#x} -> {lookup_stub:#x}',
            'pick': f'{pick:#x} -> {pick_stub:#x}', 'loop': f'{loop:#x} classes 0-{DELIA}'}


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
    """One TOOLTIPS stub: the Hagie block for flag 10 and #GameUI_Delia, then the join."""
    replaced = bytes.fromhex(old)
    if any(join - 3 <= r < join + len(replaced) for r in relocs):
        raise PreparationError(f'Relocated dword inside the join {join:#x}; nothing is patched.')
    layout.pad()
    name = layout.string('#GameUI_Delia')
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
    """The PRECACHE stub: PrecacheModel(path, 0) for each Delia motion model, then the entry."""
    layout.pad()
    stub = layout.here()
    for text in DELIA_MOTIONS:
        address = layout.strings[text]
        layout.body += b'\x6a\x00\x68'
        layout.pointer(address)
        layout.body += b'\xe8' + struct.pack('<i', precache_model - (layout.here() + 5)) + b'\x83\xc4\x08'
    replaced = bytes.fromhex(old)
    layout.body += replaced + b'\xe9' + struct.pack('<i', entry + len(replaced) - (layout.here() + len(replaced) + 5))
    edits.append((entry, replaced, b'\xe9' + struct.pack('<i', stub - (entry + 5))))
    return f'{entry:#x} -> {stub:#x} ({len(DELIA_MOTIONS)} models)'


def plan_dll(name: str, data: bytes) -> tuple[bytes, dict]:
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
    wanted = {}
    for va, bound, _ in SWITCHES.get(name, ()):
        _, default, table, _ = switch_parts(image, va, bound)
        case = struct.unpack_from('<I', image.data, image.offset(table + 4 * 9))[0]
        cases = {struct.unpack_from('<I', image.data, image.offset(table + 4 * i))[0] for i in range(bound + 1)}
        # The Hagie case ends at the next case, the default or the join the other cases jump to
        # (the last case usually falls through into it).
        first = min(cases)
        joins = {int(i.op_str, 16) for i in md.disasm(bytes(image.data[image.offset(first):image.offset(case)]), first)
                 if i.mnemonic == 'jmp' and re.fullmatch(r'0x[0-9a-f]+', i.op_str)}
        end = min(a for a in cases | joins | {default} if a > case)
        wanted[va] = (case, end, default, table)
    for text in sorted({delia(read_string(image, struct.unpack_from('<I', image.data, image.offset(a))[0]) or '')
                        for va, (case, end, _, _) in wanted.items()
                        for a in range(case, end - 3) if a in relocs} - {''}):
        if 'delia' in text.lower():
            layout.string(text)
    for _, text in TABLES.get(name, ()):
        if text:
            layout.string(text)
    layout.pad()

    for va, bound, pet in SWITCHES.get(name, ()):
        register, _, _, disp = switch_parts(image, va, bound)
        case, end, default, table = wanted[va]
        stub = layout.here()
        strings = clone_case(image, layout, case, end, relocs)
        layout.pad()
        entries = [struct.unpack_from('<I', image.data, image.offset(table + 4 * i))[0] for i in range(bound + 1)]
        count = PET + 1 if pet else DELIA + 1
        new_table = layout.here()
        for i in range(count):
            if i < DELIA:
                layout.pointer(entries[i])
            elif i == DELIA:
                layout.pointer(stub)
            elif pet and i == PET:
                layout.pointer(entries[10])
            else:
                layout.pointer(default)
        edits.append((va + 2, bytes((bound,)), bytes((count - 1,))))
        edits.append((disp, struct.pack('<I', table), struct.pack('<I', new_table)))
        report['switches'].append({'cmp': hex(va), 'register': register, 'bound': f'{bound} -> {count - 1}',
                                   'pet': PET if pet else None, 'delia_case': hex(stub), 'strings': strings})

    for va, text in TABLES.get(name, ()):
        entries = [struct.unpack_from('<I', image.data, image.offset(va + 4 * i))[0] for i in range(10)]
        new_table = layout.here()
        for i in range(TABLE_SIZE):
            value = entries[i] if i < DELIA else (layout.strings[text] if i == DELIA and text else
                                                  0 if i == DELIA else entries[0])
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
        report['tables'].append({'table': hex(va), 'moved_to': hex(new_table), 'delia': text, 'references': users})

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


def patch(client_copy: Path, apply: bool = False) -> dict:
    plan = {'mode': 'apply' if apply else 'dry-run', 'pet': PET, 'dlls': []}
    for name, prefix in EXPECTED.items():
        dll = reject_original(client_copy / 'bin' / name)
        data = dll.read_bytes()
        if not digest(data).startswith(prefix):
            raise PreparationError(f'{name}: expected the .hcls copy {prefix}…, found {digest(data)[:16]}.')
        image, report = plan_dll(name, data)
        backup = dll.with_name(f'{name}.{digest(data)[:16]}.pre-routing.bak')
        plan['dlls'].append({'dll': str(dll), 'before_sha256': digest(data), 'after_sha256': digest(image),
                             'backup': str(backup), **report})
        if apply:
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
        print(json.dumps(patch(args.client_copy, args.apply), indent=1))
    except PreparationError as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
