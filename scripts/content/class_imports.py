"""Content rows of a class imported from the NA client (skill new-class-port, step 2).

`build` reads the NA heroes.db3 (read-only) and writes the class's rows, with the NA columns, to
.cache/private-content/class-imports/<class>.db3 (never versioned: game data). The rows:
  - items whose NA ClassRestriction names the class, or whose name has the class or weapon name,
    that the 1.69 lacks and whose Category the 1.69 knows; their EquipItemInfo, Customize*,
    DyeableParts and PatternInfo rows;
  - skills whose ClassRestriction names the class, with their ranks, arguments and constraints;
  - recipes that make an imported item from known materials (RecipeMaterialInfo Type 0 is the product),
    and their craft shop rows, moved to the 1.69 shop that lists the same recipes;
  - the class's FeatureMatrix rows and its StatusEffectGroupInfo and SynthesisSkillBonus rows.
`install_server` (content_additions.connect) and `apply_client` (apply_lab_content_additions) put them
in front of a 1.69 database the same way:
  - target columns only: the NA value where the NA has the column, else the column's most common value;
  - ClassRestriction text -> the class's bit (the imported rows name only the class);
  - FeatureMatrix rows take the laboratory locale columns of the Delia row (enabled on EN-EU-X-GM);
  - rows of the NA US region become EU rows (enUS -> enEU, CashShopType en-US -> en-EU);
  - CostumeType of the class's items renumbered like its player_costume.txt entries (class_costumes);
  - existing items the class can wear get its bit: their mask already allows every 1.69 female class
    (Miri's body is female) and the NA expression does not exclude the class (female(!miri));
  - server only: CharacterClassInfo and CharacterBaseLoadInfo rows copied from `stats_from` (the NA
    client has no stat tables; a laboratory choice).
"""
from __future__ import annotations
import argparse
import collections
import hashlib
import json
import re
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

import class_costumes

ROOT = Path(__file__).resolve().parents[2]
NA_DATABASE = ROOT / '.cache/na-db/heroes-plain.db3'
IMPORTS = ROOT / '.cache/private-content/class-imports'
# Staged NA classes (scripts/client/stage_na_class.py): class-assets.json and merge/text/.
CLASS_ASSETS = ROOT / '.cache/private-assets/classes'
NAME = re.compile(r'^[a-z]{2,16}$')
# Item tables keyed by ItemClass and skill tables by skill id (column name per table).
ITEM_TABLES = ('ItemClassInfo', 'EquipItemInfo', 'CustomizeItemInfo', 'CustomizeDefaultInfo', 'CustomizePriceInfo',
               'DyeableParts', 'PatternInfo')
SKILL_TABLES = {'SkillInfo': 'SkillID', 'SkillRankInfo': 'SkillID', 'SkillArgumentInfo': 'SkillId',
                'SkillConstraintInfo': 'SkillId'}
RECIPE_TABLES = ('RecipeInfo', 'RecipeMaterialInfo', 'ManufactureRecipeInfo', 'CraftShopInfo')
MENTION_TABLES = {'StatusEffectGroupInfo': ('Type', 'Level'), 'SynthesisSkillBonus': ('ID',)}
FEATURE_TEMPLATE = 'Delia'
FEATURE_TOKEN = re.compile(r'[A-Za-z0-9_.]+')
# The NA's English region is US; the laboratory client is EU English (EN-EU-X-GM): rows of the NA US
# region become EU rows (feature token and customization CashShopType), as the 1.69's Delia rows are.
REGION_FEATURES = {'enus': 'enEU'}
REGION_TAGS = {'en-US': 'en-EU'}
FEATURE_SKIP = re.compile(r'relationship|startmovie|event_', re.IGNORECASE)
STAT_TABLES = {'CharacterClassInfo': 'Identifier', 'CharacterBaseLoadInfo': 'Character'}
# 1.69 classes with a female body (Fiona, Evy, Vella, Lynn, Arisha, Delia).
FEMALE_169 = (1 << 1) | (1 << 2) | (1 << 5) | (1 << 7) | (1 << 8) | (1 << 10)
ALL_169 = (1 << 11) - 1  # classes 0-10
# Equipment sets: the set rows of the imported items (the Miri's level-60 pythonshock_set and 15 more);
# a set the 1.69 lacks comes with its SetInfo and SetEffectInfo rows, one it has keeps the 1.69's effects.
SET_TABLES = ('SetItemInfo', 'SetInfo', 'SetEffectInfo')
# Weapon mastery: the NA dropped the mastery rank requirement of its weapons (every DRAKER row has no
# RequiredSkill), the 1.69 still asks it (greatsword_titan: skill_hurk_greatsword_mastery rank 6). An
# imported weapon without a requirement takes the class's own mastery, at the rank of the 1.69 weapon of
# the reference equip class with the same name (draker_titan -> greatsword_titan), else the most common
# rank of that class's weapons at its level (or the nearest lower one), within the ranks the class's
# mastery has (the Miri's skips 9-12: a higher requirement takes the highest rank below it).
MASTERY_REFERENCE = {'draker': 'greatsword'}
TABLES = ITEM_TABLES + tuple(SKILL_TABLES) + RECIPE_TABLES + tuple(MENTION_TABLES) + ('FeatureMatrix',) + SET_TABLES


class ImportError_(ValueError):
    pass


def columns(db: sqlite3.Connection, table: str, schema: str = 'main') -> list[str]:
    return [r[1] for r in db.execute(f'PRAGMA {schema}.table_info([{table}])')]


def names_class(expression: str | None, name: str) -> bool:
    """The NA ClassRestriction text lets the class in by name (not 'female(!miri)')."""
    tokens = re.split(r'[,()&|+]', (expression or '').lower())
    return name in [t.strip() for t in tokens] and f'!{name}' not in (expression or '').lower()


def excludes_class(expression: str | None, name: str) -> bool:
    return f'!{name}' in (expression or '').lower()


def _rows(db: sqlite3.Connection, table: str, where: str, params) -> tuple[list[str], list[tuple]]:
    cursor = db.execute(f'SELECT * FROM [{table}] WHERE {where}', list(params))
    return [c[0] for c in cursor.description], cursor.fetchall()


def _chunks(values, size=400):
    values = sorted(values)
    for i in range(0, len(values), size):
        yield values[i:i + size]


def select(na: sqlite3.Connection, base: sqlite3.Connection, name: str, weapon: str) -> tuple[dict, dict]:
    """The class's NA rows by table and a report of what was left out."""
    categories = {r[0] for r in base.execute('SELECT DISTINCT Category FROM ItemClassInfo')}
    known = {r[0] for r in base.execute('SELECT ItemClass FROM ItemClassInfo')}
    items, dropped = set(), collections.Counter()
    for item, category, restriction in na.execute('SELECT ItemClass, Category, ClassRestriction FROM ItemClassInfo'):
        lowered = item.lower()
        if not (names_class(restriction, name) or (name in lowered or weapon in lowered)) or excludes_class(restriction, name):
            continue
        if item in known:
            continue
        if category not in categories:
            dropped[f'category {category}'] += 1
            continue
        items.add(item)
    skills = {r[0] for r in na.execute('SELECT SkillID, ClassRestriction FROM SkillInfo') if names_class(r[1], name)}
    out: dict[str, tuple[list[str], list[tuple]]] = {}

    def gather(table, column, keys):
        cols, rows = None, []
        for chunk in _chunks(keys):
            cols, part = _rows(na, table, f'[{column}] IN ({",".join("?" * len(chunk))})', chunk)
            rows += part
        if cols is None:
            cols = columns(na, table)
        out[table] = (cols, rows)

    for table in ITEM_TABLES:
        gather(table, 'ItemClass', items)
    for table, column in SKILL_TABLES.items():
        gather(table, column, skills)
    # Recipes: RecipeMaterialInfo lists the product (Type 0) and the materials (Type 1); a recipe comes
    # when it makes an imported item from materials the 1.69 or the import has.
    have = known | items
    products, materials = {}, collections.defaultdict(list)
    for recipe_id, material, kind in na.execute('SELECT RecipeID, ItemClass, Type FROM RecipeMaterialInfo'):
        if kind == 0:
            products[recipe_id] = material
        else:
            materials[recipe_id].append(material)
    wanted = {r for r, product in products.items() if product in items}
    recipe_ids = {r for r in wanted if all(m in have for m in materials.get(r, []))}
    dropped['recipe with unknown material'] += len(wanted) - len(recipe_ids)
    for table in ('RecipeMaterialInfo', 'RecipeInfo', 'ManufactureRecipeInfo'):
        gather(table, 'RecipeID', recipe_ids)
    # Craft shops: the NA shop maps to the 1.69 shop that lists the same recipes (FERGHUS_Craft -> _Craft);
    # the class's recipes go after the 1.69 shop's last row, in the NA order.
    shop_map = collections.Counter()
    base_shops = {}
    for shop, recipe in base.execute('SELECT ShopID, RecipeID FROM CraftShopInfo'):
        base_shops.setdefault(recipe, shop)
    for shop, recipe in na.execute('SELECT ShopID, RecipeID FROM CraftShopInfo'):
        if recipe in base_shops:
            shop_map[(shop, base_shops[recipe])] += 1
    best = {}
    for (na_shop, shop), count in shop_map.most_common():
        best.setdefault(na_shop, shop)
    last = dict(base.execute('SELECT ShopID, max([Order]) FROM CraftShopInfo GROUP BY ShopID'))
    cols, rows = _rows(na, 'CraftShopInfo', '1', [])
    shop_at, order_at, recipe_at = cols.index('ShopID'), cols.index('Order'), cols.index('RecipeID')
    shop_rows = []
    for row in sorted((r for r in rows if r[recipe_at] in recipe_ids and r[shop_at] in best),
                      key=lambda r: (r[shop_at], r[order_at])):
        shop = best[row[shop_at]]
        last[shop] = last.get(shop, 0) + 1
        row = list(row)
        row[shop_at], row[order_at] = shop, last[shop]
        shop_rows.append(tuple(row))
    dropped['craft shop row without a 1.69 shop'] += sum(1 for r in rows if r[recipe_at] in recipe_ids and r[shop_at] not in best)
    out['CraftShopInfo'] = (cols, shop_rows)
    for table, key in MENTION_TABLES.items():
        cols = columns(na, table)
        text = ' OR '.join(f'lower(cast([{c}] AS TEXT)) LIKE ?' for c in cols)
        cols, rows = _rows(na, table, text, [f'%{name}%'] * len(cols))
        present = {tuple(r) for r in base.execute(f'SELECT {",".join(f"[{k}]" for k in key)} FROM [{table}]')}
        out[table] = (cols, [r for r in rows if tuple(r[cols.index(k)] for k in key) not in present])
    cols, rows = _rows(na, 'FeatureMatrix', 'lower(Feature) LIKE ?', [f'%{name}%'])
    present = {r[0] for r in base.execute('SELECT Feature FROM FeatureMatrix')}
    out['FeatureMatrix'] = (cols, [r for r in rows if r[0] not in present and not FEATURE_SKIP.search(r[0])])
    # Rows gated by a feature the 1.69 FeatureMatrix lacks (other NA classes, NA-only events) would
    # never be enabled and the server's profiles refuse unknown features: they stay out.
    known_features = {r[0].lower() for r in base.execute('SELECT Feature FROM FeatureMatrix') if r[0]}
    known_features |= {r[0].lower() for r in out['FeatureMatrix'][1]}
    for table, (cols, rows) in out.items():
        if 'Feature' not in cols or table == 'FeatureMatrix':
            continue
        at = cols.index('Feature')
        kept = [r for r in rows if not r[at] or all(t.lower() in known_features for t in FEATURE_TOKEN.findall(r[at]))]
        dropped[f'{table} row with unknown feature'] += len(rows) - len(kept)
        out[table] = (cols, kept)
    remaining = {r[0] for r in out['ItemClassInfo'][1]}
    for table in ITEM_TABLES[1:]:
        cols, rows = out[table]
        at = cols.index('ItemClass')
        kept = [r for r in rows if r[at] in remaining]
        dropped[f'{table} row of a dropped item'] += len(rows) - len(kept)
        out[table] = (cols, kept)
    if all(columns(na, t) and columns(base, t) for t in SET_TABLES):
        gather('SetItemInfo', 'ItemClass', remaining)
        cols, rows = out['SetItemInfo']
        base_sets = {r[0] for r in base.execute('SELECT SetID FROM SetInfo')}
        new_sets = {r[cols.index('SetID')] for r in rows} - base_sets
        gather('SetInfo', 'SetID', new_sets)
        gather('SetEffectInfo', 'SetID', new_sets)
    return out, {k: v for k, v in dropped.items() if v}


def build(name: str, weapon: str, index: int, stats_from: int, na_path: Path, base_path: Path, out_dir: Path) -> dict:
    if not NAME.match(name) or not NAME.match(weapon) or not 10 <= index < 32 or not 0 <= stats_from < index:
        raise ImportError_('Class and weapon names must be lower-case words, the class 10-31 and stats_from below it.')
    with closing(sqlite3.connect(f'file:{na_path}?mode=ro', uri=True)) as na, \
            closing(sqlite3.connect(f'file:{base_path}?mode=ro', uri=True)) as base:
        rows, dropped = select(na, base, name, weapon)
    out_dir.mkdir(parents=True, exist_ok=True)
    target = out_dir / f'{name}.db3'
    temporary = target.with_name(target.name + '.part')
    temporary.unlink(missing_ok=True)
    with closing(sqlite3.connect(temporary)) as db:
        db.execute('CREATE TABLE [_class](name TEXT, weapon TEXT, class_index INTEGER, stats_from INTEGER, source TEXT)')
        db.execute('INSERT INTO [_class] VALUES(?,?,?,?,?)', (name, weapon, index, stats_from, str(na_path)))
        for table, (cols, data) in rows.items():
            db.execute(f'CREATE TABLE [{table}]({",".join(f"[{c}]" for c in cols)})')
            db.executemany(f'INSERT INTO [{table}] VALUES({",".join("?" * len(cols))})', data)
        db.commit()
    temporary.replace(target)
    return {'database': str(target), 'rows': {t: len(d) for t, (_, d) in rows.items()}, 'dropped': dropped}


TOKEN_LINE = re.compile(r'^[ \t]*"([^"\r\n]+)"[ \t]+"((?:[^"\\\r\n]|\\.)*)"', re.M)


def class_text(classes: Path = CLASS_ASSETS, imports: list[Path] | None = None) -> dict[str, str]:
    """The staged classes' NA English tokens: keys naming the class or its weapon, and keys ending in
    the id of an imported item or skill (HEROES_ITEM_NAME_<ITEM>, HEROES_SKILL_DESC_<SKILL>...)."""
    tokens: dict[str, str] = {}
    ids: set[str] = set()
    for path in imports or []:
        with closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as imported:
            for table, column in (('ItemClassInfo', 'ItemClass'), ('SkillInfo', 'SkillID')):
                ids |= {str(r[0]).upper() for r in imported.execute(f'SELECT [{column}] FROM [{table}]')}
    for manifest in sorted(classes.glob('*/class-assets.json')):
        data = json.loads(manifest.read_text(encoding='utf-8'))
        if not data.get('text'):
            continue
        needles = (str(data.get('class', '')).lower(), str(data.get('weapon', '')).lower())
        if not all(re.fullmatch(r'[a-z]{2,16}', n) for n in needles):
            raise ImportError_(f'Bad class names in {manifest}.')
        file = (manifest.parent / 'merge' / 'text' / 'heroes_text_english.txt').resolve()
        if not file.is_relative_to(manifest.parent.resolve()) or not file.is_file():
            raise ImportError_(f'Missing staged text of {manifest.parent.name}.')
        for key, value in TOKEN_LINE.findall(file.read_bytes().decode('utf-16')):
            upper = key.upper()
            if any(n in key.lower() for n in needles) or any(
                    upper[i + 1:] in ids for i, c in enumerate(upper) if c == '_'):
                tokens.setdefault(key, value)
    return tokens

# --- install -------------------------------------------------------------------------------------

def load_imports(directory: Path = IMPORTS) -> list[Path]:
    return sorted(directory.glob('*.db3')) if directory.is_dir() else []


def meta(imported: sqlite3.Connection) -> dict:
    name, weapon, index, stats_from, _ = imported.execute('SELECT * FROM [_class]').fetchone()
    if not NAME.match(name) or not NAME.match(weapon) or not 10 <= index < 32:
        raise ImportError_('Bad class import metadata.')
    return {'name': name, 'weapon': weapon, 'index': index, 'stats_from': stats_from}


def digest(paths: list[Path], classes: Path = CLASS_ASSETS) -> str:
    """SHA-256 over the import databases and the staged classes' manifests (what the item exports read)."""
    h = hashlib.sha256()
    for path in sorted(paths) + sorted(classes.glob('*/class-assets.json')):
        h.update(path.name.encode() + b'\0' + hashlib.sha256(path.read_bytes()).digest())
    return h.hexdigest()


def read_meta(path: Path) -> dict:
    with closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as imported:
        return meta(imported)


def _defaults(db: sqlite3.Connection, table: str, schema: str, cols: list[str]) -> dict:
    out = {}
    for c in cols:
        row = db.execute(f'SELECT [{c}], count(*) n FROM {schema}.[{table}] GROUP BY 1 ORDER BY n DESC LIMIT 1').fetchone()
        out[c] = row[0] if row else None
    return out


def mastery_requirements(db: sqlite3.Connection, imported: sqlite3.Connection, info: dict,
                         schema: str = 'main') -> dict[str, tuple[str, int]]:
    """Imported weapon -> (mastery skill, rank) for the weapons without a requirement (MASTERY_REFERENCE)."""
    reference = MASTERY_REFERENCE.get(info['weapon'])
    skill = f"skill_{info['name']}_{info['weapon']}_mastery"
    needed = {'RequiredLevel', 'RequiredSkill', 'RequiredSkillRank'}
    if (not reference or not needed <= set(columns(db, 'ItemClassInfo', schema))
            or not needed <= set(columns(imported, 'ItemClassInfo'))
            or not imported.execute("SELECT 1 FROM sqlite_master WHERE name='SkillRankInfo'").fetchone()):
        return {}
    ranks = sorted({r[0] for r in imported.execute('SELECT [Rank] FROM SkillRankInfo WHERE SkillID=?', (skill,))})
    if not ranks:
        return {}
    by_name, by_level = {}, collections.defaultdict(collections.Counter)
    for item, level, required, rank in db.execute(
            f'SELECT i.ItemClass, i.RequiredLevel, i.RequiredSkill, i.RequiredSkillRank FROM {schema}.[ItemClassInfo] i '
            f'JOIN {schema}.[EquipItemInfo] e ON e.ItemClass = i.ItemClass WHERE upper(e.EquipClass) = ?', (reference.upper(),)):
        prefix = reference + '_'
        if item.lower().startswith(prefix):
            by_name[item.lower()[len(prefix):]] = rank if required else None
        if required:
            by_level[level][rank] += 1
    out = {}
    weapons = [r[0] for r in imported.execute('SELECT ItemClass FROM EquipItemInfo WHERE upper(EquipClass) = ?',
                                              (info['weapon'].upper(),))]
    levels = dict(imported.execute('SELECT ItemClass, RequiredLevel FROM ItemClassInfo WHERE RequiredSkill IS NULL'))
    prefix = info['weapon'] + '_'
    for item in weapons:
        if item not in levels:
            continue
        suffix = item.lower()[len(prefix):] if item.lower().startswith(prefix) else None
        if suffix in by_name:
            wanted = by_name[suffix]
        else:
            lower = [level for level in by_level if level <= (levels[item] or 0)]
            wanted = by_level[max(lower)].most_common(1)[0][0] if lower else None
        if not wanted:
            continue
        out[item] = (skill, max(r for r in ranks if r <= wanted) if any(r <= wanted for r in ranks) else ranks[0])
    return out


def converted(db: sqlite3.Connection, imported: sqlite3.Connection, table: str, info: dict, remap: dict,
              schema: str = 'main') -> tuple[list[str], list[tuple]]:
    """The imported rows of one table in the target's columns."""
    target = columns(db, table, schema)
    if not target or not imported.execute("SELECT 1 FROM sqlite_master WHERE name=?", (table,)).fetchone():
        return target, []
    cursor = imported.execute(f'SELECT * FROM [{table}]')
    source = [c[0] for c in cursor.description]
    defaults = _defaults(db, table, schema, [c for c in target if c not in source])
    # Booleans: the 1.69 stores some as the text True/False where the NA has 0/1 (CustomizeItemInfo
    # IsCash), and the other way round; each column takes the target table's form.
    text_bools, int_bools = set(), set()
    for c in (c for c in target if c in source):
        seen = {r[0] for r in db.execute(f'SELECT DISTINCT [{c}] FROM {schema}.[{table}] LIMIT 4')} - {None}
        if seen and seen <= {'True', 'False'}:
            text_bools.add(c)
        elif seen and seen <= {0, 1}:
            int_bools.add(c)
    mastery = mastery_requirements(db, imported, info, schema) if table == 'ItemClassInfo' else {}
    template = None
    if table == 'FeatureMatrix':
        template = db.execute(f'SELECT * FROM {schema}.[FeatureMatrix] WHERE Feature=?', (FEATURE_TEMPLATE,)).fetchone()
    rows = []
    for row in cursor:
        values = dict(zip(source, row))
        out = {c: values[c] if c in values else defaults[c] for c in target}
        if isinstance(out.get('Feature'), str) and table != 'FeatureMatrix':
            out['Feature'] = FEATURE_TOKEN.sub(lambda m: REGION_FEATURES.get(m.group(0).lower(), m.group(0)), out['Feature'])
        if out.get('CashShopType') in REGION_TAGS:
            out['CashShopType'] = REGION_TAGS[out['CashShopType']]
        for c in text_bools:
            if out[c] in (0, 1):
                out[c] = 'True' if out[c] else 'False'
        for c in int_bools:
            if out[c] in ('True', 'False'):
                out[c] = int(out[c] == 'True')
        if out.get('ItemClass') in mastery and 'RequiredSkill' in out:
            out['RequiredSkill'], out['RequiredSkillRank'] = mastery[out['ItemClass']]
        if 'ClassRestriction' in out and isinstance(out['ClassRestriction'], str):
            out['ClassRestriction'] = 1 << info['index'] if names_class(out['ClassRestriction'], info['name']) else 0
        if table == 'EquipItemInfo' and remap and out.get('CostumeType') is not None:
            section = remap.get('_category', {}).get(out['ItemClass'])
            out['CostumeType'] = int(remap.get((section, str(out['CostumeType'])), out['CostumeType']))
        if table == 'EquipItemInfo' and isinstance(out.get('CostumeType'), int) \
                and str(out.get('EquipClass', '')).lower() == info['weapon']:
            out['CostumeType'] = class_costumes.weapon_subtype(info['weapon'], out['CostumeType'])
        if template is not None:
            # The class's own feature holds the version that enables it: the Delia row's, which the
            # 1.69 client accepts (NA versions are 4.x). Other features (limits) keep the NA EN-US value.
            for column in (c for c in target if c.startswith('EN-EU')):
                if out['Feature'].lower() == info['name']:
                    out[column] = template[target.index(column)]
                elif out[column] is None:
                    out[column] = values.get(column.replace('EN-EU', 'EN-US'))
        rows.append(tuple(out[c] for c in target))
    return target, rows


def costume_remap_for(imported: sqlite3.Connection, info: dict, remap: dict) -> dict:
    """The player_costume remap with each imported item's section (Category -> section)."""
    if not remap:
        return {}
    categories = dict(imported.execute('SELECT ItemClass, Category FROM ItemClassInfo'))
    return {**remap, '_category': {k: class_costumes.SECTIONS.get(v) for k, v in categories.items()}}


def wearable_items(db: sqlite3.Connection, imported: sqlite3.Connection, info: dict, na_restrictions: dict,
                   schema: str = 'main') -> list[str]:
    """Existing items the class gets its bit on."""
    out = []
    for item, mask in db.execute(f'SELECT ItemClass, ClassRestriction FROM {schema}.[ItemClassInfo]'):
        if isinstance(mask, int) and mask & FEMALE_169 == FEMALE_169 and not mask & (1 << info['index']) \
                and not excludes_class(na_restrictions.get(item), info['name']):
            out.append(item)
    return out


def shared_skills(db: sqlite3.Connection, info: dict, na_restrictions: dict, schema: str = 'main') -> list[str]:
    """Existing skills the class gets its bit on: those every 1.69 class has (the NA gives them to every
    class but Grimden, '!grimden', or retired the armor masteries, 'nothing'), unless the NA leaves the
    class out."""
    out = []
    for skill, mask in db.execute(f'SELECT SkillID, ClassRestriction FROM {schema}.[SkillInfo]'):
        if isinstance(mask, int) and mask & ALL_169 == ALL_169 and not mask & (1 << info['index']) \
                and not excludes_class(na_restrictions.get(skill), info['name']):
            out.append(skill)
    return out


def na_restrictions(imported_path: Path, table: str = 'ItemClassInfo', key: str = 'ItemClass') -> dict:
    with closing(sqlite3.connect(f'file:{imported_path}?mode=ro', uri=True)) as imported:
        source = imported.execute('SELECT source FROM [_class]').fetchone()[0]
    with closing(sqlite3.connect(f'file:{source}?mode=ro', uri=True)) as na:
        return dict(na.execute(f'SELECT [{key}], ClassRestriction FROM [{table}]'))


def install_server(db: sqlite3.Connection, paths: list[Path], remap: dict | None = None) -> dict:
    """TEMP tables in front of the server content: temp._imported_<table> (rows after the original ones)
    and temp._class_base_ItemClassInfo (the original rows with the class bits); content_additions.install
    builds the views of its item tables on them, the other tables get their view here."""
    report = {}
    imported_rows: dict[str, list] = collections.defaultdict(list)
    bits: dict[str, int] = collections.defaultdict(int)
    skill_bits: dict[str, int] = collections.defaultdict(int)
    stats = []
    for path in paths:
        with closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as imported:
            info = meta(imported)
            class_remap = costume_remap_for(imported, info, (remap or {}).get(info['name'], {}))
            for table in TABLES:
                cols, rows = converted(db, imported, table, info, class_remap)
                imported_rows[table] += rows
            for item in wearable_items(db, imported, info, na_restrictions(path)):
                bits[item] |= 1 << info['index']
            for skill in shared_skills(db, info, na_restrictions(path, 'SkillInfo', 'SkillID')):
                skill_bits[skill] |= 1 << info['index']
            stats.append(info)
            report[info['name']] = {t: len(r) for t, r in imported_rows.items()}
    for table, rows in imported_rows.items():
        cols = columns(db, table)
        db.execute(f'CREATE TEMP TABLE [_imported_{table}] AS SELECT * FROM main.[{table}] WHERE 0')
        db.executemany(f'INSERT INTO temp.[_imported_{table}] VALUES({",".join("?" * len(cols))})', rows)
    for table, key in STAT_TABLES.items():
        if not columns(db, table):
            continue
        db.execute(f'CREATE TEMP TABLE [_imported_{table}] AS SELECT * FROM main.[{table}] WHERE 0')
        cols = columns(db, table)
        for info in stats:
            for row in db.execute(f'SELECT * FROM main.[{table}] WHERE [{key}]=?', (info['stats_from'],)).fetchall():
                values = dict(zip(cols, row))
                values[key] = info['index']
                db.execute(f'INSERT INTO temp.[_imported_{table}] VALUES({",".join("?" * len(cols))})',
                           [values[c] for c in cols])
        imported_rows.setdefault(table, [])
    if bits:
        db.execute('CREATE TEMP TABLE [_class_base_ItemClassInfo] AS SELECT * FROM main.[ItemClassInfo]')
        db.executemany('UPDATE temp.[_class_base_ItemClassInfo] SET ClassRestriction = ClassRestriction | ? WHERE ItemClass=?',
                       [(bit, item) for item, bit in bits.items()])
    if skill_bits:
        db.execute('CREATE TEMP TABLE [_class_base_SkillInfo] AS SELECT * FROM main.[SkillInfo]')
        db.executemany('UPDATE temp.[_class_base_SkillInfo] SET ClassRestriction = ClassRestriction | ? WHERE SkillID=?',
                       [(bit, skill) for skill, bit in skill_bits.items()])
    from content_additions import TABLES as ITEM_EDITOR_TABLES  # noqa: PLC0415 (circular at import time)
    based = {'ItemClassInfo': bool(bits), 'SkillInfo': bool(skill_bits)}
    for table in imported_rows:
        if table in ITEM_EDITOR_TABLES:
            continue  # content_additions.install builds these views (class base + imported + added rows)
        base = f'temp.[_class_base_{table}]' if based.get(table) else f'main.[{table}]'
        db.execute(f'CREATE TEMP VIEW [{table}] AS SELECT * FROM {base} UNION ALL SELECT * FROM temp.[_imported_{table}]')
    db.commit()
    report['class_bits'] = len(bits)
    report['skill_bits'] = len(skill_bits)
    return report


def apply_client(db: sqlite3.Connection, original: sqlite3.Connection | None, paths: list[Path],
                 remap: dict | None = None) -> dict:
    """The client copy's heroes.db3: each table an import touches comes back to its original rows
    (`original`, the copy before any laboratory change), then gets the imported rows and the class bits.
    Run before the item editor's rows (they go on top)."""
    report = {}
    tables = set(TABLES)
    if original is not None:
        for table in sorted(tables):
            if columns(db, table) and columns(original, table):
                db.execute(f'DELETE FROM main.[{table}]')
                cols = columns(db, table)
                db.executemany(f'INSERT INTO main.[{table}]({",".join(f"[{c}]" for c in cols)}) VALUES({",".join("?" * len(cols))})',
                               original.execute(f'SELECT {",".join(f"[{c}]" for c in cols)} FROM main.[{table}]'))
    for path in paths:
        with closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as imported:
            info = meta(imported)
            class_remap = costume_remap_for(imported, info, (remap or {}).get(info['name'], {}))
            counts = {}
            for table in TABLES:
                cols, rows = converted(db, imported, table, info, class_remap)
                if rows:
                    db.executemany(f'INSERT INTO main.[{table}]({",".join(f"[{c}]" for c in cols)}) '
                                   f'VALUES({",".join("?" * len(cols))})', rows)
                counts[table] = len(rows)
            items = wearable_items(db, imported, info, na_restrictions(path))
            db.executemany('UPDATE main.[ItemClassInfo] SET ClassRestriction = ClassRestriction | ? WHERE ItemClass=?',
                           [(1 << info['index'], item) for item in items])
            counts['class_bits'] = len(items)
            skills = shared_skills(db, info, na_restrictions(path, 'SkillInfo', 'SkillID'))
            db.executemany('UPDATE main.[SkillInfo] SET ClassRestriction = ClassRestriction | ? WHERE SkillID=?',
                           [(1 << info['index'], skill) for skill in skills])
            counts['skill_bits'] = len(skills)
            report[info['name']] = counts
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--class', dest='class_name', required=True)
    parser.add_argument('--weapon', required=True)
    parser.add_argument('--index', type=int, required=True)
    parser.add_argument('--stats-from', type=int, required=True, help='class whose stat rows the class copies')
    parser.add_argument('--na-database', type=Path, default=NA_DATABASE)
    parser.add_argument('--out', type=Path, default=IMPORTS)
    args = parser.parse_args()
    import content_additions  # noqa: PLC0415
    try:
        print(json.dumps(build(args.class_name, args.weapon, args.index, args.stats_from, args.na_database,
                               content_additions.SERVER_DATABASE, args.out)))
    except (ImportError_, sqlite3.Error, OSError) as error:
        print(json.dumps({'built': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
