use crate::aabb::Aabb;
use crate::hittable::{HitRecord, Hittable};
use crate::material::Material;
use crate::ray::Ray;
use crate::vec3::{Point3, Vec3};
use std::f64::consts::PI;

pub struct Sphere {
    pub center: Point3,
    pub radius: f64,
    pub material: Material,
}

impl Sphere {
    pub fn new(center: Point3, radius: f64, material: Material) -> Self {
        Sphere { center, radius, material }
    }
}

/// Standard spherical UV mapping for a unit outward normal: `u` wraps once
/// around the equator (longitude), `v` runs 0 (south pole) to 1 (north pole).
fn sphere_uv(outward_normal: Vec3) -> (f64, f64) {
    let theta = (-outward_normal.y).acos();
    let phi = (-outward_normal.z).atan2(outward_normal.x) + PI;
    (phi / (2.0 * PI), theta / PI)
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let oc = ray.origin - self.center;
        let a = ray.direction.length_squared();
        let half_b = oc.dot(ray.direction);
        let c = oc.length_squared() - self.radius * self.radius;
        let discriminant = half_b * half_b - a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrt_d = discriminant.sqrt();

        // Nearest root within [t_min, t_max]; fall back to the far root.
        let mut root = (-half_b - sqrt_d) / a;
        if root < t_min || root > t_max {
            root = (-half_b + sqrt_d) / a;
            if root < t_min || root > t_max {
                return None;
            }
        }

        let p = ray.at(root);
        let outward_normal = (p - self.center) / self.radius;
        let (u, v) = sphere_uv(outward_normal);
        Some(HitRecord::new(p, root, ray, outward_normal, self.material.clone(), u, v))
    }

    fn bounding_box(&self) -> Aabb {
        let r = Vec3::splat(self.radius);
        Aabb::new(self.center - r, self.center + r)
    }
}
