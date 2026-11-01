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
        # a one-piece reference to a moved entry follows it
        onepiece = SOURCE.replace(b'"models/player/miri/a.mdl"', b'"models/player/miri/a.mdl"\r\n\t\t"onepiece"\t"9100;3"')
        self.assertIn(b'"onepiece"\t"9001;3"', class_costumes.rename(onepiece, remap))

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
        def trail(n, points, material):
            return (b'\t\t"%d"\r\n\t\t{\r\n\t\t\t"material"\t"Effects/%s"\r\n\t\t\t"attack_points"\t"%s"\r\n\t\t}\r\n'
                    % (n, material, points))
        trails = (b'\t"TrailData"\r\n\t{\r\n' + b''.join(trail(n, b'0 1 2', b'fire_%d' % n) for n in range(4))
                  + trail(4, b'0 1 2 3 4', b'chain') + b'\t}\r\n')
        own = (b'\t\t\t"TrailData"\r\n\t\t\t{\r\n\t\t\t\t// "material"\t"Effects/old"\r\n'
               b'\t\t\t\t"0" { "material"\t"Effects/miri_smash_night" "attack_points" "0 1 2" }\r\n\t\t\t}\r\n')
        knight = (b'\t\t"1033"\r\n\t\t{\r\n\t\t\t"playermodel"\t"models/weapon/draker/draker_dragonknight.mdl"\r\n'
                  + own + b'\t\t}\r\n')
        script = WEAPON.replace(b'\t"SubTypeInfoOverride"', trails + b'\t"SubTypeInfoOverride"', 1)
        script = script.replace(b'\t}\r\n}\r\n', knight + b'\t}\r\n}\r\n', 1)
        test, newbie, dragon = class_costumes.weapon_costumes(script, 'draker')
        self.assertEqual(dragon['key'], '11133')
        for costume in (test, newbie):
            # the weapon's own trails (not the greatsword's), as many as the 1.69's 12 handles hold:
            # 4 trails of points 0-2; the fifth (points 0-4) would make 5 * 5 (C4 MI24 crash)
            block = class_costumes.kv_merge.parse(costume['trails'].encode()).children[0]
            self.assertEqual([c.key for c in block.children], ['0', '1', '2', '3'])
            self.assertIn('Effects/fire_0"', costume['trails'])
            self.assertNotIn('chain', costume['trails'])
        # a sub-type's own trails win over the weapon's, comments left out
        self.assertIn('miri_smash_night', dragon['trails'])
        self.assertNotIn('Effects/old', dragon['trails'])
        self.assertNotIn('trails', class_costumes.weapon_costumes(WEAPON, 'draker')[0])

    def test_trails_stop_where_the_169_handles_end(self):
        def block(*points):
            text = b'"TrailData"\r\n{\r\n' + b''.join(b'"%d" { "attack_points" "%s" }\r\n' % (n, p) for n, p in enumerate(points)) + b'}'
            return class_costumes.kv_merge.parse(text).children[0]
        keys = lambda trails: [t.key for t in class_costumes.fitting_trails(trails)]
        self.assertEqual(keys(block(b'0 1 2', b'0 1 2', b'0 1 2', b'0 1 2', b'0 1 2 3 4')), ['0', '1', '2', '3'])
        self.assertEqual(keys(block(*[b'0 1'] * 7)), ['0', '1', '2', '3', '4', '5'])
        self.assertEqual(keys(block(b'0 1 2 3 4 5', b'0')), ['0', '1'])
        self.assertEqual(keys(block(b'0 1 2 3 4 5 6 7 8 9 10 11 12')), [])

    def test_a_weapon_without_lab_entity_keeps_its_numbers(self):
        self.assertEqual(class_costumes.weapon_costumes(WEAPON, 'blaster'), [])
        self.assertEqual(class_costumes.weapon_subtype('blaster', 1), 1)


if __name__ == '__main__':
    unittest.main()
