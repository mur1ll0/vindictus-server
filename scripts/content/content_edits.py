"""Published content edits of the admin panel (B4.4), as `vindictus-server content-edits-export`
writes them to .cache/private-content/content-edits.json: ShopInfo rows to add, change or
remove, and cash shop prices. Applied after the original rows and lab_content_additions.json,
by export_npc_scripts.py, export_cash_shop.py and scripts/client/apply_lab_content_additions.py.
A missing file is an empty overlay."""
import json, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
EDITS = ROOT / '.cache/private-content/content-edits.json'
EMPTY = {'format_version': 1, 'version': 0, 'shop_rows': [], 'cash_prices': {}, 'cash_default': None}
MAX_BYTES = 8 * 1024 * 1024


def _int(v, low, high, what):
    if type(v) is not int or not low <= v <= high:
        raise ValueError('Content edit ' + what)
    return v


def _text(v, what, limit=128):
    if not isinstance(v, str) or not v or len(v) > limit or not v.isascii():
        raise ValueError('Content edit ' + what)
    return v


def load(path=EDITS):
    path = pathlib.Path(path)
    if not path.exists():
        return EMPTY
    data = path.read_bytes()
    if len(data) > MAX_BYTES:
        raise ValueError('Content edits exceed bound')
    overlay = json.loads(data)
    if overlay.get('format_version') != 1:
        raise ValueError('Content edits format')
    for r in overlay['shop_rows']:
        _text(r['ShopID'], 'ShopID'); _int(r['Order'], 0, 2**15 - 1, 'Order')
        if not r.get('Remove'):
            _text(r['ItemClass'], 'ItemClass', 50); _text(r['PriceItemClass'], 'PriceItemClass', 50)
            _int(r['Count'], 1, 9999, 'Count'); _int(r['PriceCount'], 1, 2_000_000_000, 'PriceCount')
    for k, v in overlay['cash_prices'].items():
        _text(k, 'category'); _int(v, 1, 1_000_000, 'cash price')
    if overlay['cash_default'] is not None:
        _int(overlay['cash_default'], 1, 1_000_000, 'cash default')
    return overlay


def apply_shop_rows(rows, overlay):
    """ShopInfo dict rows with the overlay: remove, change the sold fields, or add a row."""
    out = list(rows)
    index = {(r['ShopID'], r['Order']): i for i, r in enumerate(out)}
    removed = set()
    for e in overlay['shop_rows']:
        key = (e['ShopID'], e['Order'])
        if e.get('Remove'):
            if key in index:
                removed.add(index[key])
            continue
        fields = {k: e[k] for k in ('ItemClass', 'Count', 'PriceItemClass', 'PriceCount')}
        if key in index:
            removed.discard(index[key])
            out[index[key]] = {**out[index[key]], **fields}
        else:
            index[key] = len(out)
            out.append({'ShopID': e['ShopID'], 'Order': e['Order'], **fields, 'AppearQuestID': None,
                        'CharacterBind': False, 'ID': None, 'RestrictionResetTime': None,
                        'RestrictionPeriodDay': 0, 'RestrictionCount': 0, 'Feature': None})
    return [r for i, r in enumerate(out) if i not in removed]


def apply_prices(default, table, overlay):
    """Cash shop default price and category prices with the overlay."""
    table = {**table, **overlay['cash_prices']}
    if overlay['cash_default'] is not None:
        default = overlay['cash_default']
    return default, table
