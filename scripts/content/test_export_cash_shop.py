import contextlib, json, pathlib, sqlite3, tempfile, unittest

from export_cash_shop import read_package

SCHEMA = (
    'CREATE TABLE FeatureMatrix(Feature TEXT,"STUDY-A" TEXT);'
    'CREATE TABLE EquipItemInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE ItemClassInfo(ItemClass TEXT,Feature TEXT,MaxStack INTEGER,Storage INTEGER,Bind TEXT,TradeOnBind TEXT,Category TEXT,ClassRestriction INT);'
    'CREATE TABLE ItemClassExtraInfo(ItemClass TEXT,Feature TEXT);'
    'CREATE TABLE CustomizeItemInfo(Category varchar,[Order] INT,ItemClass varchar,Weight INT,IsCash INT,IsNew INT,Description varchar,CashShopType varchar,Feature varchar);'
    'CREATE TABLE CustomizePriceInfo(ItemClass varchar,Duration INT,Price INT,CashShopType varchar,Feature varchar);'
    'CREATE TABLE CustomizeCouponInfo(ItemClass varchar,Category varchar,Price INT,CashShopType varchar,Feature varchar);'
    'CREATE TABLE CustomizeColorInfo(Category varchar,[Order] INT,Color INT,Weight INT,IsCash bit,Description varchar,CashShopType varchar,Feature varchar);'
    'CREATE TABLE RandomItem(ItemClass TEXT);CREATE TABLE RandomItemV2(ItemClass TEXT);'
    'CREATE TABLE DailyItemProcessInfo(DailyProcessID INT,DateFrom TEXT,DateUntil TEXT,Period INT,CafeLevel INT,CafeType INT,LevelMin INT,LevelMax INT,QueryString TEXT,Message TEXT,Feature TEXT);'
    "INSERT INTO DailyItemProcessInfo VALUES(110,'2010-02-02 00:00:00.000','3000-12-31 00:00:00.001',1440,0,3,NULL,NULL,'giveitemuntil cash_revive 3','GameUI_Heroes_DailyItemProcess_GiveItemMessage','HomeCafeBonus&&enEU');"
    'CREATE TABLE StatusEffectGroupInfo(Type TEXT,Level INT,Abslevel INT,IDTag TEXT,Duration INT,CombatCount INT,ServerSide INT,RealTimePeriod INT,ServerEffect1 TEXT,ServerEffect1_Arg1 TEXT,ServerEffect1_Arg2 TEXT,ServerEffect1_Arg3 TEXT,ServerEffect2 TEXT,ServerEffect2_Arg1 TEXT,ServerEffect2_Arg2 TEXT,ServerEffect2_Arg3 TEXT,ServerEffect3 TEXT,ServerEffect3_Arg1 TEXT,ServerEffect3_Arg2 TEXT,ServerEffect3_Arg3 TEXT,ServerEffect TEXT);'
    'CREATE TABLE Cashshop_Tircoin_notsell_list(CashShopType varchar,ItemClass varchar,Amount INT,Duration INT,Price INT,Feature varchar);'
)


class CashShopExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        self.db = root / 'heroesContents.db3'
        self.config = root / 'ServiceCore.dll.config'
        self.prices = root / 'prices.json'
        self.text = root / 'heroes_text_english_eu.txt'
        self.text.write_bytes(('\ufeff"lang"\r\n{\r\n\t"Tokens"\r\n\t{\r\n'
                               '\t\t"HEROES_ITEM_NAME_HAIR_A"\t"Short Hair"\r\n'
                               '\t\t"HEROES_ITEM_DESC_HAIR_A"\t"A hair.\\nPermanent."\r\n'
                               '\t\t"HEROES_ITEM_NAME_POTION"\t"\ud3ec\uc158"\r\n\t}\r\n}\r\n').encode('utf-16-le'))
        self.prices.write_text(json.dumps({'default': 500, 'categories': {'HAIR': 800},
                                           'value_products': {'potion': [7, 30]}}), encoding='utf-8')
        self.config.write_text('<configuration><setting name="GameCode"><value>42</value></setting><setting name="heroesContentsDB3"><value>heroesContents.db3</value></setting></configuration>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.executescript(SCHEMA)
            db.executemany('INSERT INTO FeatureMatrix VALUES (?,?)', [('GameCode', '42'), ('CurrentVer', '3.09.00'), ('OverrideSetting', ''), ('StoryTabInInventory', ''), ('HomeCafeBonus_StatusEffect', 'homecafe_bonus,account_homecafe_bonus')])
            db.executemany('INSERT INTO StatusEffectGroupInfo VALUES (' + ','.join('?' * 21) + ')', [
                ('homecafe_bonus', 1, 1, 'homecafe_bonus', 2592000, -1, 1, 1, 'stat_adder', 'ATK', '19', '0', 'stat_adder', 'MATK', '18', '0', None, None, None, None, '#HomeCafeBonusEX'),
                ('other', 1, 1, None, 10, -1, 1, 0, 'stat_adder', 'STR;DEX', '1;2', '0', None, None, None, None, None, None, None, None, None)])
            db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?,?,?,?,?,?)', [('hair_a', None, 1, 2, None, None, 'HAIR', 2), ('potion', None, 99, 1, None, None, None, 2047)])
            db.execute('INSERT INTO CustomizeItemInfo VALUES (?,?,?,?,?,?,?,?,?)', ('HAIR', 1, 'hair_a', 0, 1, 0, None, 'en-EU', 'STUDY-A'))
            db.executemany('INSERT INTO CustomizePriceInfo VALUES (?,?,?,?,?)', [('hair_a', -1, 139, 'en-EU', ''), ('hair_a', 30, 69, 'en-EU', '')])
            db.commit()

    def test_categories_follow_the_catalog_rows(self):
        p = read_package(self.db, self.config, self.prices, self.text)
        # The EU text of potion is still Korean: no english name.
        self.assertEqual(p['names'], [{'name': 'Short Hair', 'description': 'A hair.\nPermanent.'}, None])
        self.assertEqual(p['categories'], ['HAIR', None])
        self.assertEqual((p['root_category'], p['default_price'], p['prices']), (105, 500, [{'category': 'HAIR', 'price': 800}]))
        self.assertEqual([(r['duration'], r['price']) for r in p['customize_prices']], [(-1, 139), (30, 69)])
        self.assertEqual(p['customize_items'][0]['category'], 'HAIR')
        self.assertIn('STUDY-A', [f['key'] for f in p['features']])
        self.assertEqual(p['class_restrictions'], [2, 2047])
        self.assertEqual(p['value_products'], [{'item': 'potion', 'days': [7, 30]}])
        self.assertEqual(p['without_contents'], [])
        self.assertEqual(p['daily_items'], [{'id': 110, 'start_ticks': 634006656000000000, 'end_ticks': 946707264000010000, 'period_minutes': 1440, 'cafe_level': 0, 'cafe_type': 3,
                                             'level_min': None, 'level_max': None, 'query': 'giveitemuntil cash_revive 3',
                                             'message': 'GameUI_Heroes_DailyItemProcess_GiveItemMessage', 'feature': 'HomeCafeBonus&&enEU'}])
        self.assertIn('enEU', [f['key'] for f in p['features']])
        self.assertEqual(p['status_effects'], [{'type': 'homecafe_bonus', 'level': 1, 'abs_level': 1, 'id_tag': 'homecafe_bonus', 'duration': 2592000,
                                                 'combat_count': -1, 'kind': 'UseExpireTime', 'stat_adder': [['ATK', 19], ['MATK', 18]]},
                                                {'type': 'other', 'level': 1, 'abs_level': 1, 'id_tag': None, 'duration': 10,
                                                 'combat_count': -1, 'kind': 'UseRemainTime', 'stat_adder': [['DEX', 2], ['STR', 1]]}])

    def test_price_table_is_bounded(self):
        self.prices.write_text(json.dumps({'default': 0, 'categories': {}}), encoding='utf-8')
        with self.assertRaises(ValueError):
            read_package(self.db, self.config, self.prices, self.text)


if __name__ == '__main__':
    unittest.main()
