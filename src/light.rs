use crate::vec3::{Color, Point3};

pub enum Light {
    Point { position: Point3, color: Color, intensity: f64 },
}

/// Direction from a shading point to the light, its distance, and its
/// radiant color at that point (falloff already applied).
pub struct LightSample {
    pub direction: Point3,
    pub distance: f64,
    pub color: Color,
}

impl Light {
    pub fn sample(&self, from: Point3) -> LightSample {
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
        }
    }
}
