import contextlib, pathlib, sqlite3, tempfile, unittest

import content_overrides as overrides


class ContentOverrideTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = pathlib.Path(self.tmp.name)
        self.server = base / 'server.db3'
        self.original = base / 'original.db3'
        schema = 'CREATE TABLE QuestInfo(ID TEXT,Feature TEXT,DropItemCountMin INT,DropItemCountMax INT,MaxPlayCount INT,QuestSet INT,DisableUserShip TEXT,EnableUserDSMode TEXT);'
        with contextlib.closing(sqlite3.connect(self.server)) as db:
            db.executescript(schema)
            db.executemany('INSERT INTO QuestInfo VALUES (?,?,?,?,?,?,?,?)', [
                ('quest_a', 'f', 20, 20, None, 1, 'False', 'False'),
                ('quest_new', None, 20, 20, None, 1, 'False', 'False'),
            ])
            db.commit()
        with contextlib.closing(sqlite3.connect(self.original)) as db:
            db.executescript(schema)
            db.execute('INSERT INTO QuestInfo VALUES (?,?,?,?,?,?,?,?)', ('quest_a', 'other', 1, 2, 1, 2000, 'True', 'True'))
            db.commit()
        self.saved = (overrides.ORIGINAL_DATABASE, overrides.ORIGINAL_SHA256)
        overrides.ORIGINAL_DATABASE = self.original
        overrides.ORIGINAL_SHA256 = overrides._digest(self.original)
        overrides._cache.clear()

    def tearDown(self):
        overrides.ORIGINAL_DATABASE, overrides.ORIGINAL_SHA256 = self.saved
        overrides._cache.clear()
        self.tmp.cleanup()

    def rows(self):
        with contextlib.closing(sqlite3.connect(self.server)) as db:
            return list(overrides.select(db, 'QuestInfo'))

    def test_original_columns_replace_only_the_listed_values(self):
        a, new = self.rows()
        # Listed columns come from the original row; others (Feature) stay the server's.
        self.assertEqual((a['DropItemCountMin'], a['DropItemCountMax'], a['MaxPlayCount'], a['QuestSet']), (1, 2, 1, 2000))
        self.assertEqual((a['DisableUserShip'], a['EnableUserDSMode'], a['Feature']), ('True', 'True', 'f'))
        # A row without an original counterpart keeps the server values.
        self.assertEqual((new['ID'], new['DropItemCountMin']), ('quest_new', 20))

    def test_changed_original_or_repeated_key_is_refused(self):
        overrides.ORIGINAL_SHA256 = '0' * 64
        with self.assertRaises(ValueError):
            self.rows()
        with contextlib.closing(sqlite3.connect(self.original)) as db:
            db.execute('INSERT INTO QuestInfo VALUES (?,?,?,?,?,?,?,?)', ('quest_a', None, 1, 1, 1, 1, 'True', 'True'))
            db.commit()
        overrides.ORIGINAL_SHA256 = overrides._digest(self.original)
        overrides._cache.clear()
        with self.assertRaises(ValueError):
            self.rows()


if __name__ == '__main__':
    unittest.main()
