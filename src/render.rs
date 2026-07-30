use crate::glitch::GlitchMode;
use crate::hittable::HitRecord;
use crate::material::Material;
use crate::ray::Ray;
use crate::scene::Scene;
use crate::vec3::{Color, Point3, Vec3};
use image::{ImageBuffer, Rgb, RgbImage};
use rand::Rng;
use rayon::prelude::*;

const SHADOW_EPS: f64 = 1e-3;
/// Blinn-Phong specular exponent for the highlight on diffuse surfaces.
const SHININESS: f64 = 32.0;

#[derive(Debug, Clone, Copy)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u32,
    pub max_depth: u32,
    pub glitch: GlitchMode,
}

impl RenderSettings {
    pub fn new(width: u32, height: u32, samples_per_pixel: u32, max_depth: u32) -> Self {
        RenderSettings { width, height, samples_per_pixel, max_depth, glitch: GlitchMode::None }
    }

    pub fn with_glitch(mut self, glitch: GlitchMode) -> Self {
        self.glitch = glitch;
        self
    }
}

fn to_rgb8(c: Color) -> Rgb<u8> {
    // Gamma-correct (gamma 2.0, i.e. sqrt) before quantizing to 8-bit.
    let clamp = |v: f64| (v.max(0.0).sqrt().min(1.0) * 255.999) as u8;
    Rgb([clamp(c.x), clamp(c.y), clamp(c.z)])
}

/// Renders the scene, one worker thread per row via rayon, and returns the
/// finished image. Each row gets its own RNG so threads never contend.
pub fn render(scene: &Scene, settings: &RenderSettings) -> RgbImage {
    let RenderSettings { width, height, samples_per_pixel, max_depth, glitch } = *settings;
    let mut buffer = vec![0u8; (width * height * 3) as usize];

    buffer
        .par_chunks_mut(3 * width as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let mut rng = rand::thread_rng();
            for x in 0..width {
                let mut color = Color::ZERO;
                for _ in 0..samples_per_pixel {
                    let s = (x as f64 + rng.gen_range(0.0..1.0)) / (width - 1) as f64;
                    let t = 1.0 - (y as f64 + rng.gen_range(0.0..1.0)) / (height - 1) as f64;
                    let ray = scene.camera.get_ray(s, t, &mut rng);
                    color += match glitch {
                        GlitchMode::HitChainDrift => hit_chain_drift_color(&ray, scene, &mut rng),
                        GlitchMode::CoinFlipMiss => coin_flip_miss_entry(&ray, scene, max_depth, &mut rng),
                        _ => {
                            let ctx = RayContext { primary_dir: ray.direction.normalized(), glitch };
                            ray_color(&ray, scene, max_depth, &mut rng, &ctx, None)
                        }
                    };
                }
                color /= samples_per_pixel as f64;
                let pixel = to_rgb8(color);
                let idx = (x * 3) as usize;
                row[idx..idx + 3].copy_from_slice(&pixel.0);
            }
        });

    ImageBuffer::from_raw(width, height, buffer).expect("buffer sized to width*height*3")
}

/// Per-primary-ray context threaded through the recursive bounce chain.
/// `primary_dir` only matters for [`GlitchMode::KaleidoscopeStaleDirection`],
/// which (deliberately) reflects it instead of the current bounce direction.
struct RayContext {
    primary_dir: Vec3,
    glitch: GlitchMode,
}

/// `frozen_normal` implements [`GlitchMode::KaleidoscopeStaleNormal`]: once
/// set (at the first bounce), every later bounce reflects/refracts around
/// this same stale axis instead of re-reading the real surface normal, even
/// though ray origins keep coming from genuine geometry hits.
fn ray_color(
    ray: &Ray,
    scene: &Scene,
    depth: u32,
    rng: &mut impl Rng,
    ctx: &RayContext,
    frozen_normal: Option<Vec3>,
) -> Color {
    if depth == 0 {
        return Color::ZERO;
    }

    let Some(mut rec) = scene.hit(ray, SHADOW_EPS, f64::INFINITY) else {
        return scene.background(ray);
    };

    if ctx.glitch == GlitchMode::NormalDrift {
        rec.normal = drift_normal(rec.normal, rec.p);
    }

    let (axis, next_frozen) = match ctx.glitch {
        GlitchMode::KaleidoscopeStaleNormal => {
            let n = frozen_normal.unwrap_or(rec.normal);
            (n, Some(n))
        }
        _ => (rec.normal, None),
    };

    match rec.material {
        Material::Emissive { color, intensity } => color.sample(rec.p, rec.u, rec.v) * intensity,

        Material::Lambertian { albedo } => {
            let albedo = albedo.sample(rec.p, rec.u, rec.v);
            let view_dir = -ray.direction.normalized();
            let mut color = scene.ambient * albedo;
            for light in &scene.lights {
                let sample = light.sample(rec.p);
                let shadow_ray = Ray::new(rec.p, sample.direction);
                let in_shadow = scene
                    .hit(&shadow_ray, SHADOW_EPS, sample.distance - SHADOW_EPS)
                    .is_some();
                if in_shadow {
                    continue;
                }
                let n_dot_l = rec.normal.dot(sample.direction).max(0.0);
                color += albedo * sample.color * n_dot_l;

                // Blinn-Phong specular highlight.
                if n_dot_l > 0.0 {
                    let half_vec = (sample.direction + view_dir).normalized();
                    let spec = rec.normal.dot(half_vec).max(0.0).powf(SHININESS);
                    color += sample.color * spec * 0.25;
                }
            }
            color
        }

        Material::Metal { albedo, fuzz } => {
            let albedo = albedo.sample(rec.p, rec.u, rec.v);
            // The flagship glitch: reflect the ORIGINAL camera ray's
            // direction instead of the direction the current bounce actually
            // arrived along. Every mirror in the scene ends up folding the
            // same primary view around its own normal, and because it
            // compounds bounce over bounce, facets tile into kaleidoscopic
            // mirror copies instead of showing what's really there.
            let incoming = match ctx.glitch {
                GlitchMode::KaleidoscopeStaleDirection => ctx.primary_dir,
                _ => ray.direction.normalized(),
            };
            let mut reflected = reflect_maybe_flipped(incoming, axis, ctx.glitch) + fuzz * Vec3::random_in_unit_sphere(rng);

            if ctx.glitch == GlitchMode::AxisSwapReflect {
                reflected = Vec3::new(reflected.y, reflected.z, reflected.x);
            }
            if ctx.glitch == GlitchMode::AngularFold {
                reflected = angular_fold(reflected);
            }

            if ctx.glitch != GlitchMode::FlippedReflectSign && reflected.dot(rec.normal) <= 0.0 {
                // Fuzz pushed the reflection below the surface; absorb. (Under
                // FlippedReflectSign, going "below" the surface is the point.)
                return Color::ZERO;
            }
            let bounced = ray_color(&Ray::new(rec.p, reflected), scene, depth - 1, rng, ctx, next_frozen);

            if ctx.glitch == GlitchMode::EnergyBleed {
                albedo + bounced
            } else {
                albedo * bounced
            }
        }

        Material::Dielectric { ior } => {
            let entering = rec.front_face;
            let refraction_ratio = match ctx.glitch {
                // Front/back test flipped: rays bend as if crossing the
                // opposite boundary they actually hit.
                GlitchMode::SwappedEta => if entering { ior } else { 1.0 / ior },
                _ => if entering { 1.0 / ior } else { ior },
            };

            let incoming = match ctx.glitch {
                GlitchMode::KaleidoscopeStaleDirection => ctx.primary_dir,
                _ => ray.direction.normalized(),
            };

            let cos_theta = (-incoming).dot(axis).min(1.0);
            let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
            let cannot_refract = refraction_ratio * sin_theta > 1.0;

            let mut reflectance = schlick_reflectance(cos_theta, refraction_ratio);
            if ctx.glitch == GlitchMode::InvertedFresnel {
                reflectance = 1.0 - reflectance;
            }

            let mut direction = if cannot_refract || reflectance > rng.gen_range(0.0..1.0) {
                reflect_maybe_flipped(incoming, axis, ctx.glitch)
            } else {
                incoming.refract(axis, refraction_ratio)
            };

            if ctx.glitch == GlitchMode::AxisSwapReflect {
                direction = Vec3::new(direction.y, direction.z, direction.x);
            }
            if ctx.glitch == GlitchMode::AngularFold {
                direction = angular_fold(direction);
            }

            let bounced = ray_color(&Ray::new(rec.p, direction), scene, depth - 1, rng, ctx, next_frozen);
            if ctx.glitch == GlitchMode::EnergyBleed {
                Color::splat(0.05) + bounced
            } else {
                bounced
            }
        }
    }
}

/// Shared, monotonically-decreasing reflection budget - mirrors `hit`'s
/// `depth` field in the original C++, which never resets across the whole
/// call tree since it's one struct passed around by reference.
const HIT_CHAIN_INITIAL_DEPTH: u32 = 6;
/// The original shader's `reflectionCount` constructor argument. A plausible
/// default for a scene tuned to look decent without being unusably slow on
/// 2013-era student hardware.
const HIT_CHAIN_REFLECTION_COUNT: u32 = 3;

/// Mutable state standing in for the original's `HitStruct& hit` - shared
/// and progressively overwritten by (nominally) reflection attempts whose
/// real color contribution never survives to be used (see [`GlitchMode::HitChainDrift`]).
struct HitChainState {
    p: Point3,
    normal: Vec3,
    incoming: Vec3,
    material: Material,
    u: f64,
    v: f64,
    depth: u32,
}

fn hit_chain_drift_color(ray: &Ray, scene: &Scene, rng: &mut impl Rng) -> Color {
    let Some(rec) = scene.hit(ray, SHADOW_EPS, f64::INFINITY) else {
        return scene.background(ray);
    };
    let mut state = HitChainState {
        p: rec.p,
        normal: rec.normal,
        incoming: ray.direction.normalized(),
        u: rec.u,
        v: rec.v,
        material: rec.material,
        depth: HIT_CHAIN_INITIAL_DEPTH,
    };
    hit_chain_color(scene, &mut state, rng)
}

fn mirror_coef_for(material: &Material) -> f64 {
    match material {
        Material::Lambertian { .. } => 0.0,
        Material::Metal { .. } => 0.9,
        // The original shader had no refraction at all - "glass" objects
        // were almost certainly just this same diffuse+mirror blend at a
        // high mirror coefficient, which is exactly why the reference
        // crystal render was this bug's showcase piece.
        Material::Dielectric { .. } => 0.85,
        Material::Emissive { .. } => 0.0,
    }
}

fn albedo_for(material: &Material, p: Point3, u: f64, v: f64) -> Color {
    match material {
        Material::Lambertian { albedo } => albedo.sample(p, u, v),
        Material::Metal { albedo, .. } => albedo.sample(p, u, v),
        Material::Dielectric { .. } => Color::new(0.9, 0.85, 0.95),
        Material::Emissive { color, intensity } => color.sample(p, u, v) * *intensity,
    }
}

/// Direct port of `MultiReflectionShader::getHitColor`'s actual behavior
/// (not its apparent intent). `state` plays the role of the shared,
/// by-reference `hit` - every level of recursion mutates it in place, and
/// nothing here resets it back to the original surface point.
// `rng` is only ever forwarded to the recursive call, never sampled here -
// faithful to the original, which had no randomness in this path either.
// Kept in the signature (not `_rng`) for symmetry with the other recursive
// glitch functions and in case a future variant wants to add jitter.
#[allow(clippy::only_used_in_recursion)]
fn hit_chain_color(scene: &Scene, state: &mut HitChainState, rng: &mut impl Rng) -> Color {
    // Not in the original (which had no emissive materials at all), but the
    // sane behavior: a light source shows its own glow, full stop, rather
    // than participating in the drift.
    if let Material::Emissive { color, intensity } = &state.material {
        return color.sample(state.p, state.u, state.v) * *intensity;
    }

    let mirror_coef = mirror_coef_for(&state.material);
    let mut shade = Color::ZERO;

    for light in &scene.lights {
        for _ in 0..HIT_CHAIN_REFLECTION_COUNT {
            let sample = light.sample(state.p);
            let shadow_ray = Ray::new(state.p, sample.direction);
            let in_shadow = scene.hit(&shadow_ray, SHADOW_EPS, sample.distance - SHADOW_EPS).is_some();
            let shadow_mult = if in_shadow { 0.0 } else { 1.0 };

            // Original bug: abs(N.L), not max(0, N.L) - back-facing samples
            // light up too.
            let m = state.normal.dot(sample.direction).abs();
            let albedo = albedo_for(&state.material, state.p, state.u, state.v);
            shade += (albedo * sample.color * m * shadow_mult) / HIT_CHAIN_REFLECTION_COUNT as f64;

            if mirror_coef > 0.0 && state.depth > 0 {
                state.depth -= 1;
                let reflected_dir = state.incoming.reflect(state.normal);
                let ray = Ray::new(state.p, reflected_dir);
                if let Some(rec) = scene.hit(&ray, SHADOW_EPS, f64::INFINITY) {
                    // calculateReflection(hit, scene): mutates the shared
                    // hit state to the new surface, unconditionally.
                    *state = HitChainState { p: rec.p, normal: rec.normal, incoming: reflected_dir, u: rec.u, v: rec.v, material: rec.material, depth: state.depth };
                    // rCol += hit.getHitColor(scene): computed, then thrown
                    // away when the shadowing local `rCol` goes out of
                    // scope. Only `state`'s mutation (including whatever
                    // this recursive call does to it) survives.
                    let _discarded = hit_chain_color(scene, state, rng);
                }
                // Reflected ray missed everything: original falls back to
                // re-adding the same direct-light term into the (still
                // discarded) rCol - a no-op for the final image.
            }
        }
        // Bug: applied once per light rather than once overall, so it
        // compounds multiplicatively across multiple lights.
        shade *= 1.0 - mirror_coef;
    }

    shade
}

/// Entry point for [`GlitchMode::CoinFlipMiss`], modeled on the original
/// source's *other* function, `mapLightRay` (see `reference/`). Unlike
/// `hit-chain-drift` this isn't a literal port - `mapLightRay` looks like
/// the entry point to a forward light-tracing/photon-mapping pass, an
/// architecture this backward ray tracer doesn't have - but its specific
/// bugs translate directly onto a normal recursive bounce.
fn coin_flip_miss_entry(ray: &Ray, scene: &Scene, depth: u32, rng: &mut impl Rng) -> Color {
    match scene.hit(ray, SHADOW_EPS, f64::INFINITY) {
        Some(rec) => coin_flip_miss_color(ray, &rec, scene, Color::ONE, depth, rng),
        None => scene.background(ray),
    }
}

fn clamp01(c: Color) -> Color {
    Color::new(c.x.clamp(0.0, 1.0), c.y.clamp(0.0, 1.0), c.z.clamp(0.0, 1.0))
}

fn coin_flip_miss_color(ray: &Ray, rec: &HitRecord, scene: &Scene, incoming_light: Color, depth: u32, rng: &mut impl Rng) -> Color {
    // Bug: dots the ray's own incident direction against the normal without
    // negating it first. A ray arriving at a front-facing surface has
    // direction roughly opposite the normal, so this is negative and clamps
    // to 0 - ordinary front-lit surfaces go dark; only grazing/back-facing
    // geometry (where this dot product happens to be positive) lights up.
    let m = ray.direction.normalized().dot(rec.normal).max(0.0);
    let albedo = albedo_for(&rec.material, rec.p, rec.u, rec.v);
    let mut shade = albedo * incoming_light * m;

    let mirror_coef = mirror_coef_for(&rec.material);
    if mirror_coef > 0.0 {
        shade = shade * (1.0 - mirror_coef) + incoming_light * mirror_coef;
    }

    // Bug: an unweighted coin flip instead of properly-weighted Russian
    // roulette (which would divide the result by the survival probability
    // to stay unbiased). Some paths stop after one hit, others bounce many
    // times, and both get averaged into the pixel with equal weight -
    // brightness varies noisily sample to sample.
    if depth == 0 || rng.gen_range(0.0..1.0) > 0.5 {
        return clamp01(shade);
    }

    let reflected = ray.direction.normalized().reflect(rec.normal);
    let bounce_ray = Ray::new(rec.p, reflected);
    match scene.hit(&bounce_ray, SHADOW_EPS, f64::INFINITY) {
        Some(next_rec) => coin_flip_miss_color(&bounce_ray, &next_rec, scene, shade, depth - 1, rng),
        // Bug: an unclamped miss sentinel. Unlike the "stop" branch above,
        // this value is returned as-is straight into the caller's `shade *
        // incoming_light` multiplication one level up - a negative feeding
        // into a product can flip the sign of the *next* level's result too,
        // occasionally producing an impossible bright pixel from two
        // negatives multiplying positive.
        None => Color::new(-1.0, -1.0, -1.0),
    }
}

/// Standard reflect, or - under [`GlitchMode::FlippedReflectSign`] - the
/// same formula with its sign flipped (`d + 2(d.n)n`), which sends the
/// "reflected" ray further into the surface instead of away from it.
fn reflect_maybe_flipped(d: Vec3, n: Vec3, glitch: GlitchMode) -> Vec3 {
    if glitch == GlitchMode::FlippedReflectSign {
        d + 2.0 * d.dot(n) * n
    } else {
        d.reflect(n)
    }
}

/// Number of mirror wedges in [`GlitchMode::AngularFold`] - matches a
/// typical toy kaleidoscope's 3-mirror arrangement, doubled by the fold
/// itself to 6 repeats around the circle.
const FOLD_SEGMENTS: f64 = 3.0;

/// Folds `d`'s azimuthal angle around the world up-axis into a single
/// repeating, mirrored wedge - the actual optical principle behind a
/// kaleidoscope, applied to a ray direction instead of a light ray bouncing
/// down a mirrored tube. Preserves the angle from the axis (so it doesn't
/// change how "grazing" the bounce is), only where around the axis it points.
fn angular_fold(d: Vec3) -> Vec3 {
    let axis = Vec3::new(0.0, 1.0, 0.0);
    let cos_theta = d.dot(axis).clamp(-1.0, 1.0);
    let d_perp = d - axis * cos_theta;
    let perp_len = d_perp.length();
    if perp_len < 1e-9 {
        return d; // Aligned with the fold axis; no azimuth to fold.
    }

    let b1 = Vec3::new(1.0, 0.0, 0.0);
    let b1 = (b1 - axis * axis.dot(b1)).normalized();
    let b2 = axis.cross(b1);

    let phi = d_perp.dot(b2).atan2(d_perp.dot(b1));
    let wedge = std::f64::consts::TAU / FOLD_SEGMENTS;
    let mut folded = phi.rem_euclid(wedge);
    if folded > wedge / 2.0 {
        folded = wedge - folded; // Mirror the back half of the wedge.
    }

    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    axis * cos_theta + (b1 * folded.cos() + b2 * folded.sin()) * sin_theta
}

/// Corrupts a unit normal's *length* (not just direction) as a smooth
/// function of world position, simulating a "forgot to renormalize after
/// interpolation/transform" bug. Feeding a non-unit normal into reflect/
/// refract/lighting math distorts angles in a spatially organic way.
fn drift_normal(n: Vec3, p: Point3) -> Vec3 {
    let drift = 1.0 + 0.8 * (p.x * 4.0).sin() * (p.y * 6.0 + p.z * 3.0).cos();
    n * drift
}

/// Schlick's approximation for Fresnel reflectance.
fn schlick_reflectance(cosine: f64, refraction_ratio: f64) -> f64 {
    let r0 = ((1.0 - refraction_ratio) / (1.0 + refraction_ratio)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
}
