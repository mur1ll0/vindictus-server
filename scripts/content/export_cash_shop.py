"""Laboratory cash shop catalog. The original categories, products, prices and balances lived
in the Nexon NISMS billing server, which is not part of the evidence; the operator chose
(2026-10-24) the cash-tab items (ItemClassInfo.Storage 2) grouped by ItemClassInfo.Category,
priced by scripts/content/cash_shop_prices.json. The export keeps ItemClassInfo.Category
aligned with the catalog's class rows; the server builds the products from the effective
classes. It also carries the beauty shop tables (CustomizeItemInfo, CustomizePriceInfo,
CustomizeCouponInfo, ItemContents.LoadCustomizedItem) with LanguageTag and the region values
of their features."""
import argparse, contextlib, json, pathlib, sqlite3
from export_item_catalog import read_package as catalog_read, digest
from export_npc_scripts import region_features
from export_reward_policy import feature_atoms

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRICES = pathlib.Path(__file__).resolve().parent / 'cash_shop_prices.json'


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


def read_package(database, config, prices_path=PRICES):
    package = catalog_read(database, config)
    manifest = package['manifest']
    prices = json.loads(prices_path.read_text(encoding='utf-8'))
    default = prices['default']
    table = prices['categories']
    if type(default) is not int or default <= 0 or any(
            type(v) is not int or v <= 0 or not text(k) for k, v in table.items()):
        raise ValueError('Cash shop price table')
    with contextlib.closing(sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        db.row_factory = sqlite3.Row
        categories = []
        for index, r in enumerate(db.execute('Select ItemClass, Feature, Category From [ItemClassInfo]')):
            if index >= len(package['classes']):
                raise ValueError('Catalog alignment changed')
            if (r['ItemClass'].encode('utf-16-le').hex() != package['classes'][index]['item_utf16']
                    or r['Feature'] != package['classes'][index]['feature']):
                raise ValueError('Catalog order changed')
            categories.append(text(r['Category']))
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
        values = region_features(db, config)
    atoms = {'LanguageTag', 'AvatarBeauty', 'CashShop'}
    for r in items + prices + coupons:
        atoms |= feature_atoms(r['feature'] or '')
    if len(atoms) > 512 or any(not k.isascii() or not k or len(k) > 256 for k in atoms):
        raise ValueError('Cash shop feature subset bound')
    features = [dict(key=k, value=values.get(k)) for k in sorted(atoms)]
    if digest(database) != manifest['database_sha256'] or digest(config) != manifest['config_sha256']:
        raise ValueError('Cash shop source changed during export')
    return dict(format_version=2, source='heroesContents.cash-shop', database_sha256=manifest['database_sha256'],
                config_sha256=manifest['config_sha256'], root_category=105, categories=categories,
                default_price=default, prices=[dict(category=k, price=v) for k, v in sorted(table.items())],
                customize_items=items, customize_prices=prices, customize_coupons=coupons, features=features)


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
    print('Cash shop catalog exported: %d class categories, %d prices; values private'
          % (len(package['categories']), len(package['prices'])))


if __name__ == '__main__':
    main()
