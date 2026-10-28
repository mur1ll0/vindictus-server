import pathlib
import re
import struct
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_class_routing as routing  # noqa: E402
from patch_class_table import Image  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'


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
        data = (LAB / name).read_bytes()
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
                    original = Image((LAB / name).read_bytes())
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

    def test_every_new_pointer_is_relocated(self):
        for name in ('client.dll', 'server.dll'):
            image, report = self.plan(name)
            relocs = routing.relocations(image)
            section = int(report['section']['va'], 16)
            inside = [va for va in relocs if va >= section]
            self.assertEqual(len(inside), report['section']['pointers'])


if __name__ == '__main__':
    unittest.main()
