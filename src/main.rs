use clap::Parser;
use lumin::camera::Camera;
use lumin::glitch::GlitchMode;
use lumin::hittable::Hittable;
use lumin::light::Light;
use lumin::material::Material;
use lumin::mesh;
use lumin::render::{self, RenderSettings};
use lumin::scene::Scene;
use lumin::scene_desc::SceneDesc;
use lumin::sphere::Sphere;
use lumin::triangle::Triangle;
use lumin::vec3::{Color, Point3, Vec3};
use std::path::PathBuf;

const MAX_DEPTH: u32 = 12;
const SAMPLES_PER_PIXEL: u32 = 256;

/// lumin: a small ray tracer, plus a gallery of deliberately broken
/// reflection/refraction shaders. With no arguments, renders the built-in
/// demo gallery (baseline scenes + every glitch mode). Given a scene file,
/// renders just that scene.
#[derive(Parser)]
#[command(name = "lumin", version)]
struct Cli {
    /// RON scene file to render (see scenes/*.ron for examples). If
    /// omitted, runs the built-in demo/gallery instead.
    scene: Option<PathBuf>,

    /// Image width in pixels.
    #[arg(short, long, default_value_t = 800)]
    width: u32,

    /// Image height in pixels. Defaults to width / aspect.
    #[arg(long)]
    height: Option<u32>,

    /// Width / height.
    #[arg(long, default_value_t = 16.0 / 9.0)]
    aspect: f64,

    /// Samples per pixel (antialiasing / glass noise quality).
    #[arg(short, long, default_value_t = 128)]
    samples: u32,

    /// Maximum ray bounce depth.
    #[arg(short, long, default_value_t = 24)]
    depth: u32,

    /// Which glitch mode to render. Ignored if --gallery is set.
    #[arg(short, long, default_value = "none")]
    glitch: GlitchMode,

    /// Render every glitch mode instead of just --glitch, one file per mode.
    #[arg(long)]
    gallery: bool,

    /// Output file (single-scene mode) or directory (--gallery mode).
    #[arg(short, long, default_value = "renders/output.png")]
    output: PathBuf,
}

/// The pale blue sky used by the "honest" physically-based scenes.
const SKY_BOTTOM: Color = Color::new(1.0, 1.0, 1.0);
const SKY_TOP: Color = Color::new(0.5, 0.7, 1.0);

fn reflection_scene(aspect_ratio: f64) -> Scene {
    let objects: Vec<Box<dyn Hittable>> = vec![
        // Ground.
        Box::new(Sphere::new(
            Point3::new(0.0, -100.5, -1.0),
            100.0,
            Material::Lambertian { albedo: Color::new(0.6, 0.6, 0.65) },
        )),
        // Diffuse sphere on the left.
        Box::new(Sphere::new(
            Point3::new(-1.1, 0.0, -1.4),
            0.5,
            Material::Lambertian { albedo: Color::new(0.2, 0.6, 0.3) },
        )),
        // Polished metal sphere in the middle.
        Box::new(Sphere::new(
            Point3::new(0.0, 0.0, -1.2),
            0.5,
            Material::Metal { albedo: Color::new(0.85, 0.85, 0.9), fuzz: 0.02 },
        )),
        // Glass sphere on the right (solid, so it also reflects a bit via Fresnel).
        Box::new(Sphere::new(
            Point3::new(1.1, 0.0, -1.6),
            0.5,
            Material::Dielectric { ior: 1.5 },
        )),
        // Small brushed-metal sphere floating in front, to catch a few bounces.
        Box::new(Sphere::new(
            Point3::new(0.15, -0.3, -0.55),
            0.2,
            Material::Metal { albedo: Color::new(0.9, 0.7, 0.3), fuzz: 0.15 },
        )),
    ];

    let lights = vec![
        Light::Point { position: Point3::new(-4.0, 5.0, 2.0), color: Color::ONE, intensity: 40.0 },
        Light::Point { position: Point3::new(3.0, 3.0, 3.0), color: Color::new(0.6, 0.7, 1.0), intensity: 15.0 },
    ];

    let camera = Camera::new(
        Point3::new(0.0, 0.8, 2.5),
        Point3::new(0.0, 0.0, -1.0),
        Vec3::new(0.0, 1.0, 0.0),
        45.0,
        aspect_ratio,
    );

    Scene { objects, lights, ambient: Color::splat(0.08), camera, sky_bottom: SKY_BOTTOM, sky_top: SKY_TOP }
}

fn crystal_scene(aspect_ratio: f64) -> anyhow::Result<Scene> {
    let crystal = mesh::load_obj(
        "assets/crystal.obj",
        Material::Dielectric { ior: 1.7 },
        1.0,
        Point3::new(0.0, -0.02, -1.1),
    )?;

    let objects: Vec<Box<dyn Hittable>> = vec![
        Box::new(Sphere::new(
            Point3::new(0.0, -100.5, -1.0),
            100.0,
            Material::Lambertian { albedo: Color::new(0.55, 0.55, 0.6) },
        )),
        Box::new(Sphere::new(
            Point3::new(0.9, -0.15, -0.5),
            0.35,
            Material::Metal { albedo: Color::new(0.9, 0.75, 0.4), fuzz: 0.05 },
        )),
        Box::new(Sphere::new(
            Point3::new(-0.9, -0.2, -0.6),
            0.3,
            Material::Lambertian { albedo: Color::new(0.7, 0.2, 0.5) },
        )),
        crystal,
    ];

    let lights = vec![
        Light::Point { position: Point3::new(-3.0, 4.0, 2.0), color: Color::ONE, intensity: 35.0 },
        Light::Point { position: Point3::new(2.5, 2.5, 2.5), color: Color::new(0.7, 0.8, 1.0), intensity: 12.0 },
    ];

    let camera = Camera::new(
        Point3::new(0.0, 0.9, 2.6),
        Point3::new(0.0, 0.1, -1.0),
        Vec3::new(0.0, 1.0, 0.0),
        40.0,
        aspect_ratio,
    );

    Ok(Scene { objects, lights, ambient: Color::splat(0.08), camera, sky_bottom: SKY_BOTTOM, sky_top: SKY_TOP })
}

/// A cluster of crystals and mirror spheres, tuned to give reflective rays
/// plenty of chances to bounce a few times between facets - exactly the
/// condition under which the college-era kaleidoscope bug showed up. Vivid
/// sky so folded/duplicated background shows up clearly in the glitch modes.
/// Uses the denser 26-facet crystal (vs. the 6-facet one used for the
/// mesh-pipeline test render): more facets means more distinct normals for
/// a broken reflection axis to fragment light across, which reads as finer,
/// more kaleidoscope-like tiling.
fn glitch_scene(aspect_ratio: f64) -> anyhow::Result<Scene> {
    let big_crystal = mesh::load_obj(
        "assets/crystal_dense.obj",
        Material::Dielectric { ior: 1.8 },
        1.15,
        Point3::new(0.0, 0.15, -1.3),
    )?;
    let small_crystal = mesh::load_obj(
        "assets/crystal_dense.obj",
        Material::Dielectric { ior: 1.8 },
        0.5,
        Point3::new(-0.85, -0.35, -0.7),
    )?;

    let objects: Vec<Box<dyn Hittable>> = vec![
        Box::new(Sphere::new(
            Point3::new(0.0, -100.5, -1.0),
            100.0,
            Material::Lambertian { albedo: Color::new(0.5, 0.5, 0.55) },
        )),
        // Mirror sphere close to the crystals, so reflected/refracted rays
        // ping-pong between it and the facets for several bounces.
        Box::new(Sphere::new(
            Point3::new(0.95, -0.1, -0.55),
            0.45,
            Material::Metal { albedo: Color::new(0.92, 0.92, 0.95), fuzz: 0.0 },
        )),
        Box::new(Sphere::new(
            Point3::new(0.15, -0.35, -0.15),
            0.15,
            Material::Metal { albedo: Color::new(0.9, 0.75, 0.35), fuzz: 0.05 },
        )),
        big_crystal,
        small_crystal,
    ];

    let lights = vec![
        Light::Point { position: Point3::new(-3.5, 4.5, 2.5), color: Color::ONE, intensity: 40.0 },
        Light::Point { position: Point3::new(3.0, 2.0, 3.0), color: Color::new(1.0, 0.85, 0.6), intensity: 18.0 },
    ];

    let camera = Camera::new(
        Point3::new(0.0, 0.85, 2.4),
        Point3::new(0.0, 0.1, -1.0),
        Vec3::new(0.0, 1.0, 0.0),
        42.0,
        aspect_ratio,
    );

    Ok(Scene {
        objects,
        lights,
        ambient: Color::splat(0.1),
        camera,
        sky_bottom: Color::new(1.0, 0.85, 0.1),
        sky_top: Color::new(0.85, 0.15, 0.55),
    })
}

/// Builds a flat rectangular mirror from two triangles, given its four
/// corners in order around the perimeter. Winding is auto-corrected against
/// `desired_normal` so callers don't have to hand-derive it.
fn quad_facing(p0: Point3, p1: Point3, p2: Point3, p3: Point3, desired_normal: Vec3, material: Material) -> [Box<dyn Hittable>; 2] {
    let n = (p1 - p0).cross(p2 - p0);
    let (a, b, c, d) = if n.dot(desired_normal) < 0.0 { (p0, p3, p2, p1) } else { (p0, p1, p2, p3) };
    [Box::new(Triangle::new(a, b, c, material)), Box::new(Triangle::new(a, c, d, material))]
}

/// Two facing mirror walls with a crystal between them: the classic "hall of
/// mirrors" setup, where correct reflection produces a receding tunnel of
/// self-images. Any error in the reflection axis compounds far more
/// dramatically here than off a single isolated mirror, since each of the
/// (correctly) many bounces down the corridor re-exposes the same mistake.
fn hall_of_mirrors_scene(aspect_ratio: f64) -> anyhow::Result<Scene> {
    let crystal = mesh::load_obj(
        "assets/crystal.obj",
        Material::Dielectric { ior: 1.8 },
        0.8,
        Point3::new(0.0, 0.0, -1.4),
    )?;

    let mirror = Material::Metal { albedo: Color::new(0.95, 0.95, 0.97), fuzz: 0.0 };
    let (y0, y1) = (-0.5, 2.2);
    let (z_near, z_far) = (1.5, -6.0);
    let mut objects: Vec<Box<dyn Hittable>> = Vec::new();

    // Left wall (x = -1.6), facing +x.
    objects.extend(quad_facing(
        Point3::new(-1.6, y0, z_near),
        Point3::new(-1.6, y0, z_far),
        Point3::new(-1.6, y1, z_far),
        Point3::new(-1.6, y1, z_near),
        Vec3::new(1.0, 0.0, 0.0),
        mirror,
    ));
    // Right wall (x = 1.6), facing -x.
    objects.extend(quad_facing(
        Point3::new(1.6, y0, z_near),
        Point3::new(1.6, y0, z_far),
        Point3::new(1.6, y1, z_far),
        Point3::new(1.6, y1, z_near),
        Vec3::new(-1.0, 0.0, 0.0),
        mirror,
    ));
    objects.push(Box::new(Sphere::new(
        Point3::new(0.0, -100.5, -1.0),
        100.0,
        Material::Lambertian { albedo: Color::new(0.5, 0.5, 0.55) },
    )));
    objects.push(Box::new(Sphere::new(
        Point3::new(0.9, -0.15, -0.4),
        0.35,
        Material::Metal { albedo: Color::new(0.9, 0.75, 0.35), fuzz: 0.05 },
    )));
    objects.push(crystal);

    let lights = vec![
        Light::Point { position: Point3::new(0.0, 2.0, 1.0), color: Color::ONE, intensity: 25.0 },
        Light::Point { position: Point3::new(0.0, 1.5, -3.0), color: Color::new(1.0, 0.8, 0.6), intensity: 20.0 },
    ];

    let camera = Camera::new(
        Point3::new(0.0, 0.7, 2.2),
        Point3::new(0.0, 0.3, -1.5),
        Vec3::new(0.0, 1.0, 0.0),
        50.0,
        aspect_ratio,
    );

    Ok(Scene {
        objects,
        lights,
        ambient: Color::splat(0.1),
        camera,
        sky_bottom: Color::new(1.0, 0.85, 0.1),
        sky_top: Color::new(0.85, 0.15, 0.55),
    })
}

fn render_to(scene: &Scene, settings: &RenderSettings, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let img = render::render(scene, settings);
    img.save(path)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// The built-in demo: every baseline scene plus the full glitch gallery.
/// Run with no CLI arguments.
fn run_builtin_demo() -> anyhow::Result<()> {
    let aspect_ratio = 16.0 / 9.0;
    let width: u32 = 800;
    let height: u32 = (width as f64 / aspect_ratio) as u32;
    let base = RenderSettings::new(width, height, SAMPLES_PER_PIXEL, MAX_DEPTH);

    render_to(&reflection_scene(aspect_ratio), &base, "renders/02_reflection_refraction.png")?;
    render_to(&crystal_scene(aspect_ratio)?, &base, "renders/03_mesh_bvh_crystal.png")?;
    render_to(&hall_of_mirrors_scene(aspect_ratio)?, &base, "renders/04_hall_of_mirrors.png")?;

    // Deeper bounce budget than the baseline renders: several of the glitch
    // modes send rays ricocheting internally through convex geometry rather
    // than exiting quickly, so they need the extra depth to fully develop.
    let glitch_settings = RenderSettings::new(width, height, SAMPLES_PER_PIXEL, 40);

    let showcase = glitch_scene(aspect_ratio)?;
    for mode in GlitchMode::ALL {
        let settings = glitch_settings.with_glitch(mode);
        let path = format!("renders/glitch_{}.png", mode.slug());
        render_to(&showcase, &settings, &path)?;
    }

    // Bonus: the axis-swap glitch down an actual mirrored corridor produces
    // a distinct receding-band tiling effect that a single isolated
    // reflective object can't - worth its own showcase image.
    let hall_glitch = glitch_settings.with_glitch(GlitchMode::AxisSwapReflect);
    render_to(&hall_of_mirrors_scene(aspect_ratio)?, &hall_glitch, "renders/glitch_hall_axis_swap_reflect.png")?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let Some(scene_path) = cli.scene else {
        return run_builtin_demo();
    };

    let height = cli.height.unwrap_or_else(|| (cli.width as f64 / cli.aspect) as u32);
    let settings = RenderSettings::new(cli.width, height, cli.samples, cli.depth);
    let scene_desc = SceneDesc::load(&scene_path)?;

    if cli.gallery {
        // In gallery mode `--output` names a directory; scene needs
        // rebuilding per mode since meshes are consumed on scene build.
        for mode in GlitchMode::ALL {
            let scene = SceneDesc::load(&scene_path)?.build(cli.aspect)?;
            let path = cli.output.join(format!("{}.png", mode.slug()));
            render_to(&scene, &settings.with_glitch(mode), path)?;
        }
    } else {
        let scene = scene_desc.build(cli.aspect)?;
        render_to(&scene, &settings.with_glitch(cli.glitch), &cli.output)?;
    }

    Ok(())
}
