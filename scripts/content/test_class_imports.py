import sqlite3
import tempfile
import unittest
from contextlib import closing
from pathlib import Path

import class_imports

FM = ('Feature', 'EN-US', 'EN-US-X-GM', 'EN-EU', 'EN-EU-X-GM')


def make(path: Path, na: bool) -> None:
    """A tiny heroes.db3: the NA one with text restrictions and an extra column, the 1.69 one with masks."""
    restriction = 'TEXT' if na else 'INT'
    extra = ', IconFG TEXT' if na else ''
    with closing(sqlite3.connect(path)) as db:
        db.execute(f'CREATE TABLE ItemClassInfo(ItemClass TEXT, Category TEXT, ClassRestriction {restriction}, MaxStack INT{extra})')
        db.execute('CREATE TABLE EquipItemInfo(ItemClass TEXT, CostumeType INT, ATK INT, EquipClass TEXT)')
        db.execute('CREATE TABLE CustomizeItemInfo(ItemClass TEXT, IsCash, Feature TEXT, CashShopType TEXT)')
        db.execute("INSERT INTO CustomizeItemInfo VALUES(?, ?, ?, ?)",
                   ('draker_newbie', 1, 'enUS', 'en-US') if na else ('sword', 'False', None, 'en-EU'))
        if na:  # gated by a feature the 1.69 does not have: left out
            db.execute("INSERT INTO CustomizeItemInfo VALUES('miri_inner', 0, 'Calia', NULL)")
        for table in ('CustomizeDefaultInfo', 'CustomizePriceInfo', 'DyeableParts', 'PatternInfo'):
            db.execute(f'CREATE TABLE {table}(ItemClass TEXT, Feature TEXT)')
        db.execute(f'CREATE TABLE SkillInfo(SkillID TEXT, ClassRestriction {restriction})')
        db.execute('CREATE TABLE SkillRankInfo(SkillID TEXT, Rank INT, Feature TEXT)')
        db.execute('CREATE TABLE SkillArgumentInfo(SkillId TEXT, Rank INT, KeyWord TEXT, Value TEXT)')
        db.execute('CREATE TABLE SkillConstraintInfo(SkillId TEXT, Rank INT)')
        db.execute('CREATE TABLE RecipeInfo(RecipeID TEXT, AppearQuestID TEXT, DisappearQuestID TEXT)')
        db.execute('CREATE TABLE RecipeMaterialInfo(RecipeID TEXT, ItemClass TEXT, Num INT, Type INT)')
        db.execute('CREATE TABLE ManufactureRecipeInfo(RecipeID TEXT)')
        db.execute('CREATE TABLE CraftShopInfo(ShopID TEXT, [Order] INT, RecipeID TEXT)')
        db.execute('CREATE TABLE StatusEffectGroupInfo(Type TEXT, Level INT)')
        if na:  # the Miri's own effect, one she uses without her name in it, and another class's
            db.executemany('INSERT INTO StatusEffectGroupInfo VALUES(?, ?)',
                           [('miri_dragon_transform', 1), ('resound_stamina', 1), ('delia_guard', 1)])
        db.execute('CREATE TABLE SynthesisSkillBonus(ID INT, SkillID TEXT)')
        db.execute(f'CREATE TABLE FeatureMatrix({",".join(f"[{c}]" for c in FM)})')
        db.execute('CREATE TABLE SetInfo(SetID TEXT)')
        db.execute('CREATE TABLE SetItemInfo(ItemClass TEXT, BaseItemClass TEXT, SetID TEXT)')
        db.execute('CREATE TABLE SetEffectInfo(SetID TEXT, SetCount INT, EffectTarget TEXT, Amount INT)')
        db.execute("INSERT INTO SetInfo VALUES('old_set')")
        db.execute("INSERT INTO SetEffectInfo VALUES('old_set', 2, 'STR', 5)")
        if na:  # a new set with its effects, and an imported item joining a set the 1.69 has
            db.execute("INSERT INTO SetInfo VALUES('pythonshock_set')")
            db.executemany('INSERT INTO SetItemInfo VALUES(?,?,?)', [('draker_newbie', 'draker_newbie', 'pythonshock_set'),
                                                                     ('miri_inner', 'miri_inner', 'old_set'),
                                                                     ('sword', 'sword', 'other_set')])
            db.executemany('INSERT INTO SetEffectInfo VALUES(?,?,?,?)', [('pythonshock_set', 2, 'DEF', 190),
                                                                         ('pythonshock_set', 2, 'STR', 111),
                                                                         ('old_set', 2, 'STR', 99)])
        db.execute("INSERT INTO RecipeMaterialInfo VALUES('recipe_sword', 'sword', 1, 0)")
        db.execute("INSERT INTO RecipeMaterialInfo VALUES('recipe_sword', 'gold', 10, 1)")
        if na:
            db.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?)', [
                ('draker_newbie', 'WEAPON', 'miri', 1, 'fg'), ('miri_inner', 'INNERARMOR', 'miri', 1, None),
                ('miri_tail', 'AVATAR_WEAPON', 'miri', 1, None), ('potion', 'POTION', 'all', 99, None),
                ('dress', 'TUNIC', 'female(!miri)', 1, None), ('sword', 'WEAPON', 'fiona', 1, None)])
            db.executemany('INSERT INTO EquipItemInfo VALUES(?,?,?,?)', [('draker_newbie', 3, 10, 'DRAKER'),
                                                                 ('miri_inner', 9100, 0, None)])
            db.execute("INSERT INTO SkillInfo VALUES('skill_miri_attack', 'miri')")
            db.execute("INSERT INTO SkillInfo VALUES('skill_fiona_attack', 'fiona')")
            db.execute("INSERT INTO SkillInfo VALUES('skill_defence_mastery', '!grimden')")
            db.execute("INSERT INTO SkillInfo VALUES('skill_no_miri', '!miri')")
            db.execute("INSERT INTO SkillRankInfo VALUES('skill_miri_attack', 1, 'Miri')")
            db.execute('CREATE TABLE StoryLineInfo(StoryLineID TEXT, ClassRestriction TEXT)')
            db.execute("INSERT INTO StoryLineInfo VALUES('story_no_miri', '!miri')")
            db.execute("INSERT INTO RecipeMaterialInfo VALUES('recipe_draker', 'draker_newbie', 1, 0)")
            db.execute("INSERT INTO RecipeMaterialInfo VALUES('recipe_draker', 'gold', 10, 1)")
            db.execute("INSERT INTO RecipeMaterialInfo VALUES('recipe_draker_new', 'draker_newbie', 1, 0)")
            db.execute("INSERT INTO RecipeMaterialInfo VALUES('recipe_draker_new', 'unknown_ore', 1, 1)")
            db.execute("INSERT INTO CraftShopInfo VALUES('FERGHUS_Craft', 3, 'recipe_sword')")
            db.execute("INSERT INTO CraftShopInfo VALUES('FERGHUS_Craft', 9, 'recipe_draker')")
            db.execute("INSERT INTO FeatureMatrix VALUES('Miri', '4.00.00', '4.00.00', NULL, NULL)")
            db.execute("INSERT INTO FeatureMatrix VALUES('DressRankLimit_Miri', '100', '100', NULL, NULL)")
            db.execute("INSERT INTO FeatureMatrix VALUES('relationship_hurk_miri', '4.27.00', '4.27.00', NULL, NULL)")
        else:
            db.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?)', [
                ('potion', 'POTION', 2047, 99), ('dress', 'TUNIC', 1446, 1), ('sword', 'WEAPON', 2, 1),
                ('helm', 'HELM', 1024, 1), ('fiona_inner', 'INNERARMOR', 2, 1),
                ('gold', 'POTION', 2, 1)])
            db.execute("INSERT INTO EquipItemInfo VALUES('sword', 0, 5, 'SWORD')")
            db.execute("INSERT INTO SkillInfo VALUES('skill_fiona_attack', 2)")
            # Every 1.69 class (0-10); the NA leaves Miri out of the second only.
            db.execute("INSERT INTO SkillInfo VALUES('skill_defence_mastery', 2047)")
            db.execute("INSERT INTO SkillInfo VALUES('skill_no_miri', 2047)")
            db.execute("INSERT INTO CraftShopInfo VALUES('_Craft', 40, 'recipe_sword')")
            # storylines and titles for every 1.69 class; the NA leaves Miri out of one storyline
            db.execute('CREATE TABLE StoryLineInfo(StoryLineID TEXT, ClassRestriction INT)')
            db.executemany('INSERT INTO StoryLineInfo VALUES(?,?)',
                           [('skill_all_xe_move', 2047), ('story_no_miri', 2047), ('story_fiona', 2)])
            db.execute('CREATE TABLE TitleInfo(ID INT, ClassRestriction INT)')
            db.executemany('INSERT INTO TitleInfo VALUES(?,?)', [(1, 2047), (2, 1024)])
            db.execute("INSERT INTO FeatureMatrix VALUES('Delia', '3.09.00', '3.09.00', '3.09.00', '3.09.00')")
            db.execute("INSERT INTO FeatureMatrix VALUES('enUS', '1', '1', NULL, NULL)")
            db.execute("INSERT INTO FeatureMatrix VALUES('enEU', NULL, NULL, '1', '1')")
            db.execute('CREATE TABLE CharacterClassInfo(Identifier INT, Level INT, BaseSTR INT)')
            db.executemany('INSERT INTO CharacterClassInfo VALUES(?,?,?)', [(10, 1, 9), (10, 2, 11), (9, 1, 8)])
        db.commit()


class ClassImportsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.na, self.base, self.out = root / 'na.db3', root / 'base.db3', root / 'imports'
        make(self.na, True)
        make(self.base, False)
        self.report = class_imports.build('miri', 'draker', 11, 10, self.na, self.base, self.out)
        self.imported = self.out / 'miri.db3'

    def test_build_selects_the_class_rows(self):
        rows = self.report['rows']
        self.assertEqual((rows['ItemClassInfo'], rows['SkillInfo'], rows['SkillRankInfo']), (2, 1, 1))
        self.assertEqual(self.report['dropped'], {'category AVATAR_WEAPON': 1, 'recipe with unknown material': 1,
                                                  'CustomizeItemInfo row with unknown feature': 1})
        self.assertEqual((rows['RecipeMaterialInfo'], rows['CraftShopInfo'], rows['FeatureMatrix']), (2, 1, 2))
        # Sets: the imported items' set rows; only the new set brings its SetInfo and effects.
        self.assertEqual((rows['SetItemInfo'], rows['SetInfo'], rows['SetEffectInfo']), (2, 1, 2))
        # Status effects: those naming the class and the ones it uses by name (MENTIONED_BY_NAME).
        with closing(sqlite3.connect(self.imported)) as db:
            self.assertEqual(sorted(db.execute('SELECT Type FROM StatusEffectGroupInfo')),
                             [('miri_dragon_transform',), ('resound_stamina',)])
        with closing(sqlite3.connect(self.imported)) as db:
            # The NA shop becomes the 1.69 shop with the same recipes, after its last row.
            self.assertEqual(db.execute('SELECT * FROM CraftShopInfo').fetchall(), [('_Craft', 41, 'recipe_draker')])

    def test_server_overlay_converts_columns_masks_features_and_stats(self):
        with closing(sqlite3.connect(f'file:{self.base}?mode=ro', uri=True)) as db:
            report = class_imports.install_server(db, [self.imported], {'miri': {('inner_armor', '9100'): '9044'}})
            # ItemClassInfo is left to content_additions.install; the other tables get views here.
            items = dict(db.execute('SELECT ItemClass, ClassRestriction FROM temp._imported_ItemClassInfo'))
            self.assertEqual(items, {'draker_newbie': 2048, 'miri_inner': 2048})
            base = dict(db.execute('SELECT ItemClass, ClassRestriction FROM temp._class_base_ItemClassInfo'))
            self.assertEqual(base, {'potion': 4095, 'dress': 1446, 'sword': 2, 'helm': 1024, 'fiona_inner': 2, 'gold': 2})
            self.assertEqual(dict(db.execute('SELECT ItemClass, CostumeType FROM temp._imported_EquipItemInfo')),
                             {'draker_newbie': 10103, 'miri_inner': 9044})
            features = db.execute('SELECT * FROM FeatureMatrix ORDER BY Feature').fetchall()
            self.assertIn(('Miri', '4.00.00', '4.00.00', '3.09.00', '3.09.00'), features)
            self.assertIn(('DressRankLimit_Miri', '100', '100', '100', '100'), features)
            self.assertEqual(db.execute('SELECT * FROM CharacterClassInfo WHERE Identifier=11').fetchall(),
                             [(11, 1, 9), (11, 2, 11)])
            self.assertEqual(db.execute('SELECT SkillID, ClassRestriction FROM SkillInfo').fetchall(),
                             [('skill_fiona_attack', 2), ('skill_defence_mastery', 4095), ('skill_no_miri', 2047),
                              ('skill_miri_attack', 2048)])
            self.assertEqual((report['class_bits'], report['skill_bits']), (1, 1))
            # storylines and titles for every class take the class too (C4 MI25)
            self.assertEqual(dict(db.execute('SELECT * FROM StoryLineInfo')),
                             {'skill_all_xe_move': 4095, 'story_no_miri': 2047, 'story_fiona': 2})
            self.assertEqual(dict(db.execute('SELECT * FROM TitleInfo')), {1: 4095, 2: 1024})
            self.assertEqual(report['shared_bits'], {'StoryLineInfo': 1, 'TitleInfo': 1})
            # The NA 0/1 becomes the 1.69 text boolean of the column.
            # ... and the NA US region becomes the laboratory's EU region.
            self.assertEqual(db.execute("SELECT IsCash, Feature, CashShopType FROM CustomizeItemInfo "
                                        "WHERE ItemClass='draker_newbie'").fetchone(), ('True', 'enEU', 'en-EU'))

    def test_client_copy_gets_rows_once_and_comes_back(self):
        original = self.base.with_name('original.db3')
        original.write_bytes(self.base.read_bytes())
        with closing(sqlite3.connect(self.base)) as db, closing(sqlite3.connect(original)) as backup:
            for _ in range(2):
                report = class_imports.apply_client(db, backup, [self.imported])
            self.assertEqual(report['miri']['ItemClassInfo'], 2)
            self.assertEqual(db.execute('SELECT count(*) FROM ItemClassInfo').fetchone()[0], 8)
            self.assertEqual(db.execute("SELECT ClassRestriction FROM ItemClassInfo WHERE ItemClass='potion'").fetchone()[0], 4095)
            self.assertEqual(report['miri']['skill_bits'], 1)
            self.assertEqual(dict(db.execute("SELECT SkillID, ClassRestriction FROM SkillInfo WHERE SkillID IN "
                                             "('skill_defence_mastery', 'skill_no_miri')")),
                             {'skill_defence_mastery': 4095, 'skill_no_miri': 2047})
            self.assertEqual(dict(db.execute('SELECT * FROM StoryLineInfo')),
                             {'skill_all_xe_move': 4095, 'story_no_miri': 2047, 'story_fiona': 2})
            self.assertEqual(report['miri']['TitleInfo_bits'], 1)
            class_imports.apply_client(db, backup, [])
            self.assertEqual(db.execute("SELECT ClassRestriction FROM StoryLineInfo WHERE StoryLineID='skill_all_xe_move'").fetchone()[0], 2047)
            self.assertEqual(db.execute("SELECT ClassRestriction FROM SkillInfo WHERE SkillID='skill_defence_mastery'").fetchone()[0], 2047)
            self.assertEqual(db.execute('SELECT count(*) FROM ItemClassInfo').fetchone()[0], 6)
            self.assertEqual(db.execute("SELECT ClassRestriction FROM ItemClassInfo WHERE ItemClass='potion'").fetchone()[0], 2047)

    def test_weapons_without_mastery_take_the_reference_rank(self):
        with closing(sqlite3.connect(':memory:')) as db, closing(sqlite3.connect(':memory:')) as imported:
            for target in (db, imported):
                target.execute('CREATE TABLE ItemClassInfo(ItemClass TEXT, RequiredLevel INT, RequiredSkill TEXT, RequiredSkillRank INT)')
                target.execute('CREATE TABLE EquipItemInfo(ItemClass TEXT, EquipClass TEXT)')
            db.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?)', [
                ('greatsword_titan', 60, 'skill_hurk_greatsword_mastery', 6), ('greatsword_newbie', 1, None, 0),
                ('greatsword_a', 70, 'skill_hurk_greatsword_mastery', 8), ('greatsword_b', 80, 'skill_hurk_greatsword_mastery', 10),
                ('sword_c', 70, 'skill_sword_mastery', 2)])
            db.executemany('INSERT INTO EquipItemInfo VALUES(?,?)', [('greatsword_titan', 'GREATSWORD'), ('greatsword_newbie', 'GREATSWORD'),
                                                                   ('greatsword_a', 'GREATSWORD'), ('greatsword_b', 'GREATSWORD'),
                                                                   ('sword_c', 'SWORD')])
            imported.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?)', [
                ('draker_titan', 60, None, 0), ('draker_newbie', 1, None, 0), ('draker_x', 75, None, 0),
                ('draker_y', 90, None, 0), ('draker_kept', 70, 'skill_other', 3), ('draker_low', 0, None, 0)])
            imported.executemany('INSERT INTO EquipItemInfo VALUES(?,?)', [(i, 'DRAKER') for i in (
                'draker_titan', 'draker_newbie', 'draker_x', 'draker_y', 'draker_kept', 'draker_low')])
            imported.execute('CREATE TABLE SkillRankInfo(SkillID TEXT, Rank INT)')
            imported.executemany('INSERT INTO SkillRankInfo VALUES(?,?)',
                                 [('skill_miri_draker_mastery', r) for r in (0, 1, 2, 3, 4, 5, 6, 7, 8, 13, 14, 15)])
            got = class_imports.mastery_requirements(db, imported, {'name': 'miri', 'weapon': 'draker'})
            self.assertEqual(got, {'draker_titan': ('skill_miri_draker_mastery', 6),   # by name
                                   'draker_x': ('skill_miri_draker_mastery', 8),       # level 70's rank
                                   'draker_y': ('skill_miri_draker_mastery', 8)})      # 10 is not a Miri rank: 8
            # No reference for another weapon; nothing without the columns.
            self.assertEqual(class_imports.mastery_requirements(db, imported, {'name': 'miri', 'weapon': 'other'}), {})

    def test_armor_takes_the_169_mastery_and_trade_layout(self):
        with closing(sqlite3.connect(':memory:')) as db, closing(sqlite3.connect(':memory:')) as imported:
            for target in (db, imported):
                target.execute('CREATE TABLE ItemClassInfo(ItemClass TEXT, TradeCategory TEXT, TradeCategorySub TEXT, '
                               'RequiredLevel INT, RequiredSkill TEXT, RequiredSkillRank INT)')
            db.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?,?)', [
                ('a', 'LIGHTARMOR', 'LIGHTARMOR_HELM', 54, 'skill_light_armor_mastery', 5),
                ('b', 'LIGHTARMOR', 'LIGHTARMOR_BOOTS', 60, 'skill_light_armor_mastery', 6),
                ('c', 'LIGHTARMOR', 'LIGHTARMOR_TUNIC', 60, 'skill_light_armor_mastery', 6),
                ('d', 'LIGHTARMOR', 'LIGHTARMOR_PANTS', 60, 'skill_light_armor_mastery', 3),
                ('e', 'PLATEARMOR', 'PLATEARMOR_HELM', 50, 'skill_plate_armor_mastery', 4)])
            imported.executemany('INSERT INTO ItemClassInfo VALUES(?,?,?,?,?,?)', [
                ('dragon_rise_head', 'HELM', 'LIGHTARMOR', 54, 'skill_common_armor_mastery', 5),
                ('pythonshock_foot', 'BOOTS', 'LIGHTARMOR', 61, 'skill_common_armor_mastery', 5),
                ('plate_helm', 'HELM', 'PLATEARMOR', 52, 'skill_common_armor_mastery', 5),
                ('ring', 'ACCESSORY', 'JEWELLERY', 50, None, 0)])
            cols, rows = class_imports.converted(db, imported, 'ItemClassInfo', {'name': 'miri', 'weapon': 'draker', 'index': 11}, {})
            got = {r[0]: r[1:] for r in rows}
            self.assertEqual(got['dragon_rise_head'], ('LIGHTARMOR', 'LIGHTARMOR_HELM', 54, 'skill_light_armor_mastery', 5))
            self.assertEqual(got['pythonshock_foot'], ('LIGHTARMOR', 'LIGHTARMOR_BOOTS', 61, 'skill_light_armor_mastery', 6))
            self.assertEqual(got['plate_helm'], ('PLATEARMOR', 'PLATEARMOR_HELM', 52, 'skill_plate_armor_mastery', 4))
            self.assertEqual(got['ring'], ('ACCESSORY', 'JEWELLERY', 50, None, 0))

    def test_imported_sets_get_name_and_bonus_text(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'miri.db3'
            with closing(sqlite3.connect(path)) as db:
                db.execute('CREATE TABLE SetEffectInfo(SetID TEXT, SetCount INT, EffectTarget TEXT, Amount INT)')
                db.executemany('INSERT INTO SetEffectInfo VALUES(?,?,?,?)', [
                    ('dragon_rise_set', 2, 'DEF', 150), ('dragon_rise_set', 2, 'INT', 35), ('dragon_rise_set', 3, 'DEF', 165)])
                db.commit()
            got = class_imports.set_tokens([path])
        self.assertEqual(got, {'HEROES_ITEM_SETITEM_NAME_DRAGON_RISE_SET': 'Dragon Rise Set',
                               'HEROES_SET_ITEM_DESC_DRAGON_RISE_SET_2': 'DEF +150\nINT +35\n',
                               'HEROES_SET_ITEM_DESC_DRAGON_RISE_SET_3': 'DEF +165\n'})

    def test_the_class_takes_the_donor_xe_move(self):
        with closing(sqlite3.connect(self.base)) as base:
            base.execute("INSERT INTO SkillInfo VALUES('skill_delia_xe_move', 1024)")
            base.executemany('INSERT INTO SkillRankInfo VALUES(?,?,?)',
                             [('skill_delia_xe_move', r, 'ExtremeMode') for r in (0, 1)])
            base.commit()
            info = class_imports.donor_xe_rows(base, 'SkillInfo', 'SkillID', ['SkillID', 'ClassRestriction'], 'miri', set())
            # the class's own name and restriction (converted() turns the name into its bit)
            self.assertEqual(info, [('skill_miri_xe_move', 'miri')])
            ranks = class_imports.donor_xe_rows(base, 'SkillRankInfo', 'SkillID', ['Feature', 'SkillID', 'Rank', 'X'], 'miri', set())
            self.assertEqual(ranks, [('ExtremeMode', 'skill_miri_xe_move', 0, None), ('ExtremeMode', 'skill_miri_xe_move', 1, None)])
            # a class with its own XE keeps it
            self.assertEqual(class_imports.donor_xe_rows(base, 'SkillInfo', 'SkillID', ['SkillID'], 'miri',
                                                         {'skill_miri_xe_move'}), [])

    def test_the_class_xe_names_its_own_text_and_the_generic_icon(self):
        with closing(sqlite3.connect(':memory:')) as base:
            base.execute('CREATE TABLE SkillInfo(SkillID TEXT, ClassRestriction INT, IconType TEXT, SkillName TEXT, SkillDescription TEXT)')
            base.execute("INSERT INTO SkillInfo VALUES('skill_delia_xe_move', 1024, 'skill_bastard', "
                         "'HEROES_SKILL_NAME_skill_delia_xe_move', 'HEROES_SKILL_DESC_skill_delia_xe_move')")
            base.execute('CREATE TABLE SkillRankInfo(SkillID TEXT, Rank INT, Description TEXT)')
            base.execute("INSERT INTO SkillRankInfo VALUES('skill_delia_xe_move', 1, 'GameUI_Heroes_Desc_skill_delia_xe_move_F')")
            cols = ['SkillID', 'ClassRestriction', 'IconType', 'SkillName', 'SkillDescription']
            self.assertEqual(class_imports.donor_xe_rows(base, 'SkillInfo', 'SkillID', cols, 'miri', set()),
                             [('skill_miri_xe_move', 'miri', class_imports.XE_ICON,
                               'HEROES_SKILL_NAME_skill_miri_xe_move', 'HEROES_SKILL_DESC_skill_miri_xe_move')])
            self.assertEqual(class_imports.donor_xe_rows(base, 'SkillRankInfo', 'SkillID', ['SkillID', 'Rank', 'Description'], 'miri', set()),
                             [('skill_miri_xe_move', 1, 'GameUI_Heroes_Desc_skill_miri_xe_move_F')])

    def test_the_class_xe_takes_the_donor_text(self):
        text = ('"lang"\r\n{\r\n\t"Tokens"\r\n\t{\r\n'
                '\t\t"HEROES_SKILL_NAME_SKILL_DELIA_XE_MOVE"\t"XE Move"\r\n'
                '\t\t"GAMEUI_HEROES_DESC_SKILL_DELIA_XE_MOVE_F_EX"\t"Increases damage by 10%."\r\n'
                '\t\t"HEROES_SKILL_NAME_SKILL_DELIA_SP1"\t"Other"\r\n\t}\r\n}\r\n')
        self.assertEqual(class_imports.donor_xe_tokens(text, ['miri']),
                         {'HEROES_SKILL_NAME_skill_miri_xe_move': 'XE Move',
                          'GAMEUI_HEROES_DESC_skill_miri_xe_move_F_EX': 'Increases damage by 10%.'})

    def test_bad_arguments_are_refused(self):
        with self.assertRaises(class_imports.ImportError_):
            class_imports.build('Miri', 'draker', 11, 10, self.na, self.base, self.out)
        with self.assertRaises(class_imports.ImportError_):
            class_imports.build('miri', 'draker', 9, 1, self.na, self.base, self.out)


if __name__ == '__main__':
    unittest.main()
