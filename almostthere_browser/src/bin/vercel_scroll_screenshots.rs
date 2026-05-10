#![allow(dead_code)]

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use eframe::{App, NativeOptions, egui};
use image::{Rgba, RgbaImage};
use rich_canvas::{BrowserCanvas, BrowserDocument, configure_browser_fonts};

mod browser_app {
    include!("../main.rs");

    pub fn parse_for_capture(html: &str, source: &str) -> rich_canvas::BrowserDocument {
        parse_html_document(html, source)
    }
}

const CACHED_HTML_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sample_pages/cache/latex_elements.html"
);
const TARGET_URL: &str = "https://latex.vercel.app/elements";
const OUTPUT_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/vercel_scroll_screenshots"
);

fn main() -> io::Result<()> {
    fs::create_dir_all(OUTPUT_DIR)?;
    let html = fs::read_to_string(CACHED_HTML_PATH)?;
    let document = browser_app::parse_for_capture(&html, TARGET_URL);
    let window_size = egui::vec2(794.0, 1123.0);

    for (index, scroll_y) in [0.0, 900.0, 1800.0, 2700.0, 3600.0].into_iter().enumerate() {
        let output_path = Path::new(OUTPUT_DIR).join(format!("vercel_scroll_{index:02}.png"));
        capture_browser_window_screenshot(document.clone(), scroll_y, &output_path, window_size)?;
        println!("scroll_y={scroll_y:.0}: {}", output_path.display());
    }

    Ok(())
}

#[derive(Default)]
struct WindowCaptureResult {
    path: Option<PathBuf>,
    error: Option<String>,
}

struct BrowserWindowCaptureApp {
    canvas: BrowserCanvas,
    document: BrowserDocument,
    output_path: PathBuf,
    result: Arc<Mutex<WindowCaptureResult>>,
    requested: bool,
    captured: bool,
}

impl App for BrowserWindowCaptureApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::light());
        self.handle_screenshot_events(ctx);
        if self.captured {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.document.style.page_background))
            .show(ctx, |ui| {
                let _ = self.canvas.ui(ui, &mut self.document);
            });

        if !self.requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                "vercel_scroll",
            )));
            self.requested = true;
        }
        ctx.request_repaint();
    }
}

impl BrowserWindowCaptureApp {
    fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Screenshot { image, .. } = event else {
                continue;
            };

            if let Err(error) = color_image_to_rgba(&image).save(&self.output_path) {
                if let Ok(mut result) = self.result.lock() {
                    result.error = Some(error.to_string());
                }
            } else if let Ok(mut result) = self.result.lock() {
                result.path = Some(self.output_path.clone());
            }
            self.captured = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn capture_browser_window_screenshot(
    document: BrowserDocument,
    scroll_y: f32,
    output_path: &Path,
    window_size: egui::Vec2,
) -> io::Result<()> {
    let result = Arc::new(Mutex::new(WindowCaptureResult::default()));
    let app_result = Arc::clone(&result);
    let output_path = output_path.to_path_buf();
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(window_size)
            .with_resizable(false)
            .with_decorations(false)
            .with_title("AlmostThere Browser scroll capture"),
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        "AlmostThere Browser scroll capture",
        options,
        Box::new(move |cc| {
            configure_browser_fonts(&cc.egui_ctx);
            Ok(Box::new(BrowserWindowCaptureApp {
                canvas: BrowserCanvas {
                    zoom: 1.0,
                    scroll_offset: egui::vec2(0.0, scroll_y),
                },
                document,
                output_path,
                result: app_result,
                requested: false,
                captured: false,
            }))
        }),
    )
    .map_err(|error| io::Error::other(error.to_string()))?;

    let result = Arc::try_unwrap(result)
        .map_err(|_| io::Error::other("capture result still has owners"))?
        .into_inner()
        .map_err(|_| io::Error::other("capture result lock was poisoned"))?;

    if let Some(error) = result.error {
        return Err(io::Error::other(error));
    }
    if result.path.is_none() {
        return Err(io::Error::other("window screenshot was not captured"));
    }
    Ok(())
}

fn color_image_to_rgba(image: &egui::ColorImage) -> RgbaImage {
    let mut rgba = RgbaImage::new(image.size[0] as u32, image.size[1] as u32);
    for (index, color) in image.pixels.iter().enumerate() {
        let x = (index % image.size[0]) as u32;
        let y = (index / image.size[0]) as u32;
        rgba.put_pixel(x, y, Rgba([color.r(), color.g(), color.b(), color.a()]));
    }
    rgba
}
