use crate::aabb::Aabb;
use crate::hittable::{HitRecord, Hittable};
use crate::material::Material;
use crate::ray::Ray;
use crate::vec3::{Point3, Vec3};

const EPS: f64 = 1e-8;

pub struct Triangle {
    pub v0: Point3,
    pub v1: Point3,
    pub v2: Point3,
    /// Per-vertex normals for Phong/Gouraud-style smooth shading. When absent,
    /// the flat face normal is used everywhere (faceted look).
    pub normals: Option<(Vec3, Vec3, Vec3)>,
    pub material: Material,
}

impl Triangle {
    pub fn new(v0: Point3, v1: Point3, v2: Point3, material: Material) -> Self {
        Triangle { v0, v1, v2, normals: None, material }
    }

    pub fn with_normals(mut self, n0: Vec3, n1: Vec3, n2: Vec3) -> Self {
        self.normals = Some((n0, n1, n2));
        self
    }

    fn face_normal(&self) -> Vec3 {
        (self.v1 - self.v0).cross(self.v2 - self.v0).normalized()
    }
}

impl Hittable for Triangle {
    /// Möller–Trumbore ray-triangle intersection.
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let pvec = ray.direction.cross(edge2);
        let det = edge1.dot(pvec);
        if det.abs() < EPS {
            return None; // Ray is parallel to the triangle's plane.
        }
        let inv_det = 1.0 / det;

        let tvec = ray.origin - self.v0;
        let u = tvec.dot(pvec) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let qvec = tvec.cross(edge1);
        let v = ray.direction.dot(qvec) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = edge2.dot(qvec) * inv_det;
        if t < t_min || t > t_max {
            return None;
        }

        let p = ray.at(t);
        let outward_normal = match self.normals {
            Some((n0, n1, n2)) => {
                let w = 1.0 - u - v;
                (w * n0 + u * n1 + v * n2).normalized()
            }
            None => self.face_normal(),
        };
        Some(HitRecord::new(p, t, ray, outward_normal, self.material))
    }

    fn bounding_box(&self) -> Aabb {
        // Pad slightly: a triangle flat against an axis would otherwise
        // produce a zero-thickness slab on that axis, which is degenerate
        // for the BVH's slab test.
        let pad = Vec3::splat(1e-4);
        let min = self.v0.min(self.v1).min(self.v2) - pad;
        let max = self.v0.max(self.v1).max(self.v2) + pad;
        Aabb::new(min, max)
    }
}
