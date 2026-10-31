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
        db.execute('CREATE TABLE SynthesisSkillBonus(ID INT, SkillID TEXT)')
        db.execute(f'CREATE TABLE FeatureMatrix({",".join(f"[{c}]" for c in FM)})')
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
            class_imports.apply_client(db, backup, [])
            self.assertEqual(db.execute("SELECT ClassRestriction FROM SkillInfo WHERE SkillID='skill_defence_mastery'").fetchone()[0], 2047)
            self.assertEqual(db.execute('SELECT count(*) FROM ItemClassInfo').fetchone()[0], 6)
            self.assertEqual(db.execute("SELECT ClassRestriction FROM ItemClassInfo WHERE ItemClass='potion'").fetchone()[0], 2047)

    def test_bad_arguments_are_refused(self):
        with self.assertRaises(class_imports.ImportError_):
            class_imports.build('Miri', 'draker', 11, 10, self.na, self.base, self.out)
        with self.assertRaises(class_imports.ImportError_):
            class_imports.build('miri', 'draker', 9, 1, self.na, self.base, self.out)


if __name__ == '__main__':
    unittest.main()
