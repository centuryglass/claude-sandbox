use lumin::camera::Camera;
use lumin::hittable::Hittable;
use lumin::light::Light;
use lumin::material::Material;
use lumin::mesh;
use lumin::render::{self, RenderSettings};
use lumin::scene::Scene;
use lumin::sphere::Sphere;
use lumin::vec3::{Color, Point3, Vec3};

const MAX_DEPTH: u32 = 12;
const SAMPLES_PER_PIXEL: u32 = 256;

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

    Scene { objects, lights, ambient: Color::splat(0.08), camera }
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

    Ok(Scene { objects, lights, ambient: Color::splat(0.08), camera })
}

fn render_to(scene: &Scene, settings: &RenderSettings, path: &str) -> anyhow::Result<()> {
    let img = render::render(scene, settings);
    std::fs::create_dir_all("renders")?;
    img.save(path)?;
    println!("wrote {path}");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let aspect_ratio = 16.0 / 9.0;
    let width: u32 = 800;
    let height: u32 = (width as f64 / aspect_ratio) as u32;
    let settings = RenderSettings { width, height, samples_per_pixel: SAMPLES_PER_PIXEL, max_depth: MAX_DEPTH };

    render_to(&reflection_scene(aspect_ratio), &settings, "renders/02_reflection_refraction.png")?;
    render_to(&crystal_scene(aspect_ratio)?, &settings, "renders/03_mesh_bvh_crystal.png")?;
    Ok(())
}
