import math
import struct
import unittest

import mdl_anim as a

BONES = ('ValveBiped.Bip01', a.BACK_CARRIER, 'ValveBiped.Bip01_R_Hand', a.WEAPON_BONE)
PARENTS = (-1, 0, 0, 2)
BASE_POS = ((0.0, 0.0, 40.0), (20.0, 0.0, 0.0), (10.0, -15.0, 5.0), (2.0, 1.0, -1.0))
POS_SCALE = 1 / 256.0
ROT_SCALE = math.pi / 32767


def entry(bone: int, rot: list[list[int]], pos: list[list[int]]) -> bytes:
    """mstudioanim_t with ANIMROT and ANIMPOS, every frame of every channel stored."""
    channels = [a.encode_values(r) for r in rot] + [a.encode_values(p) for p in pos]
    offsets, at = [], 16
    for chunk in channels:
        offsets.append(at)
        at += len(chunk)
    return (struct.pack('<BBh', bone, a.ANIMROT | a.ANIMPOS, 0) + struct.pack('<3h', *[o - 4 for o in offsets[:3]])
            + struct.pack('<3h', *[o - 10 for o in offsets[3:]]) + b''.join(channels))


def anim_block(tracks: dict[int, tuple]) -> bytes:
    """tracks: bone -> (euler per frame, position per frame), in model units."""
    pieces = []
    for bone in sorted(tracks):
        eulers, positions = tracks[bone]
        rot = [[round(e[c] / ROT_SCALE) for e in eulers] for c in range(3)]
        pos = [[round((p[c] - BASE_POS[bone][c]) / POS_SCALE) for p in positions] for c in range(3)]
        pieces.append(bytearray(entry(bone, rot, pos)))
    for k, piece in enumerate(pieces[:-1]):
        struct.pack_into('<h', piece, 2, len(piece))
    return b''.join(pieces)


def synthetic(animations: dict[str, dict[int, tuple]], rot_scale=(ROT_SCALE,) * 3) -> bytes:
    """A v44 model with BONES and one single-animation sequence per entry of animations."""
    names = list(animations)
    bones_at = 0x180
    anims_at = bones_at + 216 * len(BONES)
    seqs_at = anims_at + 100 * len(names)
    data = bytearray(seqs_at + 212 * len(names))
    data[:4] = b'IDST'
    struct.pack_into('<i', data, 4, 44)
    strings = bytearray()

    def text(owner: int, field: int, value: str):
        struct.pack_into('<i', data, owner + field, len(data) + len(strings) - owner)
        strings.extend(value.encode() + b'\0')

    struct.pack_into('<ii', data, a.BONES_AT, len(BONES), bones_at)
    for k, name in enumerate(BONES):
        at = bones_at + 216 * k
        text(at, 0, name)
        struct.pack_into('<i', data, at + 4, PARENTS[k])
        struct.pack_into('<3f', data, at + 32, *BASE_POS[k])
        struct.pack_into('<4f', data, at + 44, 0, 0, 0, 1)
        struct.pack_into('<3f', data, at + 72, *(POS_SCALE,) * 3)
        struct.pack_into('<3f', data, at + 84, *(rot_scale if k == 3 else (ROT_SCALE,) * 3))
    struct.pack_into('<ii', data, a.ANIMS_AT, len(names), anims_at)
    struct.pack_into('<ii', data, a.SEQS_AT, len(names), seqs_at)
    blocks = []
    for k, name in enumerate(names):
        ad, sd = anims_at + 100 * k, seqs_at + 212 * k
        struct.pack_into('<i', data, ad, -ad)
        struct.pack_into('<i', data, sd, -sd)
        text(ad, 4, '@' + name)
        text(sd, 4, name)
        frames = len(next(iter(animations[name].values()))[0])
        struct.pack_into('<i', data, ad + 16, frames)
        struct.pack_into('<i', data, sd + 56, 1)
        blocks.append((ad, sd, k, anim_block(animations[name])))
    out = data + strings
    out += b'\0' * (-len(out) % 4)
    for ad, sd, k, block in blocks:
        struct.pack_into('<i', out, sd + 60, len(out) - sd)
        out += struct.pack('<h', k) + b'\0\0'
        struct.pack_into('<i', out, ad + 56, len(out) - ad)
        out += block + b'\0' * (-len(block) % 4)
    struct.pack_into('<i', out, a.LENGTH_AT, len(out))
    return bytes(out)


def still(frames: int, euler=(0.0, 0.0, 0.0), pos=None, bone=0):
    return ([euler] * frames, [pos or BASE_POS[bone]] * frames)


def town_model(rh_jump_in_hand=True, rot_scale=(ROT_SCALE,) * 3) -> bytes:
    back = ((0.2, -1.1, 0.3), (-18.0, 4.0, 22.0))  # weapon on the back in the reference, relative to the hand
    jump = {0: ([(0, 0, 0.1 * f) for f in range(6)], [(0, 0, 40 + 4 * f) for f in range(6)]),
            1: ([(0.05 * f, 0.1, 0) for f in range(6)], [BASE_POS[1]] * 6),
            2: ([(0.4 * f, -0.3 * f, 0.2) for f in range(6)], [(10.0, -15.0 + f, 5.0) for f in range(6)]),
            3: still(6, bone=3) if rh_jump_in_hand else ([back[0]] * 6, [back[1]] * 6)}
    reference = {0: still(2), 1: still(2, (0, 0.1, 0), bone=1), 2: still(2, (0.3, 0.2, 0.1), bone=2),
                 3: ([back[0]] * 2, [back[1]] * 2)}
    return synthetic({a.TOWN_REFERENCE: reference, a.TOWN_JUMP: jump}, rot_scale)


class MdlAnimTest(unittest.TestCase):
    def test_eulers_round_trip(self):
        for e in ((0.3, -1.2, 2.0), (2.9, 0.4, -3.0), (-1.0, 1.5, 0.2)):
            q = a.euler_quat(e)
            for solution in a.quat_eulers(q):
                self.assertLess(a.q_distance(a.euler_quat(solution), q), 1e-9)

    def test_town_jump_keeps_the_weapon_on_the_carrier(self):
        data = town_model()
        self.assertGreater(max(a.max_drift(data, a.TOWN_JUMP, a.WEAPON_BONE, a.BACK_CARRIER, a.TOWN_REFERENCE)), 1)
        out = a.town_jump_on_back(data)
        self.assertLess(max(a.max_drift(out, a.TOWN_JUMP, a.WEAPON_BONE, a.BACK_CARRIER, a.TOWN_REFERENCE)), a.PINNED)
        self.assertEqual(struct.unpack_from('<i', out, a.LENGTH_AT)[0], len(out))
        self.assertEqual(out[:len(data)][a.LENGTH_AT + 4:a.ANIMS_AT], data[a.LENGTH_AT + 4:a.ANIMS_AT])
        before, after = a.Model(data), a.Model(out)
        for sequence in (a.TOWN_REFERENCE, a.TOWN_JUMP):
            ad, bd = before.animation(sequence), after.animation(sequence)
            for f in range(before.frames(sequence)):
                for bone in range(len(BONES)):
                    if sequence == a.TOWN_JUMP and bone == 3:
                        continue
                    self.assertEqual(before.local(ad, bone, f), after.local(bd, bone, f))
        self.assertIsNone(a.town_jump_on_back(out))  # already on the back

    def test_a_jump_already_on_the_back_is_left_alone(self):
        data = synthetic({a.TOWN_REFERENCE: {3: still(2, bone=3)}, a.TOWN_JUMP: {3: still(4, bone=3)}})
        self.assertIsNone(a.town_jump_on_back(data))
        self.assertIsNone(a.town_jump_on_back(synthetic({'idle': {3: still(2, bone=3)}})))

    def test_a_rotation_beyond_the_bone_range_widens_its_scale(self):
        narrow = (ROT_SCALE, ROT_SCALE / 8, ROT_SCALE)
        data = town_model(rot_scale=narrow)
        out = a.town_jump_on_back(data)
        self.assertLess(max(a.max_drift(out, a.TOWN_JUMP, a.WEAPON_BONE, a.BACK_CARRIER, a.TOWN_REFERENCE)), a.PINNED)
        before, after = a.Model(data), a.Model(out)
        self.assertGreater(after.bones[3]['rotscale'][1], narrow[1])
        ad, bd = before.animation(a.TOWN_REFERENCE), after.animation(a.TOWN_REFERENCE)
        pos, quat = before.local(ad, 3, 0)
        pos2, quat2 = after.local(bd, 3, 0)
        self.assertEqual(pos, pos2)
        self.assertLess(a.q_distance(quat, quat2), 1e-6)

    def test_an_included_model_is_appended_once(self):
        data = town_model()
        self.assertEqual(a.included_models(data), [])
        out = a.with_included_model(data, 'models/player/_anim/female/delia/pc_delia_battle_motion_ex.mdl')
        self.assertEqual(a.included_models(out), ['models/player/_anim/female/delia/pc_delia_battle_motion_ex.mdl'])
        self.assertEqual(struct.unpack_from('<i', out, a.LENGTH_AT)[0], len(out))
        self.assertEqual(a.with_included_model(out, 'models\\player\\_anim\\female\\delia\\pc_delia_battle_motion_ex.mdl'), out)
        twice = a.with_included_model(out, 'models/player/_anim/other.mdl')
        self.assertEqual(len(a.included_models(twice)), 2)
        self.assertEqual(list(a.Model(twice).sequences), list(a.Model(data).sequences))

    def test_not_a_v44_model_is_refused(self):
        with self.assertRaises(a.ModelError):
            a.Model(b'IDST' + struct.pack('<i', 45) + bytes(0x200))


if __name__ == '__main__':
    unittest.main()
