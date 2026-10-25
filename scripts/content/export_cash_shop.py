"""Laboratory cash shop catalog. The original categories, products, prices and balances lived
in the Nexon NISMS billing server, which is not part of the evidence; the operator chose
(2026-10-24) the cash-tab items (ItemClassInfo.Storage 2) grouped by ItemClassInfo.Category,
priced by scripts/content/cash_shop_prices.json. The export keeps ItemClassInfo.Category
aligned with the catalog's class rows; the server builds the products from the effective
classes. It also carries the beauty shop tables (CustomizeItemInfo, CustomizePriceInfo,
CustomizeCouponInfo, ItemContents.LoadCustomizedItem) with LanguageTag and the region values
of their features, and the client's english_eu item names and descriptions
(HEROES_ITEM_NAME_/HEROES_ITEM_DESC_<CLASS> of heroes_text_english_eu.txt) shown by the
in-game cash shop; a class whose EU text is still Korean (never released there) has none."""
import argparse, contextlib, json, pathlib, re, sqlite3
from export_item_catalog import read_package as catalog_read, digest
from export_npc_scripts import region_features
from export_reward_policy import feature_atoms

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRICES = pathlib.Path(__file__).resolve().parent / 'cash_shop_prices.json'
LOCALIZED = (ROOT.parent / 'Vindictus1.69EN/Vindictus EU/en-EU/hfs_extracted/resource/localized_text'
             / 'english_eu/heroes_text_english_eu.txt')
ENTRY = re.compile(r'^\s*"(HEROES_ITEM_(?:NAME|DESC)_[^"]+)"\s+"(.*)"\s*$', re.M)
# Hangul, CJK and kana: text the EU localization never translated.
UNTRANSLATED = re.compile('[\u1100-\u11ff\u3040-\u30ff\u3130-\u318f\u4e00-\u9fff\uac00-\ud7a3]')


def localized_entries(path):
    data = path.read_bytes()
    if len(data) > 64 * 1024 * 1024 or data[:2] != b'\xff\xfe':
        raise ValueError('Localized text must be bounded UTF-16LE')
    return dict(ENTRY.findall(data.decode('utf-16')))


def localized(entries, item):
    key = item.upper()
    name = entries.get('HEROES_ITEM_NAME_' + key)
    if not name or UNTRANSLATED.search(name) or len(name) > 256:
        return None
    desc = entries.get('HEROES_ITEM_DESC_' + key)
    if desc is not None:
        desc = desc.replace('\\n', '\n')
        if UNTRANSLATED.search(desc) or len(desc) > 4096:
            desc = None
    return dict(name=name, description=desc or None)


def text(v, limit=256):
    if v is None:
        return None
    if not isinstance(v, str) or not v.isascii() or len(v) > limit:
        raise ValueError('Cash shop category text')
    return v


def integer(v):
    if type(v) is not int or not -(2**31) <= v < 2**31:
        raise ValueError('Cash shop integer')
    return v


def read_package(database, config, prices_path=PRICES, localized_path=LOCALIZED):
    package = catalog_read(database, config)
    entries = localized_entries(localized_path)
    manifest = package['manifest']
    prices = json.loads(prices_path.read_text(encoding='utf-8'))
    default = prices['default']
    table = prices['categories']
    if type(default) is not int or default <= 0 or any(
            type(v) is not int or v <= 0 or not text(k) for k, v in table.items()):
        raise ValueError('Cash shop price table')
    value_products = prices.get('value_products', {})
    if any(not text(k) or not v or any(type(d) is not int or not 0 < d <= 3650 for d in v)
           for k, v in value_products.items()):
        raise ValueError('Cash shop VALUE products')
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        db.row_factory = sqlite3.Row
        categories = []
        restrictions = []
        names = []
        boxed = []
        # RandomItem/RandomItemV2 open a box; a box class without them has no contents.
        openable = {r[0] for r in db.execute('Select ItemClass From [RandomItem] Union Select ItemClass From [RandomItemV2]')}
        for index, r in enumerate(db.execute('Select ItemClass, Feature, Category, ClassRestriction From [ItemClassInfo]')):
            if index >= len(package['classes']):
                raise ValueError('Catalog alignment changed')
            if (r['ItemClass'].encode('utf-16-le').hex() != package['classes'][index]['item_utf16']
                    or r['Feature'] != package['classes'][index]['feature']):
                raise ValueError('Catalog order changed')
            categories.append(text(r['Category']))
            restrictions.append(integer(r['ClassRestriction']))
            names.append(localized(entries, r['ItemClass']))
            if ((r['ItemClass'].startswith('randomitem_') or r['Category'] == 'PACKAGE')
                    and r['ItemClass'] not in openable):
                boxed.append(index)
        if len(categories) != len(package['classes']):
            raise ValueError('Catalog alignment changed')
        items = [dict(item=text(r['ItemClass']), category=text(r['Category']), shop=text(r['CashShopType']),
                      feature=text(r['Feature'])) for r in db.execute('Select * From [CustomizeItemInfo]')]
        prices = [dict(item=text(r['ItemClass']), duration=integer(r['Duration']), price=integer(r['Price']),
                       shop=text(r['CashShopType']), feature=text(r['Feature']))
                  for r in db.execute('Select * From [CustomizePriceInfo]')]
        coupons = [dict(item=text(r['ItemClass']), category=text(r['Category']), price=integer(r['Price']),
                        shop=text(r['CashShopType']), feature=text(r['Feature']))
                   for r in db.execute('Select * From [CustomizeCouponInfo]')]
        def nullable(v):
            return None if v is None else integer(v)
        colors = [dict(category=text(r['Category']), order=integer(r['Order']), color=integer(r['Color']),
                       shop=text(r['CashShopType']), feature=text(r['Feature']))
                  for r in db.execute('Select * From [CustomizeColorInfo]')]
        tir_ignored = [dict(item=text(r['ItemClass']), duration=nullable(r['Duration']), price=nullable(r['Price']),
                            shop=text(r['CashShopType']), feature=text(r['Feature']))
                       for r in db.execute('Select * From [Cashshop_Tircoin_notsell_list]')]
        values = region_features(db, config)
    atoms = {'LanguageTag', 'AvatarBeauty', 'CashShop', 'TradeAndCashShopLevelLimit'}
    for r in items + prices + coupons + colors + tir_ignored:
        atoms |= feature_atoms(r['feature'] or '')
    if len(atoms) > 512 or any(not k.isascii() or not k or len(k) > 256 for k in atoms):
        raise ValueError('Cash shop feature subset bound')
    features = [dict(key=k, value=values.get(k)) for k in sorted(atoms)]
    if digest(database) != manifest['database_sha256'] or digest(config) != manifest['config_sha256']:
        raise ValueError('Cash shop source changed during export')
    return dict(format_version=5, source='heroesContents.cash-shop', database_sha256=manifest['database_sha256'],
                config_sha256=manifest['config_sha256'], localized_sha256=digest(localized_path),
                root_category=105, categories=categories, names=names, without_contents=boxed,
                value_products=[dict(item=k, days=v) for k, v in sorted(value_products.items())],
                default_price=default, prices=[dict(category=k, price=v) for k, v in sorted(table.items())],
                customize_items=items, customize_prices=prices, customize_coupons=coupons,
                customize_colors=colors, tir_ignored=tir_ignored, class_restrictions=restrictions, features=features)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    base = ROOT.parent / 'ServidorVindictus/Server/zh-tw-x-gm/Bin'
    output = pathlib.Path(args.output).resolve()
    if output.parent != (ROOT / '.cache/private-content').resolve():
        raise ValueError('Private output outside ACL cache')
    package = read_package(base / 'heroesContents.db3', base / 'ServiceCore.dll.config')
    output.write_text(json.dumps(package, ensure_ascii=True, separators=(',', ':')), encoding='utf-8', newline='\n')
    print('Cash shop catalog exported: %d class categories, %d english names, %d prices; values private'
          % (len(package['categories']), sum(n is not None for n in package['names']), len(package['prices'])))


if __name__ == '__main__':
    main()
