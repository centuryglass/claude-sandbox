mod camera;
mod hittable;
mod light;
mod material;
mod ray;
mod render;
mod scene;
mod sphere;
mod vec3;

use camera::Camera;
use image::{ImageBuffer, Rgb};
use light::Light;
use material::Material;
use scene::Scene;
use sphere::Sphere;
use vec3::{Color, Point3, Vec3};

const MAX_DEPTH: u32 = 8;

fn to_rgb8(c: Color) -> Rgb<u8> {
    let clamp = |v: f64| (v.clamp(0.0, 1.0) * 255.999) as u8;
    Rgb([clamp(c.x), clamp(c.y), clamp(c.z)])
}

fn first_scene(aspect_ratio: f64) -> Scene {
    let objects: Vec<Box<dyn hittable::Hittable>> = vec![
        // Ground.
        Box::new(Sphere::new(
            Point3::new(0.0, -100.5, -1.0),
            100.0,
            Material::Lambertian { albedo: Color::new(0.6, 0.6, 0.65) },
        )),
        Box::new(Sphere::new(
            Point3::new(0.0, 0.0, -1.2),
            0.5,
            Material::Lambertian { albedo: Color::new(0.8, 0.2, 0.3) },
        )),
        Box::new(Sphere::new(
            Point3::new(-1.1, 0.0, -1.4),
            0.5,
            Material::Lambertian { albedo: Color::new(0.2, 0.6, 0.3) },
        )),
        Box::new(Sphere::new(
            Point3::new(1.1, 0.0, -1.6),
            0.5,
            Material::Lambertian { albedo: Color::new(0.2, 0.3, 0.8) },
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

fn main() -> anyhow::Result<()> {
    let aspect_ratio = 16.0 / 9.0;
    let width: u32 = 800;
    let height: u32 = (width as f64 / aspect_ratio) as u32;

    let scene = first_scene(aspect_ratio);

    let mut img = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let s = x as f64 / (width - 1) as f64;
            let t = 1.0 - (y as f64 / (height - 1) as f64);
            let ray = scene.camera.get_ray(s, t);
            let color = render::ray_color(&ray, &scene, MAX_DEPTH);
            img.put_pixel(x, y, to_rgb8(color));
        }
    }

    std::fs::create_dir_all("renders")?;
    let path = "renders/01_spheres_diffuse_shadows.png";
    img.save(path)?;
    println!("wrote {path}");
    Ok(())
}
