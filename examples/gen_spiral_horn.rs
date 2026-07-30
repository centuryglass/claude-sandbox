//! Procedurally generates a low-poly, fluted, spiraling "horn" OBJ mesh: a
//! generalized cylinder swept along a helical spine, its radius flaring from
//! a tight closed tip to a wide open bell, with a star/gear-shaped
//! cross-section for extra facets. The bell end is deliberately left open -
//! large and roomy enough that a camera can sit deep inside the throat,
//! looking back out through the twisting walls - see
//! `scenes/spiral_horn_interior.ron`.
use anyhow::Result;
use lumin::vec3::{Point3, Vec3};
use std::f64::consts::TAU;
use std::fmt::Write as _;

/// A point + local tangent + tube radius + self-twist on the spine curve, at
/// parameter `t` in `[0, 1]` from tip to bell.
struct SpineSample {
    center: Point3,
    tangent: Vec3,
    tube_radius: f64,
    twist: f64,
}

fn spine_point(t: f64, turns: f64, length: f64, helix_r0: f64, helix_r1: f64) -> Point3 {
    let theta = turns * TAU * t;
    let helix_r = helix_r0 + (helix_r1 - helix_r0) * t;
    Point3::new(helix_r * theta.cos(), length * t, helix_r * theta.sin())
}

fn spine_sample(t: f64, turns: f64, length: f64, helix_r0: f64, helix_r1: f64, tube_r0: f64, tube_r1: f64, self_twists: f64) -> SpineSample {
    let center = spine_point(t, turns, length, helix_r0, helix_r1);

    // Central-difference tangent; everything here is a smooth analytic
    // curve, so a small dt is plenty accurate without needing a real
    // derivative.
    let dt = 1e-4;
    let p0 = spine_point((t - dt).max(0.0), turns, length, helix_r0, helix_r1);
    let p1 = spine_point((t + dt).min(1.0), turns, length, helix_r0, helix_r1);
    let tangent = (p1 - p0).normalized();

    let tube_radius = tube_r0 + (tube_r1 - tube_r0) * t.powf(1.3);
    let twist = self_twists * TAU * t;
    SpineSample { center, tangent, tube_radius, twist }
}

/// A mesh vertex's position paired with its `(u, v)` texture coordinate, so
/// winding fixes and everything else move both together and they can never
/// end up mismatched.
type Vertex = (Point3, (f64, f64));

/// Same winding-safety trick as `gen_crystal.rs` - compute the face normal
/// and flip it if it points the wrong way - just generalized to take an
/// explicit "this point is inside the solid" reference instead of one global
/// center. A spiral tube isn't star-shaped around a single point, but each
/// short local segment of it is, so a nearby reference point is enough.
fn fix_winding(mut tri: [Vertex; 3], inward_ref: Point3) -> [Vertex; 3] {
    let [(a, _), (b, _), (c, _)] = tri;
    let normal = (b - a).cross(c - a);
    let centroid = (a + b + c) / 3.0;
    if normal.dot(centroid - inward_ref) < 0.0 {
        tri.swap(1, 2);
    }
    tri
}

fn main() -> Result<()> {
    let sides: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(7);
    let rings: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(56);
    let out_path = std::env::args().nth(3).unwrap_or_else(|| "assets/spiral_horn.obj".to_string());

    // Shape constants, tuned by eye for a kudu-horn-ish silhouette: a tight
    // spiral near the tip opening out into a wide, gently-turning bell.
    let turns = 1.5;
    let length = 4.2;
    let helix_r0 = 0.12;
    let helix_r1 = 1.15;
    let tube_r0 = 0.05;
    let tube_r1 = 0.72;
    let self_twists = 2.0; // extra rotation of the fluted cross-section along the length
    let flute_depth = 0.16;

    let samples: Vec<SpineSample> = (0..rings)
        .map(|i| {
            let t = i as f64 / (rings - 1) as f64;
            spine_sample(t, turns, length, helix_r0, helix_r1, tube_r0, tube_r1, self_twists)
        })
        .collect();

    let world_up = Vec3::new(0.0, 1.0, 0.0);
    let ring_points: Vec<Vec<Point3>> = samples
        .iter()
        .map(|s| {
            // Build a frame perpendicular to the tangent. The spiral's own
            // helix radius is never quite zero, so the tangent is never
            // exactly vertical - the world_up fallback below is just a
            // safety net, not something this particular curve hits.
            let up_ref = if s.tangent.dot(world_up).abs() > 0.98 { Vec3::new(1.0, 0.0, 0.0) } else { world_up };
            let right = s.tangent.cross(up_ref).normalized();
            let up = right.cross(s.tangent).normalized();
            (0..sides)
                .map(|j| {
                    let angle = s.twist + TAU * j as f64 / sides as f64;
                    // Frequency sides/2 (not sides itself, which would just
                    // shift the whole ring uniformly): alternates radius
                    // vertex-to-vertex, giving a star/gear cross-section
                    // instead of a smooth circle.
                    let flute = 1.0 + flute_depth * (angle * sides as f64 / 2.0).cos();
                    let r = s.tube_radius * flute;
                    s.center + right * (angle.cos() * r) + up * (angle.sin() * r)
                })
                .collect()
        })
        .collect();

    let mut faces: Vec<[Vertex; 3]> = Vec::new();

    // UV: u wraps once around the tube's cross-section, v runs 0 (tip) to 1
    // (bell). `j` past the last side uses a continuous (non-wrapped) u so
    // the seam face stretches from u=1-1/sides to a full u=1 instead of
    // snapping back to 0.
    let ring_v = |i: usize| i as f64 / (rings - 1) as f64;
    let side_u = |j: usize| j as f64 / sides as f64;

    // Tube wall: two triangles per side per ring segment.
    for i in 0..rings - 1 {
        let local_ref = (samples[i].center + samples[i + 1].center) * 0.5;
        for j in 0..sides {
            let j2 = (j + 1) % sides;
            let (u0, u1) = (side_u(j), side_u(j + 1));
            let (v0, v1) = (ring_v(i), ring_v(i + 1));
            let a = (ring_points[i][j], (u0, v0));
            let b = (ring_points[i][j2], (u1, v0));
            let c = (ring_points[i + 1][j2], (u1, v1));
            let d = (ring_points[i + 1][j], (u0, v1));
            faces.push(fix_winding([a, b, c], local_ref));
            faces.push(fix_winding([a, c, d], local_ref));
        }
    }

    // Closed tip: fan the first ring in to a point just beyond it. The wide
    // end is deliberately left open - that's the bell a camera looks out
    // through from inside. The tip's own u is the midpoint of each face's
    // two ring vertices, avoiding unnecessary UV distortion away from the
    // one unavoidable seam (same idea as `gen_crystal.rs`'s apexes).
    let tip = samples[0].center - samples[0].tangent * (samples[0].tube_radius * 2.0);
    let tip_ref = samples[1].center;
    for j in 0..sides {
        let j2 = (j + 1) % sides;
        let (u0, u1) = (side_u(j), side_u(j + 1));
        let tip_vertex = (tip, ((u0 + u1) / 2.0, 0.0));
        let a = (ring_points[0][j], (u0, ring_v(0)));
        let b = (ring_points[0][j2], (u1, ring_v(0)));
        faces.push(fix_winding([tip_vertex, a, b], tip_ref));
    }

    let mut obj = String::new();
    writeln!(obj, "# procedurally generated spiral horn (fluted, open bell, closed tip)")?;
    for face in &faces {
        for (p, _) in face {
            writeln!(obj, "v {} {} {}", p.x, p.y, p.z)?;
        }
        for (_, (u, v)) in face {
            writeln!(obj, "vt {u} {v}")?;
        }
    }
    let mut idx = 1;
    for _ in &faces {
        writeln!(obj, "f {i0}/{i0} {i1}/{i1} {i2}/{i2}", i0 = idx, i1 = idx + 1, i2 = idx + 2)?;
        idx += 3;
    }

    std::fs::create_dir_all("assets")?;
    std::fs::write(&out_path, &obj)?;
    println!("wrote {out_path} ({} triangles, {sides} sides x {rings} rings)", faces.len());
    Ok(())
}
