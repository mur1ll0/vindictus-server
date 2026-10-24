"""Read-only enhance contents: EquipItemInfo.EnhanceType aligned with the catalog's
equipment rows and ItemClassInfo.MaxAntiBind with its class rows, EnhanceInfo/
EnhanceStatInfo/EnhanceServerInfo/EnhanceBonusServerInfo/EnhanceInsteadMaterialInfo,
EnhanceRuneInfo/EnhanceRuneExtraInfo, RecipeMaterialInfo (GetMakeItemRecipe) and
AntiBindInfo (CalcRequiredBuybackAP), EnchantInfo/EnchantStatInfo/EnchantDiceInfo,
ManufactureInfo/ManufactureRecipeInfo/ManufactureMaterialInfo/ManufactureDependencyInfo
(ManufactureDataContainer), QualityDiceInfo, QualityStatInfo, SkillEnhanceInfo,
SkillEnhanceMaterialInfo and the SkillInfo.SkillEnhanceGroup texts in provider order, and the region values
of their features plus CraftPriceRatio. EnchantInfo.MinSuccessRatio/MaxSuccessRatio and
ManufactureRecipeInfo.ExperienceRequired/GradeRequired come from the original content.
SuccessRatio comes from the original content (content_overrides). REAL columns are
exported as the Single the server's System.Data.SQLite reads (REAL -> DbType.Single;
text cells are converted by SQLite itself)."""
import argparse, contextlib, json, pathlib, sqlite3, struct
import content_overrides as overrides
from export_item_catalog import read_package as catalog_read, digest
from export_npc_scripts import region_features
from export_reward_policy import feature_atoms

ROOT = pathlib.Path(__file__).resolve().parents[2]


def text(v, limit=1024):
    if v is None:
        return None
    if not isinstance(v, str):
        raise ValueError('Enhance text type')
    b = v.encode('utf-16-le', errors='surrogatepass')
    if len(b) > limit:
        raise ValueError('Enhance text bound')
    return b.hex()


def integer(v, low=-(2**31), high=2**31):
    if type(v) is not int or not low <= v < high:
        raise ValueError('Enhance integer type/range')
    return v


def double(v):
    if type(v) not in (int, float):
        raise ValueError('Enhance Double type')
    return struct.pack('>d', float(v)).hex()


def boolean(v):
    # HeroesContentsLoader: a bit column reads as bool, a text cell compares to "True".
    if isinstance(v, str):
        return v == 'True'
    if type(v) is not int or v not in (0, 1):
        raise ValueError('Enhance bit type')
    return v == 1


def read_package(database, config):
    package = catalog_read(database, config)
    manifest = package['manifest']
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        db.row_factory = sqlite3.Row

        def real(v):
            if isinstance(v, str):
                v = db.execute('SELECT CAST(? AS REAL)', (v,)).fetchone()[0]
            if type(v) not in (int, float):
                raise ValueError('Enhance Double type')
            return v

        def single(v):
            # sqlite3_column_double (SQLite converts a text cell), then (float).
            if isinstance(v, str):
                v = db.execute('SELECT CAST(? AS REAL)', (v,)).fetchone()[0]
            if type(v) not in (int, float):
                raise ValueError('Enhance Single type')
            return struct.pack('>f', float(v)).hex()

        equips = []
        for index, r in enumerate(db.execute('Select * From [EquipItemInfo]')):
            if index >= len(package['equips']):
                raise ValueError('Catalog alignment changed')
            key = text(r['ItemClass'])
            if key != package['equips'][index]['item_utf16'] or r['Feature'] != package['equips'][index]['feature']:
                raise ValueError('Catalog order changed')
            equips.append(dict(source_index=index, item_utf16=key, feature=r['Feature'],
                               enhance_type=text(r['EnhanceType']),
                               enchantable=integer(r['Enchantable'], 0, 256),
                               quality_type=text(r['QualityType'])))
        if len(equips) != len(package['equips']):
            raise ValueError('Catalog alignment changed')
        classes = []
        for index, r in enumerate(db.execute('Select * From [ItemClassInfo]')):
            if index >= len(package['classes']):
                raise ValueError('Catalog alignment changed')
            key = text(r['ItemClass'])
            if key != package['classes'][index]['item_utf16'] or r['Feature'] != package['classes'][index]['feature']:
                raise ValueError('Catalog order changed')
            classes.append(dict(source_index=index, item_utf16=key, feature=r['Feature'],
                                max_anti_bind=integer(r['MaxAntiBind'])))
        if len(classes) != len(package['classes']):
            raise ValueError('Catalog alignment changed')
        runes = [dict(source_index=i, item_utf16=text(r['ItemClass']), relation=integer(r['ReleationNum']),
                      min=integer(r['MinEnhanceLevel']), max=integer(r['MaxEnhanceLevel']), feature=r['Feature'])
                 for i, r in enumerate(db.execute('Select * From [EnhanceRuneInfo]'))]
        # EnhanceRuneExtraInfo.SuccessRatio is a float (Double) column.
        rune_extra = [dict(source_index=i, relation=integer(r['ReleationNum']),
                           success_ratio=double(real(r['SuccessRatio'])),
                           use_item_success_ratio=boolean(r['IsUseItemSuccessRatio']),
                           destroy_on_success=boolean(r['IsDestoryOnSuccess']),
                           reset_rank_on_fail=boolean(r['IsResetRankOnFail']),
                           force_rank_down_on_fail=integer(r['ForceRankDownOnFail']))
                      for i, r in enumerate(db.execute('Select * From [EnhanceRuneExtraInfo]'))]
        recipes = [dict(source_index=i, recipe_utf16=text(r['RecipeID']), item_utf16=text(r['ItemClass']),
                        num=integer(r['Num']), type=integer(r['Type'], 0, 256), feature=r['Feature'])
                   for i, r in enumerate(db.execute('Select * From [RecipeMaterialInfo]'))]
        anti_bind = [dict(source_index=i, target=text(r['Target']), from_value=text(r['From']),
                          to_value=text(r['To']), count=integer(r['Count']), ap=integer(r['AP']),
                          feature=r['Feature'])
                     for i, r in enumerate(db.execute('Select * From [AntiBindInfo]'))]
        enchant = [dict(source_index=i, id=integer(r['ID']), enchant_utf16=text(r['EnchantClass']),
                        is_prefix=boolean(r['IsPrefix']), enchant_type=text(r['EnchantType']),
                        level=integer(r['EnchantLevel']), min_arg=integer(r['MinArgValue']),
                        max_arg=integer(r['MaxArgValue']), constraint=text(r['ItemConstraint'], 16384),
                        min_ratio=integer(r['MinSuccessRatio']), max_ratio=integer(r['MaxSuccessRatio']),
                        destruction=integer(r['DestructionRatio']), mini_game=integer(r['MiniGameSize']),
                        duration=integer(r['Duration']), ignore_max_armor=boolean(r['IgnoreMaxArmor']),
                        feature=r['Feature'])
                   for i, r in enumerate(overrides.select(db, 'EnchantInfo'))]
        enchant_stats = [dict(source_index=i, enchant_utf16=text(r['EnchantClass']), order=integer(r['Order']),
                              condition=text(r['Condition']), stat=text(r['Stat']))
                         for i, r in enumerate(db.execute('Select * From [EnchantStatInfo]'))]
        enchant_dice = [dict(source_index=i, item_utf16=text(r['ItemClass']), min=integer(r['MinValue']),
                             max=integer(r['MaxValue']), skill_rank=integer(r['SkillRank']), feature=r['Feature'])
                        for i, r in enumerate(db.execute('Select * From [EnchantDiceInfo]'))]
        enhance = [dict(source_index=i, enhance_type=text(r['EnhanceType']), level=integer(r['EnhanceLevel']),
                        material1=text(r['MaterialClass1']), num1=integer(r['MaterialNum1']),
                        material2=text(r['MaterialClass2']), num2=integer(r['MaterialNum2']),
                        gold=integer(r['Gold']), success_ratio=single(r['SuccessRatio']), on_fail=text(r['OnFail']),
                        on_fail_recipe_ratio=single(r['OnFailRecipeRatio']),
                        on_fail_material_num1=integer(r['OnFailMaterialNum1']), effect=integer(r['Effect']),
                        feature=r['Feature'])
                   for i, r in enumerate(overrides.select(db, 'EnhanceInfo'))]
        stats = [dict(source_index=i, enhance_type=text(r['EnhanceType']), level=integer(r['EnhanceLevel']),
                      stat=text(r['Stat']), value_type=text(r['ValueType']), value=single(r['Value']),
                      base=text(r['Base']))
                 for i, r in enumerate(db.execute('Select * From [EnhanceStatInfo]'))]
        server = [dict(source_index=i, enhance_type=text(r['EnhanceType']), level=integer(r['EnhanceLevel']),
                       success_ratio=single(r['SuccessRatio']), feature=r['Feature'])
                  for i, r in enumerate(overrides.select(db, 'EnhanceServerInfo'))]
        bonus = [dict(source_index=i, item_utf16=text(r['ItemClass']), bonus=double(r['Bonus']),
                      min=integer(r['EnhanceMin'], -(2**15), 2**15), max=integer(r['EnhanceMax'], -(2**15), 2**15))
                 for i, r in enumerate(db.execute('Select * From [EnhanceBonusServerInfo]'))]
        instead = [dict(source_index=i, item_utf16=text(r['ItemClass']), use_count=integer(r['UseCount']),
                        slot=integer(r['Slot'], 0, 256), min=integer(r['EnhanceMin'], -(2**15), 2**15),
                        max=integer(r['EnhanceMax'], -(2**15), 2**15))
                   for i, r in enumerate(db.execute('Select * From [EnhanceInsteadMaterialInfo]'))]
        manufacture = [dict(source_index=i, manufacture_utf16=text(r['ManufactureID']), grade=integer(r['Grade']),
                            min=integer(r['ExperienceMin']), max=integer(r['ExperienceMax']))
                       for i, r in enumerate(db.execute('Select * From [ManufactureInfo]'))]
        manufacture_recipes = [dict(source_index=i, manufacture_utf16=text(r['ManufactureID']),
                                    recipe_utf16=text(r['RecipeID']),
                                    experience_required=integer(r['ExperienceRequired']),
                                    grade_required=integer(r['GradeRequired']),
                                    base_reward_exp=integer(r['BaseRewardEXP']),
                                    recipe_item_utf16=text(r['RecipeItemClass']),
                                    auto_give_quest=text(r['AutoGiveQuest']),
                                    always_listed=integer(r['AlwaysListed'], 0, 256), feature=r['Feature'])
                               for i, r in enumerate(overrides.select(db, 'ManufactureRecipeInfo'))]
        manufacture_materials = [dict(source_index=i, recipe_utf16=text(r['RecipeID']), item_utf16=text(r['ItemClass']),
                                      num=integer(r['Num']), type=integer(r['Type'], 0, 256))
                                 for i, r in enumerate(db.execute('Select * From [ManufactureMaterialInfo]'))]
        manufacture_dependencies = [dict(source_index=i, manufacture_utf16=text(r['ManufactureID']),
                                         required_utf16=text(r['RequiredManufactureID']),
                                         required_experience=integer(r['RequiredExperience']))
                                    for i, r in enumerate(db.execute('Select * From [ManufactureDependencyInfo]'))]
        quality_dice = [dict(source_index=i, item_type_utf16=text(r['ItemType']), quality=integer(r['Quality']),
                             dice=integer(r['Dice']))
                        for i, r in enumerate(db.execute('Select * From [QualityDiceInfo]'))]
        quality_stats = [dict(source_index=i, item_type_utf16=text(r['ItemType']), quality=integer(r['Quality']),
                              stat=text(r['Stat']), value_type=text(r['ValueType']), value=single(r['Value']),
                              base=text(r['Base']))
                         for i, r in enumerate(db.execute('Select * From [QualityStatInfo]'))]
        skill_enhance = [dict(source_index=i, group=integer(r['SkillEnhanceGroup']), key=integer(r['SkillEnhanceKey']),
                              item_utf16=text(r['ItemClass']), max_success_ratio=integer(r['MaxSuccessRatio']),
                              max_durability=integer(r['MaxDurability']), repair_value=integer(r['RepairValue']))
                         for i, r in enumerate(db.execute('Select * From [SkillEnhanceInfo]'))]
        skill_enhance_materials = [dict(source_index=i, item_utf16=text(r['MaterialItemClass']), type=integer(r['Type'], 0, 256),
                                        min=integer(r['MinIncreaseRatio']), max=integer(r['MaxIncreaseRatio']))
                                   for i, r in enumerate(db.execute('Select * From [SkillEnhanceMaterialInfo]'))]
        skill_enhance_groups = [dict(source_index=i, skill_utf16=text(r['SkillID']), groups=text(r['SkillEnhanceGroup']))
                                for i, r in enumerate(db.execute('Select * From [SkillInfo]'))
                                if r['SkillEnhanceGroup']]
        values = region_features(db, config)
    featured = enhance + server + runes + recipes + anti_bind + manufacture_recipes
    if any(r['feature'] is not None and not isinstance(r['feature'], str) for r in featured):
        raise ValueError('Enhance feature type')
    # Values read by EnhanceItemByDataProcessor, ItemServiceProxy and RestoreStorage.
    atoms = {'CraftPriceRatio', 'EnhanceSuccessBonus', 'EnhanceSuccessBonus_Limit', 'Event_enhance_a_shot',
             'Event_enhance_change_ratio', 'Enhance_Ferghus_v2', 'RestoreItem', 'RestoreItemCount',
             'EnchantSuccessBonus', 'Enchant_DestructibleItems', 'manufacture_event_exp', 'SkillEnhance'}
    for r in featured:
        atoms |= feature_atoms(r['feature'] or '')
    if len(atoms) > 256 or any(not k.isascii() or not k or len(k) > 256 for k in atoms):
        raise ValueError('Enhance feature subset bound')
    features = [dict(key=k, value=values.get(k)) for k in sorted(atoms)]
    if any(f['value'] is not None and (not isinstance(f['value'], str) or len(f['value']) > 256) for f in features):
        raise ValueError('Enhance feature value bound')
    if digest(database) != manifest['database_sha256'] or digest(config) != manifest['config_sha256']:
        raise ValueError('Enhance source changed during export')
    return dict(format_version=6, source='heroesContents.item-enhance', database_sha256=manifest['database_sha256'],
                config_sha256=manifest['config_sha256'], equips=equips, classes=classes, enhance=enhance,
                stats=stats, server=server, bonus=bonus, instead=instead, runes=runes, rune_extra=rune_extra,
                recipes=recipes, anti_bind=anti_bind, enchant=enchant, enchant_stats=enchant_stats,
                enchant_dice=enchant_dice, manufacture=manufacture, manufacture_recipes=manufacture_recipes,
                manufacture_materials=manufacture_materials, manufacture_dependencies=manufacture_dependencies,
                quality_dice=quality_dice, quality_stats=quality_stats, skill_enhance=skill_enhance,
                skill_enhance_materials=skill_enhance_materials, skill_enhance_groups=skill_enhance_groups,
                features=features)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    base = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    output = pathlib.Path(args.output).resolve()
    if output.parent != (ROOT / '.cache/private-content').resolve():
        raise ValueError('Private output outside ACL cache')
    package = read_package(base / 'heroesContents.db3', base / 'ServiceCore.dll.config')
    data = json.dumps(package, ensure_ascii=True, separators=(',', ':'))
    if len(data.encode()) > 32 * 1024 * 1024:
        raise ValueError('Enhance profile bound exceeded')
    output.write_text(data, encoding='utf-8', newline='\n')
    print('Enhance contents exported: %d equipment, %d enhance, %d stat, %d server rows; values private'
          % (len(package['equips']), len(package['enhance']), len(package['stats']), len(package['server'])))


if __name__ == '__main__':
    main()
