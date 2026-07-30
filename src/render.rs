use crate::glitch::GlitchMode;
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
                    let ray = scene.camera.get_ray(s, t);
                    let ctx = RayContext { primary_dir: ray.direction.normalized(), glitch };
                    color += ray_color(&ray, scene, max_depth, &mut rng, &ctx, None);
                }
                color = color / samples_per_pixel as f64;
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
        Material::Lambertian { albedo } => {
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

            let bounced = ray_color(&Ray::new(rec.p, direction), scene, depth - 1, rng, ctx, next_frozen);
            if ctx.glitch == GlitchMode::EnergyBleed {
                Color::splat(0.05) + bounced
            } else {
                bounced
            }
        }
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
