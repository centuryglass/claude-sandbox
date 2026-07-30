use crate::camera::Camera;
use crate::hittable::{HitRecord, Hittable};
use crate::light::Light;
use crate::ray::Ray;
use crate::vec3::Color;

pub struct Scene {
    pub objects: Vec<Box<dyn Hittable>>,
    pub lights: Vec<Light>,
    pub ambient: Color,
    pub camera: Camera,
}

impl Scene {
    pub fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        self.objects.hit(ray, t_min, t_max)
    }

    /// Sky gradient used where no geometry is hit.
    pub fn background(&self, ray: &Ray) -> Color {
        let unit_dir = ray.direction.normalized();
        let t = 0.5 * (unit_dir.y + 1.0);
        Color::new(1.0, 1.0, 1.0).lerp(Color::new(0.5, 0.7, 1.0), t)
    }
}
