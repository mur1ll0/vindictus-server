import random
import tempfile
import unittest
from pathlib import Path

import hfs
from apply_lab_content_additions import (INDEX, SCRIPTS_FOLDER, TEXT_FOLDERS, apply_hfs, kv_blocks, script_of, with_costumes,
                                         with_script_entries, with_tokens)

# Synthetic keystream and client files: the real ones are game content and are never versioned.
KS = bytes(random.Random(7).randrange(256) for _ in range(4096))
HEAD = b'HF\x01\x02\x0a\x00' + bytes(8)
TEXT = '"lang"\r\n{\r\n\t"Tokens"\r\n\t{\r\n\t\t"OLD"\t"old"\r\n\t}\r\n}\r\n'.encode('utf-16')
COSTUME = b'"inner_armor"\r\n{\r\n\t"16"\r\n\t{\r\n\t\t"normal"\t"models/a.mdl"\r\n\t}\r\n}\r\n"hair"\r\n{\r\n}\r\n'
ITEMS = [{'ItemClass': 'new_inner', 'template': 'old_inner', 'text': {'name': 'New'},
          'costumes': [{'section': 'inner_armor', 'key': '9100', 'category': 'female', 'normal': 'models/player/f/new.mdl'}],
          'assets': 'kit'}]


class ApplyLabHfsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.client = Path(self.temp.name) / 'client'
        (self.client / 'hfs').mkdir(parents=True)
        (Path(self.temp.name) / 'ks.bin').write_bytes(KS)
        packs = []
        for folder in TEXT_FOLDERS:
            language = folder.rsplit('/', 1)[1]
            packs.append((hfs.pack_name(folder) + '.hfs', [hfs.Entry(f'heroes_text_{language}.txt', TEXT, 0x20)]))
        packs.append((hfs.pack_name(SCRIPTS_FOLDER) + '.hfs', [hfs.Entry('player_costume.txt.comp', hfs.comp_wrap(COSTUME), 0x20)]))
        names = []
        for name, entries in packs:
            (self.client / 'hfs' / name).write_bytes(hfs.build_pack(entries, KS))
            names.append(name[:-4])
        plain = hfs.assemble_index(hfs.INDEX_MAGIC + bytes(16), sorted(names), [])
        plain, _ = hfs.sync(plain, [(n, (self.client / 'hfs' / n).read_bytes()) for n, _ in packs], KS)
        (self.client / INDEX).write_bytes(hfs.encode_index(plain, HEAD))
        self.assets = Path(self.temp.name) / 'assets'
        (self.assets / 'kit/models/player/f').mkdir(parents=True)
        (self.assets / 'kit/models/player/f/new.mdl').write_bytes(b'IDST0')
        self.original = {p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}

    def tearDown(self):
        self.temp.cleanup()

    def run_apply(self, items, apply_changes=True):
        return apply_hfs(self.client, items, Path(self.temp.name) / 'ks.bin', apply_changes, self.assets)

    def test_text_and_costume_helpers(self):
        self.assertIn('"NEW"\t"x"', with_tokens(TEXT, {'NEW': 'x'}).decode('utf-16'))
        # A renamed base item keeps one line, the new one.
        renamed = with_tokens(TEXT, {'OLD': 'renamed'}).decode('utf-16')
        self.assertEqual((renamed.count('"OLD"'), '"OLD"\t"renamed"' in renamed), (1, True))
        text = with_costumes(COSTUME, ITEMS[0]['costumes'])
        self.assertLess(text.index(b'"9100"'), text.index(b'"hair"'))
        with self.assertRaises(Exception):
            with_costumes(text, ITEMS[0]['costumes'])

    def test_weapon_and_shield_entries_go_in_their_model_blocks(self):
        weapon = (b'// LongSword\r\nWeaponData\r\n{\r\n\t"playermodel"\t"models/weapon/a.mdl"\r\n\t"SubTypeInfoOverride"\r\n'
                  b'\t{\r\n\t\t// "5" { not a block\r\n\t\t"1"\r\n\t\t{\r\n\t\t\t"playermodel"\t"models/weapon/b.mdl"\r\n\t\t}\r\n'
                  b'\t}\r\n\t"ActTable"\r\n\t{\r\n\t\t"9" "x"\r\n\t}\r\n}\r\n')
        entry = {'section': 'weapon_longsword', 'key': '2', 'normal': 'models/weapon/longsword/new.mdl'}
        text = with_script_entries(weapon, [entry], 'weapon')
        block = kv_blocks(text)[('weapondata', 'subtypeinfooverride')]
        self.assertEqual(block[2], {'1', '2'})
        self.assertLess(text.index(b'"2"'), text.index(b'"ActTable"'))
        self.assertEqual(text.count(b'models/weapon/longsword/new.mdl'), 2)
        with self.assertRaises(Exception):
            with_script_entries(text, [entry], 'weapon')
        shields = b'"Fiona"\r\n{\r\n\t"normal_paladin_subtype_id"\t"23"\r\n\t"0"\r\n\t{\r\n\t\t"normal"\t"models/weapon/shield/a.mdl"\r\n\t}\r\n}\r\n'
        text = with_script_entries(shields, [{**entry, 'section': 'shields', 'key': '1'}], 'shields')
        self.assertEqual(kv_blocks(text)[('fiona',)][2], {'0', '1'})
        self.assertEqual((script_of('weapon_staff'), script_of('shields'), script_of('inner_armor')),
                         ('weapon_staff.txt.comp', 'shields.txt.comp', 'player_costume.txt.comp'))

    def test_dry_run_writes_nothing(self):
        plan = self.run_apply(ITEMS, apply_changes=False)
        self.assertEqual(plan['hfs_files_changed'], 5)
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)

    def test_apply_is_idempotent_and_no_items_restores_the_originals(self):
        plan = self.run_apply(ITEMS)
        new_pack = hfs.pack_name('models/player/f') + '.hfs'
        self.assertEqual(plan['new_packs'], [new_pack])
        plain = hfs.decode_index((self.client / INDEX).read_bytes())
        self.assertIn(new_pack[:-4], hfs.index_packs(plain))
        keys = {e[:6] for e in hfs.index_entries(plain)}
        self.assertIn(hfs.name_key('new.mdl'), keys)
        self.assertEqual(self.run_apply(ITEMS)['hfs_files_changed'], 0)
        self.run_apply([])
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)


if __name__ == '__main__':
    unittest.main()
