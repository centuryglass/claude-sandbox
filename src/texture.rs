//! Procedural, world-space textures. No UV coordinates needed - everything
//! samples directly from the 3D hit point, which keeps mesh support simple
//! and (as a bonus) means textures interact naturally with glitch modes that
//! corrupt hit positions.
use crate::vec3::{Color, Point3};

#[derive(Debug, Clone, Copy)]
pub enum Texture {
    Solid(Color),
    Checker { a: Color, b: Color, scale: f64 },
    Stripes { a: Color, b: Color, scale: f64, axis: usize },
    /// Blends `bottom` to `top` by world-space height (y), remapped over `[y0, y1]`.
    Gradient { bottom: Color, top: Color, y0: f64, y1: f64 },
    /// Fractal value noise ("turbulence") modulating brightness around `color`.
    Noise { color: Color, scale: f64, octaves: u32 },
}

impl Texture {
    pub fn sample(&self, p: Point3) -> Color {
        match self {
            Texture::Solid(c) => *c,
            Texture::Checker { a, b, scale } => {
                let s = (p.x * scale).floor() as i64 + (p.y * scale).floor() as i64 + (p.z * scale).floor() as i64;
                if s.rem_euclid(2) == 0 { *a } else { *b }
            }
            Texture::Stripes { a, b, scale, axis } => {
                let v = p.component(*axis) * scale;
                if (v.floor() as i64).rem_euclid(2) == 0 { *a } else { *b }
            }
            Texture::Gradient { bottom, top, y0, y1 } => {
                let t = ((p.y - y0) / (y1 - y0)).clamp(0.0, 1.0);
                bottom.lerp(*top, t)
            }
            Texture::Noise { color, scale, octaves } => {
                let t = turbulence(p * *scale, *octaves);
                *color * (0.4 + 0.6 * t)
            }
        }
    }
}

/// Hash-based value noise: deterministic pseudo-random value per lattice
/// corner, trilinearly interpolated. No external noise crate - just a hash
/// function and a lerp, in the spirit of the rest of this renderer.
fn hash(x: i64, y: i64, z: i64) -> f64 {
    let mut h = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ z.wrapping_mul(2147483647);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFFFF) as f64 / 0xFFFFFF as f64
}

fn value_noise(p: Point3) -> f64 {
    let (x0, y0, z0) = (p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64);
    let (fx, fy, fz) = (p.x - x0 as f64, p.y - y0 as f64, p.z - z0 as f64);
    // Smoothstep for a less blocky interpolation than raw linear.
    let sm = |t: f64| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (sm(fx), sm(fy), sm(fz));

    let c000 = hash(x0, y0, z0);
    let c100 = hash(x0 + 1, y0, z0);
    let c010 = hash(x0, y0 + 1, z0);
    let c110 = hash(x0 + 1, y0 + 1, z0);
    let c001 = hash(x0, y0, z0 + 1);
    let c101 = hash(x0 + 1, y0, z0 + 1);
    let c011 = hash(x0, y0 + 1, z0 + 1);
    let c111 = hash(x0 + 1, y0 + 1, z0 + 1);

    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(c000, c100, u);
    let x10 = lerp(c010, c110, u);
    let x01 = lerp(c001, c101, u);
    let x11 = lerp(c011, c111, u);
    let y0v = lerp(x00, x10, v);
    let y1v = lerp(x01, x11, v);
    lerp(y0v, y1v, w)
}

fn turbulence(p: Point3, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut freq_p = p;
    let mut max = 0.0;
    for _ in 0..octaves.max(1) {
        sum += value_noise(freq_p) * amplitude;
        max += amplitude;
        amplitude *= 0.5;
        freq_p *= 2.0;
    }
    sum / max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checker_alternates_across_a_cell_boundary() {
        let tex = Texture::Checker { a: Color::new(1.0, 0.0, 0.0), b: Color::new(0.0, 1.0, 0.0), scale: 1.0 };
        assert_eq!(tex.sample(Point3::new(0.5, 0.0, 0.5)), Color::new(1.0, 0.0, 0.0));
        assert_eq!(tex.sample(Point3::new(1.5, 0.0, 0.5)), Color::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn gradient_endpoints_match_bottom_and_top() {
        let tex = Texture::Gradient { bottom: Color::new(0.0, 0.0, 0.0), top: Color::new(1.0, 1.0, 1.0), y0: 0.0, y1: 10.0 };
        assert_eq!(tex.sample(Point3::new(0.0, 0.0, 0.0)), Color::ZERO);
        assert_eq!(tex.sample(Point3::new(0.0, 10.0, 0.0)), Color::ONE);
        assert_eq!(tex.sample(Point3::new(0.0, 999.0, 0.0)), Color::ONE); // clamped past y1
    }

    #[test]
    fn value_noise_is_deterministic_and_bounded() {
        let p = Point3::new(1.7, -2.3, 0.4);
        let a = value_noise(p);
        let b = value_noise(p);
        assert_eq!(a, b);
        assert!((0.0..=1.0).contains(&a));
    }

    #[test]
    fn turbulence_stays_in_unit_range_over_many_samples() {
        for i in 0..200 {
            let p = Point3::new(i as f64 * 0.37, i as f64 * 1.11, i as f64 * -0.53);
            let t = turbulence(p, 4);
            assert!((0.0..=1.0).contains(&t), "turbulence out of range: {t}");
        }
    }
}
