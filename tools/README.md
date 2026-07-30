# tools — lumin × diffusion

Helpers for pairing lumin's renderer with a local Stable Diffusion webui
(**reForge** / stable-diffusion-webui-forge). Python, `requests` + `Pillow`
only. Assumes the webui is running at `http://localhost:7860`.

## `sdcli.py` — a diffusion cockpit

```
python3 tools/sdcli.py txt2img  -p "a prompt" -o out.png
python3 tools/sdcli.py img2img  -i in.png --denoise 0.5 -p "..." -o out.png
python3 tools/sdcli.py control  --control-image map.png --module None \
        --model-cn "control_v11f1p_sd15_depth [cfd03158]" \
        --control-mode "Prefer ControlNet" -p "..." -o out.png
python3 tools/sdcli.py upscale  -i in.png --scale 2 -o big.png
python3 tools/sdcli.py models          # list checkpoints / samplers / controlnets
```

The killer pairing: render a geometry buffer from lumin and feed it **raw**
(no preprocessor) into ControlNet, so diffusion re-shades lumin's exact
geometry:

```
cargo run --release -- scenes/reflection_demo.ron --aov depth  -o depth.png
python3 tools/sdcli.py control --control-image depth.png --module None \
        --model-cn "control_v11f1p_sd15_depth [cfd03158]" \
        --control-mode "Prefer ControlNet" -p "glossy ceramic spheres, studio" -o out.png
```

## `turntable_stylize.py` — AOV-conditioned turntables

Renders each frame of a lumin `--animate ... --aov normal` turntable through
ControlNet with a locked seed + LCM, producing a stylized-but-geometrically-
stable animation:

```
cargo run --release -- scenes/bunny.ron --animate 24 --aov normal \
      --width 576 --aspect 1.0 -o bunny_normal.gif      # writes bunny_normal_frames/
python3 tools/turntable_stylize.py bunny_normal_frames out_dir 576
```

## reForge ControlNet API gotchas (why these files exist)

reForge's ControlNet API diverges from stock A1111 `sd-webui-controlnet`, and
a malformed unit is **silently dropped** — the image comes back unchanged with
no error — so these were easy to get wrong. What actually works:

- The alwayson key is **`controlNet`** (camelCase). A lowercase `controlnet`
  key resolves the script but the args never land.
- The unit needs the **full field set**, incl. `use_preview_as_input` and
  `low_vram`; the request needs `subseed`/`subseed_strength`. A partial unit
  is dropped.
- `image` strings need the **`data:image/png;base64,`** prefix.
- `control_mode` is the enum *value*: `Balanced` | `Prefer Prompt` |
  `Prefer ControlNet` (not the A1111 long strings, not `CONTROL`).
- `module` `None` (capital N) = feed my raw map, no preprocessor.

For `img2img` + ControlNet, an LCM setup gives a big consistency win:
`<lora:lcm-lora-sdv1-5:1>` in the prompt, `LCM` sampler, `cfg ≤ 2`, `steps ≤ 8`.
