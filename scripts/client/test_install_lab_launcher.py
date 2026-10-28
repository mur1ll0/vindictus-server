import tempfile
import unittest
from pathlib import Path

from install_lab_launcher import install, launcher
from prepare_lab_client import PreparationError


class InstallLabLauncherTest(unittest.TestCase):
    def test_writes_once_and_keeps_start_bat(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'sql').mkdir()
            (root / 'sql/heroes.db3').write_bytes(b'')
            (root / 'vindictus.exe').write_bytes(b'')
            (root / 'start.bat').write_bytes(b'start vindictus.exe\r\n')
            self.assertTrue(install(root, Path('C:/lab'), Path('C:/lab/edits.json'))['changed'])
            self.assertFalse(install(root, Path('C:/lab'), Path('C:/lab/edits.json'))['changed'])
            text = (root / 'start-lab.bat').read_bytes().decode('ascii')
            self.assertIn('--apply --edits', text)
            self.assertLess(text.index('refresh_item_content.py'), text.index('apply_lab_content_additions.py'))
            self.assertTrue(text.rstrip().endswith('-noupdate -threads 4'))
            self.assertEqual((root / 'start.bat').read_bytes(), b'start vindictus.exe\r\n')

    def test_refuses_a_folder_without_the_client(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(PreparationError):
                install(Path(tmp))

    def test_launcher_uses_crlf(self):
        self.assertIn('\r\n', launcher(Path('C:/lab'), Path('C:/lab/e.json')))


if __name__ == '__main__':
    unittest.main()
