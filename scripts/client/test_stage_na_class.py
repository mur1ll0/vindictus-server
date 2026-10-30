import json
import struct
import tempfile
import unittest
from pathlib import Path

import stage_na_class as stage


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

    def test_names_are_checked(self):
        with self.assertRaises(stage.StageError):
            stage.stage('../x', 'draker', self.na, self.lab, self.out)


if __name__ == '__main__':
    unittest.main()
