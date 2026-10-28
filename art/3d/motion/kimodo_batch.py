import json
import os
import sys

import torch
from kimodo import load_model
from kimodo.exports.bvh import save_motion_bvh
from kimodo.skeleton import SOMASkeleton30, global_rots_to_local_rots
from kimodo.tools import seed_everything

jobs = json.load(open(sys.argv[1]))
out = sys.argv[2]
os.makedirs(out, exist_ok=True)
device = "cuda:0"
model = load_model("kimodo-soma-rp-v1.2", device=device, default_family="Kimodo")
skeleton = model.skeleton
if isinstance(skeleton, SOMASkeleton30):
    skeleton = skeleton.somaskel77.to(device)
for job in jobs:
    for seed in job["seeds"]:
        seed_everything(seed)
        frames = int(job["secs"] * model.fps)
        o = model(
            [job["prompt"]],
            [frames],
            num_denoising_steps=100,
            num_samples=1,
            multi_prompt=True,
            post_processing=True,
            return_numpy=True,
        )
        pos = torch.from_numpy(o["posed_joints"][0]).to(device)
        rot = torch.from_numpy(o["global_rot_mats"][0]).to(device)
        local = global_rots_to_local_rots(rot, skeleton)
        path = f"{out}/{job['name']}_{seed}.bvh"
        save_motion_bvh(path, local, pos[:, skeleton.root_idx, :], skeleton=skeleton, fps=model.fps, standard_tpose=True)
        print("saved", path, flush=True)
