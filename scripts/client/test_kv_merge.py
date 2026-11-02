import unittest

import kv_merge

BASE = (b'"base"\r\n{\r\n\t"delia"\r\n\t{\r\n\t\t"default"\t"10"\r\n\t}\r\n\t"10"\r\n\t{\r\n'
        b'\t\t"normal"\t"models/player/delia/delia_base.mdl"\r\n\t}\r\n}\r\n'
        b'"inner_armor"\r\n{\r\n\t"9000"\r\n\t{\r\n\t\t"normal"\t"models/player/x/other.mdl"\r\n\t}\r\n}\r\n')
SOURCE = (b'"base"\r\n{\r\n\t"delia"\r\n\t{\r\n\t\t"default"\t"10"\r\n\t}\r\n\t"miri"\r\n\t{\r\n\t\t"default"\t"11"\r\n\t}\r\n'
          b'\t"10"\r\n\t{\r\n\t\t"normal"\t"models/player/delia/delia_base.mdl"\r\n\t}\r\n'
          b'\t"11"\r\n\t{\r\n\t\t"normal"\t"models/player/miri/miri_base.mdl"\r\n\t}\r\n'
          b'\t"12"\r\n\t{\r\n\t\t"normal"\t"models/player/grimden/grimden_base.mdl"\r\n\t}\r\n}\r\n'
          b'"inner_armor"\r\n{\r\n\t"9000"\r\n\t{\r\n\t\t"normal"\t"models/player/miri/miri_inner.mdl"\r\n\t}\r\n}\r\n'
          b'// comment { not a block\r\n"sounds" [$WIN32]\r\n{\r\n\t"wave"\t"a.wav"\r\n\t"wave"\t"miri_b.wav"\r\n}\r\n')


class MergeTest(unittest.TestCase):
    def test_adds_only_the_class_nodes_the_169_lacks(self):
        out, report = kv_merge.merge(BASE, SOURCE, ('miri',))
        self.assertTrue(out.startswith(BASE[:BASE.index(b'\t}\r\n}')]))
        tree = kv_merge.parse(out)
        base = next(n for n in tree.children if n.key == 'base')
        self.assertEqual([c.key for c in base.children], ['delia', '10', 'miri', '11'])
        self.assertNotIn(b'grimden', out)
        # The 1.69 entry 9000 stays; the NA one is a conflict.
        self.assertIn(b'other.mdl', out)
        self.assertNotIn(b'miri_inner.mdl', out)
        self.assertEqual(report['conflicts'], ['inner_armor/9000/normal'])
        sounds = next(n for n in tree.children if n.key == 'sounds')
        self.assertEqual([c.value for c in sounds.children], ['a.wav', 'miri_b.wav'])
        self.assertEqual((report['blocks'], report['lines']), (3, 0))

    def test_merge_is_idempotent(self):
        out, _ = kv_merge.merge(BASE, SOURCE, ('miri',))
        again, report = kv_merge.merge(out, SOURCE, ('miri',))
        self.assertEqual(again, out)
        self.assertEqual((report['blocks'], report['lines']), (0, 0))

    def test_repeated_values_are_added_by_value(self):
        base = b'"sounds"\r\n{\r\n\t"wave"\t"a.wav"\r\n}\r\n'
        source = b'"sounds"\r\n{\r\n\t"wave"\t"a.wav"\r\n\t"wave"\t"miri_b.wav"\r\n}\r\n'
        out, report = kv_merge.merge(base, source, ('miri',))
        self.assertEqual(report['conflicts'], [])
        self.assertIn(b'"wave"\t"miri_b.wav"\r\n}', out)

    def test_unbalanced_files_are_refused(self):
        with self.assertRaises(kv_merge.MergeError):
            kv_merge.parse(b'"a"\r\n{\r\n')
        with self.assertRaises(kv_merge.MergeError):
            kv_merge.parse(b'}\r\n')


if __name__ == '__main__':
    unittest.main()
