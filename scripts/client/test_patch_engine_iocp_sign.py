import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_engine_iocp_sign as sign  # noqa: E402
from patch_class_table import Image  # noqa: E402
from prepare_lab_client import PreparationError  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'


def source():
    backups = sorted(LAB.glob('engine.dll.*.pre-iocpsign.bak'))
    return (backups[0] if backups else LAB / 'engine.dll').read_bytes()


class BytesTest(unittest.TestCase):
    def test_only_the_condition_changes(self):
        self.assertEqual(len(sign.PATCHED), len(sign.ORIGINAL))
        self.assertEqual(sign.ORIGINAL[:2], sign.PATCHED[:2])
        self.assertEqual((sign.ORIGINAL[2], sign.PATCHED[2]), (0x7D, 0xEB))  # jge rel8 -> jmp rel8
        self.assertEqual(sign.ORIGINAL[3], sign.PATCHED[3])


@unittest.skipUnless((LAB / 'engine.dll').is_file(), 'needs the laboratory copy in .cache/play-client')
class LabCopyTest(unittest.TestCase):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)

    def disasm(self, data, va, length):
        image = Image(data)
        o = image.offset(va)
        return list(self.md.disasm(bytes(image.data[o:o + length]), va))

    def test_original_tests_the_overlapped_pointer_sign_after_the_completion_wait(self):
        data = source()
        code = self.disasm(data, 0x425fd376, 0x30)
        self.assertEqual(f'{code[0].mnemonic} {code[0].op_str}', 'call edx')  # GetQueuedCompletionStatus
        self.assertEqual(f'{code[1].mnemonic} {code[1].op_str}', 'mov eax, dword ptr [esp + 0x18]')
        site = next(x for x in code if x.address == sign.SITE)
        self.assertEqual(f'{site.mnemonic} {site.op_str}', 'cmp eax, ebx')
        jump = next(x for x in code if x.address == sign.SITE + 2)
        self.assertEqual((jump.mnemonic, int(jump.op_str, 16)), ('jge', sign.MATCH))
        match = self.disasm(data, sign.MATCH, 8)
        self.assertEqual(f'{match[0].mnemonic} {match[0].op_str}', 'lea ebp, [ecx + 0xc0]')

    def test_patched_always_matches_and_is_idempotent(self):
        data, already = sign.plan(source())
        self.assertFalse(already)
        jump = self.disasm(data, sign.SITE + 2, 2)[0]
        self.assertEqual((jump.mnemonic, int(jump.op_str, 16)), ('jmp', sign.MATCH))
        again, already = sign.plan(data)
        self.assertTrue(already)
        self.assertEqual(again, data)
        changed = [i for i, (a, b) in enumerate(zip(source(), data)) if a != b]
        self.assertEqual(len(changed), 1)

    def test_unknown_bytes_are_refused(self):
        image = Image(source())
        o = image.offset(sign.SITE)
        image.data[o] ^= 0xFF
        with self.assertRaises(PreparationError):
            sign.plan(bytes(image.data))


if __name__ == '__main__':
    unittest.main()
