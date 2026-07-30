#!/usr/bin/env python3
"""Drive a normal-AOV turntable through reForge ControlNet -> a stylized GIF.

Each lumin normal-map frame is fed RAW (module='None') into the normalbae
ControlNet with a locked seed + prompt, so the geometry stays pinned while
diffusion re-skins it. Temporal coherence comes from the fixed seed, the
frame-to-frame continuity of the control signal, and the LCM low-step recipe.

IMPORTANT reForge/Forge API notes (differ from stock A1111 sd-webui-controlnet):
  * alwayson key is 'controlNet' (camelCase).
  * the unit MUST carry the full field set incl. use_preview_as_input,
    low_vram, and the request needs subseed/subseed_strength - a partial unit
    is silently dropped (image unaffected, no error surfaced to the client).
  * image strings need the 'data:image/png;base64,' prefix.
  * control_mode values are the enum *values*: 'Balanced' | 'Prefer Prompt'
    | 'Prefer ControlNet'.
  * module 'None' (capital N) means "use my raw map, no preprocessor".
"""
import base64
import glob
import os
import sys
import time

import requests
from PIL import Image

BASE = "http://localhost:7860"
FRAMES_DIR = sys.argv[1]
OUT_DIR = sys.argv[2]
SIZE = int(sys.argv[3]) if len(sys.argv) > 3 else 576
PROMPT = ("a polished translucent jade rabbit figurine on a dark reflective "
          "surface, carved emerald gemstone, subsurface scattering, soft "
          "studio product lighting, highly detailed, sharp focus "
          "<lora:lcm-lora-sdv1-5:1>")
NEG = "cartoon, drawing, blurry, lowres, deformed, watermark, text, extra limbs"
SEED = 555
MODEL_CN = "control_v11p_sd15_normalbae [316696f1]"

os.makedirs(OUT_DIR, exist_ok=True)


def data_uri(path):
    with open(path, "rb") as f:
        return "data:image/png;base64," + base64.b64encode(f.read()).decode()


def stylize(frame_path):
    body = {
        "sampler_name": "LCM", "batch_size": 1, "n_iter": 1,
        "steps": 8, "cfg_scale": 1.5, "width": SIZE, "height": SIZE,
        "prompt": PROMPT, "negative_prompt": NEG,
        "seed": SEED, "subseed": "-1", "subseed_strength": 0.0,
        "restore_faces": False, "tiling": False,
        "send_images": True, "save_images": False,
        "alwayson_scripts": {"controlNet": {"args": [{
            "use_preview_as_input": False, "enabled": True,
            "pixel_perfect": True, "low_vram": False,
            "module": "None", "model": MODEL_CN, "weight": 1.0,
            "image": data_uri(frame_path),
            "control_mode": "Prefer ControlNet",
            "resize_mode": "Crop and Resize",
            "guidance_start": 0.0, "guidance_end": 1.0,
            "processor_res": SIZE,
        }]}},
    }
    r = requests.post(BASE + "/sdapi/v1/txt2img", json=body, timeout=300)
    r.raise_for_status()
    b = r.json()["images"][0]
    if "," in b[:32]:
        b = b.split(",", 1)[1]
    return base64.b64decode(b)


frames = sorted(glob.glob(os.path.join(FRAMES_DIR, "frame_*.png")))
print(f"{len(frames)} frames -> stylizing")
out_paths = []
for i, fp in enumerate(frames):
    t = time.time()
    png = stylize(fp)
    op = os.path.join(OUT_DIR, f"styl_{i:03}.png")
    with open(op, "wb") as f:
        f.write(png)
    out_paths.append(op)
    print(f"  frame {i+1}/{len(frames)}  ({time.time()-t:.1f}s)")

imgs = [Image.open(p).convert("P", palette=Image.ADAPTIVE) for p in out_paths]
gif_path = os.path.join(OUT_DIR, "bunny_jade_turntable.gif")
imgs[0].save(gif_path, save_all=True, append_images=imgs[1:],
             duration=1000 // 15, loop=0, optimize=True)
print("wrote", gif_path)
