# lumin

A ray tracer written from scratch in Rust, plus a gallery of renderers that
deliberately get the physics *wrong* on purpose.

The honest half is a fairly standard Whitted-style ray tracer: spheres and
triangle meshes, a hand-rolled OBJ loader, a BVH for acceleration, point
lights with shadow rays, Lambertian/Metal/Dielectric materials (mirror
reflection, glass refraction with Schlick's Fresnel approximation),
antialiasing, and a multithreaded render loop.

<p align="center">
  <img src="gallery/reference.png" width="600" alt="Physically-correct reference render: two faceted glass crystals and a mirror sphere">
</p>

## The glitch gallery

The more interesting half. Years ago, a subtle bug in a reflection shader
produced beautiful kaleidoscope-fold artifacts in scenes with a few bounces
of light. This project chases that effect (and others like it) by
implementing a handful of plausible one-line shader bugs as selectable
render modes — see `src/glitch.rs` for the full writeup of each one.

The closest match turned out to be the most literal reading of
"kaleidoscope": `angular-fold` mirrors the reflected/refracted direction's
azimuthal angle into repeating wedges around the vertical axis — the actual
optical principle behind a toy kaleidoscope's ring of mirrors, just applied
to a ray direction instead of light in a tube:

<p align="center">
  <img src="gallery/angular_fold_spheres.png" width="400"><img src="gallery/angular_fold.png" width="400">
</p>

A few of the other deliberately-wrong reflection axes/formulas:

| | |
|---|---|
| ![axis-swap-reflect](gallery/axis_swap_reflect.png) `axis-swap-reflect` — cyclically permute the reflected vector's (x,y,z) before tracing the bounce. Reads as mottled, fragmented faceting. | ![normal-drift](gallery/normal_drift.png) `normal-drift` — corrupt the shading normal's *length* as a smooth function of position, simulating a forgot-to-renormalize bug. Surfaces ripple and marble instead of folding. |
| ![swapped-eta](gallery/swapped_eta.png) `swapped-eta` — refraction ratio computed with the front/back test flipped. Solid objects read as turned inside-out, frosted glass. | ![hall of mirrors, axis-swap-reflect](gallery/hall_axis_swap_reflect.png) `axis-swap-reflect` down an actual mirrored corridor (`scenes/` doesn't ship this one — see `hall_of_mirrors_scene` in `src/main.rs`). Hard banded tiling. |

Other modes (`kaleidoscope-stale-direction`, `kaleidoscope-stale-normal`,
`flipped-reflect-sign`, `inverted-fresnel`, `energy-bleed`) are documented in
`src/glitch.rs` alongside the specific bug each one simulates.

It even holds up on a real mesh - the Stanford bunny (69,451 triangles)
under `normal-drift`:

<p align="center">
  <img src="gallery/bunny.png" width="440" alt="Stanford bunny, correctly rendered"><img src="gallery/bunny_normal_drift.png" width="440" alt="Stanford bunny under the normal-drift glitch">
</p>

## Usage

```sh
# Build
cargo build --release

# No arguments: renders the full built-in demo (baseline scenes + every
# glitch mode) into renders/
cargo run --release

# Render a scene file
cargo run --release -- scenes/reflection_demo.ron -o renders/out.png

# ...with a specific glitch mode
cargo run --release -- scenes/crystal_showcase.ron --glitch axis-swap-reflect -o renders/out.png

# ...or every glitch mode at once, one file per mode
cargo run --release -- scenes/crystal_showcase.ron --gallery -o renders/gallery/

# Options
cargo run --release -- --help
```

Key flags: `--width`/`--height`/`--aspect`, `--samples` (antialiasing /
glass noise quality), `--depth` (max bounce count), `--glitch <mode>`.

## Scene format

Scenes are [RON](https://github.com/ron-rs/ron) files - see
`scenes/*.ron` for complete examples, or `src/scene_desc.rs` for the format
definition. Shape:

```ron
(
    camera: (look_from: (0.0, 0.8, 2.5), look_at: (0.0, 0.0, -1.0), vfov: 45.0),
    sky_bottom: (1.0, 1.0, 1.0),   // optional, defaults to a pale blue sky
    sky_top: (0.5, 0.7, 1.0),      // optional
    lights: [
        Point(position: (-4.0, 5.0, 2.0), color: (1.0, 1.0, 1.0), intensity: 40.0),
    ],
    objects: [
        Sphere(center: (0.0, -100.5, -1.0), radius: 100.0, material: Lambertian(albedo: (0.6, 0.6, 0.65))),
        Sphere(center: (0.0, 0.0, -1.2), radius: 0.5, material: Metal(albedo: (0.85, 0.85, 0.9), fuzz: 0.02)),
        Sphere(center: (1.1, 0.0, -1.6), radius: 0.5, material: Dielectric(ior: 1.5)),
        Mesh(path: "assets/crystal.obj", material: Dielectric(ior: 1.8), scale: 1.0, translate: (0.0, 0.0, -1.0)),
    ],
)
```

## Project layout

- `src/vec3.rs`, `src/ray.rs` - core math
- `src/camera.rs`, `src/light.rs`, `src/material.rs`, `src/sphere.rs`, `src/triangle.rs` - scene primitives
- `src/hittable.rs`, `src/aabb.rs`, `src/bvh.rs` - intersection + acceleration structure
- `src/mesh.rs` - hand-rolled OBJ loader
- `src/scene.rs`, `src/scene_desc.rs` - runtime scene representation and its RON (de)serialization
- `src/render.rs` - the shader: recursive ray-color evaluation, antialiasing, multithreading (rayon)
- `src/glitch.rs` - the glitch gallery: every deliberate physical-inaccuracy mode, with rationale
- `examples/gen_crystal.rs` - procedurally generates the faceted "gem" test meshes under `assets/`
- `scenes/*.ron` - example scene files
- `assets/` - OBJ test meshes (procedural crystals, Stanford bunny)

## Tests

```sh
cargo test --release
```
