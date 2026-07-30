use crate::ray::Ray;
use crate::vec3::{Point3, Vec3};

pub struct Camera {
    origin: Point3,
    lower_left: Point3,
    horizontal: Vec3,
    vertical: Vec3,
}

impl Camera {
    /// `vfov` is the vertical field of view in degrees.
    pub fn new(look_from: Point3, look_at: Point3, vup: Vec3, vfov: f64, aspect_ratio: f64) -> Self {
        let theta = vfov.to_radians();
        let h = (theta / 2.0).tan();
        let viewport_height = 2.0 * h;
        let viewport_width = aspect_ratio * viewport_height;

        let w = (look_from - look_at).normalized();
        let u = vup.cross(w).normalized();
        let v = w.cross(u);

        let origin = look_from;
        let horizontal = viewport_width * u;
        let vertical = viewport_height * v;
        let lower_left = origin - horizontal / 2.0 - vertical / 2.0 - w;

        Camera { origin, lower_left, horizontal, vertical }
    }

    /// `s`, `t` are normalized viewport coordinates in [0, 1], with (0, 0) at
    /// the bottom-left, matching the local u/v basis built in `new`.
    pub fn get_ray(&self, s: f64, t: f64) -> Ray {
        Ray::new(
            self.origin,
            self.lower_left + s * self.horizontal + t * self.vertical - self.origin,
        )
    }
}
