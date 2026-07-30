use crate::ray::Ray;
use crate::vec3::Point3;

/// Axis-aligned bounding box, used to accelerate ray-scene intersection via
/// the BVH (see bvh.rs) so we don't test every triangle against every ray.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: Point3,
    pub max: Point3,
}

impl Aabb {
    pub fn new(min: Point3, max: Point3) -> Self {
        Aabb { min, max }
    }

    pub fn surrounding(a: Aabb, b: Aabb) -> Aabb {
        Aabb::new(a.min.min(b.min), a.max.max(b.max))
    }

    /// Slab-method intersection test: does not need the hit point, just
    /// whether the ray's [t_min, t_max] interval overlaps the box at all.
    pub fn hit(&self, ray: &Ray, mut t_min: f64, mut t_max: f64) -> bool {
        for axis in 0..3 {
            let inv_d = 1.0 / ray.direction[axis];
            let mut t0 = (self.min[axis] - ray.origin[axis]) * inv_d;
            let mut t1 = (self.max[axis] - ray.origin[axis]) * inv_d;
            if inv_d < 0.0 {
                std::mem::swap(&mut t0, &mut t1);
            }
            t_min = t_min.max(t0);
            t_max = t_max.min(t1);
            if t_max <= t_min {
                return false;
            }
        }
        true
    }

    pub fn centroid(&self) -> Point3 {
        (self.min + self.max) * 0.5
    }
}
