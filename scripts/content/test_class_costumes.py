import unittest

import class_costumes

BASE = b'"inner_armor"\r\n{\r\n\t"16"\r\n\t{\r\n\t\t"normal"\t"models/a.mdl"\r\n\t}\r\n}\r\n'
SOURCE = (b'"inner_armor"\r\n{\r\n\t"9000"\r\n\t{\r\n\t\t"label"\t"9000"\r\n\t\t"normal"\t"models/player/miri/a.mdl"\r\n\t}\r\n'
          b'\t"9100"\r\n\t{\r\n\t\t"label"\t"9100"\r\n\t\t"normal"\t"models/player/miri/b.mdl"\r\n\t}\r\n'
          b'\t"9101"\r\n\t{\r\n\t\t"label"\t"9101"\r\n\t\t"normal"\t"models/player/bel/c.mdl"\r\n\t}\r\n}\r\n')


class CostumeRemapTest(unittest.TestCase):
    def test_only_class_entries_used_by_lab_items_move_to_free_numbers(self):
        taken = class_costumes.lab_taken([{'costumes': [{'section': 'inner_armor', 'key': '9100'},
                                                        {'section': 'inner_armor', 'key': '9101'},
                                                        {'section': 'weapon_staff', 'key': '7'}]}])
        remap = class_costumes.costume_remap(SOURCE, BASE, taken, ('miri',))
        # 9000 is the class's own; 9101 is Bel's (not the class); 9100 goes to the first free number.
        self.assertEqual(remap, {('inner_armor', '9100'): '9001'})
        out = class_costumes.rename(SOURCE, remap)
        self.assertIn(b'"9001"\r\n\t{\r\n\t\t"label"\t"9001"\r\n\t\t"normal"\t"models/player/miri/b.mdl"', out)
        self.assertNotIn(b'"9100"', out)
        self.assertEqual(class_costumes.rename(SOURCE, {}), SOURCE)

    def test_no_lab_items_no_remap(self):
        self.assertEqual(class_costumes.costume_remap(SOURCE, BASE, {}, ('miri',)), {})


WEAPON = (b'WeaponData\r\n{\r\n\t"playermodel"\t"models/weapon/draker/draker_test.mdl"\r\n'
          b'\t"SubTypeInfoOverride"\r\n\t{\r\n'
          b'\t\t"0"\r\n\t\t{\r\n\t\t\t"viewmodel"\t"models/weapon/draker/draker_test.mdl"\r\n'
          b'\t\t\t"playermodel"\t"models/weapon/draker/draker_test.mdl"\r\n\t\t}\r\n'
          b'\t\t"1" // newbie\r\n\t\t{\r\n\t\t\t"viewmodel"\t"models/weapon/draker/draker_newbie.mdl"\r\n'
          b'\t\t\t"playermodel"\t"models/weapon/draker/draker_newbie.mdl"\r\n\t\t}\r\n\t}\r\n}\r\n')


class WeaponSubtypeTest(unittest.TestCase):
    def test_class_subtypes_move_into_the_lab_entity_script(self):
        self.assertEqual(class_costumes.weapon_costumes(WEAPON, 'draker'), [
            {'section': 'weapon_greatsword', 'key': '10100', 'normal': 'models/weapon/draker/draker_test.mdl'},
            {'section': 'weapon_greatsword', 'key': '10101', 'normal': 'models/weapon/draker/draker_newbie.mdl'}])
        self.assertEqual(class_costumes.weapon_subtype('draker', 1), 10101)

    def test_subtypes_take_the_class_weapon_trails(self):
        trails = (b'\t"TrailData"\r\n\t{\r\n\t\t"0"\r\n\t\t{\r\n\t\t\t"material"\t"Effects/miri_trail_dragon_01"\r\n'
                  b'\t\t}\r\n\t}\r\n')
        own = (b'\t\t\t"TrailData"\r\n\t\t\t{\r\n\t\t\t\t// "material"\t"Effects/old"\r\n'
               b'\t\t\t\t"0" { "material"\t"Effects/miri_smash_night" }\r\n'
               b'\t\t\t\t"1" { "material"\t"Effects/miri_trail_dragon_01_night" }\r\n\t\t\t}\r\n')
        knight = (b'\t\t"1033"\r\n\t\t{\r\n\t\t\t"playermodel"\t"models/weapon/draker/draker_dragonknight.mdl"\r\n'
                  + own + b'\t\t}\r\n')
        script = WEAPON.replace(b'\t"SubTypeInfoOverride"', trails + b'\t"SubTypeInfoOverride"', 1)
        script = script.replace(b'\t}\r\n}\r\n', knight + b'\t}\r\n}\r\n', 1)
        test, newbie, dragon = class_costumes.weapon_costumes(script, 'draker')
        self.assertEqual(dragon['key'], '11133')
        for costume in (test, newbie):
            # the weapon's own trails (not the greatsword's), then the Dragon Knight's at 5 + n
            block = class_costumes.kv_merge.parse(costume['trails'].encode()).children[0]
            self.assertEqual([c.key for c in block.children], ['0', '5', '6'])
            self.assertIn('miri_trail_dragon_01"', costume['trails'])
            self.assertIn('"5" { "material"\t"Effects/miri_smash_night" }', costume['trails'])
            self.assertNotIn('Effects/old', costume['trails'])
        # a sub-type's own trails win over the weapon's
        self.assertEqual([c.key for c in class_costumes.kv_merge.parse(dragon['trails'].encode()).children[0].children],
                         ['0', '1', '5', '6'])
        self.assertNotIn('trails', class_costumes.weapon_costumes(WEAPON, 'draker')[0])

    def test_a_weapon_without_lab_entity_keeps_its_numbers(self):
        self.assertEqual(class_costumes.weapon_costumes(WEAPON, 'blaster'), [])
        self.assertEqual(class_costumes.weapon_subtype('blaster', 1), 1)


if __name__ == '__main__':
    unittest.main()
