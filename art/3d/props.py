import sys, torch, trimesh
from PIL import Image
from hy3dgen.shapegen import Hunyuan3DDiTFlowMatchingPipeline, FaceReducer, FloaterRemover, DegenerateFaceRemover
from hy3dgen.texgen import Hunyuan3DPaintPipeline

names = sys.argv[1:]
shape = Hunyuan3DDiTFlowMatchingPipeline.from_pretrained("tencent/Hunyuan3D-2")
meshes = {}
for n in names:
    im = Image.open(f"/tmp/rp_{n}.png").convert("RGBA")
    m = shape(image=im, num_inference_steps=40, octree_resolution=256, generator=torch.manual_seed(7))[0]
    m = FloaterRemover()(m)
    m = DegenerateFaceRemover()(m)
    m = FaceReducer()(m, max_facenum=20000)
    meshes[n] = (m, im)
    print("shape", n, len(m.faces), flush=True)
del shape
torch.cuda.empty_cache()
paint = Hunyuan3DPaintPipeline.from_pretrained("tencent/Hunyuan3D-2")
for n, (m, im) in meshes.items():
    t = paint(m, image=im)
    t.export(f"/tmp/rp_{n}.glb")
    print("tex", n, flush=True)
