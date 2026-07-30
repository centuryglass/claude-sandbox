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
use crate::texture::{ImageTexture, Texture};
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
    /// Projection type. Defaults to `Perspective`, so every scene file
    /// written before this field existed keeps rendering exactly as before -
    /// this is a hard compatibility requirement, not just a nicety.
    #[serde(default)]
    pub kind: CameraKind,
    /// Vertical field of view in degrees: `Perspective`'s classic tan-based
    /// FOV, and also the true angular field of view for `Fisheye` /
    /// `Stereographic` (see `Camera::new_fisheye`). Unused by `Orthographic`
    /// (parallel rays don't converge, so there's no angle to size - see
    /// `height` instead) and by `Equirectangular` (always covers the whole
    /// sphere, so there's nothing to size at all).
    #[serde(default = "default_vfov")]
    pub vfov: f64,
    /// World-space vertical extent of the frame. Only meaningful for
    /// `Orthographic`.
    #[serde(default = "default_ortho_height")]
    pub height: f64,
    /// Depth-of-field lens diameter. Only meaningful for `Perspective`.
    /// Default 0 is a pinhole camera (infinite depth of field) - same as
    /// every scene file written before this field existed.
    #[serde(default)]
    pub aperture: f64,
    /// Distance from the camera to the plane that's in perfect focus. Only
    /// meaningful for `Perspective` with a nonzero `aperture`.
    #[serde(default = "one")]
    pub focus_dist: f64,
}

fn default_vup() -> Vec3f {
    (0.0, 1.0, 0.0)
}

fn default_vfov() -> f64 {
    45.0
}

fn default_ortho_height() -> f64 {
    2.0
}

#[derive(Deserialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum CameraKind {
    #[default]
    Perspective,
    Orthographic,
    Fisheye,
    Stereographic,
    Equirectangular,
}

#[derive(Deserialize)]
pub enum LightDesc {
    Point { position: Vec3f, color: Vec3f, intensity: f64 },
}

#[derive(Deserialize, Clone)]
pub enum TextureDesc {
    Solid(Vec3f),
    Checker { a: Vec3f, b: Vec3f, scale: f64 },
    Stripes { a: Vec3f, b: Vec3f, scale: f64, axis: usize },
    Gradient { bottom: Vec3f, top: Vec3f, y0: f64, y1: f64 },
    Noise { color: Vec3f, scale: f64, octaves: u32 },
    /// Path to an image file (PNG/JPEG/...), sampled by UV coordinates.
    /// Fallible (unlike every other variant here) since it means loading
    /// and decoding a file from disk - hence `TextureDesc -> Texture` being
    /// a `build` method returning `Result`, not a plain infallible `From`.
    Image(String),
}

impl TextureDesc {
    fn build(self) -> Result<Texture> {
        Ok(match self {
            TextureDesc::Solid(c) => Texture::Solid(v(c)),
            TextureDesc::Checker { a, b, scale } => Texture::Checker { a: v(a), b: v(b), scale },
            TextureDesc::Stripes { a, b, scale, axis } => Texture::Stripes { a: v(a), b: v(b), scale, axis },
            TextureDesc::Gradient { bottom, top, y0, y1 } => Texture::Gradient { bottom: v(bottom), top: v(top), y0, y1 },
            TextureDesc::Noise { color, scale, octaves } => Texture::Noise { color: v(color), scale, octaves },
            TextureDesc::Image(path) => Texture::Image(std::sync::Arc::new(ImageTexture::load(&path)?)),
        })
    }
}

#[derive(Deserialize, Clone)]
pub enum MaterialDesc {
    Lambertian { albedo: TextureDesc },
    Metal { albedo: TextureDesc, fuzz: f64 },
    Dielectric { ior: f64 },
    Emissive { color: TextureDesc, intensity: f64 },
}

impl MaterialDesc {
    fn build(self) -> Result<Material> {
        Ok(match self {
            MaterialDesc::Lambertian { albedo } => Material::Lambertian { albedo: albedo.build()? },
            MaterialDesc::Metal { albedo, fuzz } => Material::Metal { albedo: albedo.build()?, fuzz },
            MaterialDesc::Dielectric { ior } => Material::Dielectric { ior },
            MaterialDesc::Emissive { color, intensity } => Material::Emissive { color: color.build()?, intensity },
        })
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
                    objects.push(Box::new(Sphere::new(v(center), radius, material.build()?)));
                }
                ObjectDesc::Mesh { path, material, scale, translate } => {
                    objects.push(mesh::load_obj(&path, material.build()?, scale, v(translate))?);
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

        let (look_from, look_at, vup) = (v(self.camera.look_from), v(self.camera.look_at), v(self.camera.vup));
        let camera = match self.camera.kind {
            CameraKind::Perspective => {
                Camera::new_thin_lens(look_from, look_at, vup, self.camera.vfov, aspect_ratio, self.camera.aperture, self.camera.focus_dist)
            }
            CameraKind::Orthographic => Camera::new_orthographic(look_from, look_at, vup, self.camera.height, aspect_ratio),
            CameraKind::Fisheye => Camera::new_fisheye(look_from, look_at, vup, self.camera.vfov, aspect_ratio),
            CameraKind::Stereographic => Camera::new_stereographic(look_from, look_at, vup, self.camera.vfov, aspect_ratio),
            CameraKind::Equirectangular => Camera::new_equirectangular(look_from, look_at, vup),
        };

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
