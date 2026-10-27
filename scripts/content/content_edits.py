"""Published content edits of the admin panel (B4.4), as `vindictus-server content-edits-export`
writes them to .cache/private-content/content-edits.json: ShopInfo rows to add, change or
remove, and cash shop prices. Applied after the original rows and lab_content_additions.json,
by export_npc_scripts.py, export_cash_shop.py and scripts/client/apply_lab_content_additions.py.
The item editor's parts (items, item_overrides, hidden_items, removed_items) are read by
content_additions.py. A missing file is an empty overlay."""
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
    # B4.5 tuning (absent from overlays exported before it).
    for k in ('features', 'quest_drops', 'enhance_ratios', 'enchant_ratios'):
        overlay.setdefault(k, {})
    for k, v in overlay['features'].items():
        _text(k, 'feature'); _int(v, 0, 10_000, 'rate')
    for k, (low, high) in overlay['quest_drops'].items():
        _text(k, 'quest'); _int(high, 0, 100, 'drop max'); _int(low, 0, high, 'drop min')
    for k, v in overlay['enhance_ratios'].items():
        _text(k, 'enhance'); _int(int(k.rsplit('#', 1)[1]), 0, 99, 'enhance level')
        if not isinstance(v, (int, float)) or not 0 <= v <= 1:
            raise ValueError('Content edit enhance ratio')
    for k, (low, high) in overlay['enchant_ratios'].items():
        _text(k, 'enchant'); _int(high, 0, 100, 'enchant max'); _int(low, 0, high, 'enchant min')
    return overlay


def apply_shop_rows(rows, overlay):
    """ShopInfo dict rows with the overlay: remove, change the sold fields, or add a row. Items the
    item editor hid (hidden_items) leave every shop."""
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
    hidden = set(overlay.get('hidden_items', []))
    return [r for i, r in enumerate(out) if i not in removed and r['ItemClass'] not in hidden]


def apply_prices(default, table, overlay):
    """Cash shop default price and category prices with the overlay."""
    table = {**table, **overlay['cash_prices']}
    if overlay['cash_default'] is not None:
        default = overlay['cash_default']
    return default, table


def apply_client_tuning(db, overlay):
    """The tuning cells the client shows, in its heroes.db3: QuestInfo drop counts, EnchantInfo
    ratios, and the EnhanceInfo ratio of every type at a tuned level (the server's
    EnhanceServerInfo overrides the per-type ratio by level). Returns the cells that changed."""
    changed = 0
    for quest, (low, high) in overlay.get('quest_drops', {}).items():
        changed += db.execute('UPDATE QuestInfo SET DropItemCountMin=?, DropItemCountMax=? WHERE ID=? AND '
                              '(DropItemCountMin IS NOT ? OR DropItemCountMax IS NOT ?)', (low, high, quest, low, high)).rowcount
    for cls, (low, high) in overlay.get('enchant_ratios', {}).items():
        changed += db.execute('UPDATE EnchantInfo SET MinSuccessRatio=?, MaxSuccessRatio=? WHERE EnchantClass=? AND '
                              '(MinSuccessRatio IS NOT ? OR MaxSuccessRatio IS NOT ?)', (low, high, cls, low, high)).rowcount
    for target, ratio in overlay.get('enhance_ratios', {}).items():
        level = int(target.rsplit('#', 1)[1])
        changed += db.execute('UPDATE EnhanceInfo SET SuccessRatio=? WHERE EnhanceLevel=? AND SuccessRatio IS NOT ?',
                              (ratio, level, ratio)).rowcount
    return changed
