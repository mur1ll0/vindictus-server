import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_engine_class_flags as flags  # noqa: E402
from patch_class_table import Image  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'
MD = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)


def source():
    backups = sorted(LAB.glob(f'engine.dll.{flags.EXPECTED}.pre-classflags.bak'))
    return (backups[0] if backups else LAB / 'engine.dll').read_bytes()


def code(data: bytes, va: int) -> list[str]:
    return [f'{x.mnemonic} {x.op_str}' for x in MD.disasm(data, va)]


class StubTest(unittest.TestCase):
    def test_stub_sets_bits_eight_nine_and_each_extra_class(self):
        text = code(flags.stub(), 0x10000)
        self.assertEqual(text[:6], ['test edx, 0x100', 'je 0x1000e', 'mov byte ptr [ecx + 0x1a0], al',
                                    'test edx, 0x200', 'je 0x1001c', 'mov byte ptr [ecx + 0x1a1], al'])
        self.assertEqual(text[6:10], ['test edx, 0x400', 'setne byte ptr [ecx + 0x1cd]',
                                      'test edx, 0x800', 'setne byte ptr [ecx + 0x1ce]'])
        self.assertEqual(text[-1], 'ret 4')
        self.assertFalse(any('0x1a2' in t for t in text))  # +0x1a2 is Indestructible

    def test_tail_jumps_to_the_stub_and_keeps_a_ret(self):
        tail = flags.new_tail(0x5C000000)
        self.assertEqual(len(tail), len(flags.OLD_TAIL))
        self.assertEqual(code(tail[:8], flags.TAIL), ['jmp 0x5c000000', 'ret 4'])


@unittest.skipUnless((LAB / 'engine.dll').is_file(), 'needs the laboratory copy in .cache/play-client')
class LabCopyTest(unittest.TestCase):
    def test_setter_jumps_to_the_new_section_and_exits_at_the_kept_ret(self):
        data, report = flags.plan(source())
        image = Image(data)
        sections = [s for s in image.sections() if s[0] == flags.SECTION]
        self.assertEqual(len(sections), 1)
        _, vsize, vaddr, _, raw = sections[0]
        self.assertEqual(int(report['stub'], 16), image.base + vaddr)
        self.assertEqual(bytes(image.data[raw:raw + vsize]), flags.stub())
        o = image.offset(flags.TAIL)
        self.assertEqual(code(bytes(image.data[o:o + 8]), flags.TAIL), [f'jmp {report["stub"]}', 'ret 4'])
        exit_ = next(MD.disasm(bytes(image.data[image.offset(flags.EXIT):image.offset(flags.EXIT) + 2]), flags.EXIT))
        self.assertEqual(int(exit_.op_str, 16), int(report['ret'], 16))
        with self.assertRaises(flags.PreparationError):
            flags.plan(data)


if __name__ == '__main__':
    unittest.main()
