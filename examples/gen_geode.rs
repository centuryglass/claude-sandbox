//! Procedurally generates a low-poly, irregularly-lumped hollow "geode"
//! shell OBJ mesh: start from an icosahedron, subdivide it into a geodesic
//! sphere, then nudge each vertex in or out by a handful of smooth radial
//! "bumps" so the inside reads as an irregular cave rather than a perfect
//! sphere. Built to be large and open enough that a camera can sit *inside*
//! it - see `scenes/geode_interior.ron`.
use anyhow::Result;
use lumin::vec3::Point3;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::fmt::Write as _;

/// Deterministic hash -> [0, 1), no crate needed. Same spirit as the hash in
/// `src/texture.rs`, just keyed by an arbitrary "channel" number instead of
/// a fixed x/y/z lattice coordinate.
fn hash(seed: u64, channel: u64) -> f64 {
    let mut h = seed.wrapping_add(channel.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    (h & 0xFFFFFF) as f64 / 0xFFFFFF as f64
}

/// One smooth radial bump (positive = outward bulge, negative = inward
/// dimple), applied as a Gaussian falloff in angular distance from `center`.
/// A handful of these summed together is enough to turn a perfect sphere
/// into something that reads as a lumpy, irregular cavity.
struct Bump {
    center: Point3, // unit direction
    amplitude: f64,
    sigma: f64, // angular width, radians
}

fn make_bumps(seed: u64, count: usize) -> Vec<Bump> {
    (0..count)
        .map(|i| {
            let s = seed.wrapping_add(i as u64 * 0x1000_0001);
            // theta = polar angle from +y, phi = azimuth; not a perfectly
            // uniform sphere sampling, but plenty even for a handful of bumps.
            let theta = hash(s, 0) * PI;
            let phi = hash(s, 1) * 2.0 * PI;
            let center = Point3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
            let amplitude = (hash(s, 2) * 2.0 - 1.0) * 0.55;
            let sigma = 0.18 + hash(s, 3) * 0.4;
            Bump { center, amplitude, sigma }
        })
        .collect()
}

/// Standard spherical UV (same formula as `Sphere`'s in `src/sphere.rs`,
/// duplicated rather than imported since it's a private helper there).
/// Takes the vertex's *undisplaced* unit direction, not its final bumped
/// position, so the radial bumps don't skew the mapping - only where a
/// point sits angularly on the base sphere determines its UV, matching how
/// a real geode's surface texture would follow the underlying rock rather
/// than stretching over every bump.
fn uv_for(dir: Point3) -> (f64, f64) {
    let theta = (-dir.y).acos();
    let phi = (-dir.z).atan2(dir.x) + PI;
    (phi / (2.0 * PI), theta / PI)
}

fn displaced_radius(dir: Point3, bumps: &[Bump]) -> f64 {
    let mut r = 1.0;
    for bump in bumps {
        let cos_angle = dir.dot(bump.center).clamp(-1.0, 1.0);
        let angle = cos_angle.acos();
        r += bump.amplitude * (-(angle * angle) / (2.0 * bump.sigma * bump.sigma)).exp();
    }
    r
}

fn icosahedron() -> (Vec<Point3>, Vec<[usize; 3]>) {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let raw = [
        (-1.0, phi, 0.0), (1.0, phi, 0.0), (-1.0, -phi, 0.0), (1.0, -phi, 0.0),
        (0.0, -1.0, phi), (0.0, 1.0, phi), (0.0, -1.0, -phi), (0.0, 1.0, -phi),
        (phi, 0.0, -1.0), (phi, 0.0, 1.0), (-phi, 0.0, -1.0), (-phi, 0.0, 1.0),
    ];
    let verts: Vec<Point3> = raw.iter().map(|&(x, y, z)| Point3::new(x, y, z).normalized()).collect();
    let faces: Vec<[usize; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    (verts, faces)
}

/// Looks up (or creates) the vertex for the midpoint of edge `(a, b)`,
/// re-projected onto the unit sphere - the standard geodesic-sphere
/// subdivision trick. Cached so shared edges between adjacent faces don't
/// produce duplicate vertices (and therefore don't crack the mesh apart).
fn get_or_add_midpoint(a: usize, b: usize, verts: &mut Vec<Point3>, cache: &mut HashMap<(usize, usize), usize>) -> usize {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&idx) = cache.get(&key) {
        return idx;
    }
    let mid = ((verts[a] + verts[b]) * 0.5).normalized();
    verts.push(mid);
    let idx = verts.len() - 1;
    cache.insert(key, idx);
    idx
}

fn subdivide(mut verts: Vec<Point3>, faces: Vec<[usize; 3]>) -> (Vec<Point3>, Vec<[usize; 3]>) {
    let mut cache = HashMap::new();
    let mut new_faces = Vec::with_capacity(faces.len() * 4);
    for [a, b, c] in faces {
        let ab = get_or_add_midpoint(a, b, &mut verts, &mut cache);
        let bc = get_or_add_midpoint(b, c, &mut verts, &mut cache);
        let ca = get_or_add_midpoint(c, a, &mut verts, &mut cache);
        new_faces.push([a, ab, ca]);
        new_faces.push([b, bc, ab]);
        new_faces.push([c, ca, bc]);
        new_faces.push([ab, bc, ca]);
    }
    (verts, new_faces)
}

fn main() -> Result<()> {
    let subdivisions: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let out_path = std::env::args().nth(2).unwrap_or_else(|| "assets/geode.obj".to_string());
    // Fixed seed so regenerating the asset is reproducible, same spirit as
    // gen_crystal.rs hardcoding its dimensions rather than exposing a seed.
    let seed: u64 = 0x6c75_6d69_6e00_0007;
    let bump_count = 18;

    let (mut verts, mut faces) = icosahedron();
    for _ in 0..subdivisions {
        (verts, faces) = subdivide(verts, faces);
    }

    let bumps = make_bumps(seed, bump_count);
    let displaced: Vec<Point3> = verts.iter().map(|&d| d * displaced_radius(d, &bumps)).collect();

    let center = Point3::ZERO;
    // Each vertex carries its displaced position (for geometry) and its
    // original undisplaced unit direction (for UV) as a pair, so a winding
    // swap moves both together and they never get mismatched.
    let mut tri_faces: Vec<[(Point3, Point3); 3]> =
        faces.iter().map(|&[a, b, c]| [(displaced[a], verts[a]), (displaced[b], verts[b]), (displaced[c], verts[c])]).collect();

    // Don't trust hand-derived winding order (same trick as gen_crystal.rs):
    // compute each face's normal and flip it if it points inward relative to
    // the mesh's overall center. The radial bumps stay small relative to the
    // base radius, so every face's centroid is still roughly star-shaped
    // around the origin and one global reference point is enough.
    for face in &mut tri_faces {
        let [(a, _), (b, _), (c, _)] = *face;
        let normal = (b - a).cross(c - a);
        let centroid = (a + b + c) / 3.0;
        if normal.dot(centroid - center) < 0.0 {
            face.swap(1, 2);
        }
    }

    let mut obj = String::new();
    writeln!(obj, "# procedurally generated geodesic geode / cave shell")?;
    for face in &tri_faces {
        for (p, _) in face {
            writeln!(obj, "v {} {} {}", p.x, p.y, p.z)?;
        }
        for (_, dir) in face {
            let (u, v) = uv_for(*dir);
            writeln!(obj, "vt {u} {v}")?;
        }
    }
    let mut idx = 1;
    for _ in &tri_faces {
        writeln!(obj, "f {i0}/{i0} {i1}/{i1} {i2}/{i2}", i0 = idx, i1 = idx + 1, i2 = idx + 2)?;
        idx += 3;
    }

    std::fs::create_dir_all("assets")?;
    std::fs::write(&out_path, &obj)?;
    println!(
        "wrote {out_path} ({} triangles, {subdivisions} subdivisions, {bump_count} bumps)",
        tri_faces.len()
    );
    Ok(())
}
