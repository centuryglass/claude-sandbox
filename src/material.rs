use crate::texture::Texture;

/// Surface materials. Deliberately a plain data enum (not a trait object):
/// at this scale a `match` in the shader is simpler and faster than dynamic
/// dispatch. `Clone` rather than `Copy` since `Texture::Image` carries an
/// `Arc`-wrapped image (cheap to clone, not to bitwise-copy).
#[derive(Debug, Clone)]
pub enum Material {
    Lambertian { albedo: Texture },
    /// Mirror-like reflector. `fuzz` in [0, 1] jitters the reflected ray to
    /// fake microfacet roughness (0 = perfect mirror).
    Metal { albedo: Texture, fuzz: f64 },
    /// Dielectric (glass-like) surface: `ior` is the material's index of
    /// refraction relative to vacuum/air (glass ~1.5, water ~1.33, diamond ~2.4).
    Dielectric { ior: f64 },
    /// Light-emitting surface: radiates `color * intensity` regardless of
    /// incident light, and does not itself receive shading. A crystal core,
    /// a lava crack, a neon strip - the object *is* a light source, rather
    /// than something a light source shines on.
    Emissive { color: Texture, intensity: f64 },
}
