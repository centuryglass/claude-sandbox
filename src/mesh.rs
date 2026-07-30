use crate::bvh::BvhNode;
use crate::hittable::Hittable;
use crate::material::Material;
use crate::triangle::Triangle;
use crate::vec3::{Point3, Vec3};
use anyhow::{Context, Result};
use std::path::Path;

/// Loads a Wavefront OBJ file into a BVH-accelerated triangle mesh.
/// Supports `v`, `vn`, and `f` records; faces with >3 vertices are
/// fan-triangulated. Texture coordinates (`vt`) are parsed but discarded -
/// this renderer has no texturing yet. All faces share one material and are
/// transformed by a uniform `scale` + `translate` applied at load time.
pub fn load_obj(
    path: impl AsRef<Path>,
    material: Material,
    scale: f64,
    translate: Vec3,
) -> Result<Box<dyn Hittable>> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading OBJ file {}", path.display()))?;

    let mut positions: Vec<Point3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut triangles: Vec<Box<dyn Hittable>> = Vec::new();

    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        let mut tokens = line.split_whitespace();
        let Some(tag) = tokens.next() else { continue };

        match tag {
            "v" => {
                let xyz = parse_floats::<3>(tokens.by_ref())
                    .with_context(|| format!("{}:{}: malformed `v`", path.display(), line_no + 1))?;
                positions.push(Point3::new(xyz[0], xyz[1], xyz[2]) * scale + translate);
            }
            "vn" => {
                let xyz = parse_floats::<3>(tokens.by_ref())
                    .with_context(|| format!("{}:{}: malformed `vn`", path.display(), line_no + 1))?;
                normals.push(Vec3::new(xyz[0], xyz[1], xyz[2]).normalized());
            }
            "f" => {
                let refs: Vec<VertexRef> = tokens
                    .map(parse_face_token)
                    .collect::<Result<_>>()
                    .with_context(|| format!("{}:{}: malformed `f`", path.display(), line_no + 1))?;
                if refs.len() < 3 {
                    continue;
                }
                // Fan triangulation: (0, i, i+1) for i in 1..len-1.
                for i in 1..refs.len() - 1 {
                    let tri = build_triangle(&positions, &normals, refs[0], refs[i], refs[i + 1], material)
                        .with_context(|| format!("{}:{}: face vertex index out of range", path.display(), line_no + 1))?;
                    triangles.push(Box::new(tri));
                }
            }
            _ => {} // vt, comments, groups, materials, etc. - not needed yet.
        }
    }

    anyhow::ensure!(!triangles.is_empty(), "OBJ file {} contained no triangles", path.display());
    Ok(BvhNode::build(triangles))
}

#[derive(Clone, Copy)]
struct VertexRef {
    pos: i64,
    normal: Option<i64>,
}

/// Parses an OBJ face token: `v`, `v/vt`, `v//vn`, or `v/vt/vn` (1-indexed,
/// negative = relative to the end of the list so far).
fn parse_face_token(tok: &str) -> Result<VertexRef> {
    let mut parts = tok.split('/');
    let pos = parts
        .next()
        .filter(|s| !s.is_empty())
        .context("missing vertex position index")?
        .parse::<i64>()?;
    let _vt = parts.next(); // texture coord index, unused
    let normal = match parts.next() {
        Some(s) if !s.is_empty() => Some(s.parse::<i64>()?),
        _ => None,
    };
    Ok(VertexRef { pos, normal })
}

fn resolve_index(idx: i64, len: usize) -> Option<usize> {
    if idx > 0 {
        usize::try_from(idx - 1).ok().filter(|&i| i < len)
    } else if idx < 0 {
        len.checked_sub((-idx) as usize)
    } else {
        None
    }
}

fn build_triangle(
    positions: &[Point3],
    normals: &[Vec3],
    a: VertexRef,
    b: VertexRef,
    c: VertexRef,
    material: Material,
) -> Result<Triangle> {
    let get_pos = |r: VertexRef| -> Result<Point3> {
        resolve_index(r.pos, positions.len())
            .map(|i| positions[i])
            .context("vertex index out of range")
    };
    let v0 = get_pos(a)?;
    let v1 = get_pos(b)?;
    let v2 = get_pos(c)?;

    let tri = Triangle::new(v0, v1, v2, material);
    let get_normal = |r: VertexRef| resolve_index(r.normal?, normals.len()).map(|i| normals[i]);
    match (get_normal(a), get_normal(b), get_normal(c)) {
        (Some(n0), Some(n1), Some(n2)) => Ok(tri.with_normals(n0, n1, n2)),
        _ => Ok(tri),
    }
}

fn parse_floats<'a, const N: usize>(tokens: impl Iterator<Item = &'a str>) -> Result<[f64; N]> {
    let vals: Vec<f64> = tokens.take(N).map(str::parse).collect::<Result<_, _>>()?;
    vals.try_into()
        .map_err(|v: Vec<f64>| anyhow::anyhow!("expected {N} numbers, got {}", v.len()))
}
