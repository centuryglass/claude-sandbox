//! Bakes a procedural equirectangular starfield/nebula environment map,
//! reusing the same hash-based value noise as the renderer's `Noise`
//! texture (see `texture::turbulence`) so this has the same hand-rolled,
//! no-external-noise-crate character as everything else here. Written to
//! sidestep the licensing uncertainty of any found space photo entirely -
//! every pixel is generated, not sourced.
use anyhow::Result;
use image::RgbImage;
use lumin::texture::{hash, turbulence};
use lumin::vec3::{Point3, Vec3};
use std::f64::consts::PI;

/// Inverse of `Vec3::to_equirect_uv`: maps equirectangular `(u, v)` back to
/// a unit direction, so noise is evaluated in 3D on the sphere itself
/// rather than in 2D image space. Sampling in 3D means the u=0/u=1 seam
/// and both poles come out seamless for free, which 2D noise-in-uv-space
/// would not give us.
fn direction_for(u: f64, v: f64) -> Vec3 {
    let theta = v * PI;
    let phi = u * 2.0 * PI - PI;
    let (st, ct) = (theta.sin(), theta.cos());
    let (sp, cp) = (phi.sin(), phi.cos());
    Vec3::new(st * cp, -ct, -st * sp)
}

/// Deterministic per-pixel "is there a star here" test, keyed off the
/// direction vector (not pixel coordinates) so equirect pole distortion
/// doesn't skew star density, and off a different lattice scale/offset
/// than the nebula noise so star placement doesn't correlate with cloud
/// brightness.
fn star_color(dir: Vec3) -> Option<Vec3> {
    let cell = 900.0;
    let (cx, cy, cz) = ((dir.x * cell) as i64, (dir.y * cell) as i64, (dir.z * cell) as i64);
    if hash(cx, cy, cz) < 0.9986 {
        return None;
    }
    let brightness = 0.5 + 0.5 * hash(cx + 7, cy + 7, cz + 7);
    // Warm-to-cool color temperature variance, like real starlight.
    let warmth = hash(cx + 13, cy + 13, cz + 13);
    let color = Vec3::new(0.9 + 0.1 * warmth, 0.85 + 0.1 * (1.0 - (warmth - 0.5).abs() * 2.0), 0.8 + 0.2 * (1.0 - warmth));
    Some(color * brightness)
}

fn main() -> Result<()> {
    let width: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2048);
    let height: u32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let out_path = std::env::args().nth(3).unwrap_or_else(|| "assets/textures/nebula.png".to_string());

    // Not pure black, so nebula wisps have somewhere to blend from and
    // starless regions don't read as a flat void.
    let deep_space = Vec3::new(0.01, 0.01, 0.02);
    // Three nebula color regions (magenta, teal, warm gold), blended by a
    // second, lower-frequency noise field so the sky reads as a few
    // distinct cloud regions rather than one uniform tint.
    let palette_a = Vec3::new(0.55, 0.08, 0.35);
    let palette_b = Vec3::new(0.05, 0.35, 0.45);
    let palette_c = Vec3::new(0.5, 0.25, 0.05);

    let smoothstep = |t: f64| { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };

    let img = RgbImage::from_fn(width, height, |x, y| {
        let u = x as f64 / width as f64;
        let v = 1.0 - y as f64 / height as f64;
        let dir = direction_for(u, v);

        // Domain warp: displace the sample point by another noise field
        // before evaluating density, which turns round noise blobs into
        // wispy, filament-like nebula shapes instead of flat-shaded ovals.
        let warp = Vec3::new(
            turbulence(dir * 1.3 + Point3::new(3.1, 1.7, 9.4), 3),
            turbulence(dir * 1.3 + Point3::new(7.2, 4.4, 2.1), 3),
            turbulence(dir * 1.3 + Point3::new(5.5, 9.9, 1.2), 3),
        ) - Vec3::new(0.5, 0.5, 0.5);
        let warped = dir + warp * 0.7;

        // Higher exponent than a "typical" contrast curve on purpose: this
        // pushes most of the sky toward near-black so the bright filaments
        // stand out and stars have somewhere dark to be visible against.
        let density = turbulence(warped * 2.5, 6).powf(2.0);
        // Fine high-frequency filaments layered on top of the broad clouds.
        let detail = turbulence(warped * 9.0, 3).powf(3.0) * 0.3;

        // Continuous three-stop color ramp (teal -> magenta -> gold) driven
        // by a slower, independent noise field, smoothstepped so regions
        // blend into each other instead of meeting at a hard edge.
        let region = turbulence(dir * 0.6 + Point3::new(11.0, 5.0, 3.0), 3);
        let palette = if region < 0.5 {
            palette_b.lerp(palette_a, smoothstep(region / 0.5))
        } else {
            palette_a.lerp(palette_c, smoothstep((region - 0.5) / 0.5))
        };

        let mut color = deep_space + palette * (density * 0.8 + detail);
        if let Some(star) = star_color(dir) {
            color += star;
        }

        let gamma = |c: f64| c.clamp(0.0, 1.0).powf(1.0 / 2.2);
        image::Rgb([(gamma(color.x) * 255.0) as u8, (gamma(color.y) * 255.0) as u8, (gamma(color.z) * 255.0) as u8])
    });

    img.save(&out_path)?;
    println!("wrote {out_path} ({width}x{height})");
    Ok(())
}
