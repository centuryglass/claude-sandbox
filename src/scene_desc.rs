//! On-disk scene description (RON format) and its conversion into a runtime
//! [`Scene`]. Kept separate from `Scene` itself because `Scene` holds live
//! `Box<dyn Hittable>` trait objects (meshes already loaded and
//! BVH-accelerated), which can't be deserialized directly.
use crate::camera::Camera;
use crate::hittable::Hittable;
use crate::light::Light;
use crate::material::Material;
use crate::mesh;
use crate::scene::Scene;
use crate::sphere::Sphere;
use crate::vec3::Vec3;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

type Vec3f = (f64, f64, f64);

fn v(t: Vec3f) -> Vec3 {
    Vec3::new(t.0, t.1, t.2)
}

#[derive(Deserialize)]
pub struct SceneDesc {
    pub camera: CameraDesc,
    #[serde(default = "default_ambient")]
    pub ambient: Vec3f,
    #[serde(default = "default_sky_bottom")]
    pub sky_bottom: Vec3f,
    #[serde(default = "default_sky_top")]
    pub sky_top: Vec3f,
    #[serde(default)]
    pub lights: Vec<LightDesc>,
    pub objects: Vec<ObjectDesc>,
}

fn default_ambient() -> Vec3f {
    (0.08, 0.08, 0.08)
}
fn default_sky_bottom() -> Vec3f {
    (1.0, 1.0, 1.0)
}
fn default_sky_top() -> Vec3f {
    (0.5, 0.7, 1.0)
}

#[derive(Deserialize)]
pub struct CameraDesc {
    pub look_from: Vec3f,
    pub look_at: Vec3f,
    #[serde(default = "default_vup")]
    pub vup: Vec3f,
    pub vfov: f64,
}

fn default_vup() -> Vec3f {
    (0.0, 1.0, 0.0)
}

#[derive(Deserialize)]
pub enum LightDesc {
    Point { position: Vec3f, color: Vec3f, intensity: f64 },
}

#[derive(Deserialize, Clone, Copy)]
pub enum MaterialDesc {
    Lambertian { albedo: Vec3f },
    Metal { albedo: Vec3f, fuzz: f64 },
    Dielectric { ior: f64 },
}

impl From<MaterialDesc> for Material {
    fn from(m: MaterialDesc) -> Material {
        match m {
            MaterialDesc::Lambertian { albedo } => Material::Lambertian { albedo: v(albedo) },
            MaterialDesc::Metal { albedo, fuzz } => Material::Metal { albedo: v(albedo), fuzz },
            MaterialDesc::Dielectric { ior } => Material::Dielectric { ior },
        }
    }
}

#[derive(Deserialize)]
pub enum ObjectDesc {
    Sphere { center: Vec3f, radius: f64, material: MaterialDesc },
    Mesh { path: String, material: MaterialDesc, #[serde(default = "one")] scale: f64, #[serde(default)] translate: Vec3f },
}

fn one() -> f64 {
    1.0
}

impl SceneDesc {
    pub fn load(path: impl AsRef<Path>) -> Result<SceneDesc> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).with_context(|| format!("reading scene file {}", path.display()))?;
        ron::from_str(&text).with_context(|| format!("parsing scene file {}", path.display()))
    }

    pub fn build(self, aspect_ratio: f64) -> Result<Scene> {
        let mut objects: Vec<Box<dyn Hittable>> = Vec::new();
        for obj in self.objects {
            match obj {
                ObjectDesc::Sphere { center, radius, material } => {
                    objects.push(Box::new(Sphere::new(v(center), radius, material.into())));
                }
                ObjectDesc::Mesh { path, material, scale, translate } => {
                    objects.push(mesh::load_obj(&path, material.into(), scale, v(translate))?);
                }
            }
        }
        anyhow::ensure!(!objects.is_empty(), "scene has no objects");

        let lights = self
            .lights
            .into_iter()
            .map(|l| match l {
                LightDesc::Point { position, color, intensity } => Light::Point { position: v(position), color: v(color), intensity },
            })
            .collect();

        let camera = Camera::new(v(self.camera.look_from), v(self.camera.look_at), v(self.camera.vup), self.camera.vfov, aspect_ratio);

        Ok(Scene {
            objects,
            lights,
            ambient: v(self.ambient),
            camera,
            sky_bottom: v(self.sky_bottom),
            sky_top: v(self.sky_top),
        })
    }
}
