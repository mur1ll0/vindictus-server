import struct
import unittest

import bsp_entities as b

DELIA = ('{\n"origin" "-32 235.912 92"\n"target_csf_name" "csf/createcharacter_delia.csf"\n"variation" "npc_delia"\n'
         '"targetname" "npc_delia"\n"classname" "npc_character_creation_actor"\n}\n')


def bsp(entities: str) -> bytes:
    """A version 20 header, one other lump (1) after the entities, and the entity lump (0)."""
    text = ('{\n"classname" "worldspawn"\n}\n' + entities).encode('latin-1') + b'\0'
    header = bytearray(8 + 16 * 64 + 4)
    header[:8] = b'VBSP' + struct.pack('<i', 20)
    other = b'PLANES!!'
    struct.pack_into('<ii', header, 8, len(header), len(text))
    struct.pack_into('<ii', header, 8 + 16, len(header) + len(text), len(other))
    return bytes(header) + text + other


class BspEntitiesTest(unittest.TestCase):
    def test_a_class_gets_a_copy_of_the_donor_actor_at_the_end(self):
        src = bsp(DELIA)
        out = b.with_creation_actors(src, 'delia', ('miri',))
        self.assertEqual(out[:len(src)][24:], src[24:])  # every other lump and its offset stay
        self.assertEqual(struct.unpack_from('<i', out, 8)[0], len(src) + (-len(src) % 4))
        self.assertEqual(out[-1:], b'\0')
        actors = {b.value(x, 'targetname'): x for x in b.blocks(b.entity_text(out))}
        miri = actors['npc_miri']
        self.assertEqual(b.value(miri, 'target_csf_name'), 'csf/createcharacter_miri.csf')
        self.assertEqual(b.value(miri, 'variation'), 'npc_delia')  # the donor's actor settings, no model
        self.assertEqual(b.value(miri, 'classname'), b.ACTOR)
        self.assertEqual(b.value(actors['npc_delia'], 'target_csf_name'), 'csf/createcharacter_delia.csf')
        self.assertIsNone(b.with_creation_actors(out, 'delia', ('miri',)))

    def test_a_map_without_the_donor_is_refused(self):
        with self.assertRaises(b.BspError):
            b.with_creation_actors(bsp(''), 'delia', ('miri',))
        with self.assertRaises(b.BspError):
            b.with_creation_actors(b'IBSP' + bytes(1100), 'delia', ('miri',))


if __name__ == '__main__':
    unittest.main()
