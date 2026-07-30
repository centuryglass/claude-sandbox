use crate::vec3::Color;

/// Surface materials. Deliberately a plain data enum (not a trait object):
/// at this scale a `match` in the shader is simpler and faster than dynamic
/// dispatch, and it keeps `HitRecord` cheaply `Copy`.
#[derive(Debug, Clone, Copy)]
pub enum Material {
    Lambertian { albedo: Color },
    /// Mirror-like reflector. `fuzz` in [0, 1] jitters the reflected ray to
    /// fake microfacet roughness (0 = perfect mirror).
    Metal { albedo: Color, fuzz: f64 },
    /// Dielectric (glass-like) surface: `ior` is the material's index of
    /// refraction relative to vacuum/air (glass ~1.5, water ~1.33, diamond ~2.4).
    Dielectric { ior: f64 },
}
