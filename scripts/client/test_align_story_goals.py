import contextlib, pathlib, sqlite3, sys, tempfile, unittest
from unittest import mock

import align_story_goals

SCHEMA = 'CREATE TABLE StoryGoalInfo(StoryLine TEXT,Phase TEXT,[Group] INT,Type TEXT,Command TEXT,Arg TEXT);'


def write(path, rows):
    with contextlib.closing(sqlite3.connect(path)) as db:
        db.executescript(SCHEMA)
        db.executemany('INSERT INTO StoryGoalInfo VALUES (?,?,?,?,?,?)', rows)
        db.commit()


def read(path):
    with contextlib.closing(sqlite3.connect(path)) as db:
        return sorted(db.execute('SELECT * FROM StoryGoalInfo').fetchall())


class AlignStoryGoalsTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = pathlib.Path(tmp.name)
        self.server = self.root / 'server.db3'
        (self.root / 'copy' / 'sql').mkdir(parents=True)
        self.copy = self.root / 'copy' / 'sql' / 'heroes.db3'
        same = ('kept', 'A', 0, 'Action', 'talk_to', 'NPC')
        write(self.server, [same, ('shayla', 'B', 0, 'Condition', 'has_item', 'gold_gnoll_skin 1')])
        write(self.copy, [same, ('shayla', 'B', 0, 'Condition', 'has_item', 'red_gnoll_skin 1'),
                          ('client_only', 'A', 0, 'Action', 'talk_to', 'NPC')])

    def run_main(self, *extra):
        argv = ['align', '--client-copy', str(self.root / 'copy'), '--server-database', str(self.server), *extra]
        with mock.patch.object(sys, 'argv', argv):
            return align_story_goals.main()

    def test_dry_run_keeps_the_copy(self):
        self.assertEqual(self.run_main(), 0)
        self.assertIn(('shayla', 'B', 0, 'Condition', 'has_item', 'red_gnoll_skin 1'), read(self.copy))

    def test_apply_takes_the_server_rows_of_differing_storylines(self):
        self.assertEqual(self.run_main('--apply'), 0)
        rows = read(self.copy)
        self.assertIn(('shayla', 'B', 0, 'Condition', 'has_item', 'gold_gnoll_skin 1'), rows)
        self.assertNotIn(('shayla', 'B', 0, 'Condition', 'has_item', 'red_gnoll_skin 1'), rows)
        self.assertIn(('client_only', 'A', 0, 'Action', 'talk_to', 'NPC'), rows)
        self.assertTrue((self.root / 'copy' / 'sql' / 'heroes.db3.pre-story-goals').exists())
        self.assertEqual(align_story_goals.plan(self.server, self.copy)[1], {})


if __name__ == '__main__':
    unittest.main()
