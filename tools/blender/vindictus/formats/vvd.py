"""Vertex data (.vvd, IDSV version 4): per vertex up to 3 bone weights, position, normal, UV, plus a
tangent block. LOD fixups map the LOD 0 vertex list onto the stored data."""
from __future__ import annotations
import struct
from dataclasses import dataclass

HEADER = struct.Struct('<4siIi8iiiii')
VERTEX = struct.Struct('<3f3BB3f3f2f')
TANGENT = struct.Struct('<4f')


class VvdError(ValueError):
    pass


@dataclass
class Vertex:
    weights: tuple   # 3 floats
    bones: tuple     # 3 bone indices
    num_bones: int
    pos: tuple
    normal: tuple
    uv: tuple


@dataclass
class Vvd:
    checksum: int
    num_lods: int
    vertices: list[Vertex]   # LOD 0 order
    tangents: list[tuple]


def read(data: bytes) -> Vvd:
    head = HEADER.unpack_from(data, 0)
    if head[0] != b'IDSV' or head[1] != 4:
        raise VvdError('Not a version 4 vertex file.')
    checksum, num_lods = head[2], head[3]
    lod_vertices = head[4:12]
    num_fixups, fixup_start, vertex_start, tangent_start = head[12:16]
    total = lod_vertices[0]
    stored = [VERTEX.unpack_from(data, vertex_start + i * VERTEX.size) for i in range((tangent_start - vertex_start) // VERTEX.size)]
    tangents = [TANGENT.unpack_from(data, tangent_start + i * TANGENT.size) for i in range(len(stored))]
    if num_fixups:
        order = []
        for k in range(num_fixups):
            lod, source, count = struct.unpack_from('<iii', data, fixup_start + 12 * k)
            if lod >= 0:
                order.extend(range(source, source + count))
    else:
        order = list(range(total))
    if len(order) != total:
        raise VvdError('LOD 0 vertex count does not match the fixups.')
    vertices = [Vertex(v[0:3], v[3:6], v[6], v[7:10], v[10:13], v[13:15]) for v in (stored[i] for i in order)]
    return Vvd(checksum, num_lods, vertices, [tangents[i] for i in order])


def write(checksum: int, num_lods: int, vertices: list[Vertex], tangents: list[tuple]) -> bytes:
    """All LODs share the LOD 0 vertices, so no fixups are needed."""
    if len(tangents) != len(vertices):
        raise VvdError('Every vertex needs a tangent.')
    vertex_start = HEADER.size
    tangent_start = vertex_start + VERTEX.size * len(vertices)
    out = bytearray(HEADER.pack(b'IDSV', 4, checksum, num_lods, *([len(vertices)] * num_lods + [0] * (8 - num_lods)),
                                0, vertex_start, vertex_start, tangent_start))
    for v in vertices:
        out += VERTEX.pack(*v.weights, *v.bones, v.num_bones, *v.pos, *v.normal, *v.uv)
    for t in tangents:
        out += TANGENT.pack(*t)
    return bytes(out)
