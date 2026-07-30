use crate::camera::Camera;
use crate::hittable::{HitRecord, Hittable};
use crate::light::Light;
use crate::ray::Ray;
use crate::texture::Texture;
use crate::vec3::Color;

pub struct Scene {
    pub objects: Vec<Box<dyn Hittable>>,
    pub lights: Vec<Light>,
    pub ambient: Color,
    pub camera: Camera,
    /// Sky gradient endpoints: color looking straight down vs. straight up.
    /// Ignored when `environment` is set.
    pub sky_bottom: Color,
    pub sky_top: Color,
    /// Optional environment map, sampled by ray direction wherever no
    /// geometry is hit - typically a `Texture::Image` equirectangular
    /// photo, though any `Texture` works (a `Noise` sky is a legitimate,
    /// if odd, choice). Overrides the `sky_bottom`/`sky_top` gradient.
    pub environment: Option<Texture>,
}

impl Scene {
    pub fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        self.objects.hit(ray, t_min, t_max)
    }

    /// Background used where no geometry is hit: the environment map if one
    /// is set, otherwise the sky gradient.
    pub fn background(&self, ray: &Ray) -> Color {
        let unit_dir = ray.direction.normalized();
        if let Some(env) = &self.environment {
            let (u, v) = unit_dir.to_equirect_uv();
            return env.sample(unit_dir, u, v);
        }
        let t = 0.5 * (unit_dir.y + 1.0);
        self.sky_bottom.lerp(self.sky_top, t)
    }
}
