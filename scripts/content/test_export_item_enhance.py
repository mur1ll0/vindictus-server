import contextlib, pathlib, sqlite3, struct, tempfile, unittest

import content_overrides as overrides
from export_item_enhance import read_package, digest

SCHEMA = (
    'CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);'
    'CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT,EnhanceType TEXT,Enchantable tinyint,QualityType nvarchar);'
    'CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT,MaxAntiBind INT,RequiredLevel INT,ExpireIn INT);'
    'CREATE TABLE EnhanceRuneInfo(ItemClass nvarchar,ReleationNum INT,MinEnhanceLevel INT,MaxEnhanceLevel INT,Feature nvarchar,ConfirmationDesc nvarchar);'
    'CREATE TABLE EnhanceRuneExtraInfo(ReleationNum INT,SuccessRatio float,IsUseItemSuccessRatio bit,IsDestoryOnSuccess bit,IsResetRankOnFail bit,ForceRankDownOnFail INT);'
    'CREATE TABLE RecipeMaterialInfo(RecipeID nvarchar,ItemClass nvarchar,Num INT,Type tinyint,Feature nvarchar);'
    'CREATE TABLE AntiBindInfo(Target nvarchar,"From" nvarchar,"To" nvarchar,Count INT,AP INT,Feature nvarchar);'
    'CREATE TABLE EnchantInfo(ID INT,EnchantClass nvarchar,IsPrefix bit,EnchantType nvarchar,EnchantLevel INT,MinArgValue INT,MaxArgValue INT,ItemConstraint nvarchar,ItemConstraintDesc nvarchar,MinSuccessRatio INT,MaxSuccessRatio INT,DestructionRatio INT,MiniGameSize INT,Duration INT,IgnoreMaxArmor bit,Feature nvarchar);'
    'CREATE TABLE EnchantStatInfo(EnchantClass nvarchar,"Order" INT,Condition nvarchar,ConditionDesc nvarchar,Stat nvarchar,StatDesc nvarchar);'
    'CREATE TABLE EnchantDiceInfo(ItemClass nvarchar,MinValue INT,MaxValue INT,SkillRank INT,Feature nvarchar);'
    'CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE EnhanceInfo(EnhanceType varchar,EnhanceLevel INT,MaterialClass1 varchar,MaterialNum1 INT,MaterialClass2 varchar,MaterialNum2 INT,Gold INT,SuccessRatio REAL,OnFail varchar,OnFailRecipeRatio REAL,OnFailMaterialNum1 INT,Effect INT,Feature varchar);'
    'CREATE TABLE EnhanceStatInfo(EnhanceType varchar,EnhanceLevel INT,Stat varchar,ValueType varchar,Value REAL,Base varchar);'
    'CREATE TABLE EnhanceServerInfo(EnhanceType nvarchar,EnhanceLevel INT,SuccessRatio REAL,Feature nvarchar);'
    'CREATE TABLE EnhanceBonusServerInfo(ItemClass varchar,Bonus float,EnhanceMin smallint,EnhanceMax smallint);'
    'CREATE TABLE ManufactureInfo(ManufactureID nvarchar,Grade INT,ExperienceMin INT,ExperienceMax INT);'
    'CREATE TABLE ManufactureRecipeInfo(ManufactureID nvarchar,RecipeID nvarchar,ExperienceRequired INT,GradeRequired INT,BaseRewardEXP INT,RecipeItemClass nvarchar,AutoGiveQuest nvarchar,AlwaysListed tinyint,Feature nvarchar);'
    'CREATE TABLE ManufactureMaterialInfo(RecipeID nvarchar,ItemClass nvarchar,Num INT,Type tinyint);'
    'CREATE TABLE ManufactureDependencyInfo(ManufactureID nvarchar,RequiredManufactureID nvarchar,RequiredExperience INT);'
    'CREATE TABLE QualityDiceInfo(ItemType nvarchar,Quality INT,Dice INT);'
    'CREATE TABLE QualityStatInfo(ItemType varchar,Quality int,Stat varchar,ValueType varchar,Value real,Base varchar);'
    'CREATE TABLE EnhanceInsteadMaterialInfo(ItemClass varchar,UseCount INT,Slot tinyint,EnhanceMin smallint,EnhanceMax smallint);'
)


def fill(db, ratio):
    db.executescript(SCHEMA)
    db.execute('INSERT INTO EnchantInfo VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', (6, 'hard', 'True', 'TEMP', 1, 0, 0, 'CLOTH,WEAPON', None, 100 if ratio == 1.0 else 50, 100 if ratio == 1.0 else 80, 0, 100, 0, 'False', None))
    db.execute('INSERT INTO EnchantStatInfo VALUES (?,?,?,?,?,?)', ('hard', 1, 'skillrank>=(skill_str_mastery,3)', None, 'DEF+2', None))
    db.execute('INSERT INTO EnchantDiceInfo VALUES (?,?,?,?,?)', ('enchant_elixir_lvl1', 1, 10, 0, 'enEU'))
    db.executemany('INSERT INTO FeatureMatrix VALUES (?,?)', [('GameCode', '42'), ('CurrentVer', '3.09.00'), ('OverrideSetting', ''), ('StoryTabInInventory', ''), ('enEU', '1'), ('zhCN', '')])
    db.executemany('INSERT INTO EquipItemInfo VALUES (?,?,?,?,?)', [('sword', None, 'SWORD', 1, 'WEAPON_LVL_1'), ('cloak', None, None, 0, None)])
    db.executemany('INSERT INTO ManufactureInfo VALUES (?,?,?,?)', [('metal_weapon', 0, 0, 99), ('metal_weapon', 1, 100, 199)])
    db.execute('INSERT INTO ManufactureRecipeInfo VALUES (?,?,?,?,?,?,?,?,?)', ('metal_weapon', 'recipe_sword', 0 if ratio == 1.0 else 150, 0 if ratio == 1.0 else 1, 3, 'recipe_book', 'quest_x', 0, 'Delia&&S3_CraftRenewal'))
    db.executemany('INSERT INTO ManufactureMaterialInfo VALUES (?,?,?,?)', [('recipe_sword', 'sword', 1, 0), ('recipe_sword', 'stone', 4, 1)])
    db.execute('INSERT INTO ManufactureDependencyInfo VALUES (?,?,?)', ('metal_weapon_dc', 'metal_weapon', 350))
    db.executemany('INSERT INTO QualityDiceInfo VALUES (?,?,?)', [('WEAPON_LVL_1', 1, 50), ('WEAPON_LVL_1', 2, 650)])
    db.execute('INSERT INTO QualityStatInfo VALUES (?,?,?,?,?,?)', ('WEAPON_LVL_1', 1, 'ATK', 'Multiply', -0.01, 'ATK'))
    db.executemany('INSERT INTO EnhanceInfo VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)', [
        ('SWORD', 1, 'stone', 2, None, 0, 500, ratio, 'NoPenalty', 0.0, 0, 1, 'enEU'),
        ('SWORD', 9, 'stone', 4, 'elixir', 1, 9000, ratio, 'Destroy', 0.5, 21, 3, 'enEU'),
    ])
    db.executemany('INSERT INTO EnhanceStatInfo VALUES (?,?,?,?,?,?)', [('SWORD', 1, 'ATK', 'Add', 50.0, 'ATK'), ('SWORD', 1, 'Weight', 'Multiply', -0.1, 'Weight')])
    db.executemany('INSERT INTO EnhanceServerInfo VALUES (?,?,?,?)', [('1', 1, ratio, '!zhCN'), ('9', 9, ratio, '!zhCN')])
    db.execute('INSERT INTO EnhanceBonusServerInfo VALUES (?,?,?,?)', ('gevne', 3.3, 1, 15))
    db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?,?,?,?)', [('sword', None, 1, 1, None, None, 3, 1, None), ('stone', None, 99, 1, None, None, 0, 1, None)])
    db.executemany('INSERT INTO EnhanceRuneInfo VALUES (?,?,?,?,?,?)', [('rune', 0, 4, 15, 'enEU', None)])
    db.executemany('INSERT INTO EnhanceRuneExtraInfo VALUES (?,?,?,?,?,?)', [(0, '.4', 0, 1, 0, 0)])
    db.executemany('INSERT INTO RecipeMaterialInfo VALUES (?,?,?,?,?)', [('recipe_sword', 'sword', 1, 0, None), ('recipe_sword', 'stone', 4, 1, None)])
    db.executemany('INSERT INTO AntiBindInfo VALUES (?,?,?,?,?,?)', [('Base', None, None, 2, 0, None), ('Enhance', '9', '15', 0, 500, 'enEU')])
    db.execute('INSERT INTO EnhanceInsteadMaterialInfo VALUES (?,?,?,?,?)', ('gevne', 1, 1, 1, 15))
    db.commit()


class ItemEnhanceExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        self.db = root / 'heroesContents.db3'
        self.original = root / 'original.db3'
        self.config = root / 'ServiceCore.dll.config'
        self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            fill(db, 1.0)
        # The original keeps text ratios ('.33') in its REAL column.
        with contextlib.closing(sqlite3.connect(self.original)) as db:
            fill(db, '.33')
        saved = (overrides.ORIGINAL_DATABASE, overrides.ORIGINAL_SHA256)
        overrides.ORIGINAL_DATABASE = self.original
        overrides.ORIGINAL_SHA256 = overrides._digest(self.original)
        overrides._cache.clear()

        def restore():
            overrides.ORIGINAL_DATABASE, overrides.ORIGINAL_SHA256 = saved
            overrides._cache.clear()
        self.addCleanup(restore)

    def test_original_ratios_become_the_single_the_server_reads(self):
        before = (digest(self.db), digest(self.config))
        p = read_package(self.db, self.config)
        single = lambda v: struct.pack('>f', v).hex()
        self.assertEqual([r['success_ratio'] for r in p['enhance']], [single(0.33)] * 2)
        self.assertEqual([r['success_ratio'] for r in p['server']], [single(0.33)] * 2)
        self.assertEqual(p['enhance'][1]['on_fail_recipe_ratio'], single(0.5))
        self.assertEqual(p['stats'][1]['value'], single(-0.1))
        self.assertEqual([e['enhance_type'] for e in p['equips']], ['SWORD'.encode('utf-16-le').hex(), None])
        self.assertEqual(p['bonus'][0]['bonus'], struct.pack('>d', 3.3).hex())
        self.assertEqual(sorted(f['key'] for f in p['features']), ['CraftPriceRatio', 'Delia', 'EnchantSuccessBonus', 'Enchant_DestructibleItems', 'EnhanceSuccessBonus', 'EnhanceSuccessBonus_Limit', 'Enhance_Ferghus_v2', 'Event_enhance_a_shot', 'Event_enhance_change_ratio', 'RestoreItem', 'RestoreItemCount', 'S3_CraftRenewal', 'enEU', 'manufacture_event_exp', 'zhCN'])
        self.assertEqual([c['max_anti_bind'] for c in p['classes']], [3, 0])
        self.assertEqual((p['enchant'][0]['min_ratio'], p['enchant'][0]['max_ratio'], p['enchant'][0]['is_prefix']), (50, 80, True))
        self.assertEqual(p['enchant_stats'][0]['order'], 1)
        self.assertEqual(p['enchant_dice'][0]['max'], 10)
        recipe = p['manufacture_recipes'][0]
        self.assertEqual((recipe['experience_required'], recipe['grade_required'], recipe['base_reward_exp']), (150, 1, 3))
        self.assertEqual([m['max'] for m in p['manufacture']], [99, 199])
        self.assertEqual([m['type'] for m in p['manufacture_materials']], [0, 1])
        self.assertEqual(p['manufacture_dependencies'][0]['required_experience'], 350)
        self.assertEqual([d['dice'] for d in p['quality_dice']], [50, 650])
        self.assertEqual(p['quality_stats'][0]['value'], single(-0.01))
        self.assertEqual(p['equips'][0]['quality_type'], 'WEAPON_LVL_1'.encode('utf-16-le').hex())
        self.assertEqual(p['rune_extra'][0]['success_ratio'], struct.pack('>d', 0.4).hex())
        self.assertEqual((p['rune_extra'][0]['destroy_on_success'], p['rune_extra'][0]['reset_rank_on_fail']), (True, False))
        self.assertEqual([r['type'] for r in p['recipes']], [0, 1])
        self.assertEqual(p['anti_bind'][1]['from_value'], '9'.encode('utf-16-le').hex())
        self.assertEqual(before, (digest(self.db), digest(self.config)))

    def test_a_null_single_is_refused(self):
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute('UPDATE EnhanceStatInfo SET Value=NULL')
            db.commit()
        with self.assertRaises(ValueError):
            read_package(self.db, self.config)


if __name__ == '__main__':
    unittest.main()
