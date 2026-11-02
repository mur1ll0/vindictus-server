import pathlib
import re
import struct
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import patch_client_launcher_login as login  # noqa: E402
from patch_class_routing import relocations  # noqa: E402
from patch_class_table import Image  # noqa: E402
from prepare_lab_client import PreparationError  # noqa: E402

LAB = pathlib.Path(__file__).resolve().parents[2] / '.cache/play-client/bin'
ORIGINAL_DLL = LAB / 'client.dll.b284ba719047556b.bak'
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)


def source():
    """The lab client.dll before this patch (its backup when the copy already has it)."""
    backups = sorted(LAB.glob('client.dll.*.pre-launcherlogin.bak'))
    return (backups[0] if backups else LAB / 'client.dll').read_bytes()


def disasm(data, va, length):
    image = Image(data)
    o = image.offset(va)
    return list(md.disasm(bytes(image.data[o:o + length]), va))


def text(data, va):
    image = Image(data)
    o = image.offset(va)
    return bytes(image.data[o:image.data.index(0, o)]).decode()


def imm(ins):
    found = re.search(r'0x[0-9a-f]+', ins.op_str)
    return int(found.group(), 16) if found else None


class StubTest(unittest.TestCase):
    va = 0x39000000

    def test_stub_calls_branches_and_relocations(self):
        body, relocs = login.build_stub(self.va)
        self.assertEqual(body[0x100:0x100 + len(login.PARAMETER) + 1], login.PARAMETER.encode() + b'\0')
        code = list(md.disasm(body[:0x100], self.va))
        stop = next(i for i, x in enumerate(code) if x.mnemonic == 'ret')
        code = code[:stop + 1]
        self.assertEqual((code[0].mnemonic, code[0].op_str), ('call', f'dword ptr [{login.COMMAND_LINE:#x}]'))
        calls = {imm(x) for x in code if x.mnemonic == 'call' and not x.op_str.startswith('dword')}
        self.assertEqual(calls, {login.SCALEFORM, login.SERVER_NAME, login.NEW, login.KEYVALUES, login.STATES,
                                 login.TRANSITION})
        epilogue = code[-5].address
        self.assertEqual(bytes(body[epilogue - self.va:epilogue - self.va + len(login.EPILOGUE)]), login.EPILOGUE)
        targets = {imm(x) for x in code if x.mnemonic.startswith('j')}
        self.assertIn(epilogue, targets)
        self.assertTrue(all(self.va <= t <= epilogue for t in targets))
        # The ticket is pushed as the ID value right before the "ID" key.
        pushes = [x for x in code if x.mnemonic == 'push']
        at = next(i for i, x in enumerate(pushes) if imm(x) == login.ID)
        self.assertEqual(pushes[at - 1].op_str, 'edi')
        self.assertEqual([imm(x) for x in pushes[at - 3:at - 1]], [login.EMPTY, login.PASSWORD])
        self.assertEqual(imm(pushes[at + 1]), login.TRANSITION_ARGS)
        # Every absolute dword is relocated, and only those.
        absolute = {self.va + 0x100, login.COMMAND_LINE, login.EMPTY, login.SELECTED_SERVER, login.ENDPOINTS,
                    login.PASSWORD, login.ID, login.TRANSITION_ARGS, login.LOGIN_OK}
        values = {struct.unpack_from('<I', body, r)[0] for r in relocs}
        self.assertEqual(values, absolute)


@unittest.skipUnless(ORIGINAL_DLL.is_file(), 'needs the laboratory copy in .cache/play-client')
class LabCopyTest(unittest.TestCase):
    def test_original_login_command_uses_the_same_calls(self):
        data = ORIGINAL_DLL.read_bytes()
        code = disasm(data, 0x3826a3c5, 0x120)
        values = [imm(x) for x in code]
        for expected in (login.SELECTED_SERVER, login.SERVER_NAME, login.NEW, login.PASSWORD, login.ID,
                         login.TRANSITION_ARGS, login.KEYVALUES, login.LOGIN_OK):
            self.assertIn(expected, values)
        tail = disasm(data, 0x3826a4d3, 0x0c)
        self.assertEqual([imm(x) for x in tail if x.mnemonic == 'call'], [login.STATES, login.TRANSITION])
        self.assertEqual(text(data, login.LOGIN_OK), 'LoginDialog_OK')
        self.assertEqual(text(data, login.TRANSITION_ARGS), 'TransitionArgs')
        self.assertEqual(text(data, login.SELECTED_SERVER), 'GameTitleMenu_V2.selectedServerStr')
        self.assertEqual(text(data, login.EMPTY), '')

    def test_command_line_parm_value_is_used_the_same_way(self):
        data = ORIGINAL_DLL.read_bytes()
        code = disasm(data, 0x3826ab2a, 0x19)
        self.assertEqual(code[0].op_str, f'dword ptr [{login.COMMAND_LINE:#x}]')
        self.assertIn('eax, dword ptr [edx + 0x20]', [x.op_str for x in code])
        pushes = [imm(x) for x in code if x.mnemonic == 'push']
        self.assertEqual(pushes[0], login.EMPTY)  # default first, then the name
        self.assertEqual(text(data, pushes[1]), '-event_account')
        self.assertEqual(code[-1].op_str, 'eax')

    def test_show_login_dialog_ends_at_the_site_and_nothing_jumps_inside(self):
        data = source()
        before = disasm(data, 0x3826a3a3, login.SITE - 0x3826a3a3)
        loaded = next(x for x in before if x.op_str.startswith('ebx, 0x'))
        self.assertEqual(text(data, imm(loaded)), 'GameTitleMenu_V2.selectServerItem')
        self.assertEqual(imm(before[-1]), 0x3826a110)
        start, end = login.HANDLER
        for x in disasm(data, start, end - start):
            if x.mnemonic.startswith('j') and imm(x) is not None:
                self.assertFalse(login.SITE < imm(x) < login.SITE + len(login.EPILOGUE), hex(x.address))

    def test_patched_jumps_to_the_section_and_keeps_the_relocations(self):
        data = source()
        patched, already = login.plan(data)
        self.assertFalse(already)
        image = Image(patched)
        section = [s for s in image.sections() if s[0] == login.SECTION][0]
        stub = image.base + section[2]
        (jmp,) = disasm(patched, login.SITE, 5)
        self.assertEqual((jmp.mnemonic, imm(jmp)), ('jmp', stub))
        self.assertEqual(text(patched, stub + 0x100), login.PARAMETER)
        old, new = relocations(Image(data)), relocations(image)
        self.assertTrue(old <= new)
        self.assertEqual(len(new - old), len(login.build_stub(stub)[1]))
        self.assertTrue(all(stub <= r < stub + 0x100 for r in new - old))
        again, already = login.plan(patched)
        self.assertTrue(already)
        self.assertEqual(again, patched)

    def test_unknown_bytes_are_refused(self):
        image = Image(source())
        o = image.offset(login.SITE)
        image.data[o] ^= 0xFF
        with self.assertRaises(PreparationError):
            login.plan(bytes(image.data))


if __name__ == '__main__':
    unittest.main()
