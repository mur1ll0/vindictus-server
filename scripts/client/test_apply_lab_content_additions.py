import json
import sqlite3
import struct
import tempfile
import unittest
from contextlib import closing
from pathlib import Path

import apply_lab_content_additions as a
from apply_lab_content_additions import apply
from prepare_lab_client import PreparationError

ROW = {'ShopID': 'FERGHUS_Shop', 'Order': 124, 'ItemClass': 'enhance_elixir', 'Count': 1, 'PriceItemClass': 'gold',
       'PriceCount': 4000, 'AppearQuestID': None, 'CharacterBind': 'False', 'ID': None, 'RestrictionResetTime': None,
       'RestrictionPeriodDay': 0, 'RestrictionCount': 0, 'Feature': None}


class MarketplaceTabsTest(unittest.TestCase):
    def test_lab_tabs_open_delia_and_add_miri_after_the_weapons(self):
        original = [('WEAPON', 'PHANTOMDAGGER', 'Hagie'), ('WEAPON', 'BASTARDSWORD', 'Delia'), ('ETC', 'ETC_ETC', None)]
        rows = a.target_categories(original, [
            {'MainCategoryName': 'WEAPON', 'Name': 'BASTARDSWORD', 'Feature': None},
            {'MainCategoryName': 'WEAPON', 'Name': 'DRAKER', 'Feature': None},
            {'MainCategoryName': 'NEW', 'Name': 'X', 'Feature': None}])
        self.assertEqual(rows, [('WEAPON', 'PHANTOMDAGGER', 'Hagie'), ('WEAPON', 'BASTARDSWORD', None),
                                ('WEAPON', 'DRAKER', None), ('ETC', 'ETC_ETC', None), ('NEW', 'X', None)])
        self.assertEqual(a.target_categories(rows, [{'MainCategoryName': 'WEAPON', 'Name': 'DRAKER', 'Feature': None}]), rows)


class ShopPriceTest(unittest.TestCase):
    def test_price_changes_reach_the_client_rows(self):
        empty = a.content_edits.EMPTY
        rows = a.target_rows([ROW], [], empty, [{'ShopID': 'FERGHUS_Shop', 'Order': 124, 'PriceItemClass': 'gold', 'PriceCount': 9}])
        self.assertEqual((rows[0]['PriceItemClass'], rows[0]['PriceCount']), ('gold', 9))
        # A shop the copy lacks is skipped; a missing row of a present shop fails.
        self.assertEqual(a.target_rows([ROW], [], empty, [{'ShopID': 'NONE', 'Order': 1, 'PriceItemClass': 'gold', 'PriceCount': 9}]), [ROW])
        with self.assertRaises(PreparationError):
            a.target_rows([ROW], [], empty, [{'ShopID': 'FERGHUS_Shop', 'Order': 1, 'PriceItemClass': 'gold', 'PriceCount': 9}])


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
        # The developer's imported classes never reach these tests.
        from unittest import mock
        import content_additions
        patcher = mock.patch.object(content_additions, 'CLASS_IMPORTS', self.root / 'no-imports')
        patcher.start()
        self.addCleanup(patcher.stop)

    def count(self):
        with closing(sqlite3.connect(self.db)) as db:
            return db.execute("SELECT count(*) FROM ShopInfo WHERE ItemClass='enhance_elixir'").fetchone()[0]

    def test_class_scripts_join_the_original_pack_of_an_existing_folder(self):
        import hfs  # noqa: PLC0415
        packs = sorted([hfs.pack_name('scripts/actionstate', 0), hfs.pack_name('scripts/actionstate', 1),
                        hfs.pack_name('models/player/fiona', 0)])
        plain = hfs.assemble_index(hfs.INDEX_MAGIC + bytes(16), packs, [])
        files = [('scripts/actionstate/player_action_state_miri.txt', Path('a'), 's1'),
                 ('scripts/newfolder/x.txt', Path('b'), 's2'),
                 ('models/player/fiona/miri_inner.mdl', Path('c'), 's3')]
        out = a.injected_packs(plain, files, Path('copy'))
        # The folder's last original part; a new folder and non-script files keep their own packs.
        self.assertEqual(out, {Path('copy/hfs') / (hfs.pack_name('scripts/actionstate', 1) + '.hfs'):
                               [('scripts/actionstate/player_action_state_miri.txt', Path('a'), 's1')]})

    def test_ui_elements_declare_the_class_dialogs(self):
        xml = ('<UIElementInfo>\r\n  <ElementList>\r\n    <Element name="ShipHPGauge" filename="ShipHPGauge.gfx" />\r\n'
               '    <Element name="Other" filename="Other.gfx" />\r\n  </ElementList>\r\n  <PreloadList>\r\n'
               '    <Element>ShipHPGauge</Element>\r\n  </PreloadList>\r\n  <MoveableDialogs>\r\n'
               '    <Element name="QuickSlotPanel" background="hitBox" />\r\n  </MoveableDialogs>\r\n</UIElementInfo>\r\n').encode()
        out = a.ui_elements_with_classes(xml)
        lines = out.decode().split('\r\n')
        self.assertTrue(lines[3].startswith('    <Element name="MiriSkillDialog" filename="MiriSkillDialog.gfx"'))
        self.assertEqual(lines[8], '    <Element>MiriSkillDialog</Element>')
        self.assertEqual(lines[9], '  </PreloadList>')
        self.assertEqual(lines[12], '    <Element name="MiriSkillDialog" background="MC_DragonEnergy.BG" />')
        self.assertEqual(lines[13], '  </MoveableDialogs>')
        self.assertIsNone(a.ui_elements_with_classes(out))  # already declared
        # hidden on a game state change, as ShipHPGauge (C4 MI27: the dragon stayed on the select screen)
        hiding = xml.replace(b'  <MoveableDialogs>', b'  <HideOnChangeTownState>\r\n    <Element>ShipHPGauge</Element>\r\n'
                             b'  </HideOnChangeTownState>\r\n  <DeactivateInGameUI>\r\n  </DeactivateInGameUI>\r\n  <MoveableDialogs>')
        out = a.ui_elements_with_classes(hiding).decode()
        self.assertIn('<Element>MiriSkillDialog</Element>\r\n  </HideOnChangeTownState>', out)
        self.assertIn('<Element>MiriSkillDialog</Element>\r\n  </DeactivateInGameUI>', out)
        with self.assertRaises(PreparationError):
            a.ui_elements_with_classes(b'<UIElementInfo></UIElementInfo>')

    def test_original_fixes_find_the_pack_of_their_file(self):
        import hfs  # noqa: PLC0415
        folder = 'models/player/_anim/female/delia'
        packs = sorted([hfs.pack_name(folder, 0), hfs.pack_name(folder, 1), hfs.pack_name('models/player/fiona', 0)])
        entry = hfs.name_key('pc_delia_town_motion.mdl') + struct.pack('<H', packs.index(hfs.pack_name(folder, 1))) + bytes(8)
        plain = hfs.assemble_index(hfs.INDEX_MAGIC + bytes(16), packs, [entry])
        fix = a.ORIGINAL_FIXES[folder + '/pc_delia_town_motion.mdl']
        fixes = {folder + '/pc_delia_town_motion.mdl': fix}
        self.assertEqual(a.fixed_packs(plain, Path('copy'), fixes),
                         {Path('copy/hfs') / (hfs.pack_name(folder, 1) + '.hfs'): {'pc_delia_town_motion.mdl': fix}})
        with self.assertRaises(PreparationError):
            a.fixed_packs(hfs.assemble_index(hfs.INDEX_MAGIC + bytes(16), packs, []), Path('copy'), fixes)

    def test_class_layers_keep_only_their_marketplace_tabs_and_excluded_items_stay_out(self):
        with closing(sqlite3.connect(self.db)) as db:
            db.execute('CREATE TABLE SubCategoryInfo(MainCategoryName TEXT, Name TEXT, Feature TEXT)')
            db.executemany('INSERT INTO SubCategoryInfo VALUES(?,?,?)',
                           [('WEAPON', 'BASTARDSWORD', 'Delia'), ('ETC', 'ETC_ETC', None)])
            db.commit()
        self.additions.write_text(json.dumps({'ShopInfo': [ROW], 'SubCategoryInfo': [
            {'MainCategoryName': 'WEAPON', 'Name': 'BASTARDSWORD', 'Feature': None, 'class': 'delia'},
            {'MainCategoryName': 'WEAPON', 'Name': 'DRAKER', 'Feature': None, 'class': 'miri'}]}), encoding='utf-8')

        def tabs():
            with closing(sqlite3.connect(self.db)) as db:
                return list(db.execute('SELECT MainCategoryName, Name, Feature FROM SubCategoryInfo ORDER BY rowid'))

        item = {'ItemClass': 'test_inner', 'template': 'old_inner'}
        plan = apply(self.root, True, self.additions, items=[item], classes=(), exclude_items=('test_inner',))
        self.assertEqual(plan['excluded_items'], ['test_inner'])
        self.assertEqual(plan['items'], 0)
        self.assertEqual(tabs(), [('WEAPON', 'BASTARDSWORD', 'Delia'), ('ETC', 'ETC_ETC', None)])
        apply(self.root, True, self.additions, items=[], classes=('delia',))
        self.assertEqual(tabs(), [('WEAPON', 'BASTARDSWORD', None), ('ETC', 'ETC_ETC', None)])
        apply(self.root, True, self.additions, items=[])
        self.assertEqual(tabs(), [('WEAPON', 'BASTARDSWORD', None), ('WEAPON', 'DRAKER', None), ('ETC', 'ETC_ETC', None)])

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

    def test_hidden_items_leave_the_shops_until_shown_again(self):
        edits = {'format_version': 1, 'version': 3, 'cash_prices': {}, 'cash_default': None, 'shop_rows': [],
                 'hidden_items': ['enhance_elixir']}
        apply(self.root, True, self.additions, edits, items=[])
        self.assertEqual(self.count(), 0)
        apply(self.root, True, self.additions, items=[])
        self.assertEqual(self.count(), 1)

    def test_an_order_used_by_another_item_is_refused(self):
        with closing(sqlite3.connect(self.db)) as db:
            db.execute("INSERT INTO ShopInfo(ShopID,[Order],ItemClass) VALUES('FERGHUS_Shop',124,'other')")
            db.commit()
        with self.assertRaises(PreparationError):
            apply(self.root, True, self.additions, items=[])


if __name__ == '__main__':
    unittest.main()
