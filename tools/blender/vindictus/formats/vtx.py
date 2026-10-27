"""Optimized model (.vtx, version 7): per LOD and mesh, strip groups of hardware-skinned triangle lists.
A strip has a palette of at most `max_bones_per_strip` bones (bone state changes: hardware slot ->
model bone) and its vertices name palette slots. Layout from the Source SDK optimize.h (packed)."""
from __future__ import annotations
import struct
from dataclasses import dataclass

HEADER = struct.Struct('<iiHHiIiiii')
BODYPART = struct.Struct('<ii')
MODEL = struct.Struct('<ii')
LOD = struct.Struct('<iif')
MESH = struct.Struct('<iiB')
STRIPGROUP = struct.Struct('<iiiiiiB')
STRIP = struct.Struct('<iiiihBii')
VERTEX = struct.Struct('<3BBH3b')
BONESTATE = struct.Struct('<ii')
STRIPGROUP_IS_HWSKINNED = 0x02
STRIP_IS_TRILIST = 0x01


class VtxError(ValueError):
    pass


@dataclass
class Vtx:
    checksum: int
    max_bones_per_strip: int
    num_lods: int
    # [bodypart][model][mesh] -> triangles of mesh-local vertex ids, LOD 0
    triangles: list[list[list[list[tuple]]]]


def read(data: bytes) -> Vtx:
    version, _, max_strip, _, _, checksum, num_lods, _, num_bodyparts, bodypart_offset = HEADER.unpack_from(data, 0)
    if version != 7:
        raise VtxError(f'Unsupported optimized model version {version}.')
    parts = []
    for b in range(num_bodyparts):
        bp = bodypart_offset + b * BODYPART.size
        num_models, model_offset = BODYPART.unpack_from(data, bp)
        models = []
        for m in range(num_models):
            mo = bp + model_offset + m * MODEL.size
            _, lod_offset = MODEL.unpack_from(data, mo)
            lod = mo + lod_offset
            num_meshes, mesh_offset, _ = LOD.unpack_from(data, lod)
            meshes = []
            for k in range(num_meshes):
                me = lod + mesh_offset + k * MESH.size
                num_groups, group_offset, _ = MESH.unpack_from(data, me)
                triangles = []
                for g in range(num_groups):
                    sg = me + group_offset + g * STRIPGROUP.size
                    num_verts, vert_offset, num_indices, index_offset, num_strips, strip_offset, _ = STRIPGROUP.unpack_from(data, sg)
                    original = [VERTEX.unpack_from(data, sg + vert_offset + i * VERTEX.size)[4] for i in range(num_verts)]
                    indices = struct.unpack_from(f'<{num_indices}H', data, sg + index_offset)
                    for s in range(num_strips):
                        count, first, _, _, _, flags, _, _ = STRIP.unpack_from(data, sg + strip_offset + s * STRIP.size)
                        if not flags & STRIP_IS_TRILIST:
                            raise VtxError('Triangle strips are not supported, only lists.')
                        for i in range(first, first + count, 3):
                            triangles.append((original[indices[i]], original[indices[i + 1]], original[indices[i + 2]]))
                meshes.append(triangles)
            models.append(meshes)
        parts.append(models)
    return Vtx(checksum, max_strip, num_lods, parts)


def strips_of(triangles: list[tuple], vertex_bones: list[tuple], max_bones: int) -> list[tuple]:
    """Greedy split of a mesh's triangles into strips whose bone palettes fit `max_bones`.
    Returns (palette, triangles) pairs; vertex_bones[i] are the model bones of mesh vertex i."""
    strips, palette, current = [], [], []
    used = set()
    for tri in triangles:
        bones = {b for v in tri for b in vertex_bones[v]}
        if len(used | bones) > max_bones:
            strips.append((palette, current))
            palette, current, used = [], [], set()
        for b in sorted(bones - used):
            palette.append(b)
        used |= bones
        current.append(tri)
    if current:
        strips.append((palette, current))
    return strips


def write(checksum: int, num_lods: int, max_bones_per_strip: int, meshes_per_model: list[list[list[tuple]]],
          vertex_bones: list[list[list[tuple]]], vertex_weights_count: list[list[list[int]]]) -> bytes:
    """One body part, the given models; each LOD repeats LOD 0. meshes_per_model[model][mesh] are triangles of
    mesh-local vertex ids; vertex_bones[model][mesh][v] the model bones of that vertex (in VVD weight order);
    vertex_weights_count[model][mesh][v] how many of them are used."""
    out = bytearray(HEADER.size)

    def mesh_block(triangles, bones, counts):
        strips = strips_of(triangles, bones, max_bones_per_strip)
        verts, indices, strip_rows, states = bytearray(), [], [], bytearray()
        num_verts = 0
        for palette, tris in strips:
            slot = {b: i for i, b in enumerate(palette)}
            local = {}
            first_vert, first_index = num_verts, len(indices)
            for tri in tris:
                for v in tri:
                    if v not in local:
                        local[v] = num_verts
                        ids = [slot[b] for b in bones[v]] + [0] * (3 - len(bones[v]))
                        verts += VERTEX.pack(0, 1, 2, counts[v], v, *ids)
                        num_verts += 1
                    indices.append(local[v])
            strip_rows.append((len(indices) - first_index, first_index, num_verts - first_vert, first_vert,
                               3, STRIP_IS_TRILIST, len(palette), len(states)))
            for i, b in enumerate(palette):
                states += BONESTATE.pack(i, b)
        return num_verts, verts, indices, strip_rows, states

    # Layout: header, body part, models, then per model its LOD headers, meshes, strip groups and data.
    bodypart_at = len(out)
    out += BODYPART.pack(len(meshes_per_model), BODYPART.size)
    models_at = len(out)
    out += bytes(MODEL.size * len(meshes_per_model))
    for m, meshes in enumerate(meshes_per_model):
        lods_at = len(out)
        struct.pack_into('<ii', out, models_at + m * MODEL.size, num_lods, lods_at - (models_at + m * MODEL.size))
        out += bytes(LOD.size * num_lods)
        blocks = [mesh_block(meshes[k], vertex_bones[m][k], vertex_weights_count[m][k]) for k in range(len(meshes))]
        for lod in range(num_lods):
            lod_at = lods_at + lod * LOD.size
            meshes_at = len(out)
            LOD.pack_into(out, lod_at, len(meshes), meshes_at - lod_at, 0.0)
            out += bytes(MESH.size * len(meshes))
            for k, (num_verts, verts, indices, strip_rows, states) in enumerate(blocks):
                mesh_at = meshes_at + k * MESH.size
                group_at = len(out)
                MESH.pack_into(out, mesh_at, 1, group_at - mesh_at, 0)
                out += bytes(STRIPGROUP.size)
                vert_off = len(out) - group_at
                out += verts
                index_off = len(out) - group_at
                out += struct.pack(f'<{len(indices)}H', *indices)
                strip_off = len(out) - group_at
                states_at = strip_off + STRIP.size * len(strip_rows)
                for row in strip_rows:
                    out += STRIP.pack(*row[:7], states_at + row[7] - (len(out) - group_at))
                out += states
                STRIPGROUP.pack_into(out, group_at, num_verts, vert_off, len(indices), index_off, len(strip_rows), strip_off,
                                     STRIPGROUP_IS_HWSKINNED)
    replacements_at = len(out)
    out += BODYPART.pack(0, 0) * num_lods
    HEADER.pack_into(out, 0, 7, 24, max_bones_per_strip, 9, 3, checksum, num_lods, replacements_at, 1, bodypart_at)
    return bytes(out)
