use crate::vec3::Color;

/// Surface materials. Deliberately a plain data enum (not a trait object):
/// at this scale a `match` in the shader is simpler and faster than dynamic
/// dispatch, and it keeps `HitRecord` cheaply `Copy`.
#[derive(Debug, Clone, Copy)]
pub enum Material {
    Lambertian { albedo: Color },
}
