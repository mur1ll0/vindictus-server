import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_client_auto_login as auto  # noqa: E402
from patch_class_table import Image  # noqa: E402
from prepare_lab_client import PreparationError  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'
# The 1.69 original (patch_shop_list_limit's backup); every client.dll patch leaves this code alone.
ORIGINAL_DLL = LAB / 'client.dll.b284ba719047556b.bak'


def text(image, va):
    o = image.offset(va)
    return bytes(image.data[o:image.data.index(0, o)]).decode()


class BytesTest(unittest.TestCase):
    def test_same_length_and_a_six_byte_nop(self):
        self.assertEqual(len(auto.PATCHED), len(auto.ORIGINAL))
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        (nop,) = md.disasm(auto.PATCHED, auto.SITE)
        self.assertEqual((nop.mnemonic, nop.size), ('nop', 6))
        (je,) = md.disasm(auto.ORIGINAL, auto.SITE)
        self.assertEqual((je.mnemonic, int(je.op_str, 16)), ('je', auto.DIALOG))


@unittest.skipUnless(ORIGINAL_DLL.is_file(), 'needs the laboratory copy in .cache/play-client')
class LabCopyTest(unittest.TestCase):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)

    def disasm(self, data, va, length):
        image = Image(data)
        o = image.offset(va)
        return list(self.md.disasm(bytes(image.data[o:o + length]), va))

    def test_original_gates_the_passport_read_on_the_event_modes(self):
        data = ORIGINAL_DLL.read_bytes()
        image = Image(data)
        code = self.disasm(data, 0x385233d4, 0x80)
        pushes = [text(image, int(x.op_str, 16)) for x in code if x.mnemonic == 'push' and x.op_str.startswith('0x38b')]
        self.assertEqual(pushes[:3], ['E3_Event_2010', 'Event_PAX_2011_enUS', 'NGM_disable'])
        site = next(x for x in code if x.address == auto.SITE)
        self.assertEqual((site.mnemonic, int(site.op_str, 16)), ('je', auto.DIALOG))
        self.assertTrue(any(x.address == auto.PASSPORT_READ for x in code))
        # The NGM_disable branch reads the event_account ConVar (flags and parent of 0x38e5d9c8).
        reads = {x.op_str for x in code if x.mnemonic == 'mov' and '0x38e5d9' in x.op_str}
        self.assertEqual(reads, {'esi, dword ptr [0x38e5d9dc]', 'edi, dword ptr [0x38e5d9e0]'})
        # The ConVar is registered as "event_account".
        register = self.disasm(data, 0x38aa5670, 0x16)
        self.assertEqual(text(image, int(register[2].op_str, 16)), 'event_account')
        self.assertEqual(register[3].op_str, 'ecx, 0x38e5d9c8')
        # Neither event mode → LoginDialog_V2.
        dialog = self.disasm(data, auto.DIALOG, 0x10)
        self.assertEqual(text(image, int(dialog[3].op_str, 16)), 'LoginDialog_V2')

    def test_patched_falls_through_and_is_idempotent(self):
        data = ORIGINAL_DLL.read_bytes()
        patched, already = auto.plan(data)
        self.assertFalse(already)
        nop = self.disasm(patched, auto.SITE, 6)[0]
        self.assertEqual((nop.mnemonic, nop.address + nop.size), ('nop', auto.PASSPORT_READ))
        again, already = auto.plan(patched)
        self.assertTrue(already)
        self.assertEqual(again, patched)
        changed = [i for i, (a, b) in enumerate(zip(data, patched)) if a != b]
        self.assertLessEqual(len(changed), 6)

    def test_lab_copy_keeps_the_site_reachable(self):
        data = (LAB / 'client.dll').read_bytes()
        image = Image(data)
        o = image.offset(auto.SITE)
        self.assertIn(bytes(image.data[o:o + 6]), (auto.ORIGINAL, auto.PATCHED))

    def test_unknown_bytes_are_refused(self):
        image = Image(ORIGINAL_DLL.read_bytes())
        o = image.offset(auto.SITE)
        image.data[o] ^= 0xFF
        with self.assertRaises(PreparationError):
            auto.plan(bytes(image.data))


if __name__ == '__main__':
    unittest.main()
