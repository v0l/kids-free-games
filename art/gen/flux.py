#!/usr/bin/env python3
import json
import random
import sys
import time
import urllib.request
import uuid

HOST = "http://127.0.0.1:8188"


def workflow(prompt, w, h, seed, steps=22, guidance=3.5):
    return {
        "1": {"class_type": "UNETLoader", "inputs": {"unet_name": "flux1-dev.safetensors", "weight_dtype": "fp8_e4m3fn"}},
        "2": {"class_type": "DualCLIPLoader", "inputs": {"clip_name1": "t5xxl_fp8_e4m3fn.safetensors", "clip_name2": "clip_l.safetensors", "type": "flux"}},
        "3": {"class_type": "VAELoader", "inputs": {"vae_name": "flux_ae.safetensors"}},
        "4": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["2", 0]}},
        "5": {"class_type": "FluxGuidance", "inputs": {"conditioning": ["4", 0], "guidance": guidance}},
        "6": {"class_type": "EmptySD3LatentImage", "inputs": {"width": w, "height": h, "batch_size": 1}},
        "7": {"class_type": "KSampler", "inputs": {"model": ["1", 0], "seed": seed, "steps": steps, "cfg": 1.0, "sampler_name": "euler", "scheduler": "simple", "positive": ["5", 0], "negative": ["5", 0], "latent_image": ["6", 0], "denoise": 1.0}},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["7", 0], "vae": ["3", 0]}},
        "9": {"class_type": "SaveImage", "inputs": {"images": ["8", 0], "filename_prefix": "kg"}},
    }


def post(path, data):
    req = urllib.request.Request(HOST + path, data=json.dumps(data).encode(), headers={"Content-Type": "application/json"})
    return json.loads(urllib.request.urlopen(req).read())


def get(path):
    return urllib.request.urlopen(HOST + path).read()


def generate(prompt, w, h, seed, out):
    pid = post("/prompt", {"prompt": workflow(prompt, w, h, seed), "client_id": str(uuid.uuid4())})["prompt_id"]
    while True:
        hist = json.loads(get(f"/history/{pid}"))
        if pid in hist:
            break
        time.sleep(1)
    img = hist[pid]["outputs"]["9"]["images"][0]
    data = get(f"/view?filename={img['filename']}&subfolder={img['subfolder']}&type={img['type']}")
    open(out, "wb").write(data)


if __name__ == "__main__":
    jobs = json.load(open(sys.argv[1]))
    for j in jobs:
        t = time.time()
        generate(j["prompt"], j.get("w", 1024), j.get("h", 1024), j.get("seed", random.randint(0, 2**31)), j["out"])
        print(f"{j['out']} {time.time() - t:.0f}s", flush=True)
