use crate::vec3::{Color, Point3, Vec3};
use rand::Rng;

pub enum Light {
    Point { position: Point3, color: Color, intensity: f64 },
    /// A finite rectangular area light: a quad centered at `center`, spanning
    /// `u_axis` and `v_axis` (full edge vectors, not half - so a 2x2 light
    /// facing down is `u_axis: (2,0,0), v_axis: (0,0,2)`). Emits from one
    /// face only, in the `u_axis.cross(v_axis)` direction.
    ///
    /// The original 2013 source (`reference/AreaLight.h`, referenced from
    /// `MultiReflectionShader.cpp`) had exactly this: `AreaLight::getLightPt`
    /// picks a jittered point on the light per ray instead of always using
    /// one fixed position. Sampled fresh on every shadow ray here too, so
    /// with enough samples-per-pixel the many slightly-different shadow rays
    /// average into a soft penumbra instead of a hard edge - the classic
    /// Monte Carlo soft-shadow technique, and (per the reference source's own
    /// comment gating it behind `getMaxRayNum() > 1`) exactly the sampling
    /// regime it was written for.
    Rect { center: Point3, u_axis: Vec3, v_axis: Vec3, color: Color, intensity: f64 },
}

/// Direction from a shading point to the light, its distance, and its
/// radiant color at that point (falloff already applied).
pub struct LightSample {
    pub direction: Point3,
    pub distance: f64,
    pub color: Color,
}

impl Light {
    pub fn sample(&self, from: Point3, rng: &mut impl Rng) -> LightSample {
        match *self {
            Light::Point { position, color, intensity } => {
                let to_light = position - from;
                let distance = to_light.length();
                // Inverse-square falloff, normalized so `intensity` roughly
                // matches perceived brightness at distance 1.
                let attenuation = intensity / (distance * distance).max(1e-4);
                LightSample {
                    direction: to_light / distance,
                    distance,
                    color: color * attenuation,
                }
            }
            Light::Rect { center, u_axis, v_axis, color, intensity } => {
                let su = rng.gen_range(-0.5..0.5);
                let sv = rng.gen_range(-0.5..0.5);
                let point = center + u_axis * su + v_axis * sv;

                let to_light = point - from;
                let distance = to_light.length();
                let direction = to_light / distance;

                let raw_normal = u_axis.cross(v_axis);
                let area = raw_normal.length();
                let light_normal = raw_normal / area;
                // Only the face the light emits from contributes - a shading
                // point behind the panel sees no light from it at all.
                let cos_light = light_normal.dot(-direction).max(0.0);
                // Scales with area so a bigger panel reads as a brighter
                // light, same as a real-world softbox, not just a wider
                // spread of the same total brightness.
                let attenuation = intensity * cos_light * area / (distance * distance).max(1e-4);

                LightSample { direction, distance, color: color * attenuation }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_light_samples_land_within_its_extent() {
        let light = Light::Rect {
            center: Point3::new(0.0, 5.0, 0.0),
            u_axis: Vec3::new(2.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, 0.0, 2.0),
            color: Color::ONE,
            intensity: 10.0,
        };
        let mut rng = rand::thread_rng();
        for _ in 0..200 {
            let sample = light.sample(Point3::new(0.0, 0.0, 0.0), &mut rng);
            // The light spans x in [-1, 1], z in [-1, 1] at y = 5; the
            // sampled point (center + direction * distance) must stay
            // within that quad however the RNG jitters it.
            let point = Point3::new(0.0, 0.0, 0.0) + sample.direction * sample.distance;
            assert!((-1.0..=1.0).contains(&point.x), "x out of range: {}", point.x);
            assert!((-1.0..=1.0).contains(&point.z), "z out of range: {}", point.z);
            assert!((point.y - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn rect_light_gives_no_contribution_from_behind_its_face() {
        // u_axis.cross(v_axis) = (1,0,0) x (0,0,1) = (0,-1,0), so this light
        // faces -y. A point above it (positive y) is behind the emitting
        // face and should get nothing.
        let light = Light::Rect {
            center: Point3::new(0.0, 0.0, 0.0),
            u_axis: Vec3::new(1.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, 0.0, 1.0),
            color: Color::ONE,
            intensity: 10.0,
        };
        let mut rng = rand::thread_rng();
        let sample = light.sample(Point3::new(0.0, 3.0, 0.0), &mut rng);
        assert_eq!(sample.color, Color::ZERO);
    }

    #[test]
    fn rect_light_area_scales_brightness() {
        let from = Point3::new(0.0, 0.0, 0.0);
        let small = Light::Rect {
            center: Point3::new(0.0, 5.0, 0.0),
            u_axis: Vec3::new(1.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, 0.0, 1.0),
            color: Color::ONE,
            intensity: 10.0,
        };
        let big = Light::Rect {
            center: Point3::new(0.0, 5.0, 0.0),
            u_axis: Vec3::new(4.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, 0.0, 4.0),
            color: Color::ONE,
            intensity: 10.0,
        };
        let mut rng = rand::thread_rng();
        // Sample straight down the light's axis (su = sv = 0 in expectation
        // over many draws is close enough at the center) so distance/cosine
        // stay effectively identical between the two - only area differs.
        let small_avg: f64 = (0..500).map(|_| small.sample(from, &mut rng).color.x).sum::<f64>() / 500.0;
        let big_avg: f64 = (0..500).map(|_| big.sample(from, &mut rng).color.x).sum::<f64>() / 500.0;
        assert!(big_avg > small_avg * 10.0, "big={big_avg}, small={small_avg}");
    }
}
