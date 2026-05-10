#![allow(dead_code)]

use std::{fs, io, path::Path};

use rich_canvas::{BrowserDocument, CanvasBlock};

mod browser_app {
    include!("../main.rs");

    pub fn parse_for_snapshot(html: &str, source: &str) -> rich_canvas::BrowserDocument {
        parse_html_document(html, source)
    }
}

const CACHED_HTML_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sample_pages/cache/latex_elements.html"
);
const TARGET_URL: &str = "https://latex.vercel.app/elements";
const SNAPSHOT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/image_layout_snapshot.txt"
);
const SNAPSHOT_WINDOW_WIDTH: f32 = 794.0;

fn main() -> io::Result<()> {
    let html = fs::read_to_string(CACHED_HTML_PATH)?;
    let document = browser_app::parse_for_snapshot(&html, TARGET_URL);
    let tags = image_tags(&html);
    let images = document_images(&document);
    let content_width = browser_content_width(&document, SNAPSHOT_WINDOW_WIDTH);

    let mut out = String::new();
    out.push_str("AlmostThere image layout snapshot\n");
    out.push_str(&format!("source: {TARGET_URL}\n"));
    out.push_str(&format!(
        "snapshot_window_width: {SNAPSHOT_WINDOW_WIDTH:.1}\n"
    ));
    out.push_str(&format!(
        "style.main_max_width: {:.1}\n",
        document.style.main_max_width
    ));
    out.push_str(&format!(
        "style.main_padding_x: {:.1}\n",
        document.style.main_padding_x
    ));
    out.push_str(&format!(
        "style.image_width_percent: {:?}\n",
        document.style.image_width_percent
    ));
    out.push_str(&format!(
        "style.image_height_auto: {}\n",
        document.style.image_height_auto
    ));
    out.push_str(&format!("computed_content_width: {content_width:.1}\n\n"));

    for (index, tag) in tags.iter().enumerate() {
        let image = images
            .iter()
            .find(|image| image.src == tag.resolved_src || image.src.ends_with(&tag.src))
            .copied();
        let intrinsic = image
            .map(|image| {
                (
                    image.color_image_size[0] as f32,
                    image.color_image_size[1] as f32,
                )
            })
            .unwrap_or((0.0, 0.0));
        let preferred = image.map(|image| image.size).unwrap_or((0.0, 0.0));
        let final_size = image_display_size(
            preferred,
            content_width,
            document.style.image_width_percent,
            document.style.image_height_auto,
        );

        out.push_str(&format!("image[{index}]\n"));
        out.push_str(&format!(
            "  parent_stack: {}\n",
            tag.parent_stack.join(" > ")
        ));
        out.push_str(&format!("  src: {}\n", tag.src));
        out.push_str(&format!("  resolved_src: {}\n", tag.resolved_src));
        out.push_str(&format!("  attr_width: {:?}\n", tag.width));
        out.push_str(&format!("  attr_height: {:?}\n", tag.height));
        out.push_str(&format!(
            "  intrinsic_decoded_size: {:.1} x {:.1}\n",
            intrinsic.0, intrinsic.1
        ));
        out.push_str(&format!(
            "  preferred_size_after_attrs: {:.1} x {:.1}\n",
            preferred.0, preferred.1
        ));
        out.push_str(&format!("  containing_block_width: {content_width:.1}\n"));
        out.push_str(&format!(
            "  percent_width_target: {:.1}\n",
            percent_width_target(content_width, document.style.image_width_percent)
                .unwrap_or(f32::INFINITY)
        ));
        out.push_str(&format!(
            "  final_display_size: {:.1} x {:.1}\n\n",
            final_size.0, final_size.1
        ));
    }

    let path = Path::new(SNAPSHOT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &out)?;
    print!("{out}");
    Ok(())
}

#[derive(Clone, Debug)]
struct ImageTag {
    parent_stack: Vec<String>,
    src: String,
    resolved_src: String,
    width: Option<f32>,
    height: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
struct DocumentImage<'a> {
    src: &'a str,
    size: (f32, f32),
    color_image_size: [usize; 2],
}

fn document_images(document: &BrowserDocument) -> Vec<DocumentImage<'_>> {
    let mut images = Vec::new();
    collect_document_images(&document.blocks, &mut images);
    images
}

fn collect_document_images<'a>(blocks: &'a [CanvasBlock], images: &mut Vec<DocumentImage<'a>>) {
    for block in blocks {
        match block {
            CanvasBlock::Image { src, image, .. } => images.push(DocumentImage {
                src,
                size: (image.size.x, image.size.y),
                color_image_size: image.color_image.size,
            }),
            CanvasBlock::Panel { children } => collect_document_images(children, images),
            _ => {}
        }
    }
}

fn image_tags(html: &str) -> Vec<ImageTag> {
    let mut tags = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut remaining = html;

    while let Some(open_index) = remaining.find('<') {
        remaining = &remaining[open_index..];
        let Some(close_index) = remaining.find('>') else {
            break;
        };
        let raw = &remaining[..close_index + 1];
        if raw.starts_with("</") {
            if let Some(tag) = raw
                .trim_start_matches("</")
                .trim_end_matches('>')
                .split_whitespace()
                .next()
            {
                if let Some(position) = stack.iter().rposition(|item| item == tag) {
                    stack.truncate(position);
                }
            }
        } else if let Some(tag) = tag_name(raw) {
            if tag == "img" {
                let src = extract_attr_local(raw, "src").unwrap_or_default();
                tags.push(ImageTag {
                    parent_stack: stack.clone(),
                    resolved_src: resolve_resource_url(TARGET_URL, &src),
                    src,
                    width: extract_attr_local(raw, "width").and_then(parse_dimension),
                    height: extract_attr_local(raw, "height").and_then(parse_dimension),
                });
            } else if !is_void_tag(tag) {
                stack.push(tag.to_owned());
            }
        }
        remaining = &remaining[close_index + 1..];
    }

    tags
}

fn tag_name(raw: &str) -> Option<&str> {
    if raw.starts_with("<!") {
        return None;
    }
    let tag = raw.trim_start_matches('<').trim_start_matches('/');
    let end = tag
        .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
        .unwrap_or(tag.len());
    let tag = &tag[..end];
    (!tag.is_empty()).then_some(tag)
}

fn is_void_tag(tag: &str) -> bool {
    matches!(tag, "br" | "img" | "input" | "hr" | "meta" | "link")
}

fn extract_attr_local(tag: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=");
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)? + 1;
        Some(rest[1..end].to_owned())
    } else {
        let end = rest
            .find(|ch: char| ch.is_whitespace() || ch == '>')
            .unwrap_or(rest.len());
        Some(rest[..end].trim_end_matches('/').to_owned())
    }
}

fn parse_dimension(value: String) -> Option<f32> {
    value
        .trim()
        .trim_end_matches("px")
        .parse::<f32>()
        .ok()
        .filter(|value| *value > 0.0)
}

fn resolve_resource_url(source: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("file://") {
        return href.to_owned();
    }
    if let Ok(base) = reqwest::Url::parse(source) {
        if let Ok(resolved) = base.join(href) {
            return resolved.to_string();
        }
    }
    href.to_owned()
}

fn browser_content_width(document: &BrowserDocument, window_width: f32) -> f32 {
    (window_width - document.style.main_padding_x * 2.0)
        .min(document.style.main_max_width)
        .max(280.0)
}

fn percent_width_target(containing_width: f32, width_percent: Option<f32>) -> Option<f32> {
    width_percent.map(|percent| containing_width * percent / 100.0)
}

fn image_display_size(
    preferred_size: (f32, f32),
    containing_width: f32,
    width_percent: Option<f32>,
    height_auto: bool,
) -> (f32, f32) {
    let (preferred_width, preferred_height) = preferred_size;
    if let (Some(percent), true) = (width_percent, height_auto) {
        let width = containing_width * percent / 100.0;
        let aspect = if preferred_width > 0.0 {
            preferred_height / preferred_width
        } else {
            1.0
        };
        return (width, width * aspect);
    }
    let scale = (containing_width / preferred_width).min(1.0);
    (preferred_width * scale, preferred_height * scale)
}
