"""Source engine studio model (.mdl) of the Vindictus 1.69 client: versions 44 (game) and 48 (newer
clients, accepted by the 1.69 engine). Only the parts an item model needs are read: header, bones,
textures, material folders, skin table and the body part / model / mesh tree. Layout from the Source
SDK studio.h (studiohdr_t, mstudiobone_t, mstudiobodyparts_t, mstudiomodel_t, mstudiomesh_t)."""
from __future__ import annotations
import struct
from dataclasses import dataclass, field

VERSIONS = (44, 45, 46, 47, 48)
BONE_SIZE = 216
BODYPART_SIZE = 16
MODEL_SIZE = 148
MESH_SIZE = 116
TEXTURE_SIZE = 64
MAX_LODS = 8


class MdlError(ValueError):
    pass


def cstring(data: bytes, at: int) -> str:
    end = data.index(b'\0', at)
    return data[at:end].decode('latin-1')


@dataclass
class Bone:
    name: str
    parent: int
    pos: tuple
    quat: tuple
    rot: tuple
    pose_to_bone: tuple  # 3x4 row-major
    flags: int


@dataclass
class Mesh:
    offset: int          # absolute offset of the mstudiomesh_t
    material: int
    num_vertices: int
    vertex_offset: int   # first vertex of the mesh inside its model
    lod_vertices: list[int]


@dataclass
class Model:
    offset: int
    name: str
    num_vertices: int
    vertex_index: int    # byte offset of the model's first vertex in the VVD vertex block
    tangents_index: int
    meshes: list[Mesh] = field(default_factory=list)


@dataclass
class BodyPart:
    offset: int
    name: str
    base: int
    models: list[Model] = field(default_factory=list)


@dataclass
class Mdl:
    data: bytes
    version: int
    checksum: int
    name: str
    flags: int
    bones: list[Bone]
    textures: list[str]
    texture_offsets: list[int]
    cdmaterials: list[str]
    skins: list[list[int]]
    bodyparts: list[BodyPart]


def read(data: bytes) -> Mdl:
    if data[:4] != b'IDST':
        raise MdlError('Not a studio model.')
    version, checksum = struct.unpack_from('<iI', data, 4)
    if version not in VERSIONS:
        raise MdlError(f'Unsupported studio model version {version}.')
    name = data[12:76].split(b'\0')[0].decode('latin-1')
    length = struct.unpack_from('<i', data, 76)[0]
    if length != len(data):
        raise MdlError('Studio model length does not match the file.')
    flags = struct.unpack_from('<i', data, 152)[0]
    num_bones, bone_index = struct.unpack_from('<ii', data, 156)
    num_textures, texture_index, num_cd, cd_index = struct.unpack_from('<iiii', data, 204)
    num_skinref, num_families, skin_index = struct.unpack_from('<iii', data, 220)
    num_bodyparts, bodypart_index = struct.unpack_from('<ii', data, 232)

    bones = []
    for i in range(num_bones):
        at = bone_index + i * BONE_SIZE
        name_index, parent = struct.unpack_from('<ii', data, at)
        pos = struct.unpack_from('<3f', data, at + 32)
        quat = struct.unpack_from('<4f', data, at + 44)
        rot = struct.unpack_from('<3f', data, at + 60)
        pose = struct.unpack_from('<12f', data, at + 96)
        bone_flags = struct.unpack_from('<i', data, at + 160)[0]
        bones.append(Bone(cstring(data, at + name_index), parent, pos, quat, rot, pose, bone_flags))

    textures, texture_offsets = [], []
    for i in range(num_textures):
        at = texture_index + i * TEXTURE_SIZE
        texture_offsets.append(at)
        textures.append(cstring(data, at + struct.unpack_from('<i', data, at)[0]))
    cdmaterials = [cstring(data, struct.unpack_from('<i', data, cd_index + 4 * i)[0]) for i in range(num_cd)]
    skins = [list(struct.unpack_from(f'<{num_skinref}h', data, skin_index + 2 * num_skinref * f)) for f in range(num_families)]

    bodyparts = []
    for b in range(num_bodyparts):
        at = bodypart_index + b * BODYPART_SIZE
        name_index, num_models, base, model_index = struct.unpack_from('<iiii', data, at)
        part = BodyPart(at, cstring(data, at + name_index), base)
        for m in range(num_models):
            mat = at + model_index + m * MODEL_SIZE
            mname = data[mat:mat + 64].split(b'\0')[0].decode('latin-1')
            num_meshes, mesh_index, num_vertices, vertex_index, tangents_index = struct.unpack_from('<iiiii', data, mat + 72)
            model = Model(mat, mname, num_vertices, vertex_index, tangents_index)
            for k in range(num_meshes):
                mesh_at = mat + mesh_index + k * MESH_SIZE
                material, _, mesh_vertices, vertex_offset = struct.unpack_from('<iiii', data, mesh_at)
                lods = list(struct.unpack_from('<8i', data, mesh_at + 52))
                model.meshes.append(Mesh(mesh_at, material, mesh_vertices, vertex_offset, lods))
            part.models.append(model)
        bodyparts.append(part)
    return Mdl(data, version, checksum, name, flags, bones, textures, texture_offsets, cdmaterials, skins, bodyparts)


def models(mdl: Mdl) -> list[Model]:
    return [m for part in mdl.bodyparts for m in part.models]


# --- writing (mesh swap on a template) ----------------------------------------------------------

def append_string(out: bytearray, text: str) -> int:
    at = len(out)
    out += text.encode('latin-1') + b'\0'
    return at


def rewrite(mdl: Mdl, name: str | None, cdmaterials: list[str] | None, textures: list[str] | None,
            mesh_counts: list[list[int]]) -> bytes:
    """The template model with a new internal name, material folders, texture names and per-model mesh
    vertex counts (in VVD order). Mesh and model counts stay those of the template; new strings are
    appended at the end of the file."""
    out = bytearray(mdl.data)
    if name is not None:
        raw = name.encode('latin-1')
        if len(raw) > 63:
            raise MdlError('Model name longer than 63 bytes.')
        out[12:76] = raw + bytes(64 - len(raw))
    if cdmaterials is not None:
        num_cd, cd_index = struct.unpack_from('<ii', out, 212)
        if len(cdmaterials) != num_cd:
            raise MdlError('The material folder count must stay that of the template.')
        for i, folder in enumerate(cdmaterials):
            struct.pack_into('<i', out, cd_index + 4 * i, append_string(out, folder))
    if textures is not None:
        if len(textures) != len(mdl.textures):
            raise MdlError('The texture count must stay that of the template.')
        for at, texture in zip(mdl.texture_offsets, textures):
            struct.pack_into('<i', out, at, append_string(out, texture) - at)
    all_models = models(mdl)
    if len(mesh_counts) != len(all_models) or any(len(c) != len(m.meshes) for c, m in zip(mesh_counts, all_models)):
        raise MdlError('Mesh counts must follow the template models and meshes.')
    total = 0
    for model, counts in zip(all_models, mesh_counts):
        struct.pack_into('<i', out, model.offset + 80, sum(counts))       # numvertices
        struct.pack_into('<i', out, model.offset + 84, total * 48)        # vertexindex (bytes)
        struct.pack_into('<i', out, model.offset + 88, total * 16)        # tangentsindex (bytes)
        first = 0
        for mesh, count in zip(model.meshes, counts):
            struct.pack_into('<ii', out, mesh.offset + 8, count, first)   # numvertices, vertexoffset
            struct.pack_into('<8i', out, mesh.offset + 52, *([count] * MAX_LODS))
            first += count
        total += sum(counts)
    struct.pack_into('<i', out, 76, len(out))
    return bytes(out)
