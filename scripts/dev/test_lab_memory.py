import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lab_memory  # noqa: E402


class FakeGame(lab_memory.Game):
    """A Game over a dict of dwords, without a process."""

    def __init__(self, memory, symbols):
        self.memory, self.symbols = memory, symbols

    def read(self, address, size):
        return b''.join(struct.pack('<i', self.memory.get(address + i, 0)) for i in range(0, size, 4))[:size]


class EvalTest(unittest.TestCase):
    def test_symbols_offsets_and_dereference(self):
        game = FakeGame({0x1000: 0x2000}, {'client.costume.real': 0x1000})
        self.assertEqual(game.eval('0x10+4'), 0x14)
        self.assertEqual(game.eval('client.costume.real'), 0x1000)
        self.assertEqual(game.eval('[client.costume.real]+0xbb8'), 0x2BB8)
        self.assertEqual(game.eval('[ 0x1000 ] - 0x8'), 0x1FF8)

    def test_bad_expressions_are_refused(self):
        game = FakeGame({}, {})
        for text in ('unknown', '[0x10', '0x10 0x20', '0x10;'):
            with self.assertRaises(lab_memory.InspectError):
                game.eval(text)


class SymbolsTest(unittest.TestCase):
    def test_report_symbols_follow_a_moved_module(self):
        report = {'dlls': [{'dll': 'x/bin/client.dll', 'costume': {'shadow': '0x3921d3e0'},
                            'match_table': {'map': '0x3921d260'}}]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'class-routing.json'
            path.write_text(json.dumps(report), encoding='utf-8')
            original, lab_memory.REPORT = lab_memory.REPORT, path
            try:
                same = lab_memory.load_symbols({'client.dll': (0x38000000, 0, '')})
                moved = lab_memory.load_symbols({'client.dll': (0x39000000, 0, '')})
            finally:
                lab_memory.REPORT = original
        self.assertEqual(same['client.costume.shadow'], 0x3921D3E0)
        self.assertEqual(same['client.costume.real'], 0x3921E3E0)
        self.assertEqual(moved['client.match.delia_map'], 0x3921D260 + 0x1000000)


if __name__ == '__main__':
    unittest.main()
