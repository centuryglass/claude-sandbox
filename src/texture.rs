//! Textures: procedural world-space ones (sampled directly from the 3D hit
//! point - works uniformly across spheres and arbitrary meshes with zero UV
//! plumbing, and interacts naturally with glitch modes that corrupt hit
//! positions) plus image textures (which do need real UV coordinates, since
//! there's no other sane way to map a photo onto a surface).
use crate::vec3::{Color, Point3};
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Texture {
    Solid(Color),
    Checker { a: Color, b: Color, scale: f64 },
    Stripes { a: Color, b: Color, scale: f64, axis: usize },
    /// Blends `bottom` to `top` by world-space height (y), remapped over `[y0, y1]`.
    Gradient { bottom: Color, top: Color, y0: f64, y1: f64 },
    /// Fractal value noise ("turbulence") modulating brightness around `color`.
    Noise { color: Color, scale: f64, octaves: u32 },
    /// An image sampled by UV coordinates, bilinearly filtered, tiling
    /// beyond `[0, 1]`. `Arc`-wrapped so cloning a `Texture` (cheap and
    /// frequent - every `Material` carries one) never copies pixel data.
    Image(Arc<ImageTexture>),
}

impl Texture {
    /// `p` is used by the world-space variants; `u`, `v` by `Image`. Each
    /// variant only looks at whichever it needs.
    pub fn sample(&self, p: Point3, u: f64, v: f64) -> Color {
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
            Texture::Image(img) => img.sample(u, v),
        }
    }
}

/// A decoded, immutable image sampled by UV coordinates.
pub struct ImageTexture {
    width: u32,
    height: u32,
    // Row-major, top-to-bottom, RGB, one byte per channel - decoded once at
    // load time so sampling is a plain index instead of repeated format
    // dispatch through the `image` crate.
    pixels: Vec<[u8; 3]>,
}

impl std::fmt::Debug for ImageTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageTexture").field("width", &self.width).field("height", &self.height).finish_non_exhaustive()
    }
}

impl ImageTexture {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let img = image::open(path).with_context(|| format!("loading image texture {}", path.display()))?.into_rgb8();
        let (width, height) = (img.width(), img.height());
        let pixels = img.pixels().map(|p| p.0).collect();
        Ok(ImageTexture { width, height, pixels })
    }

    /// Bilinear sample, tiling `u`/`v` beyond `[0, 1]`. `v = 0` is the
    /// bottom of the image and `v = 1` the top, matching this renderer's
    /// other conventions (`sphere_uv`, viewport `t`) - the opposite of the
    /// image's own top-to-bottom row order, hence the flip below. Sampled
    /// values are used as-is (not sRGB-decoded): every other color in this
    /// renderer is an author-specified linear-ish `[0, 1]` value with gamma
    /// correction applied once at final output, so treating image bytes the
    /// same way keeps the whole pipeline consistent, even if not
    /// colorimetrically "correct".
    fn sample(&self, u: f64, v: f64) -> Color {
        let u = u.rem_euclid(1.0);
        let v = 1.0 - v.rem_euclid(1.0);

        let fx = u * self.width as f64 - 0.5;
        let fy = v * self.height as f64 - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let (tx, ty) = (fx - x0, fy - y0);

        let wrap = |v: f64, n: u32| (v.rem_euclid(n as f64)) as u32;
        let texel = |xi: f64, yi: f64| -> Color {
            let px = self.pixels[(wrap(yi, self.height) * self.width + wrap(xi, self.width)) as usize];
            Color::new(px[0] as f64 / 255.0, px[1] as f64 / 255.0, px[2] as f64 / 255.0)
        };

        let c00 = texel(x0, y0);
        let c10 = texel(x0 + 1.0, y0);
        let c01 = texel(x0, y0 + 1.0);
        let c11 = texel(x0 + 1.0, y0 + 1.0);
        c00.lerp(c10, tx).lerp(c01.lerp(c11, tx), ty)
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
        assert_eq!(tex.sample(Point3::new(0.5, 0.0, 0.5), 0.0, 0.0), Color::new(1.0, 0.0, 0.0));
        assert_eq!(tex.sample(Point3::new(1.5, 0.0, 0.5), 0.0, 0.0), Color::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn gradient_endpoints_match_bottom_and_top() {
        let tex = Texture::Gradient { bottom: Color::new(0.0, 0.0, 0.0), top: Color::new(1.0, 1.0, 1.0), y0: 0.0, y1: 10.0 };
        assert_eq!(tex.sample(Point3::new(0.0, 0.0, 0.0), 0.0, 0.0), Color::ZERO);
        assert_eq!(tex.sample(Point3::new(0.0, 10.0, 0.0), 0.0, 0.0), Color::ONE);
        assert_eq!(tex.sample(Point3::new(0.0, 999.0, 0.0), 0.0, 0.0), Color::ONE); // clamped past y1
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

    #[test]
    fn image_texture_sample_matches_nearest_source_pixel_at_texel_centers() {
        // A 2x2 image: red, green / blue, white. Sampling exactly at a
        // texel center should reproduce that texel's color (no blending).
        let pixels = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
        let img = ImageTexture { width: 2, height: 2, pixels };
        // v=0 is the bottom of the image per our convention, so v near 1
        // samples the top row (red/green) and v near 0 samples blue/white.
        assert_close(img.sample(0.25, 0.75), Color::new(1.0, 0.0, 0.0));
        assert_close(img.sample(0.75, 0.75), Color::new(0.0, 1.0, 0.0));
        assert_close(img.sample(0.25, 0.25), Color::new(0.0, 0.0, 1.0));
        assert_close(img.sample(0.75, 0.25), Color::ONE);
    }

    #[test]
    fn image_texture_wraps_beyond_unit_range() {
        let pixels = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
        let img = ImageTexture { width: 2, height: 2, pixels };
        assert_eq!(img.sample(0.25, 0.75), img.sample(1.25, 0.75));
        assert_eq!(img.sample(0.25, 0.75), img.sample(-0.75, 0.75));
    }

    fn assert_close(a: Color, b: Color) {
        assert!((a - b).length() < 1e-6, "{a:?} != {b:?}");
    }
}
