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
        # The developer's published overlay never reaches these tests.
        patcher = mock.patch.object(additions, 'EDITS', pathlib.Path(self.temp.name) / 'no-edits.json')
        patcher.start()
        self.addCleanup(patcher.stop)

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

    def server(self, items, overrides):
        with (mock.patch.object(additions, 'SERVER_DATABASE', self.database),
              mock.patch.object(additions, 'load', return_value=items),
              mock.patch.object(additions, 'load_overrides', return_value=overrides)):
            return additions.connect(self.database)

    def test_base_overrides_change_cells_in_place_and_clones_copy_them(self):
        before = self.database.read_bytes()
        overrides = {'old_inner': {'set': {'EquipItemInfo': {'CostumeType': 30}, 'ItemClassInfo': {'Icon': 'edited'}}}}
        with closing(self.server(ITEMS, overrides)) as db:
            equips = db.execute('Select * From [EquipItemInfo]').fetchall()
            classes = db.execute('SELECT ItemClass, Icon FROM ItemClassInfo').fetchall()
        self.assertEqual(equips[:3], [('a', 1, None), ('old_inner', 30, None), ('old_inner', 30, 'enUS')])
        # The clone takes the edited base cells, then its own changes.
        self.assertEqual(equips[3:], [('new_inner', 16, None), ('new_inner', 16, 'enUS')])
        self.assertEqual(classes, [('a', 'a'), ('old_inner', 'edited'), ('new_inner', 'edited')])
        self.assertEqual(self.database.read_bytes(), before)

    def test_override_of_a_missing_or_added_item_is_refused(self):
        for overrides in ({'missing': {'set': {'ItemClassInfo': {'Icon': 'x'}}}},
                          {'new_inner': {'set': {'ItemClassInfo': {'Icon': 'x'}}}}):
            with self.assertRaises(ValueError):
                self.server(ITEMS, overrides)

    def test_set_id_adds_a_set_row(self):
        with closing(sqlite3.connect(self.database)) as db:
            db.execute('CREATE TABLE SetItemInfo(ItemClass TEXT, BaseItemClass TEXT, SetID TEXT)')
            db.execute("INSERT INTO SetItemInfo VALUES('a', 'a', 'set_a')")
            db.commit()
        with closing(self.server([{**ITEMS[0], 'set_id': 'set_a'}], {})) as db:
            self.assertEqual(db.execute('SELECT * FROM SetItemInfo').fetchall(),
                             [('a', 'a', 'set_a'), ('new_inner', 'new_inner', 'set_a')])
        with self.assertRaises(ValueError):
            self.server([{**ITEMS[0], 'set_id': 'set_b'}], {})

    def test_panel_items_replace_and_remove_file_items(self):
        path = pathlib.Path(self.temp.name) / 'items.json'
        path.write_text(json.dumps({'items': ITEMS + [{**ITEMS[0], 'ItemClass': 'gone'}]}), encoding='utf-8')
        edits = {'items': [{**ITEMS[0], 'set': {'EquipItemInfo': {'CostumeType': 17}}}], 'removed_items': ['gone'],
                 'item_overrides': {'a': {'set': {'ItemClassInfo': {'Price': 5}}, 'text': {'name': 'Renamed'}}},
                 'hidden_items': ['a']}
        items = additions.load(path, edits)
        self.assertEqual([(i['ItemClass'], i['set']['EquipItemInfo']['CostumeType']) for i in items], [('new_inner', 17)])
        self.assertEqual(additions.hidden_items(edits), ['a'])
        self.assertEqual(additions.text_entries(items, additions.load_overrides(edits))['HEROES_ITEM_NAME_A'], 'Renamed')
        self.assertEqual(additions.load(path)[0]['set']['EquipItemInfo']['CostumeType'], 16)
        for bad in ({'item_overrides': {'a': {'set': {'ItemClassInfo': {'ItemClass': 'b'}}}}},
                    {'item_overrides': {'a': {'set': {'ShopInfo': {'Price': 1}}}}},
                    {'hidden_items': ['Bad Name']}):
            with self.assertRaises(ValueError):
                additions.panel_edits(bad)

    def test_client_overrides_are_undone_and_removed_items_leave(self):
        original = pathlib.Path(self.temp.name) / 'original.db3'
        with closing(sqlite3.connect(self.database)) as db, closing(sqlite3.connect(original)) as copy:
            db.backup(copy)
        overrides = {'old_inner': {'set': {'EquipItemInfo': {'CostumeType': 30}}}}
        with closing(sqlite3.connect(self.database)) as db, closing(sqlite3.connect(original)) as base:
            additions.apply_client(db, ITEMS, overrides, base, {})
            self.assertEqual(db.execute("SELECT CostumeType FROM EquipItemInfo WHERE ItemClass='old_inner'").fetchall(), [(30,), (30,)])
            additions.apply_client(db, [], {}, base, {'added': ['new_inner'], 'overridden': ['old_inner']})
            self.assertEqual(db.execute("SELECT CostumeType FROM EquipItemInfo WHERE ItemClass='old_inner'").fetchall(), [(24,), (24,)])
            self.assertEqual(db.execute("SELECT count(*) FROM ItemClassInfo WHERE ItemClass='new_inner'").fetchone(), (0,))

    def test_weapon_and_shield_costumes_need_no_body(self):
        path = pathlib.Path(self.temp.name) / 'items.json'
        weapon = {**ITEMS[0], 'costumes': [{'section': 'weapon_longsword', 'key': '10007',
                                            'normal': 'models/weapon/longsword/new.mdl'}]}
        path.write_text(json.dumps({'items': [weapon]}), encoding='utf-8')
        self.assertEqual(additions.load(path)[0]['costumes'][0]['section'], 'weapon_longsword')
        inner = {**ITEMS[0], 'costumes': [{'section': 'inner_armor', 'key': '9102', 'normal': 'models/player/evy/new.mdl'}]}
        path.write_text(json.dumps({'items': [inner]}), encoding='utf-8')
        with self.assertRaises(ValueError):
            additions.load(path)

    def test_digest_follows_the_item_content(self):
        self.assertEqual(additions.digest(ITEMS, {}), additions.digest(json.loads(json.dumps(ITEMS)), {}))
        self.assertNotEqual(additions.digest(ITEMS, {}), additions.digest(ITEMS, {'a': {'set': {}}}))


if __name__ == '__main__':
    unittest.main()
