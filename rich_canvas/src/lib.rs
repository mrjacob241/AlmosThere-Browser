use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

use egui::{
    Align2, Button, Color32, ColorImage, CornerRadius, FontFamily, FontId, Frame, Margin, Pos2,
    Rect, RichText, ScrollArea, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2,
    epaint::TextShape,
    text::{LayoutJob, TextFormat},
    vec2,
};

const BROWSER_REGULAR_FONT_NAME: &str = "browser_regular";
const BROWSER_BOLD_FONT_NAME: &str = "browser_bold";
const IMAGE_MIN_SIZE: f32 = 24.0;

#[derive(Clone, Debug, Default)]
pub struct BrowserCanvas {
    pub zoom: f32,
    pub scroll_offset: Vec2,
}

#[derive(Clone, Debug, Default)]
pub struct BrowserDocument {
    pub title: String,
    pub source: String,
    pub style: BrowserStyle,
    pub canvas_graph: CanvasGraph,
    pub blocks: Vec<CanvasBlock>,
}

#[derive(Clone, Debug)]
pub struct BrowserStyle {
    pub page_background: Color32,
    pub text_color: Color32,
    pub link_color: Color32,
    pub body_font_size: f32,
    pub h1_font_size: f32,
    pub h2_font_size: f32,
    pub main_max_width: f32,
    pub main_padding_x: f32,
    pub main_padding_y: f32,
    pub panel_gap: f32,
    pub panel_padding: f32,
    pub panel_background: Color32,
    pub panel_border_color: Color32,
    pub panel_border_width: f32,
    pub panel_radius: u8,
    pub button_padding_x: f32,
    pub button_padding_y: f32,
    pub button_background: Color32,
    pub button_border_color: Color32,
    pub button_border_width: f32,
    pub button_radius: u8,
    pub input_padding: f32,
    pub input_border_color: Color32,
    pub input_border_width: f32,
    pub input_radius: u8,
    pub image_width_percent: Option<f32>,
    pub image_height_auto: bool,
    pub css_variables: HashMap<String, String>,
    pub block_rules: Vec<CssBlockRule>,
}

impl Default for BrowserStyle {
    fn default() -> Self {
        Self {
            page_background: Color32::from_rgb(245, 247, 250),
            text_color: Color32::from_rgb(31, 41, 51),
            link_color: Color32::from_rgb(7, 89, 133),
            body_font_size: 16.0,
            h1_font_size: 32.0,
            h2_font_size: 22.0,
            main_max_width: 760.0,
            main_padding_x: 20.0,
            main_padding_y: 32.0,
            panel_gap: 24.0,
            panel_padding: 16.0,
            panel_background: Color32::WHITE,
            panel_border_color: Color32::from_rgb(204, 214, 224),
            panel_border_width: 1.0,
            panel_radius: 8,
            button_padding_x: 12.0,
            button_padding_y: 8.0,
            button_background: Color32::from_rgb(224, 242, 254),
            button_border_color: Color32::from_rgb(7, 89, 133),
            button_border_width: 1.0,
            button_radius: 4,
            input_padding: 8.0,
            input_border_color: Color32::from_rgb(154, 166, 178),
            input_border_width: 1.0,
            input_radius: 4,
            image_width_percent: None,
            image_height_auto: false,
            css_variables: HashMap::new(),
            block_rules: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElementStyleKey {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<String>,
    pub parent: Option<Box<ElementStyleKey>>,
    pub previous_sibling: Option<Box<ElementStyleKey>>,
}

#[derive(Clone, Debug, Default)]
pub struct CssBlockRule {
    pub selector: CssSelector,
    pub style: CssBoxStyle,
    pub order: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<String>,
    pub ancestor: Option<SimpleCssSelector>,
    pub parent: Option<SimpleCssSelector>,
    pub previous_sibling: Option<SimpleCssSelector>,
    pub requires_previous_sibling: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimpleCssSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CssBoxStyle {
    pub display: Option<CssDisplay>,
    pub color: Option<Color32>,
    pub background: Option<Color32>,
    pub margin: Option<CssEdges>,
    pub margin_top: Option<f32>,
    pub margin_right: Option<f32>,
    pub margin_bottom: Option<f32>,
    pub margin_left: Option<f32>,
    pub margin_auto: CssEdgeAutoSpec,
    pub padding: Option<CssEdges>,
    pub padding_top: Option<f32>,
    pub padding_right: Option<f32>,
    pub padding_bottom: Option<f32>,
    pub padding_left: Option<f32>,
    pub border_width: Option<f32>,
    pub border_color: Option<Color32>,
    pub border_radius: Option<u8>,
    pub width: Option<CssLength>,
    pub max_width: Option<CssLength>,
    pub min_width: Option<CssLength>,
    pub height: Option<CssLength>,
    pub min_height: Option<CssLength>,
    pub font_size: Option<f32>,
    pub font_weight_bold: Option<bool>,
    pub font_style_italic: Option<bool>,
    pub text_decoration_underline: Option<bool>,
    pub text_decoration_strikethrough: Option<bool>,
    pub text_background: Option<Color32>,
    pub text_align: Option<CssTextAlign>,
    pub flex_grow: Option<f32>,
    pub flex_direction: Option<CssFlexDirection>,
    pub justify_content: Option<CssJustifyContent>,
    pub align_items: Option<CssAlignItems>,
    pub grid_template_columns: Option<usize>,
    pub gap: Option<f32>,
    pub overflow_hidden: Option<bool>,
    pub position: Option<CssPosition>,
    pub z_index: Option<i32>,
    pub inset: Option<CssEdges>,
    pub inset_sides: CssInset,
    pub object_fit: Option<CssObjectFit>,
}

#[derive(Clone, Debug)]
pub struct ResolvedBoxStyle {
    pub display: CssDisplay,
    pub color: Color32,
    pub background: Color32,
    pub margin: CssEdges,
    pub margin_auto: CssEdgeAuto,
    pub padding: CssEdges,
    pub border_width: f32,
    pub border_color: Color32,
    pub border_radius: u8,
    pub width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_width: Option<f32>,
    pub height: Option<CssLength>,
    pub min_height: Option<CssLength>,
    pub font_size: f32,
    pub font_weight_bold: bool,
    pub font_style_italic: bool,
    pub text_decoration_underline: bool,
    pub text_decoration_strikethrough: bool,
    pub text_background: Color32,
    pub text_align: CssTextAlign,
    pub flex_grow: f32,
    pub flex_direction: CssFlexDirection,
    pub justify_content: CssJustifyContent,
    pub align_items: CssAlignItems,
    pub grid_template_columns: Option<usize>,
    pub gap: f32,
    pub overflow_hidden: bool,
    pub position: CssPosition,
    pub z_index: Option<i32>,
    pub inset: Option<CssEdges>,
    pub inset_sides: CssInset,
    pub object_fit: CssObjectFit,
}

impl Default for ResolvedBoxStyle {
    fn default() -> Self {
        Self {
            display: CssDisplay::Block,
            color: BrowserStyle::default().text_color,
            background: Color32::TRANSPARENT,
            margin: CssEdges::default(),
            margin_auto: CssEdgeAuto::default(),
            padding: CssEdges::default(),
            border_width: 0.0,
            border_color: Color32::TRANSPARENT,
            border_radius: 0,
            width: None,
            max_width: None,
            min_width: None,
            height: None,
            min_height: None,
            font_size: BrowserStyle::default().body_font_size,
            font_weight_bold: false,
            font_style_italic: false,
            text_decoration_underline: false,
            text_decoration_strikethrough: false,
            text_background: Color32::TRANSPARENT,
            text_align: CssTextAlign::Left,
            flex_grow: 0.0,
            flex_direction: CssFlexDirection::Row,
            justify_content: CssJustifyContent::FlexStart,
            align_items: CssAlignItems::Stretch,
            grid_template_columns: None,
            gap: 0.0,
            overflow_hidden: false,
            position: CssPosition::Static,
            z_index: None,
            inset: None,
            inset_sides: CssInset::default(),
            object_fit: CssObjectFit::Fill,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssDisplay {
    None,
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    Table,
    ListItem,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CssEdges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CssEdgeAuto {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CssEdgeAutoSpec {
    pub top: Option<bool>,
    pub right: Option<bool>,
    pub bottom: Option<bool>,
    pub left: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CssInset {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CssLength {
    Auto,
    Px(f32),
    Percent(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssTextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssFlexDirection {
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssJustifyContent {
    FlexStart,
    Center,
    SpaceBetween,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssAlignItems {
    Stretch,
    Center,
    FlexStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssPosition {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssObjectFit {
    Fill,
    Contain,
    Cover,
}

#[derive(Clone, Debug, Default)]
pub struct CanvasGraph {
    pub viewport: Vec2,
    pub objects: Vec<CanvasObject>,
}

#[derive(Clone, Debug)]
pub enum CanvasObject {
    Text(CanvasTextObject),
    Rect(CanvasRectObject),
    Input(CanvasInputObject),
    Image(CanvasImageObject),
    Svg(CanvasSvgObject),
    Media(CanvasMediaObject),
    ClipStart(CanvasClipObject),
    ClipEnd,
}

#[derive(Clone, Debug)]
pub struct CanvasClipObject {
    pub rect: Rect,
    pub border_radius: u8,
}

#[derive(Clone, Debug)]
pub struct CanvasTextObject {
    pub text: String,
    pub rect: Rect,
    pub color: Color32,
    pub font_size: f32,
    pub font_weight_bold: bool,
    pub font_style_italic: bool,
    pub text_decoration_underline: bool,
    pub text_decoration_strikethrough: bool,
    pub text_background: Color32,
    pub text_align: CssTextAlign,
    pub href: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CanvasRectObject {
    pub rect: Rect,
    pub fill: Color32,
    pub border_color: Color32,
    pub border_width: f32,
    pub border_radius: u8,
}

#[derive(Clone, Debug)]
pub struct CanvasInputObject {
    pub label: String,
    pub value: String,
    pub rect: Rect,
    pub font_size: f32,
}

#[derive(Clone, Debug)]
pub struct CanvasImageObject {
    pub rect: Rect,
    pub src: String,
    pub alt: String,
    pub image: ImageBlock,
    pub object_fit: CssObjectFit,
}

#[derive(Clone, Debug)]
pub struct CanvasSvgObject {
    pub rect: Rect,
    pub svg: SvgBlock,
}

#[derive(Clone, Debug)]
pub struct CanvasMediaObject {
    pub rect: Rect,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasMeasuredText {
    pub text: String,
    pub size: Vec2,
}

#[derive(Clone, Debug)]
pub struct BrowserFontSizeLut {
    pub font_size: f32,
    pub font_weight_bold: bool,
    pub font_style_italic: bool,
    pub line_height: f32,
    pub glyph_sizes: HashMap<char, Vec2>,
    text_sizes: HashMap<String, Vec2>,
}

impl BrowserFontSizeLut {
    pub fn glyph_size(&self, character: char) -> Option<Vec2> {
        self.glyph_sizes.get(&character).copied()
    }

    pub fn text_size(&self, text: &str) -> Option<Vec2> {
        self.text_sizes.get(text).copied()
    }

    fn insert_text_size(&mut self, text: &str, size: Vec2) {
        self.text_sizes.insert(text.to_owned(), size);
    }
}

#[derive(Clone, Debug)]
pub enum CanvasBlock {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph {
        text: String,
    },
    Link {
        text: String,
        href: String,
    },
    InlineText {
        spans: Vec<InlineSpan>,
    },
    ListItem {
        depth: usize,
        ordered: bool,
        text: String,
        href: Option<String>,
    },
    Quote {
        text: String,
    },
    Rule,
    Preformatted {
        text: String,
    },
    Media {
        label: String,
    },
    Svg {
        svg: SvgBlock,
    },
    Image {
        alt: String,
        src: String,
        image: ImageBlock,
    },
    EcosiaHero {
        hero: EcosiaHeroBlock,
    },
    SearchResultsPage {
        page: SearchResultsPageBlock,
    },
    Table {
        caption: String,
        rows: Vec<Vec<String>>,
    },
    Button {
        text: String,
    },
    Input {
        label: String,
        value: String,
    },
    Panel {
        children: Vec<CanvasBlock>,
    },
    Box {
        style_key: ElementStyleKey,
        children: Vec<CanvasBlock>,
    },
    StyledBox {
        style: ResolvedBoxStyle,
        children: Vec<CanvasBlock>,
    },
}

#[derive(Clone, Debug)]
pub struct EcosiaHeroBlock {
    pub background_src: String,
    pub background: ImageBlock,
    pub search_placeholder: String,
    pub search_value: String,
    pub ai_button_text: String,
    pub tree_count: String,
    pub tree_description: String,
    pub investment_count: String,
    pub investment_description: String,
    pub seed_count: String,
    pub show_sign_in: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SearchResultsPageBlock {
    pub brand: String,
    pub query: String,
    pub nav_items: Vec<String>,
    pub region: String,
    pub videos: Vec<SearchMediaResult>,
    pub results: Vec<SearchResultItem>,
    pub sidebar: Option<SearchSidebarCard>,
    pub related_queries: Vec<String>,
    pub footer_cards: Vec<SearchFooterCard>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchMediaResult {
    pub title: String,
    pub source: String,
    pub href: String,
    pub image: Option<ImageBlock>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchResultItem {
    pub source_name: String,
    pub display_url: String,
    pub title: String,
    pub href: String,
    pub description: String,
    pub thumbnail: Option<ImageBlock>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchSidebarCard {
    pub title: String,
    pub description: String,
    pub links: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchFooterCard {
    pub title: String,
    pub link_text: String,
    pub image: Option<ImageBlock>,
}

#[derive(Clone, Debug)]
pub struct SvgBlock {
    pub size: Vec2,
    pub shapes: Vec<SvgShape>,
}

impl SvgBlock {
    pub fn new(size: Vec2, shapes: Vec<SvgShape>) -> Self {
        Self { size, shapes }
    }

    fn paint(&self, ui: &mut Ui, font_scale: f32) {
        let size = self.size * font_scale;
        let (rect, _response) = ui.allocate_exact_size(size, Sense::hover());
        self.paint_in_rect(ui, rect);
    }

    pub fn paint_in_rect(&self, ui: &mut Ui, rect: Rect) {
        let scale_x = rect.width() / self.size.x.max(1.0);
        let scale_y = rect.height() / self.size.y.max(1.0);
        let painter = ui.painter().with_clip_rect(rect);

        for shape in &self.shapes {
            match shape {
                SvgShape::PathFallback { fill } => {
                    let radius = rect.width().min(rect.height()) * 0.12;
                    let stroke =
                        Stroke::new(rect.width().min(rect.height()).max(1.0) * 0.12, *fill);
                    let center = rect.center();
                    let mark_width = rect.width() * 0.62;
                    let mark_height = rect.height() * 0.44;
                    painter.line_segment(
                        [
                            Pos2::new(center.x - mark_width * 0.5, center.y),
                            Pos2::new(center.x - mark_width * 0.15, center.y + mark_height * 0.35),
                        ],
                        stroke,
                    );
                    painter.line_segment(
                        [
                            Pos2::new(center.x - mark_width * 0.15, center.y + mark_height * 0.35),
                            Pos2::new(center.x + mark_width * 0.5, center.y - mark_height * 0.35),
                        ],
                        stroke,
                    );
                    painter.circle_filled(center, radius, *fill);
                }
                SvgShape::Rect {
                    x,
                    y,
                    width,
                    height,
                    fill,
                } => {
                    let rect = Rect::from_min_size(
                        Pos2::new(rect.left() + x * scale_x, rect.top() + y * scale_y),
                        Vec2::new(width * scale_x, height * scale_y),
                    );
                    painter.rect_filled(rect, 0.0, *fill);
                }
                SvgShape::Circle {
                    cx,
                    cy,
                    r,
                    fill,
                    stroke,
                    stroke_width,
                } => {
                    let center = Pos2::new(rect.left() + cx * scale_x, rect.top() + cy * scale_y);
                    let radius = r * scale_x.min(scale_y);
                    painter.circle_filled(center, radius, *fill);
                    if let Some(stroke) = stroke {
                        painter.circle_stroke(
                            center,
                            radius,
                            Stroke::new(stroke_width * scale_x.min(scale_y), *stroke),
                        );
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum SvgShape {
    PathFallback {
        fill: Color32,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: Color32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: Color32,
        stroke: Option<Color32>,
        stroke_width: f32,
    },
}

#[derive(Clone)]
pub struct ImageBlock {
    pub path: PathBuf,
    pub size: Vec2,
    pub color_image: ColorImage,
    texture: Option<TextureHandle>,
}

impl std::fmt::Debug for ImageBlock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageBlock")
            .field("path", &self.path)
            .field("size", &self.size)
            .field("color_image_size", &self.color_image.size)
            .finish()
    }
}

impl ImageBlock {
    pub fn from_color_image(path: PathBuf, size: Vec2, color_image: ColorImage) -> Self {
        Self {
            path,
            size: size.max(Vec2::splat(1.0)),
            color_image,
            texture: None,
        }
    }

    pub fn from_encoded_bytes(
        path: PathBuf,
        bytes: &[u8],
        requested_size: Option<Vec2>,
    ) -> Result<Self, image::ImageError> {
        Self::from_encoded_bytes_with_aspect(path, bytes, requested_size, false)
    }

    pub fn from_encoded_bytes_with_aspect(
        path: PathBuf,
        bytes: &[u8],
        requested_size: Option<Vec2>,
        preserve_aspect: bool,
    ) -> Result<Self, image::ImageError> {
        let rgba_image = decode_image_bytes(&path, bytes)?;
        let pixel_size = vec2(rgba_image.width() as f32, rgba_image.height() as f32);
        let size = if preserve_aspect {
            requested_size
                .map(|requested| aspect_preserving_size(pixel_size, requested))
                .unwrap_or(pixel_size)
        } else {
            requested_size.unwrap_or(pixel_size)
        };
        let color_image = ColorImage::from_rgba_unmultiplied(
            [rgba_image.width() as usize, rgba_image.height() as usize],
            rgba_image.as_raw(),
        );

        Ok(Self {
            path,
            size: size.max(Vec2::splat(IMAGE_MIN_SIZE)),
            color_image,
            texture: None,
        })
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, image::ImageError> {
        let path = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&path)?;
        Self::from_encoded_bytes(path, &bytes, None)
    }

    pub fn reload_from_path(&mut self, path: impl AsRef<Path>) -> Result<(), image::ImageError> {
        let replacement = Self::from_encoded_bytes(
            path.as_ref().to_path_buf(),
            &std::fs::read(path.as_ref())?,
            Some(self.size),
        )?;

        self.path = replacement.path;
        self.color_image = replacement.color_image;
        self.texture = None;
        Ok(())
    }

    pub fn texture_handle(&mut self, ui: &Ui, image_id: &str) -> TextureHandle {
        if self.texture.is_none() {
            let texture_name = format!("embedded-browser-image-{image_id}");
            self.texture = Some(ui.ctx().load_texture(
                texture_name,
                self.color_image.clone(),
                TextureOptions::LINEAR,
            ));
        }

        self.texture
            .as_ref()
            .expect("texture is initialized above")
            .clone()
    }

    pub fn invalidate_texture(&mut self) {
        self.texture = None;
    }

    fn paint(&mut self, ui: &mut Ui, src: &str, style: &BrowserStyle, font_scale: f32) {
        let available_width = ui.available_width().max(IMAGE_MIN_SIZE);
        let scaled_size = self.size * font_scale;
        let display_size = image_display_size(scaled_size, available_width, style);
        let (rect, _) = ui.allocate_exact_size(display_size, Sense::hover());
        let texture = self.texture_handle(ui, src);
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    fn paint_cover(&mut self, ui: &mut Ui, src: &str, rect: Rect, tint: Color32) {
        let texture = self.texture_handle(ui, src);
        let source_size = self.size.max(Vec2::splat(1.0));
        let target_aspect = rect.width() / rect.height().max(1.0);
        let source_aspect = source_size.x / source_size.y.max(1.0);
        let uv = if source_aspect > target_aspect {
            let visible_width = target_aspect / source_aspect;
            let inset = (1.0 - visible_width) * 0.5;
            Rect::from_min_max(Pos2::new(inset, 0.0), Pos2::new(1.0 - inset, 1.0))
        } else {
            let visible_height = source_aspect / target_aspect;
            let inset = (1.0 - visible_height) * 0.5;
            Rect::from_min_max(Pos2::new(0.0, inset), Pos2::new(1.0, 1.0 - inset))
        };
        ui.painter().image(texture.id(), rect, uv, tint);
    }
}

fn decode_image_bytes(path: &Path, bytes: &[u8]) -> Result<image::RgbaImage, image::ImageError> {
    match image::load_from_memory(bytes) {
        Ok(image) => Ok(image.to_rgba8()),
        Err(error) => {
            if let Some(format) = path
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(image::ImageFormat::from_extension)
            {
                return image::ImageReader::with_format(Cursor::new(bytes), format)
                    .decode()
                    .map(|image| image.to_rgba8());
            }
            Err(error)
        }
    }
}

fn image_display_size(
    scaled_size: Vec2,
    containing_block_width: f32,
    style: &BrowserStyle,
) -> Vec2 {
    let containing_block_width = containing_block_width.max(IMAGE_MIN_SIZE);
    if let (Some(width_percent), true) = (style.image_width_percent, style.image_height_auto) {
        let aspect = if scaled_size.x > 0.0 {
            scaled_size.y / scaled_size.x
        } else {
            1.0
        };
        let width = (containing_block_width * width_percent / 100.0).max(IMAGE_MIN_SIZE);
        return vec2(width, width * aspect).max(Vec2::splat(IMAGE_MIN_SIZE));
    }

    let fit_scale = (containing_block_width / scaled_size.x).min(1.0);
    (scaled_size * fit_scale).max(Vec2::splat(IMAGE_MIN_SIZE))
}

fn aspect_preserving_size(pixel_size: Vec2, requested_size: Vec2) -> Vec2 {
    if pixel_size.x <= 0.0 || pixel_size.y <= 0.0 {
        return requested_size;
    }
    let aspect = pixel_size.y / pixel_size.x;
    if requested_size.x > 0.0 {
        vec2(requested_size.x, requested_size.x * aspect)
    } else if requested_size.y > 0.0 {
        vec2(requested_size.y / aspect, requested_size.y)
    } else {
        pixel_size
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineSpan {
    pub text: String,
    pub href: Option<String>,
    pub strong: bool,
    pub emphasis: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub small: bool,
    pub raised: bool,
    pub lowered: bool,
    pub highlight: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitTarget {
    Link { href: String },
    Button { text: String },
    Input { label: String },
}

impl BrowserDocument {
    pub fn input_value_mut(&mut self, label_to_find: &str) -> Option<&mut String> {
        self.blocks.iter_mut().find_map(|block| match block {
            CanvasBlock::Input { label, value } if label == label_to_find => Some(value),
            CanvasBlock::EcosiaHero { hero } if label_to_find == "Search" => {
                Some(&mut hero.search_value)
            }
            CanvasBlock::Panel { children } => children.iter_mut().find_map(|child| match child {
                CanvasBlock::Input { label, value } if label == label_to_find => Some(value),
                CanvasBlock::EcosiaHero { hero } if label_to_find == "Search" => {
                    Some(&mut hero.search_value)
                }
                _ => None,
            }),
            CanvasBlock::Box { children, .. } => {
                children.iter_mut().find_map(|child| match child {
                    CanvasBlock::Input { label, value } if label == label_to_find => Some(value),
                    CanvasBlock::EcosiaHero { hero } if label_to_find == "Search" => {
                        Some(&mut hero.search_value)
                    }
                    _ => None,
                })
            }
            CanvasBlock::StyledBox { children, .. } => {
                children.iter_mut().find_map(|child| match child {
                    CanvasBlock::Input { label, value } if label == label_to_find => Some(value),
                    CanvasBlock::EcosiaHero { hero } if label_to_find == "Search" => {
                        Some(&mut hero.search_value)
                    }
                    _ => None,
                })
            }
            _ => None,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct BrowserCanvasResponse {
    pub clicked: Option<HitTarget>,
    pub hovered: Option<HitTarget>,
    pub changed_inputs: Vec<InputChange>,
    pub submitted_inputs: Vec<InputSubmit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputChange {
    pub label: String,
    pub value_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputSubmit {
    pub label: String,
    pub value: String,
}

impl BrowserCanvas {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            scroll_offset: Vec2::ZERO,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, document: &mut BrowserDocument) -> BrowserCanvasResponse {
        let mut canvas_response = BrowserCanvasResponse::default();
        let font_scale = self.zoom.clamp(0.75, 2.0);
        let style = document.style.clone();

        let output = ScrollArea::vertical()
            .id_salt("almostthere_browser_canvas")
            .auto_shrink(false)
            .scroll_offset(self.scroll_offset)
            .show(ui, |ui| {
                let available_width = ui.available_width();
                ui.set_min_width(available_width);
                let main_max_width = if document_prefers_wide_layout(document)
                    || document.canvas_graph.viewport.x > style.main_max_width
                {
                    available_width / font_scale
                } else {
                    style.main_max_width
                };
                let content_width = (available_width - style.main_padding_x * 2.0 * font_scale)
                    .min(main_max_width * font_scale)
                    .max(280.0);
                let left_margin = ((available_width - content_width) * 0.5)
                    .max(style.main_padding_x * font_scale);

                ui.add_space(style.main_padding_y * font_scale);
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    ui.vertical(|ui| {
                        ui.set_width(content_width);
                        if document.canvas_graph.objects.is_empty() {
                            for block in &mut document.blocks {
                                paint_block(ui, block, &style, font_scale, &mut canvas_response);
                            }
                        } else {
                            paint_canvas_graph(
                                ui,
                                &mut document.canvas_graph,
                                content_width,
                                font_scale,
                                &mut canvas_response,
                            );
                        }
                    });
                });
            });
        self.scroll_offset = output.state.offset;

        canvas_response
    }

    pub fn canvas_graph_ui(
        &mut self,
        ui: &mut Ui,
        style: &BrowserStyle,
        graph: &mut CanvasGraph,
    ) -> BrowserCanvasResponse {
        let mut canvas_response = BrowserCanvasResponse::default();
        let font_scale = self.zoom.clamp(0.75, 2.0);

        let output = ScrollArea::vertical()
            .id_salt("almostthere_browser_debug_canvas")
            .auto_shrink(false)
            .scroll_offset(self.scroll_offset)
            .show(ui, |ui| {
                let available_width = ui.available_width();
                ui.set_min_width(available_width);
                let main_max_width = if graph.viewport.x > style.main_max_width {
                    available_width / font_scale
                } else {
                    style.main_max_width
                };
                let content_width = (available_width - style.main_padding_x * 2.0 * font_scale)
                    .min(main_max_width * font_scale)
                    .max(280.0);
                let left_margin = ((available_width - content_width) * 0.5)
                    .max(style.main_padding_x * font_scale);

                ui.add_space(style.main_padding_y * font_scale);
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    ui.vertical(|ui| {
                        ui.set_width(content_width);
                        paint_canvas_graph(
                            ui,
                            graph,
                            content_width,
                            font_scale,
                            &mut canvas_response,
                        );
                    });
                });
            });
        self.scroll_offset = output.state.offset;

        canvas_response
    }
}

fn document_prefers_wide_layout(document: &BrowserDocument) -> bool {
    document.blocks.iter().any(|block| {
        matches!(
            block,
            CanvasBlock::EcosiaHero { .. } | CanvasBlock::SearchResultsPage { .. }
        )
    })
}

fn paint_canvas_graph(
    ui: &mut Ui,
    graph: &mut CanvasGraph,
    content_width: f32,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    let graph_width = graph.viewport.x.max(1.0);
    let scale = (content_width / graph_width).max(0.1) * font_scale;
    let graph_size = vec2(content_width.max(1.0), (graph.viewport.y * scale).max(1.0));
    let (canvas_rect, _) = ui.allocate_exact_size(graph_size, Sense::hover());
    let mut painter = ui.painter().with_clip_rect(canvas_rect);
    let mut clip_stack = Vec::new();

    for (index, object) in graph.objects.iter_mut().enumerate() {
        match object {
            CanvasObject::ClipStart(clip) => {
                let rect = canvas_object_rect(canvas_rect.min, clip.rect, scale);
                let previous = painter.clip_rect();
                let next = previous.intersect(rect);
                clip_stack.push(previous);
                painter = ui.painter().with_clip_rect(next);
            }
            CanvasObject::ClipEnd => {
                let previous = clip_stack.pop().unwrap_or(canvas_rect);
                painter = ui.painter().with_clip_rect(previous);
            }
            CanvasObject::Text(text) => {
                let rect = canvas_object_rect(canvas_rect.min, text.rect, scale);
                let family = if text.font_weight_bold {
                    browser_bold_family()
                } else {
                    browser_regular_family()
                };
                let font_size = text.font_size * scale;
                let stroke = Stroke::new((1.0 * scale).max(1.0), text.color);
                let text_format = TextFormat {
                    font_id: FontId::new(font_size, family),
                    color: text.color,
                    background: text.text_background,
                    italics: text.font_style_italic,
                    underline: if text.text_decoration_underline {
                        stroke
                    } else {
                        Stroke::NONE
                    },
                    strikethrough: if text.text_decoration_strikethrough {
                        stroke
                    } else {
                        Stroke::NONE
                    },
                    line_height: Some(rect.height().max(font_size)),
                    ..Default::default()
                };
                let galley =
                    painter.layout_job(LayoutJob::simple_format(text.text.clone(), text_format));
                let text_width = galley.size().x.min(rect.width()).max(1.0);
                let text_rect = match text.text_align {
                    CssTextAlign::Left => {
                        Rect::from_min_size(rect.min, vec2(text_width, rect.height()))
                    }
                    CssTextAlign::Center => Rect::from_center_size(
                        Pos2::new(rect.center().x, rect.center().y),
                        vec2(text_width, rect.height()),
                    ),
                    CssTextAlign::Right => Rect::from_min_size(
                        Pos2::new(rect.right() - text_width, rect.top()),
                        vec2(text_width, rect.height()),
                    ),
                };
                painter.add(TextShape::new(text_rect.left_top(), galley, text.color));
                if let Some(href) = &text.href {
                    let response = ui.interact(
                        rect,
                        ui.make_persistent_id(("canvas_graph_text", index)),
                        Sense::click(),
                    );
                    if response.hovered() {
                        canvas_response.hovered = Some(HitTarget::Link { href: href.clone() });
                    }
                    if response.clicked() {
                        canvas_response.clicked = Some(HitTarget::Link { href: href.clone() });
                    }
                }
            }
            CanvasObject::Rect(rect_object) => {
                let rect = canvas_object_rect(canvas_rect.min, rect_object.rect, scale);
                painter.rect(
                    rect,
                    CornerRadius::same(rect_object.border_radius),
                    rect_object.fill,
                    Stroke::new(rect_object.border_width * scale, rect_object.border_color),
                    egui::StrokeKind::Outside,
                );
            }
            CanvasObject::Input(input) => {
                let rect = canvas_object_rect(canvas_rect.min, input.rect, scale);
                let mut value = input.value.clone();
                let response = ui.put(
                    rect,
                    egui::TextEdit::singleline(&mut value)
                        .hint_text(input.label.as_str())
                        .frame(false)
                        .font(FontId::new(
                            input.font_size * scale,
                            browser_regular_family(),
                        ))
                        .text_color(Color32::WHITE),
                );
                if response.hovered() {
                    canvas_response.hovered = Some(HitTarget::Input {
                        label: input.label.clone(),
                    });
                }
                if response.changed() {
                    input.value = value.clone();
                    canvas_response.changed_inputs.push(InputChange {
                        label: input.label.clone(),
                        value_len: value.chars().count(),
                    });
                }
                if response.lost_focus() && ui.input(|state| state.key_pressed(egui::Key::Enter)) {
                    canvas_response.submitted_inputs.push(InputSubmit {
                        label: input.label.clone(),
                        value,
                    });
                }
            }
            CanvasObject::Image(image) => {
                let rect = canvas_object_rect(canvas_rect.min, image.rect, scale);
                let texture = image.image.texture_handle(ui, &image.src);
                let uv = match image.object_fit {
                    CssObjectFit::Cover => cover_image_uv(image.image.size, image.rect.size()),
                    CssObjectFit::Contain | CssObjectFit::Fill => {
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
                    }
                };
                painter.image(texture.id(), rect, uv, Color32::WHITE);
            }
            CanvasObject::Svg(svg) => {
                let rect = canvas_object_rect(canvas_rect.min, svg.rect, scale);
                svg.svg.paint_in_rect(ui, rect);
            }
            CanvasObject::Media(media) => {
                let rect = canvas_object_rect(canvas_rect.min, media.rect, scale);
                painter.rect(
                    rect,
                    CornerRadius::same(3),
                    Color32::from_rgb(238, 241, 245),
                    Stroke::new(1.0 * scale, Color32::from_rgb(195, 205, 215)),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    media.label.as_str(),
                    FontId::new(14.0 * scale, browser_regular_family()),
                    Color32::from_rgb(90, 100, 110),
                );
            }
        }
    }
}

fn cover_image_uv(source_size: Vec2, target_size: Vec2) -> Rect {
    let source_size = source_size.max(Vec2::splat(1.0));
    let target_size = target_size.max(Vec2::splat(1.0));
    let target_aspect = target_size.x / target_size.y;
    let source_aspect = source_size.x / source_size.y;
    if source_aspect > target_aspect {
        let visible_width = target_aspect / source_aspect;
        let inset = (1.0 - visible_width) * 0.5;
        Rect::from_min_max(Pos2::new(inset, 0.0), Pos2::new(1.0 - inset, 1.0))
    } else {
        let visible_height = source_aspect / target_aspect;
        let inset = (1.0 - visible_height) * 0.5;
        Rect::from_min_max(Pos2::new(0.0, inset), Pos2::new(1.0, 1.0 - inset))
    }
}

fn canvas_object_rect(origin: Pos2, rect: Rect, scale: f32) -> Rect {
    Rect::from_min_size(origin + rect.min.to_vec2() * scale, rect.size() * scale)
}

pub fn configure_browser_fonts(ctx: &egui::Context) {
    let regular_font = include_bytes!("../../fonts/LiberationSans-Regular.ttf").to_vec();
    let bold_font = include_bytes!("../../fonts/LiberationSans-Bold.ttf").to_vec();

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        BROWSER_REGULAR_FONT_NAME.into(),
        Arc::new(egui::FontData::from_owned(regular_font)),
    );
    fonts.font_data.insert(
        BROWSER_BOLD_FONT_NAME.into(),
        Arc::new(egui::FontData::from_owned(bold_font)),
    );
    fonts.families.insert(
        FontFamily::Name(BROWSER_REGULAR_FONT_NAME.into()),
        vec![BROWSER_REGULAR_FONT_NAME.into()],
    );
    fonts.families.insert(
        FontFamily::Name(BROWSER_BOLD_FONT_NAME.into()),
        vec![BROWSER_BOLD_FONT_NAME.into()],
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, BROWSER_REGULAR_FONT_NAME.into());
    ctx.set_fonts(fonts);
}

pub fn measure_browser_textbox(ctx: &egui::Context, text: &str, style: &ResolvedBoxStyle) -> Vec2 {
    browser_text_galley(ctx, text, style).size()
}

pub fn calculate_browser_font_size_lut(
    ctx: &egui::Context,
    style: &ResolvedBoxStyle,
) -> BrowserFontSizeLut {
    let mut glyph_sizes = HashMap::new();
    for byte in 32_u8..=126 {
        let character = byte as char;
        glyph_sizes.insert(
            character,
            measure_browser_textbox(ctx, &character.to_string(), style),
        );
    }

    BrowserFontSizeLut {
        font_size: style.font_size,
        font_weight_bold: style.font_weight_bold,
        font_style_italic: style.font_style_italic,
        line_height: measure_browser_textbox(ctx, "M", style)
            .y
            .max((style.font_size * 1.35).max(1.0)),
        glyph_sizes,
        text_sizes: HashMap::new(),
    }
}

pub fn wrap_browser_textboxes(
    ctx: Option<&egui::Context>,
    text: &str,
    max_width: f32,
    style: &ResolvedBoxStyle,
) -> Vec<CanvasMeasuredText> {
    let max_width = max_width.max(1.0);
    if text.is_empty() {
        return Vec::new();
    }

    let Some(ctx) = ctx else {
        return wrap_browser_textboxes_estimated(text, max_width, style.font_size);
    };
    let mut lut = calculate_browser_font_size_lut(ctx, style);

    let text_size = measure_browser_textbox_with_lut(ctx, &mut lut, text, style);
    if text_size.x <= max_width {
        return vec![CanvasMeasuredText {
            text: text.to_owned(),
            size: text_size,
        }];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty()
            && measure_browser_textbox_with_lut(ctx, &mut lut, &candidate, style).x > max_width
        {
            lines.push(CanvasMeasuredText {
                size: measure_browser_textbox_with_lut(ctx, &mut lut, &current, style),
                text: std::mem::take(&mut current),
            });
        }
        if current.is_empty() {
            if measure_browser_textbox_with_lut(ctx, &mut lut, word, style).x <= max_width {
                current.push_str(word);
            } else {
                lines.extend(split_long_browser_word(
                    ctx, &mut lut, word, max_width, style,
                ));
            }
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(CanvasMeasuredText {
            size: measure_browser_textbox_with_lut(ctx, &mut lut, &current, style),
            text: current,
        });
    }
    lines
}

fn measure_browser_textbox_with_lut(
    ctx: &egui::Context,
    lut: &mut BrowserFontSizeLut,
    text: &str,
    style: &ResolvedBoxStyle,
) -> Vec2 {
    if let Some(size) = lut.text_size(text) {
        return size;
    }

    let size = measure_browser_textbox(ctx, text, style);
    lut.insert_text_size(text, size);
    size
}

fn split_long_browser_word(
    ctx: &egui::Context,
    lut: &mut BrowserFontSizeLut,
    word: &str,
    max_width: f32,
    style: &ResolvedBoxStyle,
) -> Vec<CanvasMeasuredText> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for character in word.chars() {
        let candidate = format!("{current}{character}");
        if !current.is_empty()
            && measure_browser_textbox_with_lut(ctx, lut, &candidate, style).x > max_width
        {
            lines.push(CanvasMeasuredText {
                size: measure_browser_textbox_with_lut(ctx, lut, &current, style),
                text: std::mem::take(&mut current),
            });
        }
        current.push(character);
    }
    if !current.is_empty() {
        lines.push(CanvasMeasuredText {
            size: measure_browser_textbox_with_lut(ctx, lut, &current, style),
            text: current,
        });
    }
    lines
}

fn browser_text_galley(
    ctx: &egui::Context,
    text: &str,
    style: &ResolvedBoxStyle,
) -> std::sync::Arc<egui::Galley> {
    let family = if style.font_weight_bold {
        browser_bold_family()
    } else {
        browser_regular_family()
    };
    let stroke = Stroke::new(1.0, style.color);
    let text_format = TextFormat {
        font_id: FontId::new(style.font_size, family),
        color: style.color,
        background: style.text_background,
        italics: style.font_style_italic,
        underline: if style.text_decoration_underline {
            stroke
        } else {
            Stroke::NONE
        },
        strikethrough: if style.text_decoration_strikethrough {
            stroke
        } else {
            Stroke::NONE
        },
        line_height: Some((style.font_size * 1.35).max(1.0)),
        ..Default::default()
    };
    ctx.fonts_mut(|fonts| fonts.layout_job(LayoutJob::simple_format(text.to_owned(), text_format)))
}

fn wrap_browser_textboxes_estimated(
    text: &str,
    max_width: f32,
    font_size: f32,
) -> Vec<CanvasMeasuredText> {
    let max_chars = (max_width / (font_size * 0.56).max(1.0)).floor().max(8.0) as usize;
    if text.chars().count() <= max_chars {
        return vec![CanvasMeasuredText {
            text: text.to_owned(),
            size: vec2(
                estimated_browser_text_width(text, font_size).min(max_width),
                font_size * 1.35,
            ),
        }];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if !current.is_empty()
            && current.chars().count() + separator + word.chars().count() > max_chars
        {
            lines.push(CanvasMeasuredText {
                size: vec2(
                    estimated_browser_text_width(&current, font_size).min(max_width),
                    font_size * 1.35,
                ),
                text: std::mem::take(&mut current),
            });
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(CanvasMeasuredText {
            size: vec2(
                estimated_browser_text_width(&current, font_size).min(max_width),
                font_size * 1.35,
            ),
            text: current,
        });
    }
    lines
}

fn estimated_browser_text_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * 0.56
}

fn paint_block(
    ui: &mut Ui,
    block: &mut CanvasBlock,
    style: &BrowserStyle,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    match block {
        CanvasBlock::Heading { level, text } => {
            let size = match level {
                1 => style.h1_font_size,
                2 => style.h2_font_size,
                3 => 20.0,
                _ => 18.0,
            } * font_scale;
            let heading = RichText::new(text.as_str())
                .size(size)
                .family(browser_bold_family())
                .strong()
                .color(style.text_color);
            if heading_centered_by_default(*level) {
                ui.vertical_centered(|ui| {
                    ui.label(heading);
                });
            } else {
                ui.label(heading);
            }
            ui.add_space(if *level == 1 { 18.0 } else { 12.0 } * font_scale);
        }
        CanvasBlock::Paragraph { text } => {
            ui.label(
                RichText::new(text.as_str())
                    .size(style.body_font_size * font_scale)
                    .family(browser_regular_family())
                    .color(style.text_color),
            );
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::Link { text, href } => {
            let response = ui.link(
                RichText::new(text.as_str())
                    .size(style.body_font_size * font_scale)
                    .family(browser_regular_family())
                    .color(style.link_color)
                    .underline(),
            );
            if response.hovered() {
                canvas_response.hovered = Some(HitTarget::Link { href: href.clone() });
            }
            if response.clicked() {
                canvas_response.clicked = Some(HitTarget::Link { href: href.clone() });
            }
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::InlineText { spans } => {
            paint_inline_spans(ui, spans, style, font_scale, canvas_response);
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::ListItem {
            depth,
            ordered,
            text,
            href,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add_space((*depth as f32 * 22.0 + 10.0) * font_scale);
                ui.label(
                    RichText::new(if *ordered { "1." } else { "•" })
                        .size(style.body_font_size * font_scale)
                        .family(browser_regular_family())
                        .color(style.text_color),
                );
                if let Some(href) = href {
                    let response = ui.link(
                        RichText::new(text.as_str())
                            .size(style.body_font_size * font_scale)
                            .family(browser_regular_family())
                            .color(style.link_color)
                            .underline(),
                    );
                    if response.hovered() {
                        canvas_response.hovered = Some(HitTarget::Link { href: href.clone() });
                    }
                    if response.clicked() {
                        canvas_response.clicked = Some(HitTarget::Link { href: href.clone() });
                    }
                } else {
                    ui.label(
                        RichText::new(text.as_str())
                            .size(style.body_font_size * font_scale)
                            .family(browser_regular_family())
                            .color(style.text_color),
                    );
                }
            });
            ui.add_space(6.0 * font_scale);
        }
        CanvasBlock::Quote { text } => {
            Frame::new()
                .stroke(Stroke::new(
                    2.0 * font_scale,
                    Color32::from_rgb(180, 190, 200),
                ))
                .inner_margin(Margin::symmetric(
                    (14.0 * font_scale) as i8,
                    (8.0 * font_scale) as i8,
                ))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text.as_str())
                            .size(style.body_font_size * font_scale)
                            .family(browser_regular_family())
                            .italics()
                            .color(style.text_color),
                    );
                });
            ui.add_space(14.0 * font_scale);
        }
        CanvasBlock::Rule => {
            ui.add_space(8.0 * font_scale);
            ui.separator();
            ui.add_space(18.0 * font_scale);
        }
        CanvasBlock::Preformatted { text } => {
            Frame::new()
                .fill(Color32::from_rgb(248, 248, 248))
                .stroke(Stroke::new(1.0, Color32::from_rgb(210, 210, 210)))
                .inner_margin(Margin::same((10.0 * font_scale) as i8))
                .show(ui, |ui| {
                    ui.monospace(text.as_str());
                });
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::Media { label } => {
            Frame::new()
                .fill(Color32::from_rgb(238, 241, 245))
                .stroke(Stroke::new(1.0, Color32::from_rgb(195, 205, 215)))
                .corner_radius(CornerRadius::same(3))
                .inner_margin(Margin::symmetric(
                    (12.0 * font_scale) as i8,
                    (18.0 * font_scale) as i8,
                ))
                .show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new(label.as_str())
                                .size(style.body_font_size * font_scale)
                                .family(browser_regular_family())
                                .color(Color32::from_rgb(90, 100, 110)),
                        );
                    });
                });
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::Svg { svg } => {
            svg.paint(ui, font_scale);
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::Image { src, image, .. } => {
            image.paint(ui, src, style, font_scale);
            ui.add_space(16.0 * font_scale);
        }
        CanvasBlock::EcosiaHero { hero } => {
            paint_ecosia_hero(ui, hero, style, font_scale, canvas_response);
            ui.add_space(24.0 * font_scale);
        }
        CanvasBlock::SearchResultsPage { page } => {
            paint_search_results_page(ui, page, font_scale, canvas_response);
            ui.add_space(24.0 * font_scale);
        }
        CanvasBlock::Table { caption, rows } => {
            if !caption.is_empty() {
                ui.label(
                    RichText::new(caption.as_str())
                        .size(style.body_font_size * font_scale)
                        .family(browser_bold_family())
                        .strong()
                        .color(style.text_color),
                );
                ui.add_space(6.0 * font_scale);
            }
            egui::Grid::new(format!("table_{:p}", rows))
                .striped(true)
                .spacing([18.0 * font_scale, 8.0 * font_scale])
                .show(ui, |ui| {
                    for row in rows {
                        for cell in row {
                            ui.label(
                                RichText::new(cell.as_str())
                                    .size(14.0 * font_scale)
                                    .family(browser_regular_family())
                                    .color(style.text_color),
                            );
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(18.0 * font_scale);
        }
        CanvasBlock::Button { text } => {
            let button_width = button_width_for_text(text, style, font_scale);
            let button_height =
                style.body_font_size * font_scale + style.button_padding_y * 2.0 * font_scale + 2.0;
            let response = ui.add_sized(
                [button_width, button_height],
                Button::new(
                    RichText::new(text.as_str())
                        .size(14.0 * font_scale)
                        .family(browser_regular_family())
                        .color(style.link_color),
                )
                .fill(style.button_background)
                .stroke(Stroke::new(
                    style.button_border_width,
                    style.button_border_color,
                ))
                .corner_radius(CornerRadius::same(style.button_radius)),
            );
            if response.hovered() {
                canvas_response.hovered = Some(HitTarget::Button { text: text.clone() });
            }
            if response.clicked() {
                canvas_response.clicked = Some(HitTarget::Button { text: text.clone() });
            }
            ui.add_space(1.0 * font_scale);
        }
        CanvasBlock::Input { label, value } => {
            ui.label(
                RichText::new(label.as_str())
                    .size(16.0 * font_scale)
                    .family(browser_bold_family())
                    .strong()
                    .color(style.text_color),
            );
            ui.add_space(6.0 * font_scale);
            let response = ui
                .scope(|ui| {
                    let input_stroke = Stroke::new(
                        style.input_border_width * font_scale,
                        style.input_border_color,
                    );
                    let widgets = &mut ui.style_mut().visuals.widgets;
                    widgets.inactive.bg_stroke = input_stroke;
                    widgets.hovered.bg_stroke = input_stroke;
                    widgets.open.bg_stroke = input_stroke;

                    ui.add_sized(
                        [ui.available_width(), 34.0 * font_scale],
                        egui::TextEdit::singleline(value),
                    )
                })
                .inner;
            if response.hovered() {
                canvas_response.hovered = Some(HitTarget::Input {
                    label: label.clone(),
                });
            }
            if response.changed() {
                canvas_response.changed_inputs.push(InputChange {
                    label: label.clone(),
                    value_len: value.chars().count(),
                });
            }
            ui.add_space(2.0 * font_scale);
        }
        CanvasBlock::Panel { children } => {
            Frame::new()
                .fill(style.panel_background)
                .stroke(Stroke::new(
                    style.panel_border_width,
                    style.panel_border_color,
                ))
                .corner_radius(CornerRadius::same(style.panel_radius))
                .inner_margin(Margin::same(style.panel_padding as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for child in children {
                        paint_block(ui, child, style, font_scale, canvas_response);
                    }
                });
            ui.add_space(style.panel_gap * font_scale);
        }
        CanvasBlock::Box {
            style_key,
            children,
        } => {
            paint_css_box(ui, style_key, children, style, font_scale, canvas_response);
        }
        CanvasBlock::StyledBox {
            style: box_style,
            children,
        } => {
            paint_resolved_css_box(ui, box_style, children, style, font_scale, canvas_response);
        }
    }
}

fn paint_css_box(
    ui: &mut Ui,
    style_key: &ElementStyleKey,
    children: &mut [CanvasBlock],
    document_style: &BrowserStyle,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    let box_style = computed_box_style(document_style, style_key);
    if box_style.display == Some(CssDisplay::None) {
        return;
    }

    if let Some(margin) = box_style.margin {
        ui.add_space(margin.top * font_scale);
    }

    let available_width = ui.available_width().max(1.0);
    let target_width = box_style
        .width
        .map(|length| css_length_px(length, available_width))
        .or_else(|| {
            box_style
                .max_width
                .map(|length| available_width.min(css_length_px(length, available_width)))
        })
        .unwrap_or(available_width);
    let target_width = if let Some(min_width) = box_style.min_width {
        target_width.max(css_length_px(min_width, available_width))
    } else {
        target_width
    }
    .min(available_width)
    .max(1.0);

    let padding = box_style.padding.unwrap_or_default();
    let fill = box_style.background.unwrap_or(Color32::TRANSPARENT);
    let stroke = Stroke::new(
        box_style.border_width.unwrap_or(0.0) * font_scale,
        box_style.border_color.unwrap_or(Color32::TRANSPARENT),
    );
    let radius = box_style.border_radius.unwrap_or(0);
    let inner_margin = Margin {
        left: (padding.left * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
        right: (padding.right * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
        top: (padding.top * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
        bottom: (padding.bottom * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
    };

    let mut paint_children = |ui: &mut Ui, children: &mut [CanvasBlock]| {
        ui.set_width(target_width);
        for child in children {
            paint_block(ui, child, document_style, font_scale, canvas_response);
        }
    };

    if fill == Color32::TRANSPARENT && stroke.width <= 0.0 && padding == CssEdges::default() {
        ui.set_width(target_width);
        paint_children(ui, children);
    } else {
        Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(radius))
            .inner_margin(inner_margin)
            .show(ui, |ui| paint_children(ui, children));
    }

    if let Some(margin) = box_style.margin {
        ui.add_space(margin.bottom * font_scale);
    }
}

fn css_length_px(length: CssLength, containing_width: f32) -> f32 {
    match length {
        CssLength::Auto => containing_width,
        CssLength::Px(px) => px,
        CssLength::Percent(percent) => containing_width * percent / 100.0,
    }
}

fn paint_resolved_css_box(
    ui: &mut Ui,
    box_style: &ResolvedBoxStyle,
    children: &mut [CanvasBlock],
    document_style: &BrowserStyle,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    if box_style.display == CssDisplay::None {
        return;
    }

    ui.add_space(box_style.margin.top * font_scale);

    let available_width = ui.available_width().max(1.0);
    let mut target_width = box_style
        .width
        .or(box_style
            .max_width
            .map(|max_width| available_width.min(max_width)))
        .unwrap_or(available_width);
    if let Some(min_width) = box_style.min_width {
        target_width = target_width.max(min_width);
    }
    target_width = target_width.min(available_width).max(1.0);

    let fill = box_style.background;
    let stroke = Stroke::new(box_style.border_width * font_scale, box_style.border_color);
    let padding = box_style.padding;
    let inner_margin = Margin {
        left: (padding.left * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
        right: (padding.right * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
        top: (padding.top * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
        bottom: (padding.bottom * font_scale)
            .round()
            .clamp(0.0, i8::MAX as f32) as i8,
    };

    let mut paint_children = |ui: &mut Ui, children: &mut [CanvasBlock]| {
        ui.set_width(target_width);
        for child in children {
            paint_block(ui, child, document_style, font_scale, canvas_response);
        }
    };

    if fill == Color32::TRANSPARENT && stroke.width <= 0.0 && padding == CssEdges::default() {
        ui.set_width(target_width);
        paint_children(ui, children);
    } else {
        Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(box_style.border_radius))
            .inner_margin(inner_margin)
            .show(ui, |ui| paint_children(ui, children));
    }

    ui.add_space(box_style.margin.bottom * font_scale);
}

fn paint_search_results_page(
    ui: &mut Ui,
    page: &mut SearchResultsPageBlock,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    let text = Color32::from_rgb(232, 234, 237);
    let muted = Color32::from_rgb(154, 160, 166);
    let link = Color32::from_rgb(138, 180, 248);
    let border = Color32::from_rgb(60, 64, 67);
    let surface = Color32::from_rgb(32, 33, 36);
    let width = ui.available_width().max(320.0);

    Frame::new()
        .fill(Color32::from_rgb(26, 27, 30))
        .inner_margin(Margin::same((18.0 * font_scale) as i8))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(page.brand.as_str())
                        .size(26.0 * font_scale)
                        .family(browser_bold_family())
                        .color(text),
                );
                ui.add_space(18.0 * font_scale);
                let search_width = (ui.available_width() - 90.0 * font_scale)
                    .clamp(240.0 * font_scale, 640.0 * font_scale);
                Frame::new()
                    .fill(Color32::from_rgb(48, 49, 52))
                    .stroke(Stroke::new(1.0, border))
                    .corner_radius(CornerRadius::same((22.0 * font_scale) as u8))
                    .inner_margin(Margin::symmetric(
                        (16.0 * font_scale) as i8,
                        (10.0 * font_scale) as i8,
                    ))
                    .show(ui, |ui| {
                        ui.set_width(search_width);
                        ui.label(
                            RichText::new(page.query.as_str())
                                .size(17.0 * font_scale)
                                .family(browser_regular_family())
                                .color(text),
                        );
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("1 seed")
                            .size(14.0 * font_scale)
                            .family(browser_regular_family())
                            .color(muted),
                    );
                });
            });

            ui.add_space(16.0 * font_scale);
            ui.horizontal_wrapped(|ui| {
                for item in &page.nav_items {
                    let color = if item.eq_ignore_ascii_case("web") {
                        text
                    } else {
                        muted
                    };
                    ui.label(
                        RichText::new(item.as_str())
                            .size(14.0 * font_scale)
                            .family(browser_regular_family())
                            .color(color),
                    );
                    ui.add_space(18.0 * font_scale);
                }
                if !page.region.is_empty() {
                    ui.label(
                        RichText::new(format!("Search region: {}", page.region))
                            .size(14.0 * font_scale)
                            .family(browser_regular_family())
                            .color(muted),
                    );
                }
            });

            ui.add_space(18.0 * font_scale);
            paint_search_mainline(
                ui,
                page,
                text,
                muted,
                link,
                border,
                surface,
                font_scale,
                canvas_response,
            );
            paint_search_side(
                ui,
                page,
                text,
                muted,
                link,
                border,
                surface,
                font_scale,
                canvas_response,
            );
        });
}

fn paint_search_mainline(
    ui: &mut Ui,
    page: &mut SearchResultsPageBlock,
    text: Color32,
    muted: Color32,
    link: Color32,
    border: Color32,
    surface: Color32,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    if !page.videos.is_empty() {
        ui.label(
            RichText::new("Videos")
                .size(20.0 * font_scale)
                .family(browser_bold_family())
                .color(text),
        );
        ui.add_space(8.0 * font_scale);
        for video in &mut page.videos {
            paint_search_media_result(
                ui,
                video,
                muted,
                link,
                border,
                surface,
                font_scale,
                canvas_response,
            );
            ui.add_space(8.0 * font_scale);
        }
        ui.add_space(12.0 * font_scale);
    }

    for result in &mut page.results {
        paint_search_result(ui, result, text, muted, link, font_scale, canvas_response);
        ui.add_space(22.0 * font_scale);
    }
}

fn paint_search_side(
    ui: &mut Ui,
    page: &mut SearchResultsPageBlock,
    text: Color32,
    muted: Color32,
    link: Color32,
    border: Color32,
    surface: Color32,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    if let Some(sidebar) = &page.sidebar {
        Frame::new()
            .fill(surface)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same((16.0 * font_scale) as i8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(sidebar.title.as_str())
                        .size(21.0 * font_scale)
                        .family(browser_bold_family())
                        .color(text),
                );
                ui.add_space(8.0 * font_scale);
                ui.label(
                    RichText::new(sidebar.description.as_str())
                        .size(14.0 * font_scale)
                        .family(browser_regular_family())
                        .color(muted),
                );
                for (label, href) in &sidebar.links {
                    paint_search_link(ui, label, href, link, 14.0, font_scale, canvas_response);
                }
            });
        ui.add_space(18.0 * font_scale);
    }

    if !page.related_queries.is_empty() {
        Frame::new()
            .fill(surface)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same((16.0 * font_scale) as i8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Related searches")
                        .size(18.0 * font_scale)
                        .family(browser_bold_family())
                        .color(text),
                );
                ui.add_space(8.0 * font_scale);
                for query in &page.related_queries {
                    paint_search_link(
                        ui,
                        query,
                        "#offline-link",
                        link,
                        14.0,
                        font_scale,
                        canvas_response,
                    );
                }
            });
        ui.add_space(18.0 * font_scale);
    }

    for card in &mut page.footer_cards {
        Frame::new()
            .fill(surface)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same((14.0 * font_scale) as i8))
            .show(ui, |ui| {
                if let Some(image) = &mut card.image {
                    paint_image_exact(
                        ui,
                        image,
                        &card.title,
                        vec2(ui.available_width(), 90.0 * font_scale),
                    );
                    ui.add_space(8.0 * font_scale);
                }
                ui.label(
                    RichText::new(card.title.as_str())
                        .size(15.0 * font_scale)
                        .family(browser_bold_family())
                        .color(text),
                );
                paint_search_link(
                    ui,
                    &card.link_text,
                    "#offline-link",
                    link,
                    14.0,
                    font_scale,
                    canvas_response,
                );
            });
        ui.add_space(12.0 * font_scale);
    }
}

fn paint_search_media_result(
    ui: &mut Ui,
    result: &mut SearchMediaResult,
    muted: Color32,
    link: Color32,
    border: Color32,
    surface: Color32,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    Frame::new()
        .fill(surface)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same((10.0 * font_scale) as i8))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                if let Some(image) = &mut result.image {
                    paint_image_exact(
                        ui,
                        image,
                        &result.title,
                        vec2(124.0 * font_scale, 70.0 * font_scale),
                    );
                    ui.add_space(10.0 * font_scale);
                }
                ui.vertical(|ui| {
                    paint_search_link(
                        ui,
                        &result.title,
                        &result.href,
                        link,
                        15.0,
                        font_scale,
                        canvas_response,
                    );
                    if !result.source.is_empty() {
                        ui.label(
                            RichText::new(result.source.as_str())
                                .size(13.0 * font_scale)
                                .family(browser_regular_family())
                                .color(muted),
                        );
                    }
                });
            });
        });
}

fn paint_search_result(
    ui: &mut Ui,
    result: &mut SearchResultItem,
    text: Color32,
    muted: Color32,
    link: Color32,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            let source = if result.source_name.is_empty() {
                result.display_url.as_str()
            } else {
                result.source_name.as_str()
            };
            ui.label(
                RichText::new(source)
                    .size(13.0 * font_scale)
                    .family(browser_regular_family())
                    .color(text),
            );
            if !result.display_url.is_empty() && result.display_url != source {
                ui.label(
                    RichText::new(result.display_url.as_str())
                        .size(12.0 * font_scale)
                        .family(browser_regular_family())
                        .color(muted),
                );
            }
            paint_search_link(
                ui,
                &result.title,
                &result.href,
                link,
                19.0,
                font_scale,
                canvas_response,
            );
            if !result.description.is_empty() {
                ui.label(
                    RichText::new(result.description.as_str())
                        .size(14.0 * font_scale)
                        .family(browser_regular_family())
                        .color(muted),
                );
            }
        });
        if let Some(thumbnail) = &mut result.thumbnail {
            ui.add_space(10.0 * font_scale);
            paint_image_exact(
                ui,
                thumbnail,
                &result.title,
                vec2(92.0 * font_scale, 70.0 * font_scale),
            );
        }
    });
}

fn paint_search_link(
    ui: &mut Ui,
    label: &str,
    href: &str,
    color: Color32,
    size: f32,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    let response = ui.link(
        RichText::new(label)
            .size(size * font_scale)
            .family(browser_regular_family())
            .color(color),
    );
    if response.hovered() {
        canvas_response.hovered = Some(HitTarget::Link {
            href: href.to_owned(),
        });
    }
    if response.clicked() {
        canvas_response.clicked = Some(HitTarget::Link {
            href: href.to_owned(),
        });
    }
}

fn paint_image_exact(ui: &mut Ui, image: &mut ImageBlock, image_id: &str, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size.max(Vec2::splat(IMAGE_MIN_SIZE)), Sense::hover());
    let texture = image.texture_handle(ui, image_id);
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn paint_ecosia_hero(
    ui: &mut Ui,
    hero: &mut EcosiaHeroBlock,
    style: &BrowserStyle,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    let width = ui.available_width().max(320.0);
    let height = (width * 0.78).clamp(520.0, 760.0) * font_scale;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    let radius = 32.0 * font_scale;

    painter.rect_filled(rect, radius, Color32::from_rgb(78, 128, 73));
    hero.background.paint_cover(
        ui,
        &hero.background_src,
        rect,
        Color32::from_rgba_premultiplied(255, 255, 255, 245),
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 80)),
        egui::StrokeKind::Inside,
    );

    let pad = 26.0 * font_scale;
    if hero.show_sign_in {
        paint_pill(
            &painter,
            Pos2::new(rect.left() + pad, rect.top() + pad),
            "Sign in",
            92.0 * font_scale,
            42.0 * font_scale,
            Color32::from_rgb(255, 255, 255),
            Color32::from_rgb(22, 72, 48),
            16.0 * font_scale,
        );
    }

    let seed_text = format!("{} seed", hero.seed_count.trim());
    paint_pill(
        &painter,
        Pos2::new(rect.right() - pad - 118.0 * font_scale, rect.top() + pad),
        &seed_text,
        118.0 * font_scale,
        42.0 * font_scale,
        Color32::from_rgb(255, 255, 255),
        Color32::from_rgb(22, 72, 48),
        16.0 * font_scale,
    );
    painter.circle_filled(
        Pos2::new(
            rect.right() - pad - 96.0 * font_scale,
            rect.top() + pad + 21.0 * font_scale,
        ),
        8.0 * font_scale,
        Color32::from_rgb(248, 200, 86),
    );

    painter.text(
        Pos2::new(rect.center().x, rect.top() + height * 0.32),
        Align2::CENTER_CENTER,
        "ECOSIA",
        FontId::new(58.0 * font_scale, browser_bold_family()),
        Color32::WHITE,
    );

    let search_width = (width * 0.66).clamp(420.0, 690.0);
    let search_height = 60.0 * font_scale;
    let search_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + height * 0.44),
        vec2(search_width, search_height),
    );
    painter.rect_filled(search_rect, search_height * 0.5, Color32::WHITE);
    painter.rect_stroke(
        search_rect,
        search_height * 0.5,
        Stroke::new(1.0, Color32::from_rgb(221, 230, 221)),
        egui::StrokeKind::Inside,
    );
    let search_button_center = Pos2::new(
        search_rect.left() + 28.0 * font_scale,
        search_rect.center().y,
    );
    let search_button_rect = Rect::from_center_size(
        search_button_center,
        vec2(44.0 * font_scale, 44.0 * font_scale),
    );
    let search_response = ui
        .interact(
            search_button_rect,
            ui.make_persistent_id("ecosia-search-submit"),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if search_response.hovered() {
        painter.circle_filled(
            search_button_center,
            18.0 * font_scale,
            Color32::from_rgb(232, 241, 235),
        );
        canvas_response.hovered = Some(HitTarget::Button {
            text: "Search".to_owned(),
        });
    }
    paint_search_icon(&painter, search_button_center, font_scale);
    if search_response.clicked() {
        canvas_response.clicked = Some(HitTarget::Button {
            text: "Search".to_owned(),
        });
    }
    let ai_width = 104.0 * font_scale;
    let ai_rect = Rect::from_center_size(
        Pos2::new(
            search_rect.right() - ai_width * 0.5 - 10.0 * font_scale,
            search_rect.center().y,
        ),
        vec2(ai_width, 38.0 * font_scale),
    );

    let input_rect = Rect::from_min_max(
        Pos2::new(
            search_rect.left() + 56.0 * font_scale,
            search_rect.top() + 12.0 * font_scale,
        ),
        Pos2::new(
            ai_rect.left() - 14.0 * font_scale,
            search_rect.bottom() - 12.0 * font_scale,
        ),
    );
    let input_response = ui
        .scope(|ui| {
            let widgets = &mut ui.style_mut().visuals.widgets;
            widgets.inactive.bg_fill = Color32::TRANSPARENT;
            widgets.hovered.bg_fill = Color32::TRANSPARENT;
            widgets.active.bg_fill = Color32::TRANSPARENT;
            widgets.open.bg_fill = Color32::TRANSPARENT;
            widgets.inactive.bg_stroke = Stroke::NONE;
            widgets.hovered.bg_stroke = Stroke::NONE;
            widgets.active.bg_stroke = Stroke::NONE;
            widgets.open.bg_stroke = Stroke::NONE;
            ui.visuals_mut().selection.bg_fill = Color32::from_rgb(210, 234, 218);
            ui.put(
                input_rect,
                egui::TextEdit::singleline(&mut hero.search_value)
                    .font(FontId::new(19.0 * font_scale, browser_regular_family()))
                    .text_color(style.text_color)
                    .hint_text(
                        RichText::new(hero.search_placeholder.trim())
                            .size(19.0 * font_scale)
                            .family(browser_regular_family())
                            .color(Color32::from_rgb(82, 99, 88)),
                    )
                    .desired_width(input_rect.width())
                    .frame(false),
            )
        })
        .inner;
    if input_response.hovered() {
        canvas_response.hovered = Some(HitTarget::Input {
            label: "Search".to_owned(),
        });
    }
    if input_response.changed() {
        canvas_response.changed_inputs.push(InputChange {
            label: "Search".to_owned(),
            value_len: hero.search_value.chars().count(),
        });
    }

    let ai_response = ui.put(
        ai_rect,
        Button::new(
            RichText::new(format!("  {}", hero.ai_button_text.trim()))
                .size(14.0 * font_scale)
                .family(browser_bold_family())
                .color(Color32::from_rgb(30, 91, 67)),
        )
        .fill(Color32::from_rgb(248, 250, 247))
        .stroke(Stroke::new(1.0, Color32::from_rgb(208, 220, 209)))
        .corner_radius(CornerRadius::same((19.0 * font_scale) as u8)),
    );
    painter.circle_filled(
        Pos2::new(ai_rect.left() + 17.0 * font_scale, ai_rect.center().y),
        3.5 * font_scale,
        Color32::from_rgb(35, 116, 84),
    );
    if ai_response.hovered() {
        canvas_response.hovered = Some(HitTarget::Button {
            text: hero.ai_button_text.clone(),
        });
    }
    if ai_response.clicked() {
        canvas_response.clicked = Some(HitTarget::Button {
            text: hero.ai_button_text.clone(),
        });
    }

    let counter_width = (width * 0.58).clamp(430.0, 620.0);
    let counter_height = 104.0 * font_scale;
    let counter_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + height * 0.62),
        vec2(counter_width, counter_height),
    );
    painter.rect_filled(
        counter_rect,
        28.0 * font_scale,
        Color32::from_rgba_premultiplied(255, 255, 255, 218),
    );
    painter.rect_stroke(
        counter_rect,
        28.0 * font_scale,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 190)),
        egui::StrokeKind::Inside,
    );
    let divider_x = counter_rect.center().x;
    painter.line_segment(
        [
            Pos2::new(divider_x, counter_rect.top() + 18.0 * font_scale),
            Pos2::new(divider_x, counter_rect.bottom() - 18.0 * font_scale),
        ],
        Stroke::new(1.0, Color32::from_rgb(204, 216, 201)),
    );
    paint_counter_item(
        &painter,
        Rect::from_min_max(
            counter_rect.min,
            Pos2::new(divider_x, counter_rect.bottom()),
        ),
        &hero.tree_count,
        &hero.tree_description,
        Color32::from_rgb(35, 116, 84),
        font_scale,
    );
    paint_counter_item(
        &painter,
        Rect::from_min_max(Pos2::new(divider_x, counter_rect.top()), counter_rect.max),
        &hero.investment_count,
        &hero.investment_description,
        Color32::from_rgb(35, 116, 84),
        font_scale,
    );
    ui.advance_cursor_after_rect(rect);
}

fn paint_pill(
    painter: &egui::Painter,
    pos: Pos2,
    text: &str,
    width: f32,
    height: f32,
    fill: Color32,
    text_color: Color32,
    font_size: f32,
) {
    let rect = Rect::from_min_size(pos, vec2(width, height));
    painter.rect_filled(rect, height * 0.5, fill);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text.trim(),
        FontId::new(font_size, browser_bold_family()),
        text_color,
    );
}

fn paint_search_icon(painter: &egui::Painter, center: Pos2, font_scale: f32) {
    painter.circle_stroke(
        center + vec2(-2.0 * font_scale, -2.0 * font_scale),
        8.0 * font_scale,
        Stroke::new(2.0 * font_scale, Color32::from_rgb(38, 71, 53)),
    );
    painter.line_segment(
        [
            center + vec2(5.0 * font_scale, 5.0 * font_scale),
            center + vec2(12.0 * font_scale, 12.0 * font_scale),
        ],
        Stroke::new(2.0 * font_scale, Color32::from_rgb(38, 71, 53)),
    );
}

fn paint_counter_item(
    painter: &egui::Painter,
    rect: Rect,
    count: &str,
    description: &str,
    accent: Color32,
    font_scale: f32,
) {
    painter.circle_filled(
        Pos2::new(rect.left() + 42.0 * font_scale, rect.center().y),
        16.0 * font_scale,
        accent,
    );
    painter.text(
        Pos2::new(
            rect.left() + 72.0 * font_scale,
            rect.center().y - 14.0 * font_scale,
        ),
        Align2::LEFT_CENTER,
        count.trim(),
        FontId::new(20.0 * font_scale, browser_bold_family()),
        Color32::from_rgb(18, 56, 38),
    );
    painter.text(
        Pos2::new(
            rect.left() + 72.0 * font_scale,
            rect.center().y + 15.0 * font_scale,
        ),
        Align2::LEFT_CENTER,
        description.trim(),
        FontId::new(12.0 * font_scale, browser_regular_family()),
        Color32::from_rgb(57, 82, 66),
    );
}

fn heading_centered_by_default(level: u8) -> bool {
    level == 1
}

fn paint_inline_spans(
    ui: &mut Ui,
    spans: &[InlineSpan],
    style: &BrowserStyle,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for span in spans {
            paint_inline_span(ui, span, style, font_scale, canvas_response);
        }
    });
}

fn paint_inline_span(
    ui: &mut Ui,
    span: &InlineSpan,
    style: &BrowserStyle,
    font_scale: f32,
    canvas_response: &mut BrowserCanvasResponse,
) {
    if span.text.is_empty() {
        return;
    }

    if span.raised || span.lowered {
        let response = paint_shifted_inline_span(ui, span, style, font_scale);
        if let Some(href) = &span.href {
            if response.hovered() {
                canvas_response.hovered = Some(HitTarget::Link { href: href.clone() });
            }
            if response.clicked() {
                canvas_response.clicked = Some(HitTarget::Link { href: href.clone() });
            }
        }
        return;
    }

    let mut rich = RichText::new(span.text.as_str())
        .size(style.body_font_size * font_scale)
        .family(if span.strong {
            browser_bold_family()
        } else {
            browser_regular_family()
        })
        .color(if span.href.is_some() {
            style.link_color
        } else {
            style.text_color
        });

    if span.strong {
        rich = rich.strong();
    }
    if span.emphasis {
        rich = rich.italics();
    }
    if span.underline || span.href.is_some() {
        rich = rich.underline();
    }
    if span.strikethrough {
        rich = rich.strikethrough();
    }
    if span.code {
        rich = rich.code();
    }
    if span.small {
        rich = rich.small();
    }
    if span.highlight {
        rich = rich.background_color(Color32::from_rgb(255, 245, 157));
    }

    if let Some(href) = &span.href {
        let response = ui.link(rich);
        if response.hovered() {
            canvas_response.hovered = Some(HitTarget::Link { href: href.clone() });
        }
        if response.clicked() {
            canvas_response.clicked = Some(HitTarget::Link { href: href.clone() });
        }
    } else {
        ui.label(rich);
    }
}

fn paint_shifted_inline_span(
    ui: &mut Ui,
    span: &InlineSpan,
    style: &BrowserStyle,
    font_scale: f32,
) -> egui::Response {
    let body_size = style.body_font_size * font_scale;
    let script_size = body_size * 0.68;
    let family = if span.strong {
        browser_bold_family()
    } else {
        browser_regular_family()
    };
    let color = if span.href.is_some() {
        style.link_color
    } else {
        style.text_color
    };
    let font_id = FontId::new(script_size, family);
    let galley = ui
        .painter()
        .layout_no_wrap(span.text.clone(), font_id, color);
    let line_height = body_size * 1.25;
    let desired_size = vec2(galley.size().x, line_height);
    let sense = if span.href.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired_size, sense);
    let vertical_offset = if span.raised {
        body_size * 0.02
    } else {
        body_size * 0.43
    };
    let text_pos = rect.min + vec2(0.0, vertical_offset);

    if span.highlight {
        ui.painter().rect_filled(
            Rect::from_min_size(text_pos, galley.size()),
            0.0,
            Color32::from_rgb(255, 245, 157),
        );
    }
    ui.painter().galley(text_pos, galley.clone(), color);

    if span.underline || span.href.is_some() {
        let y = text_pos.y + galley.size().y - 1.0;
        ui.painter().line_segment(
            [
                Pos2::new(text_pos.x, y),
                Pos2::new(text_pos.x + galley.size().x, y),
            ],
            Stroke::new(1.0, color),
        );
    }
    if span.strikethrough {
        let y = text_pos.y + galley.size().y * 0.55;
        ui.painter().line_segment(
            [
                Pos2::new(text_pos.x, y),
                Pos2::new(text_pos.x + galley.size().x, y),
            ],
            Stroke::new(1.0, color),
        );
    }

    response
}

pub fn page_background_color() -> Color32 {
    BrowserStyle::default().page_background
}

pub fn parse_basic_css(css: &str) -> BrowserStyle {
    parse_basic_css_inner(css, None)
}

pub fn parse_basic_css_for_viewport(css: &str, viewport_width: f32) -> BrowserStyle {
    parse_basic_css_inner(css, Some(viewport_width))
}

fn parse_basic_css_inner(css: &str, viewport_width: Option<f32>) -> BrowserStyle {
    let mut style = BrowserStyle::default();
    let css = strip_css_comments(css);
    let css = if let Some(viewport_width) = viewport_width {
        expand_supported_css_at_rule_blocks(&css, viewport_width)
    } else {
        strip_unsupported_css_at_rule_blocks(&css)
    };
    style.css_variables = collect_css_custom_properties(&css);
    for (order, rule) in css.split('}').enumerate() {
        let Some((selectors, declarations)) = rule.split_once('{') else {
            continue;
        };
        for selector in selectors.split(',').map(str::trim) {
            let variables = style.css_variables.clone();
            apply_css_rule(&mut style, selector, declarations, &variables);
            if let (Some(selector), Some(box_style)) = (
                parse_css_selector(selector),
                parse_css_box_style_with_vars(declarations, &style.css_variables),
            ) {
                style.block_rules.push(CssBlockRule {
                    selector,
                    style: box_style,
                    order,
                });
            }
        }
    }
    style
}

fn collect_css_custom_properties(css: &str) -> HashMap<String, String> {
    let mut variables = HashMap::new();
    for rule in css.split('}') {
        let Some((selectors, declarations)) = rule.split_once('{') else {
            continue;
        };
        if !selector_list_contains_root(selectors) {
            continue;
        }
        for declaration in declarations.split(';') {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let property = property.trim();
            if property.starts_with("--") {
                variables.insert(property.to_owned(), value.trim().to_owned());
            }
        }
    }
    variables
}

fn selector_list_contains_root(selectors: &str) -> bool {
    selectors.split(',').any(|selector| {
        let selector = selector.trim();
        selector == ":root" || selector == "html" || selector == ".dark" || selector == "html.dark"
    })
}

fn resolve_css_vars(value: &str, variables: &HashMap<String, String>) -> String {
    let mut resolved = value.to_owned();
    for _ in 0..8 {
        let Some(start) = resolved.find("var(") else {
            break;
        };
        let Some(end) = find_function_end(&resolved, start + 3) else {
            break;
        };
        let inner = &resolved[start + 4..end];
        let (name, fallback) = split_css_function_args(inner);
        let replacement = variables
            .get(name.trim())
            .cloned()
            .or_else(|| fallback.map(|value| value.trim().to_owned()))
            .unwrap_or_default();
        resolved.replace_range(start..=end, &replacement);
    }
    resolved
}

fn find_function_end(value: &str, open_paren_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in value
        .char_indices()
        .skip_while(|(index, _)| *index < open_paren_index)
    {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_css_function_args(value: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return (&value[..index], Some(&value[index + 1..])),
            _ => {}
        }
    }
    (value, None)
}

fn strip_css_comments(css: &str) -> String {
    let mut stripped = String::with_capacity(css.len());
    let mut remaining = css;
    while let Some(start) = remaining.find("/*") {
        stripped.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("*/") else {
            return stripped;
        };
        remaining = &after_start[end + 2..];
    }
    stripped.push_str(remaining);
    stripped
}

fn strip_unsupported_css_at_rule_blocks(css: &str) -> String {
    let mut stripped = String::with_capacity(css.len());
    let mut index = 0usize;
    while index < css.len() {
        let rest = &css[index..];
        let Some(relative_at) = rest.find('@') else {
            stripped.push_str(rest);
            break;
        };
        let at = index + relative_at;
        stripped.push_str(&css[index..at]);
        let Some(rule_end) = find_css_at_rule_end(css, at) else {
            break;
        };
        index = rule_end;
    }
    stripped
}

fn expand_supported_css_at_rule_blocks(css: &str, viewport_width: f32) -> String {
    let mut expanded = String::with_capacity(css.len());
    let mut index = 0usize;
    while index < css.len() {
        let rest = &css[index..];
        let Some(relative_at) = rest.find('@') else {
            expanded.push_str(rest);
            break;
        };
        let at = index + relative_at;
        expanded.push_str(&css[index..at]);
        let Some(rule_end) = find_css_at_rule_end(css, at) else {
            break;
        };
        if css[at..].starts_with("@media")
            && let Some(open_brace) = css[at..rule_end].find('{').map(|open| at + open)
            && media_query_matches_viewport(&css[at + "@media".len()..open_brace], viewport_width)
        {
            let inner = &css[open_brace + 1..rule_end.saturating_sub(1)];
            expanded.push_str(&expand_supported_css_at_rule_blocks(inner, viewport_width));
        }
        index = rule_end;
    }
    expanded
}

fn media_query_matches_viewport(query: &str, viewport_width: f32) -> bool {
    let query = query.to_ascii_lowercase();
    if query.contains("prefers-") {
        return false;
    }
    let mut matched_any_constraint = false;
    for part in query.split("and") {
        let part = part.trim().trim_matches(|ch| ch == '(' || ch == ')').trim();
        if part.is_empty() || part == "screen" || part == "only screen" {
            continue;
        }
        if let Some(value) = part
            .strip_prefix("min-width:")
            .and_then(parse_css_media_length_px)
        {
            matched_any_constraint = true;
            if viewport_width < value {
                return false;
            }
            continue;
        }
        if let Some(value) = part
            .strip_prefix("max-width:")
            .and_then(parse_css_media_length_px)
        {
            matched_any_constraint = true;
            if viewport_width > value {
                return false;
            }
            continue;
        }
        if let Some(value) = part
            .strip_prefix("width <=")
            .and_then(parse_css_media_length_px)
        {
            matched_any_constraint = true;
            if viewport_width > value {
                return false;
            }
            continue;
        }
        if let Some(value) = part
            .strip_prefix("width >=")
            .and_then(parse_css_media_length_px)
        {
            matched_any_constraint = true;
            if viewport_width < value {
                return false;
            }
            continue;
        }
        return false;
    }
    matched_any_constraint
}

fn parse_css_media_length_px(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("calc(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let mut total = 0.0;
        let mut sign = 1.0;
        for token in inner.split_whitespace() {
            match token {
                "+" => sign = 1.0,
                "-" => sign = -1.0,
                _ => {
                    total += sign * parse_css_media_length_px(token)?;
                    sign = 1.0;
                }
            }
        }
        return Some(total);
    }
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse::<f32>().ok();
    }
    if let Some(rem) = value.strip_suffix("rem") {
        return rem.trim().parse::<f32>().ok().map(|rem| rem * 16.0);
    }
    if let Some(em) = value.strip_suffix("em") {
        return em.trim().parse::<f32>().ok().map(|em| em * 16.0);
    }
    value.parse::<f32>().ok()
}

fn find_css_at_rule_end(css: &str, at: usize) -> Option<usize> {
    let mut cursor = at;
    while cursor < css.len() {
        let ch = css[cursor..].chars().next()?;
        match ch {
            ';' => return Some(cursor + ch.len_utf8()),
            '{' => return find_css_block_end(css, cursor).map(|end| end + 1),
            _ => cursor += ch.len_utf8(),
        }
    }
    Some(css.len())
}

fn find_css_block_end(css: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in css[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace + index);
                }
            }
            _ => {}
        }
    }
    None
}

fn apply_css_rule(
    style: &mut BrowserStyle,
    selector: &str,
    declarations: &str,
    variables: &HashMap<String, String>,
) {
    for declaration in declarations.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        let value = resolve_css_vars(value.trim(), variables);
        let value = value.as_str();
        match (selector, property) {
            ("body", "color") => apply_color(value, &mut style.text_color),
            ("body", "background") | ("body", "background-color") => {
                apply_color(value, &mut style.page_background)
            }
            ("body", "font-size") => apply_px(value, &mut style.body_font_size),
            ("body", "max-width") => apply_px(value, &mut style.main_max_width),
            ("body", "padding") => {
                if let Some((y, x)) = parse_padding_2(value) {
                    style.main_padding_y = y;
                    style.main_padding_x = x;
                }
            }
            ("main", "max-width") => apply_px(value, &mut style.main_max_width),
            ("main", "padding") => {
                if let Some((y, x)) = parse_padding_2(value) {
                    style.main_padding_y = y;
                    style.main_padding_x = x;
                }
            }
            (".panel", "padding") => apply_px(value, &mut style.panel_padding),
            (".panel", "background") | (".panel", "background-color") => {
                apply_color(value, &mut style.panel_background)
            }
            (".panel", "border") => {
                if let Some((width, color)) = parse_border(value) {
                    style.panel_border_width = width;
                    style.panel_border_color = color;
                }
            }
            (".panel", "border-radius") => apply_radius(value, &mut style.panel_radius),
            ("h1", "font-size") => apply_px(value, &mut style.h1_font_size),
            ("h2", "font-size") => apply_px(value, &mut style.h2_font_size),
            ("a", "color") | ("a:visited", "color") | ("button", "color") => {
                apply_color(value, &mut style.link_color)
            }
            ("button", "padding") => {
                if let Some((y, x)) = parse_padding_2(value) {
                    style.button_padding_y = y;
                    style.button_padding_x = x;
                }
            }
            ("button", "background") | ("button", "background-color") => {
                apply_color(value, &mut style.button_background)
            }
            ("button", "border") => {
                if let Some((width, color)) = parse_border(value) {
                    style.button_border_width = width;
                    style.button_border_color = color;
                }
            }
            ("button", "border-radius") => apply_radius(value, &mut style.button_radius),
            ("input", "padding") => apply_px(value, &mut style.input_padding),
            ("input", "border") => {
                if let Some((width, color)) = parse_border(value) {
                    style.input_border_width = width;
                    style.input_border_color = color;
                }
            }
            ("input", "border-radius") => apply_radius(value, &mut style.input_radius),
            ("img" | "*", "max-width") => {
                style.image_width_percent = parse_percent(value);
            }
            ("img" | "*", "height") if value == "auto" => style.image_height_auto = true,
            _ => {}
        }
    }
}

pub fn computed_box_style(style: &BrowserStyle, key: &ElementStyleKey) -> CssBoxStyle {
    let mut out = CssBoxStyle::default();
    let mut matches = style
        .block_rules
        .iter()
        .filter(|rule| css_selector_matches(&rule.selector, key))
        .collect::<Vec<_>>();
    matches.sort_by_key(|rule| (css_selector_specificity(&rule.selector), rule.order));

    for rule in matches {
        merge_css_box_style(&mut out, &rule.style);
    }

    out
}

pub fn parse_inline_box_style(declarations: &str) -> Option<CssBoxStyle> {
    parse_css_box_style_with_vars(declarations, &HashMap::new())
}

fn css_selector_matches(selector: &CssSelector, key: &ElementStyleKey) -> bool {
    let current = SimpleCssSelector {
        tag: selector.tag.clone(),
        id: selector.id.clone(),
        classes: selector.classes.clone(),
        attributes: selector.attributes.clone(),
    };
    if !simple_css_selector_matches(&current, key) {
        return false;
    }
    if let Some(ancestor_selector) = &selector.ancestor {
        if !ancestor_css_selector_matches(ancestor_selector, key.parent.as_deref()) {
            return false;
        }
    }
    if let Some(parent_selector) = &selector.parent {
        let Some(parent) = &key.parent else {
            return false;
        };
        if !simple_css_selector_matches(parent_selector, parent) {
            return false;
        }
    }
    if selector.requires_previous_sibling && key.previous_sibling.is_none() {
        return false;
    }
    if let Some(previous_selector) = &selector.previous_sibling {
        let Some(previous) = &key.previous_sibling else {
            return false;
        };
        if !simple_css_selector_matches(previous_selector, previous) {
            return false;
        }
    }
    true
}

fn simple_css_selector_matches(selector: &SimpleCssSelector, key: &ElementStyleKey) -> bool {
    if let Some(tag) = &selector.tag {
        if !tag.eq_ignore_ascii_case(&key.tag) {
            return false;
        }
    }
    if let Some(id) = &selector.id {
        if key.id.as_deref() != Some(id.as_str()) {
            return false;
        }
    }
    selector
        .classes
        .iter()
        .all(|class| key.classes.iter().any(|key_class| key_class == class))
        && selector.attributes.iter().all(|attribute| {
            key.attributes
                .iter()
                .any(|key_attribute| key_attribute == attribute)
        })
}

fn ancestor_css_selector_matches(
    selector: &SimpleCssSelector,
    parent: Option<&ElementStyleKey>,
) -> bool {
    let mut current = parent;
    while let Some(key) = current {
        if simple_css_selector_matches(selector, key) {
            return true;
        }
        current = key.parent.as_deref();
    }
    false
}

fn css_selector_specificity(selector: &CssSelector) -> usize {
    let current = SimpleCssSelector {
        tag: selector.tag.clone(),
        id: selector.id.clone(),
        classes: selector.classes.clone(),
        attributes: selector.attributes.clone(),
    };
    simple_css_selector_specificity(&current)
        + selector
            .ancestor
            .as_ref()
            .map(simple_css_selector_specificity)
            .unwrap_or_default()
        + selector
            .parent
            .as_ref()
            .map(simple_css_selector_specificity)
            .unwrap_or_default()
        + selector
            .previous_sibling
            .as_ref()
            .map(simple_css_selector_specificity)
            .unwrap_or_default()
}

fn simple_css_selector_specificity(selector: &SimpleCssSelector) -> usize {
    selector.id.iter().count() * 100
        + (selector.classes.len() + selector.attributes.len()) * 10
        + selector.tag.iter().count()
}

fn merge_css_box_style(target: &mut CssBoxStyle, source: &CssBoxStyle) {
    if source.display.is_some() {
        target.display = source.display;
    }
    if source.color.is_some() {
        target.color = source.color;
    }
    if source.background.is_some() {
        target.background = source.background;
    }
    if source.margin.is_some() {
        target.margin = source.margin;
    }
    if source.margin_auto.top.is_some() {
        target.margin_auto.top = source.margin_auto.top;
    }
    if source.margin_auto.right.is_some() {
        target.margin_auto.right = source.margin_auto.right;
    }
    if source.margin_auto.bottom.is_some() {
        target.margin_auto.bottom = source.margin_auto.bottom;
    }
    if source.margin_auto.left.is_some() {
        target.margin_auto.left = source.margin_auto.left;
    }
    if source.margin_top.is_some() {
        target.margin_top = source.margin_top;
    }
    if source.margin_right.is_some() {
        target.margin_right = source.margin_right;
    }
    if source.margin_bottom.is_some() {
        target.margin_bottom = source.margin_bottom;
    }
    if source.margin_left.is_some() {
        target.margin_left = source.margin_left;
    }
    if source.padding.is_some() {
        target.padding = source.padding;
    }
    if source.padding_top.is_some() {
        target.padding_top = source.padding_top;
    }
    if source.padding_right.is_some() {
        target.padding_right = source.padding_right;
    }
    if source.padding_bottom.is_some() {
        target.padding_bottom = source.padding_bottom;
    }
    if source.padding_left.is_some() {
        target.padding_left = source.padding_left;
    }
    if source.border_width.is_some() {
        target.border_width = source.border_width;
    }
    if source.border_color.is_some() {
        target.border_color = source.border_color;
    }
    if source.border_radius.is_some() {
        target.border_radius = source.border_radius;
    }
    if source.width.is_some() {
        target.width = source.width;
    }
    if source.max_width.is_some() {
        target.max_width = source.max_width;
    }
    if source.min_width.is_some() {
        target.min_width = source.min_width;
    }
    if source.height.is_some() {
        target.height = source.height;
    }
    if source.min_height.is_some() {
        target.min_height = source.min_height;
    }
    if source.font_size.is_some() {
        target.font_size = source.font_size;
    }
    if source.font_weight_bold.is_some() {
        target.font_weight_bold = source.font_weight_bold;
    }
    if source.font_style_italic.is_some() {
        target.font_style_italic = source.font_style_italic;
    }
    if source.text_decoration_underline.is_some() {
        target.text_decoration_underline = source.text_decoration_underline;
    }
    if source.text_decoration_strikethrough.is_some() {
        target.text_decoration_strikethrough = source.text_decoration_strikethrough;
    }
    if source.text_background.is_some() {
        target.text_background = source.text_background;
    }
    if source.text_align.is_some() {
        target.text_align = source.text_align;
    }
    if source.flex_grow.is_some() {
        target.flex_grow = source.flex_grow;
    }
    if source.flex_direction.is_some() {
        target.flex_direction = source.flex_direction;
    }
    if source.justify_content.is_some() {
        target.justify_content = source.justify_content;
    }
    if source.align_items.is_some() {
        target.align_items = source.align_items;
    }
    if source.grid_template_columns.is_some() {
        target.grid_template_columns = source.grid_template_columns;
    }
    if source.gap.is_some() {
        target.gap = source.gap;
    }
    if source.overflow_hidden.is_some() {
        target.overflow_hidden = source.overflow_hidden;
    }
    if source.position.is_some() {
        target.position = source.position;
    }
    if source.z_index.is_some() {
        target.z_index = source.z_index;
    }
    if source.inset.is_some() {
        target.inset = source.inset;
    }
    if source.inset_sides.top.is_some() {
        target.inset_sides.top = source.inset_sides.top;
    }
    if source.inset_sides.right.is_some() {
        target.inset_sides.right = source.inset_sides.right;
    }
    if source.inset_sides.bottom.is_some() {
        target.inset_sides.bottom = source.inset_sides.bottom;
    }
    if source.inset_sides.left.is_some() {
        target.inset_sides.left = source.inset_sides.left;
    }
    if source.object_fit.is_some() {
        target.object_fit = source.object_fit;
    }
}

fn parse_css_selector(selector: &str) -> Option<CssSelector> {
    let selector = normalize_css_selector(selector)?;
    if selector.is_empty() || selector == "*" {
        return None;
    }

    if let Some((left, right)) = split_selector_once(&selector, '+') {
        let right = parse_simple_css_selector(right)?;
        let (parent, previous_sibling) = parse_previous_sibling_selector(left)?;
        return Some(CssSelector {
            tag: right.tag,
            id: right.id,
            classes: right.classes,
            attributes: right.attributes,
            ancestor: None,
            parent,
            previous_sibling: Some(previous_sibling),
            requires_previous_sibling: true,
        });
    }

    if let Some((parent, child)) = split_selector_once(&selector, '>') {
        let child = parse_simple_css_selector(child)?;
        let parent = parse_simple_css_selector(parent)?;
        return Some(CssSelector {
            tag: child.tag,
            id: child.id,
            classes: child.classes,
            attributes: child.attributes,
            ancestor: None,
            parent: Some(parent),
            previous_sibling: None,
            requires_previous_sibling: false,
        });
    }

    if selector.chars().any(char::is_whitespace) {
        let (ancestor, descendant) = split_descendant_selector(&selector)?;
        let descendant = parse_simple_css_selector(descendant)?;
        let ancestor = parse_simple_css_selector(ancestor)?;
        return Some(CssSelector {
            tag: descendant.tag,
            id: descendant.id,
            classes: descendant.classes,
            attributes: descendant.attributes,
            ancestor: Some(ancestor),
            parent: None,
            previous_sibling: None,
            requires_previous_sibling: false,
        });
    }

    let simple = parse_simple_css_selector(&selector)?;
    if simple.tag.is_none()
        && simple.id.is_none()
        && simple.classes.is_empty()
        && simple.attributes.is_empty()
    {
        None
    } else {
        Some(CssSelector {
            tag: simple.tag,
            id: simple.id,
            classes: simple.classes,
            attributes: simple.attributes,
            ancestor: None,
            parent: None,
            previous_sibling: None,
            requires_previous_sibling: false,
        })
    }
}

fn split_descendant_selector(selector: &str) -> Option<(&str, &str)> {
    let mut parts = selector.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    let descendant = parts.pop()?;
    let ancestor = parts.pop()?;
    if ancestor.contains(['>', '+']) || descendant.contains(['>', '+']) {
        None
    } else {
        Some((ancestor, descendant))
    }
}

fn parse_previous_sibling_selector(
    selector: &str,
) -> Option<(Option<SimpleCssSelector>, SimpleCssSelector)> {
    if let Some((parent, previous)) = split_selector_once(selector, '>') {
        Some((
            Some(parse_simple_css_selector(parent)?),
            parse_simple_css_selector(previous)?,
        ))
    } else {
        Some((None, parse_simple_css_selector(selector)?))
    }
}

fn parse_simple_css_selector(selector: &str) -> Option<SimpleCssSelector> {
    let selector = selector.trim();
    if selector.is_empty()
        || selector.chars().any(char::is_whitespace)
        || selector.contains('>')
        || selector.contains('+')
    {
        return None;
    }

    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();
    let (selector, attributes) = strip_simple_selector_attributes(selector);
    let mut token = String::new();
    let mut mode = 't';
    for ch in selector.chars().chain(std::iter::once('.')) {
        if ch == '.' || ch == '#' {
            push_selector_part(&mut tag, &mut id, &mut classes, mode, &token);
            token.clear();
            mode = ch;
        } else {
            token.push(ch);
        }
    }

    Some(SimpleCssSelector {
        tag,
        id,
        classes,
        attributes,
    })
}

fn strip_simple_selector_attributes(selector: &str) -> (String, Vec<String>) {
    let mut simple = String::new();
    let mut attributes = Vec::new();
    let mut chars = selector.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '[' {
            simple.push(ch);
            continue;
        }
        let mut raw = String::new();
        for attr_ch in chars.by_ref() {
            if attr_ch == ']' {
                break;
            }
            raw.push(attr_ch);
        }
        let name = raw
            .split(['=', '~', '|', '^', '$', '*', ' '])
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if !name.is_empty() {
            attributes.push(name);
        }
    }
    (simple, attributes)
}

fn normalize_css_selector(selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    if selector.contains('~')
        || selector.contains("::")
        || selector.contains(":before")
        || selector.contains(":after")
        || selector.contains(":not(")
    {
        return None;
    }

    let mut normalized = String::new();
    let mut in_pseudo = false;
    let mut pseudo_depth = 0usize;
    for ch in selector.chars() {
        match ch {
            ':' => in_pseudo = true,
            '(' if in_pseudo => pseudo_depth += 1,
            ')' if in_pseudo && pseudo_depth > 0 => {
                pseudo_depth -= 1;
                if pseudo_depth == 0 {
                    in_pseudo = false;
                }
            }
            ch if in_pseudo && pseudo_depth == 0 && matches!(ch, '.' | '#' | ' ' | '>' | '+') => {
                in_pseudo = false;
                normalized.push(ch);
            }
            _ if !in_pseudo => normalized.push(ch),
            _ => {}
        }
    }
    let normalized = normalized.trim().to_owned();
    (!normalized.is_empty()).then_some(normalized)
}

fn split_selector_once(selector: &str, combinator: char) -> Option<(&str, &str)> {
    let index = selector.find(combinator)?;
    let left = selector[..index].trim();
    let right = selector[index + combinator.len_utf8()..].trim();
    if left.is_empty() || right.is_empty() || right.contains('>') || right.contains('+') {
        None
    } else {
        Some((left, right))
    }
}

fn push_selector_part(
    tag: &mut Option<String>,
    id: &mut Option<String>,
    classes: &mut Vec<String>,
    mode: char,
    token: &str,
) {
    let token = token.trim();
    if token.is_empty() || token == "*" {
        return;
    }
    match mode {
        't' => *tag = Some(token.to_ascii_lowercase()),
        '#' => *id = Some(token.to_owned()),
        '.' => classes.push(token.to_owned()),
        _ => {}
    }
}

fn parse_css_box_style_with_vars(
    declarations: &str,
    variables: &HashMap<String, String>,
) -> Option<CssBoxStyle> {
    let mut style = CssBoxStyle::default();
    let mut seen = false;

    for declaration in declarations.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        let value = resolve_css_vars(value.trim(), variables);
        let value = value.as_str();
        match property {
            "display" => {
                style.display = parse_display(value);
                seen |= style.display.is_some();
            }
            "color" => {
                style.color = parse_color(value);
                seen |= style.color.is_some();
            }
            "background" | "background-color" => {
                style.background = parse_color(value);
                seen |= style.background.is_some();
            }
            "margin" => {
                if let Some((edges, auto)) = parse_margin_edges(value) {
                    style.margin = Some(edges);
                    style.margin_auto = auto;
                    seen = true;
                }
            }
            "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
                if value.eq_ignore_ascii_case("auto") {
                    match property {
                        "margin-top" => {
                            style.margin_top = Some(0.0);
                            style.margin_auto.top = Some(true);
                        }
                        "margin-right" => {
                            style.margin_right = Some(0.0);
                            style.margin_auto.right = Some(true);
                        }
                        "margin-bottom" => {
                            style.margin_bottom = Some(0.0);
                            style.margin_auto.bottom = Some(true);
                        }
                        "margin-left" => {
                            style.margin_left = Some(0.0);
                            style.margin_auto.left = Some(true);
                        }
                        _ => {}
                    }
                    seen = true;
                } else if let Some(px) = parse_px(value) {
                    match property {
                        "margin-top" => {
                            style.margin_top = Some(px);
                            style.margin_auto.top = Some(false);
                        }
                        "margin-right" => {
                            style.margin_right = Some(px);
                            style.margin_auto.right = Some(false);
                        }
                        "margin-bottom" => {
                            style.margin_bottom = Some(px);
                            style.margin_auto.bottom = Some(false);
                        }
                        "margin-left" => {
                            style.margin_left = Some(px);
                            style.margin_auto.left = Some(false);
                        }
                        _ => {}
                    }
                    seen = true;
                }
            }
            "padding" => {
                style.padding = parse_edges(value);
                seen |= style.padding.is_some();
            }
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                if let Some(px) = parse_px(value) {
                    match property {
                        "padding-top" => style.padding_top = Some(px),
                        "padding-right" => style.padding_right = Some(px),
                        "padding-bottom" => style.padding_bottom = Some(px),
                        "padding-left" => style.padding_left = Some(px),
                        _ => {}
                    }
                    seen = true;
                }
            }
            "border" => {
                if let Some((width, color)) = parse_border(value) {
                    style.border_width = Some(width);
                    style.border_color = Some(color);
                    seen = true;
                }
            }
            "border-width" => {
                style.border_width = parse_px(value);
                seen |= style.border_width.is_some();
            }
            "border-color" => {
                style.border_color = parse_color(value);
                seen |= style.border_color.is_some();
            }
            "border-radius" => {
                style.border_radius =
                    parse_px(value).map(|px| px.round().clamp(0.0, u8::MAX as f32) as u8);
                seen |= style.border_radius.is_some();
            }
            "width" => {
                style.width = parse_css_length(value);
                seen |= style.width.is_some();
            }
            "max-width" => {
                style.max_width = parse_css_length(value);
                seen |= style.max_width.is_some();
            }
            "min-width" => {
                style.min_width = parse_css_length(value);
                seen |= style.min_width.is_some();
            }
            "height" => {
                style.height = parse_css_length(value);
                seen |= style.height.is_some();
            }
            "min-height" => {
                style.min_height = parse_css_length(value);
                seen |= style.min_height.is_some();
            }
            "font-size" => {
                style.font_size = parse_px(value);
                seen |= style.font_size.is_some();
            }
            "font-weight" => {
                style.font_weight_bold =
                    Some(value == "bold" || value.parse::<u16>().is_ok_and(|weight| weight >= 600));
                seen = true;
            }
            "font-style" => {
                style.font_style_italic = Some(value == "italic" || value == "oblique");
                seen = true;
            }
            "text-decoration" | "text-decoration-line" => {
                let value = value.to_ascii_lowercase();
                if value.contains("underline") {
                    style.text_decoration_underline = Some(true);
                    seen = true;
                }
                if value.contains("line-through") {
                    style.text_decoration_strikethrough = Some(true);
                    seen = true;
                }
                if value == "none" {
                    style.text_decoration_underline = Some(false);
                    style.text_decoration_strikethrough = Some(false);
                    seen = true;
                }
            }
            "text-align" => {
                style.text_align = match value {
                    "center" => Some(CssTextAlign::Center),
                    "right" | "end" => Some(CssTextAlign::Right),
                    "left" | "start" => Some(CssTextAlign::Left),
                    _ => None,
                };
                seen |= style.text_align.is_some();
            }
            "flex" | "flex-grow" => {
                style.flex_grow = parse_flex_grow(value);
                seen |= style.flex_grow.is_some();
            }
            "flex-direction" => {
                style.flex_direction = match value {
                    "row" | "row-reverse" => Some(CssFlexDirection::Row),
                    "column" | "column-reverse" => Some(CssFlexDirection::Column),
                    _ => None,
                };
                seen |= style.flex_direction.is_some();
            }
            "justify-content" => {
                style.justify_content = match value {
                    "center" => Some(CssJustifyContent::Center),
                    "space-between" => Some(CssJustifyContent::SpaceBetween),
                    "flex-start" | "start" | "left" | "normal" => {
                        Some(CssJustifyContent::FlexStart)
                    }
                    _ => None,
                };
                seen |= style.justify_content.is_some();
            }
            "align-items" => {
                style.align_items = match value {
                    "center" => Some(CssAlignItems::Center),
                    "flex-start" | "start" | "normal" => Some(CssAlignItems::FlexStart),
                    "stretch" => Some(CssAlignItems::Stretch),
                    _ => None,
                };
                seen |= style.align_items.is_some();
            }
            "grid-template-columns" => {
                style.grid_template_columns = parse_grid_template_columns(value);
                seen |= style.grid_template_columns.is_some();
            }
            "gap" | "row-gap" | "column-gap" => {
                style.gap = parse_px(value);
                seen |= style.gap.is_some();
            }
            "overflow" | "overflow-x" | "overflow-y" => {
                if value == "hidden" {
                    style.overflow_hidden = Some(true);
                    seen = true;
                }
            }
            "position" => {
                style.position = match value {
                    "relative" => Some(CssPosition::Relative),
                    "absolute" => Some(CssPosition::Absolute),
                    "fixed" => Some(CssPosition::Fixed),
                    "sticky" => Some(CssPosition::Sticky),
                    "static" => Some(CssPosition::Static),
                    _ => None,
                };
                seen |= style.position.is_some();
            }
            "z-index" => {
                if value != "auto" {
                    style.z_index = value.parse::<i32>().ok();
                    seen |= style.z_index.is_some();
                }
            }
            "inset" => {
                style.inset = parse_edges(value);
                if let Some(edges) = style.inset {
                    style.inset_sides = CssInset {
                        top: Some(edges.top),
                        right: Some(edges.right),
                        bottom: Some(edges.bottom),
                        left: Some(edges.left),
                    };
                }
                seen |= style.inset.is_some();
            }
            "top" | "right" | "bottom" | "left" => {
                let mut edges = style.inset.unwrap_or_default();
                if let Some(px) = parse_px(value) {
                    set_edge(&mut edges, property, px);
                    set_inset_side(&mut style.inset_sides, property, px);
                    style.inset = Some(edges);
                    seen = true;
                }
            }
            "object-fit" => {
                style.object_fit = match value {
                    "cover" => Some(CssObjectFit::Cover),
                    "contain" => Some(CssObjectFit::Contain),
                    "fill" => Some(CssObjectFit::Fill),
                    _ => None,
                };
                seen |= style.object_fit.is_some();
            }
            _ => {}
        }
    }

    seen.then_some(style)
}

fn parse_display(value: &str) -> Option<CssDisplay> {
    match value.split_whitespace().next().unwrap_or(value) {
        "none" => Some(CssDisplay::None),
        "block" => Some(CssDisplay::Block),
        "inline" => Some(CssDisplay::Inline),
        "inline-block" => Some(CssDisplay::InlineBlock),
        "contents" => Some(CssDisplay::Inline),
        "inline-flex" => Some(CssDisplay::InlineBlock),
        "flex" => Some(CssDisplay::Flex),
        "grid" | "inline-grid" => Some(CssDisplay::Grid),
        "table" => Some(CssDisplay::Table),
        "list-item" => Some(CssDisplay::ListItem),
        _ => None,
    }
}

fn parse_flex_grow(value: &str) -> Option<f32> {
    let first = value.split_whitespace().next()?.trim();
    match first {
        "none" => Some(0.0),
        "auto" | "initial" => Some(0.0),
        _ => first.parse::<f32>().ok().map(|value| value.max(0.0)),
    }
}

fn parse_grid_template_columns(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || value == "none" {
        return None;
    }
    if let Some(repeat_start) = value.find("repeat(") {
        let inner = &value[repeat_start + "repeat(".len()..];
        let count = inner.split(',').next()?.trim().parse::<usize>().ok()?;
        return (count > 0).then_some(count);
    }
    let columns = split_css_value_list(value)
        .into_iter()
        .filter(|token| {
            let token = token.trim();
            !token.is_empty() && token != "/" && !token.eq_ignore_ascii_case("subgrid")
        })
        .count();
    (columns > 0).then_some(columns)
}

fn parse_color(value: &str) -> Option<Color32> {
    parse_hex_color(value)
        .or_else(|| parse_rgb_color(value))
        .or_else(|| parse_hsl_color(value))
        .or_else(|| match value.trim().to_ascii_lowercase().as_str() {
            "transparent" => Some(Color32::TRANSPARENT),
            "white" => Some(Color32::WHITE),
            "black" => Some(Color32::BLACK),
            "red" => Some(Color32::RED),
            "green" => Some(Color32::GREEN),
            "blue" => Some(Color32::BLUE),
            _ => None,
        })
}

fn parse_edges(value: &str) -> Option<CssEdges> {
    let values = split_css_value_list(value)
        .iter()
        .filter_map(|value| parse_px(value))
        .collect::<Vec<_>>();
    match values.as_slice() {
        [all] => Some(CssEdges {
            top: *all,
            right: *all,
            bottom: *all,
            left: *all,
        }),
        [vertical, horizontal] => Some(CssEdges {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        }),
        [top, horizontal, bottom] => Some(CssEdges {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        }),
        [top, right, bottom, left, ..] => Some(CssEdges {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        }),
        _ => None,
    }
}

fn parse_margin_edges(value: &str) -> Option<(CssEdges, CssEdgeAutoSpec)> {
    let values = split_css_value_list(value);
    let expanded = match values.as_slice() {
        [all] => [all.as_str(), all.as_str(), all.as_str(), all.as_str()],
        [vertical, horizontal] => [
            vertical.as_str(),
            horizontal.as_str(),
            vertical.as_str(),
            horizontal.as_str(),
        ],
        [top, horizontal, bottom] => [
            top.as_str(),
            horizontal.as_str(),
            bottom.as_str(),
            horizontal.as_str(),
        ],
        [top, right, bottom, left, ..] => {
            [top.as_str(), right.as_str(), bottom.as_str(), left.as_str()]
        }
        _ => return None,
    };

    let mut edges = CssEdges::default();
    let mut auto = CssEdgeAutoSpec::default();
    for (index, token) in expanded.iter().enumerate() {
        let is_auto = token.eq_ignore_ascii_case("auto");
        let px = if is_auto { Some(0.0) } else { parse_px(token) }?;
        match index {
            0 => {
                edges.top = px;
                auto.top = Some(is_auto);
            }
            1 => {
                edges.right = px;
                auto.right = Some(is_auto);
            }
            2 => {
                edges.bottom = px;
                auto.bottom = Some(is_auto);
            }
            3 => {
                edges.left = px;
                auto.left = Some(is_auto);
            }
            _ => {}
        }
    }
    Some((edges, auto))
}

fn set_edge(edges: &mut CssEdges, property: &str, px: f32) {
    match property
        .rsplit_once('-')
        .map(|(_, edge)| edge)
        .unwrap_or(property)
    {
        "top" => edges.top = px,
        "right" => edges.right = px,
        "bottom" => edges.bottom = px,
        "left" => edges.left = px,
        _ => {}
    }
}

fn set_inset_side(inset: &mut CssInset, property: &str, px: f32) {
    match property {
        "top" => inset.top = Some(px),
        "right" => inset.right = Some(px),
        "bottom" => inset.bottom = Some(px),
        "left" => inset.left = Some(px),
        _ => {}
    }
}

fn parse_css_length(value: &str) -> Option<CssLength> {
    if value.trim().eq_ignore_ascii_case("auto") {
        Some(CssLength::Auto)
    } else {
        parse_percent(value)
            .map(CssLength::Percent)
            .or_else(|| parse_px(value).map(CssLength::Px))
    }
}

fn button_width_for_text(text: &str, style: &BrowserStyle, font_scale: f32) -> f32 {
    text.chars().count() as f32 * 6.45 * font_scale + style.button_padding_x * 2.0 * font_scale
}

fn apply_color(value: &str, target: &mut Color32) {
    if let Some(color) = parse_hex_color(value) {
        *target = color;
    } else if let Some(color) = parse_hsl_color(value) {
        *target = color;
    } else if value.contains("var(--body-color)") {
        *target = Color32::from_rgb(27, 24, 24);
    } else if value.contains("var(--body-bg-color)") {
        *target = Color32::from_rgb(249, 250, 251);
    } else if value.contains("var(--link-visited)") {
        *target = Color32::from_rgb(168, 0, 0);
    }
}

fn apply_px(value: &str, target: &mut f32) {
    if let Some(px) = parse_px(value) {
        *target = px;
    }
}

fn apply_radius(value: &str, target: &mut u8) {
    if let Some(px) = parse_px(value) {
        *target = px.round().clamp(0.0, u8::MAX as f32) as u8;
    }
}

fn parse_padding_2(value: &str) -> Option<(f32, f32)> {
    let values = split_css_value_list(value)
        .iter()
        .filter_map(|value| parse_px(value))
        .collect::<Vec<_>>();
    match values.as_slice() {
        [all] => Some((*all, *all)),
        [y, x, ..] => Some((*y, *x)),
        _ => None,
    }
}

fn parse_border(value: &str) -> Option<(f32, Color32)> {
    let values = split_css_value_list(value);
    let width = values.iter().find_map(|value| parse_px(value))?;
    let color = values.iter().find_map(|value| parse_color(value))?;
    Some((width, color))
}

fn split_css_value_list(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in value.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ch if ch.is_whitespace() && depth == 0 => {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_owned());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_owned());
    }
    out
}

fn parse_px(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("calc(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_calc_length(inner);
    }
    if let Some(rem) = value.strip_suffix("rem") {
        return rem.trim().parse::<f32>().ok().map(|rem| rem * 16.0);
    }
    if let Some(em) = value.strip_suffix("em") {
        return em.trim().parse::<f32>().ok().map(|em| em * 16.0);
    }
    if let Some(ch) = value.strip_suffix("ch") {
        return ch.trim().parse::<f32>().ok().map(|ch| ch * 8.0);
    }
    if let Some(vh) = value.strip_suffix("vh") {
        return vh.trim().parse::<f32>().ok().map(|vh| vh * 9.0);
    }
    if let Some(vw) = value.strip_suffix("vw") {
        return vw.trim().parse::<f32>().ok().map(|vw| vw * 12.8);
    }
    value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("pt"))
        .unwrap_or(value)
        .parse::<f32>()
        .ok()
}

fn parse_calc_length(value: &str) -> Option<f32> {
    let mut total = 0.0;
    let mut current = String::new();
    let mut sign = 1.0;
    let mut saw_term = false;

    for ch in value.chars().chain(std::iter::once('+')) {
        if ch == '+' || ch == '-' {
            if !current.trim().is_empty() {
                total += sign * parse_calc_length_term(current.trim())?;
                current.clear();
                saw_term = true;
            }
            sign = if ch == '-' { -1.0 } else { 1.0 };
        } else {
            current.push(ch);
        }
    }

    saw_term.then_some(total)
}

fn parse_calc_length_term(value: &str) -> Option<f32> {
    let value = value.trim();
    if value.contains('*') {
        let mut product = 1.0;
        for part in value.split('*') {
            product *= parse_calc_length_factor(part.trim())?;
        }
        return Some(product);
    }
    if let Some((left, right)) = value.split_once('/') {
        let denominator = parse_calc_length_factor(right.trim())?;
        if denominator == 0.0 {
            return None;
        }
        return Some(parse_calc_length_factor(left.trim())? / denominator);
    }
    parse_calc_length_factor(value)
}

fn parse_calc_length_factor(value: &str) -> Option<f32> {
    parse_px(value).or_else(|| value.parse::<f32>().ok())
}

fn parse_percent(value: &str) -> Option<f32> {
    value
        .trim()
        .strip_suffix('%')?
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|percent| *percent > 0.0)
}

fn parse_hex_color(value: &str) -> Option<Color32> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() == 3 {
        let mut expanded = String::with_capacity(6);
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
        let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
        let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
        return Some(Color32::from_rgb(r, g, b));
    }
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn parse_rgb_color(value: &str) -> Option<Color32> {
    let value = value.trim().to_ascii_lowercase();
    let inner = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts = inner.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    let r = parse_color_channel(parts[0])?;
    let g = parse_color_channel(parts[1])?;
    let b = parse_color_channel(parts[2])?;
    let a = parts
        .get(3)
        .and_then(|alpha| alpha.parse::<f32>().ok())
        .map(|alpha| (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
        .unwrap_or(255);
    Some(Color32::from_rgba_premultiplied(r, g, b, a))
}

fn parse_color_channel(value: &str) -> Option<u8> {
    if let Some(percent) = parse_percent(value) {
        Some((percent.clamp(0.0, 100.0) * 2.55).round() as u8)
    } else {
        value
            .parse::<f32>()
            .ok()
            .map(|channel| channel.clamp(0.0, 255.0).round() as u8)
    }
}

fn parse_hsl_color(value: &str) -> Option<Color32> {
    let inner = value
        .trim()
        .strip_prefix("hsl(")?
        .trim_end_matches(')')
        .replace(',', " ");
    let parts = inner
        .split_whitespace()
        .map(|part| part.trim_end_matches('%'))
        .collect::<Vec<_>>();
    let [h, s, l, ..] = parts.as_slice() else {
        return None;
    };
    let h = h.parse::<f32>().ok()? / 360.0;
    let s = s.parse::<f32>().ok()? / 100.0;
    let l = l.parse::<f32>().ok()? / 100.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    Some(Color32::from_rgb(
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    ))
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> u8 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    let value = if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    };
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

fn browser_regular_family() -> FontFamily {
    FontFamily::Name(BROWSER_REGULAR_FONT_NAME.into())
}

fn browser_bold_family() -> FontFamily {
    FontFamily::Name(BROWSER_BOLD_FONT_NAME.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_document_keeps_blocks_in_order() {
        let document = BrowserDocument {
            title: "Sample".to_owned(),
            source: "fixture".to_owned(),
            style: BrowserStyle::default(),
            canvas_graph: CanvasGraph::default(),
            blocks: vec![
                CanvasBlock::Heading {
                    level: 1,
                    text: "Title".to_owned(),
                },
                CanvasBlock::Paragraph {
                    text: "Body".to_owned(),
                },
            ],
        };

        assert_eq!(document.blocks.len(), 2);
        assert!(matches!(document.blocks[0], CanvasBlock::Heading { .. }));
        assert!(matches!(document.blocks[1], CanvasBlock::Paragraph { .. }));
    }

    #[test]
    fn h1_is_centered_by_default() {
        assert!(heading_centered_by_default(1));
        assert!(!heading_centered_by_default(2));
        assert!(!heading_centered_by_default(3));
    }

    #[test]
    fn browser_textbox_measurement_uses_configured_fonts_and_wrap_width() {
        let ctx = egui::Context::default();
        configure_browser_fonts(&ctx);
        let style = ResolvedBoxStyle {
            font_size: 18.0,
            font_weight_bold: true,
            ..ResolvedBoxStyle::default()
        };

        let _ = ctx.run(Default::default(), |_| {
            let short = measure_browser_textbox(&ctx, "Hello", &style);
            let long = measure_browser_textbox(&ctx, "Hello browser", &style);
            assert!(short.x > 0.0);
            assert!(long.x > short.x);

            let lut = calculate_browser_font_size_lut(&ctx, &style);
            assert_eq!(lut.font_size, style.font_size);
            assert!(lut.glyph_size('H').is_some_and(|size| size.x > 0.0));
            assert!(lut.glyph_size(' ').is_some_and(|size| size.x > 0.0));

            let lines = wrap_browser_textboxes(
                Some(&ctx),
                "Hello browser rendering flow",
                short.x + 1.0,
                &style,
            );
            assert!(lines.len() > 1);
            assert!(lines.iter().all(|line| line.size.x <= short.x + 1.0));
        });
    }

    #[test]
    fn input_value_mut_persists_textbox_edits() {
        let mut document = BrowserDocument {
            title: "Sample".to_owned(),
            source: "fixture".to_owned(),
            style: BrowserStyle::default(),
            canvas_graph: CanvasGraph::default(),
            blocks: vec![CanvasBlock::Input {
                label: "Name".to_owned(),
                value: "AlmostThere".to_owned(),
            }],
        };

        let value = document.input_value_mut("Name").unwrap();
        value.clear();
        value.push_str("Edited");

        assert!(matches!(
            &document.blocks[0],
            CanvasBlock::Input { value, .. } if value == "Edited"
        ));
    }

    #[test]
    fn input_value_mut_finds_nested_panel_inputs() {
        let mut document = BrowserDocument {
            title: "Sample".to_owned(),
            source: "fixture".to_owned(),
            style: BrowserStyle::default(),
            canvas_graph: CanvasGraph::default(),
            blocks: vec![CanvasBlock::Panel {
                children: vec![CanvasBlock::Input {
                    label: "Name".to_owned(),
                    value: "AlmostThere".to_owned(),
                }],
            }],
        };

        document
            .input_value_mut("Name")
            .unwrap()
            .push_str(" Browser");

        assert!(matches!(
            &document.blocks[0],
            CanvasBlock::Panel { children } if matches!(
                &children[0],
                CanvasBlock::Input { value, .. } if value == "AlmostThere Browser"
            )
        ));
    }

    #[test]
    fn input_value_mut_finds_ecosia_search_input() {
        let mut document = BrowserDocument {
            title: "Sample".to_owned(),
            source: "fixture".to_owned(),
            style: BrowserStyle::default(),
            canvas_graph: CanvasGraph::default(),
            blocks: vec![CanvasBlock::EcosiaHero {
                hero: EcosiaHeroBlock {
                    background_src: "fixture.png".to_owned(),
                    background: ImageBlock {
                        path: PathBuf::from("fixture.png"),
                        size: vec2(1.0, 1.0),
                        color_image: ColorImage::new([1, 1], vec![Color32::WHITE]),
                        texture: None,
                    },
                    search_placeholder: "Search the web...".to_owned(),
                    search_value: String::new(),
                    ai_button_text: "AI Chat".to_owned(),
                    tree_count: String::new(),
                    tree_description: String::new(),
                    investment_count: String::new(),
                    investment_description: String::new(),
                    seed_count: "1".to_owned(),
                    show_sign_in: true,
                },
            }],
        };

        document
            .input_value_mut("Search")
            .unwrap()
            .push_str("trees");

        assert!(matches!(
            &document.blocks[0],
            CanvasBlock::EcosiaHero { hero } if hero.search_value == "trees"
        ));
    }

    #[test]
    fn button_width_includes_css_like_horizontal_padding() {
        let width = button_width_for_text("Test Button", &BrowserStyle::default(), 1.0);
        assert!((94.0..=96.0).contains(&width));
    }

    #[test]
    fn parse_basic_css_extracts_fixture_button_padding_and_colors() {
        let style = parse_basic_css(
            r#"
            body { color: #1f2933; background: #f5f7fa; }
            main { max-width: 760px; padding: 32px 20px; }
            .panel { padding: 16px; border: 1px solid #ccd6e0; border-radius: 8px; background: #ffffff; }
            h1 { font-size: 32px; }
            h2 { font-size: 22px; }
            a, button { color: #075985; }
            button { padding: 8px 12px; border: 1px solid #075985; border-radius: 4px; background: #e0f2fe; }
            input { padding: 8px; border: 1px solid #9aa6b2; border-radius: 4px; }
            "#,
        );

        assert_eq!(style.page_background, Color32::from_rgb(245, 247, 250));
        assert_eq!(style.text_color, Color32::from_rgb(31, 41, 51));
        assert_eq!(style.link_color, Color32::from_rgb(7, 89, 133));
        assert_eq!(style.main_max_width, 760.0);
        assert_eq!(style.main_padding_y, 32.0);
        assert_eq!(style.main_padding_x, 20.0);
        assert_eq!(style.panel_padding, 16.0);
        assert_eq!(style.panel_border_color, Color32::from_rgb(204, 214, 224));
        assert_eq!(style.button_padding_y, 8.0);
        assert_eq!(style.button_padding_x, 12.0);
        assert_eq!(style.button_background, Color32::from_rgb(224, 242, 254));
        assert_eq!(style.input_border_color, Color32::from_rgb(154, 166, 178));
    }

    #[test]
    fn parse_basic_css_detects_auto_image_height() {
        let style = parse_basic_css("img { max-width: 100%; height: auto; display: block; }");

        assert!(style.image_height_auto);
        assert_eq!(style.image_width_percent, Some(100.0));
    }

    #[test]
    fn parse_basic_css_applies_universal_image_sizing_rule() {
        let style = parse_basic_css("* { max-width: 75%; height: auto; }");

        assert!(style.image_height_auto);
        assert_eq!(style.image_width_percent, Some(75.0));
    }

    #[test]
    fn parse_basic_css_ignores_comments_before_selectors() {
        let style = parse_basic_css(
            r#"
            /* Make images easier to work with */
            img {
                max-width: 100%;
                height: auto;
            }
            "#,
        );

        assert!(style.image_height_auto);
        assert_eq!(style.image_width_percent, Some(100.0));
    }

    #[test]
    fn parse_basic_css_ignores_unsupported_media_query_blocks() {
        let style = parse_basic_css(
            r#"
            :root {
                --fg: #111111;
            }
            .latex-dark {
                --fg: #eeeeee;
            }
            body {
                color: var(--fg);
                background: #ffffff;
            }
            @media (prefers-color-scheme: dark) {
                :root {
                    --fg: #eeeeee;
                }
                body {
                    color: var(--fg);
                    background: #222222;
                }
            }
            a {
                color: #cc0000;
            }
            "#,
        );

        assert_eq!(style.text_color, Color32::from_rgb(0x11, 0x11, 0x11));
        assert_eq!(style.page_background, Color32::WHITE);
        assert_eq!(style.link_color, Color32::from_rgb(0xcc, 0x00, 0x00));
    }

    #[test]
    fn complex_selectors_do_not_collapse_to_their_last_simple_selector() {
        let style = parse_basic_css(
            r#"
            nav ol > li { display: block; }
            nav li { display: flex; }
            li.selected { display: inline-block; }
            article > * + * { margin-top: 1em; }
            "#,
        );
        let plain_li = ElementStyleKey {
            tag: "li".to_owned(),
            id: None,
            classes: Vec::new(),
            ..ElementStyleKey::default()
        };
        let selected_li = ElementStyleKey {
            tag: "li".to_owned(),
            id: None,
            classes: vec!["selected".to_owned()],
            ..ElementStyleKey::default()
        };

        assert_eq!(computed_box_style(&style, &plain_li).display, None);
        assert_eq!(
            computed_box_style(&style, &selected_li).display,
            Some(CssDisplay::InlineBlock)
        );

        let article_child_without_previous = ElementStyleKey {
            tag: "header".to_owned(),
            parent: Some(Box::new(ElementStyleKey {
                tag: "article".to_owned(),
                ..ElementStyleKey::default()
            })),
            ..ElementStyleKey::default()
        };
        let article_child_with_previous = ElementStyleKey {
            tag: "footer".to_owned(),
            parent: Some(Box::new(ElementStyleKey {
                tag: "article".to_owned(),
                ..ElementStyleKey::default()
            })),
            previous_sibling: Some(Box::new(ElementStyleKey {
                tag: "div".to_owned(),
                ..ElementStyleKey::default()
            })),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &article_child_without_previous).margin,
            None
        );
        assert_eq!(
            computed_box_style(&style, &article_child_with_previous).margin_top,
            Some(16.0)
        );
    }

    #[test]
    fn parse_basic_css_carries_flex_layout_properties() {
        let style = parse_basic_css(
            r#"
            .toolbar {
                display: flex;
                flex-direction: row;
                justify-content: space-between;
                align-items: center;
                gap: 12px;
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "div".to_owned(),
            id: None,
            classes: vec!["toolbar".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.display, Some(CssDisplay::Flex));
        assert_eq!(computed.flex_direction, Some(CssFlexDirection::Row));
        assert_eq!(
            computed.justify_content,
            Some(CssJustifyContent::SpaceBetween)
        );
        assert_eq!(computed.align_items, Some(CssAlignItems::Center));
        assert_eq!(computed.gap, Some(12.0));
    }

    #[test]
    fn parse_basic_css_carries_position_z_index() {
        let style = parse_basic_css(
            r#"
            .header {
                position: sticky;
                z-index: 3;
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["header".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.position, Some(CssPosition::Sticky));
        assert_eq!(computed.z_index, Some(3));
    }

    #[test]
    fn parse_basic_css_preserves_auto_margins() {
        let style = parse_basic_css(
            r#"
            .spacer { margin-left: auto; }
            .centered { margin: 0 auto; }
            "#,
        );
        let spacer = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["spacer".to_owned()],
            ..ElementStyleKey::default()
        };
        let centered = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["centered".to_owned()],
            ..ElementStyleKey::default()
        };

        let spacer_style = computed_box_style(&style, &spacer);
        assert_eq!(spacer_style.margin_left, Some(0.0));
        assert_eq!(spacer_style.margin_auto.left, Some(true));

        let centered_style = computed_box_style(&style, &centered);
        assert_eq!(centered_style.margin, Some(CssEdges::default()));
        assert_eq!(centered_style.margin_auto.left, Some(true));
        assert_eq!(centered_style.margin_auto.right, Some(true));
    }

    #[test]
    fn parse_basic_css_ignores_not_selectors_instead_of_broadening_them() {
        let style = parse_basic_css(
            r#"
            .button__icon:not(.button__icon--with-text) { display: none; }
            .button__icon--with-text { display: inline-block; }
            "#,
        );
        let icon = ElementStyleKey {
            tag: "svg".to_owned(),
            classes: vec!["button__icon".to_owned()],
            ..ElementStyleKey::default()
        };
        let icon_with_text = ElementStyleKey {
            tag: "svg".to_owned(),
            classes: vec![
                "button__icon".to_owned(),
                "button__icon--with-text".to_owned(),
            ],
            ..ElementStyleKey::default()
        };

        assert_eq!(computed_box_style(&style, &icon).display, None);
        assert_eq!(
            computed_box_style(&style, &icon_with_text).display,
            Some(CssDisplay::InlineBlock)
        );
    }

    #[test]
    fn parse_basic_css_ignores_pseudo_elements_instead_of_broadening_them() {
        let style = parse_basic_css(
            r#"
            .button:before { width: 100%; }
            .button::after { min-width: 48px; }
            "#,
        );
        let button = ElementStyleKey {
            tag: "a".to_owned(),
            classes: vec!["button".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &button);

        assert_eq!(computed.width, None);
        assert_eq!(computed.min_width, None);
    }

    #[test]
    fn parse_basic_css_carries_grid_template_column_count() {
        let style = parse_basic_css(
            ".counter { display: grid; grid-template-columns: 1fr auto 1fr; } .cards { grid-template-columns: repeat(4, minmax(0, 1fr)); }",
        );
        let counter = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["counter".to_owned()],
            ..ElementStyleKey::default()
        };
        let cards = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["cards".to_owned()],
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &counter).grid_template_columns,
            Some(3)
        );
        assert_eq!(
            computed_box_style(&style, &cards).grid_template_columns,
            Some(4)
        );
    }

    #[test]
    fn scoped_attribute_selectors_match_by_simple_compound_selector() {
        let style = parse_basic_css(
            r#"
            .main-header__navigation[data-v-c74220bd] { display: flex; gap: .75rem; }
            a.button[data-v-752dcc4e] { padding: calc(.5rem + 4px) 1rem; }
            .dark .button[data-v-752dcc4e] { display: flex; }
            "#,
        );
        let nav = ElementStyleKey {
            tag: "div".to_owned(),
            id: None,
            classes: vec!["main-header__navigation".to_owned()],
            attributes: vec!["data-v-c74220bd".to_owned()],
            ..ElementStyleKey::default()
        };
        let button = ElementStyleKey {
            tag: "a".to_owned(),
            id: None,
            classes: vec!["button".to_owned()],
            attributes: vec!["data-v-752dcc4e".to_owned()],
            parent: Some(Box::new(ElementStyleKey {
                tag: "html".to_owned(),
                classes: vec!["dark".to_owned()],
                ..ElementStyleKey::default()
            })),
            ..ElementStyleKey::default()
        };

        let nav_style = computed_box_style(&style, &nav);
        let button_style = computed_box_style(&style, &button);

        assert_eq!(nav_style.display, Some(CssDisplay::Flex));
        assert_eq!(nav_style.gap, Some(12.0));
        assert_eq!(button_style.display, Some(CssDisplay::Flex));
        assert_eq!(
            button_style.padding,
            Some(CssEdges {
                top: 12.0,
                right: 16.0,
                bottom: 12.0,
                left: 16.0,
            })
        );
    }

    #[test]
    fn descendant_selectors_match_ancestor_context() {
        let style = parse_basic_css(
            r#"
            .hero-search button { color: #ffffff; font-weight: 700; }
            "#,
        );
        let button = ElementStyleKey {
            tag: "button".to_owned(),
            parent: Some(Box::new(ElementStyleKey {
                tag: "div".to_owned(),
                classes: vec!["hero-search".to_owned()],
                ..ElementStyleKey::default()
            })),
            ..ElementStyleKey::default()
        };
        let outside_button = ElementStyleKey {
            tag: "button".to_owned(),
            parent: Some(Box::new(ElementStyleKey {
                tag: "div".to_owned(),
                classes: vec!["other".to_owned()],
                ..ElementStyleKey::default()
            })),
            ..ElementStyleKey::default()
        };

        let button_style = computed_box_style(&style, &button);
        let outside_button_style = computed_box_style(&style, &outside_button);

        assert_eq!(button_style.color, Some(Color32::WHITE));
        assert_eq!(button_style.font_weight_bold, Some(true));
        assert_eq!(outside_button_style.color, None);
    }

    #[test]
    fn css_custom_properties_resolve_in_box_styles() {
        let style = parse_basic_css(
            r#"
            :root {
                --space-m: 1.5rem;
                --brand: #123456;
            }
            .card {
                padding: var(--space-m);
                color: var(--brand);
                width: calc(10rem + 20px);
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "section".to_owned(),
            id: None,
            classes: vec!["card".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(
            computed.padding,
            Some(CssEdges {
                top: 24.0,
                right: 24.0,
                bottom: 24.0,
                left: 24.0,
            })
        );
        assert_eq!(computed.color, Some(Color32::from_rgb(18, 52, 86)));
        assert_eq!(computed.width, Some(CssLength::Px(180.0)));
    }

    #[test]
    fn side_specific_box_edges_survive_later_rules_in_the_cascade() {
        let style = parse_basic_css(
            r#"
            h2 { margin-top: 3rem; padding-left: 2rem; }
            h2, h3 { margin-bottom: 0.8rem; padding-right: 1rem; }
            "#,
        );
        let key = ElementStyleKey {
            tag: "h2".to_owned(),
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.margin_top, Some(48.0));
        assert_eq!(computed.margin_bottom, Some(12.8));
        assert_eq!(computed.padding_left, Some(32.0));
        assert_eq!(computed.padding_right, Some(16.0));
    }

    #[test]
    fn image_width_percentage_uses_containing_block_width() {
        let style = parse_basic_css("img { max-width: 100%; height: auto; }");

        assert_eq!(
            image_display_size(vec2(750.0, 450.0), 640.0, &style),
            vec2(640.0, 384.0)
        );
        assert_eq!(
            image_display_size(vec2(600.0, 400.0), 640.0, &style),
            vec2(640.0, 426.6667)
        );
    }

    #[test]
    fn image_block_decodes_avif_bytes() {
        let bytes = include_bytes!("../../sample_pages/test_2x2.avif");
        let block = ImageBlock::from_encoded_bytes(PathBuf::from("test_2x2.avif"), bytes, None)
            .expect("AVIF fixture should decode");

        assert_eq!(block.color_image.size, [2, 2]);
        assert_eq!(block.size, vec2(24.0, 24.0));
    }
}
