"""Admin panel catalog (B4.4b): which NPC opens each shop and where it stands, the NPCs' English
names, and the icon of each item class, for the shop and inventory screens. Read-only.

- NPCs and places: the server's NPC talk XML (Data/XML/**/*.xml), `<Talk id=NPC location=...>`
  with `<open_shop>SHOP</open_shop>` inside; an empty `<open_shop>` opens the NPC's default
  `<NPC>_Shop`. The location prefix names the place (t01 Colhen, rochest, morvan, event, ...).
- Names: HEROES_NPC_<ID> and GAMEUI_HEROES_NPCDESCRIPTION_<ID> of the client's english_eu text.
- Icons: ItemClassInfo.Icon of the server content; the client's scaleform/item_icon/<Icon>.tga
  converted to PNG under .cache/private-content/icons/item (game assets, never committed)."""
import argparse, contextlib, json, pathlib, re, sqlite3
import xml.etree.ElementTree as ET
from export_cash_shop import LOCALIZED, UNTRANSLATED
from export_item_catalog import digest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SERVER = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm'
CLIENT = ROOT.parent / 'Vindictus1.69EN/Vindictus EU/en-EU/hfs_extracted'
OUTPUT_ROOT = ROOT / '.cache/private-content'
NAME = re.compile(r'^\s*"((?:HEROES_NPC|GAMEUI_HEROES_NPCDESCRIPTION)_[^"]+)"\s+"(.*)"\s*$', re.M)
ICON = re.compile(r'^[A-Za-z0-9_]{1,96}$')
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


def read_package(xml_root=SERVER / 'Data/XML', database=SERVER / 'Bin/heroesContents.db3', localized=LOCALIZED):
    data = localized.read_bytes()
    if len(data) > 64 * 1024 * 1024 or data[:2] != b'\xff\xfe':
        raise ValueError('Localized text must be bounded UTF-16LE')
    # The localization writes line breaks as the two characters "\n".
    texts = {k: v.replace('\\n', '\n') for k, v in NAME.findall(data.decode('utf-16'))
             if not UNTRANSLATED.search(v) and len(v) <= 512}
    with contextlib.closing(sqlite3.connect(pathlib.Path(database).resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        shop_ids = {r[0].lower(): r[0] for r in db.execute('SELECT DISTINCT ShopID FROM ShopInfo') if r[0]}
        icons = {}
        for item, icon in db.execute('SELECT ItemClass, Icon FROM ItemClassInfo'):
            if item and icon and ICON.match(icon) and ICON.match(item):
                icons.setdefault(item.upper(), icon.lower())
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
                item_icons=dict(sorted(icons.items())))


def convert_icons(icons, source=CLIENT / 'scaleform/item_icon', target=OUTPUT_ROOT / 'icons/item'):
    """The referenced item icons as PNG; existing files are kept (rerun-safe)."""
    from PIL import Image
    target.mkdir(parents=True, exist_ok=True)
    done = missing = 0
    for icon in sorted(set(icons.values())):
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
    icons = (0, 0) if args.no_icons else convert_icons(package['item_icons'])
    print('Panel catalog exported: %d shops with NPCs, %d NPCs, %d item icons (%d PNG, %d missing)'
          % (len(package['shops']), len(package['npcs']), len(package['item_icons']), icons[0], icons[1]))


if __name__ == '__main__':
    main()
