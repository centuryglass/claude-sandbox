use crate::material::Material;
use crate::ray::Ray;
use crate::scene::Scene;
use crate::vec3::{Color, Vec3};
use image::{ImageBuffer, Rgb, RgbImage};
use rand::Rng;
use rayon::prelude::*;

const SHADOW_EPS: f64 = 1e-3;
/// Blinn-Phong specular exponent for the highlight on diffuse surfaces.
const SHININESS: f64 = 32.0;

pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u32,
    pub max_depth: u32,
}

fn to_rgb8(c: Color) -> Rgb<u8> {
    // Gamma-correct (gamma 2.0, i.e. sqrt) before quantizing to 8-bit.
    let clamp = |v: f64| (v.max(0.0).sqrt().min(1.0) * 255.999) as u8;
    Rgb([clamp(c.x), clamp(c.y), clamp(c.z)])
}

/// Renders the scene, one worker thread per row via rayon, and returns the
/// finished image. Each row gets its own RNG so threads never contend.
pub fn render(scene: &Scene, settings: &RenderSettings) -> RgbImage {
    let RenderSettings { width, height, samples_per_pixel, max_depth } = *settings;
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
                    color += ray_color(&ray, scene, max_depth, &mut rng);
                }
                color = color / samples_per_pixel as f64;
                let pixel = to_rgb8(color);
                let idx = (x * 3) as usize;
                row[idx..idx + 3].copy_from_slice(&pixel.0);
            }
        });

    ImageBuffer::from_raw(width, height, buffer).expect("buffer sized to width*height*3")
}

pub fn ray_color(ray: &Ray, scene: &Scene, depth: u32, rng: &mut impl Rng) -> Color {
    if depth == 0 {
        return Color::ZERO;
    }

    let Some(rec) = scene.hit(ray, SHADOW_EPS, f64::INFINITY) else {
        return scene.background(ray);
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
            let reflected =
                ray.direction.normalized().reflect(rec.normal) + fuzz * Vec3::random_in_unit_sphere(rng);
            if reflected.dot(rec.normal) <= 0.0 {
                // Fuzz pushed the reflection below the surface; absorb.
                return Color::ZERO;
            }
            let bounced = ray_color(&Ray::new(rec.p, reflected), scene, depth - 1, rng);
            albedo * bounced
        }

        Material::Dielectric { ior } => {
            let refraction_ratio = if rec.front_face { 1.0 / ior } else { ior };
            let unit_dir = ray.direction.normalized();

            let cos_theta = (-unit_dir).dot(rec.normal).min(1.0);
            let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
            let cannot_refract = refraction_ratio * sin_theta > 1.0;

            let reflectance = schlick_reflectance(cos_theta, refraction_ratio);
            let direction = if cannot_refract || reflectance > rng.gen_range(0.0..1.0) {
                unit_dir.reflect(rec.normal)
            } else {
                unit_dir.refract(rec.normal, refraction_ratio)
            };

            ray_color(&Ray::new(rec.p, direction), scene, depth - 1, rng)
        }
    }
}

/// Schlick's approximation for Fresnel reflectance.
fn schlick_reflectance(cosine: f64, refraction_ratio: f64) -> f64 {
    let r0 = ((1.0 - refraction_ratio) / (1.0 + refraction_ratio)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
}
