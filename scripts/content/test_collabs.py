"""Collab events (collabs.py): keys, packs, texts, outfits for every class and the events export."""
import json
import pathlib
import sqlite3
import tempfile
import unittest

import collabs
import export_item_economy as economy


class Keys(unittest.TestCase):
    def test_collab_and_region(self):
        self.assertEqual(collabs.collab('Event_sao_npc_jajp'), 'SAO')
        self.assertEqual(collabs.collab('Event_SAO_AllUserGoal_us'), 'SAO')
        self.assertEqual(collabs.collab('Event_monsterbingo_fairytail'), 'Fairy Tail')
        self.assertIsNone(collabs.collab('Event_season_2013'))
        self.assertEqual(collabs.adapted('Event_sao_npc_jajp'), 'JP')
        self.assertEqual(collabs.adapted('Event_SAO_lisbet_blacksmith_kokr'), 'KR')
        self.assertEqual(collabs.adapted('Event_SAO_AllUserGoal_cn_1'), 'CN')
        self.assertIsNone(collabs.adapted('Event_SAO_AllUserGoal_us'))

    def test_packs_need_their_keys(self):
        keys = {k for p in collabs.PACKS for k in p['enable'] + p['disable']}
        self.assertEqual(len(collabs.check_packs(keys)), 2)
        with self.assertRaises(ValueError):
            collabs.check_packs(keys - {'Event_sao_npc_jajp'})
        self.assertEqual(collabs.check_packs(set()), [])

    def test_sao_pack_has_one_goal_set(self):
        sao = next(p for p in collabs.PACKS if p['id'] == 'sao_eu')
        goals = [k for k in sao['enable'] if 'AllUserGoal_' in k and 'Cat_Shop' not in k and not k.endswith('_enus')]
        self.assertEqual(goals, ['Event_SAO_AllUserGoal_us'])
        self.assertIn('AllUserGoalEvent', sao['enable'])
        self.assertIn('Event_SAO_AllUserGoal_aus', sao['disable'])


class Texts(unittest.TestCase):
    def write(self, data):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        path = pathlib.Path(folder.name) / 'texts.json'
        path.write_text(json.dumps(data), encoding='utf-8')
        return path

    def test_versioned_texts_are_english_lines(self):
        texts = collabs.texts()
        self.assertGreater(len(texts), 300)
        self.assertIn('HEROES_NPC_FAIRYTAIL_NATSU', texts)
        self.assertIn('GAMEUI_HEROES_NPCDESCRIPTION_FAIRYTAIL_ELSA', texts)

    def test_rejects_quotes_cjk_and_repeats(self):
        for bad in ({'lab': {'HEROES_NPC_X': 'say "hi"'}}, {'lab': {'HEROES_NPC_X': '안녕'}},
                    {'lab': {'heroes_npc_x': 'lower'}}, {'official_na': {'HEROES_NPC_X': 'a'}, 'lab': {'HEROES_NPC_X': 'b'}}):
            with self.assertRaises(ValueError):
                collabs.texts(self.write(bad))

    def test_only_where_english_is_missing(self):
        mine = {'HEROES_NPC_A': 'A', 'HEROES_NPC_B': 'B', 'HEROES_NPC_C': 'C'}
        entries = {'heroes_npc_a': 'Official A', 'HEROES_NPC_B': '한글'}
        self.assertEqual(collabs.missing_in(entries, mine), {'HEROES_NPC_B': 'B', 'HEROES_NPC_C': 'C'})


COSTUME = b"""
"upper_body"
{
    "448" { "category" "tall" "label" "450" }
    "449" { "category" "giant" "label" "450" }
    "450" { "category" "male" "label" "450" }
    "500005" { "category" "female" "label" "500005" }
}
"""
MALE = 1 | 1 << 3 | 1 << 4 | 1 << 6 | 1 << 9
FEMALE = 1 << 1 | 1 << 2 | 1 << 5 | 1 << 7 | 1 << 8 | 1 << 10 | 1 << 11


class Outfits(unittest.TestCase):
    def db(self):
        db = sqlite3.connect(':memory:')
        db.execute('CREATE TABLE ItemClassInfo (ItemClass TEXT, Category TEXT, ClassRestriction INT)')
        db.execute('CREATE TABLE EquipItemInfo (ItemClass TEXT, CostumeType INT)')
        db.executemany('INSERT INTO ItemClassInfo VALUES (?,?,?)', [
            ('avatar_kirito_upper_jajp', 'AVATAR_TUNIC', 601), ('avatar_kirito_upper_jajp', 'AVATAR_TUNIC', 601),
            ('female_fairytail_lucy_upper', 'TUNIC', 1446), ('pattern_sao_kirito', 'PATTERN', 2047),
            ('necklace_cash_fairytail_ignil', 'NECKLACE', 2047), ('plain_tunic', 'TUNIC', 1),
            ('done_sao_upper', 'TUNIC', collabs.PORTED)])
        db.executemany('INSERT INTO EquipItemInfo VALUES (?,?)', [
            ('avatar_kirito_upper_jajp', 450), ('female_fairytail_lucy_upper', 500005), ('done_sao_upper', 450)])
        return db

    def test_body_categories_of_the_label(self):
        bodies = collabs.costume_bodies(COSTUME)
        self.assertEqual(bodies[('upper_body', '449')], {'male', 'giant', 'tall'})
        self.assertEqual(bodies[('upper_body', '500005')], {'female'})

    def test_masks_admit_the_bodies_with_a_model(self):
        # Kirito's 601 already is every male/giant/tall class: no female model, no change.
        self.assertEqual(601, MALE)
        self.assertEqual(collabs.unlock_masks(self.db(), COSTUME), {
            'female_fairytail_lucy_upper': 1446 | FEMALE,  # adds the Miri
            'necklace_cash_fairytail_ignil': collabs.PORTED})
        self.assertEqual(collabs.PORTED, 4095)
        # Without the costume file only the accessories open up.
        self.assertEqual(collabs.unlock_masks(self.db(), None), {'necklace_cash_fairytail_ignil': collabs.PORTED})

    def test_unlock_overrides_keep_the_panel_value(self):
        import content_additions
        overrides = {'female_fairytail_lucy_upper': {'set': {'ItemClassInfo': {'ClassRestriction': 2}}, 'text': {'name': 'L'}},
                     'plain_tunic': {'set': {'ItemClassInfo': {'Price': 5}}}}
        out = content_additions.with_collab_unlocks(self.db(), overrides, COSTUME)
        self.assertEqual(out['necklace_cash_fairytail_ignil'], {'set': {'ItemClassInfo': {'ClassRestriction': collabs.PORTED}}})
        self.assertNotIn('avatar_kirito_upper_jajp', out)
        self.assertEqual(out['female_fairytail_lucy_upper']['set']['ItemClassInfo'], {'ClassRestriction': 2})
        self.assertEqual(out['female_fairytail_lucy_upper']['text'], {'name': 'L'})
        self.assertEqual(out['plain_tunic'], overrides['plain_tunic'])
        self.assertNotIn('necklace_cash_fairytail_ignil', overrides)


class Stories(unittest.TestCase):
    def test_collab_gifts_lose_the_2014_expiry_only(self):
        import export_npc_scripts as npc
        gift = ['give_item', [['id', 'g'], ['arg1', 'coupon_bingo_pink_fairytail_2014[EXPIRE:2014/7/30/14/59/59]'],
                              ['arg2', '1']], '', [], '']
        talk = ['talk', [['arg1', 'x[EXPIRE:2014/1/1/0/0/0]']], '', [], '']
        tree = npc.without_collab_expiry(['Do', [], '', [gift, talk], ''])
        self.assertEqual(tree[3][0][1][1], ['arg1', 'coupon_bingo_pink_fairytail_2014'])
        self.assertEqual(tree[3][1], talk)
        later = ['give_item', [['arg1', 'a[EXPIRE:2099/1/1/0/0/0]']], '', [], '']
        self.assertEqual(npc.without_collab_expiry(later), later)


class Events(unittest.TestCase):
    """read_events lists collab keys of other regions and the keys only the NPC scripts read."""

    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        root = pathlib.Path(folder.name)
        (root / 'Default').mkdir()
        (root / 'NpcTalkInfo.xml').write_text(
            '<FileList><Default Feature="Event_sao_npc_jajp">Default\\Default_SAO_Kirito</Default>'
            '<Default>Default\\Default_Cat</Default></FileList>', encoding='utf-8')
        (root / 'Default/Default_SAO_Kirito.xml').write_text(
            '<DefaultPhase><Talk id="SAO_Kirito" location="t01_inn" command="greeting"><Do/></Talk></DefaultPhase>',
            encoding='utf-8')
        (root / 'Default/Default_Cat.xml').write_text(
            '<DefaultPhase><ButtonSet id="x"><Button caption="C" command="shop"><Condition><AND>'
            '<feature>Event_SAO_AllUserGoal_Cat_Shop_enus</feature></AND></Condition></Button></ButtonSet></DefaultPhase>',
            encoding='utf-8')
        self.root = root
        db = sqlite3.connect(':memory:')
        db.row_factory = sqlite3.Row
        db.execute('CREATE TABLE FeatureMatrix (Feature TEXT, "EN-EU-X-GM" TEXT)')
        db.executemany('INSERT INTO FeatureMatrix VALUES (?,?)', [
            ('Event_sao_npc_jajp', None), ('Event_SAO_AllUserGoal_Cat_Shop_enus', None),
            ('Event_kokr_newyear', None), ('Event_SAO_lisbet_blacksmith_kokr', None), ('Event_unused', None)])
        db.execute('CREATE TABLE ShopInfo (ShopID TEXT, ItemClass TEXT, Count INT, PriceItemClass TEXT, PriceCount INT, Feature TEXT)')
        db.executemany('INSERT INTO ShopInfo VALUES (?,?,?,?,?,?)', [
            ('KOKR_Shop', 'a', 1, 'gold', 1, 'Event_kokr_newyear'),
            ('SAO_Shop', 'b', 1, 'gold', 1, 'Event_SAO_lisbet_blacksmith_kokr')])
        self.db = db

    def test_collab_and_npc_keys(self):
        events = {e['key']: e for e in economy.read_events(self.db, {'region': 'EN-EU-X-GM'}, self.root)}
        self.assertEqual(sorted(events), ['Event_SAO_AllUserGoal_Cat_Shop_enus', 'Event_SAO_lisbet_blacksmith_kokr',
                                          'Event_sao_npc_jajp'])
        npc = events['Event_sao_npc_jajp']
        self.assertEqual(npc['uses'], {'NpcTalkInfo': 1})
        self.assertEqual((npc['collab'], npc['adapted']), ('SAO', 'JP'))
        self.assertEqual(npc['detail']['npcs'], [['SAO_Kirito', 't01_inn']])
        self.assertEqual(events['Event_SAO_AllUserGoal_Cat_Shop_enus']['uses'], {'NpcCondition': 1})
        self.assertNotIn('adapted', events['Event_SAO_AllUserGoal_Cat_Shop_enus'])
        self.assertEqual(events['Event_SAO_lisbet_blacksmith_kokr']['adapted'], 'KR')

    def test_without_scripts_only_tables(self):
        events = [e['key'] for e in economy.read_events(self.db, {'region': 'EN-EU-X-GM'})]
        self.assertEqual(events, ['Event_SAO_lisbet_blacksmith_kokr'])


if __name__ == '__main__':
    unittest.main()
