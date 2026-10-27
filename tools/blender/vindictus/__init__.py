"""Vindictus model kits for Blender 5.1: import a kit exported by the admin panel (or
scripts/client/model_kit.py) as armature, weighted meshes and textured materials, edit it, and export
an edited kit that the server turns into a new item (docs/future/item-editor.md)."""
import bpy
from bpy.props import BoolProperty, EnumProperty, FloatProperty, StringProperty
from bpy_extras.io_utils import ExportHelper, ImportHelper

from . import blender_io
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


def menu_import(self, context):
    self.layout.operator(VINDICTUS_OT_import_kit.bl_idname, text='Vindictus Kit (.zip / kit.json)')


def menu_export(self, context):
    self.layout.operator(VINDICTUS_OT_export_kit.bl_idname, text='Vindictus Kit (.zip)')


CLASSES = (VINDICTUS_OT_import_kit, VINDICTUS_OT_export_kit)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.TOPBAR_MT_file_import.append(menu_import)
    bpy.types.TOPBAR_MT_file_export.append(menu_export)


def unregister():
    bpy.types.TOPBAR_MT_file_export.remove(menu_export)
    bpy.types.TOPBAR_MT_file_import.remove(menu_import)
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
