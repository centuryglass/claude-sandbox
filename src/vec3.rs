use std::ops::{Add, AddAssign, Div, DivAssign, Index, Mul, MulAssign, Neg, Sub};

/// A 3-component float vector, doing triple duty as point / direction / RGB color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub type Point3 = Vec3;
pub type Color = Vec3;

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const ONE: Vec3 = Vec3::new(1.0, 1.0, 1.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub const fn splat(v: f64) -> Self {
        Vec3::new(v, v, v)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn normalized(self) -> Vec3 {
        self / self.length()
    }

    pub fn near_zero(self) -> bool {
        const EPS: f64 = 1e-8;
        self.x.abs() < EPS && self.y.abs() < EPS && self.z.abs() < EPS
    }

    /// Reflect `self` (incoming direction) about normal `n`. `n` must be unit length.
    pub fn reflect(self, n: Vec3) -> Vec3 {
        self - 2.0 * self.dot(n) * n
    }

    /// Refract `self` (incoming *unit* direction) through a surface with unit normal `n`,
    /// where `etai_over_etat` is the ratio of refractive indices (incident / transmitted).
    pub fn refract(self, n: Vec3, etai_over_etat: f64) -> Vec3 {
        let cos_theta = (-self).dot(n).min(1.0);
        let r_out_perp = etai_over_etat * (self + cos_theta * n);
        let r_out_parallel = -((1.0 - r_out_perp.length_squared()).abs().sqrt()) * n;
        r_out_perp + r_out_parallel
    }

    pub fn lerp(self, other: Vec3, t: f64) -> Vec3 {
        self * (1.0 - t) + other * t
    }

    pub fn min(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x.min(other.x), self.y.min(other.y), self.z.min(other.z))
    }

    pub fn max(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x.max(other.x), self.y.max(other.y), self.z.max(other.z))
    }

    pub fn component(self, axis: usize) -> f64 {
        self[axis]
    }

    pub fn random(rng: &mut impl rand::Rng, min: f64, max: f64) -> Vec3 {
        Vec3::new(
            rng.gen_range(min..max),
            rng.gen_range(min..max),
            rng.gen_range(min..max),
        )
    }

    /// Uniformly-distributed point strictly inside the unit ball, via
    /// rejection sampling (simple, and plenty fast for our sample counts).
    pub fn random_in_unit_sphere(rng: &mut impl rand::Rng) -> Vec3 {
        loop {
            let p = Vec3::random(rng, -1.0, 1.0);
            if p.length_squared() < 1.0 {
                return p;
            }
        }
    }

    pub fn random_unit_vector(rng: &mut impl rand::Rng) -> Vec3 {
        Vec3::random_in_unit_sphere(rng).normalized()
    }

    /// Uniformly-distributed point strictly inside the unit disk in the xy
    /// plane (z = 0) - used for thin-lens depth-of-field sampling.
    pub fn random_in_unit_disk(rng: &mut impl rand::Rng) -> Vec3 {
        loop {
            let p = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), 0.0);
            if p.length_squared() < 1.0 {
                return p;
            }
        }
    }
}

impl Index<usize> for Vec3 {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vec3 index out of range: {i}"),
        }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// Componentwise multiply (used constantly for tinting colors by albedo).
impl Mul for Vec3 {
    type Output = Vec3;
    fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

impl MulAssign<f64> for Vec3 {
    fn mul_assign(&mut self, s: f64) {
        self.x *= s;
        self.y *= s;
        self.z *= s;
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f64) -> Vec3 {
        self * (1.0 / s)
    }
}

impl DivAssign<f64> for Vec3 {
    fn div_assign(&mut self, s: f64) {
        *self *= 1.0 / s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect_off_flat_normal_flips_perpendicular_component() {
        let v = Vec3::new(1.0, -1.0, 0.0);
        let n = Vec3::new(0.0, 1.0, 0.0);
        let r = v.reflect(n);
        assert!((r - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn cross_of_axes_gives_third_axis() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn normalized_has_unit_length() {
        let v = Vec3::new(3.0, 4.0, 0.0).normalized();
        assert!((v.length() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn refract_straight_through_at_normal_incidence_is_unchanged_direction() {
        // A ray hitting dead-on (parallel to normal) should pass straight through
        // regardless of the index ratio.
        let v = Vec3::new(0.0, -1.0, 0.0);
        let n = Vec3::new(0.0, 1.0, 0.0);
        let r = v.refract(n, 1.0 / 1.5);
        assert!((r - v).length() < 1e-9);
    }
}
