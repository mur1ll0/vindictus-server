import json
import struct
import tempfile
import unittest
from pathlib import Path

import mdl_anim
import stage_na_class as stage
import test_mdl_anim as models


def model(textures: list[str], folders: list[str]) -> bytes:
    """A v44 studio header with texture names and cdmaterials folders, enough for model_materials."""
    header = bytearray(408)
    header[:4] = b'IDST'
    struct.pack_into('<i', header, 4, 44)
    texture_at = 408
    cd_at = texture_at + 64 * len(textures)
    strings_at = cd_at + 4 * len(folders)
    body = bytearray(strings_at - 408)
    strings = bytearray()
    for i, name in enumerate(textures):
        at = texture_at + 64 * i
        struct.pack_into('<i', body, at - 408, strings_at + len(strings) - at)
        strings += name.encode() + b'\0'
    for i, folder in enumerate(folders):
        struct.pack_into('<i', body, cd_at + 4 * i - 408, strings_at + len(strings))
        strings += folder.encode() + b'\0'
    struct.pack_into('<ii', header, 204, len(textures), texture_at)
    struct.pack_into('<ii', header, 212, len(folders), cd_at)
    data = bytes(header + body + strings)
    return data[:76] + struct.pack('<i', len(data)) + data[80:]


class StageTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        base = Path(self.temp.name)
        self.na, self.lab, self.out = base / 'na', base / 'lab', base / 'out'
        files = {
            'models/player/miri/pc_miri_body.mdl': model(['miri_skin', 'shared_eye'], ['models/player/miri/', 'models/player/fiona/']),
            'materials/models/player/miri/miri_skin.vmt': b'"VertexLitGeneric" { "$basetexture" "models/player/miri/miri_skin" }',
            'materials/models/player/fiona/miri_skin.vmt': b'"VertexLitGeneric" { }',
            'materials/models/player/miri/miri_skin.vtf': b'VTF\0',
            'materials/models/player/fiona/shared_eye.vmt': b'"VertexLitGeneric" { "$basetexture" "models/player/fiona/eye" }',
            'models/weapon/draker/draker_test.mdl': model([], []),
            'scripts/weapon_draker.txt': b'WeaponData {}',
            'sound/voice/npc/relationship/miri/hi.wav': b'RIFF',
            'csf/jajp_miri_draker_03.csf': b'x',
            'scripts/player_costume.txt': b'"miri" {}',
        }
        for path, data in files.items():
            (self.na / path).parent.mkdir(parents=True, exist_ok=True)
            (self.na / path).write_bytes(data)
        # The 1.69 already has Fiona's eye texture.
        (self.lab / 'materials/models/player/fiona').mkdir(parents=True)
        (self.lab / 'materials/models/player/fiona/eye.vtf').write_bytes(b'VTF\0')
        (self.lab / 'scripts').mkdir()
        (self.lab / 'scripts/player_costume.txt').write_bytes(b'"delia" {}')

    def tearDown(self):
        self.temp.cleanup()

    def test_selection_closure_and_manifest(self):
        manifest = stage.stage('miri', 'draker', self.na, self.lab, self.out)
        paths = {f['path'] for f in manifest['files']}
        self.assertEqual(paths, {
            'models/player/miri/pc_miri_body.mdl', 'materials/models/player/miri/miri_skin.vmt',
            'materials/models/player/miri/miri_skin.vtf', 'materials/models/player/fiona/shared_eye.vmt',
            'models/weapon/draker/draker_test.mdl', 'scripts/weapon_draker.txt',
            # Selected by its name (NA keeps Miri textures in Fiona's folder), not by the closure.
            'materials/models/player/fiona/miri_skin.vmt'})
        self.assertEqual(manifest['missing'], [])
        self.assertEqual(json.loads((self.out / stage.MANIFEST).read_text())['class'], 'miri')
        self.assertTrue((self.out / 'materials/models/player/miri/miri_skin.vtf').is_file())
        # Shared scripts the 1.69 has are kept whole for the merge, not staged as files.
        self.assertEqual([(m['path'], m['needles']) for m in manifest['merge']],
                         [('scripts/player_costume.txt', ['miri', 'draker'])])
        self.assertEqual((self.out / 'merge/scripts/player_costume.txt').read_bytes(), b'"miri" {}')

    def test_the_grid_gets_the_na_icon_and_the_eu_intro_video(self):
        import io  # noqa: PLC0415
        from PIL import Image  # noqa: PLC0415
        import gfx_patch  # noqa: PLC0415
        import test_gfx_patch  # noqa: PLC0415
        # the NA grid with the class's own button on the NA atlas
        movie = gfx_patch.with_class_button(test_gfx_patch.grid_movie(), 'Miri', 'NewCharacterDialog_V2_IA.dds',
                                            (70, 136), (144, 140))
        atlas = io.BytesIO()
        Image.new('RGBA', (128, 256), (10, 200, 30, 255)).save(atlas, 'DDS', pixel_format='DXT5')
        files = {stage.GRID_MOVIE: movie, 'scaleform/newcharacterdialog_v2_ia.dds': atlas.getvalue(),
                 'scaleform/video/createcharacter/_usa_newcharacter_miri.usm': b'CRID usa'}
        for path, data in files.items():
            (self.na / path).parent.mkdir(parents=True, exist_ok=True)
            (self.na / path).write_bytes(data)
        records = {f['path']: f for f in stage.stage('miri', 'draker', self.na, self.lab, self.out)['files']}
        icon = records['scaleform/newcharacterdialog_v2_imiri.dds']
        self.assertEqual((icon['source'], icon['rect']), ('scaleform/newcharacterdialog_v2_ia.dds', [0, 0, 70, 136]))
        staged = Image.open(self.out / icon['path']).convert('RGBA')
        self.assertEqual(staged.size, gfx_patch.ICON_ATLAS_SIZE)
        self.assertEqual(staged.getpixel((35, 68))[3], 255)
        self.assertEqual(staged.getpixel((100, 200))[3], 0)
        alias = records['scaleform/video/createcharacter/_eneu_newcharacter_miri.usm']
        self.assertEqual(alias['copied_from'], 'scaleform/video/createcharacter/_usa_newcharacter_miri.usm')
        self.assertEqual((self.out / alias['path']).read_bytes(), b'CRID usa')

    def test_v45_models_are_converted(self):
        v44 = model(['miri_skin'], ['models/player/miri/'])
        path = b'player\\miri\\pc_miri_body.mdl'
        v45 = bytearray(v44[:408] + path + bytes(260 - len(path)) + v44[408:])
        struct.pack_into('<i', v45, 4, 45)
        struct.pack_into('<i', v45, 76, len(v45))
        for at in (208, 216):
            struct.pack_into('<i', v45, at, struct.unpack_from('<i', v45, at)[0] + 260)
        cd_at = struct.unpack_from('<i', v45, 216)[0]
        struct.pack_into('<i', v45, cd_at, struct.unpack_from('<i', v45, cd_at)[0] + 260)
        (self.na / 'models/player/miri/pc_miri_body.mdl').write_bytes(bytes(v45))
        manifest = stage.stage('miri', 'draker', self.na, self.lab, self.out)
        record = next(f for f in manifest['files'] if f['path'].endswith('pc_miri_body.mdl'))
        self.assertTrue(record['converted'])
        self.assertEqual((self.out / record['path']).read_bytes(), v44)

    def test_assets_named_by_the_weapon_native_code_are_staged(self):
        cable = stage.NATIVE_ASSETS['draker'][0]
        (self.na / cable).parent.mkdir(parents=True, exist_ok=True)
        (self.na / cable).write_bytes(b'"UnlitGeneric" { "$basetexture" "cable/heroes_metalchain_05" }')
        (self.na / 'materials/cable/heroes_metalchain_05.vtf').write_bytes(b'VTF\0')
        (self.na / 'materials/cable/heroes_metalchain_04.vmt').write_bytes(b'"UnlitGeneric" { }')
        paths = {f['path'] for f in stage.stage('miri', 'draker', self.na, self.lab, self.out)['files']}
        self.assertIn(cable, paths)
        self.assertIn('materials/cable/heroes_metalchain_05.vtf', paths)
        self.assertNotIn('materials/cable/heroes_metalchain_04.vmt', paths)
        self.assertNotIn(cable, {f['path'] for f in stage.stage('miri', 'other', self.na, self.lab, self.out / 'b')['files']})

    def test_names_are_checked(self):
        with self.assertRaises(stage.StageError):
            stage.stage('../x', 'draker', self.na, self.lab, self.out)

    def test_an_extreme_motion_stub_stands_for_the_battle_motion(self):
        def mdl(sequences):
            data = bytearray(model([], []))
            struct.pack_into('<i', data, 0xbc, sequences)
            return bytes(data)
        path = 'models/player/_anim/female/miri/pc_miri_battle_motion_ex.mdl'
        self.assertEqual(stage.extreme_motion_sibling(path, mdl(1)),
                         'models/player/_anim/female/miri/pc_miri_battle_motion.mdl')
        self.assertIsNone(stage.extreme_motion_sibling(path, mdl(509)))
        self.assertIsNone(stage.extreme_motion_sibling(path.replace('_ex', ''), mdl(1)))

    def test_action_state_includes_the_lab_lacks_are_dropped(self):
        script = (b'"PlayerActionState"\r\n{\r\n\t"Header"\r\n\t{\r\n'
                  b'\t\t"include"\t"scripts/actionstate/common_state_movement.txt"\r\n'
                  b'\t\t"include"\t"scripts/actionstate/common_state_rope.txt" // rope\r\n'
                  b'\t\t"include_condition" "death_match scripts/actionstate/common_state_pvp.txt"\r\n\t}\r\n}\r\n')
        known = {'scripts/actionstate/common_state_movement.txt'}.__contains__
        out = stage.without_missing_includes('scripts/actionstate/player_action_state_miri.txt', script, known)
        self.assertIn(b'common_state_movement.txt', out)
        self.assertNotIn(b'common_state_rope.txt', out)
        self.assertIn(b'"include_condition"', out)
        self.assertEqual(stage.without_missing_includes('scripts/weapon_draker.txt', script, known), script)

    def test_a_battle_ex_model_without_the_jump_includes_the_donor(self):
        path = 'models/player/_anim/female/miri/pc_miri_battle_motion_ex.mdl'
        donor = stage.JUMP_DONORS['female']
        without = models.synthetic({'battle_idle_lower': {3: models.still(2, bone=3)}})
        with_jump = models.synthetic({stage.JUMP_SEQUENCE: {3: models.still(2, bone=3)}})
        self.assertEqual(stage.jump_donor(path, without, {donor}.__contains__), donor)
        self.assertIsNone(stage.jump_donor(path, with_jump, {donor}.__contains__))
        self.assertIsNone(stage.jump_donor(path, without, lambda p: False))
        self.assertIsNone(stage.jump_donor(path.replace('female', 'male'), without, lambda p: True))
        self.assertIsNone(stage.jump_donor(path.replace('_ex', ''), without, lambda p: True))

    def test_an_abandoned_extreme_mode_stages_the_full_moveset(self):
        """The NA _ex stub model, state and events scripts (another class's copies) give way to the full
        ones; the events gain the donor's blocks of the sequences the donor model supplies."""
        anim = 'models/player/_anim/female/miri/'
        still = {3: models.still(2, bone=3)}
        na = {
            anim + 'pc_miri_battle_motion_ex.mdl': models.synthetic({'idle': still}),
            anim + 'pc_miri_battle_motion.mdl': models.synthetic({'battle_idle_lower': still, 'get_item': still}),
            anim + 'pc_miri_motion.txt': b'"get_item"\r\n{\r\n\t"30"\t"AE_CUSTOM_MOTION\tItem_Loot"\r\n}\r\n',
            anim + 'pc_miri_motion_ex.txt': b'"delia_sp1"\r\n{\r\n}\r\n',
            'scripts/actionstate/player_action_state_miri.txt':
                b'"PlayerActionState" {\r\n"Header" {\r\n\t"include"\t"scripts/actionstate/common_state_movement.txt"\r\n'
                b'\t"include"\t"scripts/actionstate/common_state_item.txt"\r\n}\r\n"miri" {} }',
            'scripts/actionstate/player_action_state_miri_ex.txt': b'"PlayerActionState" { "delia" {} }',
        }
        for path, data in na.items():
            (self.na / path).parent.mkdir(parents=True, exist_ok=True)
            (self.na / path).write_bytes(data)
        for name in ('common_state_movement.txt', 'common_state_movement_ex.txt', 'common_state_item.txt'):
            (self.lab / 'scripts/actionstate').mkdir(parents=True, exist_ok=True)
            (self.lab / 'scripts/actionstate' / name).write_bytes(b'"PlayerActionState" {}')
        donor = self.lab / stage.JUMP_DONORS['female']
        donor.parent.mkdir(parents=True)
        donor.write_bytes(models.synthetic({stage.JUMP_SEQUENCE: still, 'search_evil_core': still, 'get_item': still}))
        (donor.parent / 'pc_delia_motion_ex.txt').write_bytes(
            b'// "comment" { }\r\n"get_item"\r\n{\r\n\t"0"\t"AE_KEY_CLEAR"\r\n}\r\n'
            b'"search_evil_core"\r\n{\r\n\t"92"\t"AE_CUSTOM_MOTION\tItem_Loot"\r\n\t"$endframe-10$"\r\n\t{\r\n\t\t"event"\t"x"\r\n\t}\r\n}\r\n'
            b'"jump_begin"\r\n{\r\n}\r\n')
        manifest = stage.stage('miri', 'draker', self.na, self.lab, self.out)
        records = {r['path']: r for r in manifest['files']}
        states = 'scripts/actionstate/player_action_state_miri_ex.txt'
        # her full script, with the 1.69's _ex movement (the jumps) where the 1.69 has one
        self.assertEqual((self.out / states).read_bytes(), na['scripts/actionstate/player_action_state_miri.txt'].replace(
            b'common_state_movement.txt', b'common_state_movement_ex.txt'))
        self.assertEqual(records[states]['copied_from'], 'scripts/actionstate/player_action_state_miri.txt')
        events = (self.out / anim / 'pc_miri_motion_ex.txt').read_bytes()
        self.assertTrue(events.startswith(na[anim + 'pc_miri_motion.txt'].rstrip()))
        self.assertEqual(sorted(stage.event_blocks(events)), ['get_item', 'jump_begin', 'search_evil_core'])
        self.assertIn(b'"92"\t"AE_CUSTOM_MOTION\tItem_Loot"', events)
        self.assertNotIn(b'AE_KEY_CLEAR', events)  # the class's own get_item stays
        self.assertNotIn(b'delia_sp1', events)
        self.assertEqual(records[anim + 'pc_miri_motion_ex.txt']['events_added'], 2)
        self.assertEqual(stage.extreme_scripts(anim + 'pc_miri_battle_motion_ex.mdl'),
                         {states: 'scripts/actionstate/player_action_state_miri.txt',
                          anim + 'pc_miri_motion_ex.txt': anim + 'pc_miri_motion.txt'})

    def test_effects_bring_their_materials(self):
        efx = b'"particle"\r\n{\r\n\t"material"\t\t"effects/fire_ani_03"\r\n\t"model" "models/effects/x.mdl"\r\n}\r\n'
        self.assertEqual(stage.dependencies('effects/player/miri/sp3.efx', efx),
                         ['materials/effects/fire_ani_03.vmt', 'models/effects/x.mdl'])

    def test_pbs_materials_become_heroes_armor(self):
        vmt = (b'"Heroes_PBS_Texture_Transition"\r\n{\r\n\t"$basetexture" "m\\black"\r\n\t"$bumpmap" "m\\normal"\r\n'
               b'\t"$alphatest" 1\r\n\t"$metallic" 0\r\n\t"$transition_map" "m\\knight"\r\n}\r\n'
               b'// "Heroes_Armor"\r\n// {\r\n//\t"$basetexture" "m\\old"\r\n// }\r\n')
        out = stage.pbs_as_armor('materials/m/knight.vmt', vmt).decode()
        self.assertTrue(out.startswith('"Heroes_Armor"'))
        self.assertIn('"$basetexture" "m\\knight"', out)
        self.assertIn('"$bumpmap" "m\\normal"', out)
        self.assertIn('"$alphatest" "1"', out)
        self.assertNotIn('metallic', out)
        self.assertNotIn('old', out)
        plain = b'"Heroes_Armor"\r\n{\r\n}\r\n'
        self.assertEqual(stage.pbs_as_armor('materials/m/a.vmt', plain), plain)
        self.assertEqual(stage.pbs_as_armor('models/m/a.mdl', vmt), vmt)

    def test_the_dragon_knight_look_keeps_the_held_weapon(self):
        csf = (b'"costume"\r\n{\r\n\t"upper_body"\r\n\t{\r\n\t\t"sn"\t\t"500152"\r\n\t}\r\n'
               b'\t"weapon"\r\n\t{\r\n\t\t"name"\t\t"weapon_draker"\r\n\t\t"sub_type"\t\t"1033"\r\n\t}\r\n}\r\n')
        out = stage.without_csf_weapon(csf)
        self.assertNotIn(b'weapon', out)
        self.assertIn(b'"500152"', out)
        self.assertEqual([c.key for c in stage.kv_merge.parse(out).children[0].children], ['upper_body'])

    def test_the_dragon_knight_look_wears_the_start_menu_wings(self):
        csf = (b'"costume"\r\n{\r\n\t"upper_body"\r\n\t{\r\n\t\t"sn"\t\t"500152"\r\n\t}\r\n'
               b'\t"rear"\r\n\t{\r\n\t\t"sn"\t\t"-1"\r\n\t\t"color"\t\t"-1 -1 -1"\r\n\t}\r\n}\r\n')
        menu = (b'"costume"\r\n{\r\n\t"rear"\r\n\t{\r\n\t\t"sn"\t\t"500139"\r\n\t\t"color"\t\t"8323072 32512 127"\r\n\t}\r\n'
                b'\t"motion"\r\n\t{\r\n\t\t"sn"\t\t"112"\r\n\t}\r\n}\r\n')
        out = stage.with_csf_rear(csf, menu)
        rear = stage._costume_child(out, 'rear')
        self.assertEqual({c.key: c.value for c in rear.children}, {'sn': '500139', 'color': '8323072 32512 127'})
        self.assertEqual([c.key for c in stage.kv_merge.parse(out).children[0].children], ['upper_body', 'rear'])
        self.assertNotIn(b'motion', out)
        self.assertEqual(stage.with_csf_rear(csf, b'"costume"\r\n{\r\n}\r\n'), csf)

    def test_the_wings_are_the_whole_skeleton_ones(self):
        script = (b'"rear"\r\n{\r\n\t"500139"\r\n\t{\r\n\t\t"category"\t"female"\r\n'
                  b'\t\t"normal"\t"models/rear/dragonknight/miri_dragonknight_wing.mdl"\r\n\t}\r\n}\r\n'
                  b'"upper_body"\r\n{\r\n\t"500152"\r\n\t{\r\n'
                  b'\t\t"normal"\t"models/player/miri/dragonknight/miri_dragonknight.mdl"\r\n\t}\r\n}\r\n')
        out = stage.with_knight_wings(script, 'miri')
        self.assertEqual(out, script.replace(b'models/rear/dragonknight/miri_dragonknight_wing.mdl',
                                             b'models/player/miri/dragonknight/miri_dragonknight_wing.mdl'))
        self.assertIn(b'"models/player/miri/dragonknight/miri_dragonknight.mdl"', out)  # the body keeps its jiggles
        self.assertEqual(stage.with_knight_wings(out, 'miri'), out)

    def test_the_whole_skeleton_wings_draw_the_wing_material(self):
        import struct
        old, new = (d.format(klass='miri') for d in stage.KNIGHT_WING_DIR)
        self.assertEqual(len(old), len(new))
        mdl = bytearray(b'IDST' + bytes(0x200))
        struct.pack_into('<ii', mdl, 0xd4, 1, 0x100)
        struct.pack_into('<i', mdl, 0x100, 0x120)
        mdl[0x120:0x120 + len(old) + 1] = old.encode() + b'\0'
        out = stage.with_material_dir(bytes(mdl), old, new)
        self.assertEqual(out[0x120:0x120 + len(new) + 1], new.encode() + b'\0')
        self.assertEqual(len(out), len(mdl))
        self.assertEqual(stage.with_material_dir(bytes(mdl), old, new[:-2]), bytes(mdl))  # same length only
        vmt = b'"Heroes_Faceskin"\r\n{\r\n\t"$basetexture" "models\\player\\miri\\dragonknight\\miri_dragonknight_wing"\r\n\t"$nocull"\t\t0\r\n}\r\n'
        self.assertIn(b'"$nocull"\t\t1', stage.knight_wing_vmt(vmt))
        self.assertIn(b'miri_dragonknight_wing"', stage.knight_wing_vmt(vmt))

    def test_the_transformation_flames_the_wings(self):
        motion = b'"miri_SP4"\r\n{\r\n\t"240"\t"AE_CL_PLAY_EFFECT\teffects/player/miri/sp4_during_02.efx"\r\n}\r\n"miri_SP5"\r\n{\r\n}\r\n'
        out = stage.with_wing_flames('models/player/_anim/female/miri/pc_miri_motion_ex.txt', motion, 'miri')
        for name in stage.KNIGHT_WING_FLAMES:
            self.assertEqual(out.count(f'\t"240"\t"AE_CL_PLAY_EFFECT\teffects/player/miri/{name}.efx"\r\n'.encode()), 1)
        self.assertEqual(stage.event_blocks(out).keys(), stage.event_blocks(motion).keys())
        self.assertEqual(stage.with_wing_flames('models/player/_anim/female/miri/pc_miri_motion_ex.txt', out, 'miri'), out)
        self.assertEqual(stage.with_wing_flames('scripts/x.txt', motion, 'miri'), motion)

    def test_the_transformation_spends_the_sp_its_state_checks(self):
        states = (b'"Miri"\r\n{\r\n\t"Output"\r\n\t{\r\n\t\t"AttackSP03"\r\n\t\t{\r\n\t\t\t"property"\r\n\t\t\t{\r\n'
                  b'\t\t\t\t"SPGauge_need"\t"250"\r\n\t\t\t}\r\n\t\t}\r\n\t\t"AttackSP04"\r\n\t\t{\r\n\t\t\t"property"\r\n'
                  b'\t\t\t{\r\n\t\t\t\t"SPGauge_check_only"\t\t"200"\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n}\r\n')
        checks = stage.state_sp_checks(states)
        self.assertEqual(checks, {'AttackSP04': 200})
        motion = b'"miri_SP4"\r\n{\r\n\t{\r\n\t\t"event"\t"AE_CUSTOM_MOTION\tREDUCE_DRAGON_KNIGHT_SP_GAUGE"\r\n\t}\r\n}\r\n'
        out = stage.with_sp_reduce('models/player/_anim/female/miri/pc_miri_motion.txt', motion, checks)
        self.assertEqual(out, motion.replace(b'AE_CUSTOM_MOTION\tREDUCE_DRAGON_KNIGHT_SP_GAUGE', b'AE_GAIN_SP_GAUGE\t-200'))
        self.assertEqual(stage.with_sp_reduce('models/player/_anim/female/miri/pc_miri_motion.txt', motion, {}), motion)

    def test_models_bring_their_mesh_files(self):
        mdl = b'IDST' + bytes(0x200)
        known = {'models/a/x.vvd', 'models/a/x.dx90.vtx', 'models/a/x.phy'}.__contains__
        deps = stage.dependencies('models/a/x.mdl', mdl, known)
        self.assertEqual([d for d in deps if not d.startswith('materials/')], ['models/a/x.vvd', 'models/a/x.dx90.vtx', 'models/a/x.phy'])

    def test_imported_costume_models_are_staged(self):
        import sqlite3
        db_path = Path(self.temp.name) / 'miri.db3'
        db = sqlite3.connect(db_path)
        db.execute('CREATE TABLE ItemClassInfo (ItemClass TEXT, Category TEXT)')
        db.execute('CREATE TABLE EquipItemInfo (ItemClass TEXT, EquipClass TEXT, CostumeType INT)')
        db.executemany('INSERT INTO ItemClassInfo VALUES (?, ?)', [('dragon_rise_head', 'HELM'), ('draker_x', 'WEAPON')])
        db.executemany('INSERT INTO EquipItemInfo VALUES (?, ?, ?)', [('dragon_rise_head', 'LIGHT_ARMOR', 790), ('draker_x', 'DRAKER', 33)])
        db.commit(); db.close()
        costume = (b'"head"\r\n{\r\n\t"790"\r\n\t{\r\n\t\t"category"\t"female"\r\n'
                   b'\t\t"normal"\t"models/armor/dragon_rises/pc_female_dragon_rises_head.mdl"\r\n\t\t"broken"\t"null"\r\n\t}\r\n'
                   b'\t"33"\r\n\t{\r\n\t\t"normal"\t"models/other.mdl"\r\n\t}\r\n}\r\n'
                   b'"upper_body"\r\n{\r\n\t"790"\r\n\t{\r\n\t\t"category"\t"male"\r\n\t\t"normal"\t"models/armor/x_male.mdl"\r\n\t}\r\n}\r\n')
        self.assertEqual(stage.imported_costumes('miri', costume, db_path),
                         {'models/armor/dragon_rises/pc_female_dragon_rises_head.mdl'})
        # the tunic's one-piece entry (same section) comes with it (C4 MI24)
        onepiece = costume.replace(b'"category"\t"male"\r\n\t\t"normal"\t"models/armor/x_male.mdl"',
                                   b'"category"\t"female"\r\n\t\t"normal"\t"models/armor/dr/upper.mdl"\r\n\t\t"onepiece"\t"10704"')
        onepiece = onepiece.replace(b'\r\n}\r\n', b'\r\n\t"10704"\r\n\t{\r\n\t\t"normal"\t"models/armor/dr/onepiece.mdl"\r\n\t}\r\n'
                                                  b'\t"10705"\r\n\t{\r\n\t\t"normal"\t"models/armor/dr/unrelated.mdl"\r\n\t}\r\n}\r\n')
        self.assertEqual(stage.imported_costumes('miri', onepiece, db_path),
                         {'models/armor/dragon_rises/pc_female_dragon_rises_head.mdl', 'models/armor/dr/upper.mdl',
                          'models/armor/dr/onepiece.mdl'})

    def test_imported_item_icons_are_staged(self):
        import sqlite3
        db_path = Path(self.temp.name) / 'miri.db3'
        db = sqlite3.connect(db_path)
        db.execute('CREATE TABLE ItemClassInfo (ItemClass TEXT, Icon TEXT, IconFG TEXT, IconBG TEXT)')
        db.execute("INSERT INTO ItemClassInfo VALUES ('dragon_rise_head', 'eq_dragon_rises_female_head', NULL, 'bg_redflame')")
        db.commit(); db.close()
        na = {'scaleform/item_icon/eq_dragon_rises_female_head.tga': 0, 'scaleform/item_icon/bg_redflame.tga': 0,
              'scaleform/item_icon/other.tga': 0}
        self.assertEqual(stage.imported_icons('miri', na, db_path),
                         {'scaleform/item_icon/eq_dragon_rises_female_head.tga', 'scaleform/item_icon/bg_redflame.tga'})
        self.assertEqual(stage.imported_icons('miri', na, Path(self.temp.name) / 'none.db3'), set())

    def test_donor_states_fill_what_the_class_lacks(self):
        mine = (b'"PlayerActionState"\r\n{\r\n\t"Header"\r\n\t{\r\n\t}\r\n'
                b'\t"StateChangeConditionDefine"\r\n\t{\r\n\t\t"JumpAttackStrong"\r\n\t\t{\r\n\t\t\t"priority"\t"9"\r\n\t\t}\r\n\t}\r\n'
                b'\t"StateDefine"\r\n\t{\r\n\t\t"Idle"\r\n\t\t{\r\n\t\t}\r\n\t}\r\n}\r\n')
        donor = (b'"PlayerActionState"\r\n{\r\n\t"StateChangeConditionDefine"\r\n\t{\r\n'
                 b'\t\t"JumpAttack"\r\n\t\t{\r\n\t\t\t"command"\t"+IN_ATTACK/"\r\n\t\t}\r\n'
                 b'\t\t"JumpAttackStrong"\r\n\t\t{\r\n\t\t\t"priority"\t"2"\r\n\t\t}\r\n'
                 b'\t\t"SuperPower"\r\n\t\t{\r\n\t\t}\r\n\t}\r\n'
                 b'\t"StateDefine"\r\n\t{\r\n\t\t"JumpAttack"\r\n\t\t{\r\n\t\t\t"output" { "sequence" "jump_attack" }\r\n\t\t}\r\n\t}\r\n}\r\n')
        out, added = stage.with_donor_states(mine, donor, ('JumpAttack', 'JumpAttackStrong'))
        self.assertEqual(added, ['StateChangeConditionDefine/JumpAttack', 'StateDefine/JumpAttack'])
        root = next(n for n in stage.kv_merge.parse(out).children if n.block)
        sections = {c.key: [x.key for x in c.children] for c in root.children}
        self.assertEqual(sections['StateChangeConditionDefine'], ['JumpAttackStrong', 'JumpAttack'])
        self.assertEqual(sections['StateDefine'], ['Idle', 'JumpAttack'])
        self.assertIn(b'"priority"\t"9"', out)  # the class's own state stays
        self.assertNotIn(b'SuperPower', out)
        self.assertEqual(stage.with_donor_states(out, donor, ('JumpAttack', 'JumpAttackStrong'))[1], [])
        xe = donor.replace(b'"SuperPower"\r\n\t\t{\r\n', b'"SuperPower"\r\n\t\t{\r\n\t\t\t"required_skill"\t"skill_delia_xe_move.1"\r\n'
                                                       b'\t\t\t"state_options" { "is_weapon"\t"weapon_bastardsword" }\r\n')
        out, added = stage.with_donor_states(mine, xe, ('SuperPower',), stage.donor_renames('miri', 'draker'))
        self.assertEqual(added, ['StateChangeConditionDefine/SuperPower'])
        # the class's XE skill and weapon entity, not the donor's
        self.assertIn(b'"required_skill"\t"skill_miri_xe_move.1"', out)
        self.assertIn(b'"is_weapon"\t"weapon_greatsword"', out)
        self.assertNotIn(b'delia', out)
        self.assertNotIn(b'bastard', out)

    def test_the_dragon_is_a_summoned_model_with_its_own_effects(self):
        motion = (b'"miri_SP5_attack"\r\n{\r\n'
                  b'\t"0"\t"AE_CUSTOM_MOTION_CLIENT\tPLAY_DRAGON_EFFECT\teffects/player/miri/SP5_dragon_body.efx"\r\n'
                  b'\t"0"\t"AE_CUSTOM_WEAPON_EVENT_INDP\tSUMMON_DRAGON"\r\n'
                  b'\t"$endframe-1$"\r\n\t{\r\n\t\t"event"\t"AE_CUSTOM_WEAPON_EVENT_INDP\tRETURN_DRAGON"\r\n'
                  b'\t\t"is_finally"\t"1"\r\n\t}\r\n}\r\n')
        out = stage.with_summoned_model('models/player/_anim/female/miri/pc_miri_motion_ex.txt', motion, 'miri')
        model = b'models\\effects\\mesh\\pc_miri_sp5_dragon.mdl'
        self.assertIn(b'"AE_CREATE_ATTACKABLE_MODEL\t' + model + b' dragon_attack 0.0"', out)
        self.assertIn(b'"AE_DELETE_ATTACKABLE_MODEL\t' + model + b' 0.0"', out)
        self.assertNotIn(b'DRAGON_EFFECT', out)
        self.assertNotIn(b'_DRAGON"', out)
        self.assertNotIn(b'thorn', out)  # the MI39 probe is gone
        self.assertEqual(out.count(b'"is_finally"\t"1"'), 1)
        self.assertEqual(stage.summoned_model_events(motion),
                         b'"dragon_attack"\r\n{\r\n\t"0"\t"AE_CL_PLAY_EFFECT\teffects/player/miri/SP5_dragon_body.efx"\r\n}\r\n')
        self.assertIsNone(stage.summoned_model_events(b'"x" { }'))
        # the eyes: the lab glow on each eye attachment
        eyes = stage.summoned_model_events(motion, 'miri')
        for eye in ('LEFT_EYE', 'RIGHT_EYE'):
            self.assertIn(f'\t"0"\t"AE_CL_PLAY_EFFECT\teffects/player/miri/pc_miri_sp5_dragon_eye.efx attachment_1 {eye}"\r\n'
                          .encode(), eyes)
        efx = stage.SUMMONED_EYE_EFX
        self.assertIn('"material"\t"effects/redeye1.vmt"', efx)
        self.assertIn('"follow_parent_1"\t"1"', efx)
        self.assertNotIn('attachment_1', efx)
        self.assertEqual(efx.count('{'), efx.count('}'))

    def test_the_battle_sprint_ends_in_battle_idle(self):
        script = (b'"battle_run_sprint"\r\n{\r\n\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tSprintStop"\r\n'
                  b'\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tSprintStopGamePad"\r\n}\r\n'
                  b'"battle_run_sprint_minigame"\r\n{\r\n\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tSprintStop"\r\n}\r\n')
        out = stage.without_sprint_stop('models/player/_anim/female/miri/pc_miri_motion_ex.txt', script)
        sprint = out[slice(*stage.event_blocks(out)['battle_run_sprint'])]
        self.assertNotIn(b'SprintStop', sprint)
        self.assertEqual(sprint.count(b'CONDITION\tBattleIdle"'), 2)
        self.assertIn(b'SprintStop', out[slice(*stage.event_blocks(out)['battle_run_sprint_minigame'])])
        self.assertEqual(stage.without_sprint_stop('scripts/x.txt', script), script)

    def test_ref_group_activations_come_from_the_donor(self):
        group = lambda *states: (b'"$ref_group$"\r\n{\r\n\t"common_idle_run_active_states"\r\n\t{\r\n'
                                 + b''.join(b'\t\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\t%s"\r\n' % s for s in states)
                                 + b'\t}\r\n}\r\n')
        mine, donor = group(b'Kick'), group(b'Kick', b'SearchEvilCore', b'Other')
        out, added = stage.with_ref_group_activations(mine, donor)
        self.assertEqual(added, ['common_idle_run_active_states/SearchEvilCore'])
        self.assertEqual(out, group(b'Kick', b'SearchEvilCore'))
        self.assertEqual(stage.with_ref_group_activations(out, donor)[1], [])

    def test_jump_activations_come_from_the_donor(self):
        mine = (b'"battle_idle_lower"\r\n{\r\n\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tBattleRun"\r\n}\r\n'
                b'"battle_run_lower"\r\n{\r\n\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpBegin"\r\n}\r\n'
                b'"miri_only"\r\n{\r\n}\r\n')
        donor = (b'"battle_idle_lower"\r\n{\r\n\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpBegin"\r\n'
                 b'\t"5"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpAttack"\r\n\t"$endframe$"\r\n\t{\r\n'
                 b'\t\t"event"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpTown"\r\n\t}\r\n}\r\n'
                 b'"battle_run_lower"\r\n{\r\n\t"3"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpBegin"\r\n}\r\n')
        out, opened = stage.with_jump_activations(mine, donor)
        self.assertEqual(opened, ['battle_idle_lower'])
        idle = out[slice(*stage.event_blocks(out)['battle_idle_lower'])]
        self.assertIn(b'\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpBegin"\r\n\t"5"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tJumpAttack"\r\n}', idle)
        self.assertNotIn(b'JumpTown', out)  # nested "event" lines are not taken
        self.assertEqual(out[slice(*stage.event_blocks(out)['battle_run_lower'])],
                         mine[slice(*stage.event_blocks(mine)['battle_run_lower'])])
        self.assertEqual(stage.with_jump_activations(out, donor)[1], [])
        xe = donor.replace(b'JumpBegin"\r\n}', b'JumpBegin"\r\n\t"0"\t"AE_ACTIVATE_STATE_CHANGE_CONDITION\tSuperPower"\r\n}')
        out, opened = stage.with_jump_activations(out, xe)
        self.assertEqual(opened, ['battle_run_lower'])  # the XE move opens where the donor's does
        self.assertIn(b'SuperPower', out[slice(*stage.event_blocks(out)['battle_run_lower'])])

    def test_the_sp_skill_section_takes_the_169_shape(self):
        na = (b'"delia"\r\n{\r\n\t"0"\r\n\t{\r\n\t\t"skill_delia_sp1" { "key" "IN_SPSKILL2" "id" "AttackSP01" }\r\n\t}\r\n}\r\n'
              b'"miri"\r\n{\r\n\t"0"\r\n\t{\r\n'
              b'\t\t"skill_miri_sp1"\r\n\t\t{\r\n\t\t\t"key"\t"IN_SPSKILL2"\r\n\t\t\t"id"\t"AttackSP01"\r\n\t\t}\r\n'
              b'\t\t// vocation\r\n\t\t"skill_voc_dk_active_throw_attack" { "key" "IN_SPSKILL12" "id" "SP_DarkKnight_ThrowAttack" }\r\n'
              b'\t}\r\n}\r\n"grimden" { "0" { } }\r\n')
        lab = b'"delia"\r\n{\r\n\t"skill_delia_sp1" { "key" "IN_SPSKILL2" "id" "AttackSP01" }\r\n}\r\n'
        out = stage.spskill_for_169(na, lab, 'miri')
        miri = next(n for n in stage.kv_merge.parse(out).children if n.key == 'miri')
        self.assertEqual([c.key for c in miri.children], ['skill_miri_sp1'])
        self.assertEqual({k.key: k.value for k in miri.children[0].children}, {'key': 'IN_SPSKILL2', 'id': 'AttackSP01'})
        self.assertNotIn(b'skill_voc_dk', out)
        self.assertEqual(out.replace(out[out.find(b'"miri"'):out.find(b'"grimden"')], b''),
                         na.replace(na[na.find(b'"miri"'):na.find(b'"grimden"')], b''))
        self.assertEqual(stage.spskill_for_169(lab, lab, 'delia'), lab)  # already flat

    def test_town_motion_models_get_the_jump_on_the_back(self):
        data = models.town_model()
        path = 'models/player/_anim/female/miri/pc_miri_town_motion.mdl'
        self.assertNotEqual(stage.town_motion(path, data), data)
        self.assertEqual(stage.town_motion(path.replace('town', 'battle'), data), data)
        donor = models.town_model(rh_jump_in_hand=False)
        with_donor = stage.town_motion(path, data, donor)
        self.assertNotEqual(with_donor, stage.town_motion(path, data))
        self.assertLess(max(mdl_anim.max_drift(with_donor, mdl_anim.TOWN_JUMP, mdl_anim.WEAPON_BONE, mdl_anim.BACK_CARRIER,
                                               mdl_anim.TOWN_REFERENCE)), mdl_anim.PINNED)


if __name__ == '__main__':
    unittest.main()
