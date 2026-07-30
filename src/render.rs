use crate::material::Material;
use crate::ray::Ray;
use crate::scene::Scene;
use crate::vec3::Color;

const SHADOW_EPS: f64 = 1e-3;

pub fn ray_color(ray: &Ray, scene: &Scene, depth: u32) -> Color {
    if depth == 0 {
        return Color::ZERO;
    }

    let Some(rec) = scene.hit(ray, SHADOW_EPS, f64::INFINITY) else {
        return scene.background(ray);
    };

    match rec.material {
        Material::Lambertian { albedo } => {
            let mut color = scene.ambient * albedo;
            for light in &scene.lights {
                let sample = light.sample(rec.p);
                let shadow_ray = Ray::new(rec.p, sample.direction);
                let in_shadow = scene
                    .hit(&shadow_ray, SHADOW_EPS, sample.distance - SHADOW_EPS)
                    .is_some();
                if !in_shadow {
                    let diffuse = rec.normal.dot(sample.direction).max(0.0);
                    color += albedo * sample.color * diffuse;
                }
            }
            color
        }
    }
}
