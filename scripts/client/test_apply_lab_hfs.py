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
ARMOR_FOLDER = 'models/armor/a'
UPPER = ARMOR_FOLDER + '/pc_female_a_upper.mdl'


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
        # A 1.69 armour model, for the item editor's replacement kits.
        packs.append((hfs.pack_name(ARMOR_FOLDER) + '.hfs', [hfs.Entry('pc_female_a_upper.mdl.comp', hfs.comp_wrap(b'IDST-old'), 0x20),
                                                             hfs.Entry('pc_female_a_upper.vvd.comp', hfs.comp_wrap(b'IDSV-old'), 0x20)]))
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
        # The developer's imported classes never reach these tests.
        from unittest import mock
        import content_additions
        patcher = mock.patch.object(content_additions, 'CLASS_IMPORTS', Path(self.temp.name) / 'no-imports')
        patcher.start()
        self.addCleanup(patcher.stop)

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
        trails = {**entry, 'key': '3', 'trails': '"TrailData"\r\n{\r\n"0" { "material" "Effects/miri_smash" }\r\n}'}
        text = with_script_entries(text, [trails], 'weapon')
        self.assertEqual(kv_blocks(text)[('weapondata', 'subtypeinfooverride', '3', 'traildata')][2], {'0'})
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

    def stage_class(self, files: dict[str, bytes]) -> None:
        import hashlib
        import json
        root = self.assets / 'classes/miri'
        records = []
        for path, data in files.items():
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_bytes(data)
            records.append({'path': path, 'sha256': hashlib.sha256(data).hexdigest()})
        (root / 'class-assets.json').write_text(json.dumps({'files': records}), encoding='utf-8')

    def test_class_assets_get_their_own_parts_and_are_cached(self):
        import apply_lab_content_additions as lab
        self.stage_class({'models/player/miri/a.mdl': b'A' * 60, 'models/player/miri/b.mdl': b'B' * 60,
                          'sound/voice/pc/miri_hi.wav': b'RIFF', 'scripts/player_action_state_miri.txt': b'"x" {}'})
        limit, lab.CLASS_PART_LIMIT = lab.CLASS_PART_LIMIT, 100
        # the fake client has none of the 1.69 files ORIGINAL_FIXES rewrites
        fixed, lab.fixed_packs = lab.fixed_packs, lambda plain, client, fixes=None: {}
        self.addCleanup(setattr, lab, 'fixed_packs', fixed)
        try:
            plan = self.run_apply([])
        finally:
            lab.CLASS_PART_LIMIT = limit
        packs = {hfs.pack_name('models/player/miri', 0), hfs.pack_name('models/player/miri', 1),
                 hfs.pack_name('sound/voice/pc')}
        # the script goes into the 1.69's own scripts pack (INJECTED_PREFIXES), the rest into new parts
        self.assertEqual((plan['class_files'], plan['class_packs'], plan['class_packs_built'], plan['class_injected']),
                         (3, 3, 3, 1))
        plain = hfs.decode_index((self.client / INDEX).read_bytes())
        self.assertTrue(packs <= set(hfs.index_packs(plain)))
        self.assertIn(hfs.pack_name(SCRIPTS_FOLDER, 0), hfs.index_packs(plain))
        keys = {e[:6] for e in hfs.index_entries(plain)}
        for name in ('a.mdl', 'b.mdl', 'miri_hi.wav', 'player_action_state_miri.txt', 'player_costume.txt'):
            self.assertIn(hfs.name_key(name), keys)
        sound = hfs.read_pack((self.client / 'hfs' / (hfs.pack_name('sound/voice/pc') + '.hfs')).read_bytes(), KS)
        self.assertEqual([(e.name, e.data) for e in sound], [('miri_hi.wav', b'RIFF')])
        model = hfs.read_pack((self.client / 'hfs' / (hfs.pack_name('models/player/miri', 1) + '.hfs')).read_bytes(), KS)
        self.assertEqual([(e.name, hfs.comp_unwrap(e.data)) for e in model], [('b.mdl.comp', b'B' * 60)])
        lab.CLASS_PART_LIMIT = 100
        try:
            again = self.run_apply([])
        finally:
            lab.CLASS_PART_LIMIT = limit
        self.assertEqual((again['class_packs_built'], again['hfs_files_changed']), (0, 0))
        (self.assets / 'classes/miri/class-assets.json').unlink()
        self.run_apply([])
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)

    def test_collab_textures_get_a_new_pack_and_undo(self):
        import hashlib
        import json
        # a 1.69 icon pack: an icon of an existing folder joins that pack
        icons = hfs.pack_name('scaleform/item_icon') + '.hfs'
        (self.client / 'hfs' / icons).write_bytes(hfs.build_pack([hfs.Entry('box_gold.tga.comp', hfs.comp_wrap(b'TGA0'), 0x20)], KS))
        plain, _ = hfs.sync(hfs.decode_index((self.client / INDEX).read_bytes()),
                            [(icons[:-4], (self.client / 'hfs' / icons).read_bytes())], KS)
        (self.client / INDEX).write_bytes(hfs.encode_index(plain, HEAD))
        self.original = {p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}
        root = self.assets / 'collabs/fairytail'
        files = {'materials/models/armor/fairytail/pc_female_ftlucy_jpn_upper.vmt': b'"Heroes_Faceskin" {}',
                 'materials/models/armor/fairytail/pc_female_ftlucy_jpn_upper.vtf': b'VTF\0data',
                 'scaleform/item_icon/eq_ftlucy_jpn_upper.tga': b'TGA1'}
        for path, data in files.items():
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_bytes(data)
        records = [{'path': p, 'sha256': hashlib.sha256(d).hexdigest()} for p, d in files.items()]
        (root / 'collab-assets.json').write_text(json.dumps({'files': records}), encoding='utf-8')
        report = self.run_apply([])
        self.assertEqual((report['hfs'], report['collab_files'], report['collab_injected'], report['class_files'], report['class_packs']),
                         ('applied', 3, 1, 0, 1))
        self.assertEqual([(e.name, hfs.comp_unwrap(e.data)) for e in hfs.read_pack((self.client / 'hfs' / icons).read_bytes(), KS)],
                         [('box_gold.tga.comp', b'TGA0'), ('eq_ftlucy_jpn_upper.tga.comp', b'TGA1')])
        folder = 'materials/models/armor/fairytail'
        pack = self.client / 'hfs' / (hfs.pack_name(folder) + '.hfs')
        self.assertEqual(sorted((e.name, hfs.comp_unwrap(e.data)) for e in hfs.read_pack(pack.read_bytes(), KS)),
                         [('pc_female_ftlucy_jpn_upper.vmt.comp', b'"Heroes_Faceskin" {}'),
                          ('pc_female_ftlucy_jpn_upper.vtf.comp', b'VTF\0data')])
        plain = hfs.decode_index((self.client / INDEX).read_bytes())
        self.assertIn(hfs.pack_name(folder), hfs.index_packs(plain))
        self.assertIn(hfs.name_key('pc_female_ftlucy_jpn_upper.vtf'), {e[:6] for e in hfs.index_entries(plain)})
        self.assertIn(hfs.name_key('eq_ftlucy_jpn_upper.tga'), {e[:6] for e in hfs.index_entries(plain)})
        self.assertNotIn(hfs.pack_name('scaleform/item_icon', 1), hfs.index_packs(plain))
        self.assertEqual(self.run_apply([])['hfs_files_changed'], 0)
        # a game path outside materials/ is refused
        records.append({'path': 'scripts/x.txt', 'sha256': '0' * 64})
        (root / 'collab-assets.json').write_text(json.dumps({'files': records}), encoding='utf-8')
        from prepare_lab_client import PreparationError
        with self.assertRaises(PreparationError):
            self.run_apply([])
        (root / 'collab-assets.json').unlink()
        self.run_apply([])
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)

    def test_class_text_adds_only_missing_class_tokens(self):
        import json
        root = self.assets / 'classes/miri'
        (root / 'merge/text').mkdir(parents=True)
        na = '"lang"\r\n{\r\n\t"Tokens"\r\n\t{\r\n\t\t"OLD"\t"miri old"\r\n\t\t"HEROES_ITEM_NAME_DRAKER_X"\t"Dragon \\"X\\""\r\n' \
             '\t\t"HEROES_UNRELATED"\t"x"\r\n\t}\r\n}\r\n'
        (root / 'merge/text/heroes_text_english.txt').write_bytes(na.encode('utf-16'))
        (root / 'class-assets.json').write_text(json.dumps({'class': 'miri', 'weapon': 'draker', 'files': [],
                                                            'text': {'path': 'text/heroes_text_english.txt'}}), encoding='utf-8')
        plan = self.run_apply([])
        self.assertEqual(plan['class_tokens'], 1)  # OLD's key does not name the class
        pack = self.client / 'hfs' / (hfs.pack_name(TEXT_FOLDERS[0]) + '.hfs')
        text = hfs.read_pack(pack.read_bytes(), KS)[0].data.decode('utf-16')
        self.assertIn('"HEROES_ITEM_NAME_DRAKER_X"\t"Dragon \\"X\\""', text)
        self.assertIn('"OLD"\t"old"', text)
        self.assertNotIn('miri old', text)
        self.assertNotIn('HEROES_UNRELATED', text)
        (root / 'class-assets.json').unlink()
        self.run_apply([])
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)

    def test_collab_texts_replace_only_untranslated_lines_and_undo(self):
        texts = {'OLD': 'english old', 'HEROES_NPC_FAIRYTAIL_NATSU': 'Natsu', 'KOREAN': 'Translated'}
        korean = TEXT.decode('utf-16').replace('"OLD"', '"KOREAN"\t"\ub098\uce20"\r\n\t\t"OLD"').encode('utf-16')
        for path in (self.client / 'hfs').iterdir():
            if path.name.startswith(tuple(hfs.pack_name(f) for f in TEXT_FOLDERS)):
                entry = hfs.read_pack(path.read_bytes(), KS)[0]
                path.write_bytes(hfs.build_pack([hfs.Entry(entry.name, korean, entry.attr)], KS))
        original = {p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}
        apply_hfs(self.client, [], Path(self.temp.name) / 'ks.bin', True, self.assets, collab_texts=texts)
        pack = self.client / 'hfs' / (hfs.pack_name(TEXT_FOLDERS[0]) + '.hfs')
        text = hfs.read_pack(pack.read_bytes(), KS)[0].data.decode('utf-16')
        self.assertIn('"HEROES_NPC_FAIRYTAIL_NATSU"\t"Natsu"', text)
        self.assertIn('"KOREAN"\t"Translated"', text)
        self.assertNotIn('\ub098\uce20', text)
        self.assertIn('"OLD"\t"old"', text)  # English already there stays
        self.assertNotIn('english old', text)
        apply_hfs(self.client, [], Path(self.temp.name) / 'ks.bin', True, self.assets)
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, original)

    def test_class_merges_add_the_class_lines_to_the_169_script_and_undo(self):
        import json
        root = self.assets / 'classes/miri'
        (root / 'merge/scripts').mkdir(parents=True)
        na = COSTUME.replace(b'"hair"', b'"miri"\r\n{\r\n\t"default"\t"11"\r\n}\r\n"hair"').replace(
            b'\t"16"', b'\t"9100"\r\n\t{\r\n\t\t"label"\t"9100"\r\n\t\t"normal"\t"models/player/miri/in.mdl"\r\n\t}\r\n\t"16"')
        (root / 'merge/scripts/player_costume.txt').write_bytes(na)
        (root / 'class-assets.json').write_text(json.dumps({'files': [], 'merge': [
            {'path': 'scripts/player_costume.txt', 'needles': ['miri']}]}), encoding='utf-8')
        plan = self.run_apply(ITEMS)
        self.assertEqual(plan['class_merges']['files'], 1)
        scripts = self.client / 'hfs' / (hfs.pack_name(SCRIPTS_FOLDER) + '.hfs')
        text = hfs.comp_unwrap(hfs.read_pack(scripts.read_bytes(), KS)[0].data)
        # The item editor's entry still goes in, after the class lines; the class's 9100 moves to 9000.
        self.assertIn(b'"miri"', text)
        self.assertIn(b'"9100"\r\n\t{\r\n\t\t"category"\t"female"', text)
        self.assertIn(b'"9000"\r\n\t{\r\n\t\t"label"\t"9000"\r\n\t\t"normal"\t"models/player/miri/in.mdl"', text)
        self.assertEqual(plan['class_merges']['renumbered'], {'inner_armor/9100': '9000'})
        self.assertEqual(self.run_apply(ITEMS)['hfs_files_changed'], 0)
        (root / 'class-assets.json').unlink()
        self.run_apply([])
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)

    def test_class_layers_and_excluded_items(self):
        import json
        import apply_lab_content_additions as lab
        self.assertEqual(lab.layer_fixes(None, []), {})
        self.assertIs(lab.layer_fixes(None, ['file']), lab.ORIGINAL_FIXES)
        self.assertEqual(lab.layer_fixes((), ['file']), {})
        self.assertEqual(set(lab.layer_fixes(('delia',), ['file'])), set(lab.DELIA_FIXES))
        self.assertEqual(set(lab.layer_fixes(('delia', 'miri'), [])), set(lab.DELIA_FIXES))
        self.assertEqual(set(lab.layer_fixes(('delia', 'miri'), ['file'])), set(lab.ORIGINAL_FIXES))
        self.assertEqual(list(lab.ORIGINAL_FIXES)[0], 'models/player/_anim/female/delia/pc_delia_town_motion.mdl')
        root = self.assets / 'classes/miri'
        (root / 'merge/scripts').mkdir(parents=True)
        na = COSTUME.replace(b'"hair"', b'"miri"\r\n{\r\n\t"default"\t"11"\r\n}\r\n"hair"').replace(
            b'\t"16"', b'\t"9100"\r\n\t{\r\n\t\t"label"\t"9100"\r\n\t\t"normal"\t"models/player/miri/in.mdl"\r\n\t}\r\n\t"16"')
        (root / 'merge/scripts/player_costume.txt').write_bytes(na)
        (root / 'class-assets.json').write_text(json.dumps({'files': [], 'merge': [
            {'path': 'scripts/player_costume.txt', 'needles': ['miri']}]}), encoding='utf-8')
        ks = Path(self.temp.name) / 'ks.bin'
        scripts = self.client / 'hfs' / (hfs.pack_name(SCRIPTS_FOLDER) + '.hfs')
        # A layer without the Miri ignores her staged files.
        plan = apply_hfs(self.client, ITEMS, ks, True, self.assets, classes=())
        self.assertEqual((plan['class_merges']['files'], plan['original_fixes']), (0, []))
        self.assertNotIn(b'"miri"', hfs.comp_unwrap(hfs.read_pack(scripts.read_bytes(), KS)[0].data))
        # An excluded item leaves the copy and its number stays taken: the Miri's 9100 still moves to 9000.
        # (the fake client has none of the 1.69 files the fixes rewrite: only the chosen fixes are checked)
        chosen = []
        fixed, lab.fixed_packs = lab.fixed_packs, lambda plain, client, fixes=None: chosen.append(set(fixes)) or {}
        self.addCleanup(setattr, lab, 'fixed_packs', fixed)
        plan = apply_hfs(self.client, [], ks, True, self.assets, classes=('delia', 'miri'), numbering=ITEMS)
        text = hfs.comp_unwrap(hfs.read_pack(scripts.read_bytes(), KS)[0].data)
        self.assertEqual(plan['class_merges']['renumbered'], {'inner_armor/9100': '9000'})
        self.assertEqual(chosen, [set(lab.DELIA_FIXES)])  # merges only: no staged Miri file
        self.assertNotIn(b'"9100"', text)
        self.assertIn(b'"9000"\r\n\t{\r\n\t\t"label"\t"9000"', text)

    def write_kit(self, assets: str, files: dict[str, bytes], model: str = UPPER) -> dict:
        import json
        root = self.assets / assets
        for path, data in files.items():
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_bytes(data)
        (root / 'kit.json').write_text(json.dumps({'model': model}), encoding='utf-8')
        return {'set': {}, 'text': {}, 'model': {'assets': assets, 'model': model}}

    def apply_overrides(self, overrides):
        return apply_hfs(self.client, [], Path(self.temp.name) / 'ks.bin', True, self.assets, overrides=overrides)

    def test_replacement_kit_rewrites_the_169_model_adds_new_materials_and_undoes(self):
        material = 'materials/models/armor/a/pc_female_a_upper/a_diffuse.vmt'
        overrides = {'a_upper': self.write_kit('kit_fix', {UPPER: b'IDST-fixed', material: b'"VertexLitGeneric" {}'})}
        plan = self.apply_overrides(overrides)
        self.assertEqual(plan['model_replacements'], {'files': 2, 'new': 1, 'packs_rewritten': 1})
        armor = hfs.read_pack((self.client / 'hfs' / (hfs.pack_name(ARMOR_FOLDER) + '.hfs')).read_bytes(), KS)
        # The model changes inside its own pack; the vvd the kit leaves out stays.
        self.assertEqual([(e.name, hfs.comp_unwrap(e.data)) for e in armor],
                         [('pc_female_a_upper.mdl.comp', b'IDST-fixed'), ('pc_female_a_upper.vvd.comp', b'IDSV-old')])
        new_pack = hfs.pack_name('materials/models/armor/a/pc_female_a_upper') + '.hfs'
        self.assertEqual(plan['new_packs'], [new_pack])
        plain = hfs.decode_index((self.client / INDEX).read_bytes())
        self.assertIn(new_pack[:-4], hfs.index_packs(plain))
        self.assertIn(hfs.name_key('a_diffuse.vmt'), {e[:6] for e in hfs.index_entries(plain)})
        self.assertEqual(self.apply_overrides(overrides)['hfs_files_changed'], 0)
        # A columns-only override (or none) gives the original model back.
        self.apply_overrides({'a_upper': {'set': {'ItemClassInfo': {'Price': 5}}, 'text': {}}})
        self.assertEqual({p.name: p.read_bytes() for p in (self.client / 'hfs').iterdir()}, self.original)

    def test_replacement_kit_swaps_a_staged_class_file(self):
        import apply_lab_content_additions as lab
        model = 'models/armor/pytonshock/pc_female_pytonshock_upper.mdl'
        self.stage_class({model: b'IDST-na'})
        fixed, lab.fixed_packs = lab.fixed_packs, lambda plain, client, fixes=None: {}
        self.addCleanup(setattr, lab, 'fixed_packs', fixed)
        overrides = {'pytonshock_upper': self.write_kit('kit_na', {model: b'IDST-pushed'}, model)}
        plan = self.apply_overrides(overrides)
        self.assertEqual(plan['model_replacements'], {'files': 1, 'new': 0, 'packs_rewritten': 0})
        pack = hfs.read_pack((self.client / 'hfs' / (hfs.pack_name('models/armor/pytonshock') + '.hfs')).read_bytes(), KS)
        self.assertEqual([hfs.comp_unwrap(e.data) for e in pack], [b'IDST-pushed'])
        self.apply_overrides({})
        pack = hfs.read_pack((self.client / 'hfs' / (hfs.pack_name('models/armor/pytonshock') + '.hfs')).read_bytes(), KS)
        self.assertEqual([hfs.comp_unwrap(e.data) for e in pack], [b'IDST-na'])

    def test_replacement_kits_are_checked(self):
        from prepare_lab_client import PreparationError
        good = self.write_kit('kit_fix', {UPPER: b'IDST-fixed'})
        for overrides in ({'a_upper': self.write_kit('kit_no_model', {ARMOR_FOLDER + '/other.mdl': b'IDST'})},
                          {'a_upper': good, 'b_upper': good},
                          {'a_upper': {'model': {'assets': 'kit_missing', 'model': UPPER}}}):
            with self.assertRaises(PreparationError):
                self.apply_overrides(overrides)


if __name__ == '__main__':
    unittest.main()
