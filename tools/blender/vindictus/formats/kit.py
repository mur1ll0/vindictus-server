"""A model kit (scripts/client/model_kit.py): the game files of one studio model and its materials in the
game's folder layout. `load` turns it into plain geometry per model and mesh; `save` writes an edited
kit by swapping the template's meshes (skeleton, header, attachments and sequences stay the template's).
No bpy here: the Blender add-on and the tests share it."""
from __future__ import annotations
import json
import re
from dataclasses import dataclass, field
from pathlib import Path

from . import mdl as mdl_format, vtf as vtf_format, vtx as vtx_format, vvd as vvd_format

VTX_SUFFIXES = ('.dx90.vtx', '.dx80.vtx', '.sw.vtx')
TEXTURE_PARAM = re.compile(r'("\$(basetexture|bumpmap)"\s+")([^"]*)(")', re.IGNORECASE)


class KitError(ValueError):
    pass


@dataclass
class MeshData:
    material: int                    # skin reference index of the template mesh
    positions: list[tuple]
    normals: list[tuple]
    uvs: list[tuple]
    weights: list[list[tuple]]       # per vertex [(bone index, weight), ...] up to 3
    triangles: list[tuple]           # mesh-local vertex ids


@dataclass
class Material:
    texture: str                     # name in the model
    vmt_path: str | None             # kit-relative path
    vmt: str | None
    maps: dict[str, str] = field(default_factory=dict)  # parameter ($basetexture...) -> kit-relative .vtf path


@dataclass
class Kit:
    root: Path
    model_path: str                  # models/....mdl
    mdl: mdl_format.Mdl
    num_lods: int
    max_bones_per_strip: int
    models: list[list[MeshData]]     # [model][mesh]
    materials: list[Material]


MAX_ZIP_FILES = 512
MAX_ZIP_BYTES = 512 * 1024 * 1024


def extract_zip(zip_path: Path, folder: Path) -> Path:
    """Extracts a kit zip (the panel's "Exportar kit") into `folder` and returns the kit root.
    Only game paths inside the folder are written; the zip must have a kit.json."""
    import zipfile
    folder = Path(folder).resolve()
    with zipfile.ZipFile(zip_path) as archive:
        entries = [e for e in archive.infolist() if not e.is_dir()]
        if len(entries) > MAX_ZIP_FILES or sum(e.file_size for e in entries) > MAX_ZIP_BYTES:
            raise KitError('The kit zip is too large.')
        names = {e.filename.replace('\\', '/').lower(): e for e in entries}
        if 'kit.json' not in names:
            raise KitError('The zip is not a model kit (no kit.json).')
        for name, entry in names.items():
            target = (folder / name).resolve()
            if not target.is_relative_to(folder) or name.startswith('/') or '..' in name.split('/'):
                raise KitError(f'Unsafe path in the kit zip: {entry.filename}')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.read(entry))
    return folder


def zip_folder(folder: Path, zip_path: Path) -> None:
    """A kit folder as a stored zip (the panel's "Exportar kit" format: forward slashes, kit.json)."""
    import zipfile
    folder = Path(folder)
    if not (folder / 'kit.json').is_file():
        raise KitError('The folder is not a model kit (no kit.json).')
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_STORED) as archive:
        for path in sorted(p for p in folder.rglob('*') if p.is_file()):
            archive.write(path, path.relative_to(folder).as_posix())


def _norm(path: str) -> str:
    return path.replace('\\', '/').lower().strip('/')


def load(root: Path, model_path: str | None = None) -> Kit:
    root = Path(root)
    if model_path is None:
        model_path = json.loads((root / 'kit.json').read_text(encoding='utf-8'))['model']
    model_path = _norm(model_path)
    base = root / model_path[:-4]
    mdl = mdl_format.read((root / model_path).read_bytes())
    vvd = vvd_format.read(base.with_name(base.name + '.vvd').read_bytes())
    vtx = vtx_format.read(base.with_name(base.name + '.dx90.vtx').read_bytes())
    if not (mdl.checksum == vvd.checksum == vtx.checksum):
        raise KitError('Model, vertex and optimized model checksums differ.')
    models = []
    for b, part in enumerate(mdl.bodyparts):
        for m, model in enumerate(part.models):
            first = model.vertex_index // 48
            meshes = []
            for k, mesh in enumerate(model.meshes):
                vertices = vvd.vertices[first + mesh.vertex_offset:first + mesh.vertex_offset + mesh.num_vertices]
                meshes.append(MeshData(
                    mesh.material,
                    [v.pos for v in vertices], [v.normal for v in vertices], [v.uv for v in vertices],
                    [[(v.bones[i], v.weights[i]) for i in range(v.num_bones)] for v in vertices],
                    vtx.triangles[b][m][k]))
            models.append(meshes)
    materials = []
    for texture in mdl.textures:
        found = Material(texture, None, None)
        for folder in mdl.cdmaterials:
            rel = 'materials/' + _norm(folder + texture) + '.vmt'
            if (root / rel).is_file():
                text = (root / rel).read_text(encoding='latin-1')
                found = Material(texture, rel, text, {p.lower(): 'materials/' + _norm(v) + '.vtf'
                                                      for _, p, v, _ in TEXTURE_PARAM.findall(text) if v})
                break
        materials.append(found)
    return Kit(root, model_path, mdl, vtx.num_lods, vtx.max_bones_per_strip, models, materials)


def read_texture(kit: Kit, rel: str):
    return vtf_format.read((kit.root / rel).read_bytes())


def _tangents(mesh: MeshData) -> list[tuple]:
    """Per-vertex tangents from UV derivatives (w = handedness), as studiomdl computes them."""
    tan = [[0.0, 0.0, 0.0] for _ in mesh.positions]
    bit = [[0.0, 0.0, 0.0] for _ in mesh.positions]
    for a, b, c in mesh.triangles:
        p0, p1, p2 = mesh.positions[a], mesh.positions[b], mesh.positions[c]
        u0, u1, u2 = mesh.uvs[a], mesh.uvs[b], mesh.uvs[c]
        e1 = [p1[i] - p0[i] for i in range(3)]
        e2 = [p2[i] - p0[i] for i in range(3)]
        du1, dv1, du2, dv2 = u1[0] - u0[0], u1[1] - u0[1], u2[0] - u0[0], u2[1] - u0[1]
        det = du1 * dv2 - du2 * dv1
        if abs(det) < 1e-12:
            continue
        r = 1.0 / det
        t = [(e1[i] * dv2 - e2[i] * dv1) * r for i in range(3)]
        s = [(e2[i] * du1 - e1[i] * du2) * r for i in range(3)]
        for v in (a, b, c):
            for i in range(3):
                tan[v][i] += t[i]
                bit[v][i] += s[i]
    out = []
    for n, t, s in zip(mesh.normals, tan, bit):
        d = sum(n[i] * t[i] for i in range(3))
        t = [t[i] - n[i] * d for i in range(3)]
        length = sum(x * x for x in t) ** 0.5
        if length < 1e-9:
            t, length = [1.0, 0.0, 0.0], 1.0
        t = [x / length for x in t]
        cross = (n[1] * t[2] - n[2] * t[1], n[2] * t[0] - n[0] * t[2], n[0] * t[1] - n[1] * t[0])
        w = -1.0 if sum(cross[i] * s[i] for i in range(3)) < 0 else 1.0
        out.append((t[0], t[1], t[2], w))
    return out


def save(kit: Kit, out: Path, model_path: str, models: list[list[MeshData]],
         textures: dict[str, bytes] | None = None) -> list[str]:
    """Writes an edited kit to `out` under `model_path` (models/....mdl). Materials move to a folder named
    after the model (materials/<model folder>/<model name>/); `textures` replaces .vtf files by the
    parameter path they had in the template (kit-relative). Returns the kit-relative files written."""
    out = Path(out)
    model_path = _norm(model_path)
    if not model_path.startswith('models/') or not model_path.endswith('.mdl'):
        raise KitError('The model path must be models/....mdl.')
    template_models = mdl_format.models(kit.mdl)
    if len(models) != len(template_models) or any(len(a) != len(b.meshes) for a, b in zip(models, template_models)):
        raise KitError('The edited model must keep the template models and meshes (one per material).')
    folder = model_path[len('models/'):-4]                      # player/fiona/fiona_inner_lab
    material_folder = 'models/' + folder + '/'                  # materials/models/player/fiona/fiona_inner_lab/
    vertices, tangents, counts = [], [], []
    vtx_meshes, vtx_bones, vtx_weights = [], [], []
    for meshes in models:
        counts.append([len(m.positions) for m in meshes])
        mesh_tris, mesh_bones, mesh_counts = [], [], []
        for mesh in meshes:
            for i in range(len(mesh.positions)):
                w = sorted(mesh.weights[i], key=lambda bw: -bw[1])[:3]
                total = sum(x for _, x in w) or 1.0
                w = [(b, x / total) for b, x in w] or [(0, 1.0)]
                bones = [b for b, _ in w]
                vertices.append(vvd_format.Vertex(tuple(x for _, x in w) + (0.0,) * (3 - len(w)), tuple(bones) + (0,) * (3 - len(bones)),
                                                  len(w), tuple(mesh.positions[i]), tuple(mesh.normals[i]), tuple(mesh.uvs[i])))
            tangents += _tangents(mesh)
            mesh_tris.append(mesh.triangles)
            mesh_bones.append([tuple(b for b, _ in sorted(ws, key=lambda bw: -bw[1])[:3]) or (0,) for ws in mesh.weights])
            mesh_counts.append([max(1, min(3, len(ws))) for ws in mesh.weights])
        vtx_meshes.append(mesh_tris)
        vtx_bones.append(mesh_bones)
        vtx_weights.append(mesh_counts)
    if len(vertices) > 65535:
        raise KitError('A model can have at most 65535 vertices.')
    checksum = kit.mdl.checksum
    written = []
    base = out / model_path[:-4]
    base.parent.mkdir(parents=True, exist_ok=True)
    name = model_path[len('models/'):].replace('/', '\\')
    (out / model_path).write_bytes(mdl_format.rewrite(kit.mdl, name, [material_folder.replace('/', '\\')] * len(kit.mdl.cdmaterials),
                                                      None, counts))
    (base.with_name(base.name + '.vvd')).write_bytes(vvd_format.write(checksum, kit.num_lods, vertices, tangents))
    optimized = vtx_format.write(checksum, kit.num_lods, kit.max_bones_per_strip, vtx_meshes, vtx_bones, vtx_weights)
    for suffix in VTX_SUFFIXES:
        base.with_name(base.name + suffix).write_bytes(optimized)
    written += [model_path] + [model_path[:-4] + s for s in ('.vvd',) + VTX_SUFFIXES]
    textures = textures or {}
    for material in kit.materials:
        if material.vmt is None:
            continue
        new_maps = {}
        text = material.vmt
        for param, rel in material.maps.items():
            name_only = rel.rsplit('/', 1)[1]
            new_rel = 'materials/' + material_folder + name_only
            new_maps[param] = new_rel
            data = textures.get(rel)
            if data is None and (kit.root / rel).is_file():
                data = (kit.root / rel).read_bytes()
            if data is not None:
                (out / new_rel).parent.mkdir(parents=True, exist_ok=True)
                (out / new_rel).write_bytes(data)
                written.append(new_rel)
        text = TEXTURE_PARAM.sub(lambda m: m.group(1) + new_maps.get(m.group(2).lower(), 'materials/' + m.group(3) + '.vtf')[len('materials/'):-len('.vtf')]
                                 .replace('/', '\\') + m.group(4) if m.group(3) else m.group(0), text)
        vmt_rel = 'materials/' + material_folder + material.texture.lower() + '.vmt'
        (out / vmt_rel).parent.mkdir(parents=True, exist_ok=True)
        (out / vmt_rel).write_text(text, encoding='latin-1')
        written.append(vmt_rel)
    written = sorted(set(written))
    (out / 'kit.json').write_text(json.dumps({'format_version': 1, 'model': model_path, 'template': kit.model_path,
                                              'files': written}, indent=1), encoding='utf-8')
    return written
