use crate::ray::Ray;
use crate::vec3::{Point3, Vec3};
use rand::Rng;

/// A camera projection. Plain data enum with a `match` in [`Camera::get_ray`],
/// same style as [`crate::material::Material`] and [`crate::texture::Texture`]
/// and for the same reason: at this scale a match is simpler and faster than
/// trait-object dispatch, and every variant is cheap to keep around.
///
/// `Perspective` is the default and the original projection this renderer
/// shipped with; every other variant is additive. `get_ray(s, t)` takes
/// normalized viewport coordinates in `[0, 1] x [0, 1]`, `(0, 0)` at the
/// bottom-left, for all variants - what differs is only how that unit square
/// gets mapped to a ray direction.
pub enum Camera {
    Perspective(PerspectiveCamera),
    Orthographic(OrthographicCamera),
    Fisheye(FisheyeCamera),
    Stereographic(StereographicCamera),
    Equirectangular(EquirectangularCamera),
}

impl Camera {
    /// Standard pinhole/perspective projection. `vfov` is the vertical field
    /// of view in degrees. Kept as a free function (not a method on
    /// [`PerspectiveCamera`]) under its original name so every existing call
    /// site - hand-built demo scenes in `main.rs`, RON scenes with no `kind`
    /// - keeps compiling and rendering byte-identical output.
    pub fn new(look_from: Point3, look_at: Point3, vup: Vec3, vfov: f64, aspect_ratio: f64) -> Self {
        Camera::Perspective(PerspectiveCamera::new(look_from, look_at, vup, vfov, aspect_ratio))
    }

    /// Orthographic (parallel-projection) camera: every ray shares the same
    /// direction, only the origin sweeps across the viewport. No convergence
    /// means no field of view to speak of - `height` is the world-space
    /// vertical extent of the frame instead, playing the role `vfov` plays
    /// for `Perspective`.
    pub fn new_orthographic(look_from: Point3, look_at: Point3, vup: Vec3, height: f64, aspect_ratio: f64) -> Self {
        Camera::Orthographic(OrthographicCamera::new(look_from, look_at, vup, height, aspect_ratio))
    }

    /// True equidistant fisheye: the angle between a ray and the view axis
    /// is *linear* in the ray's radius on the image plane, which is the
    /// literal defining property of an equidistant lens (as opposed to
    /// `Perspective`'s `tan`-based mapping, which only approximates "field of
    /// view" for angles well under 90 degrees). `fov_degrees` is therefore a
    /// real angular field of view and can meaningfully go all the way to 360
    /// for a full-sphere image.
    pub fn new_fisheye(look_from: Point3, look_at: Point3, vup: Vec3, fov_degrees: f64, aspect_ratio: f64) -> Self {
        Camera::Fisheye(FisheyeCamera::new(look_from, look_at, vup, fov_degrees, aspect_ratio))
    }

    /// Stereographic ("little planet") fisheye variant: same idea as
    /// `Fisheye`, but the radius-to-angle mapping is `r = 2 tan(theta / 2)`
    /// instead of `r = theta`. Angles well past 90 degrees get compressed
    /// much harder toward the frame edge than equidistant does, which is
    /// exactly what curls a wide-FOV horizon into a ring around the image
    /// center - point the camera straight up or down with a wide `fov_degrees`
    /// to get the classic tiny-planet look.
    pub fn new_stereographic(look_from: Point3, look_at: Point3, vup: Vec3, fov_degrees: f64, aspect_ratio: f64) -> Self {
        Camera::Stereographic(StereographicCamera::new(look_from, look_at, vup, fov_degrees, aspect_ratio))
    }

    /// Equirectangular (360x180 panorama) camera: longitude maps linearly
    /// across `s`, latitude linearly across `t`. Always covers the entire
    /// sphere around `look_from`, so unlike every other variant it takes no
    /// field-of-view parameter at all - and unlike `Orthographic` it doesn't
    /// take an aspect ratio either, since the mapping only makes geometric
    /// sense at a 2:1 width:height (render with `--aspect 2.0`); other
    /// aspect ratios just stretch the sphere instead of cropping it.
    pub fn new_equirectangular(look_from: Point3, look_at: Point3, vup: Vec3) -> Self {
        Camera::Equirectangular(EquirectangularCamera::new(look_from, look_at, vup))
    }

    /// Thin-lens perspective camera: like `new`, but rays originate from a
    /// random point on a lens disk of radius `aperture / 2` instead of a
    /// single point, all still converging on the same plane at `focus_dist`,
    /// the classic depth-of-field trick. `aperture = 0.0` is exactly `new`'s
    /// pinhole camera (and, since multiplying by a zero-radius disk offset
    /// is a no-op, produces byte-identical rays to it).
    pub fn new_thin_lens(look_from: Point3, look_at: Point3, vup: Vec3, vfov: f64, aspect_ratio: f64, aperture: f64, focus_dist: f64) -> Self {
        Camera::Perspective(PerspectiveCamera::new_thin_lens(look_from, look_at, vup, vfov, aspect_ratio, aperture, focus_dist))
    }

    /// `s`, `t` are normalized viewport coordinates in [0, 1], with (0, 0) at
    /// the bottom-left. `rng` only matters for `Perspective` cameras with a
    /// nonzero aperture (lens-position sampling for depth of field); every
    /// other variant ignores it.
    pub fn get_ray(&self, s: f64, t: f64, rng: &mut impl Rng) -> Ray {
        match self {
            Camera::Perspective(c) => c.get_ray(s, t, rng),
            Camera::Orthographic(c) => c.get_ray(s, t),
            Camera::Fisheye(c) => c.get_ray(s, t),
            Camera::Stereographic(c) => c.get_ray(s, t),
            Camera::Equirectangular(c) => c.get_ray(s, t),
        }
    }
}

/// Right-handed local basis for a camera looking from `look_from` toward
/// `look_at`. `w` points *backward* - from `look_at` towards `look_from`,
/// i.e. away from the view direction, so "forward" is always `-w` - which
/// matches the convention the original `Camera::new` used. Shared by every
/// projection below; only how `(s, t)` turns into a direction in this basis
/// differs between them.
struct Basis {
    origin: Point3,
    u: Vec3,
    v: Vec3,
    w: Vec3,
}

impl Basis {
    fn new(look_from: Point3, look_at: Point3, vup: Vec3) -> Self {
        let w = (look_from - look_at).normalized();
        let u = vup.cross(w).normalized();
        let v = w.cross(u);
        Basis { origin: look_from, u, v, w }
    }
}

pub struct PerspectiveCamera {
    origin: Point3,
    lower_left: Point3,
    horizontal: Vec3,
    vertical: Vec3,
    u: Vec3,
    v: Vec3,
    lens_radius: f64,
}

impl PerspectiveCamera {
    fn new(look_from: Point3, look_at: Point3, vup: Vec3, vfov: f64, aspect_ratio: f64) -> Self {
        // aperture 0, focus_dist 1: multiplying every viewport term by
        // focus_dist=1.0 is a bit-exact no-op, so this is byte-identical to
        // the pre-depth-of-field formula.
        PerspectiveCamera::new_thin_lens(look_from, look_at, vup, vfov, aspect_ratio, 0.0, 1.0)
    }

    fn new_thin_lens(look_from: Point3, look_at: Point3, vup: Vec3, vfov: f64, aspect_ratio: f64, aperture: f64, focus_dist: f64) -> Self {
        let theta = vfov.to_radians();
        let h = (theta / 2.0).tan();
        let viewport_height = 2.0 * h;
        let viewport_width = aspect_ratio * viewport_height;

        let basis = Basis::new(look_from, look_at, vup);
        let origin = basis.origin;
        let horizontal = focus_dist * viewport_width * basis.u;
        let vertical = focus_dist * viewport_height * basis.v;
        let lower_left = origin - horizontal / 2.0 - vertical / 2.0 - focus_dist * basis.w;

        PerspectiveCamera { origin, lower_left, horizontal, vertical, u: basis.u, v: basis.v, lens_radius: aperture / 2.0 }
    }

    fn get_ray(&self, s: f64, t: f64, rng: &mut impl Rng) -> Ray {
        let lens = self.lens_radius * Vec3::random_in_unit_disk(rng);
        let offset = self.u * lens.x + self.v * lens.y;
        Ray::new(self.origin + offset, self.lower_left + s * self.horizontal + t * self.vertical - self.origin - offset)
    }
}

/// Parallel-projection camera: every ray leaves parallel to `forward`, only
/// the *origin* sweeps across the viewport plane. That's the whole
/// difference from `PerspectiveCamera` - compare `get_ray` to see it: no
/// `- self.origin` reshaping a fixed apex into a spread of directions,
/// because there is no apex.
pub struct OrthographicCamera {
    lower_left: Point3,
    horizontal: Vec3,
    vertical: Vec3,
    forward: Vec3,
}

impl OrthographicCamera {
    fn new(look_from: Point3, look_at: Point3, vup: Vec3, height: f64, aspect_ratio: f64) -> Self {
        let viewport_height = height;
        let viewport_width = aspect_ratio * viewport_height;

        let basis = Basis::new(look_from, look_at, vup);
        let horizontal = viewport_width * basis.u;
        let vertical = viewport_height * basis.v;
        let lower_left = basis.origin - horizontal / 2.0 - vertical / 2.0;

        OrthographicCamera { lower_left, horizontal, vertical, forward: -basis.w }
    }

    fn get_ray(&self, s: f64, t: f64) -> Ray {
        let origin = self.lower_left + s * self.horizontal + t * self.vertical;
        Ray::new(origin, self.forward)
    }
}

/// Maps normalized viewport coordinates to an angle-from-axis `theta` and an
/// azimuth (as its cos/sin directly, sidestepping an `atan2` + re-`cos`/`sin`
/// round trip) around it. `aspect_ratio` is folded into `x` so a step of one
/// image-space unit covers the same angle horizontally and vertically -
/// otherwise a non-square frame would fisheye unevenly on its two axes.
fn radial_axes(s: f64, t: f64, aspect_ratio: f64) -> (f64, f64, f64) {
    let x = (2.0 * s - 1.0) * aspect_ratio;
    let y = 2.0 * t - 1.0;
    let r = (x * x + y * y).sqrt();
    if r < 1e-9 {
        (r, 1.0, 0.0) // On-axis: azimuth is undefined, so pick one arbitrarily.
    } else {
        (r, x / r, y / r)
    }
}

pub struct FisheyeCamera {
    origin: Point3,
    u: Vec3,
    v: Vec3,
    w: Vec3,
    aspect_ratio: f64,
    /// Half the total angular field of view, in radians.
    half_fov: f64,
}

impl FisheyeCamera {
    fn new(look_from: Point3, look_at: Point3, vup: Vec3, fov_degrees: f64, aspect_ratio: f64) -> Self {
        let basis = Basis::new(look_from, look_at, vup);
        FisheyeCamera {
            origin: basis.origin,
            u: basis.u,
            v: basis.v,
            w: basis.w,
            aspect_ratio,
            half_fov: fov_degrees.to_radians() / 2.0,
        }
    }

    fn get_ray(&self, s: f64, t: f64) -> Ray {
        let (r, cos_phi, sin_phi) = radial_axes(s, t, self.aspect_ratio);
        // Equidistant: angle from the view axis grows linearly with radius.
        let theta = r * self.half_fov;
        let dir = -self.w * theta.cos() + (self.u * cos_phi + self.v * sin_phi) * theta.sin();
        Ray::new(self.origin, dir)
    }
}

pub struct StereographicCamera {
    origin: Point3,
    u: Vec3,
    v: Vec3,
    w: Vec3,
    aspect_ratio: f64,
    /// Stereographic radius (`2 tan(theta/2)`) at the frame edge, i.e. at
    /// `theta = half_fov`. Scales the unit-square radius up into the same
    /// units before inverting the stereographic mapping back to an angle.
    edge_radius: f64,
}

impl StereographicCamera {
    fn new(look_from: Point3, look_at: Point3, vup: Vec3, fov_degrees: f64, aspect_ratio: f64) -> Self {
        let basis = Basis::new(look_from, look_at, vup);
        let half_fov = fov_degrees.to_radians() / 2.0;
        StereographicCamera {
            origin: basis.origin,
            u: basis.u,
            v: basis.v,
            w: basis.w,
            aspect_ratio,
            edge_radius: 2.0 * (half_fov / 2.0).tan(),
        }
    }

    fn get_ray(&self, s: f64, t: f64) -> Ray {
        let (r, cos_phi, sin_phi) = radial_axes(s, t, self.aspect_ratio);
        let r_stereo = r * self.edge_radius;
        // Invert r = 2 tan(theta / 2) for theta. Grows *faster* than the
        // equidistant fisheye's linear mapping as theta approaches/passes 90
        // degrees, which is what compresses a very wide field of view down
        // into a bounded image radius - and what makes the compression bite
        // hardest right at the frame edge, curling a wide horizon into a ring.
        let theta = 2.0 * (r_stereo / 2.0).atan();
        let dir = -self.w * theta.cos() + (self.u * cos_phi + self.v * sin_phi) * theta.sin();
        Ray::new(self.origin, dir)
    }
}

pub struct EquirectangularCamera {
    origin: Point3,
    u: Vec3,
    v: Vec3,
    w: Vec3,
}

impl EquirectangularCamera {
    fn new(look_from: Point3, look_at: Point3, vup: Vec3) -> Self {
        let basis = Basis::new(look_from, look_at, vup);
        EquirectangularCamera { origin: basis.origin, u: basis.u, v: basis.v, w: basis.w }
    }

    fn get_ray(&self, s: f64, t: f64) -> Ray {
        // Longitude sweeps a full turn across s (0 = straight ahead, ±pi =
        // straight behind, where the left/right edges of the image meet).
        // Latitude sweeps pole-to-pole across t.
        let phi = (2.0 * s - 1.0) * std::f64::consts::PI;
        let lat = (2.0 * t - 1.0) * (std::f64::consts::PI / 2.0);
        let dir = self.u * (lat.cos() * phi.sin()) + self.v * lat.sin() - self.w * (lat.cos() * phi.cos());
        Ray::new(self.origin, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: Vec3, b: Vec3, eps: f64) {
        assert!((a - b).length() < eps, "{a:?} != {b:?}");
    }

    #[test]
    fn perspective_matches_hand_derived_original_formula() {
        // Regression guard for the hard backward-compatibility requirement:
        // re-derive lower_left/horizontal/vertical exactly the way the
        // pre-refactor single-struct Camera::new did, and check get_ray
        // agrees to the bit (well within float noise).
        let (look_from, look_at, vup) = (Point3::new(0.0, 0.8, 2.5), Point3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        let (vfov, aspect) = (45.0_f64, 16.0 / 9.0);

        let theta = vfov.to_radians();
        let h = (theta / 2.0).tan();
        let viewport_height = 2.0 * h;
        let viewport_width = aspect * viewport_height;
        let w = (look_from - look_at).normalized();
        let u = vup.cross(w).normalized();
        let v = w.cross(u);
        let origin = look_from;
        let horizontal = viewport_width * u;
        let vertical = viewport_height * v;
        let lower_left = origin - horizontal / 2.0 - vertical / 2.0 - w;
        let expected = Ray::new(origin, lower_left + 0.3 * horizontal + 0.7 * vertical - origin);

        let cam = Camera::new(look_from, look_at, vup, vfov, aspect);
        let actual = cam.get_ray(0.3, 0.7, &mut rand::thread_rng());
        assert_eq!(actual.origin, expected.origin);
        assert_eq!(actual.direction, expected.direction);
    }

    #[test]
    fn perspective_center_ray_points_at_look_at() {
        let look_from = Point3::new(0.0, 0.0, 5.0);
        let look_at = Point3::new(0.0, 0.0, 0.0);
        let cam = Camera::new(look_from, look_at, Vec3::new(0.0, 1.0, 0.0), 60.0, 1.0);
        let ray = cam.get_ray(0.5, 0.5, &mut rand::thread_rng());
        assert_close(ray.direction.normalized(), (look_at - look_from).normalized(), 1e-9);
    }

    #[test]
    fn orthographic_rays_share_one_direction_but_fan_out_in_origin() {
        let cam = Camera::new_orthographic(Point3::new(0.0, 0.0, 5.0), Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0, 1.0);
        let a = cam.get_ray(0.0, 0.0, &mut rand::thread_rng());
        let b = cam.get_ray(1.0, 1.0, &mut rand::thread_rng());
        assert_close(a.direction, b.direction, 1e-9);
        assert!((a.origin - b.origin).length() > 1.0, "corner rays should originate far apart");
    }

    #[test]
    fn fisheye_center_matches_perspective_center() {
        let look_from = Point3::new(0.0, 0.0, 5.0);
        let look_at = Point3::new(0.0, 0.0, 0.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let cam = Camera::new_fisheye(look_from, look_at, vup, 180.0, 1.0);
        let ray = cam.get_ray(0.5, 0.5, &mut rand::thread_rng());
        assert_close(ray.direction.normalized(), (look_at - look_from).normalized(), 1e-9);
    }

    #[test]
    fn fisheye_edge_ray_bends_by_half_the_field_of_view() {
        let look_from = Point3::new(0.0, 0.0, 5.0);
        let look_at = Point3::new(0.0, 0.0, 0.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let fov = 180.0_f64;
        let cam = Camera::new_fisheye(look_from, look_at, vup, fov, 1.0);
        let forward = (look_at - look_from).normalized();
        let top = cam.get_ray(0.5, 1.0, &mut rand::thread_rng()).direction.normalized();
        let angle = top.dot(forward).clamp(-1.0, 1.0).acos();
        assert!((angle - (fov / 2.0).to_radians()).abs() < 1e-6);
    }

    #[test]
    fn stereographic_center_matches_perspective_center() {
        let look_from = Point3::new(0.0, 0.0, 5.0);
        let look_at = Point3::new(0.0, 0.0, 0.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let cam = Camera::new_stereographic(look_from, look_at, vup, 250.0, 1.0);
        let ray = cam.get_ray(0.5, 0.5, &mut rand::thread_rng());
        assert_close(ray.direction.normalized(), (look_at - look_from).normalized(), 1e-9);
    }

    #[test]
    fn equirectangular_forward_direction_at_frame_center() {
        let look_from = Point3::new(1.0, 2.0, 3.0);
        let look_at = Point3::new(1.0, 2.0, 0.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let cam = Camera::new_equirectangular(look_from, look_at, vup);
        let ray = cam.get_ray(0.5, 0.5, &mut rand::thread_rng());
        assert_close(ray.direction.normalized(), (look_at - look_from).normalized(), 1e-9);
    }

    #[test]
    fn equirectangular_directions_all_stay_unit_length() {
        let cam = Camera::new_equirectangular(Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        for i in 0..=10 {
            for j in 0..=10 {
                let (s, t) = (i as f64 / 10.0, j as f64 / 10.0);
                let dir = cam.get_ray(s, t, &mut rand::thread_rng()).direction;
                assert!((dir.length() - 1.0).abs() < 1e-9, "s={s} t={t} len={}", dir.length());
            }
        }
    }

    #[test]
    fn zero_aperture_thin_lens_matches_pinhole() {
        let (look_from, look_at, vup) = (Point3::new(0.0, 0.8, 2.5), Point3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        let pinhole = Camera::new(look_from, look_at, vup, 45.0, 16.0 / 9.0);
        let thin_lens = Camera::new_thin_lens(look_from, look_at, vup, 45.0, 16.0 / 9.0, 0.0, 1.0);
        // Aperture 0 => lens_radius 0 => the disk offset is always exactly
        // zero, whatever the RNG draws - so this must hold for any seed.
        for _ in 0..20 {
            let a = pinhole.get_ray(0.37, 0.62, &mut rand::thread_rng());
            let b = thin_lens.get_ray(0.37, 0.62, &mut rand::thread_rng());
            assert_eq!(a.origin, b.origin);
            assert_eq!(a.direction, b.direction);
        }
    }

    #[test]
    fn nonzero_aperture_scatters_ray_origins_but_keeps_focus_point() {
        let (look_from, look_at, vup) = (Point3::new(0.0, 0.0, 5.0), Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let focus_dist = 5.0;
        let cam = Camera::new_thin_lens(look_from, look_at, vup, 40.0, 1.0, 0.5, focus_dist);
        let mut rng = rand::thread_rng();

        let mut origins_differ = false;
        let mut focus_points = Vec::new();
        for _ in 0..50 {
            let ray = cam.get_ray(0.5, 0.5, &mut rng);
            if (ray.origin - look_from).length() > 1e-9 {
                origins_differ = true;
            }
            // All rays through the same (s, t) should converge back to the
            // same point on the focus plane, regardless of lens offset.
            // Convergence happens at ray parameter 1, not `focus_dist`
            // itself - horizontal/vertical/lower_left are already scaled by
            // focus_dist when the camera is built, so `direction` reaches
            // the focus plane after traveling its own full length once.
            focus_points.push(ray.at(1.0));
        }
        assert!(origins_differ, "nonzero aperture should jitter ray origins across samples");
        for p in &focus_points[1..] {
            assert_close(*p, focus_points[0], 1e-6);
        }
    }
}
