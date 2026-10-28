import bpy, sys
from mathutils import Vector

src, out, tris, tex = sys.argv[sys.argv.index("--") + 1:][:4]
tris, tex = int(tris), int(tex)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
bpy.ops.object.select_all(action="DESELECT")
for o in meshes:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
if len(meshes) > 1:
    bpy.ops.object.join()
ob = bpy.context.view_layer.objects.active
ob.name = "prop"
if ob.parent:
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
for o in list(bpy.context.scene.objects):
    if o.type == "EMPTY":
        bpy.data.objects.remove(o)
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.mesh.remove_doubles(threshold=1e-5)
bpy.ops.mesh.normals_make_consistent(inside=False)
bpy.ops.object.mode_set(mode="OBJECT")
bpy.ops.object.modifier_add(type="TRIANGULATE")
bpy.ops.object.modifier_apply(modifier="Triangulate")
dec = ob.modifiers.new("d", "DECIMATE")
dec.ratio = min(1.0, tris / max(1, len(ob.data.polygons)))
bpy.ops.object.modifier_apply(modifier="d")
vs = [v.co for v in ob.data.vertices]
lo = Vector((min(v.x for v in vs), min(v.y for v in vs), min(v.z for v in vs)))
hi = Vector((max(v.x for v in vs), max(v.y for v in vs), max(v.z for v in vs)))
k = 1.0 / (hi.z - lo.z)
ob.scale = (k, k, k)
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
ob.location = (-(lo.x + hi.x) / 2 * k, -(lo.y + hi.y) / 2 * k, -lo.z * k)
bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
for p in ob.data.polygons:
    p.use_smooth = True
for img in bpy.data.images:
    if img.size[0] > tex:
        img.scale(tex, tex)
print("tris", len(ob.data.polygons), "size", tuple(round(x, 3) for x in ob.dimensions))
bpy.ops.object.select_all(action="DESELECT")
ob.select_set(True)
bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", use_selection=True, export_image_format="JPEG", export_jpeg_quality=88, export_yup=True)
