"""Blender side of the Vindictus add-on: kit <-> armature, meshes and materials (formats/kit.py)."""
from __future__ import annotations
import hashlib
from pathlib import Path

import bpy
import numpy as np
from mathutils import Matrix

from .formats import kit as kit_format, vtf as vtf_format

PROP_KIT, PROP_MODEL, PROP_INDEX, PROP_SCALE = 'vindictus_kit', 'vindictus_model', 'vindictus_model_index', 'vindictus_scale'
PROP_MESH, PROP_VTF, PROP_HASH = 'vindictus_mesh', 'vindictus_vtf', 'vindictus_pixels'
# Source bones point along +X, Blender bones along +Y.
BONE_AXIS = Matrix.Rotation(-1.5707963267948966, 4, 'Z')


def bone_world(bone) -> Matrix:
    p = bone.pose_to_bone
    pose = Matrix(((p[0], p[1], p[2], p[3]), (p[4], p[5], p[6], p[7]), (p[8], p[9], p[10], p[11]), (0, 0, 0, 1)))
    return pose.inverted_safe()


def pixels_hash(image) -> str:
    data = np.empty(len(image.pixels), np.float32)
    image.pixels.foreach_get(data)
    return hashlib.sha1(np.round(data * 255).astype(np.uint8).tobytes()).hexdigest()


def make_image(kit, rel: str, name: str):
    rgba, info = kit_format.read_texture(kit, rel)
    h, w = rgba.shape[:2]
    image = bpy.data.images.new(name, w, h, alpha=True)
    image.pixels.foreach_set((rgba[::-1].astype(np.float32) / 255.0).ravel())
    image.pack()
    image[PROP_VTF] = rel
    image[PROP_HASH] = pixels_hash(image)
    return image


def make_material(kit, index: int, import_textures: bool):
    source = kit.materials[index]
    material = bpy.data.materials.new(source.texture)
    material[PROP_MESH] = index
    material.use_nodes = True
    nodes, links = material.node_tree.nodes, material.node_tree.links
    shader = nodes.get('Principled BSDF')
    if import_textures and source.maps.get('basetexture') and (kit.root / source.maps['basetexture']).is_file():
        tex = nodes.new('ShaderNodeTexImage')
        tex.image = make_image(kit, source.maps['basetexture'], source.texture)
        tex.location = (-400, 250)
        links.new(tex.outputs['Color'], shader.inputs['Base Color'])
        links.new(tex.outputs['Alpha'], shader.inputs['Alpha'])
    if import_textures and source.maps.get('bumpmap') and (kit.root / source.maps['bumpmap']).is_file():
        tex = nodes.new('ShaderNodeTexImage')
        tex.image = make_image(kit, source.maps['bumpmap'], source.texture + '_normal')
        tex.image.colorspace_settings.name = 'Non-Color'
        tex.location = (-600, -200)
        normal = nodes.new('ShaderNodeNormalMap')
        normal.location = (-300, -200)
        links.new(tex.outputs['Color'], normal.inputs['Color'])
        links.new(normal.outputs['Normal'], shader.inputs['Normal'])
    return material


def build_armature(kit, name: str, scale: float):
    data = bpy.data.armatures.new(name)
    obj = bpy.data.objects.new(name, data)
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode='EDIT')
    worlds = [bone_world(b) for b in kit.mdl.bones]
    edit = []
    for bone, world in zip(kit.mdl.bones, worlds):
        eb = data.edit_bones.new(bone.name)
        eb.head, eb.tail = (0, 0, 0), (0, 1, 0)
        eb.length = 1.0
        m = Matrix.Scale(scale, 4) @ world @ BONE_AXIS
        eb.matrix = m
        edit.append(eb)
    for bone, eb in zip(kit.mdl.bones, edit):
        if bone.parent >= 0:
            eb.parent = edit[bone.parent]
        children = [edit[i] for i, b in enumerate(kit.mdl.bones) if b.parent == kit.mdl.bones.index(bone)]
        if children:
            length = (children[0].head - eb.head).length
            if length > 1e-4:
                eb.length = length
    bpy.ops.object.mode_set(mode='OBJECT')
    return obj


def build_mesh(kit, model_index: int, name: str, scale: float, materials):
    meshes = kit.models[model_index]
    positions, normals, uvs, faces, face_material, weights = [], [], [], [], [], []
    for k, mesh in enumerate(meshes):
        first = len(positions)
        positions += [(p[0] * scale, p[1] * scale, p[2] * scale) for p in mesh.positions]
        normals += mesh.normals
        uvs += [(u, 1.0 - v) for u, v in mesh.uvs]
        weights += mesh.weights
        faces += [(first + a, first + b, first + c) for a, b, c in mesh.triangles]
        face_material += [k] * len(mesh.triangles)
    data = bpy.data.meshes.new(name)
    data.from_pydata(positions, [], faces)
    data.update()
    for material in materials:
        data.materials.append(material)
    data.polygons.foreach_set('material_index', face_material)
    uv = data.uv_layers.new(name='UVMap')
    loop_vertex = np.empty(len(data.loops), np.int32)
    data.loops.foreach_get('vertex_index', loop_vertex)
    uv_array = np.array(uvs, np.float32)[loop_vertex]
    uv.data.foreach_set('uv', uv_array.ravel())
    data.normals_split_custom_set_from_vertices(normals)
    obj = bpy.data.objects.new(name, data)
    bpy.context.collection.objects.link(obj)
    groups = {}
    for v, ws in enumerate(weights):
        for bone, w in ws:
            name_ = kit.mdl.bones[bone].name
            group = groups.get(name_) or obj.vertex_groups.new(name=name_)
            groups[name_] = group
            group.add([v], w, 'REPLACE')
    return obj


def import_kit(root: str, import_mesh=True, import_textures=True, import_skeleton=True, mode='NEW', scale=1.0, target=None):
    """mode: NEW objects, REPLACE the mesh data of `target`, or ADD the imported geometry to `target`."""
    kit = kit_format.load(Path(root))
    name = Path(kit.model_path).stem
    armature = build_armature(kit, name + '_skeleton', scale) if import_skeleton else None
    if armature is not None:
        armature[PROP_KIT], armature[PROP_MODEL], armature[PROP_SCALE] = str(kit.root), kit.model_path, scale
    created = []
    if import_mesh:
        materials = [make_material(kit, i, import_textures) for i in range(len(kit.materials))]
        for m in range(len(kit.models)):
            obj = build_mesh(kit, m, f'{name}_{m}', scale, [materials[mesh.material] for mesh in kit.models[m]])
            obj[PROP_KIT], obj[PROP_MODEL], obj[PROP_INDEX], obj[PROP_SCALE] = str(kit.root), kit.model_path, m, scale
            for slot, mesh in enumerate(kit.models[m]):
                obj.data.materials[slot][PROP_MESH] = slot
            if armature is not None:
                obj.parent = armature
                obj.modifiers.new('Armature', 'ARMATURE').object = armature
            created.append(obj)
    if mode in ('REPLACE', 'ADD') and target is not None and target.type == 'MESH' and created:
        source = created[0]
        if mode == 'REPLACE':
            old = target.data
            target.data = source.data
            if old.users == 0:
                bpy.data.meshes.remove(old)
            # Vertex groups belong to the object: rebuild the target's from the imported ones.
            target.vertex_groups.clear()
            for group in source.vertex_groups:
                target.vertex_groups.new(name=group.name)
            _copy_weights(source, target)
            bpy.data.objects.remove(source)
        else:
            bpy.ops.object.select_all(action='DESELECT')
            source.select_set(True)
            target.select_set(True)
            bpy.context.view_layer.objects.active = target
            bpy.ops.object.join()
        created = [target]
    return armature, created


def _copy_weights(source, target):
    names = [g.name for g in source.vertex_groups]
    for v in source.data.vertices:
        for g in v.groups:
            target.vertex_groups[names[g.group]].add([v.index], g.weight, 'REPLACE')


def mesh_data(obj, kit, scale: float) -> list:
    """The object's geometry as template meshes (one per material slot that maps to a template mesh)."""
    bone_index = {b.name: i for i, b in enumerate(kit.mdl.bones)}
    group_bone = {g.index: bone_index.get(g.name) for g in obj.vertex_groups}
    data = obj.data
    data.calc_loop_triangles()
    template = kit.models[obj[PROP_INDEX]]
    out = [kit_format.MeshData(t.material, [], [], [], [], []) for t in template]
    keys = [dict() for _ in template]
    uv_layer = data.uv_layers.active.data if data.uv_layers.active else None
    corner_normals = data.corner_normals
    for tri in data.loop_triangles:
        slot_material = data.materials[tri.material_index] if tri.material_index < len(data.materials) else None
        k = slot_material.get(PROP_MESH, tri.material_index) if slot_material else tri.material_index
        if k >= len(out):
            k = len(out) - 1
        ids = []
        for loop in tri.loops:
            v = data.loops[loop].vertex_index
            n = tuple(round(x, 5) for x in corner_normals[loop].vector)
            uv = tuple(round(x, 6) for x in uv_layer[loop].uv) if uv_layer else (0.0, 0.0)
            # One game vertex per Blender vertex and UV: imported meshes already split hard edges into
            # separate vertices, so corner normals of one vertex differ only by rounding.
            key = (v, uv)
            index = keys[k].get(key)
            if index is None:
                index = len(out[k].positions)
                keys[k][key] = index
                co = data.vertices[v].co
                out[k].positions.append((co.x / scale, co.y / scale, co.z / scale))
                out[k].normals.append(n)
                out[k].uvs.append((uv[0], 1.0 - uv[1]))
                ws = [(group_bone[g.group], g.weight) for g in data.vertices[v].groups
                      if group_bone.get(g.group) is not None and g.weight > 0]
                out[k].weights.append(ws or [(0, 1.0)])
            ids.append(index)
        out[k].triangles.append(tuple(ids))
    return out


def changed_textures(objects) -> dict[str, bytes]:
    """VTF bytes of imported images whose pixels changed since import, by template path."""
    textures = {}
    for obj in objects:
        for material in obj.data.materials:
            if material is None or not material.use_nodes:
                continue
            for node in material.node_tree.nodes:
                image = getattr(node, 'image', None)
                if image is None or PROP_VTF not in image or pixels_hash(image) == image.get(PROP_HASH):
                    continue
                w, h = image.size
                data = np.empty(w * h * 4, np.float32)
                image.pixels.foreach_get(data)
                rgba = np.clip(np.rint(data.reshape(h, w, 4)[::-1] * 255), 0, 255).astype(np.uint8)
                textures[image[PROP_VTF]] = vtf_format.write(rgba)
    return textures


def export_kit(objects, out: str, model_path: str) -> list[str]:
    meshes = [o for o in objects if o.type == 'MESH' and PROP_KIT in o]
    if not meshes:
        raise kit_format.KitError('Select meshes imported from a Vindictus kit.')
    kit = kit_format.load(Path(meshes[0][PROP_KIT]), meshes[0][PROP_MODEL])
    scale = float(meshes[0].get(PROP_SCALE, 1.0))
    models = [m.copy() for m in kit.models]
    for obj in meshes:
        models[obj[PROP_INDEX]] = mesh_data(obj, kit, scale)
    return kit_format.save(kit, Path(out), model_path, models, changed_textures(meshes))
