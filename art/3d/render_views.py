import bpy, sys, math
from mathutils import Vector
argv = sys.argv[sys.argv.index("--") + 1:]
src, out = argv[0], argv[1]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
objs = [o for o in bpy.context.scene.objects if o.type == "MESH"]
lo = Vector((1e9,) * 3); hi = Vector((-1e9,) * 3)
for o in objs:
    for c in o.bound_box:
        w = o.matrix_world @ Vector(c)
        lo = Vector(map(min, lo, w)); hi = Vector(map(max, hi, w))
mid = (lo + hi) / 2; size = (hi - lo).length
sc = bpy.context.scene
sc.render.engine = "BLENDER_WORKBENCH"
sc.display.shading.light = "STUDIO"
sc.display.shading.color_type = "TEXTURE"
sc.render.resolution_x = sc.render.resolution_y = 384
sc.render.film_transparent = False
sc.world = bpy.data.worlds.new("w"); sc.world.color = (0.9, 0.9, 0.9)
cam = bpy.data.objects.new("c", bpy.data.cameras.new("c")); sc.collection.objects.link(cam); sc.camera = cam
cam.data.type = "ORTHO"; cam.data.ortho_scale = size * 0.9
for i, ang in enumerate((0, 90, 180, 270)):
    a = math.radians(ang)
    cam.location = mid + Vector((math.sin(a), -math.cos(a), 0)) * size * 2
    cam.rotation_euler = Vector(mid - cam.location).to_track_quat("-Z", "Y").to_euler()
    sc.render.filepath = f"{out}_{i}.png"
    bpy.ops.render.render(write_still=True)
print("bounds", tuple(lo), tuple(hi))
