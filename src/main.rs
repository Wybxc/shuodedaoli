use std::{
    f32::consts::PI,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use eframe::NativeOptions;
use egui::{load::SizedTexture, mutex::RwLock, ColorImage, ImageSource, Slider, ViewportBuilder};
use image::{DynamicImage, GenericImageView, Pixel, RgbImage};
use nalgebra::{vector, Rotation3};
use rayon::prelude::*;

use crate::projection::Projection;

mod listener;
mod projection;

type Vec3u8 = nalgebra::SVector<u8, 3>;
type Vec3f = nalgebra::SVector<f32, 3>;

fn interpolation(q1: image::Rgb<u8>, x1: f32, q2: image::Rgb<u8>, x2: f32) -> image::Rgb<u8> {
    let q1: Vec3f = Vec3u8::from_iterator(q1.channels().iter().copied()).cast();
    let q2: Vec3f = Vec3u8::from_iterator(q2.channels().iter().copied()).cast();
    let q = (q1.scale(x1) + q2.scale(x2)) / (x1 + x2);
    image::Rgb([q[0] as u8, q[1] as u8, q[2] as u8])
}

fn bilinear_interpolation(img: &DynamicImage, x: f32, y: f32) -> image::Rgb<u8> {
    let (width, height) = img.dimensions();
    let x1 = (x.max(0.) as u32).min(width - 1);
    let y1 = (y.max(0.) as u32).min(height - 1);
    let x2 = (x1 + 1).min(width - 1);
    let y2 = (y1 + 1).min(height - 1);

    let q11 = img.get_pixel(x1, y1).to_rgb();
    let q21 = img.get_pixel(x2, y1).to_rgb();
    let q12 = img.get_pixel(x1, y2).to_rgb();
    let q22 = img.get_pixel(x2, y2).to_rgb();

    let r1 = interpolation(q11, x2 as f32 - x, q21, x - x1 as f32);
    let r2 = interpolation(q12, x2 as f32 - x, q22, x - x1 as f32);
    interpolation(r1, y2 as f32 - y, r2, y - y1 as f32)
}

fn stereographic_projection(img: &DynamicImage, out: &mut RgbImage, proj: Projection) {
    out.enumerate_pixels_mut()
        .par_bridge()
        .for_each(|(x, y, pixel)| {
            let p = proj.proj(vector![x as f32, y as f32]);
            *pixel = bilinear_interpolation(img, p.x, p.y);
        });
}

fn main() -> eframe::Result<()> {
    let mut image = None;
    let mut offset = (0.0, 0.4);
    let mut rotation = (0.0, 0.09, 0.0);
    let mut scale = 1.5;

    let out_image: Arc<RwLock<Option<RgbImage>>> = Arc::new(RwLock::new(None));
    let out_tex = Arc::new(RwLock::new(None));
    let processing = Arc::new(AtomicBool::new(false));

    let options = NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([900., 600.]),
        ..Default::default()
    };
    eframe::run_simple_native("说的道理", options, move |ctx, _frame| {
        egui_extras::install_image_loaders(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let mut listener = listener::Listerner::new();

                    listener += ui.add(Slider::new(&mut offset.0, -1.0..=1.0).text("Offset X"));
                    listener += ui.add(Slider::new(&mut offset.1, -1.0..=1.0).text("Offset Y"));
                    ui.shrink_width_to_current();
                    ui.separator();

                    listener += ui.add(Slider::new(&mut rotation.0, 0.0..=PI).text("Rotation X"));
                    listener += ui.add(Slider::new(&mut rotation.1, 0.0..=PI).text("Rotation Y"));
                    listener += ui.add(Slider::new(&mut rotation.2, 0.0..=PI).text("Rotation Z"));
                    ui.shrink_width_to_current();
                    ui.separator();

                    listener += ui.add(Slider::new(&mut scale, 0.1..=5.0).text("Scale"));
                    ui.shrink_width_to_current();
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Select Image").clicked() {
                            let path = rfd::FileDialog::new()
                                .add_filter("Image", &["jpg", "jpeg", "png", "bmp", "gif", "webp"])
                                .pick_file();
                            if let Some(path) = path {
                                match image::open(path) {
                                    Ok(img) => {
                                        image = Some(Arc::new(img));
                                        listener += true;
                                    }
                                    Err(e) => {
                                        rfd::MessageDialog::new()
                                            .set_title("Error")
                                            .set_description(format!("Failed to open image: {}", e))
                                            .show();
                                    }
                                }
                            }
                        }

                        if ui.button("Save Image").clicked() {
                            if let Some(out_image) = &*out_image.read() {
                                let path = rfd::FileDialog::new()
                                    .add_filter("Image", &["png"])
                                    .set_file_name("output.png")
                                    .save_file();
                                if let Some(path) = path {
                                    if let Err(e) = out_image.save(path) {
                                        rfd::MessageDialog::new()
                                            .set_title("Error")
                                            .set_description(format!("Failed to save image: {}", e))
                                            .show();
                                    }
                                }
                            }
                        }
                    });

                    let offset = vector![offset.0, offset.1];
                    let rotation = Rotation3::from_euler_angles(rotation.0, rotation.1, rotation.2);

                    if !listener.changed() {
                        return;
                    }

                    if processing.load(Ordering::Relaxed) {
                        ui.spinner();
                    } else if let Some(image) = &image {
                        let image = Arc::clone(image);
                        let out_image = Arc::clone(&out_image);
                        let out_tex = Arc::clone(&out_tex);
                        let processing = Arc::clone(&processing);
                        let tex_manager = Arc::clone(&ctx.tex_manager());
                        thread::spawn(move || {
                            processing.store(true, Ordering::Relaxed);

                            let mut out = RgbImage::new(600, 600);
                            let img_size = vector![image.width(), image.height()];
                            let proj_size = vector![out.width(), out.height()];
                            let proj =
                                Projection::new(img_size, proj_size, offset, rotation, scale);
                            stereographic_projection(&image, &mut out, proj);

                            out_tex.write().replace(SizedTexture::new(
                                tex_manager.write().alloc(
                                    "out".into(),
                                    ColorImage::from_rgb(
                                        proj_size.cast().into(),
                                        out.as_flat_samples().as_slice(),
                                    )
                                    .into(),
                                    Default::default(),
                                ),
                                <[f32; 2]>::from(proj_size.cast()),
                            ));
                            out_image.write().replace(out);

                            processing.store(false, Ordering::Relaxed);
                        });
                    }
                });

                if let Some(out_tex) = *out_tex.read() {
                    ui.image(ImageSource::Texture(out_tex));
                }
            });
        });
    })
}
