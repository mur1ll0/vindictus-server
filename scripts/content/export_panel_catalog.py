"""Admin panel catalog (B4.4b): which NPC opens each shop and where it stands, the NPCs' English
names, and the icon of each item class, for the shop and inventory screens. Read-only.

- NPCs and places: the server's NPC talk XML (Data/XML/**/*.xml), `<Talk id=NPC location=...>`
  with `<open_shop>SHOP</open_shop>` inside; an empty `<open_shop>` opens the NPC's default
  `<NPC>_Shop`. The location prefix names the place (t01 Colhen, rochest, morvan, event, ...).
- Names: HEROES_NPC_<ID> and GAMEUI_HEROES_NPCDESCRIPTION_<ID> of the client's english_eu text.
- Icons: ItemClassInfo.Icon (and IconBG, drawn under it) of the server content; the client's
  scaleform/item_icon/<Icon>.tga converted to PNG under .cache/private-content/icons/item (game
  assets, never committed). Equipment icons are dye masks: red, green and blue mark the parts
  painted with the item's color 1, 2 and 3.
- Tooltip (B4.4e): the static fields the client's ItemToolTip.gfx shows (setBaseInfo, setStats,
  addConstraint, setItemTrade, setCondition, setSetInfo) from ItemClassInfo, EquipItemInfo and
  SetItemInfo, and the client's own texts for them (GameUI_Heroes_ItemToolTip*, ItemStat,
  EnchantStat, EnchantCondition, Attribute prefix/suffix names, ItemConstraint, set names).
- Item editor (docs/future/item-editor.md, cut 2): the editable columns of every item
  (content_additions.EDITABLE, first row), its origin (base, added, or base edited by the panel),
  whether the panel hid it, and the definition of added items, for the panel's item form.
- Name aliases: a class whose english_eu text was never translated (Korean) takes the English
  name and description of another class with the same icon (the same design for another
  character class, e.g. fiona_noluck_inner_devilhunter → delia_inner_devilhunter)."""
import argparse, contextlib, json, pathlib, re, sqlite3
import content_additions
import xml.etree.ElementTree as ET
from export_cash_shop import LOCALIZED, UNTRANSLATED
from export_item_catalog import digest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SERVER = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm'
CLIENT = ROOT.parent / 'Vindictus1.69EN/Vindictus EU/en-EU/hfs_extracted'
OUTPUT_ROOT = ROOT / '.cache/private-content'
NAME = re.compile(r'^\s*"((?:HEROES_NPC|GAMEUI_HEROES_NPCDESCRIPTION)_[^"]+)"\s+"(.*)"\s*$', re.M)
# Client texts of the item tooltip, keyed like the client (upper case).
TOOLTIP = re.compile(r'^\s*"((?:GAMEUI_HEROES_ITEM_?TOOLTIP|GAMEUI_HEROES_ITEMSTAT|GAMEUI_HEROES_ENCHANTSTAT|'
                     r'GAMEUI_HEROES_ENCHANTCONDITION|GAMEUI_HEROES_ATTRIBUTE_(?:PREFIX|SUFFIX)|GAMEUI_HEROES_ITEMCONSTRAINT|'
                     r'HEROES_SET_ITEM|HEROES_ITEM_SETITEM)[^"]*)"\s+"(.*)"\s*$', re.M | re.I)
QUEST_TITLE = re.compile(r'^\s*"HEROES_QUEST_TITLE_([^"]+)"\s+"(.*)"\s*$', re.M | re.I)
ITEM_TEXT = re.compile(r'^\s*"(HEROES_ITEM_(?:NAME|DESC)_[^"]+)"\s+"(.*)"\s*$', re.M)
# ItemClassInfo / EquipItemInfo columns the tooltip shows (short key in the catalog).
CLASS_FIELDS = [('Rarity', 'r'), ('Category', 'c'), ('SellPrice', 'sp'), ('Price', 'p'), ('RequiredLevel', 'rl'),
                ('RequiredSkill', 'rs'), ('RequiredSkillRank', 'rr'), ('ClassRestriction', 'cr'), ('Unique', 'u'),
                ('MaxAntiBind', 'ab'), ('TradeRestirction', 'tr'), ('Bind', 'b'), ('ExpireIn', 'ex'), ('IconBG', 'bg'),
                ('Level', 'lv'), ('MaxStack', 'ms')]
EQUIP_FIELDS = [('EquipClass', 'ec'), ('CostumeType', 'ct'), ('MaxDurability', 'md'), ('Weight', 'w'), ('ColorOverride', 'co'),
                ('Enchantable', 'en'), ('Dyeable', 'dy'), ('Synthesizable', 'sy'), ('EnhanceType', 'et')]
ICON = re.compile(r'^[A-Za-z0-9_]{1,96}$')
KEYLIKE = re.compile(r'^[A-Za-z0-9_]{1,128}$')
# Location prefix → place shown by the panel.
PLACES = [('t01', 'Colhen'), ('rochest', 'Rocheste'), ('morvan', 'Malina (Morvan)'), ('field1', 'Campos'),
          ('event', 'Eventos'), ('ship', 'Navios'), ('dev', 'Desenvolvimento'), ('armor', 'Ofícios'),
          ('metal', 'Ofícios'), ('sewing', 'Ofícios'), ('workmanship', 'Ofícios')]


def place(location):
    prefix = (location or '').split('_')[0].split('.')[0].lower()
    for key, name in PLACES:
        if prefix == key or prefix.startswith(key):
            return name
    return 'Outros'


def npc_shops(xml_root):
    """(npc, location, shop) of every <open_shop> inside a <Talk>."""
    found = set()
    for path in sorted(xml_root.rglob('*.xml')):
        if path.stat().st_size > 16 * 1024 * 1024:
            raise ValueError('NPC XML bound')
        try:
            tree = ET.fromstring(path.read_bytes())
        except ET.ParseError:
            continue
        for talk in tree.iter('Talk'):
            npc, location = talk.get('id'), talk.get('location')
            if not npc:
                continue
            for shop in talk.iter('open_shop'):
                found.add((npc, location or '', (shop.text or '').strip() or npc + '_Shop'))
    return found


def read_package(xml_root=SERVER / 'Data/XML', database=SERVER / 'Bin/heroesContents.db3', localized=LOCALIZED,
                 panel=None):
    data = localized.read_bytes()
    if len(data) > 64 * 1024 * 1024 or data[:2] != b'\xff\xfe':
        raise ValueError('Localized text must be bounded UTF-16LE')
    # The localization writes line breaks as the two characters "\n".
    decoded = data.decode('utf-16')
    texts = {k: v.replace('\\n', '\n') for k, v in NAME.findall(decoded)
             if not UNTRANSLATED.search(v) and len(v) <= 512}
    tooltip_text = {k.upper(): v.replace('\\n', '\n') for k, v in TOOLTIP.findall(decoded)
                    if not UNTRANSLATED.search(v) and len(v) <= 1024}
    item_text = {k.upper(): v for k, v in ITEM_TEXT.findall(decoded)}
    quest_titles = {k.upper(): v for k, v in QUEST_TITLE.findall(decoded)
                    if KEYLIKE.match(k) and not UNTRANSLATED.search(v) and len(v) <= 256}
    with contextlib.closing(content_additions.connect(database)) as db:
        db.execute('PRAGMA query_only=ON')
        shop_ids = {r[0].lower(): r[0] for r in db.execute('SELECT DISTINCT ShopID FROM ShopInfo') if r[0]}
        icons = {}
        for item, icon in db.execute('SELECT ItemClass, Icon FROM ItemClassInfo'):
            if item and icon and ICON.match(icon) and ICON.match(item):
                icons.setdefault(item.upper(), icon.lower())
        info = item_info(db)
        if panel is None:
            server = pathlib.Path(database).resolve() == content_additions.SERVER_DATABASE.resolve()
            panel = (content_additions.load(), content_additions.panel_edits()) if server else ([], None)
        added, edits = panel
        edits = content_additions.panel_edits(edits or {})
        editor = item_edit(db, added, edits['item_overrides'], edits['hidden_items'])
    aliases = name_aliases(icons, item_text)
    shops, npcs = {}, {}
    for npc, location, shop in npc_shops(pathlib.Path(xml_root)):
        real = shop_ids.get(shop.lower())
        if real is None:
            continue
        entry = {'npc': npc, 'location': location, 'place': place(location)}
        if entry not in shops.setdefault(real, []):
            shops[real].append(entry)
        npcs[npc.upper()] = [texts.get('HEROES_NPC_' + npc.upper()), texts.get('GAMEUI_HEROES_NPCDESCRIPTION_' + npc.upper())]
    for entries in shops.values():
        entries.sort(key=lambda e: (e['place'], e['npc']))
    return dict(format_version=1, source='npc-talk-xml+heroes_text_english_eu', localized_sha256=digest(localized),
                database_sha256=digest(database), shops=dict(sorted(shops.items())), npcs=dict(sorted(npcs.items())),
                item_icons=dict(sorted(icons.items())), item_info=dict(sorted(info.items())),
                item_aliases=dict(sorted(aliases.items())), item_edit=dict(sorted(editor.items())),
                tooltip_text=dict(sorted(tooltip_text.items())),
                quest_titles=dict(sorted(quest_titles.items())))


def columns(db, table):
    return {r[1] for r in db.execute('PRAGMA table_info([%s])' % table)}


def item_info(db):
    """Class (upper case) → the tooltip's static fields; defaults (None, 0, '') left out."""
    def pick(row, fields):
        out = {}
        for (_, key), value in zip(fields, row):
            if isinstance(value, float) and value.is_integer():
                value = int(value)
            if isinstance(value, str) and value.lower() in ('true', 'false'):
                value = int(value.lower() == 'true')
            if value not in (None, '', 0):
                out[key] = value
        return out
    have = columns(db, 'ItemClassInfo')
    fields = [f for f in CLASS_FIELDS if f[0] in have]
    info = {}
    if fields:
        for row in db.execute('SELECT ItemClass,' + ','.join('[%s]' % f for f, _ in fields) + ' FROM ItemClassInfo'):
            if row[0] and ICON.match(row[0]):
                info.setdefault(row[0].upper(), pick(row[1:], fields))
    have = columns(db, 'EquipItemInfo')
    fields = [f for f in EQUIP_FIELDS if f[0] in have]
    if fields:
        for row in db.execute('SELECT ItemClass,' + ','.join('[%s]' % f for f, _ in fields) + ' FROM EquipItemInfo'):
            if row[0] and row[0].upper() in info:
                for key, value in pick(row[1:], fields).items():
                    info[row[0].upper()].setdefault(key, value)
    if {'ItemClass', 'SetID'} <= columns(db, 'SetItemInfo'):
        for item, set_id in db.execute('SELECT ItemClass, SetID FROM SetItemInfo'):
            if item and set_id and item.upper() in info:
                info[item.upper()].setdefault('set', set_id)
    return info


def item_edit(db, added=(), overrides=None, hidden=()):
    """Class (upper case) → the item editor's view: f = editable columns of the item's first row of
    each table (None, 0 and '' left out), o = origin (b base, a added, e base edited by the panel),
    h = 1 when hidden, d = the definition of an added item (template, set, text, costumes, assets)."""
    out, seen = {}, set()
    for table, wanted in content_additions.EDITABLE.items():
        fields = [f for f in wanted if f in columns(db, table)]
        if not fields:
            continue
        for row in db.execute('SELECT ItemClass,' + ','.join('[%s]' % f for f in fields) + ' FROM [%s]' % table):
            if not row[0] or not ICON.match(row[0]) or (row[0], table) in seen:
                continue
            seen.add((row[0], table))
            entry = out.setdefault(row[0].upper(), {'o': 'b', 'f': {}})
            for field, value in zip(fields, row[1:]):
                if isinstance(value, float) and value.is_integer():
                    value = int(value)
                if value not in (None, '', 0):
                    entry['f'][field] = value
    for name in overrides or {}:
        if name.upper() in out:
            out[name.upper()]['o'] = 'e'
    for item in added:
        if item['ItemClass'].upper() in out:
            out[item['ItemClass'].upper()].update(o='a', d=item)
    for name in hidden:
        if name.upper() in out:
            out[name.upper()]['h'] = 1
    return out


def name_aliases(icons, item_text):
    """Classes without an English name → a class with the same icon that has one."""
    def english(cls):
        name = item_text.get('HEROES_ITEM_NAME_' + cls)
        return name is not None and len(name) <= 256 and not UNTRANSLATED.search(name)
    by_icon = {}
    for cls, icon in sorted(icons.items()):
        if english(cls):
            by_icon.setdefault(icon, cls)
    return {cls: by_icon[icon] for cls, icon in icons.items() if not english(cls) and icon in by_icon}


def convert_icons(icons, source=CLIENT / 'scaleform/item_icon', target=OUTPUT_ROOT / 'icons/item'):
    """The referenced item icons as PNG; existing files are kept (rerun-safe)."""
    from PIL import Image
    target.mkdir(parents=True, exist_ok=True)
    done = missing = 0
    for icon in sorted(set(icons)):
        out = target / (icon + '.png')
        if out.exists():
            done += 1
            continue
        src = source / (icon + '.tga')
        if not src.is_file():
            missing += 1
            continue
        with Image.open(src) as image:
            image.convert('RGBA').save(out, optimize=True)
        done += 1
    return done, missing


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    parser.add_argument('--no-icons', action='store_true')
    args = parser.parse_args()
    output = pathlib.Path(args.output).resolve()
    if output.parent != OUTPUT_ROOT.resolve():
        raise ValueError('Private output outside ACL cache')
    package = read_package()
    output.write_text(json.dumps(package, ensure_ascii=True, separators=(',', ':')), encoding='utf-8', newline='\n')
    wanted = set(package['item_icons'].values()) | {i['bg'].lower() for i in package['item_info'].values()
                                                     if isinstance(i.get('bg'), str) and ICON.match(i['bg'])}
    icons = (0, 0) if args.no_icons else convert_icons(wanted)
    print('Panel catalog exported: %d shops with NPCs, %d NPCs, %d item icons (%d PNG, %d missing), '
          '%d tooltip classes, %d name aliases, %d tooltip texts'
          % (len(package['shops']), len(package['npcs']), len(package['item_icons']), icons[0], icons[1],
             len(package['item_info']), len(package['item_aliases']), len(package['tooltip_text'])))


if __name__ == '__main__':
    main()
