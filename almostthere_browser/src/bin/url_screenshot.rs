#![allow(dead_code)]

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use eframe::{App, NativeOptions, egui};
use image::{Rgba, RgbaImage};
use rich_canvas::{
    BrowserCanvas, BrowserDocument, CanvasBlock, CanvasObject, configure_browser_fonts,
};

mod browser_app {
    include!("../main.rs");

    pub fn load_for_capture(input: &str) -> std::io::Result<rich_canvas::BrowserDocument> {
        load_url_document(input)
    }

    pub fn fetch_html_for_capture(url: &str) -> std::io::Result<(String, String)> {
        let response = http_client()?
            .get(url)
            .send()
            .map_err(std::io::Error::other)?;
        let final_url = response.url().to_string();
        let response = response.error_for_status().map_err(std::io::Error::other)?;
        let html = response.text().map_err(std::io::Error::other)?;
        Ok((final_url, html))
    }

    pub fn live_js_debug_for_capture(html: &str, source: &str) -> String {
        live_js_debug_report(html, Some(source))
    }
}

const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/url_screenshots");

fn main() -> io::Result<()> {
    let url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.ecosia.org/".to_owned());
    let output_name = env::args()
        .nth(2)
        .unwrap_or_else(|| "url_capture.png".to_owned());
    let scroll_y = env::args()
        .nth(3)
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.0);
    fs::create_dir_all(OUTPUT_DIR)?;
    if url.starts_with("http://") || url.starts_with("https://") {
        if let Ok((final_url, html)) = browser_app::fetch_html_for_capture(&url) {
            let html_path = Path::new(OUTPUT_DIR).join("last_capture.html");
            let live_js_debug = browser_app::live_js_debug_for_capture(&html, &final_url);
            let live_js_debug_path = Path::new(OUTPUT_DIR).join("last_live_js_debug.txt");
            fs::write(&html_path, html)?;
            fs::write(&live_js_debug_path, live_js_debug)?;
            println!("html_url: {final_url}");
            println!("html_dump: {}", html_path.display());
            println!("live_js_debug: {}", live_js_debug_path.display());
        }
    }

    let document = browser_app::load_for_capture(&url)?;
    let summary = summarize_document(&document);
    let output_path = Path::new(OUTPUT_DIR).join(output_name);

    capture_browser_window_screenshot(
        document.clone(),
        scroll_y,
        egui::vec2(1280.0, 900.0),
        &output_path,
    )?;

    println!("url: {}", document.source);
    println!("title: {}", document.title);
    println!("blocks: {}", document.blocks.len());
    println!(
        "canvas_graph_objects: {}",
        document.canvas_graph.objects.len()
    );
    print_canvas_graph_summary(&document);
    println!("images: {}", summary.images);
    println!("svgs: {}", summary.svgs);
    println!("media_placeholders: {}", summary.media_placeholders);
    println!("inputs: {}", summary.inputs);
    println!("buttons: {}", summary.buttons);
    println!("links: {}", summary.links);
    println!("scroll_y: {scroll_y:.1}");
    println!("screenshot: {}", output_path.display());
    println!("first_blocks:");
    print_blocks(&document.blocks, 0, &mut 0, 32);

    Ok(())
}

fn print_canvas_graph_summary(document: &BrowserDocument) {
    let mut rects = 0;
    let mut texts = 0;
    let mut images = 0;
    let mut svgs = 0;
    let mut media = 0;
    for object in &document.canvas_graph.objects {
        match object {
            CanvasObject::Rect(_) => rects += 1,
            CanvasObject::Text(_) => texts += 1,
            CanvasObject::Input(_) => texts += 1,
            CanvasObject::Image(_) => images += 1,
            CanvasObject::Svg(_) => svgs += 1,
            CanvasObject::Media(_) => media += 1,
            CanvasObject::Button(_) => {}
            CanvasObject::ClipStart(_) | CanvasObject::ClipEnd => {}
        }
    }
    println!(
        "canvas_graph: rects={rects} texts={texts} images={images} svgs={svgs} media={media} viewport={:.1}x{:.1}",
        document.canvas_graph.viewport.x, document.canvas_graph.viewport.y
    );
}

#[derive(Clone, Copy, Debug, Default)]
struct DocumentSummary {
    images: usize,
    svgs: usize,
    media_placeholders: usize,
    inputs: usize,
    buttons: usize,
    links: usize,
}

fn summarize_document(document: &BrowserDocument) -> DocumentSummary {
    let mut summary = DocumentSummary::default();
    summarize_blocks(&document.blocks, &mut summary);
    summary
}

fn summarize_blocks(blocks: &[CanvasBlock], summary: &mut DocumentSummary) {
    for block in blocks {
        match block {
            CanvasBlock::Image { .. } => summary.images += 1,
            CanvasBlock::EcosiaHero { .. } => summary.images += 1,
            CanvasBlock::SearchResultsPage { page } => {
                summary.images += page
                    .videos
                    .iter()
                    .filter(|item| item.image.is_some())
                    .count();
                summary.images += page
                    .results
                    .iter()
                    .filter(|item| item.thumbnail.is_some())
                    .count();
                summary.images += page
                    .footer_cards
                    .iter()
                    .filter(|item| item.image.is_some())
                    .count();
                summary.links +=
                    page.videos.len() + page.results.len() + page.related_queries.len();
                if let Some(sidebar) = &page.sidebar {
                    summary.links += sidebar.links.len();
                }
            }
            CanvasBlock::Svg { .. } => summary.svgs += 1,
            CanvasBlock::Media { .. } => summary.media_placeholders += 1,
            CanvasBlock::Input { .. } => summary.inputs += 1,
            CanvasBlock::Button { .. } => summary.buttons += 1,
            CanvasBlock::Link { .. } => summary.links += 1,
            CanvasBlock::Panel { children } => summarize_blocks(children, summary),
            CanvasBlock::Box { children, .. } => summarize_blocks(children, summary),
            CanvasBlock::StyledBox { children, .. } => summarize_blocks(children, summary),
            _ => {}
        }
    }
}

fn print_blocks(blocks: &[CanvasBlock], depth: usize, count: &mut usize, max: usize) {
    for block in blocks {
        if *count >= max {
            return;
        }
        *count += 1;
        let indent = "  ".repeat(depth);
        match block {
            CanvasBlock::Heading { level, text } => {
                println!("{indent}- h{level}: {}", shorten(text));
            }
            CanvasBlock::Paragraph { text } => {
                println!("{indent}- p: {}", shorten(text));
            }
            CanvasBlock::InlineText { spans } => {
                let text = spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>();
                println!("{indent}- inline: {}", shorten(&text));
            }
            CanvasBlock::Link { text, href } => {
                println!("{indent}- link: {} -> {}", shorten(text), shorten(href));
            }
            CanvasBlock::ListItem { text, .. } => {
                println!("{indent}- li: {}", shorten(text));
            }
            CanvasBlock::Quote { text } => {
                println!("{indent}- quote: {}", shorten(text));
            }
            CanvasBlock::Rule => println!("{indent}- hr"),
            CanvasBlock::Preformatted { text } => {
                println!("{indent}- pre: {}", shorten(text));
            }
            CanvasBlock::Media { label } => {
                println!("{indent}- media: {}", shorten(label));
            }
            CanvasBlock::Svg { svg } => {
                println!(
                    "{indent}- svg: size={:.1}x{:.1} shapes={}",
                    svg.size.x,
                    svg.size.y,
                    svg.shapes.len()
                );
            }
            CanvasBlock::Image { alt, src, image } => {
                println!(
                    "{indent}- image: alt='{}' size={:.1}x{:.1} src={}",
                    shorten(alt),
                    image.size.x,
                    image.size.y,
                    shorten(src)
                );
            }
            CanvasBlock::EcosiaHero { hero } => {
                println!(
                    "{indent}- ecosia-hero: bg={:.1}x{:.1} search='{}' trees='{}' investments='{}'",
                    hero.background.size.x,
                    hero.background.size.y,
                    shorten(&hero.search_placeholder),
                    shorten(&hero.tree_count),
                    shorten(&hero.investment_count)
                );
            }
            CanvasBlock::SearchResultsPage { page } => {
                println!(
                    "{indent}- search-results: query='{}' nav={} videos={} results={} sidebar={} footer_cards={}",
                    shorten(&page.query),
                    page.nav_items.len(),
                    page.videos.len(),
                    page.results.len(),
                    page.sidebar.is_some(),
                    page.footer_cards.len()
                );
            }
            CanvasBlock::Table { caption, .. } => {
                println!("{indent}- table: {}", shorten(caption));
            }
            CanvasBlock::Button { text } => {
                println!("{indent}- button: {}", shorten(text));
            }
            CanvasBlock::Input { label, value } => {
                println!(
                    "{indent}- input: label='{}' value='{}'",
                    shorten(label),
                    shorten(value)
                );
            }
            CanvasBlock::Panel { children } => {
                println!("{indent}- panel");
                print_blocks(children, depth + 1, count, max);
            }
            CanvasBlock::Box {
                style_key,
                children,
            } => {
                println!(
                    "{indent}- box: tag={} id={} classes={}",
                    style_key.tag,
                    style_key.id.as_deref().unwrap_or(""),
                    style_key.classes.join(".")
                );
                print_blocks(children, depth + 1, count, max);
            }
            CanvasBlock::StyledBox { style, children } => {
                println!(
                    "{indent}- styled-box: display={:?} margin={:.1}/{:.1}/{:.1}/{:.1} padding={:.1}/{:.1}/{:.1}/{:.1}",
                    style.display,
                    style.margin.top,
                    style.margin.right,
                    style.margin.bottom,
                    style.margin.left,
                    style.padding.top,
                    style.padding.right,
                    style.padding.bottom,
                    style.padding.left
                );
                print_blocks(children, depth + 1, count, max);
            }
        }
    }
}

fn shorten(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 96 {
        compact.chars().take(93).collect::<String>() + "..."
    } else {
        compact
    }
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
                "url_capture",
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
    window_size: egui::Vec2,
    output_path: &Path,
) -> io::Result<()> {
    let result = Arc::new(Mutex::new(WindowCaptureResult::default()));
    let app_result = Arc::clone(&result);
    let output_path = output_path.to_path_buf();
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(window_size)
            .with_resizable(false)
            .with_decorations(false)
            .with_title("AlmostThere Browser URL capture"),
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        "AlmostThere Browser URL capture",
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
