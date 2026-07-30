use crate::material::Material;
use crate::ray::Ray;
use crate::vec3::{Point3, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct HitRecord {
    pub p: Point3,
    pub normal: Vec3,
    pub t: f64,
    pub front_face: bool,
    pub material: Material,
}

impl HitRecord {
    /// `outward_normal` must be unit length. Figures out which side of the
    /// surface the ray hit from and flips the stored normal to always face
    /// the incoming ray, recording the original orientation in `front_face`.
    pub fn new(p: Point3, t: f64, ray: &Ray, outward_normal: Vec3, material: Material) -> Self {
        let front_face = ray.direction.dot(outward_normal) < 0.0;
        let normal = if front_face { outward_normal } else { -outward_normal };
        HitRecord { p, normal, t, front_face, material }
    }
}

pub trait Hittable: Sync {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord>;
}

impl Hittable for Vec<Box<dyn Hittable>> {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let mut closest = t_max;
        let mut result = None;
        for object in self {
            if let Some(rec) = object.hit(ray, t_min, closest) {
                closest = rec.t;
                result = Some(rec);
            }
        }
        result
    }
}
