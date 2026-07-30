//! Procedurally generates a low-poly hexagonal bipyramid ("crystal point")
//! OBJ mesh, used as a test asset for the mesh/BVH/OBJ-loading pipeline and
//! as a stand-in gem shape for the glitch gallery.
use anyhow::Result;
use lumin::vec3::Point3;
use std::f64::consts::PI;
use std::fmt::Write as _;

fn main() -> Result<()> {
    let sides: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(6);
    let out_path = std::env::args().nth(2).unwrap_or_else(|| "assets/crystal.obj".to_string());
    let radius = 0.6;
    let top = Point3::new(0.0, 1.1, 0.0);
    let bottom = Point3::new(0.0, -1.1, 0.0);
    let center = Point3::ZERO;

    let ring: Vec<Point3> = (0..sides)
        .map(|i| {
            let theta = 2.0 * PI * (i as f64) / (sides as f64);
            Point3::new(radius * theta.cos(), 0.0, radius * theta.sin())
        })
        .collect();

    let mut faces: Vec<[Point3; 3]> = Vec::new();
    for i in 0..sides {
        let a = ring[i];
        let b = ring[(i + 1) % sides];
        faces.push([a, b, top]);
        faces.push([b, a, bottom]);
    }

    // Don't trust hand-derived winding order: compute each face's normal and
    // flip it if it points inward, so the mesh is correct regardless.
    for face in &mut faces {
        let [a, b, c] = *face;
        let normal = (b - a).cross(c - a);
        let centroid = (a + b + c) / 3.0;
        if normal.dot(centroid - center) < 0.0 {
            face.swap(1, 2);
        }
    }

    let mut obj = String::new();
    writeln!(obj, "# procedurally generated hexagonal bipyramid")?;
    for face in &faces {
        for v in face {
            writeln!(obj, "v {} {} {}", v.x, v.y, v.z)?;
        }
    }
    let mut idx = 1;
    for _ in &faces {
        writeln!(obj, "f {} {} {}", idx, idx + 1, idx + 2)?;
        idx += 3;
    }

    std::fs::create_dir_all("assets")?;
    std::fs::write(&out_path, &obj)?;
    println!("wrote {out_path} ({} triangles, {sides} sides)", faces.len());
    Ok(())
}
