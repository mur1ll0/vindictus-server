import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_engine_class_flags as flags  # noqa: E402
from patch_class_table import Image  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'


def source():
    backups = sorted(LAB.glob(f'engine.dll.{flags.EXPECTED}.pre-classflags.bak'))
    return (backups[0] if backups else LAB / 'engine.dll').read_bytes()


class TailTest(unittest.TestCase):
    def test_new_tail_fits_and_ends_in_ret(self):
        self.assertEqual(len(flags.NEW_TAIL), len(flags.OLD_TAIL))
        self.assertEqual(flags.NEW_TAIL[-3:], b'\xc2\x04\x00')


@unittest.skipUnless((LAB / 'engine.dll').is_file(), 'needs the laboratory copy in .cache/play-client')
class LabCopyTest(unittest.TestCase):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)

    def test_setter_sets_flags_eight_nine_and_delia_and_exits_at_the_new_ret(self):
        data, report = flags.plan(source())
        image = Image(data)
        o = image.offset(flags.TAIL)
        code = [f'{x.mnemonic} {x.op_str}' for x in self.md.disasm(bytes(image.data[o:o + 32]), flags.TAIL)]
        self.assertEqual(code[0], 'add ecx, 0x1a0')
        for text in ('mov byte ptr [ecx], al', 'mov byte ptr [ecx + 1], al', 'setne byte ptr [ecx + 0x2d]'):
            self.assertIn(text, code)
        self.assertEqual(0x1A0 + 0x2D, flags.DELIA_FLAG)
        self.assertFalse(any('ecx + 2]' in text for text in code))  # +0x1a2 is Indestructible
        self.assertEqual(code[-1], 'ret 4')
        exit_ = next(self.md.disasm(bytes(image.data[image.offset(flags.EXIT):image.offset(flags.EXIT) + 2]), flags.EXIT))
        self.assertEqual(int(exit_.op_str, 16), int(report['ret'], 16))


if __name__ == '__main__':
    unittest.main()
