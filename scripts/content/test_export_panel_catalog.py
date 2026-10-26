import contextlib, pathlib, sqlite3, tempfile, unittest

from export_panel_catalog import place, read_package


class PanelCatalogTests(unittest.TestCase):
    def test_shops_by_npc_and_place_names_and_icons(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / 'xml/Default').mkdir(parents=True)
            (root / 'xml/Default/Default_Ferghus.xml').write_text(
                '<DefaultPhase><Talk id="FERGHUS" location="t01_bs"><Do><open_shop>\n</open_shop></Do></Talk>'
                '<Talk id="ARTHYEN" location="rochest.store"><open_shop>ARTHYEN_SHOP</open_shop>'
                '<open_shop>MISSING_SHOP</open_shop></Talk></DefaultPhase>', encoding='utf-8')
            db = root / 'heroesContents.db3'
            with contextlib.closing(sqlite3.connect(db)) as c:
                c.execute('CREATE TABLE ShopInfo(ShopID TEXT)')
                c.executemany('INSERT INTO ShopInfo VALUES(?)', [('FERGHUS_Shop',), ('ARTHYEN_SHOP',)])
                c.execute('CREATE TABLE ItemClassInfo(ItemClass TEXT, Icon TEXT, Rarity INT, Category TEXT, IconBG TEXT)')
                c.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?)', [
                    ('enhance_elixir', 'Enhance_Elixir', 1, 'MATERIAL', None), ('bad', 'a/b', 0, None, None),
                    ('fiona_noluck_inner_x', 'fiona_inner_x', 1, 'INNERARMOR', 'fiona_inner'),
                    ('delia_inner_x', 'fiona_inner_x', 1, 'INNERARMOR', 'evy_inner')])
                c.execute('CREATE TABLE EquipItemInfo(ItemClass TEXT, MaxDurability INT, Weight REAL, Dyeable TEXT)')
                c.execute("INSERT INTO EquipItemInfo VALUES('fiona_noluck_inner_x', 0, 24.0, 'True')")
                c.commit()
            text = root / 'heroes_text_english_eu.txt'
            lines = ['"HEROES_NPC_FERGHUS" "Ferghus"', '"GAMEUI_HEROES_NPCDESCRIPTION_FERGHUS" "Line one\\nLine two"',
                     '"HEROES_NPC_ARTHYEN" "한글"', '"HEROES_ITEM_NAME_FIONA_NOLUCK_INNER_X" "한글"',
                     '"HEROES_ITEM_NAME_DELIA_INNER_X" "Crescent Moon Inner Armor"',
                     '"GAMEUI_HEROES_ItemStat_ATK" "ATT"', '"GAMEUI_HEROES_ITEMTOOLTIP_RARITY_5" "Rare"']
            text.write_bytes(b'\xff\xfe' + '\r\n'.join(lines).encode('utf-16-le'))
            package = read_package(root / 'xml', db, text)
        self.assertEqual(package['shops']['FERGHUS_Shop'], [{'npc': 'FERGHUS', 'location': 't01_bs', 'place': 'Colhen'}])
        self.assertEqual(package['shops']['ARTHYEN_SHOP'][0]['place'], 'Rocheste')
        self.assertNotIn('MISSING_SHOP', package['shops'])
        self.assertEqual(package['npcs']['FERGHUS'], ['Ferghus', 'Line one\nLine two'])
        self.assertEqual(package['npcs']['ARTHYEN'], [None, None])
        self.assertEqual(package['item_icons']['ENHANCE_ELIXIR'], 'enhance_elixir')
        self.assertNotIn('BAD', package['item_icons'])
        self.assertEqual(package['item_aliases'], {'FIONA_NOLUCK_INNER_X': 'DELIA_INNER_X'})
        self.assertEqual(package['item_info']['FIONA_NOLUCK_INNER_X'],
                         {'r': 1, 'c': 'INNERARMOR', 'bg': 'fiona_inner', 'w': 24, 'dy': 1})
        self.assertEqual(package['tooltip_text'], {'GAMEUI_HEROES_ITEMSTAT_ATK': 'ATT', 'GAMEUI_HEROES_ITEMTOOLTIP_RARITY_5': 'Rare'})
        self.assertEqual((place('morvan.inn'), place('event.pinkrabbit'), place('')), ('Malina (Morvan)', 'Eventos', 'Outros'))


if __name__ == '__main__':
    unittest.main()
