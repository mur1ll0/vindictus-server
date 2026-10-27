import json
import sqlite3
import tempfile
import unittest
from contextlib import closing
from pathlib import Path

from apply_lab_content_additions import apply
from prepare_lab_client import PreparationError

ROW = {'ShopID': 'FERGHUS_Shop', 'Order': 124, 'ItemClass': 'enhance_elixir', 'Count': 1, 'PriceItemClass': 'gold',
       'PriceCount': 4000, 'AppearQuestID': None, 'CharacterBind': 'False', 'ID': None, 'RestrictionResetTime': None,
       'RestrictionPeriodDay': 0, 'RestrictionCount': 0, 'Feature': None}


class ApplyLabContentAdditionsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / 'sql').mkdir()
        self.db = self.root / 'sql/heroes.db3'
        with closing(sqlite3.connect(self.db)) as db:
            db.execute('CREATE TABLE ShopInfo(ShopID TEXT,[Order] INT,ItemClass TEXT,Count INT,PriceItemClass TEXT,PriceCount INT,'
                       'AppearQuestID TEXT,CharacterBind TEXT,ID INT,RestrictionResetTime TEXT,RestrictionPeriodDay INT,'
                       'RestrictionCount INT,Feature TEXT)')
            db.commit()
        self.additions = self.root / 'additions.json'
        self.additions.write_text(json.dumps({'ShopInfo': [ROW]}), encoding='utf-8')

    def count(self):
        with closing(sqlite3.connect(self.db)) as db:
            return db.execute("SELECT count(*) FROM ShopInfo WHERE ItemClass='enhance_elixir'").fetchone()[0]

    def test_dry_run_changes_nothing_and_apply_is_idempotent(self):
        self.assertEqual(apply(self.root, False, self.additions, items=[])['shop_rows_missing'], 1)
        self.assertEqual(self.count(), 0)
        apply(self.root, True, self.additions, items=[])
        apply(self.root, True, self.additions, items=[])
        self.assertEqual(self.count(), 1)
        self.assertTrue((self.root / 'sql/heroes.db3.pre-lab-additions').exists())

    def test_panel_edits_change_and_remove_rows_and_undo_with_the_backup(self):
        with closing(sqlite3.connect(self.db)) as db:
            db.execute("INSERT INTO ShopInfo(ShopID,[Order],ItemClass,Count,PriceItemClass,PriceCount,CharacterBind,"
                       "RestrictionPeriodDay,RestrictionCount) VALUES('FERGHUS_Shop',1,'longsword',1,'gold',100,'False',0,0)")
            db.commit()
        edits = {'format_version': 1, 'version': 2, 'cash_prices': {}, 'cash_default': None, 'shop_rows': [
            {'ShopID': 'FERGHUS_Shop', 'Order': 1, 'Remove': True},
            {'ShopID': 'FERGHUS_Shop', 'Order': 124, 'ItemClass': 'enhance_elixir', 'Count': 2, 'PriceItemClass': 'gold', 'PriceCount': 3000},
        ]}
        self.assertTrue(apply(self.root, True, self.additions, edits, items=[])['changed'])
        with closing(sqlite3.connect(self.db)) as db:
            rows = db.execute('SELECT [Order],Count,PriceCount FROM ShopInfo ORDER BY [Order]').fetchall()
        self.assertEqual(rows, [(124, 2, 3000)])
        self.assertFalse(apply(self.root, True, self.additions, edits, items=[])['changed'])
        # Without the edits the original row and the addition come back from the backup.
        apply(self.root, True, self.additions, items=[])
        with closing(sqlite3.connect(self.db)) as db:
            rows = db.execute('SELECT [Order],PriceCount FROM ShopInfo ORDER BY [Order]').fetchall()
        self.assertEqual(rows, [(1, 100), (124, 4000)])

    def test_an_order_used_by_another_item_is_refused(self):
        with closing(sqlite3.connect(self.db)) as db:
            db.execute("INSERT INTO ShopInfo(ShopID,[Order],ItemClass) VALUES('FERGHUS_Shop',124,'other')")
            db.commit()
        with self.assertRaises(PreparationError):
            apply(self.root, True, self.additions, items=[])


if __name__ == '__main__':
    unittest.main()
