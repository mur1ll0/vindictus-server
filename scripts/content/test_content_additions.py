import json
import pathlib
import sqlite3
import tempfile
import unittest
from contextlib import closing
from unittest import mock

import content_additions as additions

ITEMS = [{'ItemClass': 'new_inner', 'template': 'old_inner', 'set': {'EquipItemInfo': {'CostumeType': 16}},
          'text': {'name': 'New Inner', 'desc': 'Desc'}}]


def make_database(path):
    with closing(sqlite3.connect(path)) as db:
        db.execute('CREATE TABLE ItemClassInfo(ItemClass TEXT, Icon TEXT, Feature TEXT)')
        db.execute('CREATE TABLE EquipItemInfo(ItemClass TEXT, CostumeType INTEGER, Feature TEXT)')
        db.execute('CREATE TABLE CustomizeDefaultInfo(ItemClass TEXT, Color1 INTEGER)')
        db.execute('CREATE TABLE CustomizePriceInfo(ItemClass TEXT, Duration INTEGER, Feature TEXT)')
        db.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?)', [('a', 'a', None), ('old_inner', 'old', None)])
        db.executemany('INSERT INTO EquipItemInfo VALUES(?,?,?)', [('a', 1, None), ('old_inner', 24, None), ('old_inner', 24, 'enUS')])
        db.execute("INSERT INTO CustomizeDefaultInfo VALUES('old_inner', 7)")
        db.commit()


class ContentAdditionsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.database = pathlib.Path(self.temp.name) / 'heroesContents.db3'
        make_database(self.database)

    def tearDown(self):
        self.temp.cleanup()

    def test_server_tables_end_with_the_added_rows_and_the_file_is_unchanged(self):
        before = self.database.read_bytes()
        with mock.patch.object(additions, 'SERVER_DATABASE', self.database), mock.patch.object(additions, 'load', return_value=ITEMS):
            with closing(additions.connect(self.database)) as db:
                db.execute('PRAGMA query_only=ON')
                equips = db.execute('Select * From [EquipItemInfo]').fetchall()
                classes = db.execute('SELECT ItemClass, Icon FROM ItemClassInfo').fetchall()
                prices = db.execute('SELECT * FROM CustomizePriceInfo').fetchall()
        self.assertEqual(equips[:3], [('a', 1, None), ('old_inner', 24, None), ('old_inner', 24, 'enUS')])
        self.assertEqual(equips[3:], [('new_inner', 16, None), ('new_inner', 16, 'enUS')])
        self.assertEqual(classes[-1], ('new_inner', 'old'))
        self.assertEqual(prices, [])
        self.assertEqual(self.database.read_bytes(), before)

    def test_other_databases_are_opened_plain(self):
        with mock.patch.object(additions, 'load', return_value=ITEMS):
            with closing(additions.connect(self.database)) as db:
                self.assertEqual(len(db.execute('SELECT * FROM EquipItemInfo').fetchall()), 3)

    def test_client_rows_are_replaced_not_duplicated(self):
        with closing(sqlite3.connect(self.database)) as db:
            self.assertEqual(additions.apply_client(db, ITEMS), 4)
            additions.apply_client(db, ITEMS)
            rows = db.execute("SELECT CostumeType FROM EquipItemInfo WHERE ItemClass='new_inner'").fetchall()
        self.assertEqual(rows, [(16,), (16,)])

    def test_an_existing_item_class_is_refused(self):
        with mock.patch.object(additions, 'SERVER_DATABASE', self.database), \
                mock.patch.object(additions, 'load', return_value=[{**ITEMS[0], 'ItemClass': 'a'}]):
            with self.assertRaises(ValueError):
                additions.connect(self.database)

    def test_load_validates_items_and_text_keys(self):
        path = pathlib.Path(self.temp.name) / 'items.json'
        path.write_text(json.dumps({'items': ITEMS}), encoding='utf-8')
        self.assertEqual(additions.text_entries(additions.load(path)),
                         {'HEROES_ITEM_NAME_NEW_INNER': 'New Inner', 'HEROES_ITEM_DESC_NEW_INNER': 'Desc'})
        for bad in ({**ITEMS[0], 'ItemClass': 'Bad Name'}, {**ITEMS[0], 'set': {'EquipItemInfo': {'ItemClass': 'x'}}},
                    {**ITEMS[0], 'costumes': [{'section': 'inner_armor', 'key': 'x', 'category': 'female', 'normal': 'models/a.mdl'}]},
                    {**ITEMS[0], 'text': {'name': 'a "quote"'}}):
            path.write_text(json.dumps({'items': [bad]}), encoding='utf-8')
            with self.assertRaises(ValueError):
                additions.load(path)


if __name__ == '__main__':
    unittest.main()
