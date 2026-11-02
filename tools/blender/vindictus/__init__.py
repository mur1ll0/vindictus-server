"""Vindictus model kits for Blender 5.1: import a kit exported by the admin panel (or
scripts/client/model_kit.py) as armature, weighted meshes and textured materials, edit it, and export
an edited kit that the server turns into a new item (docs/future/item-editor.md)."""
import bpy
from bpy.props import BoolProperty, EnumProperty, FloatProperty, StringProperty
from bpy_extras.io_utils import ExportHelper, ImportHelper

from . import blender_io, mesh_fix
from .formats import kit as kit_format


def kit_root(path: str) -> str:
    """The kit folder of a picked file: kit.json's folder, or a zip (the panel's "Exportar kit") extracted
    into the add-on's user folder. Exports read the kit's original textures, so the folder stays."""
    import hashlib
    from pathlib import Path
    picked = Path(path)
    if picked.suffix.lower() != '.zip':
        return str(picked.parent)
    digest = hashlib.sha1(picked.read_bytes()).hexdigest()[:8]
    try:
        base = Path(bpy.utils.extension_path_user(__package__, path='kits', create=True))
    except (ValueError, RuntimeError):
        base = picked.parent  # loaded outside the extension system (development)
    # Short folder names: material paths inside a kit are long and Windows paths stop at 260 characters.
    folder = base / f'{picked.stem[:16]}-{digest}'
    if not (folder / 'kit.json').is_file():
        kit_format.extract_zip(picked, folder)
    return str(folder)


class VINDICTUS_OT_import_kit(bpy.types.Operator, ImportHelper):
    """Import a Vindictus model kit: the panel's .zip or a kit.json in a kit folder"""
    bl_idname = 'vindictus.import_kit'
    bl_label = 'Import Vindictus Kit'
    bl_options = {'REGISTER', 'UNDO'}
    filename_ext = '.zip'
    # Both a .zip and a kit.json are valid picks: do not append '.zip' to the chosen name.
    check_extension = False
    filter_glob: StringProperty(default='*.zip;*.json', options={'HIDDEN'})
    import_mesh: BoolProperty(name='Mesh', default=True)
    import_textures: BoolProperty(name='Textures', default=True)
    import_skeleton: BoolProperty(name='Skeleton', default=True)
    mode: EnumProperty(name='Into', items=(
        ('NEW', 'New objects', 'Create new objects'),
        ('REPLACE', 'Replace active mesh', 'Replace the mesh of the active object'),
        ('ADD', 'Add to active mesh', 'Join the imported mesh into the active object')), default='NEW')
    scale: FloatProperty(name='Scale', default=1.0, min=0.0001, description='Blender units per game unit')

    def execute(self, context):
        import zipfile
        try:
            blender_io.import_kit(kit_root(self.filepath), self.import_mesh, self.import_textures,
                                  self.import_skeleton, self.mode, self.scale, context.active_object)
        except (kit_format.KitError, zipfile.BadZipFile, ValueError, OSError) as error:
            self.report({'ERROR'}, str(error))
            return {'CANCELLED'}
        return {'FINISHED'}


class VINDICTUS_OT_export_kit(bpy.types.Operator, ExportHelper):
    """Export the selected Vindictus meshes as a new model kit (.zip, the panel's format)"""
    bl_idname = 'vindictus.export_kit'
    bl_label = 'Export Vindictus Kit'
    filename_ext = '.zip'
    filter_glob: StringProperty(default='*.zip', options={'HIDDEN'})
    model_path: StringProperty(name='Model', default='models/player/fiona/fiona_inner_new.mdl',
                               description='Game path of the new model (models/....mdl)')

    def invoke(self, context, event):
        # A kit mesh suggests its own model: the corrected kit replaces it in the panel (Modelos 3D)
        model = next((o.get(blender_io.PROP_MODEL) for o in context.selected_objects
                      if o.type == 'MESH' and blender_io.PROP_MODEL in o), None)
        if model:
            self.model_path = str(model)
        return super().invoke(context, event)

    def execute(self, context):
        import tempfile
        try:
            with tempfile.TemporaryDirectory() as folder:
                files = blender_io.export_kit(context.selected_objects, folder, self.model_path)
                kit_format.zip_folder(folder, self.filepath)
        except (kit_format.KitError, ValueError, OSError) as error:
            self.report({'ERROR'}, str(error))
            return {'CANCELLED'}
        self.report({'INFO'}, f'Exported {len(files)} files to {self.filepath}')
        return {'FINISHED'}


def kit_meshes(context):
    # the view layer's selection: context.selected_objects exists only in a 3D View
    return [o for o in context.view_layer.objects.selected if o.type == 'MESH' and blender_io.PROP_KIT in o]


class VINDICTUS_OT_push_out(bpy.types.Operator):
    """Push the selected kit meshes out along their game normals: for armour pieces that sit inside the body
    (the skin shows through them); export the kit afterwards to update the model in the panel"""
    bl_idname = 'vindictus.push_out'
    bl_label = 'Push Mesh Out (fix clipping)'
    bl_options = {'REGISTER', 'UNDO'}
    offset: FloatProperty(name='Offset', default=mesh_fix.PUSH_OUT, min=-5.0, max=5.0, step=1, precision=3,
                          description='Game units along the normal; 0.3 covered the skin under the NA pytonshock '
                                      'and Dragon Rise pieces (negative values pull the mesh in)')

    @classmethod
    def poll(cls, context):
        return context.mode == 'OBJECT' and bool(kit_meshes(context))

    def execute(self, context):
        count = sum(blender_io.push_out(obj, self.offset) for obj in kit_meshes(context))
        self.report({'INFO'}, f'Moved {count} vertices {self.offset:g} game units along their normals')
        return {'FINISHED'}


class VINDICTUS_PT_kit(bpy.types.Panel):
    bl_label = 'Vindictus Kit'
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'Vindictus'

    def draw(self, context):
        column = self.layout.column(align=True)
        column.operator(VINDICTUS_OT_import_kit.bl_idname, text='Import Kit', icon='IMPORT')
        column.operator(VINDICTUS_OT_push_out.bl_idname, text='Push Mesh Out', icon='MOD_DISPLACE')
        column.operator(VINDICTUS_OT_export_kit.bl_idname, text='Export Kit', icon='EXPORT')


def menu_object(self, context):
    self.layout.operator(VINDICTUS_OT_push_out.bl_idname)


def menu_import(self, context):
    self.layout.operator(VINDICTUS_OT_import_kit.bl_idname, text='Vindictus Kit (.zip / kit.json)')


def menu_export(self, context):
    self.layout.operator(VINDICTUS_OT_export_kit.bl_idname, text='Vindictus Kit (.zip)')


CLASSES = (VINDICTUS_OT_import_kit, VINDICTUS_OT_export_kit, VINDICTUS_OT_push_out, VINDICTUS_PT_kit)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.TOPBAR_MT_file_import.append(menu_import)
    bpy.types.TOPBAR_MT_file_export.append(menu_export)
    bpy.types.VIEW3D_MT_object.append(menu_object)


def unregister():
    bpy.types.VIEW3D_MT_object.remove(menu_object)
    bpy.types.TOPBAR_MT_file_export.remove(menu_export)
    bpy.types.TOPBAR_MT_file_import.remove(menu_import)
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
