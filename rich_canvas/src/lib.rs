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
    pub hovered_link_href: Option<String>,
    pub hovered_link_element_id: Option<String>,
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
    pub child_index: Option<usize>,
    pub child_count: Option<usize>,
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
    pub attribute_selectors: Vec<CssAttributeSelector>,
    pub nth_child: Option<CssNthChild>,
    pub nth_last_child: Option<CssNthChild>,
    pub not_selectors: Vec<SimpleCssSelector>,
    pub is_selectors: Vec<SimpleCssSelector>,
    pub where_selectors: Vec<SimpleCssSelector>,
    pub ancestor_chain: Vec<SimpleCssSelector>,
    pub ancestor: Option<SimpleCssSelector>,
    pub parent: Option<SimpleCssSelector>,
    pub previous_sibling: Option<SimpleCssSelector>,
    pub requires_previous_sibling: bool,
    pub general_previous_sibling: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimpleCssSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<String>,
    pub attribute_selectors: Vec<CssAttributeSelector>,
    pub nth_child: Option<CssNthChild>,
    pub nth_last_child: Option<CssNthChild>,
    pub not_selectors: Vec<SimpleCssSelector>,
    pub is_selectors: Vec<SimpleCssSelector>,
    pub where_selectors: Vec<SimpleCssSelector>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssAttributeSelector {
    pub name: String,
    pub operator: Option<CssAttributeOperator>,
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssAttributeOperator {
    Exact,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssNthChild {
    pub step: i32,
    pub offset: i32,
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
    pub list_style_type: Option<CssListStyleType>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub flex_basis: Option<CssLength>,
    pub flex_direction: Option<CssFlexDirection>,
    pub flex_wrap: Option<CssFlexWrap>,
    pub justify_content: Option<CssJustifyContent>,
    pub align_items: Option<CssAlignItems>,
    pub align_self: Option<CssAlignItems>,
    pub justify_items: Option<CssJustifyContent>,
    pub align_content: Option<CssAlignItems>,
    pub grid_template_columns: Option<usize>,
    pub grid_template_column_tracks: Option<Vec<CssLength>>,
    pub grid_auto_repeat_min_column_width: Option<CssLength>,
    pub grid_template_rows: Option<Vec<CssLength>>,
    pub grid_auto_rows: Option<CssLength>,
    pub grid_template_areas: Option<Vec<Vec<String>>>,
    pub grid_area: Option<String>,
    pub grid_column_start: Option<i32>,
    pub grid_column_end: Option<i32>,
    pub grid_column_span: Option<usize>,
    pub grid_row_span: Option<usize>,
    pub gap: Option<f32>,
    pub visibility_visible: Option<bool>,
    pub opacity: Option<f32>,
    pub overflow_hidden: Option<bool>,
    pub position: Option<CssPosition>,
    pub float: Option<CssFloat>,
    pub clear: Option<CssClear>,
    pub z_index: Option<i32>,
    pub inset: Option<CssEdges>,
    pub inset_sides: CssInset,
    pub transform: Option<CssTransform>,
    pub object_fit: Option<CssObjectFit>,
    pub box_sizing_border_box: Option<bool>,
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
    pub width_percent: Option<f32>,
    pub max_width: Option<f32>,
    pub max_width_percent: Option<f32>,
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
    pub list_style_type: CssListStyleType,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Option<CssLength>,
    pub flex_direction: CssFlexDirection,
    pub flex_wrap: CssFlexWrap,
    pub justify_content: CssJustifyContent,
    pub align_items: CssAlignItems,
    pub align_self: Option<CssAlignItems>,
    pub justify_items: CssJustifyContent,
    pub align_content: CssAlignItems,
    pub grid_template_columns: Option<usize>,
    pub grid_template_column_tracks: Option<Vec<CssLength>>,
    pub grid_auto_repeat_min_column_width: Option<CssLength>,
    pub grid_template_rows: Option<Vec<CssLength>>,
    pub grid_auto_rows: Option<CssLength>,
    pub grid_template_areas: Option<Vec<Vec<String>>>,
    pub grid_area: Option<String>,
    pub grid_column_start: Option<i32>,
    pub grid_column_end: Option<i32>,
    pub grid_column_span: usize,
    pub grid_row_span: usize,
    pub gap: f32,
    pub visibility_visible: bool,
    pub opacity: f32,
    pub overflow_hidden: bool,
    pub position: CssPosition,
    pub float: CssFloat,
    pub clear: CssClear,
    pub z_index: Option<i32>,
    pub inset: Option<CssEdges>,
    pub inset_sides: CssInset,
    pub transform: CssTransform,
    pub object_fit: CssObjectFit,
    pub box_sizing_border_box: bool,
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
            width_percent: None,
            max_width: None,
            max_width_percent: None,
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
            list_style_type: CssListStyleType::Disc,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            flex_direction: CssFlexDirection::Row,
            flex_wrap: CssFlexWrap::NoWrap,
            justify_content: CssJustifyContent::FlexStart,
            align_items: CssAlignItems::Stretch,
            align_self: None,
            justify_items: CssJustifyContent::FlexStart,
            align_content: CssAlignItems::Stretch,
            grid_template_columns: None,
            grid_template_column_tracks: None,
            grid_auto_repeat_min_column_width: None,
            grid_template_rows: None,
            grid_auto_rows: None,
            grid_template_areas: None,
            grid_area: None,
            grid_column_start: None,
            grid_column_end: None,
            grid_column_span: 1,
            grid_row_span: 1,
            gap: 0.0,
            visibility_visible: true,
            opacity: 1.0,
            overflow_hidden: false,
            position: CssPosition::Static,
            float: CssFloat::None,
            clear: CssClear::None,
            z_index: None,
            inset: None,
            inset_sides: CssInset::default(),
            transform: CssTransform::default(),
            object_fit: CssObjectFit::Fill,
            box_sizing_border_box: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssDisplay {
    None,
    Contents,
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    Table,
    ListItem,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CssListStyleType {
    None,
    #[default]
    Disc,
    Decimal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CssFloat {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CssClear {
    #[default]
    None,
    Both,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CssTransform {
    pub translate_x: Option<CssLength>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CssLength {
    Auto,
    Px(f32),
    Percent(f32),
    Fr(f32),
    Vw(f32),
    Vh(f32),
    Calc(CssLengthExpression),
    Min(CssLengthExpression, CssLengthExpression),
    Max(CssLengthExpression, CssLengthExpression),
    Clamp(
        CssLengthExpression,
        CssLengthExpression,
        CssLengthExpression,
    ),
    FitContent(CssLengthExpression),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CssLengthExpression {
    pub px: f32,
    pub percent: f32,
    pub vw: f32,
    pub vh: f32,
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
pub enum CssFlexWrap {
    NoWrap,
    Wrap,
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
    RichTextLine(CanvasRichTextLineObject),
    Rect(CanvasRectObject),
    Button(CanvasButtonObject),
    Input(CanvasInputObject),
    Image(CanvasImageObject),
    Svg(CanvasSvgObject),
    Media(CanvasMediaObject),
    LinkHit(CanvasLinkHitObject),
    ClipStart(CanvasClipObject),
    ClipEnd,
}

#[derive(Clone, Debug)]
pub struct CanvasRichTextLineObject {
    pub rect: Rect,
    pub spans: Vec<CanvasTextSpan>,
    pub text_align: CssTextAlign,
}

#[derive(Clone, Debug)]
pub struct CanvasTextSpan {
    pub text: String,
    pub color: Color32,
    pub font_size: f32,
    pub font_weight_bold: bool,
    pub font_style_italic: bool,
    pub text_decoration_underline: bool,
    pub text_decoration_strikethrough: bool,
    pub text_background: Color32,
    pub href: Option<String>,
    pub element_id: Option<String>,
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
    pub text_inset_x: f32,
    pub color: Color32,
    pub font_size: f32,
    pub font_weight_bold: bool,
    pub font_style_italic: bool,
    pub text_decoration_underline: bool,
    pub text_decoration_strikethrough: bool,
    pub text_background: Color32,
    pub text_align: CssTextAlign,
    pub href: Option<String>,
    pub element_id: Option<String>,
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
pub struct CanvasButtonObject {
    pub text: String,
    pub rect: Rect,
    pub button_type: String,
    pub form_id: Option<String>,
    pub form_action: Option<String>,
    pub form_method: Option<String>,
    pub element_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CanvasInputKind {
    #[default]
    Text,
    Hidden,
    Password,
    TextArea,
    Checkbox,
    Radio,
    Select {
        options: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct CanvasInputObject {
    pub label: String,
    pub name: Option<String>,
    pub value: String,
    pub default_value: String,
    pub rect: Rect,
    pub font_size: f32,
    pub color: Color32,
    pub form_id: Option<String>,
    pub form_action: Option<String>,
    pub form_method: Option<String>,
    pub element_id: Option<String>,
    pub kind: CanvasInputKind,
    pub submit_on_enter: bool,
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

#[derive(Clone, Debug)]
pub struct CanvasLinkHitObject {
    pub rect: Rect,
    pub href: String,
    pub element_id: Option<String>,
    pub debug_visible: bool,
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
    pub element_id: Option<String>,
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
    Link {
        href: String,
        element_id: Option<String>,
    },
    Button {
        text: String,
        element_id: Option<String>,
        button_type: String,
        form_id: Option<String>,
    },
    Input {
        label: String,
        element_id: Option<String>,
    },
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
    pub focused: Option<HitTarget>,
    pub changed_inputs: Vec<InputChange>,
    pub input_key_events: Vec<InputKeyEvent>,
    pub submitted_inputs: Vec<InputSubmit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputChange {
    pub label: String,
    pub value_len: usize,
    pub element_id: Option<String>,
    pub value: String,
    pub kind: CanvasInputKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputKeyEvent {
    pub label: String,
    pub element_id: Option<String>,
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputSubmit {
    pub label: String,
    pub name: Option<String>,
    pub value: String,
    pub form_id: Option<String>,
    pub form_action: Option<String>,
    pub form_method: Option<String>,
    pub element_id: Option<String>,
    pub submitter_element_id: Option<String>,
    pub kind: CanvasInputKind,
}

impl BrowserCanvas {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            scroll_offset: Vec2::ZERO,
            hovered_link_href: None,
            hovered_link_element_id: None,
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
                                false,
                                self.hovered_link_href.as_deref(),
                                self.hovered_link_element_id.as_deref(),
                            );
                        }
                    });
                });
            });
        self.scroll_offset = output.state.offset;
        (self.hovered_link_href, self.hovered_link_element_id) =
            hovered_link_identity(canvas_response.hovered.as_ref());

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
                            true,
                            self.hovered_link_href.as_deref(),
                            self.hovered_link_element_id.as_deref(),
                        );
                    });
                });
            });
        self.scroll_offset = output.state.offset;
        (self.hovered_link_href, self.hovered_link_element_id) =
            hovered_link_identity(canvas_response.hovered.as_ref());

        canvas_response
    }
}

fn hovered_link_identity(target: Option<&HitTarget>) -> (Option<String>, Option<String>) {
    match target {
        Some(HitTarget::Link { href, element_id }) => (Some(href.clone()), element_id.clone()),
        _ => (None, None),
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
    read_only: bool,
    hovered_link_href: Option<&str>,
    hovered_link_element_id: Option<&str>,
) {
    let graph_width = graph.viewport.x.max(1.0);
    let scale = (content_width / graph_width).max(0.1) * font_scale;
    let graph_size = vec2(content_width.max(1.0), (graph.viewport.y * scale).max(1.0));
    let (canvas_rect, _) = ui.allocate_exact_size(graph_size, Sense::hover());
    let mut painter = ui.painter().with_clip_rect(canvas_rect);
    let mut current_clip_rect = canvas_rect;
    let mut clip_stack = Vec::new();
    let mut submitted_forms: Vec<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = Vec::new();
    let mut reset_forms: Vec<Option<String>> = Vec::new();
    let editable_input_hit_rects =
        editable_input_hit_rects(&graph.objects, canvas_rect.min, scale, current_clip_rect);
    // (name, element_id) of radio buttons clicked this frame — used to deselect group peers.
    let mut selected_radios: Vec<(Option<String>, Option<String>)> = Vec::new();

    for (index, object) in graph.objects.iter_mut().enumerate() {
        match object {
            CanvasObject::ClipStart(clip) => {
                let rect = canvas_object_rect(canvas_rect.min, clip.rect, scale);
                let previous = painter.clip_rect();
                let next = previous.intersect(rect);
                clip_stack.push(previous);
                painter = ui.painter().with_clip_rect(next);
                current_clip_rect = next;
            }
            CanvasObject::ClipEnd => {
                let previous = clip_stack.pop().unwrap_or(canvas_rect);
                painter = ui.painter().with_clip_rect(previous);
                current_clip_rect = previous;
            }
            CanvasObject::Text(text) => {
                let rect = canvas_object_rect(canvas_rect.min, text.rect, scale);
                let text_inset_x = (text.text_inset_x * scale).max(0.0);
                let paint_rect = Rect::from_min_size(
                    Pos2::new(rect.left() + text_inset_x, rect.top()),
                    vec2((rect.width() - text_inset_x * 2.0).max(1.0), rect.height()),
                );
                let family = if text.font_weight_bold {
                    browser_bold_family()
                } else {
                    browser_regular_family()
                };
                let font_size = text.font_size * scale;
                let link_hovered = canvas_text_link_hovered(
                    text.href.as_deref(),
                    text.element_id.as_deref(),
                    hovered_link_href,
                    hovered_link_element_id,
                );
                let text_color = canvas_link_text_color(text.color, link_hovered);
                let stroke = Stroke::new((1.0 * scale).max(1.0), text_color);
                let text_format = TextFormat {
                    font_id: FontId::new(font_size, family),
                    color: text_color,
                    background: text.text_background,
                    italics: text.font_style_italic,
                    underline: if text.text_decoration_underline || link_hovered {
                        stroke
                    } else {
                        Stroke::NONE
                    },
                    strikethrough: if text.text_decoration_strikethrough {
                        stroke
                    } else {
                        Stroke::NONE
                    },
                    line_height: Some((font_size * 1.35).max(paint_rect.height()).max(1.0)),
                    ..Default::default()
                };
                let mut job = LayoutJob::simple_format(text.text.clone(), text_format);
                job.wrap.max_width = f32::INFINITY;
                job.wrap.max_rows = 1;
                let galley = painter.layout_job(job);
                let text_width = galley.size().x.min(paint_rect.width()).max(1.0);
                let text_rect = match text.text_align {
                    CssTextAlign::Left => {
                        Rect::from_min_size(paint_rect.min, vec2(text_width, paint_rect.height()))
                    }
                    CssTextAlign::Center => Rect::from_center_size(
                        Pos2::new(paint_rect.center().x, paint_rect.center().y),
                        vec2(text_width, paint_rect.height()),
                    ),
                    CssTextAlign::Right => Rect::from_min_size(
                        Pos2::new(paint_rect.right() - text_width, paint_rect.top()),
                        vec2(text_width, paint_rect.height()),
                    ),
                };
                painter.add(TextShape::new(text_rect.left_top(), galley, text_color));
                if let Some(href) = &text.href {
                    let hit_rect = rect.intersect(current_clip_rect);
                    if hit_rect.is_positive() {
                        let response = ui.interact(
                            hit_rect,
                            ui.make_persistent_id(("canvas_graph_text", index)),
                            Sense::click(),
                        );
                        if response.hovered() {
                            canvas_response.hovered = Some(HitTarget::Link {
                                href: href.clone(),
                                element_id: text.element_id.clone(),
                            });
                        }
                        if response.clicked() {
                            canvas_response.clicked = Some(HitTarget::Link {
                                href: href.clone(),
                                element_id: text.element_id.clone(),
                            });
                        }
                    }
                }
            }
            CanvasObject::RichTextLine(line) => {
                let rect = canvas_object_rect(canvas_rect.min, line.rect, scale);
                let mut job = LayoutJob::default();
                for span in &line.spans {
                    let family = if span.font_weight_bold {
                        browser_bold_family()
                    } else {
                        browser_regular_family()
                    };
                    let font_size = span.font_size * scale;
                    let link_hovered = canvas_text_link_hovered(
                        span.href.as_deref(),
                        span.element_id.as_deref(),
                        hovered_link_href,
                        hovered_link_element_id,
                    );
                    let text_color = canvas_link_text_color(span.color, link_hovered);
                    let stroke = Stroke::new((1.0 * scale).max(1.0), text_color);
                    let text_format = TextFormat {
                        font_id: FontId::new(font_size, family),
                        color: text_color,
                        background: span.text_background,
                        italics: span.font_style_italic,
                        underline: if span.text_decoration_underline || link_hovered {
                            stroke
                        } else {
                            Stroke::NONE
                        },
                        strikethrough: if span.text_decoration_strikethrough {
                            stroke
                        } else {
                            Stroke::NONE
                        },
                        line_height: Some((font_size * 1.35).max(rect.height()).max(1.0)),
                        ..Default::default()
                    };
                    job.append(&span.text, 0.0, text_format);
                }
                job.wrap.max_width = f32::INFINITY;
                job.wrap.max_rows = 1;
                let galley = painter.layout_job(job);
                let text_width = galley.size().x.min(rect.width()).max(1.0);
                let text_height = galley.size().y.max(rect.height()).max(1.0);
                let text_rect = match line.text_align {
                    CssTextAlign::Left => {
                        Rect::from_min_size(rect.min, vec2(text_width, text_height))
                    }
                    CssTextAlign::Center => Rect::from_center_size(
                        Pos2::new(rect.center().x, rect.top() + text_height * 0.5),
                        vec2(text_width, text_height),
                    ),
                    CssTextAlign::Right => Rect::from_min_size(
                        Pos2::new(rect.right() - text_width, rect.top()),
                        vec2(text_width, text_height),
                    ),
                };
                painter.add(TextShape::new(text_rect.left_top(), galley, Color32::WHITE));
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
            CanvasObject::Button(button) => {
                let rect = canvas_object_rect(canvas_rect.min, button.rect, scale);
                let hit_rect = rect.intersect(current_clip_rect);
                let Some(response) = hit_rect.is_positive().then(|| {
                    ui.interact(
                        hit_rect,
                        ui.make_persistent_id(("canvas_graph_button", index)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                }) else {
                    continue;
                };
                let defer_to_input =
                    ui.input(|input| input.pointer.hover_pos())
                        .is_some_and(|pos| {
                            button_hit_deferred_to_editable_input(
                                hit_rect,
                                pos,
                                &editable_input_hit_rects,
                            )
                        });
                if defer_to_input {
                    continue;
                }
                if response.hovered() {
                    canvas_response.hovered = Some(HitTarget::Button {
                        text: button.text.clone(),
                        element_id: button.element_id.clone(),
                        button_type: button.button_type.clone(),
                        form_id: button.form_id.clone(),
                    });
                }
                if response.clicked() {
                    canvas_response.clicked = Some(HitTarget::Button {
                        text: button.text.clone(),
                        element_id: button.element_id.clone(),
                        button_type: button.button_type.clone(),
                        form_id: button.form_id.clone(),
                    });
                    if button.button_type.eq_ignore_ascii_case("submit") {
                        submitted_forms.push((
                            button.form_id.clone(),
                            button.form_action.clone(),
                            button.form_method.clone(),
                            button.element_id.clone(),
                        ));
                    } else if button.button_type.eq_ignore_ascii_case("reset") {
                        reset_forms.push(button.form_id.clone());
                    }
                }
            }
            CanvasObject::Input(input) => {
                let rect = canvas_object_rect(canvas_rect.min, input.rect, scale);
                if read_only {
                    let display = if input.value.is_empty() {
                        input.label.as_str()
                    } else {
                        input.value.as_str()
                    };
                    ui.put(
                        rect,
                        egui::Label::new(
                            egui::RichText::new(display)
                                .size(input.font_size * scale)
                                .color(input.color),
                        ),
                    );
                } else {
                    match &input.kind {
                        CanvasInputKind::Hidden => {}
                        CanvasInputKind::Checkbox => {
                            let mut checked = input.value == "true";
                            let response = ui.put(
                                rect,
                                egui::Checkbox::new(&mut checked, input.label.as_str()),
                            );
                            if response.changed() {
                                input.value = if checked { "true" } else { "false" }.to_owned();
                                canvas_response.changed_inputs.push(InputChange {
                                    label: input.label.clone(),
                                    value_len: input.value.len(),
                                    element_id: input.element_id.clone(),
                                    value: input.value.clone(),
                                    kind: input.kind.clone(),
                                });
                            }
                        }
                        CanvasInputKind::Radio => {
                            let checked = input.value == "true";
                            let response =
                                ui.put(rect, egui::RadioButton::new(checked, input.label.as_str()));
                            if response.clicked() && !checked {
                                input.value = "true".to_owned();
                                selected_radios
                                    .push((input.name.clone(), input.element_id.clone()));
                                canvas_response.changed_inputs.push(InputChange {
                                    label: input.label.clone(),
                                    value_len: input.value.len(),
                                    element_id: input.element_id.clone(),
                                    value: input.value.clone(),
                                    kind: input.kind.clone(),
                                });
                            }
                        }
                        CanvasInputKind::Select { options } => {
                            let options = options.clone();
                            let mut selected = input.value.clone();
                            let combo_id = ui.make_persistent_id(("canvas_graph_select", index));
                            let mut selection_changed = false;
                            ui.allocate_ui_at_rect(rect, |ui| {
                                egui::ComboBox::from_id_salt(combo_id)
                                    .selected_text(selected.as_str())
                                    .width(rect.width())
                                    .show_ui(ui, |ui| {
                                        for option in &options {
                                            // selectable_label + explicit click tracking
                                            // mirrors what show_index does internally.
                                            if ui
                                                .selectable_label(
                                                    selected == *option,
                                                    option.as_str(),
                                                )
                                                .clicked()
                                            {
                                                selected = option.clone();
                                                selection_changed = true;
                                            }
                                        }
                                    });
                            });
                            if selection_changed {
                                input.value = selected.clone();
                                canvas_response.changed_inputs.push(InputChange {
                                    label: input.label.clone(),
                                    value_len: selected.chars().count(),
                                    element_id: input.element_id.clone(),
                                    value: selected,
                                    kind: input.kind.clone(),
                                });
                            }
                        }
                        CanvasInputKind::TextArea => {
                            let mut value = input.value.clone();
                            let text_edit_id = canvas_graph_text_edit_id(
                                "textarea",
                                input.element_id.as_deref(),
                                index,
                            );
                            let response = if input.submit_on_enter {
                                ui.put(
                                    rect,
                                    egui::TextEdit::singleline(&mut value)
                                        .id(text_edit_id)
                                        .hint_text(input.label.as_str())
                                        .frame(false)
                                        .font(FontId::new(
                                            input.font_size * scale,
                                            browser_regular_family(),
                                        ))
                                        .text_color(input.color),
                                )
                            } else {
                                ui.put(
                                    rect,
                                    egui::TextEdit::multiline(&mut value)
                                        .id(text_edit_id)
                                        .hint_text(input.label.as_str())
                                        .font(FontId::new(
                                            input.font_size * scale,
                                            browser_regular_family(),
                                        ))
                                        .text_color(input.color),
                                )
                            };
                            if response.hovered() {
                                canvas_response.hovered = Some(HitTarget::Input {
                                    label: input.label.clone(),
                                    element_id: input.element_id.clone(),
                                });
                            }
                            if response.changed() {
                                input.value = value.clone();
                                canvas_response.changed_inputs.push(InputChange {
                                    label: input.label.clone(),
                                    value_len: value.chars().count(),
                                    element_id: input.element_id.clone(),
                                    value: value.clone(),
                                    kind: input.kind.clone(),
                                });
                            }
                            if response.has_focus() {
                                canvas_response.focused = Some(HitTarget::Input {
                                    label: input.label.clone(),
                                    element_id: input.element_id.clone(),
                                });
                                for key in pressed_key_names(ui) {
                                    canvas_response.input_key_events.push(InputKeyEvent {
                                        label: input.label.clone(),
                                        element_id: input.element_id.clone(),
                                        key,
                                        value: value.clone(),
                                    });
                                }
                            }
                            if text_control_enter_submitted(ui, &response, false) {
                                submitted_forms.push((
                                    input.form_id.clone(),
                                    input.form_action.clone(),
                                    input.form_method.clone(),
                                    input.element_id.clone(),
                                ));
                            }
                        }
                        CanvasInputKind::Text | CanvasInputKind::Password => {
                            let password = matches!(input.kind, CanvasInputKind::Password);
                            let mut value = input.value.clone();
                            let text_edit_id = canvas_graph_text_edit_id(
                                "input",
                                input.element_id.as_deref(),
                                index,
                            );
                            let response = ui.put(
                                rect,
                                egui::TextEdit::singleline(&mut value)
                                    .id(text_edit_id)
                                    .hint_text(input.label.as_str())
                                    .frame(false)
                                    .password(password)
                                    .font(FontId::new(
                                        input.font_size * scale,
                                        browser_regular_family(),
                                    ))
                                    .text_color(input.color),
                            );
                            if response.hovered() {
                                canvas_response.hovered = Some(HitTarget::Input {
                                    label: input.label.clone(),
                                    element_id: input.element_id.clone(),
                                });
                            }
                            if response.changed() {
                                input.value = value.clone();
                                canvas_response.changed_inputs.push(InputChange {
                                    label: input.label.clone(),
                                    value_len: value.chars().count(),
                                    element_id: input.element_id.clone(),
                                    value: value.clone(),
                                    kind: input.kind.clone(),
                                });
                            }
                            if response.has_focus() {
                                canvas_response.focused = Some(HitTarget::Input {
                                    label: input.label.clone(),
                                    element_id: input.element_id.clone(),
                                });
                                for key in pressed_key_names(ui) {
                                    canvas_response.input_key_events.push(InputKeyEvent {
                                        label: input.label.clone(),
                                        element_id: input.element_id.clone(),
                                        key,
                                        value: value.clone(),
                                    });
                                }
                            }
                            if text_control_enter_submitted(ui, &response, true) {
                                submitted_forms.push((
                                    input.form_id.clone(),
                                    input.form_action.clone(),
                                    input.form_method.clone(),
                                    input.element_id.clone(),
                                ));
                            }
                        }
                    }
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
            CanvasObject::LinkHit(link) => {
                let rect = canvas_object_rect(canvas_rect.min, link.rect, scale);
                if link.debug_visible {
                    painter.rect_filled(
                        rect.intersect(current_clip_rect),
                        CornerRadius::ZERO,
                        Color32::from_rgba_unmultiplied(255, 224, 64, 42),
                    );
                }
                let hit_rect = rect.intersect(current_clip_rect);
                if hit_rect.is_positive() {
                    let response = ui
                        .interact(
                            hit_rect,
                            ui.make_persistent_id(("canvas_graph_link_hit", index)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.hovered() {
                        canvas_response.hovered = Some(HitTarget::Link {
                            href: link.href.clone(),
                            element_id: link.element_id.clone(),
                        });
                    }
                    if response.clicked() {
                        canvas_response.clicked = Some(HitTarget::Link {
                            href: link.href.clone(),
                            element_id: link.element_id.clone(),
                        });
                    }
                }
            }
        }
    }
    for (name, element_id) in selected_radios {
        for object in &mut graph.objects {
            if let CanvasObject::Input(radio) = object {
                if matches!(&radio.kind, CanvasInputKind::Radio)
                    && radio.name == name
                    && radio.element_id != element_id
                {
                    radio.value = "false".to_owned();
                }
            }
        }
    }
    for form_id in reset_forms {
        reset_inputs_for_form(&mut graph.objects, form_id.as_deref(), canvas_response);
    }
    for (form_id, form_action, form_method, submitter_element_id) in submitted_forms {
        push_submitted_inputs_for_form(
            &graph.objects,
            form_id.as_deref(),
            form_action.as_deref(),
            form_method.as_deref(),
            submitter_element_id.as_deref(),
            canvas_response,
        );
    }
}

fn reset_inputs_for_form(
    objects: &mut [CanvasObject],
    form_id: Option<&str>,
    canvas_response: &mut BrowserCanvasResponse,
) {
    for object in objects {
        let CanvasObject::Input(input) = object else {
            continue;
        };
        if !canvas_input_belongs_to_form(input, form_id) {
            continue;
        }
        if input.value != input.default_value {
            input.value = input.default_value.clone();
            canvas_response.changed_inputs.push(InputChange {
                label: input.label.clone(),
                value_len: input.value.chars().count(),
                element_id: input.element_id.clone(),
                value: input.value.clone(),
                kind: input.kind.clone(),
            });
        }
    }
}

fn push_submitted_inputs_for_form(
    objects: &[CanvasObject],
    form_id: Option<&str>,
    form_action: Option<&str>,
    form_method: Option<&str>,
    submitter_element_id: Option<&str>,
    canvas_response: &mut BrowserCanvasResponse,
) {
    for object in objects {
        let CanvasObject::Input(input) = object else {
            continue;
        };
        if canvas_input_belongs_to_form(input, form_id) {
            canvas_response.submitted_inputs.push(InputSubmit {
                label: input.label.clone(),
                name: input.name.clone(),
                value: input.value.clone(),
                form_id: input.form_id.clone(),
                form_action: form_action.map(str::to_owned),
                form_method: form_method.map(str::to_owned),
                element_id: input.element_id.clone(),
                submitter_element_id: submitter_element_id.map(str::to_owned),
                kind: input.kind.clone(),
            });
        }
    }
}

fn canvas_input_belongs_to_form(input: &CanvasInputObject, form_id: Option<&str>) -> bool {
    match (form_id, input.form_id.as_deref()) {
        (Some(expected), Some(actual)) => expected == actual,
        (None, None) => true,
        _ => false,
    }
}

fn editable_input_hit_rects(
    objects: &[CanvasObject],
    canvas_origin: Pos2,
    scale: f32,
    clip_rect: Rect,
) -> Vec<Rect> {
    objects
        .iter()
        .filter_map(|object| {
            let CanvasObject::Input(input) = object else {
                return None;
            };
            if !matches!(
                input.kind,
                CanvasInputKind::Text | CanvasInputKind::Password | CanvasInputKind::TextArea
            ) {
                return None;
            }
            let rect = canvas_object_rect(canvas_origin, input.rect, scale).intersect(clip_rect);
            rect.is_positive().then_some(rect)
        })
        .collect()
}

fn button_hit_deferred_to_editable_input(
    button_hit_rect: Rect,
    pointer_pos: Pos2,
    editable_input_hit_rects: &[Rect],
) -> bool {
    editable_input_hit_rects.iter().any(|input_rect| {
        input_rect.contains(pointer_pos) && input_rect.intersect(button_hit_rect).is_positive()
    })
}

fn canvas_link_text_color(base: Color32, hovered: bool) -> Color32 {
    if hovered {
        darken_link_color(base)
    } else {
        base
    }
}

fn canvas_text_link_hovered(
    text_href: Option<&str>,
    text_element_id: Option<&str>,
    hovered_href: Option<&str>,
    hovered_element_id: Option<&str>,
) -> bool {
    let Some(text_href) = text_href else {
        return false;
    };
    if let (Some(text_element_id), Some(hovered_element_id)) = (text_element_id, hovered_element_id)
    {
        return text_element_id == hovered_element_id;
    }
    Some(text_href) == hovered_href
}

fn darken_link_color(base: Color32) -> Color32 {
    let darken = |channel: u8| ((channel as f32) * 0.58).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(
        darken(base.r()),
        darken(base.g()),
        darken(base.b()),
        base.a(),
    )
}

fn canvas_graph_text_edit_id(
    kind: &'static str,
    element_id: Option<&str>,
    index: usize,
) -> egui::Id {
    match element_id {
        Some(element_id) => egui::Id::new(("canvas_graph_text_edit", kind, element_id)),
        None => egui::Id::new(("canvas_graph_text_edit", kind, index)),
    }
}

fn text_control_enter_submitted(ui: &Ui, response: &egui::Response, allow_shift: bool) -> bool {
    let enter_pressed = ui.input(|state| {
        state.key_pressed(egui::Key::Enter) && (allow_shift || !state.modifiers.shift)
    });
    enter_pressed && (response.has_focus() || response.lost_focus())
}

fn pressed_key_names(ui: &Ui) -> Vec<String> {
    ui.input(|state| {
        state
            .events
            .iter()
            .filter_map(|event| {
                if let egui::Event::Key {
                    key, pressed: true, ..
                } = event
                {
                    Some(key.name().to_owned())
                } else {
                    None
                }
            })
            .collect()
    })
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
    browser_text_galley_no_wrap(ctx, text, style).size()
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

    let size = measure_browser_textbox_from_lut(ctx, lut, text, style);
    lut.insert_text_size(text, size);
    size
}

fn measure_browser_textbox_from_lut(
    ctx: &egui::Context,
    lut: &mut BrowserFontSizeLut,
    text: &str,
    style: &ResolvedBoxStyle,
) -> Vec2 {
    let mut width: f32 = 0.0;
    let mut height = lut.line_height.max((style.font_size * 1.35).max(1.0));
    for character in text.chars() {
        let size = if let Some(size) = lut.glyph_size(character) {
            size
        } else {
            let size = measure_browser_textbox(ctx, &character.to_string(), style);
            lut.glyph_sizes.insert(character, size);
            size
        };
        width += size.x.max(0.0) + browser_extra_word_spacing(character, style.font_size);
        height = height.max(size.y);
    }
    vec2(width.max(1.0), height.max(1.0))
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

fn browser_text_galley_no_wrap(
    ctx: &egui::Context,
    text: &str,
    style: &ResolvedBoxStyle,
) -> std::sync::Arc<egui::Galley> {
    let mut job = browser_text_layout_job(text, style);
    job.wrap.max_width = f32::INFINITY;
    job.wrap.max_rows = 1;
    ctx.fonts_mut(|fonts| fonts.layout_job(job))
}

fn browser_text_layout_job(text: &str, style: &ResolvedBoxStyle) -> LayoutJob {
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
    LayoutJob::simple_format(text.to_owned(), text_format)
}

fn wrap_browser_textboxes_estimated(
    text: &str,
    max_width: f32,
    font_size: f32,
) -> Vec<CanvasMeasuredText> {
    let tolerance = 0.5;
    let measured_width = estimated_browser_text_width(text, font_size);
    if measured_width <= max_width + tolerance {
        return vec![CanvasMeasuredText {
            text: text.to_owned(),
            size: vec2(measured_width.min(max_width), font_size * 1.35),
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
            && estimated_browser_text_width(&candidate, font_size) > max_width + tolerance
        {
            lines.push(CanvasMeasuredText {
                size: vec2(
                    estimated_browser_text_width(&current, font_size).min(max_width),
                    font_size * 1.35,
                ),
                text: std::mem::take(&mut current),
            });
        }
        if current.is_empty() {
            current.push_str(word);
        } else {
            current.push(' ');
            current.push_str(word);
        }
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
    text.chars()
        .map(|character| estimated_browser_glyph_width(character, font_size))
        .sum::<f32>()
        .max(1.0)
}

fn estimated_browser_glyph_width(character: char, font_size: f32) -> f32 {
    let factor = match character {
        ' ' | '\t' => 0.36,
        'i' | 'l' | 'I' | '!' | '|' => 0.25,
        'j' | 'f' | 'r' | 't' | '\'' | '"' | '`' => 0.32,
        '.' | ',' | ':' | ';' => 0.28,
        '(' | ')' | '[' | ']' | '{' | '}' => 0.34,
        '-' | '_' | '/' | '\\' => 0.38,
        'm' | 'w' | 'M' | 'W' => 0.82,
        character if character.is_ascii_uppercase() => 0.64,
        character if character.is_ascii_digit() => 0.56,
        character if character.is_ascii_lowercase() => 0.50,
        _ => 0.56,
    };
    font_size * factor
}

fn browser_extra_word_spacing(character: char, font_size: f32) -> f32 {
    if character == ' ' || character == '\t' {
        font_size * 0.08
    } else {
        0.0
    }
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
                canvas_response.hovered = Some(HitTarget::Link {
                    href: href.clone(),
                    element_id: None,
                });
            }
            if response.clicked() {
                canvas_response.clicked = Some(HitTarget::Link {
                    href: href.clone(),
                    element_id: None,
                });
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
                        canvas_response.hovered = Some(HitTarget::Link {
                            href: href.clone(),
                            element_id: None,
                        });
                    }
                    if response.clicked() {
                        canvas_response.clicked = Some(HitTarget::Link {
                            href: href.clone(),
                            element_id: None,
                        });
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
                canvas_response.hovered = Some(HitTarget::Button {
                    text: text.clone(),
                    element_id: None,
                    button_type: "button".to_owned(),
                    form_id: None,
                });
            }
            if response.clicked() {
                canvas_response.clicked = Some(HitTarget::Button {
                    text: text.clone(),
                    element_id: None,
                    button_type: "button".to_owned(),
                    form_id: None,
                });
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
                    element_id: None,
                });
            }
            if response.changed() {
                canvas_response.changed_inputs.push(InputChange {
                    label: label.clone(),
                    value_len: value.chars().count(),
                    element_id: None,
                    value: value.clone(),
                    kind: CanvasInputKind::Text,
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
    const DEFAULT_VIEWPORT_WIDTH: f32 = 1280.0;
    const DEFAULT_VIEWPORT_HEIGHT: f32 = 900.0;

    match length {
        CssLength::Auto => containing_width,
        CssLength::Px(px) => px,
        CssLength::Percent(percent) => containing_width * percent / 100.0,
        CssLength::Fr(_) => containing_width,
        CssLength::Vw(vw) => DEFAULT_VIEWPORT_WIDTH * vw / 100.0,
        CssLength::Vh(vh) => DEFAULT_VIEWPORT_HEIGHT * vh / 100.0,
        CssLength::Calc(expression) => css_length_expression_px(
            expression,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        ),
        CssLength::Min(left, right) => css_length_expression_px(
            left,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        )
        .min(css_length_expression_px(
            right,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        )),
        CssLength::Max(left, right) => css_length_expression_px(
            left,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        )
        .max(css_length_expression_px(
            right,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        )),
        CssLength::Clamp(min, preferred, max) => css_length_expression_px(
            preferred,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        )
        .clamp(
            css_length_expression_px(
                min,
                containing_width,
                DEFAULT_VIEWPORT_WIDTH,
                DEFAULT_VIEWPORT_HEIGHT,
            ),
            css_length_expression_px(
                max,
                containing_width,
                DEFAULT_VIEWPORT_WIDTH,
                DEFAULT_VIEWPORT_HEIGHT,
            ),
        ),
        CssLength::FitContent(limit) => css_length_expression_px(
            limit,
            containing_width,
            DEFAULT_VIEWPORT_WIDTH,
            DEFAULT_VIEWPORT_HEIGHT,
        ),
    }
}

fn css_length_expression_px(
    expression: CssLengthExpression,
    containing_width: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> f32 {
    expression.px
        + containing_width * expression.percent / 100.0
        + viewport_width * expression.vw / 100.0
        + viewport_height * expression.vh / 100.0
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
            element_id: None,
        });
    }
    if response.clicked() {
        canvas_response.clicked = Some(HitTarget::Link {
            href: href.to_owned(),
            element_id: None,
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
            element_id: None,
            button_type: "submit".to_owned(),
            form_id: None,
        });
    }
    paint_search_icon(&painter, search_button_center, font_scale);
    if search_response.clicked() {
        canvas_response.clicked = Some(HitTarget::Button {
            text: "Search".to_owned(),
            element_id: None,
            button_type: "submit".to_owned(),
            form_id: None,
        });
        push_ecosia_search_submit(&hero.search_value, canvas_response);
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
                    .id(egui::Id::new("ecosia-hero-search-input"))
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
            element_id: None,
        });
    }
    if input_response.changed() {
        canvas_response.changed_inputs.push(InputChange {
            label: "Search".to_owned(),
            value_len: hero.search_value.chars().count(),
            element_id: None,
            value: hero.search_value.clone(),
            kind: CanvasInputKind::Text,
        });
    }
    if input_response.has_focus() {
        canvas_response.focused = Some(HitTarget::Input {
            label: "Search".to_owned(),
            element_id: None,
        });
        for key in pressed_key_names(ui) {
            canvas_response.input_key_events.push(InputKeyEvent {
                label: "Search".to_owned(),
                element_id: None,
                key,
                value: hero.search_value.clone(),
            });
        }
    }
    if text_control_enter_submitted(ui, &input_response, true) {
        push_ecosia_search_submit(&hero.search_value, canvas_response);
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
            element_id: None,
            button_type: "button".to_owned(),
            form_id: None,
        });
    }
    if ai_response.clicked() {
        canvas_response.clicked = Some(HitTarget::Button {
            text: hero.ai_button_text.clone(),
            element_id: None,
            button_type: "button".to_owned(),
            form_id: None,
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

fn push_ecosia_search_submit(value: &str, canvas_response: &mut BrowserCanvasResponse) {
    canvas_response.submitted_inputs.push(InputSubmit {
        label: "Search".to_owned(),
        name: Some("q".to_owned()),
        value: value.to_owned(),
        form_id: None,
        form_action: Some("/search".to_owned()),
        form_method: Some("get".to_owned()),
        element_id: None,
        submitter_element_id: None,
        kind: CanvasInputKind::Text,
    });
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
                canvas_response.hovered = Some(HitTarget::Link {
                    href: href.clone(),
                    element_id: span.element_id.clone(),
                });
            }
            if response.clicked() {
                canvas_response.clicked = Some(HitTarget::Link {
                    href: href.clone(),
                    element_id: span.element_id.clone(),
                });
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
            canvas_response.hovered = Some(HitTarget::Link {
                href: href.clone(),
                element_id: span.element_id.clone(),
            });
        }
        if response.clicked() {
            canvas_response.clicked = Some(HitTarget::Link {
                href: href.clone(),
                element_id: span.element_id.clone(),
            });
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
    parse_basic_css_inner(css, None, &[])
}

pub fn parse_basic_css_for_viewport(css: &str, viewport_width: f32) -> BrowserStyle {
    parse_basic_css_inner(css, Some(viewport_width), &[])
}

pub fn parse_basic_css_for_viewport_with_root_classes(
    css: &str,
    viewport_width: f32,
    root_classes: &[String],
) -> BrowserStyle {
    parse_basic_css_inner(css, Some(viewport_width), root_classes)
}

fn parse_basic_css_inner(
    css: &str,
    viewport_width: Option<f32>,
    root_classes: &[String],
) -> BrowserStyle {
    let mut style = BrowserStyle::default();
    let css = strip_css_comments(css);
    let css = if let Some(viewport_width) = viewport_width {
        expand_supported_css_at_rule_blocks(&css, viewport_width)
    } else {
        strip_unsupported_css_at_rule_blocks(&css)
    };
    style.css_variables = collect_css_custom_properties(&css, root_classes);
    for (order, rule) in css.split('}').enumerate() {
        let Some((selectors, declarations)) = rule.split_once('{') else {
            continue;
        };
        for selector in split_css_selector_list(selectors) {
            let variables = style.css_variables.clone();
            apply_css_rule(&mut style, &selector, declarations, &variables);
            if let (Some(selector), Some(box_style)) = (
                parse_css_selector(&selector),
                parse_css_box_style_with_vars(declarations, &style.css_variables, viewport_width),
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

fn collect_css_custom_properties(css: &str, root_classes: &[String]) -> HashMap<String, String> {
    let mut variables = HashMap::new();
    for rule in css.split('}') {
        let Some((selectors, declarations)) = rule.split_once('{') else {
            continue;
        };
        if !selector_list_contains_root(selectors, root_classes) {
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

fn selector_list_contains_root(selectors: &str, root_classes: &[String]) -> bool {
    split_css_selector_list(selectors).iter().any(|selector| {
        let selector = selector.trim();
        if selector == ":root" || selector == "html" || selector == "body" {
            return true;
        }
        if selector_targets_document_container(selector) {
            return true;
        }
        if let Some(class_name) = selector.strip_prefix('.') {
            return root_classes
                .iter()
                .any(|root_class| root_class == class_name);
        }
        if let Some(class_name) = selector.strip_prefix("html.") {
            return root_classes
                .iter()
                .any(|root_class| root_class == class_name);
        }
        false
    })
}

fn selector_targets_document_container(selector: &str) -> bool {
    let selector = selector.trim().to_ascii_lowercase();
    if selector.contains(char::is_whitespace)
        || selector.contains('>')
        || selector.contains('+')
        || selector.contains('~')
    {
        return false;
    }
    const ROOT_IDS: [&str; 5] = ["app", "root", "__next", "__nuxt", "svelte"];
    ROOT_IDS.iter().any(|id| {
        selector == format!("#{id}")
            || selector.starts_with(&format!("#{id}."))
            || selector.contains(&format!("[id=\"{id}\"]"))
            || selector.contains(&format!("[id='{id}']"))
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
        let Some(replacement) = variables
            .get(name.trim())
            .cloned()
            .or_else(|| fallback.map(|value| value.trim().to_owned()))
        else {
            break;
        };
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
    let mut matched_supported_media_type = false;
    for part in query.split("and") {
        let part = part.trim().trim_matches(|ch| ch == '(' || ch == ')').trim();
        if part.is_empty() {
            continue;
        }
        if matches!(part, "screen" | "only screen" | "all" | "only all") {
            matched_supported_media_type = true;
            continue;
        }
        if matches!(part, "print" | "only print") {
            return false;
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
    matched_any_constraint || matched_supported_media_type
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
        let value = resolve_css_vars(value.trim(), &variables);
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
        .filter(|rule| css_selector_fast_filter(&rule.selector, key))
        .filter(|rule| css_selector_matches(&rule.selector, key))
        .collect::<Vec<_>>();
    matches.sort_by_key(|rule| (css_selector_specificity(&rule.selector), rule.order));

    for rule in matches {
        merge_css_box_style(&mut out, &rule.style);
    }

    out
}

fn css_selector_fast_filter(selector: &CssSelector, key: &ElementStyleKey) -> bool {
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
    if let Some(nth_child) = selector.nth_child {
        let Some(child_index) = key.child_index else {
            return false;
        };
        if !nth_child_matches(nth_child, child_index) {
            return false;
        }
    }
    if let Some(nth_last_child) = selector.nth_last_child {
        if !nth_last_child_matches(nth_last_child, key) {
            return false;
        }
    }
    if selector
        .classes
        .iter()
        .any(|class| !key.classes.iter().any(|key_class| key_class == class))
    {
        return false;
    }
    if selector.attributes.iter().any(|attribute| {
        !key.attributes.iter().any(|key_attribute| {
            key_attribute == attribute || key_attribute_attribute_name(key_attribute) == *attribute
        })
    }) {
        return false;
    }
    if selector
        .attribute_selectors
        .iter()
        .any(|attribute| !css_attribute_selector_matches(attribute, &key.attributes))
    {
        return false;
    }
    if selector.requires_previous_sibling && key.previous_sibling.is_none() {
        return false;
    }
    true
}

pub fn parse_inline_box_style(declarations: &str) -> Option<CssBoxStyle> {
    parse_css_box_style_with_vars(declarations, &HashMap::new(), None)
}

fn css_selector_matches(selector: &CssSelector, key: &ElementStyleKey) -> bool {
    let current = SimpleCssSelector {
        tag: selector.tag.clone(),
        id: selector.id.clone(),
        classes: selector.classes.clone(),
        attributes: selector.attributes.clone(),
        attribute_selectors: selector.attribute_selectors.clone(),
        nth_child: selector.nth_child,
        nth_last_child: selector.nth_last_child,
        not_selectors: selector.not_selectors.clone(),
        is_selectors: selector.is_selectors.clone(),
        where_selectors: selector.where_selectors.clone(),
    };
    if !simple_css_selector_matches(&current, key) {
        return false;
    }
    if !ancestor_chain_matches(&selector.ancestor_chain, key.parent.as_deref()) {
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
        if selector.general_previous_sibling {
            if !previous_sibling_chain_matches(previous_selector, previous) {
                return false;
            }
        } else if !simple_css_selector_matches(previous_selector, previous) {
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
    if let Some(nth_child) = selector.nth_child {
        let Some(child_index) = key.child_index else {
            return false;
        };
        if !nth_child_matches(nth_child, child_index) {
            return false;
        }
    }
    if let Some(nth_last_child) = selector.nth_last_child {
        if !nth_last_child_matches(nth_last_child, key) {
            return false;
        }
    }
    selector
        .classes
        .iter()
        .all(|class| key.classes.iter().any(|key_class| key_class == class))
        && selector.attributes.iter().all(|attribute| {
            key.attributes.iter().any(|key_attribute| {
                key_attribute == attribute
                    || key_attribute_attribute_name(key_attribute) == *attribute
            })
        })
        && selector
            .attribute_selectors
            .iter()
            .all(|attribute| css_attribute_selector_matches(attribute, &key.attributes))
        && selector
            .not_selectors
            .iter()
            .all(|not_selector| !simple_css_selector_matches(not_selector, key))
        && (selector.is_selectors.is_empty()
            || selector
                .is_selectors
                .iter()
                .any(|is_selector| simple_css_selector_matches(is_selector, key)))
        && (selector.where_selectors.is_empty()
            || selector
                .where_selectors
                .iter()
                .any(|where_selector| simple_css_selector_matches(where_selector, key)))
}

fn previous_sibling_chain_matches(selector: &SimpleCssSelector, sibling: &ElementStyleKey) -> bool {
    let mut current = Some(sibling);
    while let Some(key) = current {
        if simple_css_selector_matches(selector, key) {
            return true;
        }
        current = key.previous_sibling.as_deref();
    }
    false
}

fn ancestor_chain_matches(
    selectors: &[SimpleCssSelector],
    parent: Option<&ElementStyleKey>,
) -> bool {
    let mut current = parent;
    for selector in selectors.iter().rev() {
        if simple_selector_is_plain_tag(selector, "tr")
            && current.is_some_and(|key| {
                matches!(
                    key.tag.as_str(),
                    "table" | "thead" | "tbody" | "tfoot" | "caption" | "colgroup"
                )
            })
        {
            continue;
        }
        let mut matched = None;
        while let Some(key) = current {
            if simple_css_selector_matches(selector, key) {
                matched = Some(key);
                break;
            }
            current = key.parent.as_deref();
        }
        let Some(key) = matched else {
            return false;
        };
        current = key.parent.as_deref();
    }
    true
}

fn simple_selector_is_plain_tag(selector: &SimpleCssSelector, tag: &str) -> bool {
    selector.tag.as_deref() == Some(tag)
        && selector.id.is_none()
        && selector.classes.is_empty()
        && selector.attributes.is_empty()
        && selector.attribute_selectors.is_empty()
        && selector.nth_child.is_none()
        && selector.nth_last_child.is_none()
        && selector.not_selectors.is_empty()
        && selector.is_selectors.is_empty()
        && selector.where_selectors.is_empty()
}

fn css_attribute_selector_matches(selector: &CssAttributeSelector, attributes: &[String]) -> bool {
    attributes.iter().any(|attribute| {
        let name = key_attribute_attribute_name(attribute);
        if name != selector.name {
            return false;
        }
        let Some(operator) = selector.operator else {
            return true;
        };
        let Some(expected) = selector.value.as_deref() else {
            return false;
        };
        let Some(actual) = key_attribute_value(attribute) else {
            return false;
        };
        match operator {
            CssAttributeOperator::Exact => actual == expected,
            CssAttributeOperator::Includes => {
                actual.split_whitespace().any(|part| part == expected)
            }
            CssAttributeOperator::DashMatch => {
                actual == expected
                    || actual
                        .strip_prefix(expected)
                        .is_some_and(|rest| rest.starts_with('-'))
            }
            CssAttributeOperator::Prefix => actual.starts_with(expected),
            CssAttributeOperator::Suffix => actual.ends_with(expected),
            CssAttributeOperator::Substring => actual.contains(expected),
        }
    })
}

fn key_attribute_attribute_name(attribute: &str) -> String {
    attribute
        .split_once('=')
        .map(|(name, _)| name)
        .unwrap_or(attribute)
        .trim()
        .to_ascii_lowercase()
}

fn key_attribute_value(attribute: &str) -> Option<String> {
    let (_, value) = attribute.split_once('=')?;
    Some(unquote_css_attribute_value(value.trim()))
}

fn nth_child_matches(nth_child: CssNthChild, child_index: usize) -> bool {
    let child_index = child_index as i32;
    if child_index <= 0 {
        return false;
    }
    match nth_child.step {
        0 => child_index == nth_child.offset,
        step if step > 0 => {
            child_index >= nth_child.offset && (child_index - nth_child.offset) % step == 0
        }
        step => child_index <= nth_child.offset && (nth_child.offset - child_index) % -step == 0,
    }
}

fn nth_last_child_matches(nth_last_child: CssNthChild, key: &ElementStyleKey) -> bool {
    let Some(child_index) = key.child_index else {
        return false;
    };
    let Some(child_count) = key.child_count else {
        return false;
    };
    if child_index == 0 || child_index > child_count {
        return false;
    }
    nth_child_matches(nth_last_child, child_count - child_index + 1)
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
        attribute_selectors: selector.attribute_selectors.clone(),
        nth_child: selector.nth_child,
        nth_last_child: selector.nth_last_child,
        not_selectors: selector.not_selectors.clone(),
        is_selectors: selector.is_selectors.clone(),
        where_selectors: selector.where_selectors.clone(),
    };
    simple_css_selector_specificity(&current)
        + selector
            .ancestor_chain
            .iter()
            .map(simple_css_selector_specificity)
            .sum::<usize>()
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
    let attribute_count = if selector.attribute_selectors.is_empty() {
        selector.attributes.len()
    } else {
        selector.attribute_selectors.len()
    };
    let pseudo_specificity = selector
        .not_selectors
        .iter()
        .chain(selector.is_selectors.iter())
        .map(simple_css_selector_specificity)
        .max()
        .unwrap_or_default();
    selector.id.iter().count() * 100
        + (selector.classes.len()
            + attribute_count
            + selector.nth_child.iter().count()
            + selector.nth_last_child.iter().count())
            * 10
        + selector.tag.iter().count()
        + pseudo_specificity
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
        target.margin_top = None;
        target.margin_right = None;
        target.margin_bottom = None;
        target.margin_left = None;
        target.margin_auto = source.margin_auto;
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
        target.padding_top = None;
        target.padding_right = None;
        target.padding_bottom = None;
        target.padding_left = None;
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
    if source.list_style_type.is_some() {
        target.list_style_type = source.list_style_type;
    }
    if source.flex_grow.is_some() {
        target.flex_grow = source.flex_grow;
    }
    if source.flex_shrink.is_some() {
        target.flex_shrink = source.flex_shrink;
    }
    if source.flex_basis.is_some() {
        target.flex_basis = source.flex_basis;
    }
    if source.flex_direction.is_some() {
        target.flex_direction = source.flex_direction;
    }
    if source.flex_wrap.is_some() {
        target.flex_wrap = source.flex_wrap;
    }
    if source.justify_content.is_some() {
        target.justify_content = source.justify_content;
    }
    if source.align_items.is_some() {
        target.align_items = source.align_items;
    }
    if source.align_self.is_some() {
        target.align_self = source.align_self;
    }
    if source.justify_items.is_some() {
        target.justify_items = source.justify_items;
    }
    if source.align_content.is_some() {
        target.align_content = source.align_content;
    }
    if source.grid_template_columns.is_some() {
        target.grid_template_columns = source.grid_template_columns;
    }
    if source.grid_template_column_tracks.is_some() {
        target.grid_template_column_tracks = source.grid_template_column_tracks.clone();
    }
    if source.grid_auto_repeat_min_column_width.is_some() {
        target.grid_auto_repeat_min_column_width = source.grid_auto_repeat_min_column_width;
    }
    if source.grid_template_rows.is_some() {
        target.grid_template_rows = source.grid_template_rows.clone();
    }
    if source.grid_auto_rows.is_some() {
        target.grid_auto_rows = source.grid_auto_rows;
    }
    if source.grid_template_areas.is_some() {
        target.grid_template_areas = source.grid_template_areas.clone();
    }
    if source.grid_area.is_some() {
        target.grid_area = source.grid_area.clone();
    }
    if source.grid_column_start.is_some() {
        target.grid_column_start = source.grid_column_start;
    }
    if source.grid_column_end.is_some() {
        target.grid_column_end = source.grid_column_end;
    }
    if source.grid_column_span.is_some() {
        target.grid_column_span = Some(source.grid_column_span.unwrap_or(1).max(1));
    }
    if source.grid_row_span.is_some() {
        target.grid_row_span = Some(source.grid_row_span.unwrap_or(1).max(1));
    }
    if source.gap.is_some() {
        target.gap = source.gap;
    }
    if source.visibility_visible.is_some() {
        target.visibility_visible = source.visibility_visible;
    }
    if source.opacity.is_some() {
        target.opacity = source.opacity;
    }
    if source.overflow_hidden.is_some() {
        target.overflow_hidden = source.overflow_hidden;
    }
    if source.position.is_some() {
        target.position = source.position;
    }
    if source.float.is_some() {
        target.float = source.float;
    }
    if source.clear.is_some() {
        target.clear = source.clear;
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
    if source.transform.is_some() {
        target.transform = source.transform;
    }
    if source.object_fit.is_some() {
        target.object_fit = source.object_fit;
    }
    if source.box_sizing_border_box.is_some() {
        target.box_sizing_border_box = source.box_sizing_border_box;
    }
}

fn parse_css_selector(selector: &str) -> Option<CssSelector> {
    let selector = normalize_css_selector(selector)?;
    if selector.is_empty() {
        return None;
    }
    if selector == "*" {
        return Some(CssSelector::default());
    }

    if let Some((left, right)) = split_selector_once(&selector, '+') {
        let right = parse_simple_css_selector(right)?;
        let (parent, previous_sibling) = parse_previous_sibling_selector(left)?;
        return Some(CssSelector {
            tag: right.tag,
            id: right.id,
            classes: right.classes,
            attributes: right.attributes,
            attribute_selectors: right.attribute_selectors,
            nth_child: right.nth_child,
            nth_last_child: right.nth_last_child,
            not_selectors: right.not_selectors,
            is_selectors: right.is_selectors,
            where_selectors: right.where_selectors,
            ancestor_chain: Vec::new(),
            ancestor: None,
            parent,
            previous_sibling: Some(previous_sibling),
            requires_previous_sibling: true,
            general_previous_sibling: false,
        });
    }

    if let Some((left, right)) = split_selector_once(&selector, '~') {
        let right = parse_simple_css_selector(right)?;
        let (parent, previous_sibling) = parse_previous_sibling_selector(left)?;
        return Some(CssSelector {
            tag: right.tag,
            id: right.id,
            classes: right.classes,
            attributes: right.attributes,
            attribute_selectors: right.attribute_selectors,
            nth_child: right.nth_child,
            nth_last_child: right.nth_last_child,
            not_selectors: right.not_selectors,
            is_selectors: right.is_selectors,
            where_selectors: right.where_selectors,
            ancestor_chain: Vec::new(),
            ancestor: None,
            parent,
            previous_sibling: Some(previous_sibling),
            requires_previous_sibling: true,
            general_previous_sibling: true,
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
            attribute_selectors: child.attribute_selectors,
            nth_child: child.nth_child,
            nth_last_child: child.nth_last_child,
            not_selectors: child.not_selectors,
            is_selectors: child.is_selectors,
            where_selectors: child.where_selectors,
            ancestor_chain: Vec::new(),
            ancestor: None,
            parent: Some(parent),
            previous_sibling: None,
            requires_previous_sibling: false,
            general_previous_sibling: false,
        });
    }

    if selector_has_top_level_whitespace(&selector) {
        let (ancestors, descendant) = split_descendant_selector(&selector)?;
        let descendant = parse_simple_css_selector(descendant)?;
        let ancestor_chain = ancestors
            .iter()
            .map(|ancestor| parse_simple_css_selector(ancestor))
            .collect::<Option<Vec<_>>>()?;
        return Some(CssSelector {
            tag: descendant.tag,
            id: descendant.id,
            classes: descendant.classes,
            attributes: descendant.attributes,
            attribute_selectors: descendant.attribute_selectors,
            nth_child: descendant.nth_child,
            nth_last_child: descendant.nth_last_child,
            not_selectors: descendant.not_selectors,
            is_selectors: descendant.is_selectors,
            where_selectors: descendant.where_selectors,
            ancestor_chain,
            ancestor: None,
            parent: None,
            previous_sibling: None,
            requires_previous_sibling: false,
            general_previous_sibling: false,
        });
    }

    let simple = parse_simple_css_selector(&selector)?;
    if simple.tag.is_none()
        && simple.id.is_none()
        && simple.classes.is_empty()
        && simple.attributes.is_empty()
        && simple.attribute_selectors.is_empty()
        && simple.nth_child.is_none()
        && simple.nth_last_child.is_none()
        && simple.not_selectors.is_empty()
        && simple.is_selectors.is_empty()
        && simple.where_selectors.is_empty()
    {
        None
    } else {
        Some(CssSelector {
            tag: simple.tag,
            id: simple.id,
            classes: simple.classes,
            attributes: simple.attributes,
            attribute_selectors: simple.attribute_selectors,
            nth_child: simple.nth_child,
            nth_last_child: simple.nth_last_child,
            not_selectors: simple.not_selectors,
            is_selectors: simple.is_selectors,
            where_selectors: simple.where_selectors,
            ancestor_chain: Vec::new(),
            ancestor: None,
            parent: None,
            previous_sibling: None,
            requires_previous_sibling: false,
            general_previous_sibling: false,
        })
    }
}

fn split_descendant_selector(selector: &str) -> Option<(Vec<&str>, &str)> {
    let mut parts = split_selector_by_top_level_whitespace(selector);
    if parts.len() < 2 {
        return None;
    }
    let descendant = parts.pop()?;
    if find_top_level_char(descendant, '>').is_some()
        || find_top_level_char(descendant, '+').is_some()
        || find_top_level_char(descendant, '~').is_some()
        || parts.iter().any(|ancestor| {
            find_top_level_char(ancestor, '>').is_some()
                || find_top_level_char(ancestor, '+').is_some()
                || find_top_level_char(ancestor, '~').is_some()
        })
    {
        None
    } else {
        Some((parts, descendant))
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
        || selector_has_top_level_whitespace(selector)
        || find_top_level_char(selector, '>').is_some()
        || find_top_level_char(selector, '+').is_some()
        || find_top_level_char(selector, '~').is_some()
    {
        return None;
    }

    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();
    let (selector, attribute_selectors) = strip_simple_selector_attributes(selector)?;
    let attributes = attribute_selectors
        .iter()
        .filter(|attribute| attribute.operator.is_none())
        .map(|attribute| attribute.name.clone())
        .collect::<Vec<_>>();
    let (selector, pseudo_classes) = strip_simple_selector_pseudo_classes(&selector)?;
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
        attribute_selectors,
        nth_child: pseudo_classes.nth_child,
        nth_last_child: pseudo_classes.nth_last_child,
        not_selectors: pseudo_classes.not_selectors,
        is_selectors: pseudo_classes.is_selectors,
        where_selectors: pseudo_classes.where_selectors,
    })
}

fn strip_simple_selector_attributes(selector: &str) -> Option<(String, Vec<CssAttributeSelector>)> {
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
        let attribute = parse_css_attribute_selector(&raw)?;
        attributes.push(attribute);
    }
    Some((simple, attributes))
}

#[derive(Clone, Debug, Default)]
struct CssSimplePseudoClasses {
    nth_child: Option<CssNthChild>,
    nth_last_child: Option<CssNthChild>,
    not_selectors: Vec<SimpleCssSelector>,
    is_selectors: Vec<SimpleCssSelector>,
    where_selectors: Vec<SimpleCssSelector>,
}

fn strip_simple_selector_pseudo_classes(
    selector: &str,
) -> Option<(String, CssSimplePseudoClasses)> {
    let mut simple = String::new();
    let mut pseudo_classes = CssSimplePseudoClasses::default();
    let mut index = 0usize;
    while index < selector.len() {
        let rest = &selector[index..];
        if rest.starts_with(":nth-child(") {
            let open_paren = index + ":nth-child".len();
            let end = find_function_end(selector, open_paren)?;
            pseudo_classes.nth_child =
                Some(parse_nth_child_formula(&selector[open_paren + 1..end])?);
            index = end + 1;
            continue;
        }
        if rest.starts_with(":nth-last-child(") {
            let open_paren = index + ":nth-last-child".len();
            let end = find_function_end(selector, open_paren)?;
            pseudo_classes.nth_last_child =
                Some(parse_nth_child_formula(&selector[open_paren + 1..end])?);
            index = end + 1;
            continue;
        }
        if let Some((name, offset)) = [(":not", 4usize), (":is", 3usize), (":where", 6usize)]
            .iter()
            .find_map(|(name, len)| {
                rest.starts_with(&format!("{name}("))
                    .then_some((*name, *len))
            })
        {
            let open_paren = index + offset;
            let end = find_function_end(selector, open_paren)?;
            let inner = &selector[open_paren + 1..end];
            let selectors = parse_simple_selector_function_args(inner)?;
            match name {
                ":not" => pseudo_classes.not_selectors.extend(selectors),
                ":is" => pseudo_classes.is_selectors.extend(selectors),
                ":where" => pseudo_classes.where_selectors.extend(selectors),
                _ => {}
            }
            index = end + 1;
            continue;
        }

        let ch = rest.chars().next()?;
        if ch == ':' {
            return None;
        }
        simple.push(ch);
        index += ch.len_utf8();
    }
    Some((simple, pseudo_classes))
}

fn parse_simple_selector_function_args(inner: &str) -> Option<Vec<SimpleCssSelector>> {
    let selectors = split_css_selector_list(inner);
    if selectors.is_empty() {
        return None;
    }
    selectors
        .iter()
        .map(|selector| parse_simple_css_selector(selector))
        .collect()
}

fn parse_css_attribute_selector(raw: &str) -> Option<CssAttributeSelector> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    for (operator_text, operator) in [
        ("~=", CssAttributeOperator::Includes),
        ("|=", CssAttributeOperator::DashMatch),
        ("^=", CssAttributeOperator::Prefix),
        ("$=", CssAttributeOperator::Suffix),
        ("*=", CssAttributeOperator::Substring),
        ("=", CssAttributeOperator::Exact),
    ] {
        if let Some(index) = raw.find(operator_text) {
            let name = raw[..index].trim().to_ascii_lowercase();
            let value = unquote_css_attribute_value(raw[index + operator_text.len()..].trim());
            return (!name.is_empty()).then_some(CssAttributeSelector {
                name,
                operator: Some(operator),
                value: Some(value),
            });
        }
    }

    let name = raw.split_whitespace().next()?.trim().to_ascii_lowercase();
    (!name.is_empty()).then_some(CssAttributeSelector {
        name,
        operator: None,
        value: None,
    })
}

fn unquote_css_attribute_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 {
        let mut chars = value.chars();
        let first = chars.next().unwrap_or_default();
        let last = value.chars().last().unwrap_or_default();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return value[first.len_utf8()..value.len() - last.len_utf8()].to_owned();
        }
    }
    value.to_owned()
}

fn parse_nth_child_formula(formula: &str) -> Option<CssNthChild> {
    let formula = formula
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    match formula.as_str() {
        "odd" => {
            return Some(CssNthChild { step: 2, offset: 1 });
        }
        "even" => {
            return Some(CssNthChild { step: 2, offset: 0 });
        }
        _ => {}
    }

    if let Ok(offset) = formula.parse::<i32>() {
        return (offset > 0).then_some(CssNthChild { step: 0, offset });
    }

    let n_index = formula.find('n')?;
    let step = match &formula[..n_index] {
        "" | "+" => 1,
        "-" => -1,
        value => value.parse::<i32>().ok()?,
    };
    let offset = match &formula[n_index + 1..] {
        "" => 0,
        value => value.parse::<i32>().ok()?,
    };
    (step != 0).then_some(CssNthChild { step, offset })
}

fn normalize_css_selector(selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    if selector.contains("::")
        || selector.contains(":before")
        || selector.contains(":after")
        || selector_contains_dynamic_pseudo_class(selector)
    {
        return None;
    }

    Some(selector.to_owned())
}

fn selector_contains_dynamic_pseudo_class(selector: &str) -> bool {
    [
        ":active",
        ":checked",
        ":disabled",
        ":enabled",
        ":focus",
        ":focus-visible",
        ":focus-within",
        ":hover",
        ":invalid",
        ":optional",
        ":placeholder-shown",
        ":required",
        ":target",
        ":valid",
        ":visited",
    ]
    .iter()
    .any(|pseudo| selector.contains(pseudo))
}

fn split_selector_once(selector: &str, combinator: char) -> Option<(&str, &str)> {
    let index = find_top_level_char(selector, combinator)?;
    let left = selector[..index].trim();
    let right = selector[index + combinator.len_utf8()..].trim();
    if left.is_empty()
        || right.is_empty()
        || find_top_level_char(right, '>').is_some()
        || find_top_level_char(right, '+').is_some()
        || find_top_level_char(right, '~').is_some()
    {
        None
    } else {
        Some((left, right))
    }
}

fn split_css_selector_list(selectors: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part_start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (index, ch) in selectors.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' if bracket_depth == 0 => paren_depth += 1,
            ')' if bracket_depth == 0 => paren_depth = paren_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 => {
                let part = selectors[part_start..index].trim();
                if !part.is_empty() {
                    parts.push(part.to_owned());
                }
                part_start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let part = selectors[part_start..].trim();
    if !part.is_empty() {
        parts.push(part.to_owned());
    }
    parts
}

fn find_top_level_char(selector: &str, target: char) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (index, ch) in selector.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' if bracket_depth == 0 => paren_depth += 1,
            ')' if bracket_depth == 0 => paren_depth = paren_depth.saturating_sub(1),
            ch if ch == target && paren_depth == 0 && bracket_depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn selector_has_top_level_whitespace(selector: &str) -> bool {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for ch in selector.chars() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' if bracket_depth == 0 => paren_depth += 1,
            ')' if bracket_depth == 0 => paren_depth = paren_depth.saturating_sub(1),
            ch if ch.is_whitespace() && paren_depth == 0 && bracket_depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn split_selector_by_top_level_whitespace(selector: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut start = None;
    for (index, ch) in selector.char_indices() {
        match ch {
            '[' => {
                bracket_depth += 1;
                start.get_or_insert(index);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                start.get_or_insert(index);
            }
            '(' if bracket_depth == 0 => {
                paren_depth += 1;
                start.get_or_insert(index);
            }
            ')' if bracket_depth == 0 => {
                paren_depth = paren_depth.saturating_sub(1);
                start.get_or_insert(index);
            }
            ch if ch.is_whitespace() && paren_depth == 0 && bracket_depth == 0 => {
                if let Some(part_start) = start.take() {
                    parts.push(&selector[part_start..index]);
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }
    if let Some(part_start) = start {
        parts.push(&selector[part_start..]);
    }
    parts
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
    viewport_width: Option<f32>,
) -> Option<CssBoxStyle> {
    let mut style = CssBoxStyle::default();
    let mut seen = false;
    let variables = css_variables_for_declarations(declarations, variables);

    for declaration in declarations.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        if property.starts_with("--") {
            continue;
        }
        let value = resolve_css_vars(value.trim(), &variables);
        let value = value.as_str();
        match property {
            "display" => {
                style.display = parse_display(value);
                seen |= style.display.is_some();
            }
            "float" => {
                style.float = parse_float(value);
                seen |= style.float.is_some();
            }
            "clear" => {
                style.clear = parse_clear(value);
                seen |= style.clear.is_some();
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
            "margin-inline" => {
                if apply_logical_margin_pair(value, true, &mut style) {
                    seen = true;
                }
            }
            "margin-block" => {
                if apply_logical_margin_pair(value, false, &mut style) {
                    seen = true;
                }
            }
            "margin-inline-start" => {
                if apply_logical_margin_side(value, "left", &mut style) {
                    seen = true;
                }
            }
            "margin-inline-end" => {
                if apply_logical_margin_side(value, "right", &mut style) {
                    seen = true;
                }
            }
            "margin-block-start" => {
                if apply_logical_margin_side(value, "top", &mut style) {
                    seen = true;
                }
            }
            "margin-block-end" => {
                if apply_logical_margin_side(value, "bottom", &mut style) {
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
            "padding-inline" => {
                if apply_logical_padding_pair(value, true, viewport_width, &mut style) {
                    seen = true;
                }
            }
            "padding-block" => {
                if apply_logical_padding_pair(value, false, viewport_width, &mut style) {
                    seen = true;
                }
            }
            "padding-inline-start" => {
                if let Some(px) = parse_spacing_px(value, viewport_width) {
                    style.padding_left = Some(px);
                    seen = true;
                }
            }
            "padding-inline-end" => {
                if let Some(px) = parse_spacing_px(value, viewport_width) {
                    style.padding_right = Some(px);
                    seen = true;
                }
            }
            "padding-block-start" => {
                if let Some(px) = parse_spacing_px(value, viewport_width) {
                    style.padding_top = Some(px);
                    seen = true;
                }
            }
            "padding-block-end" => {
                if let Some(px) = parse_spacing_px(value, viewport_width) {
                    style.padding_bottom = Some(px);
                    seen = true;
                }
            }
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                if let Some(px) = parse_spacing_px(value, viewport_width) {
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
            "width" | "inline-size" => {
                style.width = parse_css_length(value);
                seen |= style.width.is_some();
            }
            "max-width" | "max-inline-size" => {
                style.max_width = parse_css_length(value);
                seen |= style.max_width.is_some();
            }
            "min-width" | "min-inline-size" => {
                style.min_width = parse_css_length(value);
                seen |= style.min_width.is_some();
            }
            "height" | "block-size" => {
                style.height = parse_css_length(value);
                seen |= style.height.is_some();
            }
            "min-height" | "min-block-size" => {
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
            "list-style" | "list-style-type" => {
                style.list_style_type = parse_list_style_type(value);
                seen |= style.list_style_type.is_some();
            }
            "flex" => {
                if let Some((grow, shrink, basis)) = parse_flex_shorthand(value) {
                    style.flex_grow = Some(grow);
                    style.flex_shrink = Some(shrink);
                    style.flex_basis = basis;
                    seen = true;
                }
            }
            "flex-grow" => {
                style.flex_grow = parse_flex_factor(value);
                seen |= style.flex_grow.is_some();
            }
            "flex-shrink" => {
                style.flex_shrink = parse_flex_factor(value);
                seen |= style.flex_shrink.is_some();
            }
            "flex-basis" => {
                style.flex_basis = parse_css_length(value);
                seen |= style.flex_basis.is_some();
            }
            "flex-direction" => {
                style.flex_direction = match value {
                    "row" | "row-reverse" => Some(CssFlexDirection::Row),
                    "column" | "column-reverse" => Some(CssFlexDirection::Column),
                    _ => None,
                };
                seen |= style.flex_direction.is_some();
            }
            "flex-wrap" => {
                style.flex_wrap = parse_flex_wrap(value);
                seen |= style.flex_wrap.is_some();
            }
            "flex-flow" => {
                for token in split_css_value_list(value) {
                    let token = token.trim();
                    if style.flex_direction.is_none() {
                        style.flex_direction = match token {
                            "row" | "row-reverse" => Some(CssFlexDirection::Row),
                            "column" | "column-reverse" => Some(CssFlexDirection::Column),
                            _ => None,
                        };
                    }
                    if style.flex_wrap.is_none() {
                        style.flex_wrap = parse_flex_wrap(token);
                    }
                }
                seen |= style.flex_direction.is_some() || style.flex_wrap.is_some();
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
            "align-self" => {
                style.align_self = match value {
                    "auto" | "normal" => None,
                    "center" => Some(CssAlignItems::Center),
                    "flex-start" | "start" | "self-start" => Some(CssAlignItems::FlexStart),
                    "stretch" => Some(CssAlignItems::Stretch),
                    _ => None,
                };
                seen |= value.eq_ignore_ascii_case("auto") || style.align_self.is_some();
            }
            "justify-items" => {
                style.justify_items = match value {
                    "center" => Some(CssJustifyContent::Center),
                    "start" | "flex-start" | "left" | "normal" => {
                        Some(CssJustifyContent::FlexStart)
                    }
                    "stretch" => Some(CssJustifyContent::FlexStart),
                    _ => None,
                };
                seen |= style.justify_items.is_some();
            }
            "align-content" => {
                style.align_content = match value {
                    "center" => Some(CssAlignItems::Center),
                    "start" | "flex-start" | "normal" => Some(CssAlignItems::FlexStart),
                    "stretch" => Some(CssAlignItems::Stretch),
                    _ => None,
                };
                seen |= style.align_content.is_some();
            }
            "place-items" => {
                let values = split_css_value_list(value);
                if values
                    .iter()
                    .any(|value| value.trim().eq_ignore_ascii_case("center"))
                {
                    style.align_items = Some(CssAlignItems::Center);
                    style.justify_items = Some(CssJustifyContent::Center);
                    seen = true;
                }
            }
            "grid-template-columns" => {
                style.grid_template_columns = parse_grid_template_columns(value);
                style.grid_template_column_tracks = parse_grid_template_column_tracks(value);
                style.grid_auto_repeat_min_column_width =
                    parse_grid_auto_repeat_min_column_width(value);
                seen |= style.grid_template_columns.is_some()
                    || style.grid_template_column_tracks.is_some()
                    || style.grid_auto_repeat_min_column_width.is_some();
            }
            "grid-template-rows" => {
                style.grid_template_rows = parse_grid_template_rows(value);
                seen |= style.grid_template_rows.is_some();
            }
            "grid-auto-rows" => {
                style.grid_auto_rows = parse_grid_auto_rows(value);
                seen |= style.grid_auto_rows.is_some();
            }
            "grid-template-areas" => {
                style.grid_template_areas = parse_grid_template_areas(value);
                seen |= style.grid_template_areas.is_some();
            }
            "grid-area" => {
                style.grid_area = parse_grid_area(value);
                seen |= style.grid_area.is_some();
            }
            "grid-column" => {
                let placement = parse_grid_column_placement(value);
                style.grid_column_start = placement.start;
                style.grid_column_end = placement.end;
                style.grid_column_span = placement.span.or_else(|| parse_grid_line_span(value));
                seen |= style.grid_column_start.is_some()
                    || style.grid_column_end.is_some()
                    || style.grid_column_span.is_some();
            }
            "grid-column-start" => {
                style.grid_column_start = parse_grid_line_number(value);
                seen |= style.grid_column_start.is_some();
            }
            "grid-column-end" => {
                style.grid_column_span = parse_grid_line_span(value);
                if style.grid_column_span.is_none() {
                    style.grid_column_end = parse_grid_line_number(value);
                }
                seen |= style.grid_column_end.is_some() || style.grid_column_span.is_some();
            }
            "grid-row" | "grid-row-end" => {
                style.grid_row_span = parse_grid_line_span(value);
                seen |= style.grid_row_span.is_some();
            }
            "grid-template" => {
                style.grid_template_rows = parse_grid_template_shorthand_rows(value);
                style.grid_template_areas = parse_grid_template_shorthand_areas(value);
                style.grid_template_columns = parse_grid_template_shorthand_columns(value);
                style.grid_template_column_tracks =
                    parse_grid_template_shorthand_column_tracks(value);
                style.grid_auto_repeat_min_column_width =
                    parse_grid_template_shorthand_auto_repeat_min_column_width(value);
                seen |= style.grid_template_rows.is_some()
                    || style.grid_template_areas.is_some()
                    || style.grid_template_columns.is_some()
                    || style.grid_template_column_tracks.is_some()
                    || style.grid_auto_repeat_min_column_width.is_some();
            }
            "gap" | "row-gap" | "column-gap" => {
                style.gap = parse_gap(value);
                seen |= style.gap.is_some();
            }
            "visibility" => {
                style.visibility_visible = match value {
                    "visible" => Some(true),
                    "hidden" | "collapse" => Some(false),
                    _ => None,
                };
                seen |= style.visibility_visible.is_some();
            }
            "opacity" => {
                style.opacity = value
                    .parse::<f32>()
                    .ok()
                    .map(|opacity| opacity.clamp(0.0, 1.0));
                seen |= style.opacity.is_some();
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
                if let Some((edges, sides)) = parse_inset_edges(value, viewport_width) {
                    style.inset = Some(edges);
                    style.inset_sides = sides;
                    seen = true;
                }
            }
            "inset-inline" => {
                if apply_logical_inset_pair(value, true, viewport_width, &mut style) {
                    seen = true;
                }
            }
            "inset-block" => {
                if apply_logical_inset_pair(value, false, viewport_width, &mut style) {
                    seen = true;
                }
            }
            "inset-inline-start" => {
                if apply_logical_inset_side(value, "left", viewport_width, &mut style) {
                    seen = true;
                }
            }
            "inset-inline-end" => {
                if apply_logical_inset_side(value, "right", viewport_width, &mut style) {
                    seen = true;
                }
            }
            "inset-block-start" => {
                if apply_logical_inset_side(value, "top", viewport_width, &mut style) {
                    seen = true;
                }
            }
            "inset-block-end" => {
                if apply_logical_inset_side(value, "bottom", viewport_width, &mut style) {
                    seen = true;
                }
            }
            "top" | "right" | "bottom" | "left" => {
                let mut edges = style.inset.unwrap_or_default();
                if let Some(px) = parse_inset_value(value, viewport_width) {
                    set_edge(&mut edges, property, px);
                    set_inset_side(&mut style.inset_sides, property, px);
                    style.inset = Some(edges);
                    seen = true;
                }
            }
            "transform" => {
                style.transform = parse_css_transform(value);
                seen |= style.transform.is_some();
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
            "box-sizing" => {
                style.box_sizing_border_box = match value {
                    "border-box" => Some(true),
                    "content-box" => Some(false),
                    _ => None,
                };
                seen |= style.box_sizing_border_box.is_some();
            }
            _ => {}
        }
    }

    seen.then_some(style)
}

fn apply_logical_margin_pair(value: &str, inline_axis: bool, style: &mut CssBoxStyle) -> bool {
    let values = split_css_value_list(value);
    let (start, end) = match values.as_slice() {
        [both] => (both.as_str(), both.as_str()),
        [start, end, ..] => (start.as_str(), end.as_str()),
        _ => return false,
    };
    if inline_axis {
        apply_logical_margin_side(start, "left", style)
            & apply_logical_margin_side(end, "right", style)
    } else {
        apply_logical_margin_side(start, "top", style)
            & apply_logical_margin_side(end, "bottom", style)
    }
}

fn apply_logical_margin_side(value: &str, side: &str, style: &mut CssBoxStyle) -> bool {
    let value = value.trim();
    let (px, auto) = if value.eq_ignore_ascii_case("auto") {
        (0.0, true)
    } else if let Some(px) = parse_px(value) {
        (px, false)
    } else {
        return false;
    };
    match side {
        "top" => {
            style.margin_top = Some(px);
            style.margin_auto.top = Some(auto);
        }
        "right" => {
            style.margin_right = Some(px);
            style.margin_auto.right = Some(auto);
        }
        "bottom" => {
            style.margin_bottom = Some(px);
            style.margin_auto.bottom = Some(auto);
        }
        "left" => {
            style.margin_left = Some(px);
            style.margin_auto.left = Some(auto);
        }
        _ => return false,
    }
    true
}

fn apply_logical_padding_pair(
    value: &str,
    inline_axis: bool,
    viewport_width: Option<f32>,
    style: &mut CssBoxStyle,
) -> bool {
    let values = split_css_value_list(value);
    let (start, end) = match values.as_slice() {
        [both] => (both.as_str(), both.as_str()),
        [start, end, ..] => (start.as_str(), end.as_str()),
        _ => return false,
    };
    let Some(start_px) = parse_spacing_px(start, viewport_width) else {
        return false;
    };
    let Some(end_px) = parse_spacing_px(end, viewport_width) else {
        return false;
    };
    if inline_axis {
        style.padding_left = Some(start_px);
        style.padding_right = Some(end_px);
    } else {
        style.padding_top = Some(start_px);
        style.padding_bottom = Some(end_px);
    }
    true
}

fn parse_spacing_px(value: &str, viewport_width: Option<f32>) -> Option<f32> {
    parse_px(value).or_else(|| {
        parse_css_length(value)
            .map(|length| css_length_px(length, viewport_width.unwrap_or(1280.0)))
    })
}

fn apply_logical_inset_pair(
    value: &str,
    inline_axis: bool,
    viewport_width: Option<f32>,
    style: &mut CssBoxStyle,
) -> bool {
    let values = split_css_value_list(value);
    let (start, end) = match values.as_slice() {
        [both] => (both.as_str(), both.as_str()),
        [start, end, ..] => (start.as_str(), end.as_str()),
        _ => return false,
    };
    if inline_axis {
        apply_logical_inset_side(start, "left", viewport_width, style)
            & apply_logical_inset_side(end, "right", viewport_width, style)
    } else {
        apply_logical_inset_side(start, "top", viewport_width, style)
            & apply_logical_inset_side(end, "bottom", viewport_width, style)
    }
}

fn apply_logical_inset_side(
    value: &str,
    side: &str,
    viewport_width: Option<f32>,
    style: &mut CssBoxStyle,
) -> bool {
    if value.trim().eq_ignore_ascii_case("auto") {
        return true;
    }
    let Some(px) = parse_inset_value(value, viewport_width) else {
        return false;
    };
    let mut edges = style.inset.unwrap_or_default();
    set_edge(&mut edges, side, px);
    set_inset_side(&mut style.inset_sides, side, px);
    style.inset = Some(edges);
    true
}

fn css_variables_for_declarations<'a>(
    declarations: &str,
    inherited: &'a HashMap<String, String>,
) -> HashMap<String, String> {
    let mut variables = inherited.clone();
    for declaration in declarations.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        if property.starts_with("--") {
            let value = resolve_css_vars(value.trim(), &variables);
            variables.insert(property.to_owned(), value);
        }
    }
    variables
}

fn parse_display(value: &str) -> Option<CssDisplay> {
    match value.split_whitespace().next().unwrap_or(value) {
        "none" => Some(CssDisplay::None),
        "contents" => Some(CssDisplay::Contents),
        "block" => Some(CssDisplay::Block),
        "inline" => Some(CssDisplay::Inline),
        "inline-block" => Some(CssDisplay::InlineBlock),
        "inline-flex" => Some(CssDisplay::Flex),
        "flex" => Some(CssDisplay::Flex),
        "grid" | "inline-grid" => Some(CssDisplay::Grid),
        "table" | "inline-table" | "table-row" | "table-cell" | "table-caption"
        | "table-row-group" | "table-header-group" | "table-footer-group" | "table-column"
        | "table-column-group" => Some(CssDisplay::Table),
        "list-item" => Some(CssDisplay::ListItem),
        _ => None,
    }
}

fn parse_list_style_type(value: &str) -> Option<CssListStyleType> {
    for token in split_css_value_list(value) {
        match token.trim().to_ascii_lowercase().as_str() {
            "none" => return Some(CssListStyleType::None),
            "disc" | "circle" | "square" => return Some(CssListStyleType::Disc),
            "decimal" | "decimal-leading-zero" => return Some(CssListStyleType::Decimal),
            _ => {}
        }
    }
    None
}

fn parse_float(value: &str) -> Option<CssFloat> {
    match value.trim() {
        "none" => Some(CssFloat::None),
        "left" | "inline-start" => Some(CssFloat::Left),
        "right" | "inline-end" => Some(CssFloat::Right),
        _ => None,
    }
}

fn parse_clear(value: &str) -> Option<CssClear> {
    match value.trim() {
        "none" => Some(CssClear::None),
        "both" => Some(CssClear::Both),
        _ => None,
    }
}

fn parse_flex_factor(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok().map(|value| value.max(0.0))
}

fn parse_flex_shorthand(value: &str) -> Option<(f32, f32, Option<CssLength>)> {
    let value = value.trim();
    match value {
        "none" => return Some((0.0, 0.0, Some(CssLength::Auto))),
        "auto" => return Some((1.0, 1.0, Some(CssLength::Auto))),
        "initial" => return Some((0.0, 1.0, Some(CssLength::Auto))),
        _ => {}
    }

    let tokens = split_css_value_list(value);
    if tokens.is_empty() {
        return None;
    }

    let mut numbers = Vec::new();
    let mut basis = None;
    for token in tokens {
        let token = token.trim();
        if let Some(number) = parse_flex_factor(token) {
            numbers.push(number);
            continue;
        }
        if basis.is_none() {
            basis = parse_css_length(token);
        }
    }

    if numbers.is_empty() && basis.is_none() {
        return None;
    }

    let grow = numbers.first().copied().unwrap_or(1.0);
    let shrink = numbers.get(1).copied().unwrap_or(1.0);
    let basis = basis.or_else(|| (numbers.len() == 1).then_some(CssLength::Percent(0.0)));
    Some((grow, shrink, basis))
}

fn parse_flex_wrap(value: &str) -> Option<CssFlexWrap> {
    match value.trim() {
        "wrap" | "wrap-reverse" => Some(CssFlexWrap::Wrap),
        "nowrap" => Some(CssFlexWrap::NoWrap),
        _ => None,
    }
}

fn parse_grid_template_columns(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || value == "none" {
        return None;
    }
    if let Some(repeat_start) = value.find("repeat(") {
        let inner = &value[repeat_start + "repeat(".len()..];
        let (count, track) = split_css_function_args(inner);
        let count = count.trim();
        if count.eq_ignore_ascii_case("auto-fit") || count.eq_ignore_ascii_case("auto-fill") {
            return parse_grid_auto_repeat_column_count(track?.trim());
        }
        let count = count.parse::<usize>().ok()?;
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

fn parse_grid_template_column_tracks(value: &str) -> Option<Vec<CssLength>> {
    parse_grid_template_tracks(value, false)
}

fn parse_grid_template_tracks(value: &str, allow_auto_repeat: bool) -> Option<Vec<CssLength>> {
    let value = value.trim();
    if value.is_empty() || value == "none" {
        return None;
    }
    let mut tracks = Vec::new();
    for token in split_css_value_list(value) {
        let token = token.trim();
        if token.is_empty() || token == "/" || token.eq_ignore_ascii_case("subgrid") {
            continue;
        }
        if let Some(repeated) =
            parse_grid_template_repeat_tracks_with_auto(token, allow_auto_repeat)
        {
            tracks.extend(repeated);
            continue;
        }
        tracks.push(parse_grid_template_track_size(token).unwrap_or(CssLength::Auto));
    }
    (!tracks.is_empty()).then_some(tracks)
}

fn parse_grid_template_track_size(token: &str) -> Option<CssLength> {
    let token = token.trim();
    if let Some(track) = parse_grid_minmax_preferred_track(token) {
        return Some(track);
    }
    parse_css_length(token)
}

fn parse_grid_auto_repeat_column_count(track: &str) -> Option<usize> {
    let min_track = parse_grid_minmax_min_track(track).unwrap_or_else(|| track.trim());
    let min_px = parse_px(min_track).unwrap_or(240.0).max(1.0);
    Some(((1280.0 / min_px).floor() as usize).clamp(1, 12))
}

fn parse_grid_auto_repeat_min_column_width(value: &str) -> Option<CssLength> {
    let value = value.trim();
    let repeat_start = value.find("repeat(")?;
    let inner = &value[repeat_start + "repeat(".len()..];
    let (count, track) = split_css_function_args(inner);
    let count = count.trim();
    if !count.eq_ignore_ascii_case("auto-fit") && !count.eq_ignore_ascii_case("auto-fill") {
        return None;
    }
    let min_track = parse_grid_minmax_min_track(track?.trim())?;
    parse_css_length(min_track)
}

fn parse_grid_minmax_min_track(track: &str) -> Option<&str> {
    let track = track.trim();
    let minmax_start = track.find("minmax(")?;
    let open_paren = minmax_start + "minmax".len();
    let close_paren = find_function_end(track, open_paren)?;
    let inner = &track[open_paren + 1..close_paren];
    let (min, _) = split_css_function_args(inner);
    Some(min.trim())
}

fn parse_grid_minmax_max_track(track: &str) -> Option<&str> {
    let track = track.trim();
    let minmax_start = track.find("minmax(")?;
    let open_paren = minmax_start + "minmax".len();
    let close_paren = find_function_end(track, open_paren)?;
    let inner = &track[open_paren + 1..close_paren];
    let (_, max) = split_css_function_args(inner);
    max.map(str::trim)
}

fn parse_grid_minmax_preferred_track(track: &str) -> Option<CssLength> {
    let min_track = parse_grid_minmax_min_track(track)?;
    let max_track = parse_grid_minmax_max_track(track)?;
    if let Some(max_length) = parse_css_length(max_track) {
        return Some(max_length);
    }
    if max_track.contains("var(") {
        return Some(CssLength::Auto);
    }
    parse_css_length(min_track)
}

fn parse_grid_template_rows(value: &str) -> Option<Vec<CssLength>> {
    let value = value.trim();
    if value.is_empty() || value == "none" {
        return None;
    }
    let mut rows = Vec::new();
    for token in split_css_value_list(value) {
        let token = token.trim();
        if token.is_empty() || token == "/" || token.eq_ignore_ascii_case("subgrid") {
            continue;
        }
        if let Some(repeated) = parse_grid_template_repeat_tracks_with_auto(token, true) {
            rows.extend(repeated);
            continue;
        }
        rows.push(parse_grid_template_track_size(token).unwrap_or(CssLength::Auto));
    }
    (!rows.is_empty()).then_some(rows)
}

fn parse_grid_auto_rows(value: &str) -> Option<CssLength> {
    split_css_value_list(value)
        .into_iter()
        .find_map(|token| parse_grid_template_track_size(&token))
}

fn parse_grid_line_span(value: &str) -> Option<usize> {
    let tokens = split_css_value_list(value);
    tokens.iter().enumerate().find_map(|(index, token)| {
        token
            .eq_ignore_ascii_case("span")
            .then(|| tokens.get(index + 1)?.trim().parse::<usize>().ok())
            .flatten()
            .filter(|span| *span > 0)
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CssGridColumnPlacement {
    start: Option<i32>,
    end: Option<i32>,
    span: Option<usize>,
}

fn parse_grid_column_placement(value: &str) -> CssGridColumnPlacement {
    let Some((start, end)) = value.split_once('/') else {
        return CssGridColumnPlacement {
            span: parse_grid_line_span(value),
            ..CssGridColumnPlacement::default()
        };
    };
    CssGridColumnPlacement {
        start: parse_grid_line_number(start),
        end: parse_grid_line_number(end),
        span: parse_grid_line_span(value),
    }
}

fn parse_grid_line_number(value: &str) -> Option<i32> {
    let token = split_css_value_list(value)
        .into_iter()
        .find(|token| !token.eq_ignore_ascii_case("auto") && !token.eq_ignore_ascii_case("span"))?;
    let line = token.trim().parse::<i32>().ok()?;
    (line != 0).then_some(line)
}

fn parse_grid_template_shorthand_columns(value: &str) -> Option<usize> {
    let (_, columns) = value.rsplit_once('/')?;
    parse_grid_template_columns(columns)
}

fn parse_grid_template_shorthand_column_tracks(value: &str) -> Option<Vec<CssLength>> {
    let (_, columns) = value.rsplit_once('/')?;
    parse_grid_template_column_tracks(columns)
}

fn parse_grid_template_shorthand_auto_repeat_min_column_width(value: &str) -> Option<CssLength> {
    let (_, columns) = value.rsplit_once('/')?;
    parse_grid_auto_repeat_min_column_width(columns)
}

fn parse_grid_template_shorthand_rows(value: &str) -> Option<Vec<CssLength>> {
    if let Some((_, rows)) = parse_grid_template_shorthand_area_rows(value) {
        return Some(rows);
    }
    let (rows, _) = value.rsplit_once('/')?;
    let row_tracks = rows
        .lines()
        .filter_map(|line| {
            let after_area = line
                .rfind(['\'', '"'])
                .map(|index| &line[index + 1..])
                .unwrap_or(line);
            parse_grid_template_rows(after_area)
        })
        .flatten()
        .collect::<Vec<_>>();
    (!row_tracks.is_empty()).then_some(row_tracks)
}

fn parse_grid_template_shorthand_areas(value: &str) -> Option<Vec<Vec<String>>> {
    parse_grid_template_shorthand_area_rows(value).map(|(areas, _)| areas)
}

fn parse_grid_template_shorthand_area_rows(
    value: &str,
) -> Option<(Vec<Vec<String>>, Vec<CssLength>)> {
    let (rows, _) = value.rsplit_once('/')?;
    let mut areas = Vec::new();
    let mut row_tracks = Vec::new();
    let mut cursor = 0usize;

    while let Some((start, quote)) = find_next_css_quote(rows, cursor) {
        let after_start = start + quote.len_utf8();
        let Some(relative_end) = rows[after_start..].find(quote) else {
            break;
        };
        let end = after_start + relative_end;
        let area_row = rows[after_start..end]
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let has_area_row = !area_row.is_empty();
        if has_area_row {
            areas.push(area_row);
        }

        let after_end = end + quote.len_utf8();
        let next_quote = find_next_css_quote(rows, after_end)
            .map(|(index, _)| index)
            .unwrap_or(rows.len());
        let track_text = rows[after_end..next_quote].trim();
        if let Some(tracks) = parse_grid_template_rows(track_text) {
            row_tracks.extend(tracks);
        } else if has_area_row {
            row_tracks.push(CssLength::Auto);
        }
        cursor = next_quote;
    }

    if areas.is_empty() {
        return None;
    }
    Some((areas, row_tracks))
}

fn find_next_css_quote(value: &str, start: usize) -> Option<(usize, char)> {
    value
        .char_indices()
        .skip_while(|(index, _)| *index < start)
        .find_map(|(index, ch)| matches!(ch, '\'' | '"').then_some((index, ch)))
}

fn parse_grid_template_repeat_tracks_with_auto(
    value: &str,
    allow_auto_repeat: bool,
) -> Option<Vec<CssLength>> {
    let inner = value
        .strip_prefix("repeat(")
        .and_then(|value| value.strip_suffix(')'))?;
    let (count, track) = split_css_function_args(inner);
    let count = count.trim();
    if count.eq_ignore_ascii_case("auto-fit") || count.eq_ignore_ascii_case("auto-fill") {
        if !allow_auto_repeat {
            return None;
        }
        return Some(vec![
            parse_grid_template_track_size(track?.trim()).unwrap_or(CssLength::Auto),
        ]);
    }
    let count = count.parse::<usize>().ok()?;
    if count == 0 {
        return None;
    }
    let tracks = split_css_value_list(track?)
        .into_iter()
        .map(|track| parse_grid_template_track_size(&track).unwrap_or(CssLength::Auto))
        .collect::<Vec<_>>();
    if tracks.is_empty() {
        return None;
    }
    let mut repeated = Vec::with_capacity(tracks.len() * count);
    for _ in 0..count {
        repeated.extend(tracks.iter().copied());
    }
    Some(repeated)
}

fn parse_grid_template_areas(value: &str) -> Option<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut rest = value.trim();
    while let Some(start) = rest.find(['\'', '"']) {
        let quote = rest.as_bytes()[start] as char;
        let after_start = &rest[start + quote.len_utf8()..];
        let Some(end) = after_start.find(quote) else {
            break;
        };
        let row = after_start[..end]
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if !row.is_empty() {
            rows.push(row);
        }
        rest = &after_start[end + quote.len_utf8()..];
    }
    (!rows.is_empty()).then_some(rows)
}

fn parse_grid_area(value: &str) -> Option<String> {
    let name = value.split('/').next()?.trim();
    if name.is_empty() || name == "auto" || name == "." {
        None
    } else {
        Some(name.to_owned())
    }
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

fn parse_inset_edges(value: &str, viewport_width: Option<f32>) -> Option<(CssEdges, CssInset)> {
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
    let mut sides = CssInset::default();
    let mut saw_side = false;
    for (index, token) in expanded.iter().enumerate() {
        if token.eq_ignore_ascii_case("auto") {
            continue;
        }
        let px = parse_inset_value(token, viewport_width)?;
        saw_side = true;
        match index {
            0 => {
                edges.top = px;
                sides.top = Some(px);
            }
            1 => {
                edges.right = px;
                sides.right = Some(px);
            }
            2 => {
                edges.bottom = px;
                sides.bottom = Some(px);
            }
            3 => {
                edges.left = px;
                sides.left = Some(px);
            }
            _ => {}
        }
    }
    saw_side.then_some((edges, sides))
}

fn parse_inset_value(value: &str, viewport_width: Option<f32>) -> Option<f32> {
    parse_px(value).or_else(|| {
        parse_signed_percent(value)
            .map(|percent| viewport_width.unwrap_or(1280.0) * percent / 100.0)
    })
}

fn parse_css_transform(value: &str) -> Option<CssTransform> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(CssTransform::default());
    }
    let lower = value.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("translatex(")
        .and_then(|value| value.strip_suffix(')'))?;
    Some(CssTransform {
        translate_x: parse_css_transform_length(inner.trim()),
    })
    .filter(|transform| transform.translate_x.is_some())
}

fn parse_css_transform_length(value: &str) -> Option<CssLength> {
    if let Some(percent) = parse_signed_percent(value) {
        Some(CssLength::Percent(percent))
    } else {
        parse_px(value).map(CssLength::Px)
    }
}

fn parse_css_length(value: &str) -> Option<CssLength> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        Some(CssLength::Auto)
    } else if value.eq_ignore_ascii_case("fit-content") {
        Some(CssLength::Auto)
    } else if let Some(inner) = value
        .strip_prefix("min(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (left, right) = split_css_function_args(inner);
        Some(CssLength::Min(
            parse_css_length_expression(left.trim())?,
            parse_css_length_expression(right?.trim())?,
        ))
    } else if let Some(inner) = value
        .strip_prefix("max(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (left, right) = split_css_function_args(inner);
        Some(CssLength::Max(
            parse_css_length_expression(left.trim())?,
            parse_css_length_expression(right?.trim())?,
        ))
    } else if let Some(inner) = value
        .strip_prefix("clamp(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (min, rest) = split_css_function_args(inner);
        let (preferred, max) = split_css_function_args(rest?.trim());
        Some(CssLength::Clamp(
            parse_css_length_expression(min.trim())?,
            parse_css_length_expression(preferred.trim())?,
            parse_css_length_expression(max?.trim())?,
        ))
    } else if let Some(inner) = value
        .strip_prefix("fit-content(")
        .and_then(|value| value.strip_suffix(')'))
    {
        Some(CssLength::FitContent(parse_css_length_expression(
            inner.trim(),
        )?))
    } else if let Some(inner) = value
        .strip_prefix("calc(")
        .and_then(|value| value.strip_suffix(')'))
    {
        Some(css_length_from_expression(parse_css_length_expression(
            inner,
        )?))
    } else if let Some(vw) = value.strip_suffix("vw") {
        vw.trim().parse::<f32>().ok().map(CssLength::Vw)
    } else if let Some(vh) = value.strip_suffix("vh") {
        vh.trim().parse::<f32>().ok().map(CssLength::Vh)
    } else if let Some(fr) = value.strip_suffix("fr") {
        fr.trim().parse::<f32>().ok().map(CssLength::Fr)
    } else {
        parse_percent(value)
            .map(CssLength::Percent)
            .or_else(|| parse_px(value).map(CssLength::Px))
    }
}

fn css_length_from_expression(expression: CssLengthExpression) -> CssLength {
    if expression.percent == 0.0 && expression.vw == 0.0 && expression.vh == 0.0 {
        CssLength::Px(expression.px)
    } else {
        CssLength::Calc(expression)
    }
}

fn parse_gap(value: &str) -> Option<f32> {
    split_css_value_list(value)
        .into_iter()
        .find_map(|part| parse_px(&part))
}

fn parse_css_length_expression(value: &str) -> Option<CssLengthExpression> {
    let mut expression = CssLengthExpression::default();
    let mut current = String::new();
    let mut sign = 1.0;
    let mut depth = 0usize;
    let mut saw_term = false;

    for ch in value.chars().chain(std::iter::once('+')) {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            '+' | '-' if depth == 0 => {
                if !current.trim().is_empty() {
                    expression = add_css_length_expression_term(
                        expression,
                        parse_css_length_expression_term(current.trim())?,
                        sign,
                    );
                    current.clear();
                    saw_term = true;
                }
                sign = if ch == '-' { -1.0 } else { 1.0 };
            }
            _ => current.push(ch),
        }
    }

    saw_term.then_some(expression)
}

fn add_css_length_expression_term(
    mut expression: CssLengthExpression,
    term: CssLengthExpression,
    sign: f32,
) -> CssLengthExpression {
    expression.px += term.px * sign;
    expression.percent += term.percent * sign;
    expression.vw += term.vw * sign;
    expression.vh += term.vh * sign;
    expression
}

fn parse_css_length_expression_term(value: &str) -> Option<CssLengthExpression> {
    let value = value.trim();
    if value.contains('*') {
        let parts = value.split('*').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 2 {
            return None;
        }
        if let Ok(factor) = parts[0].parse::<f32>() {
            return scale_css_length_expression(
                parse_css_length_expression_term(parts[1])?,
                factor,
            );
        }
        if let Ok(factor) = parts[1].parse::<f32>() {
            return scale_css_length_expression(
                parse_css_length_expression_term(parts[0])?,
                factor,
            );
        }
        return None;
    }
    if let Some((left, right)) = value.split_once('/') {
        let denominator = right.trim().parse::<f32>().ok()?;
        if denominator == 0.0 {
            return None;
        }
        return scale_css_length_expression(
            parse_css_length_expression_term(left.trim())?,
            1.0 / denominator,
        );
    }
    parse_css_length_expression_factor(value)
}

fn scale_css_length_expression(
    mut expression: CssLengthExpression,
    factor: f32,
) -> Option<CssLengthExpression> {
    expression.px *= factor;
    expression.percent *= factor;
    expression.vw *= factor;
    expression.vh *= factor;
    Some(expression)
}

fn parse_css_length_expression_factor(value: &str) -> Option<CssLengthExpression> {
    let value = value.trim();
    if let Some(inner) = css_strip_wrapping_parens(value) {
        return parse_css_length_expression(inner);
    }
    if let Some(inner) = value
        .strip_prefix("calc(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_css_length_expression(inner);
    }
    if let Some(percent) = parse_percent(value) {
        return Some(CssLengthExpression {
            percent,
            ..CssLengthExpression::default()
        });
    }
    if let Some(vw) = value.strip_suffix("vw") {
        return Some(CssLengthExpression {
            vw: vw.trim().parse::<f32>().ok()?,
            ..CssLengthExpression::default()
        });
    }
    if let Some(vh) = value.strip_suffix("vh") {
        return Some(CssLengthExpression {
            vh: vh.trim().parse::<f32>().ok()?,
            ..CssLengthExpression::default()
        });
    }
    Some(CssLengthExpression {
        px: parse_px(value)?,
        ..CssLengthExpression::default()
    })
}

fn css_strip_wrapping_parens(value: &str) -> Option<&str> {
    let value = value.trim();
    let inner = value.strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index != value.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(inner.trim())
}

fn button_width_for_text(text: &str, style: &BrowserStyle, font_scale: f32) -> f32 {
    text.chars().count() as f32 * 6.45 * font_scale + style.button_padding_x * 2.0 * font_scale
}

fn apply_color(value: &str, target: &mut Color32) {
    if let Some(color) = parse_color(value) {
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
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "none" || normalized == "0" {
        return Some((0.0, Color32::TRANSPARENT));
    }
    let values = split_css_value_list(value);
    if values
        .iter()
        .any(|value| value.eq_ignore_ascii_case("none"))
    {
        return Some((0.0, Color32::TRANSPARENT));
    }
    let width = values
        .iter()
        .find_map(|value| parse_px(value).or_else(|| (value == "0").then_some(0.0)))?;
    let color = values
        .iter()
        .find_map(|value| parse_color(value))
        .unwrap_or(Color32::TRANSPARENT);
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
    if let Some(inner) = value
        .strip_prefix("min(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (left, right) = split_css_function_args(inner);
        return Some(parse_px(left.trim())?.min(parse_px(right?.trim())?));
    }
    if let Some(inner) = value
        .strip_prefix("max(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (left, right) = split_css_function_args(inner);
        return Some(parse_px(left.trim())?.max(parse_px(right?.trim())?));
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
    let mut depth = 0usize;
    let mut saw_term = false;

    for ch in value.chars().chain(std::iter::once('+')) {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            '+' | '-' if depth == 0 => {
                if !current.trim().is_empty() {
                    total += sign * parse_calc_length_term(current.trim())?;
                    current.clear();
                    saw_term = true;
                }
                sign = if ch == '-' { -1.0 } else { 1.0 };
            }
            _ => current.push(ch),
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
    if let Some(inner) = css_strip_wrapping_parens(value) {
        return parse_calc_length(inner);
    }
    parse_px(value).or_else(|| value.parse::<f32>().ok())
}

fn parse_percent(value: &str) -> Option<f32> {
    parse_signed_percent(value).filter(|percent| *percent > 0.0)
}

fn parse_signed_percent(value: &str) -> Option<f32> {
    value.trim().strip_suffix('%')?.trim().parse::<f32>().ok()
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

    fn sample_input(
        label: &str,
        name: &str,
        value: &str,
        form_id: Option<&str>,
        form_action: Option<&str>,
    ) -> CanvasObject {
        CanvasObject::Input(CanvasInputObject {
            label: label.to_owned(),
            name: Some(name.to_owned()),
            value: value.to_owned(),
            default_value: value.to_owned(),
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 20.0)),
            font_size: 14.0,
            color: Color32::BLACK,
            form_id: form_id.map(str::to_owned),
            form_action: form_action.map(str::to_owned),
            form_method: Some("get".to_owned()),
            element_id: None,
            kind: CanvasInputKind::Text,
            submit_on_enter: false,
        })
    }

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
    fn browser_textbox_lut_uses_glyph_advances_for_inline_runs() {
        let ctx = egui::Context::default();
        configure_browser_fonts(&ctx);
        let style = ResolvedBoxStyle {
            font_size: 16.0,
            ..ResolvedBoxStyle::default()
        };

        let _ = ctx.run(Default::default(), |_| {
            let mut lut = calculate_browser_font_size_lut(&ctx, &style);
            let text = "La frase viene stampata a video dal primo programma di esempio scritto in ";
            let measured = measure_browser_textbox_with_lut(&ctx, &mut lut, text, &style);
            let fixed_width_estimate = text.chars().count() as f32 * style.font_size * 0.56;

            assert!(measured.x > 400.0);
            assert!(
                measured.x < fixed_width_estimate - 80.0,
                "inline proportional text should use glyph advances, got {:.1} vs fixed {:.1}",
                measured.x,
                fixed_width_estimate
            );
        });
    }

    #[test]
    fn form_submission_collects_all_controls_for_form() {
        let objects = vec![
            sample_input("Query", "q", "trees", Some("search"), Some("/search")),
            sample_input("Page", "page", "1", Some("search"), Some("/search")),
            sample_input("Other", "q", "ignored", Some("other"), Some("/other")),
        ];
        let mut response = BrowserCanvasResponse::default();

        push_submitted_inputs_for_form(
            &objects,
            Some("search"),
            Some("/search"),
            Some("get"),
            Some("query"),
            &mut response,
        );

        assert_eq!(response.submitted_inputs.len(), 2);
        assert_eq!(response.submitted_inputs[0].name.as_deref(), Some("q"));
        assert_eq!(response.submitted_inputs[0].value, "trees");
        assert_eq!(
            response.submitted_inputs[0].form_action.as_deref(),
            Some("/search")
        );
        assert_eq!(
            response.submitted_inputs[0].form_method.as_deref(),
            Some("get")
        );
        assert_eq!(response.submitted_inputs[0].element_id.as_deref(), None);
        assert_eq!(
            response.submitted_inputs[0].submitter_element_id.as_deref(),
            Some("query")
        );
        assert_eq!(response.submitted_inputs[1].name.as_deref(), Some("page"));
        assert_eq!(response.submitted_inputs[1].value, "1");
    }

    #[test]
    fn form_reset_restores_all_controls_for_form() {
        let mut objects = vec![
            sample_input("Query", "q", "trees", Some("search"), Some("/search")),
            sample_input("Other", "q", "kept", Some("other"), Some("/other")),
        ];
        if let CanvasObject::Input(input) = &mut objects[0] {
            input.default_value = String::new();
        }
        if let CanvasObject::Input(input) = &mut objects[1] {
            input.default_value = String::new();
        }
        let mut response = BrowserCanvasResponse::default();

        reset_inputs_for_form(&mut objects, Some("search"), &mut response);

        let CanvasObject::Input(search) = &objects[0] else {
            panic!("expected input");
        };
        let CanvasObject::Input(other) = &objects[1] else {
            panic!("expected input");
        };
        assert_eq!(search.value, "");
        assert_eq!(other.value, "kept");
        assert_eq!(response.changed_inputs.len(), 1);
        assert_eq!(response.changed_inputs[0].label, "Query");
    }

    #[test]
    fn overlapping_submit_button_defers_to_editable_input_hit_area() {
        let button = Rect::from_min_max(Pos2::new(20.0, 20.0), Pos2::new(320.0, 60.0));
        let input = Rect::from_min_max(Pos2::new(32.0, 28.0), Pos2::new(240.0, 52.0));
        let outside_input = Pos2::new(280.0, 40.0);
        let inside_input = Pos2::new(100.0, 40.0);

        assert!(button_hit_deferred_to_editable_input(
            button,
            inside_input,
            &[input]
        ));
        assert!(!button_hit_deferred_to_editable_input(
            button,
            outside_input,
            &[input]
        ));
    }

    #[test]
    fn canvas_link_text_color_keeps_unhovered_link_color() {
        let link = Color32::from_rgb(26, 13, 171);

        assert_eq!(canvas_link_text_color(link, false), link);
    }

    #[test]
    fn canvas_link_text_color_darkens_hovered_link_color() {
        let link = Color32::from_rgba_premultiplied(26, 13, 171, 230);
        let hovered = canvas_link_text_color(link, true);

        assert!(hovered.r() < link.r());
        assert!(hovered.g() < link.g());
        assert!(hovered.b() < link.b());
        assert_eq!(hovered.a(), link.a());
    }

    #[test]
    fn canvas_text_link_hover_uses_element_id_when_available() {
        assert!(canvas_text_link_hovered(
            Some("/result"),
            Some("title-link"),
            Some("/result"),
            Some("title-link"),
        ));
        assert!(!canvas_text_link_hovered(
            Some("/result"),
            Some("source-link"),
            Some("/result"),
            Some("title-link"),
        ));
    }

    #[test]
    fn canvas_text_link_hover_falls_back_to_href_without_element_id() {
        assert!(canvas_text_link_hovered(
            Some("/result"),
            None,
            Some("/result"),
            None,
        ));
        assert!(!canvas_text_link_hovered(
            Some("/other"),
            None,
            Some("/result"),
            None,
        ));
    }

    #[test]
    fn canvas_graph_text_edit_id_prefers_element_id_over_index() {
        assert_eq!(
            canvas_graph_text_edit_id("input", Some("search-box"), 1),
            canvas_graph_text_edit_id("input", Some("search-box"), 99)
        );
        assert_ne!(
            canvas_graph_text_edit_id("input", None, 1),
            canvas_graph_text_edit_id("input", None, 99)
        );
        assert_ne!(
            canvas_graph_text_edit_id("input", Some("search-box"), 1),
            canvas_graph_text_edit_id("textarea", Some("search-box"), 1)
        );
    }

    #[test]
    fn ecosia_search_submit_uses_native_get_metadata() {
        let mut response = BrowserCanvasResponse::default();

        push_ecosia_search_submit("trees", &mut response);

        assert_eq!(response.submitted_inputs.len(), 1);
        let submit = &response.submitted_inputs[0];
        assert_eq!(submit.name.as_deref(), Some("q"));
        assert_eq!(submit.value, "trees");
        assert_eq!(submit.form_action.as_deref(), Some("/search"));
        assert_eq!(submit.form_method.as_deref(), Some("get"));
        assert_eq!(submit.kind, CanvasInputKind::Text);
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
    fn parse_basic_css_applies_plain_screen_media_blocks() {
        let style = parse_basic_css_for_viewport(
            r#"
            @media screen {
                .menu {
                    visibility: hidden;
                    opacity: 0;
                    position: absolute;
                }
            }
            @media print {
                .menu {
                    display: none;
                }
            }
            "#,
            1280.0,
        );
        let menu = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["menu".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &menu);

        assert_eq!(computed.visibility_visible, Some(false));
        assert_eq!(computed.opacity, Some(0.0));
        assert_eq!(computed.position, Some(CssPosition::Absolute));
        assert_eq!(computed.display, None);
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
    fn nth_child_pseudo_classes_stay_bound_to_compound_selectors() {
        let selector = parse_css_selector(".tile:nth-child(3n)").unwrap();

        assert_eq!(selector.classes, vec!["tile"]);
        assert_eq!(selector.nth_child, Some(CssNthChild { step: 3, offset: 0 }));
    }

    #[test]
    fn nth_last_child_pseudo_classes_stay_bound_to_compound_selectors() {
        let selector = parse_css_selector(".weather_table th:nth-last-child(-n+5)").unwrap();

        assert_eq!(selector.tag.as_deref(), Some("th"));
        assert_eq!(
            selector.nth_last_child,
            Some(CssNthChild {
                step: -1,
                offset: 5
            })
        );
    }

    #[test]
    fn computed_box_style_matches_classed_nth_child_cascade() {
        let style = parse_basic_css(
            r#"
            .tile { background: #ff66aa; }
            .tile:nth-child(3n) { background: #f59e0b; }
            .tile:nth-child(4n) { background: #22d3ee; }
            "#,
        );
        let tile = |child_index| ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["tile".to_owned()],
            child_index: Some(child_index),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &tile(1)).background,
            Some(Color32::from_rgb(0xff, 0x66, 0xaa))
        );
        assert_eq!(
            computed_box_style(&style, &tile(3)).background,
            Some(Color32::from_rgb(0xf5, 0x9e, 0x0b))
        );
        assert_eq!(
            computed_box_style(&style, &tile(4)).background,
            Some(Color32::from_rgb(0x22, 0xd3, 0xee))
        );
        assert_eq!(
            computed_box_style(&style, &tile(12)).background,
            Some(Color32::from_rgb(0x22, 0xd3, 0xee))
        );

        let non_tile = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["card".to_owned()],
            child_index: Some(3),
            ..ElementStyleKey::default()
        };
        assert_eq!(computed_box_style(&style, &non_tile).background, None);
    }

    #[test]
    fn computed_box_style_matches_ilmeteo_nth_last_child_show_hide_cascade() {
        let style = parse_basic_css(
            r#"
            .weather_table.more_data tr th:nth-last-child(-n+12) { display: none; }
            .weather_table.more_data tr th:nth-last-child(-n+6) { display: table-cell; }
            .weather_table.more_data.summer tr th:nth-last-child(-n+11) { display: none; }
            .weather_table.more_data.summer tr th:nth-last-child(-n+5) { display: table-cell; }
            "#,
        );
        let table = ElementStyleKey {
            tag: "table".to_owned(),
            classes: vec![
                "weather_table".to_owned(),
                "more_data".to_owned(),
                "summer".to_owned(),
            ],
            ..ElementStyleKey::default()
        };
        let tr = ElementStyleKey {
            tag: "tr".to_owned(),
            parent: Some(Box::new(table)),
            ..ElementStyleKey::default()
        };
        let th = |child_index| ElementStyleKey {
            tag: "th".to_owned(),
            child_index: Some(child_index),
            child_count: Some(14),
            parent: Some(Box::new(tr.clone())),
            ..ElementStyleKey::default()
        };

        assert_eq!(computed_box_style(&style, &th(2)).display, None);
        assert_eq!(
            computed_box_style(&style, &th(3)).display,
            Some(CssDisplay::None)
        );
        assert_eq!(
            computed_box_style(&style, &th(4)).display,
            Some(CssDisplay::None)
        );
        assert_eq!(
            computed_box_style(&style, &th(9)).display,
            Some(CssDisplay::None)
        );
        assert_eq!(
            computed_box_style(&style, &th(10)).display,
            Some(CssDisplay::Table)
        );
        assert_eq!(
            computed_box_style(&style, &th(14)).display,
            Some(CssDisplay::Table)
        );
    }

    #[test]
    fn table_section_direct_cells_match_implicit_tr_descendant_selectors() {
        let style = parse_basic_css(
            r#"
            .weather_table thead tr th:nth-last-child(-n+2) { display: none; }
            "#,
        );
        let table = ElementStyleKey {
            tag: "table".to_owned(),
            classes: vec!["weather_table".to_owned()],
            ..ElementStyleKey::default()
        };
        let thead = ElementStyleKey {
            tag: "thead".to_owned(),
            parent: Some(Box::new(table)),
            ..ElementStyleKey::default()
        };
        let th = |child_index| ElementStyleKey {
            tag: "th".to_owned(),
            child_index: Some(child_index),
            child_count: Some(4),
            parent: Some(Box::new(thead.clone())),
            ..ElementStyleKey::default()
        };

        assert_eq!(computed_box_style(&style, &th(2)).display, None);
        assert_eq!(
            computed_box_style(&style, &th(3)).display,
            Some(CssDisplay::None)
        );
        assert_eq!(
            computed_box_style(&style, &th(4)).display,
            Some(CssDisplay::None)
        );
    }

    #[test]
    fn computed_box_style_matches_odd_nth_child_with_ancestor_class() {
        let style = parse_basic_css(
            r#"
            .gallery .box { background: #14b8a6; }
            .gallery .box:nth-child(odd) { background: #ec4899; }
            "#,
        );
        let gallery = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["gallery".to_owned()],
            ..ElementStyleKey::default()
        };
        let gallery_child = |child_index| ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["box".to_owned()],
            child_index: Some(child_index),
            parent: Some(Box::new(gallery.clone())),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &gallery_child(1)).background,
            Some(Color32::from_rgb(0xec, 0x48, 0x99))
        );
        assert_eq!(
            computed_box_style(&style, &gallery_child(2)).background,
            Some(Color32::from_rgb(0x14, 0xb8, 0xa6))
        );
    }

    #[test]
    fn parse_basic_css_carries_flex_layout_properties() {
        let style = parse_basic_css(
            r#"
            .toolbar {
                display: flex;
                flex: 1 0 192px;
                flex-direction: row;
                flex-wrap: wrap;
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
        assert_eq!(computed.flex_grow, Some(1.0));
        assert_eq!(computed.flex_shrink, Some(0.0));
        assert_eq!(computed.flex_basis, Some(CssLength::Px(192.0)));
        assert_eq!(computed.flex_direction, Some(CssFlexDirection::Row));
        assert_eq!(computed.flex_wrap, Some(CssFlexWrap::Wrap));
        assert_eq!(
            computed.justify_content,
            Some(CssJustifyContent::SpaceBetween)
        );
        assert_eq!(computed.align_items, Some(CssAlignItems::Center));
        assert_eq!(computed.gap, Some(12.0));
    }

    #[test]
    fn parse_basic_css_carries_flex_align_self() {
        let style = parse_basic_css(".panel { align-self: center; } .auto { align-self: auto; }");
        let key = |class_name: &str| ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec![class_name.to_owned()],
            ..ElementStyleKey::default()
        };

        let panel = computed_box_style(&style, &key("panel"));
        assert_eq!(panel.align_self, Some(CssAlignItems::Center));

        let auto = computed_box_style(&style, &key("auto"));
        assert_eq!(auto.align_self, None);
    }

    #[test]
    fn parse_basic_css_expands_common_flex_shorthands() {
        let style = parse_basic_css(
            r#"
            .grow { flex: 1; }
            .auto { flex: auto; }
            .none { flex: none; }
            .basis { flex-basis: 25%; flex-shrink: 0; }
            "#,
        );
        let key = |class_name: &str| ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec![class_name.to_owned()],
            ..ElementStyleKey::default()
        };

        let grow = computed_box_style(&style, &key("grow"));
        assert_eq!(grow.flex_grow, Some(1.0));
        assert_eq!(grow.flex_shrink, Some(1.0));
        assert_eq!(grow.flex_basis, Some(CssLength::Percent(0.0)));

        let auto = computed_box_style(&style, &key("auto"));
        assert_eq!(auto.flex_grow, Some(1.0));
        assert_eq!(auto.flex_shrink, Some(1.0));
        assert_eq!(auto.flex_basis, Some(CssLength::Auto));

        let none = computed_box_style(&style, &key("none"));
        assert_eq!(none.flex_grow, Some(0.0));
        assert_eq!(none.flex_shrink, Some(0.0));
        assert_eq!(none.flex_basis, Some(CssLength::Auto));

        let basis = computed_box_style(&style, &key("basis"));
        assert_eq!(basis.flex_shrink, Some(0.0));
        assert_eq!(basis.flex_basis, Some(CssLength::Percent(25.0)));
    }

    #[test]
    fn parse_basic_css_resets_border_from_none_and_zero() {
        let style = parse_basic_css(
            r#"
            .none { border: none; }
            .zero { border: 0; }
            .solid { border: 0 solid transparent; }
            "#,
        );
        let key = |class_name: &str| ElementStyleKey {
            tag: "button".to_owned(),
            classes: vec![class_name.to_owned()],
            ..ElementStyleKey::default()
        };

        let none = computed_box_style(&style, &key("none"));
        assert_eq!(none.border_width, Some(0.0));
        assert_eq!(none.border_color, Some(Color32::TRANSPARENT));

        let zero = computed_box_style(&style, &key("zero"));
        assert_eq!(zero.border_width, Some(0.0));
        assert_eq!(zero.border_color, Some(Color32::TRANSPARENT));

        let solid = computed_box_style(&style, &key("solid"));
        assert_eq!(solid.border_width, Some(0.0));
        assert_eq!(solid.border_color, Some(Color32::TRANSPARENT));
    }

    #[test]
    fn parse_basic_css_carries_grid_item_and_content_alignment() {
        let style = parse_basic_css(
            r#"
            .grid {
                display: grid;
                place-items: center;
                align-content: start;
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "div".to_owned(),
            id: None,
            classes: vec!["grid".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.display, Some(CssDisplay::Grid));
        assert_eq!(computed.align_items, Some(CssAlignItems::Center));
        assert_eq!(computed.justify_items, Some(CssJustifyContent::Center));
        assert_eq!(computed.align_content, Some(CssAlignItems::FlexStart));
        assert_eq!(computed.justify_content, None);
    }

    #[test]
    fn parse_basic_css_applies_universal_box_sizing_rule_to_box_styles() {
        let style = parse_basic_css("* { box-sizing: border-box; } .item { min-width: 0; }");
        let key = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["item".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.box_sizing_border_box, Some(true));
        assert_eq!(computed.min_width, Some(CssLength::Px(0.0)));
    }

    #[test]
    fn parse_basic_css_accepts_two_value_gap_shorthand() {
        let style = parse_basic_css(".tabs { display: flex; gap: 8px 12px; }");
        let key = ElementStyleKey {
            tag: "nav".to_owned(),
            classes: vec!["tabs".to_owned()],
            ..ElementStyleKey::default()
        };

        assert_eq!(computed_box_style(&style, &key).gap, Some(8.0));
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
    fn parse_basic_css_carries_fixed_overlay_inset_and_translate_x() {
        let style = parse_basic_css_for_viewport(
            r#"
            .overlay {
                position: fixed;
                inset: 96px auto auto 50%;
                transform: translateX(-50%);
            }
            "#,
            1280.0,
        );
        let key = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["overlay".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.position, Some(CssPosition::Fixed));
        assert_eq!(computed.inset_sides.top, Some(96.0));
        assert_eq!(computed.inset_sides.right, None);
        assert_eq!(computed.inset_sides.bottom, None);
        assert_eq!(computed.inset_sides.left, Some(640.0));
        assert_eq!(
            computed
                .transform
                .and_then(|transform| transform.translate_x),
            Some(CssLength::Percent(-50.0))
        );
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
    fn parse_basic_css_maps_ltr_logical_box_properties() {
        let style = parse_basic_css(
            r#"
            .centered {
                inline-size: min(960px, calc(100vw - 32px));
                max-inline-size: 960px;
                min-inline-size: 320px;
                block-size: 120px;
                min-block-size: 80px;
                margin-inline: auto;
                margin-block: 24px 32px;
                padding-inline: 12px 20px;
                padding-block: 6px 10px;
                inset-inline: 14px 18px;
                inset-block-start: 22px;
            }
            .responsive {
                padding-inline: max(24px, calc((100vw - 960px) / 2));
            }
            "#,
        );
        let centered = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["centered".to_owned()],
            ..ElementStyleKey::default()
        };
        let responsive = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["responsive".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &centered);
        let responsive_computed = computed_box_style(&style, &responsive);

        assert_eq!(
            computed.width,
            Some(CssLength::Min(
                CssLengthExpression {
                    px: 960.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    px: -32.0,
                    vw: 100.0,
                    ..CssLengthExpression::default()
                },
            ))
        );
        assert_eq!(computed.max_width, Some(CssLength::Px(960.0)));
        assert_eq!(computed.min_width, Some(CssLength::Px(320.0)));
        assert_eq!(computed.height, Some(CssLength::Px(120.0)));
        assert_eq!(computed.min_height, Some(CssLength::Px(80.0)));
        assert_eq!(computed.margin_left, Some(0.0));
        assert_eq!(computed.margin_right, Some(0.0));
        assert_eq!(computed.margin_top, Some(24.0));
        assert_eq!(computed.margin_bottom, Some(32.0));
        assert_eq!(computed.margin_auto.left, Some(true));
        assert_eq!(computed.margin_auto.right, Some(true));
        assert_eq!(computed.padding_left, Some(12.0));
        assert_eq!(computed.padding_right, Some(20.0));
        assert_eq!(computed.padding_top, Some(6.0));
        assert_eq!(computed.padding_bottom, Some(10.0));
        assert_eq!(computed.inset_sides.left, Some(14.0));
        assert_eq!(computed.inset_sides.right, Some(18.0));
        assert_eq!(computed.inset_sides.top, Some(22.0));
        assert_eq!(responsive_computed.padding_left, Some(160.0));
        assert_eq!(responsive_computed.padding_right, Some(160.0));
    }

    #[test]
    fn parse_basic_css_matches_not_selectors_without_broadening_them() {
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

        assert_eq!(
            computed_box_style(&style, &icon).display,
            Some(CssDisplay::None)
        );
        assert_eq!(
            computed_box_style(&style, &icon_with_text).display,
            Some(CssDisplay::InlineBlock)
        );
    }

    #[test]
    fn fixture_56_body_not_selector_applies_only_without_menu_state() {
        let style = parse_basic_css(
            r#"
            body:not(.menu-open) .drawer { display: none; }
            body:not(.menu-open) .content { background: #51cf66; }
            .drawer { background: #ff6b6b; }
            "#,
        );
        let closed_body = ElementStyleKey {
            tag: "body".to_owned(),
            ..ElementStyleKey::default()
        };
        let open_body = ElementStyleKey {
            tag: "body".to_owned(),
            classes: vec!["menu-open".to_owned()],
            ..ElementStyleKey::default()
        };
        let drawer = |body: ElementStyleKey| ElementStyleKey {
            tag: "aside".to_owned(),
            classes: vec!["box".to_owned(), "drawer".to_owned()],
            parent: Some(Box::new(body)),
            ..ElementStyleKey::default()
        };
        let content = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["box".to_owned(), "content".to_owned()],
            parent: Some(Box::new(closed_body.clone())),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &drawer(closed_body)).display,
            Some(CssDisplay::None)
        );
        assert_eq!(computed_box_style(&style, &drawer(open_body)).display, None);
        assert_eq!(
            computed_box_style(&style, &content).background,
            Some(Color32::from_rgb(0x51, 0xcf, 0x66))
        );
    }

    #[test]
    fn fixture_57_is_and_where_groups_match_parent_child_context() {
        let style = parse_basic_css(
            r#"
            .box { background: #ffd43b; }
            :is(header, main, aside) > :where(.primary, .secondary) { background: #51cf66; }
            footer :is(.primary, .secondary) { background: #ff6b6b; }
            "#,
        );
        let child = |parent_tag: &str, class_name: &str| ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["box".to_owned(), class_name.to_owned()],
            parent: Some(Box::new(ElementStyleKey {
                tag: parent_tag.to_owned(),
                ..ElementStyleKey::default()
            })),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &child("header", "primary")).background,
            Some(Color32::from_rgb(0xff, 0xd4, 0x3b))
        );
        assert_eq!(
            computed_box_style(&style, &child("main", "secondary")).background,
            Some(Color32::from_rgb(0xff, 0xd4, 0x3b))
        );
        assert_eq!(
            computed_box_style(&style, &child("footer", "primary")).background,
            Some(Color32::from_rgb(0xff, 0x6b, 0x6b))
        );
    }

    #[test]
    fn parse_basic_css_carries_numeric_grid_column_lines() {
        let style = parse_basic_css(
            r#"
            .wide { grid-column: 1 / -1; }
            .middle { grid-column: 2 / 4; }
            .start { grid-column-start: 2; }
            .end { grid-column-end: -1; }
            "#,
        );
        let wide = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["wide".to_owned()],
            ..ElementStyleKey::default()
        };
        let middle = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["middle".to_owned()],
            ..ElementStyleKey::default()
        };
        let start = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["start".to_owned()],
            ..ElementStyleKey::default()
        };
        let end = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["end".to_owned()],
            ..ElementStyleKey::default()
        };

        let wide_style = computed_box_style(&style, &wide);
        assert_eq!(wide_style.grid_column_start, Some(1));
        assert_eq!(wide_style.grid_column_end, Some(-1));

        let middle_style = computed_box_style(&style, &middle);
        assert_eq!(middle_style.grid_column_start, Some(2));
        assert_eq!(middle_style.grid_column_end, Some(4));

        assert_eq!(
            computed_box_style(&style, &start).grid_column_start,
            Some(2)
        );
        assert_eq!(computed_box_style(&style, &end).grid_column_end, Some(-1));
    }

    #[test]
    fn fixture_58_value_aware_attributes_match_general_sibling_chain() {
        let style = parse_basic_css(
            r#"
            [aria-expanded="false"] ~ .menu { display: none; }
            [aria-expanded="true"] ~ .open-menu { background: #51cf66; }
            [aria-expanded="false"] ~ .closed-marker { background: #4dabf7; }
            .menu { background: #ff6b6b; }
            "#,
        );
        let toggle = ElementStyleKey {
            tag: "button".to_owned(),
            attributes: vec!["aria-expanded=false".to_owned()],
            ..ElementStyleKey::default()
        };
        let menu = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["box".to_owned(), "menu".to_owned()],
            previous_sibling: Some(Box::new(toggle.clone())),
            ..ElementStyleKey::default()
        };
        let marker = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["box".to_owned(), "closed-marker".to_owned()],
            previous_sibling: Some(Box::new(menu.clone())),
            ..ElementStyleKey::default()
        };
        let open_menu = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["box".to_owned(), "open-menu".to_owned()],
            previous_sibling: Some(Box::new(toggle)),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &menu).display,
            Some(CssDisplay::None)
        );
        assert_eq!(
            computed_box_style(&style, &marker).background,
            Some(Color32::from_rgb(0x4d, 0xab, 0xf7))
        );
        assert_eq!(computed_box_style(&style, &open_menu).background, None);
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
    fn parse_basic_css_ignores_dynamic_pseudo_classes_instead_of_broadening_them() {
        let style = parse_basic_css(
            r#"
            .button { color: #333333; }
            .button:hover { background-color: #deded9; }
            .button:active { color: #ffffff; }
            .button.force-hover { background-color: #4c4c4c; }
            "#,
        );
        let button = ElementStyleKey {
            tag: "button".to_owned(),
            classes: vec!["button".to_owned()],
            ..ElementStyleKey::default()
        };
        let forced = ElementStyleKey {
            tag: "button".to_owned(),
            classes: vec!["button".to_owned(), "force-hover".to_owned()],
            ..ElementStyleKey::default()
        };

        let button_style = computed_box_style(&style, &button);
        assert_eq!(
            button_style.color,
            Some(Color32::from_rgb(0x33, 0x33, 0x33))
        );
        assert_eq!(button_style.background, None);

        assert_eq!(
            computed_box_style(&style, &forced).background,
            Some(Color32::from_rgb(0x4c, 0x4c, 0x4c))
        );
    }

    #[test]
    fn parse_basic_css_carries_grid_template_column_count() {
        let style = parse_basic_css(
            ".counter { display: grid; grid-template-columns: 1fr auto 1fr; } .cards { grid-template-columns: repeat(4, minmax(0, 1fr)); } .auto { grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); } .page { grid-template: min-content 48px / 12.25rem minmax(0, 1fr); } .rows { grid-template-rows: 40px 25%; }",
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
        let page = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["page".to_owned()],
            ..ElementStyleKey::default()
        };
        let rows = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["rows".to_owned()],
            ..ElementStyleKey::default()
        };
        let auto = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["auto".to_owned()],
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
        assert_eq!(
            computed_box_style(&style, &page).grid_template_columns,
            Some(2)
        );
        assert_eq!(
            computed_box_style(&style, &counter).grid_template_column_tracks,
            Some(vec![
                CssLength::Fr(1.0),
                CssLength::Auto,
                CssLength::Fr(1.0)
            ])
        );
        assert_eq!(
            computed_box_style(&style, &cards).grid_template_column_tracks,
            Some(vec![
                CssLength::Fr(1.0),
                CssLength::Fr(1.0),
                CssLength::Fr(1.0),
                CssLength::Fr(1.0),
            ])
        );
        assert_eq!(
            computed_box_style(&style, &page).grid_template_column_tracks,
            Some(vec![CssLength::Px(196.0), CssLength::Fr(1.0)])
        );
        assert_eq!(
            computed_box_style(&style, &auto).grid_template_columns,
            Some(8)
        );
        assert_eq!(
            computed_box_style(&style, &auto).grid_auto_repeat_min_column_width,
            Some(CssLength::Px(160.0))
        );
        assert_eq!(
            computed_box_style(&style, &page).grid_template_rows,
            Some(vec![CssLength::Auto, CssLength::Px(48.0)])
        );
        assert_eq!(
            computed_box_style(&style, &rows).grid_template_rows,
            Some(vec![CssLength::Px(40.0), CssLength::Percent(25.0)])
        );
    }

    #[test]
    fn parse_basic_css_carries_grid_template_shorthand_named_areas() {
        let style = parse_basic_css(
            r#"
            .page {
                display: grid;
                grid-template: "head head" 72px "side main" 1fr / 240px 1fr;
            }
            header { grid-area: head; }
            aside { grid-area: side; }
            main { grid-area: main; }
            "#,
        );
        let page = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["page".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &page);

        assert_eq!(
            computed.grid_template_areas,
            Some(vec![
                vec!["head".to_owned(), "head".to_owned()],
                vec!["side".to_owned(), "main".to_owned()],
            ])
        );
        assert_eq!(
            computed.grid_template_rows,
            Some(vec![CssLength::Px(72.0), CssLength::Fr(1.0)])
        );
        assert_eq!(
            computed.grid_template_column_tracks,
            Some(vec![CssLength::Px(240.0), CssLength::Fr(1.0)])
        );
    }

    #[test]
    fn parse_basic_css_carries_multiline_grid_template_shorthand_named_areas() {
        let style = parse_basic_css(
            r#"
            .page {
                display: grid;
                grid-template:
                    "head head" 64px
                    "side main" minmax(0, 1fr)
                    / 16rem minmax(0, 1fr);
            }
            "#,
        );
        let page = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["page".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &page);

        assert_eq!(
            computed.grid_template_areas,
            Some(vec![
                vec!["head".to_owned(), "head".to_owned()],
                vec!["side".to_owned(), "main".to_owned()],
            ])
        );
        assert_eq!(
            computed.grid_template_rows,
            Some(vec![CssLength::Px(64.0), CssLength::Fr(1.0)])
        );
        assert_eq!(
            computed.grid_template_column_tracks,
            Some(vec![CssLength::Px(256.0), CssLength::Fr(1.0)])
        );
    }

    #[test]
    fn parse_basic_css_carries_grid_auto_rows_and_item_spans() {
        let style = parse_basic_css(
            r#"
            .page { display: grid; grid-auto-rows: 96px; }
            .wide { grid-column: span 3; }
            .tall { grid-row: span 2; }
            "#,
        );
        let page = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["page".to_owned()],
            ..ElementStyleKey::default()
        };
        let wide = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["wide".to_owned()],
            ..ElementStyleKey::default()
        };
        let tall = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["tall".to_owned()],
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &page).grid_auto_rows,
            Some(CssLength::Px(96.0))
        );
        assert_eq!(computed_box_style(&style, &wide).grid_column_span, Some(3));
        assert_eq!(computed_box_style(&style, &tall).grid_row_span, Some(2));
    }

    #[test]
    fn parse_basic_css_preserves_auto_fit_minmax_track_floor() {
        let style = parse_basic_css(
            ".cards { grid-template-columns: repeat(auto-fit, minmax(min(18rem, 100%), 1fr)); } .zero { grid-template-columns: repeat(4, minmax(0, 1fr)); }",
        );
        let cards = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["cards".to_owned()],
            ..ElementStyleKey::default()
        };
        let zero = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["zero".to_owned()],
            ..ElementStyleKey::default()
        };

        assert!(matches!(
            computed_box_style(&style, &cards).grid_auto_repeat_min_column_width,
            Some(CssLength::Min(_, _))
        ));
        assert_eq!(
            computed_box_style(&style, &zero).grid_template_columns,
            Some(4)
        );
        assert_eq!(
            computed_box_style(&style, &zero).grid_auto_repeat_min_column_width,
            None
        );
    }

    #[test]
    fn parse_basic_css_uses_minmax_maximum_for_explicit_grid_tracks() {
        let style = parse_basic_css(
            ".results { grid-template-columns: minmax(0, 740px) 320px; } .header { grid-template-columns: 160px minmax(240px, 720px) 1fr; } .flexible { grid-template-columns: minmax(0, 1fr) 290px; }",
        );
        let results = ElementStyleKey {
            tag: "main".to_owned(),
            classes: vec!["results".to_owned()],
            ..ElementStyleKey::default()
        };
        let header = ElementStyleKey {
            tag: "header".to_owned(),
            classes: vec!["header".to_owned()],
            ..ElementStyleKey::default()
        };
        let flexible = ElementStyleKey {
            tag: "main".to_owned(),
            classes: vec!["flexible".to_owned()],
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &results).grid_template_column_tracks,
            Some(vec![CssLength::Px(740.0), CssLength::Px(320.0)])
        );
        assert_eq!(
            computed_box_style(&style, &header).grid_template_column_tracks,
            Some(vec![
                CssLength::Px(160.0),
                CssLength::Px(720.0),
                CssLength::Fr(1.0)
            ])
        );
        assert_eq!(
            computed_box_style(&style, &flexible).grid_template_column_tracks,
            Some(vec![CssLength::Fr(1.0), CssLength::Px(290.0)])
        );
    }

    #[test]
    fn parse_basic_css_preserves_viewport_and_function_lengths() {
        let style = parse_basic_css(
            ".hero { width: 50vw; height: 100vh; min-height: calc(100vh - 2rem); max-width: min(100vw, 72rem); }",
        );
        let hero = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["hero".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &hero);

        assert_eq!(computed.width, Some(CssLength::Vw(50.0)));
        assert_eq!(computed.height, Some(CssLength::Vh(100.0)));
        assert_eq!(
            computed.min_height,
            Some(CssLength::Calc(CssLengthExpression {
                px: -32.0,
                vh: 100.0,
                ..CssLengthExpression::default()
            }))
        );
        assert_eq!(
            computed.max_width,
            Some(CssLength::Min(
                CssLengthExpression {
                    vw: 100.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    px: 1152.0,
                    ..CssLengthExpression::default()
                },
            ))
        );
    }

    #[test]
    fn parse_basic_css_preserves_max_clamp_and_fit_content_lengths() {
        let style = parse_basic_css(
            ".hero { width: clamp(18rem, 60vw, 72rem); min-width: max(16rem, 50%); max-width: fit-content(44rem); }",
        );
        let hero = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["hero".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &hero);

        assert_eq!(
            computed.width,
            Some(CssLength::Clamp(
                CssLengthExpression {
                    px: 288.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    vw: 60.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    px: 1152.0,
                    ..CssLengthExpression::default()
                },
            ))
        );
        assert_eq!(
            computed.min_width,
            Some(CssLength::Max(
                CssLengthExpression {
                    px: 256.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    percent: 50.0,
                    ..CssLengthExpression::default()
                },
            ))
        );
        assert_eq!(
            computed.max_width,
            Some(CssLength::FitContent(CssLengthExpression {
                px: 704.0,
                ..CssLengthExpression::default()
            }))
        );
    }

    #[test]
    fn css_length_px_evaluates_max_clamp_and_fit_content() {
        assert_eq!(
            css_length_px(
                CssLength::Max(
                    CssLengthExpression {
                        px: 320.0,
                        ..CssLengthExpression::default()
                    },
                    CssLengthExpression {
                        percent: 50.0,
                        ..CssLengthExpression::default()
                    },
                ),
                900.0,
            ),
            450.0
        );
        assert_eq!(
            css_length_px(
                CssLength::Clamp(
                    CssLengthExpression {
                        px: 280.0,
                        ..CssLengthExpression::default()
                    },
                    CssLengthExpression {
                        percent: 75.0,
                        ..CssLengthExpression::default()
                    },
                    CssLengthExpression {
                        px: 560.0,
                        ..CssLengthExpression::default()
                    },
                ),
                1000.0,
            ),
            560.0
        );
        assert_eq!(
            css_length_px(
                CssLength::FitContent(CssLengthExpression {
                    px: 384.0,
                    ..CssLengthExpression::default()
                }),
                1000.0,
            ),
            384.0
        );
    }

    #[test]
    fn body_color_uses_general_color_parser() {
        let style = parse_basic_css("body { color: rgb(255, 255, 255); background: blue; }");

        assert_eq!(style.text_color, Color32::WHITE);
        assert_eq!(style.page_background, Color32::BLUE);
    }

    #[test]
    fn parse_basic_css_carries_named_grid_areas() {
        let style = parse_basic_css(
            r#"
            .grid {
                display: grid;
                grid-template-areas:
                    "side main"
                    "foot foot";
            }
            main { grid-area: main; }
            "#,
        );
        let grid = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["grid".to_owned()],
            ..ElementStyleKey::default()
        };
        let main = ElementStyleKey {
            tag: "main".to_owned(),
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &grid).grid_template_areas,
            Some(vec![
                vec!["side".to_owned(), "main".to_owned()],
                vec!["foot".to_owned(), "foot".to_owned()],
            ])
        );
        assert_eq!(
            computed_box_style(&style, &main).grid_area,
            Some("main".to_owned())
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
    fn css_custom_properties_resolve_nested_fallback_chains() {
        let style = parse_basic_css(
            r#"
            :root {
                --fallback-width: 22rem;
            }
            .card {
                width: var(--missing-width, var(--also-missing, var(--fallback-width)));
                color: var(--missing-color, var(--also-missing-color, #224466));
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["card".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.width, Some(CssLength::Px(352.0)));
        assert_eq!(computed.color, Some(Color32::from_rgb(0x22, 0x44, 0x66)));
    }

    #[test]
    fn css_custom_properties_resolve_from_scoped_declarations_in_same_rule() {
        let style = parse_basic_css(
            r#"
            :root {
                --panel-width: 20rem;
                --panel-color: #111111;
            }
            .panel {
                --panel-width: clamp(18rem, 50vw, 42rem);
                --panel-color: #abcdef;
                width: var(--panel-width);
                color: var(--panel-color);
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "section".to_owned(),
            classes: vec!["panel".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(
            computed.width,
            Some(CssLength::Clamp(
                CssLengthExpression {
                    px: 288.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    vw: 50.0,
                    ..CssLengthExpression::default()
                },
                CssLengthExpression {
                    px: 672.0,
                    ..CssLengthExpression::default()
                },
            ))
        );
        assert_eq!(computed.color, Some(Color32::from_rgb(0xab, 0xcd, 0xef)));
    }

    #[test]
    fn css_custom_properties_resolve_from_document_container_rules() {
        let style = parse_basic_css_for_viewport(
            r#"
            #app {
                --main-column: 654px;
                --left-gutter: 0px;
            }
            @media only screen and (min-width: 80rem) {
                #app { --left-gutter: 120px; }
            }
            .web {
                display: grid;
                grid-template-areas: "left-gutter mainline mid-gutter sidebar";
                grid-template-columns: var(--left-gutter) minmax(0, var(--main-column)) 40px minmax(0, 360px);
            }
            .mainline { grid-area: mainline; }
            "#,
            1280.0,
        );
        let web = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["web".to_owned()],
            ..ElementStyleKey::default()
        };

        let computed = computed_box_style(&style, &web);
        assert_eq!(
            computed.grid_template_column_tracks,
            Some(vec![
                CssLength::Px(120.0),
                CssLength::Px(654.0),
                CssLength::Px(40.0),
                CssLength::Px(360.0),
            ])
        );
    }

    #[test]
    fn grid_minmax_with_unresolved_var_does_not_collapse_to_zero_track() {
        let style = parse_basic_css(
            ".web { grid-template-columns: minmax(0, var(--missing-main-width)) 320px; }",
        );
        let web = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["web".to_owned()],
            ..ElementStyleKey::default()
        };

        assert_eq!(
            computed_box_style(&style, &web).grid_template_column_tracks,
            Some(vec![CssLength::Auto, CssLength::Px(320.0)])
        );
    }

    #[test]
    fn css_custom_properties_do_not_apply_dark_tokens_without_dark_root() {
        let style = parse_basic_css(
            r#"
            :root {
                --button-content: #333333;
            }
            .dark {
                --button-content: #ffffff;
            }
            button {
                color: var(--button-content);
            }
            "#,
        );
        let key = ElementStyleKey {
            tag: "button".to_owned(),
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.color, Some(Color32::from_rgb(0x33, 0x33, 0x33)));
    }

    #[test]
    fn css_custom_properties_apply_dark_tokens_for_dark_root() {
        let style = parse_basic_css_for_viewport_with_root_classes(
            r#"
            :root {
                --button-content: #333333;
            }
            .dark {
                --button-content: #ffffff;
            }
            html.dark {
                --button-background: #222222;
            }
            button {
                color: var(--button-content);
                background-color: var(--button-background);
            }
            "#,
            1280.0,
            &["dark".to_owned()],
        );
        let key = ElementStyleKey {
            tag: "button".to_owned(),
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(computed.color, Some(Color32::WHITE));
        assert_eq!(
            computed.background,
            Some(Color32::from_rgb(0x22, 0x22, 0x22))
        );
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
    fn later_box_edge_shorthand_clears_earlier_side_overrides() {
        let style = parse_basic_css(
            r#"
            .box { margin-top: 20px; padding-left: 30px; }
            .box { margin: 0; padding: 4px; }
            "#,
        );
        let key = ElementStyleKey {
            tag: "div".to_owned(),
            classes: vec!["box".to_owned()],
            ..ElementStyleKey::default()
        };
        let computed = computed_box_style(&style, &key);

        assert_eq!(
            computed.margin,
            Some(CssEdges {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0
            })
        );
        assert_eq!(computed.margin_top, None);
        assert_eq!(
            computed.padding,
            Some(CssEdges {
                top: 4.0,
                right: 4.0,
                bottom: 4.0,
                left: 4.0
            })
        );
        assert_eq!(computed.padding_left, None);
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
