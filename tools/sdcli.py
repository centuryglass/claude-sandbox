#!/usr/bin/env python3
"""sdcli — an exploration cockpit for the lumin x stable-diffusion-webui pairing.

A single dependency-light (requests only) CLI wrapping the A1111 REST API,
built to iterate fast from the shell. Request-body field names follow
IntraPaint's DiffusionRequestBody (which documents the payload better than
the WebUI Swagger page); ControlNet uses the standard sd-webui-controlnet
alwayson_scripts arg format.

Subcommands:
  txt2img     text -> image
  img2img     image -> image (denoise refine / restyle)
  control     image -> image conditioned on a ControlNet (depth/canny/normal/...)
  upscale     ESRGAN/DAT/etc. single-image upscale (extras endpoint)
  interrogate CLIP/deepbooru caption of an image
  models      list checkpoints / samplers / controlnet models / upscalers
  use-model   switch the active checkpoint (override via options)

Every generating subcommand accepts --out and prints the resolved seed/model.
Run `python3 sdcli.py <cmd> -h` for per-command flags.
"""
import argparse
import base64
import io
import json
import sys
import time
from typing import Any, Optional

import requests

BASE_URL = "http://localhost:7860"


# ------------------------------------------------------------------ helpers
def _b64_of(path: str) -> str:
    with open(path, "rb") as f:
        return base64.b64encode(f.read()).decode()


def _data_uri(path: str) -> str:
    # reForge's ControlNet image decode expects the data: prefix.
    return "data:image/png;base64," + _b64_of(path)


def _save_images(resp: dict[str, Any], out: str) -> list[str]:
    """Write every returned image. `out` may be a template with {i}."""
    paths = []
    imgs = resp.get("images", [])
    for i, b64 in enumerate(imgs):
        if "," in b64[:32]:
            b64 = b64.split(",", 1)[1]
        p = out.format(i=i) if "{i}" in out else (
            out if len(imgs) == 1 else out.rsplit(".", 1)[0] + f"_{i}." + out.rsplit(".", 1)[1])
        with open(p, "wb") as f:
            f.write(base64.b64decode(b64))
        paths.append(p)
    return paths


def _info(resp: dict[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(resp.get("info", "{}"))
    except (json.JSONDecodeError, TypeError):
        return {}


def _post(endpoint: str, body: dict[str, Any], timeout: int = 900) -> dict[str, Any]:
    r = requests.post(BASE_URL + endpoint, json=body, timeout=timeout)
    if not r.ok:
        sys.stderr.write(f"HTTP {r.status_code}: {r.text[:500]}\n")
        r.raise_for_status()
    return r.json()


def _get(endpoint: str, timeout: int = 60) -> Any:
    r = requests.get(BASE_URL + endpoint, timeout=timeout)
    r.raise_for_status()
    return r.json()


def _controlnet_arg(image_path: str, module: str, model: str, weight: float,
                    guidance_start: float, guidance_end: float,
                    control_mode: str, pixel_perfect: bool,
                    processor_res: int, thr_a: Optional[float],
                    thr_b: Optional[float]) -> dict[str, Any]:
    """A reForge ControlNet unit. Use module='None' to feed a raw pre-computed
    control map (e.g. lumin's --aov output) with no preprocessor.

    reForge gotchas that differ from stock A1111 sd-webui-controlnet, learned
    the hard way (a partial/miskeyed unit is silently dropped with NO error to
    the client, leaving the image unchanged):
      * The full field set below is required, incl. use_preview_as_input and
        low_vram. Omitting them drops the unit.
      * `image` needs the data:image/png;base64, prefix.
      * `control_mode` is the enum *value*: 'Balanced' | 'Prefer Prompt' |
        'Prefer ControlNet'.
      * `module` 'None' (capital N) means "no preprocessor".
      * The alwayson key must be 'controlNet' (camelCase) - see cmd_control.
    """
    arg: dict[str, Any] = {
        "use_preview_as_input": False,
        "enabled": True,
        "pixel_perfect": pixel_perfect,
        "low_vram": False,
        "module": module,
        "model": model,
        "weight": weight,
        "image": _data_uri(image_path),
        "control_mode": control_mode,
        "resize_mode": "Crop and Resize",
        "guidance_start": guidance_start,
        "guidance_end": guidance_end,
        "processor_res": processor_res,
    }
    if thr_a is not None:
        arg["threshold_a"] = thr_a
    if thr_b is not None:
        arg["threshold_b"] = thr_b
    return arg


def _report(tag: str, resp: dict[str, Any], paths: list[str], dt: float) -> None:
    info = _info(resp)
    print(f"[{tag}] -> {', '.join(paths)}")
    print(f"[{tag}]    {dt:.1f}s  model={info.get('sd_model_name','?')}  "
          f"seed={info.get('seed','?')}  sampler={info.get('sampler_name','?')}  "
          f"size={info.get('width','?')}x{info.get('height','?')}")


# ------------------------------------------------------------------ commands
def cmd_txt2img(a: argparse.Namespace) -> None:
    body = {
        "prompt": a.prompt, "negative_prompt": a.negative,
        "sampler_name": a.sampler, "steps": a.steps, "cfg_scale": a.cfg,
        "width": a.width, "height": a.height, "seed": a.seed,
        "batch_size": a.batch, "n_iter": a.n_iter,
        "tiling": a.tiling, "restore_faces": a.restore_faces,
        "send_images": True, "save_images": False,
    }
    if a.model:
        body["override_settings"] = {"sd_model_checkpoint": a.model}
        body["override_settings_restore_afterwards"] = False
    t = time.time(); resp = _post("/sdapi/v1/txt2img", body)
    _report("txt2img", resp, _save_images(resp, a.out), time.time() - t)


def cmd_img2img(a: argparse.Namespace) -> None:
    body = {
        "init_images": [_b64_of(a.image)],
        "prompt": a.prompt, "negative_prompt": a.negative,
        "denoising_strength": a.denoise,
        "sampler_name": a.sampler, "steps": a.steps, "cfg_scale": a.cfg,
        "seed": a.seed, "batch_size": a.batch, "n_iter": a.n_iter,
        "resize_mode": a.resize_mode,
        "send_images": True, "save_images": False,
    }
    if a.width:
        body["width"] = a.width
    if a.height:
        body["height"] = a.height
    if a.model:
        body["override_settings"] = {"sd_model_checkpoint": a.model}
        body["override_settings_restore_afterwards"] = False
    t = time.time(); resp = _post("/sdapi/v1/img2img", body)
    _report("img2img", resp, _save_images(resp, a.out), time.time() - t)


def cmd_control(a: argparse.Namespace) -> None:
    cn = _controlnet_arg(
        a.control_image or a.image or a.init, a.module, a.model_cn, a.weight,
        a.guidance_start, a.guidance_end, a.control_mode, not a.no_pixel_perfect,
        a.processor_res, a.threshold_a, a.threshold_b)
    body: dict[str, Any] = {
        "prompt": a.prompt, "negative_prompt": a.negative,
        "sampler_name": a.sampler, "steps": a.steps, "cfg_scale": a.cfg,
        "width": a.width, "height": a.height, "seed": a.seed,
        "subseed": "-1", "subseed_strength": 0.0,
        "batch_size": a.batch, "n_iter": a.n_iter,
        "send_images": True, "save_images": False,
        # NOTE: reForge wants the camelCase key 'controlNet' (a lowercase
        # 'controlnet' key still resolves the script but the args never land,
        # so the unit is silently ignored).
        "alwayson_scripts": {"controlNet": {"args": [cn]}},
    }
    # txt2img+controlnet unless an init image is supplied (then img2img+controlnet)
    endpoint = "/sdapi/v1/txt2img"
    if a.init:
        body["init_images"] = [_b64_of(a.init)]
        body["denoising_strength"] = a.denoise
        endpoint = "/sdapi/v1/img2img"
    if a.model:
        body["override_settings"] = {"sd_model_checkpoint": a.model}
        body["override_settings_restore_afterwards"] = False
    t = time.time(); resp = _post(endpoint, body)
    _report(f"control:{a.module}", resp, _save_images(resp, a.out), time.time() - t)


def cmd_upscale(a: argparse.Namespace) -> None:
    body = {
        "image": _b64_of(a.image),
        "upscaling_resize": a.scale,
        "upscaler_1": a.upscaler,
        "resize_mode": 0,
    }
    t = time.time(); resp = _post("/sdapi/v1/extra-single-image", body)
    b64 = resp["image"]
    if "," in b64[:32]:
        b64 = b64.split(",", 1)[1]
    with open(a.out, "wb") as f:
        f.write(base64.b64decode(b64))
    print(f"[upscale] -> {a.out}  ({time.time()-t:.1f}s, {a.upscaler} x{a.scale})")


def cmd_interrogate(a: argparse.Namespace) -> None:
    resp = _post("/sdapi/v1/interrogate",
                 {"image": _b64_of(a.image), "model": a.model_caption}, timeout=120)
    print(resp.get("caption", resp))


def cmd_models(a: argparse.Namespace) -> None:
    print("== checkpoints ==")
    for m in _get("/sdapi/v1/sd-models"):
        print(" ", m["model_name"], "  (title:", m["title"] + ")")
    print("\n== samplers ==")
    print("  " + ", ".join(m["name"] for m in _get("/sdapi/v1/samplers")))
    print("\n== upscalers ==")
    print("  " + ", ".join(m["name"] for m in _get("/sdapi/v1/upscalers")))
    try:
        cn = _get("/controlnet/model_list").get("model_list", [])
        print("\n== controlnet models ==")
        for m in cn:
            print(" ", m)
        print("\n== controlnet modules (preprocessors) ==")
        print("  " + ", ".join(_get("/controlnet/module_list").get("module_list", [])))
    except Exception as e:  # noqa: BLE001
        print("controlnet not available:", e)


def cmd_use_model(a: argparse.Namespace) -> None:
    _post("/sdapi/v1/options", {"sd_model_checkpoint": a.name}, timeout=180)
    print("active checkpoint ->", _get("/sdapi/v1/options")["sd_model_checkpoint"])


# ------------------------------------------------------------------ parser
def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    def add_common_gen(sp):
        sp.add_argument("-p", "--prompt", default="")
        sp.add_argument("-n", "--negative", default="")
        sp.add_argument("--sampler", default="DPM++ 2M")
        sp.add_argument("--steps", type=int, default=30)
        sp.add_argument("--cfg", type=float, default=7.0)
        sp.add_argument("--seed", type=int, default=-1)
        sp.add_argument("--batch", type=int, default=1)
        sp.add_argument("--n-iter", type=int, default=1)
        sp.add_argument("--model", default=None, help="override checkpoint for this call")
        sp.add_argument("-o", "--out", default="out.png", help="output path ({i} for batches)")

    sp = sub.add_parser("txt2img"); add_common_gen(sp)
    sp.add_argument("--width", type=int, default=512); sp.add_argument("--height", type=int, default=512)
    sp.add_argument("--tiling", action="store_true"); sp.add_argument("--restore-faces", action="store_true")
    sp.set_defaults(func=cmd_txt2img)

    sp = sub.add_parser("img2img"); add_common_gen(sp)
    sp.add_argument("-i", "--image", required=True)
    sp.add_argument("--denoise", type=float, default=0.5)
    sp.add_argument("--width", type=int, default=0); sp.add_argument("--height", type=int, default=0)
    sp.add_argument("--resize-mode", type=int, default=1)
    sp.set_defaults(func=cmd_img2img)

    sp = sub.add_parser("control"); add_common_gen(sp)
    sp.add_argument("--control-image", "--image", dest="control_image", help="the control/condition image")
    sp.add_argument("--init", default=None, help="optional init image => img2img+controlnet")
    sp.add_argument("--denoise", type=float, default=0.6)
    sp.add_argument("--module", default="depth", help="preprocessor, or 'None' for a raw control map (e.g. lumin --aov)")
    sp.add_argument("--model-cn", default="control_v11f1p_sd15_depth [cfd03158]", help="controlnet model")
    sp.add_argument("--weight", type=float, default=1.0)
    sp.add_argument("--guidance-start", type=float, default=0.0)
    sp.add_argument("--guidance-end", type=float, default=1.0)
    sp.add_argument("--control-mode", default="Balanced",
                    help="reForge ControlMode value: 'Balanced' | 'Prefer Prompt' | 'Prefer ControlNet'")
    sp.add_argument("--processor-res", type=int, default=512)
    sp.add_argument("--threshold-a", type=float, default=None)
    sp.add_argument("--threshold-b", type=float, default=None)
    sp.add_argument("--no-pixel-perfect", action="store_true")
    sp.add_argument("--width", type=int, default=512); sp.add_argument("--height", type=int, default=512)
    sp.add_argument("image", nargs="?", help="positional control image (alias)")
    sp.set_defaults(func=cmd_control)

    sp = sub.add_parser("upscale")
    sp.add_argument("-i", "--image", required=True)
    sp.add_argument("--scale", type=float, default=2.0)
    sp.add_argument("--upscaler", default="R-ESRGAN 4x+")
    sp.add_argument("-o", "--out", default="upscaled.png")
    sp.set_defaults(func=cmd_upscale)

    sp = sub.add_parser("interrogate")
    sp.add_argument("-i", "--image", required=True)
    sp.add_argument("--model-caption", default="clip")
    sp.set_defaults(func=cmd_interrogate)

    sp = sub.add_parser("models"); sp.set_defaults(func=cmd_models)

    sp = sub.add_parser("use-model")
    sp.add_argument("name")
    sp.set_defaults(func=cmd_use_model)
    return p


def main() -> None:
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
