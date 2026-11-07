"""Class layers of the client versions (class_layers.py): each DLL patch for v1 (no new class), v2 (Delia)
and v3 (Delia and Miri), planned in memory from the 1.69 EU originals (read only)."""
import argparse
import hashlib
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / '.deps'))
import capstone  # noqa: E402

import class_layers  # noqa: E402
import patch_class_routing as routing  # noqa: E402
import patch_class_table as table  # noqa: E402
import patch_client_launcher_login as login  # noqa: E402
import patch_engine_class_flags as flags  # noqa: E402
import patch_engine_iocp_sign as iocp  # noqa: E402
import patch_shop_list_limit as shop  # noqa: E402
from patch_class_table import Image  # noqa: E402

ORIGINAL = pathlib.Path(__file__).resolve().parents[3] / 'Vindictus1.69EN/Vindictus EU/en-EU/bin'
MD = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)


def sha16(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:16]


def code(data: bytes, va: int, length: int) -> list:
    image = Image(data)
    o = image.offset(va)
    return list(MD.disasm(bytes(image.data[o:o + length]), va))


class ParseTest(unittest.TestCase):
    def test_layers_are_prefixes(self):
        self.assertEqual(class_layers.parse('none'), ())
        self.assertEqual(class_layers.parse('Delia'), ('delia',))
        self.assertEqual(class_layers.parse('delia, miri'), ('delia', 'miri'))
        for bad in ('miri', 'miri,delia', 'delia,delia', 'lann'):
            with self.assertRaises(argparse.ArgumentTypeError):
                class_layers.parse(bad)
        self.assertEqual(class_layers.VERSIONS[2], ('delia',))

    def test_bounds_and_flags_of_each_layer(self):
        self.assertEqual(table.active_classes(), table.ACTIVE_CLASSES)
        self.assertEqual(table.active_classes(('delia',)), 11)
        self.assertEqual(flags.flags_of(), flags.CLASS_FLAGS)
        self.assertEqual(flags.flags_of(('delia',)), {10: 0x1CD})
        with self.assertRaises(ValueError):
            table.active_classes(())
        text = [f'{x.mnemonic} {x.op_str}' for x in MD.disasm(flags.stub(flags.flags_of(('delia',))), 0x10000)]
        self.assertEqual(text[6:], ['test edx, 0x400', 'setne byte ptr [ecx + 0x1cd]', 'ret 4'])


@unittest.skipUnless((ORIGINAL / 'client.dll').is_file(), 'needs the 1.69 EU original (read only)')
class OriginalDllTest(unittest.TestCase):
    """Patch chains of docs/client-dll-patches.md per version, in memory."""

    @classmethod
    def setUpClass(cls):
        cls.client = (ORIGINAL / 'client.dll').read_bytes()
        cls.server = (ORIGINAL / 'server.dll').read_bytes()
        cls.engine = (ORIGINAL / 'engine.dll').read_bytes()
        cls.shop, _ = shop.patched_bytes(cls.client)

    def hcls(self, name: str, data: bytes, classes) -> bytes:
        out, already = table.patched_bytes(data, table.SPECS[name], table.active_classes(classes))
        self.assertFalse(already)
        return out

    def test_v1_launcher_login_on_a_client_without_class_sections(self):
        patched, already = login.plan(self.shop)
        self.assertFalse(already)
        image = Image(patched)
        names = [s[0] for s in image.sections()]
        self.assertIn(login.SECTION, names)
        self.assertFalse({b'.hcls\0\0\0', b'.hrte\0\0\0'} & set(names))
        stub = image.base + next(s for s in image.sections() if s[0] == login.SECTION)[2]
        (jump,) = code(patched, login.SITE, 5)
        self.assertEqual((jump.mnemonic, int(jump.op_str, 16)), ('jmp', stub))
        old, new = routing.relocations(Image(self.shop)), routing.relocations(image)
        self.assertTrue(old <= new)
        self.assertEqual(len(new - old), len(login.build_stub(stub)[1]))
        self.assertEqual(login.plan(patched), (patched, True))

    def test_v1_iocp_alone_on_the_original_engine(self):
        patched, already = iocp.plan(self.engine)
        self.assertFalse(already)
        self.assertEqual([i for i, (a, b) in enumerate(zip(self.engine, patched)) if a != b], [Image(self.engine).offset(iocp.SITE) + 2])
        (jump,) = code(patched, iocp.SITE + 2, 2)
        self.assertEqual((jump.mnemonic, int(jump.op_str, 16)), ('jmp', iocp.MATCH))
        self.assertNotIn(flags.SECTION, [s[0] for s in Image(patched).sections()])

    def test_every_class_layer_is_the_output_of_before(self):
        for name, data, expected in (('client.dll', self.shop, 'c0b5d4164cc122a3'),
                                     ('server.dll', self.server, '5f0429365ee0ccd1')):
            source = self.hcls(name, data, class_layers.LAYERS)
            self.assertEqual(sha16(source), routing.EXPECTED[name])
            with routing.class_layer():
                out, _ = routing.plan_dll(name, source)
            self.assertEqual(sha16(out), expected)
        out, _ = flags.plan(self.engine)
        self.assertEqual(sha16(iocp.plan(out)[0]), 'df6c70d2d66f52ed')

    def test_v2_routes_the_delia_only(self):
        for name, data in (('client.dll', self.shop), ('server.dll', self.server)):
            source = self.hcls(name, data, ('delia',))
            with routing.class_layer(('delia',)):
                self.assertEqual(sha16(source), routing.EXPECTED[name])
                self.assertEqual(routing.LAST, 10)
                self.assertEqual(routing.GUARD_CLASSES, (10,))
                image, report = routing.plan_dll(name, source)
            for group in ('custom_effect', 'flamebreath_leave', 'summon_parent', 'summon_follow'):
                self.assertNotIn(group, report)
            conditions = report['class_conditions']
            self.assertNotIn('chain', conditions)
            self.assertNotIn('flamebreath', conditions)
            self.assertEqual(conditions['classes'], {})
            for switch in report['switches']:
                self.assertEqual(set(switch['cases']), {10})
            # The condition name table keeps every name (the same ids in v2 and v3; no Delia script names
            # the Miri's); nothing else of the Miri goes into .hrte.
            texts = bytes(image[len(source):])
            for condition in routing.CONDITION_NAMES:
                texts = texts.replace(condition.encode(), b'')
            for miri in (b'miri', b'Miri', routing.DRAGON_KNIGHT_TYPE.encode()):
                self.assertFalse(miri in texts, f'{name}: {miri!r} in .hrte')
            self.assertIn(b'delia', texts)
        # Back to every class outside the layer.
        self.assertEqual(routing.LAST, 11)
        self.assertEqual(routing.GUARD_CLASSES, (10, 11))

    def test_v2_engine_sets_the_delia_byte_only(self):
        out, report = flags.plan(self.engine, flags.flags_of(('delia',)))
        image = Image(out)
        _, vsize, vaddr, _, raw = next(s for s in image.sections() if s[0] == flags.SECTION)
        self.assertEqual(bytes(image.data[raw:raw + vsize]), flags.stub({10: 0x1CD}))
        self.assertNotIn('class 11', report['flags'])
        self.assertEqual(iocp.plan(out)[1], False)


if __name__ == '__main__':
    unittest.main()
