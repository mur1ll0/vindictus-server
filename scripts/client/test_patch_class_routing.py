import itertools
import pathlib
import re
import struct
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_class_routing as routing  # noqa: E402
from patch_class_table import Image  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'


def source(name):
    """The .hcls copy the patch starts from: the pre-routing backup once the lab copy is patched."""
    backup = LAB / f'{name}.{routing.EXPECTED[name]}.pre-routing.bak'
    return (backup if backup.is_file() else LAB / name).read_bytes()


class NamesTest(unittest.TestCase):
    def test_hagie_names_become_delia_names_in_the_same_case(self):
        self.assertEqual(routing.delia('scripts/expression/expression_table_hagie'),
                         'scripts/expression/expression_table_delia')
        self.assertEqual(routing.delia('Player.Hagie.'), 'Player.Delia.')
        self.assertEqual(routing.delia('csf/create_lethita.csf'), 'csf/create_lethita.csf')


@unittest.skipUnless((LAB / 'client.dll').is_file() and (LAB / 'server.dll').is_file(),
                     'needs the .hcls laboratory copy in .cache/play-client')
class LabCopyTest(unittest.TestCase):
    """Plans the patch in memory over the lab copy and reads the result back."""

    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)

    def plan(self, name):
        data = source(name)
        image, report = routing.plan_dll(name, data)
        return Image(image), report

    def entry(self, image, table, index):
        return struct.unpack_from('<I', image.data, image.offset(table + 4 * index))[0]

    def test_switches_send_ten_to_delia_and_keep_the_pet_at_28(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            for switch in report['switches']:
                cmp = int(switch['cmp'], 16)
                head = list(self.md.disasm(bytes(image.data[image.offset(cmp):image.offset(cmp) + 20]), cmp))[:3]
                table = int(re.search(r'\+ (0x[0-9a-f]+)\]', head[2].op_str).group(1), 16)
                bound = int(head[0].op_str.split(', ')[1], 16)
                self.assertEqual(bound, routing.PET if switch['pet'] else routing.DELIA)
                self.assertEqual(self.entry(image, table, routing.DELIA), int(switch['delia_case'], 16))
                case = int(switch['delia_case'], 16)
                strings = [routing.read_string(image, int(v, 16))
                           for x in self.md.disasm(bytes(image.data[image.offset(case):image.offset(case) + 32]), case)
                           for v in re.findall(r'0x[0-9a-f]{8}', x.op_str)]
                self.assertTrue(any(s and 'delia' in s.lower() for s in strings), switch)
                self.assertFalse(any(s and 'hagie' in s.lower() for s in strings), switch)
                if switch['pet']:
                    original = Image(source(name))
                    old = int(re.search(r'\+ (0x[0-9a-f]+)\]', list(self.md.disasm(
                        bytes(original.data[original.offset(cmp):original.offset(cmp) + 20]), cmp))[2].op_str).group(1), 16)
                    self.assertEqual(self.entry(image, table, routing.PET), self.entry(original, old, 10))

    def test_server_tables_have_delia_at_ten_and_no_null_after_her(self):
        image, report = self.plan('server.dll')
        for table in report['tables']:
            moved = int(table['moved_to'], 16)
            ten = self.entry(image, moved, routing.DELIA)
            self.assertEqual(routing.read_string(image, ten) if ten else None, table['delia'])
            self.assertTrue(all(self.entry(image, moved, i) == self.entry(image, moved, 0) for i in range(11, 32)))
        self.assertEqual(bytes(image.data[image.offset(0x405368C5):image.offset(0x405368C5) + 2]), b'\x6a\x1c')

    def test_local_player_setup_accepts_classes_below_the_pet(self):
        image, report = self.plan('client.dll')
        self.assertEqual(report['gates'], ['0x387033d7'])
        self.assertEqual(bytes(image.data[image.offset(0x387033D7):image.offset(0x387033DA)]), b'\x83\xf8\x1c')

    def test_skill_match_table_sends_delia_to_an_eleventh_map(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            lookup, pick, loop, less = routing.MATCH[name]
            delia_map = int(report['match_table']['map'], 16)
            self.assertEqual(struct.unpack_from('<IIIIHHHHI', image.data, image.offset(delia_map)),
                             (less, 0, 0, 0, 0xFFFF, 0, 0xFFFF, 0, 0))
            self.assertEqual(bytes(image.data[image.offset(loop):image.offset(loop) + 3]), b'\x83\xf9\x0b')
            for va, size, register in ((lookup, len(routing.MATCH_LOOKUP), 'edi'), (pick, len(routing.MATCH_PICK), 'ebp')):
                code = self.stub_code(image, va)
                text = [f'{x.mnemonic} {x.op_str}' for x in code]
                self.assertIn(f'mov {register}, {delia_map:#x}', text)
                if register == 'ebp':
                    self.assertIn(f'mov dword ptr [esp + 0x10], {delia_map + 4:#x}', text)
                self.assertEqual(code[-1].mnemonic, 'jmp')
                self.assertEqual(int(code[-1].op_str, 16), va + size)

    def test_class_flag_checks_admit_class_ten(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            self.assertEqual(len(report['flag_checks']), len(routing.FLAG_CHECKS[name]))
            for va, _, register, base, default, pop in routing.FLAG_CHECKS[name]:
                jump = next(self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + 5]), va))
                stub = int(jump.op_str, 16)
                code = list(itertools.takewhile(lambda x: x.mnemonic != 'int3', self.md.disasm(
                    bytes(image.data[image.offset(stub):image.offset(stub) + 0x40]), stub)))
                text = [f'{x.mnemonic} {x.op_str}' for x in code]
                self.assertEqual(text[0], f'cmp {register}, 0xa')
                self.assertEqual(text[1], f'ja {default:#x}')
                item = 'ecx' if base == 'ecx' else 'eax'
                self.assertIn(f'mov al, byte ptr [{item} + {register} + 0x198]', text)
                self.assertIn(f'mov al, byte ptr [{item} + {routing.DELIA_FLAG:#x}]', text)
                self.assertEqual(text.count(f'ret {pop}'), 2)
                self.assertEqual(text[-1], f'ret {pop}')

    def stub_code(self, image, va):
        jump = next(self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + 5]), va))
        stub = int(jump.op_str, 16)
        return list(itertools.takewhile(lambda x: x.mnemonic != 'int3', self.md.disasm(
            bytes(image.data[image.offset(stub):image.offset(stub) + 0x80]), stub)))

    def test_tooltips_name_delia_from_flag_ten(self):
        image, report = self.plan('client.dll')
        self.assertEqual(len(report['tooltips']), len(routing.TOOLTIPS['client.dll']))
        for _, _, join, old in routing.TOOLTIPS['client.dll']:
            code = self.stub_code(image, join)
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            self.assertEqual(sum(f'{routing.DELIA_FLAG:#x}]' in t for t in text), 2)
            self.assertFalse(any('0x1a1]' in t for t in text))
            pushed = [int(x.op_str, 16) for x in code if x.mnemonic == 'push' and x.op_str.startswith('0x')]
            self.assertEqual([routing.read_string(image, v) for v in pushed], ['#GameUI_Delia'])
            self.assertEqual(int(code[-1].op_str, 16), join + len(bytes.fromhex(old)))

    def test_precache_loads_delia_motion_models_first(self):
        image, report = self.plan('server.dll')
        entry, old, precache_model = routing.PRECACHE['server.dll']
        code = self.stub_code(image, entry)
        pushed = [routing.read_string(image, int(x.op_str, 16)) for x in code
                  if x.mnemonic == 'push' and x.op_str.startswith('0x') and int(x.op_str, 16) > 0xFFFF]
        self.assertEqual(pushed, routing.DELIA_PRECACHE)
        self.assertEqual(sum(f'call {precache_model:#x}' == f'{x.mnemonic} {x.op_str}' for x in code), len(routing.DELIA_PRECACHE))
        self.assertEqual(int(code[-1].op_str, 16), entry + len(bytes.fromhex(old)))

    def test_costume_sends_class_ten_to_the_shadow_column(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            bound, block, after, accessors = routing.COSTUME[name]
            shadow = int(report['costume']['shadow'], 16)
            o = image.offset(shadow)
            self.assertEqual(struct.unpack_from('<i', image.data, o + 0xF0)[0], -1)
            self.assertEqual(struct.unpack_from('<i', image.data, o + 0x2D0 + 11 * 40)[0], -1)
            self.assertEqual(struct.unpack_from('<i', image.data, o + 0xBB8)[0], -1)
            self.assertEqual(bytes(image.data[image.offset(bound):image.offset(bound) + 3]), b'\x83\xfe\x0a')
            text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, block)]
            self.assertIn(f'mov dword ptr [esp + 0x14], {shadow:#x}', text)
            self.assertEqual(text[-1], f'jmp {block + len(routing.COSTUME_BLOCK):#x}')
            text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, after)]
            self.assertIn(f'cmp dword ptr [esp + 0x14], {shadow:#x}', text)
            self.assertEqual(text[-1], f'jmp {after + 7:#x}')
            for va, old, at in accessors:
                text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, va)]
                self.assertEqual(text[0], f'cmp dword ptr [esp + {at:#x}], 0xa' if at > 9 else f'cmp dword ptr [esp + {at}], 0xa')
                self.assertIn(f'mov ecx, {shadow:#x}', text)
                self.assertEqual(text[-1], f'jmp {va + len(bytes.fromhex(old)):#x}')

    def test_event_file_reads_delias_shadow_paths(self):
        image, report = self.plan('server.dll')
        va, old, extreme, resume, skip, keep = routing.EVENT_FILE['server.dll']
        shadow = int(report['costume']['shadow'], 16)
        text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, va)]
        self.assertEqual(text[0], 'cmp esi, 0xa')
        self.assertIn(f'mov ecx, {shadow + 0xC80:#x}', text)
        self.assertIn(f'mov ecx, {shadow + 0xBE0:#x}', text)
        self.assertIn(f'jmp {resume:#x}', text)
        self.assertEqual(text[-3:], ['cmp esi, 9', f'ja {skip:#x}', f'jmp {keep:#x}'])

    def test_server_costume_build_admits_class_ten(self):
        image, report = self.plan('server.dll')
        va = routing.COSTUME_GATE['server.dll']
        self.assertEqual(bytes(image.data[image.offset(va):image.offset(va) + 3]), b'\x83\xf8\x0a')

    def test_class_name_lookup_knows_delia(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            entry, old, compare = routing.CLASS_NAME[name]
            code = self.stub_code(image, entry)
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            pushed = [routing.read_string(image, int(x.op_str, 16)) for x in code
                      if x.mnemonic == 'push' and x.op_str.startswith('0x')]
            self.assertEqual(pushed, ['delia'])
            self.assertIn(f'call {compare:#x}', text)
            self.assertIn('mov eax, 0xa', text)
            self.assertEqual(text[-1], f'jmp {entry + 5:#x}')

    def test_fallback_entity_is_opt_in(self):
        data = source('server.dll')
        _, plain = routing.plan_dll('server.dll', data)
        self.assertNotIn('fallback_entity', plain)
        image, report = routing.plan_dll('server.dll', data, 'player_fiona')
        image = Image(image)
        push = next(self.md.disasm(bytes(image.data[image.offset(routing.FALLBACK_PUSH):
                                                     image.offset(routing.FALLBACK_PUSH) + 5]), routing.FALLBACK_PUSH))
        self.assertEqual(routing.read_string(image, int(push.op_str, 16)), 'player_fiona')

    def test_player_entity_slots_dispatch_on_the_class(self):
        image, report = self.plan('client.dll')
        name, klass, figure_slot, constant_slots, gate, _ = routing.ENTITY_SLOTS['client.dll']
        table = routing.vtable(image, name)
        original = Image(source('client.dll'))
        for slot in (figure_slot, *constant_slots):
            stub = self.entry(image, table, slot)
            code = list(itertools.takewhile(lambda x: x.mnemonic != 'int3', self.md.disasm(
                bytes(image.data[image.offset(stub):image.offset(stub) + 0x40]), stub)))
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            self.assertEqual(text[:2], [f'mov eax, dword ptr [ecx + {klass:#x}]', 'cmp eax, 0xa'])
            self.assertEqual(text[-1], f'jmp {self.entry(original, table, slot):#x}')
            if slot == figure_slot:
                self.assertIn(f'call {gate:#x}', text)
                loaded = [int(x.op_str.split(', ')[1], 16) for x in code if x.mnemonic == 'mov' and x.op_str.startswith('eax, 0x')]
                self.assertEqual([routing.read_string(image, v) for v in loaded], ['csf/bodyshape_delia_default.fig'])
            else:
                address = int(re.search(r'\[(0x[0-9a-f]+)\]', next(t for t in text if t.startswith('fld'))).group(1), 16)
                value = struct.unpack_from('<f', image.data, image.offset(address))[0]
                self.assertAlmostEqual(value, routing.EXTRA_CLASSES[0].constants[constant_slots.index(slot)], places=5)

    def test_a_second_extra_class_gets_its_own_entries(self):
        miri = routing.ExtraClass(11, 'miri', 'hagie', 'female', 'csf/bodyshape_miri_default.fig', (0.9, 1.0), 3)
        with mock.patch.multiple(routing, EXTRA_CLASSES=routing.EXTRA_CLASSES + (miri,), LAST=11):
            for name in ('client.dll', 'server.dll'):
                image, report = self.plan(name)
                for switch in report['switches']:
                    case = int(switch['cases'][11], 16)
                    self.assertIn('miri', ' '.join(s.lower() for s in switch['strings']))
                    cmp = int(switch['cmp'], 16)
                    head = list(self.md.disasm(bytes(image.data[image.offset(cmp):image.offset(cmp) + 20]), cmp))[:3]
                    table = int(re.search(r'\+ (0x[0-9a-f]+)\]', head[2].op_str).group(1), 16)
                    self.assertEqual(self.entry(image, table, 11), case)
                self.assertEqual(len(report['match_table']['maps']), 2)
                self.assertEqual(len(report['costume']['shadows']), 2)
                entry, _, _ = routing.CLASS_NAME[name]
                code = self.stub_code(image, entry)
                pushed = [routing.read_string(image, int(x.op_str, 16)) for x in code
                          if x.mnemonic == 'push' and x.op_str.startswith('0x')]
                self.assertEqual(pushed, ['delia', 'miri'])
                if name == 'server.dll':
                    moved = int(report['tables'][0]['moved_to'], 16)
                    self.assertEqual(routing.read_string(image, self.entry(image, moved, 11)), 'csf/create_miri.csf')
                    self.assertEqual(bytes(image.data[image.offset(routing.COSTUME_GATE[name]):][:3]), b'\x83\xf8\x0b')

    def test_skill_class_checks_admit_the_extra_classes(self):
        image, report = self.plan('client.dll')
        for va in routing.CLASS_BITS['client.dll']:
            head = list(self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + 16]), va))[:4]
            self.assertEqual(head[0].mnemonic, 'cmp')
            self.assertTrue(head[0].op_str.endswith(f', {routing.LAST:#x}'), head[0].op_str)
            self.assertEqual(head[1].mnemonic, 'ja')
            code = list(self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + 32]), va))[:8]
            self.assertTrue(any(f'{x.mnemonic} {x.op_str}' == 'shl eax, cl' for x in code))

    def test_class_name_accessors_name_delia(self):
        image, report = self.plan('client.dll')
        for bound, expected in zip(routing.CLASS_STRING_BOUNDS['client.dll'], ('Delia', 'D')):
            code = list(self.md.disasm(bytes(image.data[image.offset(bound - 4):image.offset(bound - 4) + 16]), bound - 4))
            self.assertEqual(f'{code[1].mnemonic} {code[1].op_str}', f'cmp eax, {routing.LAST:#x}')
            table = int(re.search(r'\+ (0x[0-9a-f]+)\]', code[3].op_str).group(1), 16)
            self.assertEqual(routing.read_string(image, self.entry(image, table, 9)), 'Hagie' if expected == 'Delia' else 'H')
            self.assertEqual(routing.read_string(image, self.entry(image, table, routing.DELIA)), expected)

    def test_body_shape_borrows_the_shape_class(self):
        image, report = self.plan('client.dll')
        for va, old, register in routing.SHAPE_CLASS['client.dll']:
            code = self.stub_code(image, va)
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            shape = routing.OLD_CLASSES.index(routing.EXTRA_CLASSES[0].shape_of)
            self.assertEqual(text[:3][0], f'cmp {register}, 0xa')
            self.assertIn(f'mov {register}, {shape}', text)
            self.assertIn(f'cmp {register}, 9', text)
            self.assertEqual(text[-1], f'jmp {va + len(bytes.fromhex(old)):#x}')

    def test_costume_default_tables_read_the_clone_column(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            clone = routing.OLD_CLASSES.index(routing.EXTRA_CLASSES[0].clone_of)
            for va in routing.CLASS_DEFAULTS[name]:
                text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, va)]
                self.assertEqual(text[2:5], ['cmp edx, 0xa', text[3], f'mov edx, {clone}'])
                self.assertEqual(text[-2:], ['mov eax, dword ptr [ecx + eax*4]', 'ret 8'])

    def test_class_weapon_lists_give_extra_classes_their_row(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            table = routing.CLASS_WEAPONS[name]
            row = int(report['class_weapons']['rows'][routing.DELIA], 16)
            items, count, grow, size, elements = struct.unpack_from('<IiiiI', image.data, image.offset(row))
            self.assertEqual((count, grow, size, elements), (5, -1, 5, items))
            listed = [routing.read_string(image, struct.unpack_from('<I', image.data, image.offset(items + 4 * i))[0])
                      for i in range(5)]
            weapons = list(routing.EXTRA_CLASSES[0].weapons)
            self.assertEqual(listed[:len(weapons)], weapons)
            self.assertTrue(all(image.data[image.offset(struct.unpack_from('<I', image.data, image.offset(items + 4 * i))[0])] == 0
                                for i in range(len(weapons), 5)))
            self.assertEqual(len(report['class_weapons']['readers']), 5)
            for entry in report['class_weapons']['readers']:
                va = int(entry.split(' -> ')[0], 16)
                text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, va)]
                self.assertTrue(text[0].endswith(', 0xa'))
                moved = [int(t.split(', ')[1], 16) for t in text if t.startswith('mov ') and t.split(', ')[1].startswith('0x')]
                self.assertEqual((moved[0] * 4 + table) & 0xFFFFFFFF, row)
                self.assertEqual(text[-1], f'jmp {va + 10:#x}')

    def test_class_alias_loads_extra_classes_as_their_clone(self):
        image, report = self.plan('server.dll')
        clone = routing.OLD_CLASSES.index(routing.EXTRA_CLASSES[0].clone_of)
        for va, old in routing.CLASS_ALIAS['server.dll']:
            text = [f'{x.mnemonic} {x.op_str}' for x in self.stub_code(image, va)]
            self.assertEqual(text[0], 'mov eax, dword ptr [esi + 0x1400]')
            self.assertIn(f'mov eax, {clone}', text)
            self.assertEqual(text[-1], f'jmp {va + len(bytes.fromhex(old)):#x}')

    def test_client_spawn_passes_the_extra_class_weapon(self):
        image, report = self.plan('client.dll')
        va, old, join = routing.CLIENT_WEAPON_SWITCH['client.dll']
        code = self.stub_code(image, va)
        text = [f'{x.mnemonic} {x.op_str}' for x in code]
        self.assertEqual(text[:3], ['cmp eax, 0xa', text[1], 'push ebx'])
        pushed = int(text[3].split(' ')[1], 16)
        self.assertEqual(routing.read_string(image, pushed), routing.EXTRA_CLASSES[0].weapons[0])
        self.assertEqual(text[4], f'jmp {join:#x}')
        self.assertEqual(text[-1], f'jmp {va + 5:#x}')

    def test_delia_conditions_get_ids_after_the_169_list(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            tail, name_map, name_insert, id_map, id_insert, bounds = routing.CONDITIONS[name]
            for va in bounds:
                self.assertEqual(bytes(image.data[image.offset(va):image.offset(va) + 3]), bytes((0x83, 0xF9, 126)))
            jump = next(self.md.disasm(bytes(image.data[image.offset(tail):image.offset(tail) + 5]), tail))
            stub = int(jump.op_str, 16)
            code = list(itertools.takewhile(lambda x: x.mnemonic != 'int3', self.md.disasm(
                bytes(image.data[image.offset(stub):image.offset(stub) + 0x400]), stub)))
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            # the stub ends with its jump back; what follows may be data, not int3 padding
            text = text[:text.index(f'jmp {tail + 6:#x}') + 1]
            self.assertEqual(text[:2], ['pushal ', 'sub esp, 8'])
            self.assertEqual(sum(t == f'call {name_insert:#x}' for t in text), 5)
            self.assertEqual(sum(t == f'call {id_insert:#x}' for t in text), 5)
            names = [routing.read_string(image, int(t.split(', ')[1], 16)) for t in text
                     if t.startswith('mov dword ptr [esp') and int(t.split(', ')[1], 16) > 0x10000]
            self.assertEqual(names[0::2], list(routing.CONDITION_NAMES))
            self.assertEqual(text[-7:-1], ['add esp, 8', 'popal ', 'pop edi', 'mov al, 1', 'pop esi', 'mov esp, ebp'])
            self.assertEqual(text[-1], f'jmp {tail + 6:#x}')

    def test_server_player_constants_dispatch_on_the_class(self):
        image, report = self.plan('server.dll')
        name, klass, _, slots, _, values = routing.ENTITY_SLOTS['server.dll']
        table = routing.vtable(image, name)
        original = Image(source('server.dll'))
        for i, slot in enumerate(slots):
            stub = self.entry(image, table, slot)
            code = list(itertools.takewhile(lambda x: x.mnemonic != 'int3', self.md.disasm(
                bytes(image.data[image.offset(stub):image.offset(stub) + 0x40]), stub)))
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            self.assertEqual(text[:2], [f'mov eax, dword ptr [ecx + {klass:#x}]', 'cmp eax, 0xa'])
            self.assertEqual(text[-1], f'jmp {self.entry(original, table, slot):#x}')
            address = int(re.search(r'\[(0x[0-9a-f]+)\]', next(t for t in text if t.startswith('fld'))).group(1), 16)
            value = struct.unpack_from('<f', image.data, image.offset(address))[0]
            self.assertAlmostEqual(value, getattr(routing.EXTRA_CLASSES[0], values)[i], places=5)

    def test_guard_keeps_state_and_blocks_for_extra_classes(self):
        image, report = self.plan('server.dll')
        set_slot, get_slot, damage_slot, guarded, forced_off, conditions_of, raise_, lower = routing.GUARD['server.dll']
        table = routing.vtable(image, routing.ENTITY_SLOTS['server.dll'][0])
        guard = report['guard']
        bodies = {set_slot: int(guard['set'], 16), get_slot: int(guard['get'], 16), damage_slot: int(guard['damage'], 16)}
        for slot, body in bodies.items():
            stub = self.entry(image, table, slot)
            text = [f'{x.mnemonic} {x.op_str}' for x in self.md.disasm(bytes(image.data[image.offset(stub):image.offset(stub) + 24]), stub)][:4]
            self.assertEqual(text[0], 'mov eax, dword ptr [ecx + 0x1400]')
            self.assertEqual(text[3], f'jmp {body:#x}')
        def body(va, size):
            return [f'{x.mnemonic} {x.op_str}' for x in itertools.takewhile(
                lambda x: x.mnemonic != 'int3', self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + size]), va))]
        put = body(bodies[set_slot], 0x100)
        self.assertIn(f'call {forced_off:#x}', put)
        self.assertEqual(sum(t == f'call {lower:#x}' for t in put), 2)
        damage = body(bodies[damage_slot], 0xa0)
        self.assertEqual(sum(t == f'call {raise_:#x}' for t in damage), 2)
        self.assertIn('push 0x7c', damage)
        self.assertIn('push 0x7d', damage)
        self.assertIn('test byte ptr [eax + 0x78], 2', damage)  # a client hit counts when the client guarded it
        self.assertIn(f'call {guarded:#x}', damage)
        self.assertEqual(damage[-1], 'ret 4')

    def test_client_guard_marks_guarded_hits_for_extra_classes(self):
        image, report = self.plan('client.dll')
        (set_slot, get_slot, damage_slot, copy, unguardable, unguardable_info,
         raise_, test, lower, allocator) = routing.CLIENT_GUARD['client.dll']
        name = routing.ENTITY_SLOTS['client.dll'][0]
        original = Image(source('client.dll'))
        table = routing.vtable(image, name)
        guard = report['guard']
        bodies = {set_slot: int(guard['set'], 16), get_slot: int(guard['get'], 16), damage_slot: int(guard['damage'], 16)}
        for slot, target in bodies.items():
            stub = self.entry(image, table, slot)
            text = [f'{x.mnemonic} {x.op_str}' for x in self.md.disasm(bytes(image.data[image.offset(stub):image.offset(stub) + 32]), stub)][:5]
            self.assertEqual(text[0], 'mov eax, dword ptr [ecx + 0x18ec]')
            self.assertEqual(text[3], f'jmp {target:#x}')
            self.assertEqual(text[4], f'jmp {self.entry(original, routing.vtable(original, name), slot):#x}')
        def body(va, size):
            return [f'{x.mnemonic} {x.op_str}' for x in itertools.takewhile(
                lambda x: x.mnemonic != 'int3', self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + size]), va))]
        put = body(bodies[set_slot], 0x100)
        self.assertEqual(sum(t == f'call {lower:#x}' for t in put), 2)
        self.assertIn('movzx edx, byte ptr [esp + 4]', put)
        damage = body(bodies[damage_slot], 0x140)
        for target in (copy, unguardable, unguardable_info, test, int(guard['base'], 16)):
            self.assertIn(f'call {target:#x}', damage)
        self.assertEqual(sum(t == f'call {raise_:#x}' for t in damage), 3)
        for text in ('push 0x7c', 'push 0x7d', 'push 0x4c', 'push 0x4d', 'or dword ptr [esp + 0x84], 2',
                     f'mov byte ptr [esi + {routing.HIT_IMPACT_SKIP:#x}], 1',
                     f'mov ecx, dword ptr [{allocator:#x}]'):
            self.assertIn(text, damage)
        self.assertEqual(int(guard['base'], 16), self.entry(original, routing.vtable(original, name), damage_slot))
        self.assertEqual(damage[-1], 'ret 4')

    def test_think_raises_armor_purge_for_extra_classes(self):
        purge = routing.FIRST_NEW_CONDITION + routing.CONDITION_NAMES.index('As_Cond_Can_Armor_Purge')
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            think, conditions_of, raise_ = routing.CLASS_CONDITIONS[name]
            entity = routing.ENTITY_SLOTS[name][0]
            original = Image(source(name))
            base = self.entry(original, routing.vtable(original, entity), think)
            stub = self.entry(image, routing.vtable(image, entity), think)
            body = int(report['class_conditions']['think'], 16)
            head = [f'{x.mnemonic} {x.op_str}' for x in self.md.disasm(bytes(image.data[image.offset(stub):image.offset(stub) + 32]), stub)][:5]
            self.assertEqual(head[3], f'jmp {body:#x}')
            self.assertEqual(head[4], f'jmp {base:#x}')
            text = [f'{x.mnemonic} {x.op_str}' for x in itertools.takewhile(
                lambda x: x.mnemonic != 'int3', self.md.disasm(bytes(image.data[image.offset(body):image.offset(body) + 0x40]), body))]
            self.assertEqual(text[2], f'call {base:#x}')
            self.assertIn(f'push {purge:#x}', text)
            self.assertIn(f'call {raise_:#x}', text)
            self.assertEqual(text[-1].strip(), 'ret')

    def test_delia_effects_register_types_commands_and_hooks(self):
        image, report = self.plan('server.dll')
        k = routing.DELIA_EFFECTS['server.dll']
        effects = report['delia_effects']

        def text(va, size):
            return [f'{x.mnemonic} {x.op_str}' for x in itertools.takewhile(
                lambda x: x.mnemonic != 'int3', self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + size]), va))]
        hook = next(self.md.disasm(bytes(image.data[image.offset(k['hook']):image.offset(k['hook']) + 5]), k['hook']))
        self.assertEqual(f'{hook.mnemonic} {hook.op_str}', f'jmp {effects["register"]}')
        register = text(int(effects['register'], 16), 0x80)
        self.assertEqual(sum(t == f'call {k["register"]:#x}' for t in register), 5)
        names = [routing.read_string(image, int(t.split()[1], 16)) for t in register if t.startswith('push 0x')]
        for name in ('stamina_adder_on_attack_time', 'delia_restore_health', 'skill_delia_strong_attack',
                     'damage_modify_to_action', 'restore_equipment_def'):
            self.assertIn(name, names)
        for name, factory in effects['factories'].items():
            body = text(int(factory, 16), 0x40)
            self.assertIn(f'call {k["base_ctor"]:#x}', body)
            vt = int(next(t for t in body if t.startswith('mov dword ptr [esi], ')).split(', ')[1], 16)
            slots = [self.entry(image, vt, i) for i in range(7)]
            self.assertEqual(slots[0], k['dtor'])
            relocs = routing.relocations(image)
            self.assertTrue(all(vt + 4 * i in relocs for i in range(7)), name)
        head = next(self.md.disasm(bytes(image.data[image.offset(k['events']):image.offset(k['events']) + 5]), k['events']))
        self.assertEqual(f'{head.mnemonic} {head.op_str}', f'jmp {effects["events"]}')
        events = text(int(effects['events'], 16), 0x300)
        self.assertIn(f'jmp {k["events"] + len(routing.EVENTS_HEAD):#x}', events)
        self.assertEqual(sum(t == f'call {k["strncmp"]:#x}' for t in events), 5)
        commands = [routing.read_string(image, int(t.split()[1], 16)) for t in events if t.startswith('push 0x')]
        self.assertIn('PURGE_ALL_ARMOR', commands)
        # PURGE_ALL_ARMOR: slots 4-8 with armour get HP 0 and the broken state without notify
        # (the notify path rebuilds the costume model)
        at = events.index('mov esi, 4')
        worn = k['clothes'] + k['worn_part']
        self.assertEqual(events[at + 1:at + 12], [
            'mov ecx, dword ptr [edi + 0x148c]', 'mov ecx, dword ptr [ecx + 4]', 'push esi', f'call {k["armor_none"]:#x}',
            'test al, al', events[at + 6], 'mov ecx, dword ptr [edi + 0x148c]', 'mov ecx, dword ptr [ecx + 4]', 'push 0',
            'push esi', f'call {k["armor_hp_set"]:#x}'])
        self.assertTrue(events[at + 6].startswith('jne '))
        # the worn part leaves the model: kept (+1) in the entry, network change, then -1
        kept = routing.DELIA_KEPT_PART
        self.assertEqual(events[at + 12], f'cmp dword ptr [edi + esi*4 + {worn:#x}], -1')
        self.assertEqual(events[at + 14:at + 23], [
            f'mov eax, dword ptr [edi + esi*4 + {worn:#x}]', 'inc eax', f'mov dword ptr [ebx + esi*4 + {kept:#x}], eax',
            'lea ecx, [edi + 0x156c]', f'lea edx, [edi + esi*4 + {worn:#x}]', 'push edx', 'mov eax, dword ptr [ecx]',
            'call dword ptr [eax + 4]', f'mov dword ptr [edi + esi*4 + {worn:#x}], 0xffffffff'])
        self.assertEqual(events[at + 23:at + 28], ['push 0', 'push 1', 'push esi', 'mov ecx, edi',
                                                   f'call {k["armor_state"]:#x}'])
        self.assertTrue(events[at + 28].startswith('jmp '))
        # no armour on the slot: with the armour system off (the ship) only the look is hidden
        self.assertEqual(events[at + 29:at + 32], [f'mov ecx, dword ptr [{k["game_state"]:#x}]',
                                                   f'call {k["armor_on"]:#x}', 'test al, al'])
        self.assertTrue(events[at + 32].startswith('jne '))
        self.assertEqual(events[at + 33], f'cmp dword ptr [edi + esi*4 + {worn:#x}], -1')
        self.assertEqual(events[at + 44:at + 46], ['inc esi', 'cmp esi, 9'])
        self.assertEqual(text(0x40814620, 0x10)[2:4], [f'mov ecx, dword ptr [{k["game_state"]:#x}]',
                                                      f'call {k["armor_on"]:#x}'])
        self.assertEqual(events[at - 3:at - 1], ['mov ecx, edi', events[at - 2]])
        self.assertEqual(events[at - 1], 'mov ebx, eax')
        # the costume object: +0x94 is its worn part per slot (0x408154b0 reads it)
        self.assertEqual(text(0x408154B0, 0x14)[3], 'mov eax, dword ptr [ecx + eax*4 + 0x94]')
        self.assertIn(f'call {k["armor_update"]:#x}', events)
        self.assertNotIn('call 0x40530ce0', events)
        self.assertNotIn(f'call {k["costume"]:#x}', events)
        at = events.index(f'call {k["armor_update"]:#x}')
        self.assertEqual(events[at + 1], 'mov byte ptr [ebx + 0x18], 1')
        self.assertEqual(events[at - 3:at - 1], ['lea ecx, [edi + 0x156c]', f'call {k["costume_parts"]:#x}'])
        # a repair (armour state with broken 0) puts a kept part back and asks for a rebuild
        head = next(self.md.disasm(bytes(image.data[image.offset(k['armor_state']):image.offset(k['armor_state']) + 5]),
                                   k['armor_state']))
        self.assertEqual(f'{head.mnemonic} {head.op_str}', f'jmp {effects["restore"]}')
        restore = text(int(effects['restore'], 16), 0xa0)
        self.assertEqual(restore[0], 'cmp byte ptr [esp + 8], 0')
        self.assertIn(f'mov edi, dword ptr [ebx + esi*4 + {kept:#x}]', restore)
        self.assertIn(f'mov dword ptr [ebx + esi*4 + {kept:#x}], 0', restore)
        self.assertIn(f'cmp dword ptr [ecx + esi*4 + {worn:#x}], -1', restore)
        self.assertIn(f'mov dword ptr [ecx + esi*4 + {worn:#x}], edi', restore)
        self.assertIn('mov byte ptr [ebx + 0x18], 1', restore)
        self.assertEqual(restore[-4:-1], ['mov eax, dword ptr [esp + 8]', 'push esi', 'mov esi, ecx'])
        self.assertEqual(restore[-1], f'jmp {k["armor_state"] + 7:#x}')
        # the player's repair: with the armour system off the kept parts come back (look only)
        head = next(self.md.disasm(bytes(image.data[image.offset(k['repair']):image.offset(k['repair']) + 5]), k['repair']))
        self.assertEqual(f'{head.mnemonic} {head.op_str}', f'jmp {effects["repair"]}')
        repair = text(int(effects['repair'], 16), 0xc0)
        self.assertIn(f'call {k["armor_on"]:#x}', repair)
        self.assertIn(f'mov dword ptr [ecx + esi*4 + {worn:#x}], edi', repair)
        self.assertIn('mov byte ptr [ebx + 0x18], 1', repair)
        at = repair.index(f'call {k["costume_parts"]:#x}')
        self.assertEqual(repair[at - 2:at], ['push ecx', 'lea ecx, [ecx + 0x156c]'])
        self.assertEqual(repair[at + 1], 'pop ecx')
        self.assertEqual(repair[-3:], ['fld dword ptr [esp + 4]', 'push ecx', f'jmp {k["repair"] + 5:#x}'])
        relocs = routing.relocations(image)
        at = next(x for x in self.md.disasm(bytes(image.data[image.offset(int(effects['repair'], 16)):
                                                          image.offset(int(effects['repair'], 16)) + 0x40]),
                                            int(effects['repair'], 16)) if x.mnemonic == 'mov' and 'ecx, dword ptr [0x' in
                  f'{x.mnemonic} {x.op_str}')
        self.assertIn(at.address + 2, relocs)
        # the think rebuilds the costume once the animation events are done
        tick = text(int(effects['tick'], 16), 0x60)
        at = tick.index('cmp byte ptr [esi + 0x18], 0')
        self.assertEqual(tick[at + 2:at + 5], ['mov byte ptr [esi + 0x18], 0', 'mov ecx, edi', f'call {k["costume"]:#x}'])
        state = text(k['armor_state'], 0x30)
        self.assertIn('call 0x405465d0', state)  # the rebuild, skipped when the third argument is 0
        self.assertIn('cmp byte ptr [esp + 0x10], 0', state)
        entity = routing.ENTITY_SLOTS['server.dll'][0]
        stub = self.entry(image, routing.vtable(image, entity), k['hit_slot'])
        head = [f'{x.mnemonic} {x.op_str}' for x in self.md.disasm(bytes(image.data[image.offset(stub):image.offset(stub) + 32]), stub)][:4]
        self.assertEqual(head[3], f'jmp {effects["on_hit"]}')
        think = text(int(report['class_conditions']['think'], 16), 0x40)
        self.assertIn(f'call {effects["tick"]}', think)
        damage = text(int(report['guard']['damage'], 16), 0xc0)
        self.assertIn(f'call {effects["forfeit"]}', damage)

    def test_item_mask_carries_bit_ten(self):
        image, report = self.plan('client.dll')
        va, old = routing.ITEM_MASK['client.dll']
        stub = int(report['item_mask'].split(' -> ')[1], 16)
        head = next(self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + 5]), va))
        self.assertEqual(f'{head.mnemonic} {head.op_str}', f'jmp {stub:#x}')
        text = [f'{x.mnemonic} {x.op_str}' for x in self.md.disasm(bytes(image.data[image.offset(stub):image.offset(stub) + 0x20]), stub)]
        self.assertIn(f'cmp byte ptr [ecx + {routing.DELIA_FLAG:#x}], 0', text)
        self.assertIn('add eax, 0x400', text)

    def test_immediately_frame_key_is_frame_zero(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            va, strcmp = routing.FRAME_KEYS[name]
            stub = int(report['frame_keys'].split(' -> ')[1], 16)
            head = next(self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + 5]), va))
            self.assertEqual(f'{head.mnemonic} {head.op_str}', f'jmp {stub:#x}')
            code = list(self.md.disasm(bytes(image.data[image.offset(stub):image.offset(stub) + 0x30]), stub))
            key = int(code[0].op_str, 16)
            self.assertEqual(routing.read_string(image, key), '$immediately$')
            text = [f'{x.mnemonic} {x.op_str}' for x in code]
            self.assertIn(f'call {strcmp:#x}', text)
            self.assertIn('xor esi, esi', text)
            self.assertIn(f'jmp {va + routing.FRAME_KEY_DONE:#x}', text)
            self.assertIn('mov ecx, dword ptr [esp + 0xa0]', text)
            self.assertIn(f'jmp {va + 7:#x}', text)

    def test_item_copy_carries_delia_flag(self):
        image, report = self.plan('client.dll')
        va, _, new = routing.ITEM_COPY['client.dll']
        code = self.md.disasm(bytes(image.data[image.offset(va):image.offset(va) + len(new)]), va)
        self.assertEqual([f'{x.mnemonic} {x.op_str}'.strip() for x in code],
                         ['mov ecx, dword ptr [edi + 0x1cc]', 'nop', 'mov dword ptr [esi + 0x1cc], ecx'])

    def test_extra_classes_must_be_contiguous(self):
        miri = routing.ExtraClass(12, 'miri', 'hagie', 'female', 'x.fig', (1.0, 1.0), 3)
        with self.assertRaises(routing.PreparationError):
            routing.check_classes(routing.EXTRA_CLASSES + (miri,))

    def test_every_new_pointer_is_relocated(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            relocs = routing.relocations(image)
            section = int(report['section']['va'], 16)
            inside = [va for va in relocs if va >= section]
            self.assertEqual(len(inside), report['section']['pointers'])


if __name__ == '__main__':
    unittest.main()
