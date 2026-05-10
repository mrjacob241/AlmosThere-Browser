#![allow(dead_code)]

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use eframe::{App, NativeOptions, egui};
use image::{Rgba, RgbaImage};
use rich_canvas::{
    BrowserCanvas, BrowserDocument, BrowserStyle, CanvasBlock, configure_browser_fonts,
    parse_basic_css,
};

const DEFAULT_PAGE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sample_pages/test_basic_page.html"
);
const DEFAULT_REFERENCE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sample_pages/ref_test_basic_page.png"
);
const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/page_screenshots");

fn main() -> io::Result<()> {
    let page_path = Path::new(DEFAULT_PAGE_PATH);
    let reference_path = Path::new(DEFAULT_REFERENCE_PATH);
    let output_dir = Path::new(OUTPUT_DIR);
    fs::create_dir_all(output_dir)?;

    let html = fs::read_to_string(page_path)?;
    let reference = image::open(reference_path).map_err(image_to_io)?.to_rgba8();
    let page = PageSnapshot::parse(&html);

    let rendered_path = output_dir.join("test_basic_page.png");
    let report_path = output_dir.join("test_basic_page_report.json");
    capture_browser_window_screenshot(
        page.to_browser_document(page_path),
        &rendered_path,
        egui::vec2(reference.width() as f32, reference.height() as f32),
    )?;
    let rendered = image::open(&rendered_path).map_err(image_to_io)?.to_rgba8();

    let comparison = compare_images(&rendered, &reference);
    fs::write(
        &report_path,
        comparison.to_json(
            page_path,
            reference_path,
            &rendered_path,
            comparison.matches_reference(),
        ),
    )?;

    println!("rendered: {}", rendered_path.display());
    println!("report: {}", report_path.display());
    println!(
        "match: {} mean_abs_delta: {:.3} changed_pixel_fraction: {:.4}",
        comparison.matches_reference(),
        comparison.mean_abs_delta,
        comparison.changed_pixel_fraction
    );

    if comparison.matches_reference() {
        Ok(())
    } else {
        Err(io::Error::other(
            "screenshot did not match reference thresholds",
        ))
    }
}

#[derive(Clone, Debug, Default)]
struct PageSnapshot {
    title: String,
    intro: String,
    style: BrowserStyle,
    panels: Vec<PanelSnapshot>,
}

#[derive(Clone, Debug, Default)]
struct PanelSnapshot {
    heading: String,
    paragraphs: Vec<String>,
    link: Option<(String, String)>,
    button: Option<String>,
    input: Option<(String, String)>,
}

impl PageSnapshot {
    fn parse(html: &str) -> Self {
        let body = extract_tag_inner(html, "body").unwrap_or(html);
        let main = extract_tag_inner(body, "main").unwrap_or(body);
        let header = extract_tag_inner(main, "header").unwrap_or("");
        let title = extract_tag_text(header, "h1")
            .or_else(|| extract_tag_text(html, "title"))
            .unwrap_or_else(|| "Untitled".to_owned());
        let intro = extract_tag_text(header, "p").unwrap_or_default();
        let style = parse_basic_css(extract_tag_inner(html, "style").unwrap_or(""));
        let panels = extract_sections(main)
            .into_iter()
            .map(|section| PanelSnapshot {
                heading: extract_tag_text(section, "h2").unwrap_or_default(),
                paragraphs: extract_paragraphs_without_link_only(section),
                link: extract_link(section),
                button: extract_tag_text(section, "button"),
                input: extract_input(section),
            })
            .collect();

        Self {
            title,
            intro,
            style,
            panels,
        }
    }

    fn to_browser_document(&self, page_path: &Path) -> BrowserDocument {
        let mut blocks = vec![
            CanvasBlock::Heading {
                level: 1,
                text: self.title.clone(),
            },
            CanvasBlock::Paragraph {
                text: self.intro.clone(),
            },
        ];

        for panel in &self.panels {
            let mut children = Vec::new();
            if !panel.heading.is_empty() {
                children.push(CanvasBlock::Heading {
                    level: 2,
                    text: panel.heading.clone(),
                });
            }
            for paragraph in &panel.paragraphs {
                children.push(CanvasBlock::Paragraph {
                    text: paragraph.clone(),
                });
            }
            if let Some((text, href)) = &panel.link {
                children.push(CanvasBlock::Link {
                    text: text.clone(),
                    href: href.clone(),
                });
            }
            if let Some(text) = &panel.button {
                children.push(CanvasBlock::Button { text: text.clone() });
            }
            if let Some((label, value)) = &panel.input {
                children.push(CanvasBlock::Input {
                    label: label.clone(),
                    value: value.clone(),
                });
            }
            blocks.push(CanvasBlock::Panel { children });
        }

        BrowserDocument {
            title: self.title.clone(),
            source: page_path.display().to_string(),
            style: self.style.clone(),
            canvas_graph: Default::default(),
            blocks,
        }
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
                "browser_page",
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
            .with_title("AlmostThere Browser screenshot capture"),
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        "AlmostThere Browser screenshot capture",
        options,
        Box::new(move |cc| {
            configure_browser_fonts(&cc.egui_ctx);
            Ok(Box::new(BrowserWindowCaptureApp {
                canvas: BrowserCanvas::new(),
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

fn render_page(page: &PageSnapshot, width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width, height, rgba(245, 247, 250));
    let content_x = 32;
    let content_w = width.saturating_sub(64).min(760);
    let text = rgba(15, 23, 42);
    let link = rgba(7, 89, 133);
    let border = rgba(204, 214, 224);

    draw_text(&mut image, content_x, 29, &page.title, 4, text);
    draw_text(&mut image, content_x, 88, &page.intro, 2, text);

    let mut y = 129;
    for (index, panel) in page.panels.iter().enumerate() {
        let panel_h = if index == 0 { 197 } else { 139 };
        draw_filled_rect(
            &mut image,
            content_x,
            y,
            content_w,
            panel_h,
            rgba(255, 255, 255),
        );
        draw_stroked_rect(&mut image, content_x, y, content_w, panel_h, border);

        let inner_x = content_x + 17;
        let mut cursor_y = y + 26;
        draw_text(&mut image, inner_x, cursor_y, &panel.heading, 3, text);
        cursor_y += 46;

        for paragraph in &panel.paragraphs {
            draw_text(&mut image, inner_x, cursor_y, paragraph, 2, text);
            cursor_y += 40;
        }

        if let Some((label, _href)) = &panel.link {
            draw_text(&mut image, inner_x, cursor_y, label, 2, link);
            draw_line(
                &mut image,
                inner_x,
                cursor_y + 18,
                inner_x + text_width(label, 2),
                cursor_y + 18,
                link,
            );
            cursor_y += 35;
        }

        if let Some(button) = &panel.button {
            draw_filled_rect(&mut image, inner_x, cursor_y, 96, 34, rgba(224, 242, 254));
            draw_stroked_rect(&mut image, inner_x, cursor_y, 96, 34, link);
            draw_text(&mut image, inner_x + 13, cursor_y + 11, button, 1, link);
        }

        if let Some((label, value)) = &panel.input {
            draw_text(&mut image, inner_x, cursor_y, label, 2, text);
            cursor_y += 25;
            draw_filled_rect(
                &mut image,
                inner_x,
                cursor_y,
                content_w.saturating_sub(34),
                34,
                rgba(255, 255, 255),
            );
            draw_stroked_rect(
                &mut image,
                inner_x,
                cursor_y,
                content_w.saturating_sub(34),
                34,
                rgba(154, 166, 178),
            );
            draw_text(&mut image, inner_x + 9, cursor_y + 10, value, 1, text);
        }

        y += panel_h + 24;
    }

    image
}

#[derive(Clone, Copy, Debug)]
struct ImageComparison {
    dimensions_match: bool,
    width: u32,
    height: u32,
    reference_width: u32,
    reference_height: u32,
    mean_abs_delta: f64,
    max_delta: u8,
    changed_pixel_fraction: f64,
}

impl ImageComparison {
    fn matches_reference(self) -> bool {
        self.dimensions_match && self.mean_abs_delta <= 16.0 && self.changed_pixel_fraction <= 0.12
    }

    fn to_json(
        self,
        page_path: &Path,
        reference_path: &Path,
        rendered_path: &Path,
        matches: bool,
    ) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"schema_version\": 1,\n",
                "  \"page\": \"{}\",\n",
                "  \"reference\": \"{}\",\n",
                "  \"rendered\": \"{}\",\n",
                "  \"matches_reference\": {},\n",
                "  \"dimensions_match\": {},\n",
                "  \"width\": {},\n",
                "  \"height\": {},\n",
                "  \"reference_width\": {},\n",
                "  \"reference_height\": {},\n",
                "  \"mean_abs_delta\": {:.6},\n",
                "  \"max_delta\": {},\n",
                "  \"changed_pixel_fraction\": {:.6}\n",
                "}}\n"
            ),
            json_escape(&page_path.display().to_string()),
            json_escape(&reference_path.display().to_string()),
            json_escape(&rendered_path.display().to_string()),
            matches,
            self.dimensions_match,
            self.width,
            self.height,
            self.reference_width,
            self.reference_height,
            self.mean_abs_delta,
            self.max_delta,
            self.changed_pixel_fraction
        )
    }
}

fn compare_images(rendered: &RgbaImage, reference: &RgbaImage) -> ImageComparison {
    let dimensions_match =
        rendered.width() == reference.width() && rendered.height() == reference.height();
    let width = rendered.width().min(reference.width());
    let height = rendered.height().min(reference.height());
    let mut total_delta = 0u64;
    let mut max_delta = 0u8;
    let mut changed_pixels = 0u64;

    for y in 0..height {
        for x in 0..width {
            let a = rendered.get_pixel(x, y).0;
            let b = reference.get_pixel(x, y).0;
            let mut pixel_delta = 0u16;
            for channel in 0..3 {
                let delta = a[channel].abs_diff(b[channel]);
                max_delta = max_delta.max(delta);
                total_delta += u64::from(delta);
                pixel_delta += u16::from(delta);
            }
            if pixel_delta > 72 {
                changed_pixels += 1;
            }
        }
    }

    let compared_pixels = u64::from(width) * u64::from(height);
    let compared_channels = compared_pixels * 3;
    ImageComparison {
        dimensions_match,
        width: rendered.width(),
        height: rendered.height(),
        reference_width: reference.width(),
        reference_height: reference.height(),
        mean_abs_delta: total_delta as f64 / compared_channels.max(1) as f64,
        max_delta,
        changed_pixel_fraction: changed_pixels as f64 / compared_pixels.max(1) as f64,
    }
}

fn draw_filled_rect(image: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    for yy in y..(y + h).min(image.height()) {
        for xx in x..(x + w).min(image.width()) {
            image.put_pixel(xx, yy, color);
        }
    }
}

fn draw_stroked_rect(image: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    if w == 0 || h == 0 {
        return;
    }
    draw_line(image, x, y, x + w - 1, y, color);
    draw_line(image, x, y + h - 1, x + w - 1, y + h - 1, color);
    draw_line(image, x, y, x, y + h - 1, color);
    draw_line(image, x + w - 1, y, x + w - 1, y + h - 1, color);
}

fn draw_line(image: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, color: Rgba<u8>) {
    if x0 == x1 {
        for y in y0.min(y1)..=y0.max(y1) {
            put_pixel_checked(image, x0, y, color);
        }
    } else if y0 == y1 {
        for x in x0.min(x1)..=x0.max(x1) {
            put_pixel_checked(image, x, y0, color);
        }
    }
}

fn draw_text(image: &mut RgbaImage, x: u32, y: u32, text: &str, scale: u32, color: Rgba<u8>) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_glyph(image, cursor_x, y, ch, scale, color);
        cursor_x += 6 * scale;
    }
}

fn draw_glyph(image: &mut RgbaImage, x: u32, y: u32, ch: char, scale: u32, color: Rgba<u8>) {
    let pattern = glyph_pattern(ch);
    for (row, bits) in pattern.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                draw_filled_rect(
                    image,
                    x + col * scale,
                    y + row as u32 * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 6 * scale
}

fn glyph_pattern(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        '0' => [
            0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        _ => [0b00000; 7],
    }
}

fn put_pixel_checked(image: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, color);
    }
}

fn extract_sections(html: &str) -> Vec<&str> {
    let mut sections = Vec::new();
    let mut remaining = html;
    while let Some(start) = remaining.find("<section") {
        let after_start = &remaining[start..];
        let Some(open_end) = after_start.find('>') else {
            break;
        };
        let content_start = start + open_end + 1;
        let after_content_start = &remaining[content_start..];
        let Some(close_index) = after_content_start.find("</section>") else {
            break;
        };
        sections.push(&after_content_start[..close_index]);
        remaining = &after_content_start[close_index + "</section>".len()..];
    }
    sections
}

fn extract_paragraphs_without_link_only(html: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut remaining = html;
    while let Some(start) = remaining.find("<p") {
        let after_start = &remaining[start..];
        let Some(open_end) = after_start.find('>') else {
            break;
        };
        let content_start = start + open_end + 1;
        let after_content_start = &remaining[content_start..];
        let Some(close_index) = after_content_start.find("</p>") else {
            break;
        };
        let content = &after_content_start[..close_index];
        let text = normalize_ws(&strip_tags(content));
        let link_text = extract_link(content)
            .map(|(label, _)| label)
            .unwrap_or_default();
        if !text.is_empty() && text != link_text {
            paragraphs.push(text);
        }
        remaining = &after_content_start[close_index + "</p>".len()..];
    }
    paragraphs
}

fn extract_input(html: &str) -> Option<(String, String)> {
    let label = extract_tag_text(html, "label").unwrap_or_else(|| "Input".to_owned());
    let input_start = html.find("<input")?;
    let input_after = &html[input_start..];
    let input_end = input_after.find('>')?;
    let input_tag = &input_after[..input_end + 1];
    Some((label, extract_attr(input_tag, "value").unwrap_or_default()))
}

fn extract_link(html: &str) -> Option<(String, String)> {
    let start = html.find("<a")?;
    let after_start = &html[start..];
    let open_end = after_start.find('>')?;
    let tag = &after_start[..open_end + 1];
    let content_start = start + open_end + 1;
    let after_content_start = &html[content_start..];
    let close_index = after_content_start.find("</a>")?;
    Some((
        normalize_ws(&strip_tags(&after_content_start[..close_index])),
        extract_attr(tag, "href").unwrap_or_default(),
    ))
}

fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    extract_tag_inner(html, tag).map(strip_tags)
}

fn extract_tag_inner<'a>(html: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = html.find(&open)?;
    let after_start = &html[start..];
    let open_end = after_start.find('>')?;
    let content_start = start + open_end + 1;
    let after_content_start = &html[content_start..];
    let close_index = after_content_start.find(&close)?;
    Some(&after_content_start[..close_index])
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    normalize_ws(&decode_basic_entities(&out))
}

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn rgba(r: u8, g: u8, b: u8) -> Rgba<u8> {
    Rgba([r, g, b, 255])
}

fn image_to_io(error: image::ImageError) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_images_match_reference_thresholds() {
        let image = RgbaImage::from_pixel(8, 8, rgba(245, 247, 250));
        let comparison = compare_images(&image, &image);
        assert!(comparison.matches_reference());
        assert_eq!(comparison.mean_abs_delta, 0.0);
    }

    #[test]
    fn dimension_mismatch_fails_reference_match() {
        let rendered = RgbaImage::from_pixel(8, 8, rgba(245, 247, 250));
        let reference = RgbaImage::from_pixel(9, 8, rgba(245, 247, 250));
        let comparison = compare_images(&rendered, &reference);
        assert!(!comparison.matches_reference());
    }
}
