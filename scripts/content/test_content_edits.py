import json, pathlib, tempfile, unittest

import content_edits

ROWS = [
    {'ShopID': 'FERGHUS_Shop', 'Order': 1, 'ItemClass': 'longsword', 'Count': 1, 'PriceItemClass': 'gold',
     'PriceCount': 100, 'AppearQuestID': None, 'CharacterBind': False, 'ID': None, 'RestrictionResetTime': None,
     'RestrictionPeriodDay': 0, 'RestrictionCount': 0, 'Feature': None},
    {'ShopID': 'FERGHUS_Shop', 'Order': 2, 'ItemClass': 'shield', 'Count': 1, 'PriceItemClass': 'gold',
     'PriceCount': 200, 'AppearQuestID': 'q', 'CharacterBind': True, 'ID': 7, 'RestrictionResetTime': None,
     'RestrictionPeriodDay': 0, 'RestrictionCount': 0, 'Feature': 'f'},
]


def overlay(**kw):
    return {**content_edits.EMPTY, **kw}


class ContentEditsTests(unittest.TestCase):
    def test_shop_rows_change_remove_and_add_keep_other_fields(self):
        out = content_edits.apply_shop_rows(ROWS, overlay(shop_rows=[
            {'ShopID': 'FERGHUS_Shop', 'Order': 2, 'ItemClass': 'shield', 'Count': 3, 'PriceItemClass': 'gold', 'PriceCount': 250},
            {'ShopID': 'FERGHUS_Shop', 'Order': 1, 'Remove': True},
            {'ShopID': 'FERGHUS_Shop', 'Order': 9, 'ItemClass': 'enhance_elixir', 'Count': 1, 'PriceItemClass': 'gold', 'PriceCount': 4000},
        ]))
        self.assertEqual([r['Order'] for r in out], [2, 9])
        self.assertEqual((out[0]['Count'], out[0]['PriceCount'], out[0]['AppearQuestID'], out[0]['Feature']), (3, 250, 'q', 'f'))
        self.assertEqual((out[1]['ItemClass'], out[1]['CharacterBind'], out[1]['RestrictionCount']), ('enhance_elixir', False, 0))
        self.assertEqual(content_edits.apply_shop_rows(ROWS, content_edits.EMPTY), ROWS)

    def test_prices_and_bounded_file(self):
        self.assertEqual(content_edits.apply_prices(500, {'HAIR': 800}, overlay(cash_prices={'HAIR': 900, 'PACKAGE': 3000}, cash_default=600)),
                         (600, {'HAIR': 900, 'PACKAGE': 3000}))
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / 'content-edits.json'
            self.assertEqual(content_edits.load(path), content_edits.EMPTY)
            path.write_text(json.dumps(overlay(cash_prices={'HAIR': 0})), encoding='utf-8')
            with self.assertRaises(ValueError):
                content_edits.load(path)


if __name__ == '__main__':
    unittest.main()
