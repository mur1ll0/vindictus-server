"""Vindictus model kits for Blender 5.1: import a kit exported by the admin panel (or
scripts/client/model_kit.py) as armature, weighted meshes and textured materials, edit it, and export
an edited kit that the server turns into a new item (docs/future/item-editor.md)."""
import bpy
from bpy.props import BoolProperty, EnumProperty, FloatProperty, StringProperty
from bpy_extras.io_utils import ImportHelper

from . import blender_io
from .formats import kit as kit_format


class VINDICTUS_OT_import_kit(bpy.types.Operator, ImportHelper):
    """Import a Vindictus model kit (kit.json in a kit folder)"""
    bl_idname = 'vindictus.import_kit'
    bl_label = 'Import Vindictus Kit'
    bl_options = {'REGISTER', 'UNDO'}
    filename_ext = '.json'
    filter_glob: StringProperty(default='kit.json', options={'HIDDEN'})
    import_mesh: BoolProperty(name='Mesh', default=True)
    import_textures: BoolProperty(name='Textures', default=True)
    import_skeleton: BoolProperty(name='Skeleton', default=True)
    mode: EnumProperty(name='Into', items=(
        ('NEW', 'New objects', 'Create new objects'),
        ('REPLACE', 'Replace active mesh', 'Replace the mesh of the active object'),
        ('ADD', 'Add to active mesh', 'Join the imported mesh into the active object')), default='NEW')
    scale: FloatProperty(name='Scale', default=1.0, min=0.0001, description='Blender units per game unit')

    def execute(self, context):
        from pathlib import Path
        try:
            blender_io.import_kit(str(Path(self.filepath).parent), self.import_mesh, self.import_textures,
                                  self.import_skeleton, self.mode, self.scale, context.active_object)
        except (kit_format.KitError, ValueError, OSError) as error:
            self.report({'ERROR'}, str(error))
            return {'CANCELLED'}
        return {'FINISHED'}


class VINDICTUS_OT_export_kit(bpy.types.Operator):
    """Export the selected Vindictus meshes as a new model kit"""
    bl_idname = 'vindictus.export_kit'
    bl_label = 'Export Vindictus Kit'
    directory: StringProperty(subtype='DIR_PATH')
    model_path: StringProperty(name='Model', default='models/player/fiona/fiona_inner_new.mdl',
                               description='Game path of the new model (models/....mdl)')

    def invoke(self, context, event):
        context.window_manager.fileselect_add(self)
        return {'RUNNING_MODAL'}

    def execute(self, context):
        try:
            files = blender_io.export_kit(context.selected_objects, self.directory, self.model_path)
        except (kit_format.KitError, ValueError, OSError) as error:
            self.report({'ERROR'}, str(error))
            return {'CANCELLED'}
        self.report({'INFO'}, f'Exported {len(files)} files')
        return {'FINISHED'}


def menu_import(self, context):
    self.layout.operator(VINDICTUS_OT_import_kit.bl_idname, text='Vindictus Kit (kit.json)')


def menu_export(self, context):
    self.layout.operator(VINDICTUS_OT_export_kit.bl_idname, text='Vindictus Kit')


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
