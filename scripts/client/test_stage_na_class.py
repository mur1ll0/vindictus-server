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

    def test_the_weapon_stays_in_the_hand_in_the_battle_rest(self):
        hand = models.BASE_POS[3]
        off = ((0.3, 0.2, 0.1), (9.0, 6.0, 3.0))  # hanging from the chain, off the hand
        data = models.synthetic({
            stage.HAND_REFERENCE: {2: models.still(2, (0.1, 0.0, 0.0), bone=2), 3: models.still(2, bone=3)},
            'battle_rest': {2: ([(0.1 * f, 0.2, 0) for f in range(5)], [models.BASE_POS[2]] * 5),
                            3: ([off[0]] * 5, [off[1]] * 5)},
            'battle_attack_01': {3: ([off[0]] * 3, [off[1]] * 3)}})
        out, pinned = stage.rest_in_hand(data)
        self.assertEqual(pinned, ['battle_rest'])
        self.assertLess(max(mdl_anim.max_drift(out, 'battle_rest', mdl_anim.WEAPON_BONE, stage.HAND, stage.HAND_REFERENCE)),
                        mdl_anim.PINNED)
        model = mdl_anim.Model(out)
        ad = model.animation('battle_rest')
        self.assertEqual([round(x, 3) for x in model.local(ad, 3, 4)[0]], [round(x, 3) for x in hand])
        self.assertEqual(stage.rest_in_hand(out), (out, []))  # the attacks keep their swing

    def test_town_motion_models_get_the_jump_on_the_back(self):
        data = models.town_model()
        path = 'models/player/_anim/female/miri/pc_miri_town_motion.mdl'
        self.assertNotEqual(stage.town_motion(path, data), data)
        self.assertEqual(stage.town_motion(path.replace('town', 'battle'), data), data)


if __name__ == '__main__':
    unittest.main()
