"""build_client_version.py rules (the builds themselves run against the original; see the session log)."""
import tempfile
import unittest
from pathlib import Path

import build_client_version as recipe
import class_layers
from prepare_lab_client import PreparationError


class RulesTest(unittest.TestCase):
    def test_exclusions_of_the_launcher_plan(self):
        for path in ('hfs_extracted/scripts/a.txt', 'lab-backup/x.hfs', 'passport/a.py', 'BlackCipher/b.dll',
                     'downloads/x', 'DownloadLists/y', 'bin/client.dll.b284ba719047556b.bak',
                     'bin/engine.dll.51ea90e70b93ad04.pre-iocpsign.bak', 'fmod.log', 'npp_api_stdout.log',
                     'nmcogame.dll.nexon', 'NMService.exe', '_eu_AdBalloonExt.exe', 'VindictusLauncher.exe',
                     'bcdedit.exe', 'start.bat', 'start-lab.bat', 'maps/graphs/t01.ain', 'demoheader.tmp',
                     'hfs-keystream.bin', 'l0-test.ps1', 'cfg/config_pendingkey_concmd.cfg', 'bin/class-routing.json',
                     'sql/heroes.db3.pre-lab-additions', 'sql/heroes.db3.lab-items.json', 'nmcogame.ini',
                     'vindictus-launcher.json', 'vindictus-launcher.exe', 'bin/config_material.txt',
                     'scaleform/CreateCharacterDialog.gfx.ac5543b072761a3f.pre-classes.bak',
                     'sql/heroes.db3.0123.before-endpoint.bak', 'bin/client.partial'):
            self.assertTrue(recipe.excluded(path), path)
        for path in ('Vindictus.exe', 'Mss32.dll', 'nmcogame.dll', 'bin/client.dll', 'sql/heroes.db3',
                     'hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs', 'maps/t01.bsp', 'cfg/config_env.cfg',
                     'scaleform/CreateCharacterDialog.gfx', 'hfs/RENOMEADo_9FB527B373DCBD159552ADD1710487A724237AF9.hfs'):
            self.assertFalse(recipe.excluded(path), path)

    def test_player_files_are_preserved(self):
        for path in ('SAVE/hidedate.hdf', 'cfg/config.cfg', 'cfg/config_env.cfg', 'cfg/config_ex.cfg',
                     'cfg/language.cfg', 'cfg/config_miri_ex.cfg'):
            self.assertTrue(recipe.matches(path, recipe.PRESERVE_GLOBS), path)
        self.assertFalse(recipe.matches('cfg/autoexec.cfg', recipe.PRESERVE_GLOBS))

    def test_versions_and_their_dll_checks(self):
        self.assertEqual(class_layers.VERSIONS, {1: (), 2: ('delia',), 3: ('delia', 'miri')})
        self.assertEqual(set(recipe.DLL_RESULTS), set(class_layers.VERSIONS))
        self.assertTrue(all(set(v) == {'client.dll', 'server.dll', 'engine.dll'} for v in recipe.DLL_RESULTS.values()))
        self.assertTrue(all(len(h) == 16 for v in recipe.DLL_RESULTS.values() for h in v.values()))

    def test_only_a_new_folder_is_built(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / 'file').write_bytes(b'x')
            with self.assertRaises(PreparationError):
                recipe.build(1, Path(tmp))
        with self.assertRaises(PreparationError):
            recipe.build(1, recipe.ROOT / '.cache/play-client')
        with self.assertRaises(PreparationError):
            recipe.build(1, recipe.ORIGINAL / 'out')


class LinkTest(unittest.TestCase):
    def test_identical_files_become_read_only_links(self):
        import json
        import os
        import stat
        with tempfile.TemporaryDirectory(dir=recipe.ROOT / '.cache') as tmp:
            old, new = Path(tmp) / 'v1', Path(tmp) / 'v2'
            for folder, data in ((old, b'same'), (new, b'same')):
                (folder / 'bin').mkdir(parents=True)
                (folder / 'bin/a.dll').write_bytes(data)
                (folder / 'b.txt').write_bytes(folder.name.encode())
            files = {'bin/a.dll': {'sha256': recipe.sha256_file(new / 'bin/a.dll'), 'size': 4},
                     'b.txt': {'sha256': recipe.sha256_file(new / 'b.txt'), 'size': 2}}
            other = {'bin/a.dll': files['bin/a.dll'], 'b.txt': {'sha256': recipe.sha256_file(old / 'b.txt'), 'size': 2}}
            (Path(tmp) / 'v1.report.json').write_text(json.dumps({'files': other}), encoding='utf-8')
            self.assertEqual(recipe.link_identical(new, files, old)['files'], 1)
            self.assertTrue(os.path.samefile(old / 'bin/a.dll', new / 'bin/a.dll'))
            self.assertFalse(os.path.samefile(old / 'b.txt', new / 'b.txt'))
            with self.assertRaises(PermissionError):
                (new / 'bin/a.dll').write_bytes(b'x')
            self.assertEqual((old / 'bin/a.dll').read_bytes(), b'same')
            os.chmod(old / 'bin/a.dll', stat.S_IWRITE | stat.S_IREAD)


class CompareTest(unittest.TestCase):
    def test_only_test_item_rows_are_explained(self):
        import sqlite3
        from contextlib import closing
        import compare_client_version as compare
        with tempfile.TemporaryDirectory() as tmp:
            paths = []
            for name, rows in (('built', [('lann_sword',)]), ('lab', [('lann_sword',), ('fiona_inner_lab',)])):
                path = Path(tmp) / f'{name}.db3'
                with closing(sqlite3.connect(path)) as db:
                    db.execute('CREATE TABLE ItemClassInfo(ItemClass TEXT)')
                    db.executemany('INSERT INTO ItemClassInfo VALUES(?)', rows)
                    db.commit()
                paths.append(path)
            diff = compare.database_diff(*paths)
            self.assertEqual((diff['unexplained'], diff['rows']['ItemClassInfo']['only_reference']), (0, 1))
            with closing(sqlite3.connect(paths[1])) as db:
                db.execute("INSERT INTO ItemClassInfo VALUES('other_item')")
                db.commit()
            self.assertEqual(compare.database_diff(*paths)['unexplained'], 1)


if __name__ == '__main__':
    unittest.main()
