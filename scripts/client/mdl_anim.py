"""Animation tracks of a studio model (IDST v44, the 1.69's version): read them, pose the skeleton and
re-pin one bone to another for a whole sequence.

The weapon follows the player's `ValveBiped.Anim_Attachment_RH` bone (every weapon model has that one
bone), a child of the right hand whose track carries the weapon to the back in the town animations: in
town_idle_lower, town_run_lower and town_rest of the Delia and of the Hurk it stays fixed relative to
`Bip01_Spine2`. The Hurk's jump_town keeps it there; the Delia's (1.69) and the Miri's (NA) leave it in
the hand, so the weapon jumped into her hand and back (C4 MI7, MI8). town_jump_on_back rewrites that
track of jump_town so the bone keeps, in every frame, the Spine2 offset it has in the first frame of
town_idle_lower; the rest of the model is unchanged and the new track goes to the end of the file.
with_included_model adds an includemodel (the Miri's battle jump comes from the Delia's model,
stage_na_class.jump_donor).

  mdl_anim.py <in.mdl> --town-jump-on-back <out.mdl>
  mdl_anim.py <in.mdl> --relative <sequence> <carrier bone> <bone>   (prints the offset per frame)
"""
from __future__ import annotations
import argparse
import math
import struct
import sys
from pathlib import Path

BONE_STRIDE = 216
SEQ_STRIDE = 212
ANIM_STRIDE = 100
# studiohdr_t offsets (v44)
LENGTH_AT, BONES_AT, ANIMS_AT, SEQS_AT = 76, 0x9C, 180, 188
# mstudioanim_t flags
RAWPOS, RAWROT, ANIMPOS, ANIMROT, DELTA, RAWROT2 = 0x01, 0x02, 0x04, 0x08, 0x10, 0x20
# mstudioanimdesc_t flags that change how the data reads
ANIM_DELTA, ANIM_ALLZEROS = 0x04, 0x20

WEAPON_BONE = 'ValveBiped.Anim_Attachment_RH'
BACK_CARRIER = 'ValveBiped.Bip01_Spine2'
TOWN_JUMP, TOWN_REFERENCE = 'jump_town', 'town_idle_lower'
PINNED = 0.05  # units (and quaternion distance) under which a track already follows its carrier


class ModelError(ValueError):
    pass


def i32(data: bytes, at: int) -> int:
    return struct.unpack_from('<i', data, at)[0]


def cstr(data: bytes, at: int) -> str:
    end = data.find(b'\0', at)
    if not 0 <= at < len(data) or end < 0:
        raise ModelError(f'Bad string offset {at}.')
    return data[at:end].decode('latin-1')


def descriptor_stride(data: bytes, index: int, count: int, default: int) -> int:
    """Stride of a descriptor array, from each descriptor's baseptr (= -its offset)."""
    if count < 2:
        return default
    for stride in range(16, 1025, 4):
        if index + stride + 4 <= len(data) and i32(data, index + stride) == -(index + stride):
            return stride
    raise ModelError('No descriptor stride.')


# ---- rotations -----------------------------------------------------------------------------------
Quat = tuple[float, float, float, float]
Vec = tuple[float, float, float]


def q_mul(a: Quat, b: Quat) -> Quat:
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz)


def q_conj(q: Quat) -> Quat:
    return (-q[0], -q[1], -q[2], q[3])


def q_rotate(q: Quat, v: Vec) -> Vec:
    x, y, z, _ = q_mul(q_mul(q, (v[0], v[1], v[2], 0.0)), q_conj(q))
    return (x, y, z)


def q_distance(a: Quat, b: Quat) -> float:
    """0 for the same rotation (q and -q included)."""
    return 1.0 - abs(sum(x * y for x, y in zip(a, b)))


def euler_quat(e: Vec) -> Quat:
    """Source's AngleQuaternion(RadianEuler): x roll, y pitch, z yaw."""
    sr, cr = math.sin(e[0] * .5), math.cos(e[0] * .5)
    sp, cp = math.sin(e[1] * .5), math.cos(e[1] * .5)
    sy, cy = math.sin(e[2] * .5), math.cos(e[2] * .5)
    return (sr * cp * cy - cr * sp * sy, cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)


def quat_eulers(q: Quat) -> list[Vec]:
    """The two RadianEuler solutions of a rotation (euler_quat of either gives q or -q)."""
    x, y, z, w = q
    m20 = 2 * (x * z - w * y)
    m21, m22 = 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)
    m10, m00 = 2 * (x * y + w * z), 1 - 2 * (y * y + z * z)
    pitch = math.asin(max(-1.0, min(1.0, -m20)))
    if abs(m20) > 1 - 1e-9:  # gimbal lock: roll folds into yaw
        m01, m11 = 2 * (x * y - w * z), 1 - 2 * (x * x + z * z)
        first = (0.0, pitch, math.atan2(-m01, m11))
        return [first]
    first = (math.atan2(m21, m22), pitch, math.atan2(m10, m00))
    second = (first[0] + math.pi, math.pi - pitch, first[2] + math.pi)
    return [first, second]


def half(bits: int) -> float:
    return struct.unpack('<e', struct.pack('<H', bits))[0]


def quaternion48(raw: bytes) -> Quat:
    a, b, c = struct.unpack('<HHH', raw)
    x, y, z = (a - 32768) / 32768.0, (b - 32768) / 32768.0, ((c & 0x7FFF) - 16384) / 16384.0
    w = math.sqrt(max(0.0, 1 - x * x - y * y - z * z))
    return (x, y, z, -w if c & 0x8000 else w)


def quaternion64(raw: bytes) -> Quat:
    v = int.from_bytes(raw, 'little')
    x, y, z = (((v >> s & 0x1FFFFF) - 1048576) / 1048576.5 for s in (0, 21, 42))
    w = math.sqrt(max(0.0, 1 - x * x - y * y - z * z))
    return (x, y, z, -w if v >> 63 else w)


# ---- model ---------------------------------------------------------------------------------------
class Model:
    def __init__(self, data: bytes):
        if data[:4] != b'IDST' or i32(data, 4) != 44 or len(data) < 0x180:
            raise ModelError('Not a v44 studio model.')
        self.data = data
        count, index = struct.unpack_from('<ii', data, BONES_AT)
        self.bones = []
        for k in range(count):
            at = index + BONE_STRIDE * k
            self.bones.append({'name': cstr(data, at + i32(data, at)), 'parent': i32(data, at + 4),
                               'pos': struct.unpack_from('<3f', data, at + 32),
                               'quat': struct.unpack_from('<4f', data, at + 44),
                               'rot': struct.unpack_from('<3f', data, at + 60),
                               'posscale': struct.unpack_from('<3f', data, at + 72),
                               'rotscale': struct.unpack_from('<3f', data, at + 84)})
        count, index = struct.unpack_from('<ii', data, ANIMS_AT)
        stride = descriptor_stride(data, index, count, ANIM_STRIDE)
        self.anims = [index + stride * k for k in range(count)]
        count, index = struct.unpack_from('<ii', data, SEQS_AT)
        stride = descriptor_stride(data, index, count, SEQ_STRIDE)
        self.sequences = {}
        for k in range(count):
            at = index + stride * k
            self.sequences[cstr(data, at + i32(data, at + 4))] = at

    def bone(self, name: str) -> int:
        for k, b in enumerate(self.bones):
            if b['name'] == name:
                return k
        raise ModelError(f'No bone {name}.')

    def animation(self, sequence: str) -> int:
        """The animation descriptor of a one-animation sequence with plain (local) data in this file."""
        at = self.sequences.get(sequence)
        if at is None:
            raise ModelError(f'No sequence {sequence}.')
        if i32(self.data, at + 56) != 1:
            raise ModelError(f'{sequence} blends several animations.')
        anim = struct.unpack_from('<h', self.data, at + i32(self.data, at + 60))[0]
        if not 0 <= anim < len(self.anims):
            raise ModelError(f'{sequence} has no animation.')
        ad = self.anims[anim]
        flags, animblock, sectionframes = i32(self.data, ad + 12), i32(self.data, ad + 52), i32(self.data, ad + 84)
        if flags & (ANIM_DELTA | ANIM_ALLZEROS) or animblock or sectionframes:
            raise ModelError(f'{sequence}: unsupported animation layout (flags {flags:#x}, block {animblock}, sections {sectionframes}).')
        return ad

    def frames(self, sequence: str) -> int:
        return i32(self.data, self.animation(sequence) + 16)

    def entries(self, ad: int) -> list[tuple[int, int, int]]:
        """(bone, start, end) of each mstudioanim_t of an animation, in file order; end by its own data."""
        data, frames = self.data, i32(self.data, ad + 16)
        at = ad + i32(data, ad + 56)
        out = []
        while True:
            bone, flags, following = struct.unpack_from('<BBh', data, at)
            end = at + 4
            for kind, size in ((RAWROT2, 8), (RAWROT, 6), (ANIMROT, 6)):
                if flags & kind:
                    if kind == ANIMROT:
                        end = max([end + 6] + [value_end(data, end + o, frames) for o in channel_offsets(data, end)])
                    else:
                        end += size
                    break
            pos_at = end if not flags & ANIMROT else at + 4 + 6
            if flags & RAWPOS:
                end = max(end, pos_at + 6)
            elif flags & ANIMPOS:
                end = max([end, pos_at + 6] + [value_end(data, pos_at + o, frames) for o in channel_offsets(data, pos_at)])
            if following and end > at + following:
                raise ModelError(f'Animation data of bone {bone} overlaps the next one.')
            out.append((bone, at, end))
            if not following:
                return out
            at += following

    def local(self, ad: int, bi: int, frame: int, entries=None) -> tuple[Vec, Quat]:
        data, b = self.data, self.bones[bi]
        for bone, at, _ in entries if entries is not None else self.entries(ad):
            if bone != bi:
                continue
            flags = data[at + 1]
            q, pos, quat = at + 4, b['pos'], b['quat']
            if flags & RAWROT2:
                quat, q = quaternion64(data[q:q + 8]), q + 8
            elif flags & RAWROT:
                quat, q = quaternion48(data[q:q + 6]), q + 6
            elif flags & ANIMROT:
                quat = euler_quat(tuple(b['rot'][c] + v * b['rotscale'][c]
                                        for c, v in enumerate(channel_values(data, q, frame))))
                q += 6
            if flags & RAWPOS:
                pos = tuple(half(v) for v in struct.unpack_from('<3H', data, q))
            elif flags & ANIMPOS:
                pos = tuple(b['pos'][c] + v * b['posscale'][c] for c, v in enumerate(channel_values(data, q, frame)))
            if flags & DELTA:
                raise ModelError('Delta bone data.')
            return pos, quat
        return b['pos'], b['quat']

    def world(self, ad: int, bi: int, frame: int, entries=None) -> tuple[Vec, Quat]:
        entries = entries if entries is not None else self.entries(ad)
        pos, quat = self.local(ad, bi, frame, entries)
        parent = self.bones[bi]['parent']
        if parent < 0:
            return pos, quat
        ppos, pquat = self.world(ad, parent, frame, entries)
        r = q_rotate(pquat, pos)
        return (ppos[0] + r[0], ppos[1] + r[1], ppos[2] + r[2]), q_mul(pquat, quat)


def relative(a: tuple[Vec, Quat], b: tuple[Vec, Quat]) -> tuple[Vec, Quat]:
    """Transform b in the frame of a."""
    (ap, aq), (bp, bq) = a, b
    return q_rotate(q_conj(aq), (bp[0] - ap[0], bp[1] - ap[1], bp[2] - ap[2])), q_mul(q_conj(aq), bq)


def compose(a: tuple[Vec, Quat], b: tuple[Vec, Quat]) -> tuple[Vec, Quat]:
    (ap, aq), (bp, bq) = a, b
    r = q_rotate(aq, bp)
    return (ap[0] + r[0], ap[1] + r[1], ap[2] + r[2]), q_mul(aq, bq)


# ---- mstudioanimvalue_t ----------------------------------------------------------------------------
def channel_offsets(data: bytes, at: int) -> list[int]:
    return [o for o in struct.unpack_from('<3h', data, at) if o]


def channel_values(data: bytes, at: int, frame: int) -> list[int]:
    return [value(data, at + o, frame) if o else 0 for o in struct.unpack_from('<3h', data, at)]


def value(data: bytes, at: int, frame: int) -> int:
    """Run-length decoded value of a channel at a frame (Source's ExtractAnimValue)."""
    k = frame
    while True:
        valid, total = data[at], data[at + 1]
        if total == 0:
            raise ModelError('Empty animation run.')
        if k < total:
            return struct.unpack_from('<h', data, at + 2 * (min(k, valid - 1) + 1))[0]
        k -= total
        at += 2 * (valid + 1)


def value_end(data: bytes, at: int, frames: int) -> int:
    covered = 0
    while covered < frames:
        valid, total = data[at], data[at + 1]
        if total == 0:
            raise ModelError('Empty animation run.')
        covered += total
        at += 2 * (valid + 1)
    return at


def encode_values(values: list[int]) -> bytes:
    """Runs of at most 255 frames, every value stored."""
    out = bytearray()
    for start in range(0, len(values), 255):
        run = values[start:start + 255]
        out += bytes((len(run), len(run))) + struct.pack(f'<{len(run)}h', *run)
    return bytes(out)


# ---- re-pinning --------------------------------------------------------------------------------------
def quantize(target: Vec, base: Vec, scale: Vec, what: str) -> list[int]:
    out = []
    for c in range(3):
        if scale[c] == 0:
            if abs(target[c] - base[c]) > 1e-3:
                raise ModelError(f'{what}: axis {c} is not animatable in this model (scale 0).')
            out.append(0)
            continue
        v = round((target[c] - base[c]) / scale[c])
        if not -32767 <= v <= 32767:
            raise ModelError(f'{what}: axis {c} out of the bone\'s range.')
        out.append(v)
    return out


def euler_track(quats: list[Quat], base: Vec, scale: Vec) -> list[Vec]:
    """One RadianEuler per frame: the solution (and 2 pi turn per axis) least outside the bone's range,
    then closest to the previous frame, so the track stays continuous."""
    def outside(e: Vec) -> float:
        return sum(max(0.0, abs(e[c] - base[c]) - 32767 * scale[c]) for c in range(3))
    track: list[Vec] = []
    for quat in quats:
        candidates = [(x, y, z) for euler in quat_eulers(quat)
                      for x in (euler[0] + k * 2 * math.pi for k in (-1, 0, 1))
                      for y in (euler[1] + k * 2 * math.pi for k in (-1, 0, 1))
                      for z in (euler[2] + k * 2 * math.pi for k in (-1, 0, 1))]
        previous = track[-1] if track else base
        track.append(min(candidates, key=lambda e: (round(outside(e), 6), math.dist(e, previous))))
    return track


def rotation_scale_rewritten(data: bytes, model: Model, bi: int, scale: Vec) -> bytes:
    """data with bone bi's rotation scale replaced and its stored euler values in every animation
    rescaled in place, so the other animations pose it as before (to the new scale's precision)."""
    out = bytearray(data)
    old = model.bones[bi]['rotscale']
    at = struct.unpack_from('<i', data, BONES_AT + 4)[0] + BONE_STRIDE * bi
    struct.pack_into('<3f', out, at + 84, *scale)
    for ad in model.anims:
        flags, animblock, sectionframes = i32(data, ad + 12), i32(data, ad + 52), i32(data, ad + 84)
        if flags & ANIM_ALLZEROS:
            continue
        if animblock or sectionframes:
            raise ModelError('Cannot rescale a bone of a sectioned or external animation.')
        frames = i32(data, ad + 16)
        for bone, entry, _ in model.entries(ad):
            if bone != bi or not data[entry + 1] & ANIMROT or data[entry + 1] & (RAWROT | RAWROT2):
                continue
            for c, offset in enumerate(struct.unpack_from('<3h', data, entry + 4)):
                if not offset or scale[c] == old[c]:
                    continue
                run, covered = entry + 4 + offset, 0
                while covered < frames:
                    valid, total = data[run], data[run + 1]
                    for k in range(valid):
                        v = struct.unpack_from('<h', data, run + 2 * (k + 1))[0]
                        struct.pack_into('<h', out, run + 2 * (k + 1), round(v * old[c] / scale[c]))
                    covered += total
                    run += 2 * (valid + 1)
    return bytes(out)


def pin_bone(data: bytes, sequence: str, bone: str, carrier: str, reference: str) -> bytes:
    """data with bone's track in sequence rewritten so the bone keeps, relative to carrier, the offset it
    has in the first frame of reference. The new animation data goes to the end of the file; the
    descriptor points at it. When the rotation leaves the range of the bone's euler scale, the scale of
    that axis grows (rotation_scale_rewritten)."""
    model = Model(data)
    bi, ci = model.bone(bone), model.bone(carrier)
    parent = model.bones[bi]['parent']
    if parent < 0 or any(b['parent'] == bi for b in model.bones):
        raise ModelError(f'{bone} must have a parent and no children.')
    ref = model.animation(reference)
    offset = relative(model.world(ref, ci, 0), model.world(ref, bi, 0))
    ad = model.animation(sequence)
    entries = model.entries(ad)
    frames = i32(data, ad + 16)
    b = model.bones[bi]
    poses = []
    for f in range(frames):
        target = compose(model.world(ad, ci, f, entries), offset)
        poses.append(relative(model.world(ad, parent, f, entries), target))
    eulers = euler_track([q for _, q in poses], b['rot'], b['rotscale'])
    scale = tuple(max(b['rotscale'][c], max(abs(e[c] - b['rot'][c]) for e in eulers) / 32000) for c in range(3))
    if scale != b['rotscale']:
        data = rotation_scale_rewritten(data, model, bi, scale)
    positions = [quantize(pos, b['pos'], b['posscale'], f'{bone} frame {f}') for f, (pos, _) in enumerate(poses)]
    rotations = [[round((e[c] - b['rot'][c]) / scale[c]) for c in range(3)] for e in eulers]
    channels = [encode_values([r[c] for r in rotations]) for c in range(3)]
    channels += [encode_values([p[c] for p in positions]) for c in range(3)]
    head = 4 + 6 + 6
    offsets, at = [], head
    for chunk in channels:
        offsets.append(at)
        at += len(chunk)
    if at > 32767:
        raise ModelError('Track too long for one animation entry.')
    rot_ptr = [o - 4 for o in offsets[:3]]
    pos_ptr = [o - 10 for o in offsets[3:]]
    track = struct.pack('<BBh', bi, ANIMROT | ANIMPOS, 0) + struct.pack('<3h', *rot_ptr) + struct.pack('<3h', *pos_ptr)
    track += b''.join(channels)
    pieces = [(bone_, data[start:end]) for bone_, start, end in entries if bone_ != bi] + [(bi, track)]
    pieces.sort(key=lambda p: p[0])
    block = bytearray()
    for k, (_, piece) in enumerate(pieces):
        piece = bytearray(piece + (b'\0' if len(piece) % 2 else b''))
        struct.pack_into('<h', piece, 2, len(piece) if k + 1 < len(pieces) else 0)
        block += piece
    out = bytearray(data)
    out += b'\0' * (-len(out) % 4)
    start = len(out)
    out += block
    out += b'\0' * (-len(out) % 4)
    struct.pack_into('<i', out, ad + 56, start - ad)
    struct.pack_into('<i', out, LENGTH_AT, len(out))
    return bytes(out)


INCLUDES_AT = 336  # numincludemodels, includemodelindex (mstudiomodelgroup_t: label, name, relative)


def included_models(data: bytes) -> list[str]:
    count, index = struct.unpack_from('<ii', data, INCLUDES_AT)
    return [cstr(data, index + 8 * k + i32(data, index + 8 * k + 4)) for k in range(count)]


def with_included_model(data: bytes, name: str) -> bytes:
    """data with one more includemodel (game path, as the model's own includes spell it): the engine
    adds the sequences of the included model that this one lacks (the first name wins). The new list
    and the name go to the end of the file."""
    Model(data)
    if name.lower().replace('/', '\\') in (n.lower().replace('/', '\\') for n in included_models(data)):
        return data
    count, index = struct.unpack_from('<ii', data, INCLUDES_AT)
    groups = [(index + 8 * k + i32(data, index + 8 * k), index + 8 * k + i32(data, index + 8 * k + 4)) for k in range(count)]
    out = bytearray(data)
    out += b'\0' * (-len(out) % 4)
    at = len(out)
    empty = at + 8 * (count + 1)
    text = empty + 1
    out += b'\0' * (8 * (count + 1)) + b'\0' + name.encode('latin-1') + b'\0'
    out += b'\0' * (-len(out) % 4)
    for k, (label, path) in enumerate(groups + [(empty, text)]):
        struct.pack_into('<ii', out, at + 8 * k, label - (at + 8 * k), path - (at + 8 * k))
    struct.pack_into('<ii', out, INCLUDES_AT, count + 1, at)
    struct.pack_into('<i', out, LENGTH_AT, len(out))
    return bytes(out)


def max_drift(data: bytes, sequence: str, bone: str, carrier: str, reference: str) -> tuple[float, float]:
    """Largest distance and quaternion distance, over the frames of sequence, between bone's offset to
    carrier and the one in the first frame of reference."""
    model = Model(data)
    bi, ci = model.bone(bone), model.bone(carrier)
    ref = model.animation(reference)
    want = relative(model.world(ref, ci, 0), model.world(ref, bi, 0))
    ad = model.animation(sequence)
    entries = model.entries(ad)
    worst = (0.0, 0.0)
    for f in range(i32(data, ad + 16)):
        pos, quat = relative(model.world(ad, ci, f, entries), model.world(ad, bi, f, entries))
        worst = (max(worst[0], math.dist(pos, want[0])), max(worst[1], q_distance(quat, want[1])))
    return worst


def town_jump_on_back(data: bytes) -> bytes | None:
    """The town motion model with the weapon on the back in jump_town (module doc), or None when the model
    has no such pair of sequences or the weapon already stays there."""
    try:
        model = Model(data)
        if TOWN_JUMP not in model.sequences or TOWN_REFERENCE not in model.sequences:
            return None
        model.bone(WEAPON_BONE)
        model.bone(BACK_CARRIER)
    except ModelError:
        return None
    if max(max_drift(data, TOWN_JUMP, WEAPON_BONE, BACK_CARRIER, TOWN_REFERENCE)) < PINNED:
        return None
    return pin_bone(data, TOWN_JUMP, WEAPON_BONE, BACK_CARRIER, TOWN_REFERENCE)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('model', type=Path)
    parser.add_argument('--town-jump-on-back', type=Path, metavar='OUT')
    parser.add_argument('--relative', nargs=3, metavar=('SEQUENCE', 'CARRIER', 'BONE'))
    args = parser.parse_args()
    try:
        data = args.model.read_bytes()
        if args.relative:
            sequence, carrier, bone = args.relative
            model = Model(data)
            ad = model.animation(sequence)
            entries = model.entries(ad)
            for f in range(i32(data, ad + 16)):
                pos, quat = relative(model.world(ad, model.bone(carrier), f, entries), model.world(ad, model.bone(bone), f, entries))
                print(f, ' '.join(f'{x:.2f}' for x in pos), ' '.join(f'{x:.3f}' for x in quat))
        if args.town_jump_on_back:
            out = town_jump_on_back(data)
            if out is None:
                print('unchanged')
                return 0
            args.town_jump_on_back.write_bytes(out)
            print('drift', max_drift(out, TOWN_JUMP, WEAPON_BONE, BACK_CARRIER, TOWN_REFERENCE))
    except (OSError, ModelError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
