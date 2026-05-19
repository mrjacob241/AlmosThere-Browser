use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eframe::{App, Frame, NativeOptions, egui};
use rich_canvas::{
    BrowserCanvas, BrowserCanvasResponse, BrowserDocument, BrowserStyle, CanvasBlock,
    CanvasButtonObject, CanvasClipObject, CanvasGraph, CanvasImageObject, CanvasInputKind,
    CanvasInputObject, CanvasMediaObject, CanvasObject, CanvasRectObject, CanvasSvgObject,
    CanvasTextObject,
    CssAlignItems, CssBoxStyle, CssDisplay, CssEdges, CssFlexDirection, CssJustifyContent,
    CssLength, CssObjectFit, CssPosition, CssTextAlign, ElementStyleKey, HitTarget, ImageBlock,
    InlineSpan, ResolvedBoxStyle, SvgBlock, SvgShape, computed_box_style, configure_browser_fonts,
    parse_basic_css_for_viewport_with_root_classes, parse_inline_box_style, wrap_browser_textboxes,
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

const APP_TITLE: &str = "AlmostThere Browser";
const DEFAULT_PAGE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sample_pages/test_basic_page.html"
);
const DEFAULT_URL: &str = "https://latex.vercel.app/elements";
const BOOKMARKS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../bookmarks.txt");
const DEBUG_EXPORT_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../target/render_debug_export");
const DEFAULT_BOOKMARK_TITLE: &str = "AlmostThere Sample Page";
const DEFAULT_URL_BOOKMARK_TITLE: &str = "HTML5 Test Page";
const LOCAL_BOOKMARK_TOKEN: &str = "[local]";
const SCRIPT_TEST_BOOKMARKS: &[(&str, &str)] = &[
    (
        "001 Basic Script Execution",
        "JustBarelyScript/UnitTest/001-basic-script-execution/index.html",
    ),
    (
        "002 Multiple Script Tags Execute In Order",
        "JustBarelyScript/UnitTest/002-multiple-script-tags-execute-in-order/index.html",
    ),
    (
        "003 Console Logging",
        "JustBarelyScript/UnitTest/003-console-logging/index.html",
    ),
    (
        "004 Element Creation",
        "JustBarelyScript/UnitTest/004-element-creation/index.html",
    ),
    (
        "005 CSS Class Assignment",
        "JustBarelyScript/UnitTest/005-css-class-assignment/index.html",
    ),
    (
        "006 setAttribute And getAttribute",
        "JustBarelyScript/UnitTest/006-setattribute-and-getattribute/index.html",
    ),
    (
        "007 innerHTML Basic Replacement",
        "JustBarelyScript/UnitTest/007-innerhtml-basic-replacement/index.html",
    ),
    (
        "008 Query Selector By ID",
        "JustBarelyScript/UnitTest/008-query-selector-by-id/index.html",
    ),
    (
        "009 Query Selector By Class",
        "JustBarelyScript/UnitTest/009-query-selector-by-class/index.html",
    ),
    (
        "010 querySelectorAll And Length",
        "JustBarelyScript/UnitTest/010-queryselectorall-and-length/index.html",
    ),
    (
        "011 For Loop DOM Update",
        "JustBarelyScript/UnitTest/011-for-loop-dom-update/index.html",
    ),
    (
        "012 Event Listener Click",
        "JustBarelyScript/UnitTest/012-event-listener-click/index.html",
    ),
    (
        "013 Event Object Target",
        "JustBarelyScript/UnitTest/013-event-object-target/index.html",
    ),
    (
        "014 Input Value Reading",
        "JustBarelyScript/UnitTest/014-input-value-reading/index.html",
    ),
    (
        "015 Input Event",
        "JustBarelyScript/UnitTest/015-input-event/index.html",
    ),
    (
        "016 Style Property Mutation",
        "JustBarelyScript/UnitTest/016-style-property-mutation/index.html",
    ),
    (
        "017 Computed Style Smoke Test",
        "JustBarelyScript/UnitTest/017-computed-style-smoke-test/index.html",
    ),
    (
        "018 setTimeout",
        "JustBarelyScript/UnitTest/018-settimeout/index.html",
    ),
    (
        "019 Promise Microtask",
        "JustBarelyScript/UnitTest/019-promise-microtask/index.html",
    ),
    (
        "020 JSON Parse And Stringify",
        "JustBarelyScript/UnitTest/020-json-parse-and-stringify/index.html",
    ),
    (
        "021 Array Operations",
        "JustBarelyScript/UnitTest/021-array-operations/index.html",
    ),
    (
        "022 Object Literals And Properties",
        "JustBarelyScript/UnitTest/022-object-literals-and-properties/index.html",
    ),
    (
        "023 Closures In Event Handlers",
        "JustBarelyScript/UnitTest/023-closures-in-event-handlers/index.html",
    ),
    (
        "024 DOMContentLoaded",
        "JustBarelyScript/UnitTest/024-domcontentloaded/index.html",
    ),
    (
        "025 Minimal Todo App",
        "JustBarelyScript/UnitTest/025-minimal-todo-app/index.html",
    ),
    (
        "026 Decorator Skip",
        "JustBarelyScript/UnitTest/026-decorator-skip/index.html",
    ),
    (
        "027 XOR Operator",
        "JustBarelyScript/UnitTest/027-xor-operator/index.html",
    ),
    (
        "028 Increment Decrement",
        "JustBarelyScript/UnitTest/028-increment-decrement/index.html",
    ),
    (
        "029 Compound Assignment",
        "JustBarelyScript/UnitTest/029-compound-assignment/index.html",
    ),
    (
        "030 Nullish Coalescing",
        "JustBarelyScript/UnitTest/030-nullish-coalescing/index.html",
    ),
    (
        "031 Default Parameters",
        "JustBarelyScript/UnitTest/031-default-parameters/index.html",
    ),
    (
        "032 Arrow Functions",
        "JustBarelyScript/UnitTest/032-arrow-functions/index.html",
    ),
    (
        "033 Spread Operator",
        "JustBarelyScript/UnitTest/033-spread-operator/index.html",
    ),
    (
        "034 Optional Chaining",
        "JustBarelyScript/UnitTest/034-optional-chaining/index.html",
    ),
    (
        "035 Template Literals",
        "JustBarelyScript/UnitTest/035-template-literals/index.html",
    ),
    (
        "036 Try Catch Finally",
        "JustBarelyScript/UnitTest/036-try-catch-finally/index.html",
    ),
    (
        "037 For Of",
        "JustBarelyScript/UnitTest/037-for-of/index.html",
    ),
    (
        "038 Class Syntax",
        "JustBarelyScript/UnitTest/038-class-syntax/index.html",
    ),
    (
        "039 ES Modules",
        "JustBarelyScript/UnitTest/039-es-modules/index.html",
    ),
    (
        "040 Async Await",
        "JustBarelyScript/UnitTest/040-async-await/index.html",
    ),
    (
        "041 Proxy",
        "JustBarelyScript/UnitTest/041-proxy/index.html",
    ),
];

fn main() -> eframe::Result<()> {
    let config = AppConfig::from_args();
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 720.0])
            .with_title(APP_TITLE),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(AlmostThereApp::new(cc, config)))),
    )
}

#[derive(Clone, Copy, Debug, Default)]
struct AppConfig {
    record_events: bool,
}

impl AppConfig {
    fn from_args() -> Self {
        Self {
            record_events: std::env::args().any(|arg| arg == "--record-events"),
        }
    }
}

struct AlmostThereApp {
    canvas: BrowserCanvas,
    debug_canvas: BrowserCanvas,
    document: BrowserDocument,
    current_html: String,
    live_html: String,
    script_state: justbarelyscript::BrowserExecutionState,
    last_hovered_element_id: Option<String>,
    render_graph_debug_text: String,
    url_input: String,
    bookmarks: Vec<Bookmark>,
    console_messages: Vec<justbarelyscript::ConsoleMessage>,
    live_js_debug_text: String,
    status: String,
    telemetry: TelemetrySession,
    text_metrics_ready: bool,
    pending_navigation: Option<PendingNavigation>,
    render_debug: PageRenderDebugState,
    page_loaded_at: std::time::Instant,
    record_events: bool,
    recorded_event_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Bookmark {
    title: String,
    url: String,
}

struct PendingNavigation {
    url: String,
    fragment: Option<String>,
    receiver: Receiver<io::Result<LoadedPageSource>>,
}

struct LoadedPageSource {
    html: String,
    source: String,
}

#[derive(Clone, Debug, Default)]
struct PageRenderDebugState {
    open: bool,
    object_limit: usize,
    active_tab: DebugPanelTab,
    /// Staged input values for the Events tab, keyed by element id.
    event_staged_values: std::collections::HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum DebugPanelTab {
    #[default]
    RenderGraph,
    CanvasGraph,
    Html,
    Console,
    LiveJs,
    Events,
}

impl AlmostThereApp {
    fn new(cc: &eframe::CreationContext<'_>, config: AppConfig) -> Self {
        configure_browser_fonts(&cc.egui_ctx);
        let telemetry = TelemetrySession::start().unwrap_or_else(|_| TelemetrySession::disabled());
        install_global_telemetry(&telemetry);
        install_telemetry_panic_hook();
        telemetry.emit(
            "session.started",
            &[
                ("app", APP_TITLE),
                (
                    "record_events",
                    if config.record_events {
                        "true"
                    } else {
                        "false"
                    },
                ),
            ],
        );
        if config.record_events {
            telemetry.emit(
                "event_recording.started",
                &[("source", "cli"), ("flag", "--record-events")],
            );
        }

        let mut bookmarks = load_bookmarks().unwrap_or_default();
        let inserted_default_bookmark = ensure_bookmark(&mut bookmarks, default_sample_bookmark())
            | ensure_bookmark(&mut bookmarks, default_url_bookmark());
        if inserted_default_bookmark {
            let _ = save_bookmarks(&bookmarks);
        }

        let (
            document,
            current_html,
            live_html,
            script_state,
            render_graph_debug_text,
            console_messages,
            live_js_debug_text,
            status,
        ) = match load_url_source(DEFAULT_URL) {
            Ok(source) => {
                let document =
                    parse_html_document_with_text_metrics(&source.html, &source.source, None);
                let live_html = apply_safe_script_browser_effects_with_source(
                    &remove_html_comments(&source.html),
                    Some(&source.source),
                );
                let script_state =
                    build_script_state_with_source(&source.html, Some(&source.source));
                let render_graph_debug_text =
                    parse_render_graph_debug_dump(&source.html, &source.source);
                let console_messages = script_console_messages_from_html_with_source(
                    &source.html,
                    Some(&source.source),
                );
                let live_js_debug_text = live_js_debug_report(&source.html, Some(&source.source));
                telemetry.emit(
                    "navigation.loaded",
                    &[
                        ("url", DEFAULT_URL),
                        ("title", &document.title),
                        ("blocks", &document.blocks.len().to_string()),
                    ],
                );
                (
                    document,
                    source.html,
                    live_html,
                    script_state,
                    render_graph_debug_text,
                    console_messages,
                    live_js_debug_text,
                    format!("Loaded {DEFAULT_URL}"),
                )
            }
            Err(error) => {
                telemetry.emit(
                    "navigation.failed",
                    &[("url", DEFAULT_URL), ("error", &error.to_string())],
                );
                let document = BrowserDocument {
                    title: "Load failed".to_owned(),
                    source: DEFAULT_URL.to_owned(),
                    style: Default::default(),
                    canvas_graph: CanvasGraph::default(),
                    blocks: vec![CanvasBlock::Paragraph {
                        text: format!("Failed to load default page {DEFAULT_URL}: {error}"),
                    }],
                };
                (
                    document,
                    String::new(),
                    String::new(),
                    justbarelyscript::BrowserExecutionState::default(),
                    String::new(),
                    vec![console_error_message(format!(
                        "Failed to load default page {DEFAULT_URL}: {error}"
                    ))],
                    String::new(),
                    format!("Failed to load default page {DEFAULT_URL}: {error}"),
                )
            }
        };

        let render_debug = PageRenderDebugState {
            open: false,
            object_limit: document.canvas_graph.objects.len(),
            active_tab: DebugPanelTab::RenderGraph,
            event_staged_values: std::collections::HashMap::new(),
        };

        Self {
            canvas: BrowserCanvas::new(),
            debug_canvas: BrowserCanvas::new(),
            document,
            current_html,
            live_html,
            script_state,
            last_hovered_element_id: None,
            render_graph_debug_text,
            url_input: DEFAULT_URL.to_owned(),
            bookmarks,
            console_messages,
            live_js_debug_text,
            status,
            telemetry,
            text_metrics_ready: false,
            pending_navigation: None,
            render_debug,
            page_loaded_at: std::time::Instant::now(),
            record_events: config.record_events,
            recorded_event_count: 0,
        }
    }

    fn load_current_input(&mut self, ctx: &egui::Context) {
        let input = self.url_input.trim();

        self.telemetry
            .emit("navigation.requested", &[("url", input)]);
        self.status = format!("Loading {input}...");
        ctx.request_repaint();

        self.start_navigation(input.to_owned(), None);
    }

    fn start_navigation(&mut self, url: String, fragment: Option<String>) {
        let (sender, receiver) = mpsc::channel();
        let thread_url = url.clone();
        thread::spawn(move || {
            emit_global_telemetry("navigation.fetch.started", &[("url", &thread_url)]);
            let result = load_url_source(&thread_url);
            match &result {
                Ok(source) => emit_global_telemetry(
                    "navigation.fetch.completed",
                    &[
                        ("url", &thread_url),
                        ("final_url", &source.source),
                        ("html_bytes", &source.html.len().to_string()),
                    ],
                ),
                Err(error) => emit_global_telemetry(
                    "navigation.fetch.failed",
                    &[("url", &thread_url), ("error", &error.to_string())],
                ),
            }
            let _ = sender.send(result);
        });
        self.pending_navigation = Some(PendingNavigation {
            url,
            fragment,
            receiver,
        });
    }

    fn poll_pending_navigation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_navigation.as_ref() else {
            return;
        };

        match pending.receiver.try_recv() {
            Ok(result) => {
                let pending = self.pending_navigation.take().expect("pending navigation");
                match result {
                    Ok(source) => {
                        self.telemetry.emit(
                            "navigation.scripts.started",
                            &[
                                ("url", &source.source),
                                ("html_bytes", &source.html.len().to_string()),
                            ],
                        );
                        self.current_html = source.html.clone();
                        self.live_html = apply_safe_script_browser_effects_with_source(
                            &remove_html_comments(&source.html),
                            Some(&source.source),
                        );
                        self.telemetry.emit(
                            "navigation.scripts.completed",
                            &[
                                ("url", &source.source),
                                ("live_html_bytes", &self.live_html.len().to_string()),
                            ],
                        );
                        self.telemetry.emit(
                            "navigation.script_state.started",
                            &[("url", &source.source)],
                        );
                        self.script_state =
                            build_script_state_with_source(&source.html, Some(&source.source));
                        self.telemetry.emit(
                            "navigation.script_state.completed",
                            &[("url", &source.source)],
                        );
                        self.page_loaded_at = std::time::Instant::now();
                        self.last_hovered_element_id = None;
                        self.telemetry.emit(
                            "navigation.render_graph.started",
                            &[("url", &source.source)],
                        );
                        self.render_graph_debug_text =
                            parse_render_graph_debug_dump(&source.html, &source.source);
                        self.telemetry.emit(
                            "navigation.render_graph.completed",
                            &[
                                ("url", &source.source),
                                ("bytes", &self.render_graph_debug_text.len().to_string()),
                            ],
                        );
                        self.telemetry
                            .emit("navigation.console.started", &[("url", &source.source)]);
                        self.console_messages = script_console_messages_from_html_with_source(
                            &source.html,
                            Some(&source.source),
                        );
                        self.telemetry.emit(
                            "navigation.console.completed",
                            &[
                                ("url", &source.source),
                                ("messages", &self.console_messages.len().to_string()),
                            ],
                        );
                        self.telemetry.emit(
                            "navigation.live_js_debug.started",
                            &[("url", &source.source)],
                        );
                        self.live_js_debug_text =
                            live_js_debug_report(&source.html, Some(&source.source));
                        self.telemetry.emit(
                            "navigation.live_js_debug.completed",
                            &[
                                ("url", &source.source),
                                ("bytes", &self.live_js_debug_text.len().to_string()),
                            ],
                        );
                        self.telemetry.emit(
                            "navigation.canvas_graph.started",
                            &[("url", &source.source)],
                        );
                        let document = parse_html_document_with_text_metrics(
                            &source.html,
                            &source.source,
                            Some(ctx),
                        );
                        self.telemetry.emit(
                            "navigation.canvas_graph.completed",
                            &[
                                ("url", &source.source),
                                ("blocks", &document.blocks.len().to_string()),
                                (
                                    "canvas_objects",
                                    &document.canvas_graph.objects.len().to_string(),
                                ),
                            ],
                        );
                        self.telemetry.emit(
                            "navigation.loaded",
                            &[
                                ("url", &pending.url),
                                ("title", &document.title),
                                ("blocks", &document.blocks.len().to_string()),
                            ],
                        );
                        self.url_input = document.source.clone();
                        self.status = format!("Loaded {}", document.source);
                        self.document = document;
                        self.canvas.scroll_offset = egui::Vec2::ZERO;
                        self.debug_canvas.scroll_offset = egui::Vec2::ZERO;
                        self.render_debug.object_limit = self.document.canvas_graph.objects.len();
                        if let Some(fragment) = pending.fragment {
                            self.scroll_to_fragment(&fragment);
                        }
                    }
                    Err(error) => {
                        self.telemetry.emit(
                            "navigation.failed",
                            &[("url", &pending.url), ("error", &error.to_string())],
                        );
                        self.status = format!("Load failed: {error}");
                        self.console_messages
                            .push(console_error_message(format!("Load failed: {error}")));
                        self.live_js_debug_text.clear();
                    }
                }
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(TryRecvError::Disconnected) => {
                let pending = self.pending_navigation.take().expect("pending navigation");
                self.status = format!("Load failed: loader stopped for {}", pending.url);
                self.console_messages.push(console_error_message(format!(
                    "Load failed: loader stopped for {}",
                    pending.url
                )));
                ctx.request_repaint();
            }
        }
    }

    fn current_url(&self) -> String {
        self.url_input.trim().to_owned()
    }

    fn current_bookmark_index(&self) -> Option<usize> {
        let current_url = self.current_url();
        self.bookmarks
            .iter()
            .position(|bookmark| bookmark.url == current_url)
    }

    fn toggle_current_bookmark(&mut self) {
        let current_url = self.current_url();
        if current_url.is_empty() {
            return;
        }

        if let Some(index) = self.current_bookmark_index() {
            let removed = self.bookmarks.remove(index);
            self.telemetry
                .emit("bookmark.removed", &[("url", &removed.url)]);
            self.status = format!("Removed bookmark: {}", removed.title);
        } else {
            let title = if self.document.title.trim().is_empty() {
                current_url.clone()
            } else {
                self.document.title.clone()
            };
            self.bookmarks.push(Bookmark {
                title: title.clone(),
                url: current_url.clone(),
            });
            self.telemetry.emit(
                "bookmark.added",
                &[("url", &current_url), ("title", &title)],
            );
            self.status = format!("Bookmarked: {title}");
        }

        if let Err(error) = save_bookmarks(&self.bookmarks) {
            self.status = format!("Bookmark save failed: {error}");
        }
    }

    fn open_bookmark(&mut self, index: usize, ctx: &egui::Context) {
        let Some(bookmark) = self.bookmarks.get(index).cloned() else {
            return;
        };
        self.open_bookmark_value(bookmark, ctx);
    }

    fn open_bookmark_value(&mut self, bookmark: Bookmark, ctx: &egui::Context) {
        self.url_input = bookmark.url;
        self.telemetry.emit(
            "bookmark.opened",
            &[("url", &self.url_input), ("title", &bookmark.title)],
        );
        self.load_current_input(ctx);
    }

    fn open_link(&mut self, href: &str, ctx: &egui::Context) {
        let href = href.trim();
        if href.is_empty()
            || href.starts_with("javascript:")
            || href.starts_with("mailto:")
            || href.starts_with("tel:")
        {
            self.status = format!("Unsupported link: {href}");
            return;
        }

        let resolved = resolve_navigation_url(&self.document.source, href);
        let fragment = url_fragment(&resolved);
        if same_document_url(&self.document.source, &resolved) {
            self.url_input = resolved;
            if let Some(fragment) = fragment {
                self.scroll_to_fragment(&fragment);
                self.status = format!("Jumped to #{fragment}");
            } else {
                self.canvas.scroll_offset = egui::Vec2::ZERO;
                self.status = "Jumped to top".to_owned();
            }
            return;
        }

        self.url_input = resolved.clone();
        self.telemetry
            .emit("navigation.requested", &[("url", &resolved)]);
        self.status = format!("Loading {resolved}...");
        ctx.request_repaint();
        self.start_navigation(resolved, fragment);
    }

    fn scroll_to_fragment(&mut self, fragment: &str) {
        self.canvas.scroll_offset.y = estimated_fragment_scroll_y(&self.document, fragment);
    }

    fn report_user_error(&mut self) {
        let console_errors = self
            .console_messages
            .iter()
            .filter(|message| message.level == justbarelyscript::ConsoleLevel::Error)
            .count();
        let budget_stops = self
            .live_js_debug_text
            .matches("statement budget exhausted")
            .count();
        let skipped_scripts = self
            .live_js_debug_text
            .matches("status: not executed")
            .count();
        self.telemetry.emit(
            "user.error",
            &[
                ("url", &self.document.source),
                ("title", &self.document.title),
                ("status", &self.status),
                ("blocks", &self.document.blocks.len().to_string()),
                (
                    "canvas_objects",
                    &self.document.canvas_graph.objects.len().to_string(),
                ),
                ("console_errors", &console_errors.to_string()),
                ("script_budget_stops", &budget_stops.to_string()),
                ("skipped_scripts", &skipped_scripts.to_string()),
            ],
        );
        self.status = format!("Recorded user.error telemetry for {}", self.document.source);
    }

    fn record_frame_events(&mut self, ctx: &egui::Context) {
        if !self.record_events {
            return;
        }

        let events = ctx.input(|input| input.events.clone());
        for event in events {
            self.recorded_event_count += 1;
            let sequence = self.recorded_event_count.to_string();
            let (kind, detail) = telemetry_event_kind_and_detail(&event);
            self.telemetry.emit(
                "input.event",
                &[
                    ("seq", &sequence),
                    ("kind", kind),
                    ("detail", &detail),
                    ("url", &self.document.source),
                ],
            );
        }
    }

    fn apply_script_effects(
        &mut self,
        effects: Vec<justbarelyscript::BrowserEffect>,
        ctx: &egui::Context,
    ) {
        if effects.is_empty() {
            return;
        }

        // Snapshot live input values so the re-parse doesn't reset what the user typed.
        let live_input_values: std::collections::HashMap<String, String> = self
            .document
            .canvas_graph
            .objects
            .iter()
            .filter_map(|obj| {
                if let CanvasObject::Input(input) = obj {
                    input
                        .element_id
                        .as_ref()
                        .map(|id| (id.clone(), input.value.clone()))
                } else {
                    None
                }
            })
            .collect();

        for effect in effects {
            match effect {
                justbarelyscript::BrowserEffect::SetTextContent { element_id, value } => {
                    self.live_html =
                        set_element_text_content_by_id(&self.live_html, &element_id, &value);
                }
                justbarelyscript::BrowserEffect::SetAttribute {
                    element_id,
                    name,
                    value,
                } => {
                    self.live_html =
                        set_element_attribute_by_id(&self.live_html, &element_id, &name, &value);
                }
                justbarelyscript::BrowserEffect::SetInnerHtml { element_id, value } => {
                    self.live_html =
                        set_element_inner_html_by_id(&self.live_html, &element_id, &value);
                }
                justbarelyscript::BrowserEffect::AppendChild { parent_id, child } => {
                    self.live_html = append_child_html_by_id(&self.live_html, &parent_id, &child);
                }
                justbarelyscript::BrowserEffect::ConsoleLog { level, text } => {
                    self.console_messages
                        .push(justbarelyscript::ConsoleMessage {
                            level: match level.as_str() {
                                "warn" => justbarelyscript::ConsoleLevel::Warn,
                                "error" => justbarelyscript::ConsoleLevel::Error,
                                "info" => justbarelyscript::ConsoleLevel::Info,
                                _ => justbarelyscript::ConsoleLevel::Log,
                            },
                            text,
                        });
                }
            }
        }

        self.document = parse_html_document_from_live_html(
            &self.live_html,
            &self.document.source.clone(),
            Some(ctx),
        );

        // Restore live input values that the re-parse reset to their HTML attribute defaults.
        for obj in &mut self.document.canvas_graph.objects {
            if let CanvasObject::Input(input) = obj {
                if let Some(id) = &input.element_id {
                    if let Some(live_value) = live_input_values.get(id) {
                        input.value = live_value.clone();
                    }
                }
            }
        }

        ctx.request_repaint();
    }

    fn debug_canvas_graph(&self) -> CanvasGraph {
        let mut graph = self.document.canvas_graph.clone();
        let limit = self.render_debug.object_limit.min(graph.objects.len());
        graph.objects.truncate(limit);
        close_truncated_canvas_clips(&mut graph.objects);
        graph
    }
}

fn close_truncated_canvas_clips(objects: &mut Vec<CanvasObject>) {
    let mut open_clips = 0usize;
    for object in objects.iter() {
        match object {
            CanvasObject::ClipStart(_) => open_clips += 1,
            CanvasObject::ClipEnd => {
                open_clips = open_clips.saturating_sub(1);
            }
            CanvasObject::Text(_)
            | CanvasObject::Rect(_)
            | CanvasObject::Button(_)
            | CanvasObject::Input(_)
            | CanvasObject::Image(_)
            | CanvasObject::Svg(_)
            | CanvasObject::Media(_) => {}
        }
    }
    objects.extend((0..open_clips).map(|_| CanvasObject::ClipEnd));
}

fn canvas_graph_debug_string(graph: &CanvasGraph) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "CanvasGraph viewport={:.1}x{:.1} objects={}",
        graph.viewport.x,
        graph.viewport.y,
        graph.objects.len()
    );
    for (index, object) in graph.objects.iter().enumerate() {
        match object {
            CanvasObject::Text(text) => {
                let _ = writeln!(
                    out,
                    "{index:04} Text rect={} size={:.1} bold={} italic={} underline={} href={} text=\"{}\"",
                    rect_debug(text.rect),
                    text.font_size,
                    text.font_weight_bold,
                    text.font_style_italic,
                    text.text_decoration_underline,
                    text.href.as_deref().unwrap_or(""),
                    shorten_debug_text(&text.text)
                );
            }
            CanvasObject::Rect(rect) => {
                let _ = writeln!(
                    out,
                    "{index:04} Rect rect={} fill={} border={} border_width={:.1} radius={}",
                    rect_debug(rect.rect),
                    color_debug(rect.fill),
                    color_debug(rect.border_color),
                    rect.border_width,
                    rect.border_radius
                );
            }
            CanvasObject::Button(button) => {
                let _ = writeln!(
                    out,
                    "{index:04} Button rect={} type={} form={} text=\"{}\"",
                    rect_debug(button.rect),
                    button.button_type,
                    button.form_id.as_deref().unwrap_or(""),
                    shorten_debug_text(&button.text)
                );
            }
            CanvasObject::Input(input) => {
                let _ = writeln!(
                    out,
                    "{index:04} Input rect={} font_size={:.1} color={} label=\"{}\" value_len={}",
                    rect_debug(input.rect),
                    input.font_size,
                    color_debug(input.color),
                    shorten_debug_text(&input.label),
                    input.value.chars().count()
                );
            }
            CanvasObject::Image(image) => {
                let _ = writeln!(
                    out,
                    "{index:04} Image rect={} fit={:?} src=\"{}\" alt=\"{}\" intrinsic={:.1}x{:.1}",
                    rect_debug(image.rect),
                    image.object_fit,
                    shorten_debug_text(&image.src),
                    shorten_debug_text(&image.alt),
                    image.image.size.x,
                    image.image.size.y
                );
            }
            CanvasObject::Svg(svg) => {
                let _ = writeln!(
                    out,
                    "{index:04} Svg rect={} intrinsic={:.1}x{:.1} shapes={}",
                    rect_debug(svg.rect),
                    svg.svg.size.x,
                    svg.svg.size.y,
                    svg.svg.shapes.len()
                );
            }
            CanvasObject::Media(media) => {
                let _ = writeln!(
                    out,
                    "{index:04} Media rect={} label=\"{}\"",
                    rect_debug(media.rect),
                    shorten_debug_text(&media.label)
                );
            }
            CanvasObject::ClipStart(clip) => {
                let _ = writeln!(
                    out,
                    "{index:04} ClipStart rect={} radius={}",
                    rect_debug(clip.rect),
                    clip.border_radius
                );
            }
            CanvasObject::ClipEnd => {
                let _ = writeln!(out, "{index:04} ClipEnd");
            }
        }
    }
    out
}

fn rect_debug(rect: egui::Rect) -> String {
    format!(
        "({:.1},{:.1}) {:.1}x{:.1}",
        rect.left(),
        rect.top(),
        rect.width(),
        rect.height()
    )
}

fn paint_alternating_debug_text(ui: &mut egui::Ui, text: &str) {
    paint_alternating_debug_text_with_canvas_thumbs(ui, text, None);
}

fn paint_console_messages(ui: &mut egui::Ui, messages: &[justbarelyscript::ConsoleMessage]) {
    egui::ScrollArea::vertical()
        .id_salt("debug_console_messages")
        .auto_shrink(false)
        .show(ui, |ui| {
            if messages.is_empty() {
                ui.label("Console is empty.");
                return;
            }

            for (index, message) in messages.iter().enumerate() {
                let is_error = message.level == justbarelyscript::ConsoleLevel::Error;
                let fill = if is_error {
                    egui::Color32::from_rgb(255, 244, 179)
                } else if index % 2 == 0 {
                    egui::Color32::from_rgb(248, 249, 251)
                } else {
                    egui::Color32::from_rgb(237, 240, 244)
                };
                let text_color = if is_error {
                    egui::Color32::from_rgb(176, 0, 32)
                } else {
                    ui.visuals().text_color()
                };
                let level = match message.level {
                    justbarelyscript::ConsoleLevel::Log => "log",
                    justbarelyscript::ConsoleLevel::Info => "info",
                    justbarelyscript::ConsoleLevel::Warn => "warn",
                    justbarelyscript::ConsoleLevel::Error => "error",
                };

                egui::Frame::new()
                    .fill(fill)
                    .inner_margin(egui::Margin::symmetric(8, 5))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.monospace(format!("[{level}]"));
                            ui.label(egui::RichText::new(&message.text).color(text_color));
                        });
                    });
            }
        });
}

fn paint_alternating_debug_text_with_canvas_thumbs(
    ui: &mut egui::Ui,
    text: &str,
    canvas_graph: Option<&CanvasGraph>,
) {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace).max(18.0);
    let char_width = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap("0".to_owned(), font_id.clone(), egui::Color32::WHITE)
            .size()
            .x
            .max(1.0)
    });
    let text_color = ui.visuals().text_color();
    let even_fill = egui::Color32::from_rgb(248, 249, 251);
    let odd_fill = egui::Color32::from_rgb(237, 240, 244);
    let width = ui.available_width().max(1.0);
    let mut in_quote = false;

    for (index, line) in text.lines().enumerate() {
        let thumb = canvas_graph.and_then(|graph| debug_line_thumbnail(graph, line));
        let thumb_slot_width = if thumb.is_some() { 42.0 } else { 0.0 };
        let text_width = (width - 12.0 - thumb_slot_width).max(1.0);
        let chars_per_line = (text_width / char_width).floor().max(1.0) as usize;
        let visual_lines = wrap_debug_line(line, chars_per_line);
        let text_height = row_height * visual_lines.len().max(1) as f32;
        let height = if thumb.is_some() {
            text_height.max(38.0)
        } else {
            text_height
        };
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
        let fill = if index % 2 == 0 { even_fill } else { odd_fill };
        ui.painter().rect_filled(rect, 0.0, fill);
        response.context_menu(|ui| {
            if ui.button("Copy Line").clicked() {
                ui.ctx().copy_text(line.to_owned());
                ui.close();
            }
        });
        for (line_index, visual_line) in visual_lines.iter().enumerate() {
            let display_line = debug_line_with_color_spacing(visual_line);
            let text_pos =
                rect.left_top() + egui::vec2(6.0, row_height * (line_index as f32 + 0.5));
            paint_debug_colored_line(
                ui,
                &display_line,
                text_pos,
                &font_id,
                text_color,
                char_width,
                &mut in_quote,
            );
            paint_debug_hex_swatches(
                ui,
                &display_line,
                text_pos,
                row_height,
                char_width,
                font_id.size * 0.2,
            );
        }
        if let Some(thumb) = thumb {
            paint_debug_line_thumbnail(ui, rect, thumb);
        }
    }

    if text.is_empty() {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, row_height), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, even_fill);
    }
}

enum DebugLineThumbnail<'a> {
    Image {
        index: usize,
        src: &'a str,
        image: &'a ImageBlock,
    },
    Svg(&'a SvgBlock),
}

fn debug_line_thumbnail<'a>(graph: &'a CanvasGraph, line: &str) -> Option<DebugLineThumbnail<'a>> {
    let index = parse_canvas_debug_line_index(line)?;
    match graph.objects.get(index)? {
        CanvasObject::Image(image) => Some(DebugLineThumbnail::Image {
            index,
            src: &image.src,
            image: &image.image,
        }),
        CanvasObject::Svg(svg) => Some(DebugLineThumbnail::Svg(&svg.svg)),
        _ => None,
    }
}

fn parse_canvas_debug_line_index(line: &str) -> Option<usize> {
    let prefix = line.get(0..4)?;
    if !prefix.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    if line.as_bytes().get(4).is_some_and(|byte| *byte != b' ') {
        return None;
    }
    prefix.parse().ok()
}

fn paint_debug_line_thumbnail(
    ui: &mut egui::Ui,
    row_rect: egui::Rect,
    thumb: DebugLineThumbnail<'_>,
) {
    let size = egui::vec2(34.0, 28.0);
    let rect = egui::Rect::from_center_size(
        egui::pos2(row_rect.right() - 24.0, row_rect.center().y),
        size,
    );
    ui.painter().rect_filled(
        rect.expand(2.0),
        3.0,
        egui::Color32::from_rgb(225, 229, 235),
    );
    ui.painter().rect_stroke(
        rect.expand(2.0),
        3.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 158, 168)),
        egui::StrokeKind::Outside,
    );

    match thumb {
        DebugLineThumbnail::Image { index, src, image } => {
            let texture = ui.ctx().load_texture(
                format!("render-debug-thumb-{index}-{src}"),
                image.color_image.clone(),
                egui::TextureOptions::LINEAR,
            );
            let image_rect = fit_rect_into(image.size, rect);
            ui.painter().image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        DebugLineThumbnail::Svg(svg) => {
            let svg_rect = fit_rect_into(svg.size, rect);
            svg.paint_in_rect(ui, svg_rect);
        }
    }
}

fn fit_rect_into(content_size: egui::Vec2, bounds: egui::Rect) -> egui::Rect {
    let content_size = content_size.max(egui::Vec2::splat(1.0));
    let scale = (bounds.width() / content_size.x)
        .min(bounds.height() / content_size.y)
        .min(1.0)
        .max(0.01);
    egui::Rect::from_center_size(bounds.center(), content_size * scale)
}

fn paint_debug_colored_line(
    ui: &mut egui::Ui,
    line: &str,
    text_pos: egui::Pos2,
    font_id: &egui::FontId,
    default_color: egui::Color32,
    char_width: f32,
    in_quote: &mut bool,
) {
    let quote_color = egui::Color32::from_rgb(190, 35, 45);
    let mut segment_start = 0usize;
    let mut segment_start_char = 0usize;

    for (byte_index, character) in line.char_indices() {
        if character != '"' {
            continue;
        }

        if *in_quote {
            let end = byte_index + character.len_utf8();
            paint_debug_line_segment(
                ui,
                &line[segment_start..end],
                text_pos,
                font_id,
                quote_color,
                segment_start_char,
                char_width,
            );
            segment_start = end;
            segment_start_char = line[..end].chars().count();
            *in_quote = false;
        } else {
            paint_debug_line_segment(
                ui,
                &line[segment_start..byte_index],
                text_pos,
                font_id,
                default_color,
                segment_start_char,
                char_width,
            );
            segment_start = byte_index;
            segment_start_char = line[..byte_index].chars().count();
            *in_quote = true;
        }
    }

    if segment_start < line.len() {
        paint_debug_line_segment(
            ui,
            &line[segment_start..],
            text_pos,
            font_id,
            if *in_quote {
                quote_color
            } else {
                default_color
            },
            segment_start_char,
            char_width,
        );
    }
}

fn paint_debug_line_segment(
    ui: &mut egui::Ui,
    segment: &str,
    text_pos: egui::Pos2,
    font_id: &egui::FontId,
    color: egui::Color32,
    start_char: usize,
    char_width: f32,
) {
    if segment.is_empty() {
        return;
    }
    ui.painter().text(
        text_pos + egui::vec2(start_char as f32 * char_width, 0.0),
        egui::Align2::LEFT_CENTER,
        segment,
        font_id.clone(),
        color,
    );
}

fn wrap_debug_line(line: &str, chars_per_line: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }

    let mut wrapped = Vec::new();
    let mut current = String::new();
    for character in line.chars() {
        if current.chars().count() >= chars_per_line {
            wrapped.push(current);
            current = String::new();
        }
        current.push(character);
    }
    if !current.is_empty() {
        wrapped.push(current);
    }
    wrapped
}

fn debug_line_with_color_spacing(line: &str) -> String {
    let mut out = String::new();
    let mut byte_index = 0usize;

    while byte_index < line.len() {
        let Some(relative_index) = line[byte_index..].find('#') else {
            out.push_str(&line[byte_index..]);
            break;
        };
        let start = byte_index + relative_index;
        out.push_str(&line[byte_index..start]);
        let candidate = &line[start..];
        let Some((hex_len, _)) = parse_debug_hex_color(candidate) else {
            out.push('#');
            byte_index = start + 1;
            continue;
        };

        out.push_str(&line[start..start + hex_len]);
        out.push(' ');
        byte_index = start + hex_len;
    }

    out
}

fn paint_debug_hex_swatches(
    ui: &mut egui::Ui,
    line: &str,
    text_pos: egui::Pos2,
    row_height: f32,
    char_width: f32,
    swatch_margin: f32,
) {
    let swatch_height = (row_height - 9.0).clamp(6.0, 11.0);
    let swatch_width = swatch_height * 0.75;
    let mut byte_index = 0usize;

    while let Some(relative_index) = line[byte_index..].find('#') {
        let start = byte_index + relative_index;
        let candidate = &line[start..];
        let Some((hex_len, color)) = parse_debug_hex_color(candidate) else {
            byte_index = start + 1;
            continue;
        };

        let before_chars = line[..start].chars().count() as f32;
        let token_chars = line[start..start + hex_len].chars().count() as f32;
        let left = text_pos.x + (before_chars + token_chars) * char_width + swatch_margin;
        let top = text_pos.y - swatch_height * 0.5;
        let rect = egui::Rect::from_min_size(
            egui::pos2(left, top),
            egui::vec2(swatch_width, swatch_height),
        );
        ui.painter().rect_filled(rect, 2.0, color);
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 96, 104)),
            egui::StrokeKind::Outside,
        );

        byte_index = start + hex_len;
    }
}

fn parse_debug_hex_color(candidate: &str) -> Option<(usize, egui::Color32)> {
    let hex_digits: String = candidate
        .chars()
        .skip(1)
        .take_while(|character| character.is_ascii_hexdigit())
        .take(8)
        .collect();
    let len = hex_digits.len();
    if !matches!(len, 3 | 4 | 6 | 8) {
        return None;
    }

    let next = candidate.chars().nth(len + 1);
    if next.is_some_and(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    let parse_pair = |value: &str| u8::from_str_radix(value, 16).ok();
    let double_digit = |character: char| -> Option<u8> {
        let mut value = String::new();
        value.push(character);
        value.push(character);
        parse_pair(&value)
    };

    let color = match len {
        3 | 4 => {
            let mut chars = hex_digits.chars();
            let r = double_digit(chars.next()?)?;
            let g = double_digit(chars.next()?)?;
            let b = double_digit(chars.next()?)?;
            let a = if len == 4 {
                double_digit(chars.next()?)?
            } else {
                255
            };
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        6 | 8 => {
            let r = parse_pair(&hex_digits[0..2])?;
            let g = parse_pair(&hex_digits[2..4])?;
            let b = parse_pair(&hex_digits[4..6])?;
            let a = if len == 8 {
                parse_pair(&hex_digits[6..8])?
            } else {
                255
            };
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        _ => return None,
    };

    Some((len + 1, color))
}

fn export_render_debug_steps(graph: &CanvasGraph) -> io::Result<usize> {
    let output_dir = Path::new(DEBUG_EXPORT_DIR);
    if output_dir.exists() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    for object_limit in 0..=graph.objects.len() {
        let mut frame_graph = graph.clone();
        frame_graph.objects.truncate(object_limit);
        close_truncated_canvas_clips(&mut frame_graph.objects);
        let image = rasterize_canvas_graph_debug_frame(&frame_graph);
        let path = output_dir.join(format!("render_debug_{object_limit:04}.png"));
        image.save(&path).map_err(io::Error::other)?;
    }

    Ok(graph.objects.len() + 1)
}

fn rasterize_canvas_graph_debug_frame(graph: &CanvasGraph) -> image::RgbaImage {
    let width = graph.viewport.x.ceil().max(1.0) as u32;
    let height = graph.viewport.y.ceil().max(1.0) as u32;
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([255, 255, 255, 255]));
    let canvas =
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32));
    let mut clip_stack = Vec::new();
    let mut clip = canvas;

    for object in &graph.objects {
        match object {
            CanvasObject::ClipStart(clip_object) => {
                clip_stack.push(clip);
                clip = clip.intersect(clip_object.rect);
            }
            CanvasObject::ClipEnd => {
                clip = clip_stack.pop().unwrap_or(canvas);
            }
            CanvasObject::Rect(rect) => draw_canvas_rect(&mut image, rect, clip),
            CanvasObject::Button(_) => {}
            CanvasObject::Input(input) => draw_canvas_input(&mut image, input, clip),
            CanvasObject::Image(canvas_image) => draw_canvas_image(&mut image, canvas_image, clip),
            CanvasObject::Svg(svg) => draw_canvas_svg(&mut image, svg, clip),
            CanvasObject::Media(media) => draw_canvas_media(&mut image, media, clip),
            CanvasObject::Text(text) => draw_canvas_text_placeholder(&mut image, text, clip),
        }
    }

    image
}

fn draw_canvas_rect(image: &mut image::RgbaImage, rect: &CanvasRectObject, clip: egui::Rect) {
    draw_filled_rect(image, rect.rect, rect.fill, clip);
    if rect.border_width > 0.0 {
        draw_rect_border(image, rect.rect, rect.border_color, rect.border_width, clip);
    }
}

fn draw_canvas_input(image: &mut image::RgbaImage, input: &CanvasInputObject, clip: egui::Rect) {
    draw_filled_rect(image, input.rect, egui::Color32::WHITE, clip);
    draw_rect_border(
        image,
        input.rect,
        egui::Color32::from_rgb(170, 180, 190),
        1.0,
        clip,
    );
    draw_text_marker(image, input.rect.shrink(4.0), input.font_size, clip);
}

fn draw_canvas_image(
    image: &mut image::RgbaImage,
    canvas_image: &CanvasImageObject,
    clip: egui::Rect,
) {
    let src_size = canvas_image.image.color_image.size;
    if src_size[0] == 0 || src_size[1] == 0 || canvas_image.image.color_image.pixels.is_empty() {
        draw_canvas_media(
            image,
            &CanvasMediaObject {
                rect: canvas_image.rect,
                label: canvas_image.alt.clone(),
            },
            clip,
        );
        return;
    }

    let dest = canvas_image.rect.intersect(clip);
    let min_x = dest.left().floor().max(0.0) as u32;
    let min_y = dest.top().floor().max(0.0) as u32;
    let max_x = dest.right().ceil().min(image.width() as f32) as u32;
    let max_y = dest.bottom().ceil().min(image.height() as f32) as u32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let uv = match canvas_image.object_fit {
        CssObjectFit::Cover => {
            cover_debug_image_uv(canvas_image.image.size, canvas_image.rect.size())
        }
        CssObjectFit::Contain | CssObjectFit::Fill => {
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0))
        }
    };
    let dest_width = canvas_image.rect.width().max(1.0);
    let dest_height = canvas_image.rect.height().max(1.0);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let tx = ((x as f32 + 0.5 - canvas_image.rect.left()) / dest_width).clamp(0.0, 1.0);
            let ty = ((y as f32 + 0.5 - canvas_image.rect.top()) / dest_height).clamp(0.0, 1.0);
            let sx = (uv.left() + uv.width() * tx) * (src_size[0].saturating_sub(1) as f32);
            let sy = (uv.top() + uv.height() * ty) * (src_size[1].saturating_sub(1) as f32);
            let source_index = sy.round() as usize * src_size[0] + sx.round() as usize;
            if let Some(color) = canvas_image.image.color_image.pixels.get(source_index) {
                image.put_pixel(x, y, color_to_rgba(*color));
            }
        }
    }
}

fn draw_canvas_svg(image: &mut image::RgbaImage, svg: &CanvasSvgObject, clip: egui::Rect) {
    let scale_x = svg.rect.width() / svg.svg.size.x.max(1.0);
    let scale_y = svg.rect.height() / svg.svg.size.y.max(1.0);
    for shape in &svg.svg.shapes {
        match shape {
            SvgShape::Rect {
                x,
                y,
                width,
                height,
                fill,
            } => {
                let rect = egui::Rect::from_min_size(
                    egui::Pos2::new(svg.rect.left() + x * scale_x, svg.rect.top() + y * scale_y),
                    egui::vec2(width * scale_x, height * scale_y),
                );
                draw_filled_rect(image, rect, *fill, clip);
            }
            SvgShape::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
                stroke_width,
            } => {
                let center = egui::Pos2::new(
                    svg.rect.left() + cx * scale_x,
                    svg.rect.top() + cy * scale_y,
                );
                draw_filled_circle(image, center, r * scale_x.min(scale_y), *fill, clip);
                if let Some(stroke) = stroke {
                    let border_rect = egui::Rect::from_center_size(
                        center,
                        egui::vec2(r * 2.0 * scale_x, r * 2.0 * scale_y),
                    );
                    draw_rect_border(image, border_rect, *stroke, *stroke_width, clip);
                }
            }
            SvgShape::PathFallback { fill } => {
                draw_filled_circle(
                    image,
                    svg.rect.center(),
                    svg.rect.width().min(svg.rect.height()) * 0.14,
                    *fill,
                    clip,
                );
            }
        }
    }
}

fn draw_canvas_media(image: &mut image::RgbaImage, media: &CanvasMediaObject, clip: egui::Rect) {
    draw_filled_rect(
        image,
        media.rect,
        egui::Color32::from_rgb(238, 241, 245),
        clip,
    );
    draw_rect_border(
        image,
        media.rect,
        egui::Color32::from_rgb(195, 205, 215),
        1.0,
        clip,
    );
    draw_text_marker(image, media.rect.shrink(8.0), 14.0, clip);
}

fn draw_canvas_text_placeholder(
    image: &mut image::RgbaImage,
    text: &CanvasTextObject,
    clip: egui::Rect,
) {
    if text.text_background != egui::Color32::TRANSPARENT {
        draw_filled_rect(image, text.rect, text.text_background, clip);
    }
    draw_text_marker(image, text.rect, text.font_size, clip);
}

fn draw_text_marker(
    image: &mut image::RgbaImage,
    rect: egui::Rect,
    font_size: f32,
    clip: egui::Rect,
) {
    let marker_height = (font_size * 0.12).round().clamp(1.0, 3.0);
    let marker_top = rect.center().y - marker_height * 0.5;
    let marker = egui::Rect::from_min_max(
        egui::Pos2::new(rect.left(), marker_top),
        egui::Pos2::new(rect.right(), marker_top + marker_height),
    );
    draw_filled_rect(image, marker, egui::Color32::from_rgb(45, 50, 58), clip);
}

fn draw_filled_rect(
    image: &mut image::RgbaImage,
    rect: egui::Rect,
    color: egui::Color32,
    clip: egui::Rect,
) {
    if color == egui::Color32::TRANSPARENT {
        return;
    }
    let rect = rect.intersect(clip);
    let min_x = rect.left().floor().max(0.0) as u32;
    let min_y = rect.top().floor().max(0.0) as u32;
    let max_x = rect.right().ceil().min(image.width() as f32) as u32;
    let max_y = rect.bottom().ceil().min(image.height() as f32) as u32;
    let color = color_to_rgba(color);
    for y in min_y..max_y {
        for x in min_x..max_x {
            image.put_pixel(x, y, color);
        }
    }
}

fn draw_rect_border(
    image: &mut image::RgbaImage,
    rect: egui::Rect,
    color: egui::Color32,
    width: f32,
    clip: egui::Rect,
) {
    if color == egui::Color32::TRANSPARENT || width <= 0.0 {
        return;
    }
    let width = width.ceil().max(1.0);
    draw_filled_rect(
        image,
        egui::Rect::from_min_max(
            rect.left_top(),
            egui::Pos2::new(rect.right(), rect.top() + width),
        ),
        color,
        clip,
    );
    draw_filled_rect(
        image,
        egui::Rect::from_min_max(
            egui::Pos2::new(rect.left(), rect.bottom() - width),
            rect.right_bottom(),
        ),
        color,
        clip,
    );
    draw_filled_rect(
        image,
        egui::Rect::from_min_max(
            rect.left_top(),
            egui::Pos2::new(rect.left() + width, rect.bottom()),
        ),
        color,
        clip,
    );
    draw_filled_rect(
        image,
        egui::Rect::from_min_max(
            egui::Pos2::new(rect.right() - width, rect.top()),
            rect.right_bottom(),
        ),
        color,
        clip,
    );
}

fn draw_filled_circle(
    image: &mut image::RgbaImage,
    center: egui::Pos2,
    radius: f32,
    color: egui::Color32,
    clip: egui::Rect,
) {
    if color == egui::Color32::TRANSPARENT || radius <= 0.0 {
        return;
    }
    let bounds =
        egui::Rect::from_center_size(center, egui::Vec2::splat(radius * 2.0)).intersect(clip);
    let min_x = bounds.left().floor().max(0.0) as u32;
    let min_y = bounds.top().floor().max(0.0) as u32;
    let max_x = bounds.right().ceil().min(image.width() as f32) as u32;
    let max_y = bounds.bottom().ceil().min(image.height() as f32) as u32;
    let radius_sq = radius * radius;
    let color = color_to_rgba(color);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let dx = x as f32 + 0.5 - center.x;
            let dy = y as f32 + 0.5 - center.y;
            if dx * dx + dy * dy <= radius_sq {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn cover_debug_image_uv(source_size: egui::Vec2, target_size: egui::Vec2) -> egui::Rect {
    let source_size = source_size.max(egui::Vec2::splat(1.0));
    let target_size = target_size.max(egui::Vec2::splat(1.0));
    let source_ratio = source_size.x / source_size.y;
    let target_ratio = target_size.x / target_size.y;
    if source_ratio > target_ratio {
        let visible_width = target_ratio / source_ratio;
        let left = (1.0 - visible_width) * 0.5;
        egui::Rect::from_min_max(
            egui::Pos2::new(left, 0.0),
            egui::Pos2::new(left + visible_width, 1.0),
        )
    } else {
        let visible_height = source_ratio / target_ratio;
        let top = (1.0 - visible_height) * 0.5;
        egui::Rect::from_min_max(
            egui::Pos2::new(0.0, top),
            egui::Pos2::new(1.0, top + visible_height),
        )
    }
}

fn color_to_rgba(color: egui::Color32) -> image::Rgba<u8> {
    image::Rgba([color.r(), color.g(), color.b(), color.a()])
}

fn input_to_path(input: &str) -> PathBuf {
    strip_url_fragment(input)
        .strip_prefix("file://")
        .map(percent_decode_file_path)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(strip_url_fragment(input)))
}

fn default_sample_bookmark() -> Bookmark {
    Bookmark {
        title: DEFAULT_BOOKMARK_TITLE.to_owned(),
        url: path_to_file_url(Path::new(DEFAULT_PAGE_PATH)),
    }
}

fn default_url_bookmark() -> Bookmark {
    Bookmark {
        title: DEFAULT_URL_BOOKMARK_TITLE.to_owned(),
        url: DEFAULT_URL.to_owned(),
    }
}

fn script_test_bookmarks() -> Vec<Bookmark> {
    SCRIPT_TEST_BOOKMARKS
        .iter()
        .map(|(title, relative_path)| Bookmark {
            title: (*title).to_owned(),
            url: path_to_file_url(&workspace_root_path().join(relative_path)),
        })
        .collect()
}

#[derive(Clone, Debug)]
struct PageScript {
    label: String,
    kind: PageScriptKind,
    source_url: Option<String>,
    byte_len: usize,
    deferred: bool,
    diagnostics: Vec<String>,
    program: Result<justbarelyscript::Program, justbarelyscript::JsError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageScriptKind {
    Inline,
    External,
}

const MAX_EXTERNAL_SCRIPT_PARSE_BYTES: usize = 5 * 1024 * 1024;
const MAX_SCRIPT_DIAGNOSTIC_BYTES: usize = MAX_EXTERNAL_SCRIPT_PARSE_BYTES;
const LIVE_JS_DEBUG_STATEMENT_BUDGET: usize = 50_000;

fn script_console_messages_from_html(html: &str) -> Vec<justbarelyscript::ConsoleMessage> {
    script_console_messages_from_html_with_source(html, None)
}

fn script_console_messages_from_html_with_source(
    html: &str,
    source: Option<&str>,
) -> Vec<justbarelyscript::ConsoleMessage> {
    let scripts = collect_page_scripts(html, source);
    let mut messages = Vec::new();

    if scripts.is_empty() {
        messages.push(justbarelyscript::ConsoleMessage {
            level: justbarelyscript::ConsoleLevel::Info,
            text: "No scripts found.".to_owned(),
        });
        return messages;
    }

    messages.push(justbarelyscript::ConsoleMessage {
        level: justbarelyscript::ConsoleLevel::Info,
        text: format!(
            "Parsed {} script(s). ConsoleSink is parser-only; filesystem, process, shell, deletion, creation, and network APIs are not exposed.",
            scripts.len()
        ),
    });

    for script in scripts {
        match script.program {
            Ok(program) => {
                messages.extend(justbarelyscript::collect_static_console_messages(&program));
                let mut state = justbarelyscript::BrowserExecutionState::default();
                seed_script_browser_globals(&mut state, source);
                state.set_execution_budget(LIVE_JS_DEBUG_STATEMENT_BUDGET);
                emit_global_telemetry(
                    "js.script.execute.started",
                    &[("phase", "console"), ("label", &script.label)],
                );
                let start = std::time::Instant::now();
                state.execute_program(&program);
                let elapsed = start.elapsed().as_millis().to_string();
                let budget_exhausted = if state.execution_budget_exhausted() {
                    "true"
                } else {
                    "false"
                };
                emit_global_telemetry(
                    "js.script.execute.completed",
                    &[
                        ("phase", "console"),
                        ("label", &script.label),
                        ("elapsed_ms", &elapsed),
                        ("budget_exhausted", budget_exhausted),
                    ],
                );
                if state.execution_budget_exhausted() {
                    messages.push(console_error_message(format!(
                        "{}: execution stopped after {} statements; continuing page load with partial JavaScript",
                        script.label, LIVE_JS_DEBUG_STATEMENT_BUDGET
                    )));
                }
            }
            Err(error) => {
                let diagnostic_suffix = if script.diagnostics.is_empty() {
                    String::new()
                } else {
                    format!(
                        " Unsupported constructs: {}.",
                        script.diagnostics.join(", ")
                    )
                };
                messages.push(console_error_message(format!(
                    "{}: {}{}",
                    script.label,
                    error.diagnostic_message(),
                    diagnostic_suffix
                )));
            }
        }
    }

    messages
}

fn live_js_debug_report(html: &str, source: Option<&str>) -> String {
    let html = remove_html_comments(html);
    let scripts = collect_page_scripts(&html, source);
    let mut out = String::new();
    out.push_str("Live JS Debug\n");
    out.push_str("================\n");
    out.push_str(&format!("source: {}\n", source.unwrap_or("<unknown>")));
    out.push_str(&format!("html_bytes: {}\n", html.len()));
    out.push_str(&format!("script_count: {}\n", scripts.len()));
    out.push_str(&format!(
        "statement_budget_per_script: {}\n\n",
        LIVE_JS_DEBUG_STATEMENT_BUDGET
    ));

    if scripts.is_empty() {
        out.push_str("No scripts found.\n");
        return out;
    }

    let mut state = justbarelyscript::BrowserExecutionState::default();
    seed_script_browser_globals(&mut state, source);
    seed_script_dom_state_from_html(&html, &mut state);
    seed_script_computed_styles_from_html(&html, &mut state);

    let mut parsed = 0usize;
    let mut skipped_or_failed = 0usize;
    let mut executed = 0usize;
    let mut budget_exhausted = 0usize;
    let mut total_effects = 0usize;

    for (index, script) in scripts.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", index + 1, script.label));
        out.push_str(&format!(
            "   kind: {}{}\n",
            match script.kind {
                PageScriptKind::Inline => "inline",
                PageScriptKind::External => "external",
            },
            if script.deferred { " defer" } else { "" }
        ));
        if let Some(url) = &script.source_url {
            out.push_str(&format!("   url: {url}\n"));
        }
        out.push_str(&format!("   bytes: {}\n", script.byte_len));
        if !script.diagnostics.is_empty() {
            out.push_str(&format!(
                "   constructs: {}\n",
                script.diagnostics.join(", ")
            ));
        }

        match &script.program {
            Ok(program) => {
                parsed += 1;
                state.set_execution_budget(LIVE_JS_DEBUG_STATEMENT_BUDGET);
                emit_global_telemetry(
                    "js.script.execute.started",
                    &[("phase", "live_js_debug"), ("label", &script.label)],
                );
                let start = std::time::Instant::now();
                state.execute_program(program);
                let elapsed = start.elapsed();
                let effects = state.drain_effects();
                let effect_count = effects.len();
                total_effects += effect_count;
                executed += 1;
                out.push_str(&format!(
                    "   status: executed in {:.2?}; effects={}; {}\n",
                    elapsed,
                    effect_count,
                    browser_effect_summary(&effects)
                ));
                if state.execution_budget_exhausted() {
                    budget_exhausted += 1;
                    out.push_str(
                        "   stop: statement budget exhausted; possible long-running script\n",
                    );
                }
                let elapsed_ms = elapsed.as_millis().to_string();
                let effect_count_string = effect_count.to_string();
                let budget_exhausted_string = if state.execution_budget_exhausted() {
                    "true"
                } else {
                    "false"
                };
                emit_global_telemetry(
                    "js.script.execute.completed",
                    &[
                        ("phase", "live_js_debug"),
                        ("label", &script.label),
                        ("elapsed_ms", &elapsed_ms),
                        ("effects", &effect_count_string),
                        ("budget_exhausted", budget_exhausted_string),
                    ],
                );
                out.push_str(&format!(
                    "   state: listeners={} pending_timers={}\n",
                    state.listener_count(),
                    state.pending_timer_count()
                ));
            }
            Err(error) => {
                skipped_or_failed += 1;
                out.push_str(&format!(
                    "   status: not executed; {}\n",
                    error.diagnostic_message()
                ));
            }
        }
        out.push('\n');
    }

    out.push_str("Summary\n");
    out.push_str("-------\n");
    out.push_str(&format!("parsed: {parsed}\n"));
    out.push_str(&format!("executed: {executed}\n"));
    out.push_str(&format!("not_executed: {skipped_or_failed}\n"));
    out.push_str(&format!("budget_exhausted: {budget_exhausted}\n"));
    out.push_str(&format!("dom_effects: {total_effects}\n"));
    out.push_str(&format!("listeners: {}\n", state.listener_count()));
    out.push_str(&format!(
        "pending_timers: {}\n",
        state.pending_timer_count()
    ));
    out
}

fn browser_effect_summary(effects: &[justbarelyscript::BrowserEffect]) -> String {
    let mut text = 0usize;
    let mut attr = 0usize;
    let mut html = 0usize;
    let mut append = 0usize;
    let mut console = 0usize;
    for effect in effects {
        match effect {
            justbarelyscript::BrowserEffect::SetTextContent { .. } => text += 1,
            justbarelyscript::BrowserEffect::SetAttribute { .. } => attr += 1,
            justbarelyscript::BrowserEffect::SetInnerHtml { .. } => html += 1,
            justbarelyscript::BrowserEffect::AppendChild { .. } => append += 1,
            justbarelyscript::BrowserEffect::ConsoleLog { .. } => console += 1,
        }
    }
    format!("text={text} attr={attr} inner_html={html} append={append} console={console}")
}

fn build_script_state(html: &str) -> justbarelyscript::BrowserExecutionState {
    build_script_state_with_source(html, None)
}

fn build_script_state_with_source(
    html: &str,
    source: Option<&str>,
) -> justbarelyscript::BrowserExecutionState {
    let html = remove_html_comments(html);
    let scripts = collect_page_scripts(&html, source);
    let mut state = justbarelyscript::BrowserExecutionState::default();
    seed_script_browser_globals(&mut state, source);
    seed_script_dom_state_from_html(&html, &mut state);
    seed_script_computed_styles_from_html(&html, &mut state);
    for script in scripts {
        let Ok(program) = script.program else {
            continue;
        };
        state.set_execution_budget(LIVE_JS_DEBUG_STATEMENT_BUDGET);
        emit_global_telemetry(
            "js.script.execute.started",
            &[("phase", "script_state"), ("label", &script.label)],
        );
        let start = std::time::Instant::now();
        state.execute_program(&program);
        let elapsed = start.elapsed().as_millis().to_string();
        let budget_exhausted = if state.execution_budget_exhausted() {
            "true"
        } else {
            "false"
        };
        state.drain_effects(); // discard initial DOM effects; we only keep event handlers
        emit_global_telemetry(
            "js.script.execute.completed",
            &[
                ("phase", "script_state"),
                ("label", &script.label),
                ("elapsed_ms", &elapsed),
                ("budget_exhausted", budget_exhausted),
                ("listeners", &state.listener_count().to_string()),
                ("pending_timers", &state.pending_timer_count().to_string()),
            ],
        );
    }
    state
}

fn seed_script_computed_styles_from_html(
    html: &str,
    state: &mut justbarelyscript::BrowserExecutionState,
) {
    let css = extract_tag_inner(html, "style").unwrap_or("");
    let browser_style = parse_basic_css_for_viewport_with_root_classes(css, 1280.0, &[]);

    let mut offset = 0;
    let mut remaining = html;
    while let Some(rel_open_start) = remaining.find('<') {
        let open_start = offset + rel_open_start;
        let after_open = &html[open_start..];
        if after_open.starts_with("</") || after_open.starts_with("<!") {
            offset = open_start + 1;
            remaining = &html[offset..];
            continue;
        }
        let Some(open_end_rel) = after_open.find('>') else {
            break;
        };
        let open_end = open_start + open_end_rel + 1;
        let open_tag = &html[open_start..open_end];
        let Some(id) = extract_attr(open_tag, "id") else {
            offset = open_end;
            remaining = &html[offset..];
            continue;
        };
        let Some(tag) = tag_name(open_tag) else {
            offset = open_end;
            remaining = &html[offset..];
            continue;
        };
        let classes: Vec<String> = extract_attr(open_tag, "class")
            .unwrap_or_default()
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect();
        let key = ElementStyleKey {
            tag: tag.to_owned(),
            id: Some(id.clone()),
            classes,
            attributes: vec![],
            parent: None,
            previous_sibling: None,
        };
        let computed = computed_box_style(&browser_style, &key);
        let mut props = std::collections::HashMap::new();
        if let Some(display) = computed.display {
            let display_str = match display {
                CssDisplay::None => "none",
                CssDisplay::Block => "block",
                CssDisplay::Inline => "inline",
                CssDisplay::InlineBlock => "inline-block",
                CssDisplay::Flex => "flex",
                CssDisplay::Grid => "grid",
                CssDisplay::Table => "table",
                CssDisplay::ListItem => "list-item",
            };
            props.insert("display".to_owned(), display_str.to_owned());
        }
        if !props.is_empty() {
            state.seed_computed_style(&id, props);
        }
        offset = open_end;
        remaining = &html[offset..];
    }
}

fn parse_html_document_from_live_html(
    live_html: &str,
    source: &str,
    text_metrics: Option<&egui::Context>,
) -> BrowserDocument {
    let reveal_hydration_hidden_content =
        page_has_unexecuted_hydration_script(live_html, Some(source));
    let mut css = extract_tag_inner(live_html, "style")
        .unwrap_or("")
        .to_owned();
    css.push_str(&load_linked_stylesheets(live_html, source).unwrap_or_default());
    let live_html = remove_non_visual_metadata_elements(live_html);
    let dom = parse_dom_document(&live_html);
    let title = dom
        .first_descendant_by_tag("title")
        .map(DomElement::text_content)
        .or_else(|| extract_tag_text(&live_html, "title"))
        .unwrap_or_else(|| "Untitled".to_owned())
        .trim()
        .to_owned();
    let root_classes = document_theme_root_classes(&dom, text_metrics);
    let style = parse_basic_css_for_viewport_with_root_classes(&css, 1280.0, &root_classes);
    let render_graph = build_render_graph(&dom, &style);
    let canvas_graph = render_graph_to_canvas_graph(
        &render_graph,
        source,
        style.image_height_auto,
        text_metrics,
        reveal_hydration_hidden_content,
    );
    let blocks = render_graph_to_blocks(&render_graph, source, style.image_height_auto);
    BrowserDocument {
        title,
        source: source.to_owned(),
        style,
        canvas_graph,
        blocks,
    }
}

fn apply_safe_script_browser_effects(html: &str) -> String {
    apply_safe_script_browser_effects_with_source(html, None)
}

fn apply_safe_script_browser_effects_with_source(html: &str, source: Option<&str>) -> String {
    apply_safe_script_browser_effects_detailed(html, source).html
}

#[derive(Debug)]
struct ScriptApplicationResult {
    html: String,
    hydration_failed: bool,
}

fn apply_safe_script_browser_effects_detailed(
    html: &str,
    source: Option<&str>,
) -> ScriptApplicationResult {
    let scripts = collect_page_scripts(html, source);
    let mut output = html.to_owned();
    let mut state = justbarelyscript::BrowserExecutionState::default();
    seed_script_browser_globals(&mut state, source);
    seed_script_dom_state_from_html(html, &mut state);
    seed_script_computed_styles_from_html(html, &mut state);
    let mut hydration_failed = false;

    for script in scripts {
        let script_hydration_candidate = page_script_is_hydration_candidate(&script);
        let Ok(program) = script.program else {
            if script_hydration_candidate {
                hydration_failed = true;
            }
            continue;
        };
        state.set_execution_budget(LIVE_JS_DEBUG_STATEMENT_BUDGET);
        emit_global_telemetry(
            "js.script.execute.started",
            &[("phase", "dom_effects"), ("label", &script.label)],
        );
        let start = std::time::Instant::now();
        state.execute_program(&program);
        let elapsed = start.elapsed().as_millis().to_string();
        let budget_exhausted = if state.execution_budget_exhausted() {
            "true"
        } else {
            "false"
        };
        let effects = state.drain_effects();
        let effect_count = effects.len().to_string();
        emit_global_telemetry(
            "js.script.execute.completed",
            &[
                ("phase", "dom_effects"),
                ("label", &script.label),
                ("elapsed_ms", &elapsed),
                ("effects", &effect_count),
                ("budget_exhausted", budget_exhausted),
            ],
        );
        for effect in effects {
            match effect {
                justbarelyscript::BrowserEffect::SetTextContent { element_id, value } => {
                    output = set_element_text_content_by_id(&output, &element_id, &value);
                }
                justbarelyscript::BrowserEffect::SetAttribute {
                    element_id,
                    name,
                    value,
                } => {
                    output = set_element_attribute_by_id(&output, &element_id, &name, &value);
                }
                justbarelyscript::BrowserEffect::SetInnerHtml { element_id, value } => {
                    output = set_element_inner_html_by_id(&output, &element_id, &value);
                }
                justbarelyscript::BrowserEffect::AppendChild { parent_id, child } => {
                    output = append_child_html_by_id(&output, &parent_id, &child);
                }
                justbarelyscript::BrowserEffect::ConsoleLog { .. } => {}
            }
        }
    }

    ScriptApplicationResult {
        html: output,
        hydration_failed,
    }
}

fn page_has_unexecuted_hydration_script(html: &str, source: Option<&str>) -> bool {
    collect_page_scripts(html, source)
        .iter()
        .any(|script| script.program.is_err() && page_script_is_hydration_candidate(script))
}

fn page_script_is_hydration_candidate(script: &PageScript) -> bool {
    script.kind == PageScriptKind::External
        || script.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.as_str(),
                "ES modules"
                    | "arrow functions"
                    | "optional chaining"
                    | "nullish coalescing"
                    | "spread/rest"
                    | "template literals"
                    | "async/await"
                    | "Promise constructor"
            )
        })
}

fn seed_script_browser_globals(
    state: &mut justbarelyscript::BrowserExecutionState,
    source: Option<&str>,
) {
    let navigator = justbarelyscript::NavigatorInfo::detect();
    let screen = justbarelyscript::ScreenInfo::detect();
    let fingerprint = justbarelyscript::FingerprintSuite::detect();
    state.seed_navigator(&navigator);
    state.seed_screen(&screen);
    state.seed_fingerprint_suite(fingerprint);
    if let Some(source) = source {
        state.seed_location(source);
    }
}

fn collect_page_scripts(html: &str, source: Option<&str>) -> Vec<PageScript> {
    let mut normal = Vec::new();
    let mut deferred = Vec::new();
    let mut remaining = html;
    let mut offset = 0usize;
    let mut index = 0usize;

    while let Some(rel_open_start) = find_ascii_case_insensitive_local(remaining, "<script") {
        let open_start = offset + rel_open_start;
        let after_open = &html[open_start..];
        let Some(open_end_rel) = after_open.find('>') else {
            break;
        };
        let open_end = open_start + open_end_rel + 1;
        let open_tag = &html[open_start..open_end];
        let after_content_start = &html[open_end..];
        let Some(close_rel) = find_ascii_case_insensitive_local(after_content_start, "</script>")
        else {
            break;
        };
        let close_start = open_end + close_rel;
        let inline_source = &html[open_end..close_start];

        index += 1;
        let defer = tag_has_bool_attr(open_tag, "defer");
        let script = if let Some(src) = extract_attr(open_tag, "src") {
            load_external_page_script(source, &src, index, defer)
        } else {
            let label = format!("Inline script {index}");
            if inline_source.len() > MAX_EXTERNAL_SCRIPT_PARSE_BYTES {
                let diagnostics = oversized_script_diagnostics(inline_source.len());
                PageScript {
                    label,
                    kind: PageScriptKind::Inline,
                    source_url: None,
                    byte_len: inline_source.len(),
                    deferred: defer,
                    diagnostics,
                    program: Err(synthetic_script_error(&format!(
                        "inline script skipped: {} bytes exceeds parser budget of {} bytes",
                        inline_source.len(),
                        MAX_EXTERNAL_SCRIPT_PARSE_BYTES
                    ))),
                }
            } else {
                let diagnostics = script_construct_diagnostics(inline_source);
                PageScript {
                    label,
                    kind: PageScriptKind::Inline,
                    source_url: None,
                    byte_len: inline_source.len(),
                    deferred: defer,
                    diagnostics,
                    program: justbarelyscript::parse_script(inline_source),
                }
            }
        };

        emit_script_parse_telemetry(index, &script);

        if defer {
            deferred.push(script);
        } else {
            normal.push(script);
        }

        offset = close_start + "</script>".len();
        remaining = &html[offset..];
    }

    normal.extend(deferred);
    normal
}

fn emit_script_parse_telemetry(index: usize, script: &PageScript) {
    let index = index.to_string();
    let bytes = script.byte_len.to_string();
    let deferred = if script.deferred { "true" } else { "false" };
    let kind = page_script_kind_str(script.kind);
    let url = script.source_url.as_deref().unwrap_or("");
    match &script.program {
        Ok(program) => {
            let statements = program.body.len().to_string();
            emit_global_telemetry(
                "js.script.parsed",
                &[
                    ("index", &index),
                    ("label", &script.label),
                    ("kind", kind),
                    ("url", url),
                    ("bytes", &bytes),
                    ("defer", deferred),
                    ("statements", &statements),
                ],
            );
        }
        Err(error) => {
            let constructs = script.diagnostics.join(", ");
            let reason = error.diagnostic_message();
            emit_global_telemetry(
                "js.script.skipped",
                &[
                    ("index", &index),
                    ("label", &script.label),
                    ("kind", kind),
                    ("url", url),
                    ("bytes", &bytes),
                    ("defer", deferred),
                    ("reason", &reason),
                    ("constructs", &constructs),
                ],
            );
        }
    }
}

fn page_script_kind_str(kind: PageScriptKind) -> &'static str {
    match kind {
        PageScriptKind::Inline => "inline",
        PageScriptKind::External => "external",
    }
}

fn load_external_page_script(
    document_source: Option<&str>,
    src: &str,
    index: usize,
    deferred: bool,
) -> PageScript {
    let Some(document_source) = document_source else {
        let label = format!("External script {index} ({src})");
        return PageScript {
            label,
            kind: PageScriptKind::External,
            source_url: Some(src.to_owned()),
            byte_len: 0,
            deferred,
            diagnostics: Vec::new(),
            program: Err(synthetic_script_error(
                "external script skipped: document source unavailable",
            )),
        };
    };

    let resolved = resolve_resource_url(document_source, src);
    let label = format!("External script {index} ({resolved})");
    if !script_allowed_for_document(document_source, &resolved) {
        return PageScript {
            label,
            kind: PageScriptKind::External,
            source_url: Some(resolved),
            byte_len: 0,
            deferred,
            diagnostics: Vec::new(),
            program: Err(synthetic_script_error(
                "external script blocked by document policy",
            )),
        };
    }

    match read_script_resource(&resolved) {
        Ok(source) => {
            if source.len() > MAX_EXTERNAL_SCRIPT_PARSE_BYTES {
                let diagnostics = oversized_script_diagnostics(source.len());
                return PageScript {
                    label,
                    kind: PageScriptKind::External,
                    source_url: Some(resolved),
                    byte_len: source.len(),
                    deferred,
                    diagnostics,
                    program: Err(synthetic_script_error(&format!(
                        "external script skipped: {} bytes exceeds parser budget of {} bytes",
                        source.len(),
                        MAX_EXTERNAL_SCRIPT_PARSE_BYTES
                    ))),
                };
            }
            let diagnostics = script_construct_diagnostics(&source);
            let program = justbarelyscript::parse_script(&source);
            PageScript {
                label,
                kind: PageScriptKind::External,
                source_url: Some(resolved),
                byte_len: source.len(),
                deferred,
                diagnostics,
                program,
            }
        }
        Err(error) => PageScript {
            label,
            kind: PageScriptKind::External,
            source_url: Some(resolved),
            byte_len: 0,
            deferred,
            diagnostics: Vec::new(),
            program: Err(synthetic_script_error(&format!(
                "external script load failed: {error}"
            ))),
        },
    }
}

fn oversized_script_diagnostics(byte_len: usize) -> Vec<String> {
    vec![format!(
        "diagnostics skipped: {byte_len} bytes exceeds diagnostic budget of {MAX_SCRIPT_DIAGNOSTIC_BYTES} bytes"
    )]
}

fn script_construct_diagnostics(source: &str) -> Vec<String> {
    let checks = [
        ("import ", "ES modules"),
        ("export ", "ES modules"),
        ("=>", "arrow functions"),
        ("?.", "optional chaining"),
        ("??", "nullish coalescing"),
        ("...", "spread/rest"),
        ("`", "template literals"),
        ("async ", "async/await"),
        ("await ", "async/await"),
        ("class ", "class syntax"),
        ("new Promise", "Promise constructor"),
        ("regeneratorRuntime", "regenerator runtime"),
        ("webpackJsonp", "Webpack runtime"),
        ("XMLHttpRequest", "XMLHttpRequest"),
        ("fetch(", "fetch"),
        ("axios", "axios"),
        ("Object.defineProperty", "property descriptors"),
        ("Object.getOwnPropertyDescriptor", "property descriptors"),
        ("Object.getPrototypeOf", "prototype reflection"),
        ("Object.create", "prototype creation"),
        ("Proxy", "Proxy"),
        ("WeakMap", "WeakMap"),
        ("Symbol", "Symbol"),
    ];

    let mut diagnostics = Vec::new();
    for (needle, label) in checks {
        let count = source.matches(needle).count();
        if count > 0 {
            let entry = format!("{label} x{count}");
            if !diagnostics.contains(&entry) {
                diagnostics.push(entry);
            }
        }
    }
    diagnostics
}

fn read_script_resource(resolved: &str) -> io::Result<String> {
    let cache = SCRIPT_RESOURCE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(cached) = guard.get(resolved).cloned()
    {
        let status = if cached.is_ok() { "ok" } else { "error" };
        let bytes = cached
            .as_ref()
            .map(|source| source.len().to_string())
            .unwrap_or_else(|_| "0".to_owned());
        emit_global_telemetry(
            "js.resource.cache_hit",
            &[("url", resolved), ("status", status), ("bytes", &bytes)],
        );
        return cached.map_err(io::Error::other);
    }

    emit_global_telemetry("js.resource.cache_miss", &[("url", resolved)]);
    let loaded = if is_remote_url(resolved) {
        http_client()?
            .get(resolved)
            .send()
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .text()
            .map_err(io::Error::other)
    } else {
        fs::read_to_string(input_to_path(resolved))
    };

    let cached = loaded
        .as_ref()
        .map(|source| source.to_owned())
        .map_err(|error| error.to_string());
    if let Ok(mut guard) = cache.lock() {
        guard.insert(resolved.to_owned(), cached);
    }

    loaded
}

fn script_allowed_for_document(source: &str, resource: &str) -> bool {
    if !resource_allowed_for_document(source, resource) {
        return false;
    }
    if is_remote_url(source) && is_remote_url(resource) {
        return same_origin_url(source, resource);
    }
    true
}

fn same_origin_url(a: &str, b: &str) -> bool {
    let Ok(a) = reqwest::Url::parse(a) else {
        return false;
    };
    let Ok(b) = reqwest::Url::parse(b) else {
        return false;
    };
    a.scheme() == b.scheme()
        && a.host_str() == b.host_str()
        && a.port_or_known_default() == b.port_or_known_default()
}

fn tag_has_bool_attr(tag: &str, attr: &str) -> bool {
    let lower_tag = tag.to_ascii_lowercase();
    let attr = attr.to_ascii_lowercase();
    lower_tag
        .split(|ch: char| ch.is_whitespace() || ch == '<' || ch == '>' || ch == '/')
        .any(|part| part == attr || part.starts_with(&format!("{attr}=")))
}

fn synthetic_script_error(message: &str) -> justbarelyscript::JsError {
    justbarelyscript::JsError {
        kind: justbarelyscript::JsErrorKind::Parse,
        message: message.to_owned(),
        span: None,
    }
}

fn find_ascii_case_insensitive_local(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn seed_script_dom_state_from_html(
    html: &str,
    state: &mut justbarelyscript::BrowserExecutionState,
) {
    let mut offset = 0;
    let mut remaining = html;
    let mut generated_id = 0usize;

    while let Some(rel_open_start) = remaining.find('<') {
        let open_start = offset + rel_open_start;
        let after_open = &html[open_start..];
        if after_open.starts_with("</") || after_open.starts_with("<!") {
            offset = open_start + 1;
            remaining = &html[offset..];
            continue;
        }

        let Some(open_end_rel) = after_open.find('>') else {
            break;
        };
        let open_end = open_start + open_end_rel + 1;
        let open_tag = &html[open_start..open_end];
        let real_id = extract_attr(open_tag, "id");
        let id = real_id.clone().unwrap_or_else(|| {
            generated_id += 1;
            format!("__dom_seed_{generated_id}")
        });

        let mut attributes = std::collections::HashMap::new();
        if let Some(real_id) = real_id {
            attributes.insert("id".to_owned(), real_id);
        }
        if let Some(classes) = extract_attr(open_tag, "class") {
            attributes.insert("class".to_owned(), classes);
        }

        let text_content = tag_name(open_tag)
            .and_then(|tag| {
                let close = format!("</{tag}>");
                let close_start = html[open_end..].find(&close)? + open_end;
                Some(decode_basic_entities(&strip_tags(
                    &html[open_end..close_start],
                )))
            })
            .unwrap_or_default();

        state.seed_existing_element(&id, text_content, attributes);
        offset = open_end;
        remaining = &html[offset..];
    }
}

fn append_child_html_by_id(
    html: &str,
    parent_id: &str,
    child: &justbarelyscript::DomElementSnapshot,
) -> String {
    let Some((_open_start, open_end, close_start, close_end)) =
        find_element_range_by_id(html, parent_id)
    else {
        return html.to_owned();
    };

    let child_html = serialize_dom_snapshot(child);
    let mut output = String::with_capacity(html.len() + child_html.len());
    output.push_str(&html[..open_end]);
    output.push_str(&html[open_end..close_start]);
    output.push_str(&child_html);
    output.push_str(&html[close_start..close_end]);
    output.push_str(&html[close_end..]);
    output
}

fn serialize_dom_snapshot(element: &justbarelyscript::DomElementSnapshot) -> String {
    let tag_name = sanitize_tag_name(&element.tag_name);
    let mut html = String::new();
    html.push('<');
    html.push_str(&tag_name);
    for (name, value) in &element.attributes {
        html.push(' ');
        html.push_str(&sanitize_attr_name(name));
        html.push_str("=\"");
        html.push_str(&encode_basic_attr(value));
        html.push('"');
    }
    html.push('>');
    if element.inner_html.is_empty() {
        html.push_str(&encode_basic_text(&element.text_content));
        for child in &element.children {
            html.push_str(&serialize_dom_snapshot(child));
        }
    } else {
        html.push_str(&element.inner_html);
    }
    html.push_str("</");
    html.push_str(&tag_name);
    html.push('>');
    html
}

fn set_element_text_content_by_id(html: &str, element_id: &str, value: &str) -> String {
    let Some((open_start, open_end, close_start, close_end)) =
        find_element_range_by_id(html, element_id)
    else {
        return html.to_owned();
    };

    let mut output = String::with_capacity(html.len() + value.len());
    output.push_str(&html[..open_end]);
    output.push_str(&encode_basic_text(value));
    output.push_str(&html[close_start..close_end]);
    output.push_str(&html[close_end..]);
    debug_assert!(output.starts_with(&html[..open_start]));
    output
}

fn set_element_inner_html_by_id(html: &str, element_id: &str, value: &str) -> String {
    let Some((open_start, open_end, close_start, close_end)) =
        find_element_range_by_id(html, element_id)
    else {
        return html.to_owned();
    };

    let mut output = String::with_capacity(html.len() + value.len());
    output.push_str(&html[..open_end]);
    output.push_str(value);
    output.push_str(&html[close_start..close_end]);
    output.push_str(&html[close_end..]);
    debug_assert!(output.starts_with(&html[..open_start]));
    output
}

fn set_element_attribute_by_id(html: &str, element_id: &str, name: &str, value: &str) -> String {
    let Some((open_start, open_end, close_start, close_end)) =
        find_element_range_by_id(html, element_id)
    else {
        return html.to_owned();
    };

    let open_tag = &html[open_start..open_end];
    let Some(insert_at) = open_tag.rfind('>') else {
        return html.to_owned();
    };
    let attr = format!(
        " {}=\"{}\"",
        sanitize_attr_name(name),
        encode_basic_attr(value)
    );
    let mut output = String::with_capacity(html.len() + attr.len());
    output.push_str(&html[..open_start + insert_at]);
    output.push_str(&attr);
    output.push_str(&html[open_start + insert_at..close_start]);
    output.push_str(&html[close_start..close_end]);
    output.push_str(&html[close_end..]);
    output
}

fn find_element_range_by_id(html: &str, element_id: &str) -> Option<(usize, usize, usize, usize)> {
    let mut offset = 0;
    let mut remaining = html;

    while let Some(rel_open_start) = remaining.find('<') {
        let open_start = offset + rel_open_start;
        let after_open = &html[open_start..];
        if after_open.starts_with("</") || after_open.starts_with("<!") {
            offset = open_start + 1;
            remaining = &html[offset..];
            continue;
        }

        let open_end = open_start + after_open.find('>')? + 1;
        let open_tag = &html[open_start..open_end];
        if extract_attr(open_tag, "id").as_deref() != Some(element_id) {
            offset = open_end;
            remaining = &html[offset..];
            continue;
        }

        let tag = tag_name(open_tag)?;
        let close = format!("</{tag}>");
        let close_rel = html[open_end..].find(&close)?;
        let close_start = open_end + close_rel;
        let close_end = close_start + close.len();
        return Some((open_start, open_end, close_start, close_end));
    }

    None
}

fn encode_basic_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn encode_basic_attr(text: &str) -> String {
    encode_basic_text(text).replace('"', "&quot;")
}

fn sanitize_tag_name(tag_name: &str) -> String {
    let sanitized: String = tag_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect();
    if sanitized.is_empty() {
        "div".to_owned()
    } else {
        sanitized.to_ascii_lowercase()
    }
}

fn sanitize_attr_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    if sanitized.is_empty() {
        "data-empty".to_owned()
    } else {
        sanitized.to_ascii_lowercase()
    }
}

fn console_error_message(text: impl Into<String>) -> justbarelyscript::ConsoleMessage {
    justbarelyscript::ConsoleMessage {
        level: justbarelyscript::ConsoleLevel::Error,
        text: text.into(),
    }
}

fn ensure_bookmark(bookmarks: &mut Vec<Bookmark>, bookmark: Bookmark) -> bool {
    if bookmarks
        .iter()
        .any(|existing| existing.url == bookmark.url)
    {
        return false;
    }

    bookmarks.push(bookmark);
    true
}

fn bookmarks_path() -> PathBuf {
    PathBuf::from(BOOKMARKS_PATH)
}

fn load_bookmarks() -> io::Result<Vec<Bookmark>> {
    let path = bookmarks_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(path)?;
    Ok(parse_bookmarks(&text))
}

fn save_bookmarks(bookmarks: &[Bookmark]) -> io::Result<()> {
    let path = bookmarks_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut out = String::new();
    for bookmark in bookmarks {
        out.push_str(&bookmark.title.replace(['\t', '\n', '\r'], " "));
        out.push('\t');
        out.push_str(&portable_bookmark_url(&bookmark.url).replace(['\t', '\n', '\r'], " "));
        out.push('\n');
    }

    fs::write(path, out)
}

fn parse_bookmarks(text: &str) -> Vec<Bookmark> {
    text.lines()
        .filter_map(|line| {
            let (title, url) = line.split_once('\t')?;
            let title = title.trim();
            let url = url.trim();
            if title.is_empty() || url.is_empty() {
                None
            } else {
                Some(Bookmark {
                    title: title.to_owned(),
                    url: resolve_bookmark_url(url),
                })
            }
        })
        .collect()
}

fn resolve_bookmark_url(url: &str) -> String {
    url.replace(LOCAL_BOOKMARK_TOKEN, &local_bookmark_path_token())
}

fn portable_bookmark_url(url: &str) -> String {
    let local_root = workspace_root_file_url();
    if url == local_root {
        return format!("file:///{LOCAL_BOOKMARK_TOKEN}");
    }
    url.strip_prefix(&(local_root + "/"))
        .map(|suffix| format!("file:///{LOCAL_BOOKMARK_TOKEN}/{suffix}"))
        .unwrap_or_else(|| url.to_owned())
}

fn local_bookmark_path_token() -> String {
    let root = workspace_root_file_url();
    root.strip_prefix("file:///")
        .unwrap_or_else(|| root.trim_start_matches("file://"))
        .to_owned()
}

fn workspace_root_file_url() -> String {
    path_to_file_url(&workspace_root_path())
}

fn workspace_root_path() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(manifest_dir).to_path_buf()
}

impl App for AlmostThereApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.record_frame_events(ctx);
        self.poll_pending_navigation(ctx);

        if self.script_state.has_pending_timers() {
            let elapsed_ms = self.page_loaded_at.elapsed().as_millis() as u64;
            let timer_effects = self.script_state.poll_timers(elapsed_ms);
            if !timer_effects.is_empty() {
                self.apply_script_effects(timer_effects, ctx);
            }
            if self.script_state.has_pending_timers() {
                ctx.request_repaint();
            }
        }

        if !self.text_metrics_ready {
            if let Ok(document) = load_url_document_with_text_metrics(&self.url_input, ctx) {
                self.document = document;
                self.render_debug.object_limit = self.document.canvas_graph.objects.len();
                if !self.current_html.is_empty() {
                    self.render_graph_debug_text =
                        parse_render_graph_debug_dump(&self.current_html, &self.document.source);
                }
            }
            self.text_metrics_ready = true;
        }

        ctx.set_visuals(egui::Visuals::light());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{APP_TITLE} :: {}",
            self.document.title
        )));

        egui::TopBottomPanel::top("browser_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Back"));
                ui.add_enabled(false, egui::Button::new("Forward"));
                if ui.button("Reload").clicked() {
                    self.load_current_input(ctx);
                }
                let debug_label = if self.render_debug.open {
                    "Close Debug"
                } else {
                    "Debug"
                };
                if ui.button(debug_label).clicked() {
                    self.render_debug.open = !self.render_debug.open;
                    if self.render_debug.open {
                        self.render_debug.object_limit = self.document.canvas_graph.objects.len();
                    }
                }
                if ui.button("Report Error").clicked() {
                    self.report_user_error();
                }
                if self.pending_navigation.is_some() {
                    ui.add(egui::Spinner::new().size(16.0));
                    ui.label("Loading");
                }
                let bookmark_label = if self.current_bookmark_index().is_some() {
                    "★"
                } else {
                    "☆"
                };
                if ui.button(bookmark_label).clicked() {
                    self.toggle_current_bookmark();
                }

                let mut bookmark_to_open = None;
                ui.menu_button("Bookmarks", |ui| {
                    if self.bookmarks.is_empty() {
                        ui.add_enabled(false, egui::Button::new("No bookmarks"));
                    } else {
                        for (index, bookmark) in self.bookmarks.iter().enumerate() {
                            if ui.button(&bookmark.title).clicked() {
                                bookmark_to_open = Some(index);
                                ui.close();
                            }
                        }
                    }
                });
                if let Some(index) = bookmark_to_open {
                    self.open_bookmark(index, ctx);
                }

                let mut script_test_to_open = None;
                ui.menu_button("Script Tests", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show(ui, |ui| {
                            for bookmark in script_test_bookmarks() {
                                if ui.button(&bookmark.title).clicked() {
                                    script_test_to_open = Some(bookmark);
                                    ui.close();
                                }
                            }
                        });
                });
                if let Some(bookmark) = script_test_to_open {
                    self.open_bookmark_value(bookmark, ctx);
                }

                let response = ui.add_sized(
                    [ui.available_width(), 24.0],
                    egui::TextEdit::singleline(&mut self.url_input),
                );
                if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.load_current_input(ctx);
                }
            });
        });

        egui::TopBottomPanel::bottom("browser_status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.pending_navigation.is_some() {
                    ui.add(egui::Spinner::new().size(14.0));
                }
                ui.label(&self.status);
                ui.separator();
                ui.label(format!("Telemetry: {}", self.telemetry.display_path()));
            });
        });

        if self.render_debug.open {
            // Snapshot interactive elements before the closure to avoid split-borrow conflicts.
            #[derive(Clone)]
            struct DebugElement {
                id: String,
                label: String,
                event_type: &'static str,
            }
            let interactive_elements: Vec<DebugElement> = self
                .document
                .canvas_graph
                .objects
                .iter()
                .filter_map(|obj| match obj {
                    CanvasObject::Input(input) => {
                        input.element_id.as_ref().map(|id| DebugElement {
                            id: id.clone(),
                            label: input.label.clone(),
                            event_type: "input",
                        })
                    }
                    CanvasObject::Button(button) => {
                        button.element_id.as_ref().map(|id| DebugElement {
                            id: id.clone(),
                            label: button.text.clone(),
                            event_type: "click",
                        })
                    }
                    _ => None,
                })
                .collect();

            let mut pending_debug_events: Vec<(String, &'static str)> = Vec::new();

            egui::SidePanel::right("render_debug_inspector")
                .resizable(true)
                .default_width(380.0)
                .width_range(260.0..=720.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.render_debug.active_tab,
                            DebugPanelTab::RenderGraph,
                            "RenderGraph",
                        );
                        ui.selectable_value(
                            &mut self.render_debug.active_tab,
                            DebugPanelTab::CanvasGraph,
                            "CanvasGraph",
                        );
                        ui.selectable_value(
                            &mut self.render_debug.active_tab,
                            DebugPanelTab::Html,
                            "HTML",
                        );
                        ui.selectable_value(
                            &mut self.render_debug.active_tab,
                            DebugPanelTab::Console,
                            "Console",
                        );
                        ui.selectable_value(
                            &mut self.render_debug.active_tab,
                            DebugPanelTab::LiveJs,
                            "Live JS",
                        );
                        ui.selectable_value(
                            &mut self.render_debug.active_tab,
                            DebugPanelTab::Events,
                            "Events",
                        );
                    });
                    ui.separator();

                    if self.render_debug.active_tab == DebugPanelTab::Console {
                        paint_console_messages(ui, &self.console_messages);
                        return;
                    }

                    if self.render_debug.active_tab == DebugPanelTab::Events {
                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                if interactive_elements.is_empty() {
                                    ui.label("No interactive elements with an id on this page.");
                                    return;
                                }
                                for elem in &interactive_elements {
                                    ui.separator();
                                    if elem.event_type == "input" {
                                        ui.label(format!("input  #{}", elem.id));
                                        let staged = self
                                            .render_debug
                                            .event_staged_values
                                            .entry(elem.id.clone())
                                            .or_default();
                                        ui.add(
                                            egui::TextEdit::singleline(staged)
                                                .desired_width(ui.available_width() - 100.0)
                                                .hint_text(&elem.label),
                                        );
                                        if ui.button("Fire input event").clicked() {
                                            pending_debug_events
                                                .push((elem.id.clone(), elem.event_type));
                                        }
                                    } else {
                                        ui.label(format!(
                                            "button #{}  \"{}\"",
                                            elem.id, elem.label
                                        ));
                                        if ui.button("Fire click event").clicked() {
                                            pending_debug_events
                                                .push((elem.id.clone(), elem.event_type));
                                        }
                                    }
                                }
                            });
                        return;
                    }

                    let text = match self.render_debug.active_tab {
                        DebugPanelTab::RenderGraph => self.render_graph_debug_text.clone(),
                        DebugPanelTab::CanvasGraph => {
                            canvas_graph_debug_string(&self.document.canvas_graph)
                        }
                        DebugPanelTab::Html => self.current_html.clone(),
                        DebugPanelTab::LiveJs => self.live_js_debug_text.clone(),
                        DebugPanelTab::Console | DebugPanelTab::Events => String::new(),
                    };

                    egui::ScrollArea::both()
                        .id_salt(("render_debug_text", self.render_debug.active_tab))
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            if self.render_debug.active_tab == DebugPanelTab::CanvasGraph {
                                paint_alternating_debug_text_with_canvas_thumbs(
                                    ui,
                                    &text,
                                    Some(&self.document.canvas_graph),
                                );
                            } else {
                                paint_alternating_debug_text(ui, &text);
                            }
                        });
                });

            // Apply events fired from the Events tab.
            for (id, event_type) in pending_debug_events {
                if event_type == "input" {
                    let value = self
                        .render_debug
                        .event_staged_values
                        .get(&id)
                        .cloned()
                        .unwrap_or_default();
                    self.script_state
                        .dom
                        .attributes_by_id
                        .entry(id.clone())
                        .or_default()
                        .insert("value".to_owned(), value);
                }
                if self.script_state.has_listener(&id, event_type) {
                    let effects = self.script_state.fire_event(&id, event_type, None);
                    self.apply_script_effects(effects, ctx);
                }
            }
        }

        let central_frame = if self.render_debug.open {
            egui::Frame::new()
                .fill(self.document.style.page_background)
                .inner_margin(egui::Margin::same(0))
        } else {
            egui::Frame::new().fill(self.document.style.page_background)
        };

        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ctx, |ui| {
                let response = if self.render_debug.open {
                    ui.horizontal(|ui| {
                        ui.label("RenderGraph -> CanvasGraph");
                        ui.separator();
                        ui.label(format!(
                            "{} / {} canvas objects",
                            self.render_debug.object_limit,
                            self.document.canvas_graph.objects.len()
                        ));
                        ui.separator();
                        if ui.button("Export Debug").clicked() {
                            match export_render_debug_steps(&self.document.canvas_graph) {
                                Ok(count) => {
                                    self.status = format!(
                                        "Exported {count} debug frames to {}",
                                        Path::new(DEBUG_EXPORT_DIR).display()
                                    );
                                }
                                Err(error) => {
                                    self.status = format!("Debug export failed: {error}");
                                }
                            }
                        }
                        ui.separator();
                        if let Some(pointer) = ui.ctx().pointer_hover_pos() {
                            ui.label(format!("Mouse x={:.0} y={:.0}", pointer.x, pointer.y));
                        } else {
                            ui.label("Mouse outside window");
                        }
                    });
                    let max_objects = self.document.canvas_graph.objects.len();
                    ui.scope(|ui| {
                        let slider_margin = 8.0;
                        let slider_width = (ui.available_width() - slider_margin * 2.0).max(280.0);
                        ui.spacing_mut().slider_width = slider_width;
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.horizontal(|ui| {
                            ui.add_space(slider_margin);
                            ui.add_sized(
                                [slider_width, 32.0],
                                egui::Slider::new(
                                    &mut self.render_debug.object_limit,
                                    0..=max_objects,
                                )
                                .show_value(false),
                            );
                            ui.add_space(slider_margin);
                        });
                    });
                    self.render_debug.object_limit =
                        self.render_debug.object_limit.min(max_objects);
                    ui.separator();

                    let mut debug_graph = self.debug_canvas_graph();
                    let _ = self.debug_canvas.canvas_graph_ui(
                        ui,
                        &self.document.style,
                        &mut debug_graph,
                    );
                    BrowserCanvasResponse::default()
                } else {
                    self.canvas.ui(ui, &mut self.document)
                };
                for input_change in &response.changed_inputs {
                    self.telemetry.emit(
                        "input.changed",
                        &[
                            ("label", &input_change.label),
                            ("value_len", &input_change.value_len.to_string()),
                        ],
                    );
                    self.status = format!(
                        "Input changed: {} ({} chars)",
                        input_change.label, input_change.value_len
                    );
                    if let Some(ref id) = input_change.element_id {
                        self.script_state
                            .dom
                            .attributes_by_id
                            .entry(id.clone())
                            .or_default()
                            .insert("value".to_owned(), input_change.value.clone());
                        if self.script_state.has_listener(id, "input") {
                            let effects = self.script_state.fire_event(id, "input", None);
                            self.apply_script_effects(effects, ctx);
                        }
                    }
                }
                if !response.submitted_inputs.is_empty() {
                    for input_submit in &response.submitted_inputs {
                        self.telemetry.emit(
                            "input.submitted",
                            &[
                                ("label", &input_submit.label),
                                ("value", &input_submit.value),
                            ],
                        );
                    }
                    if let Some(url) =
                        form_get_url_for_inputs(&response.submitted_inputs, &self.document.source)
                    {
                        self.open_link(&url, ctx);
                    }
                }
                let hovered_element_id = response.hovered.as_ref().and_then(|t| match t {
                    HitTarget::Button { element_id, .. } | HitTarget::Input { element_id, .. } => {
                        element_id.clone()
                    }
                    HitTarget::Link { .. } => None,
                });
                if let Some(target) = response.hovered {
                    match target {
                        HitTarget::Link { href } => {
                            self.status = format!("Link: {href}");
                        }
                        HitTarget::Button { text, .. } => {
                            self.status = format!("Button: {text}");
                        }
                        HitTarget::Input { label, .. } => {
                            self.status = format!("Input: {label}");
                        }
                    }
                }
                // Fire mouseover when hover enters a new element with a listener.
                if hovered_element_id != self.last_hovered_element_id {
                    if let Some(ref old_id) = self.last_hovered_element_id.clone() {
                        if self.script_state.has_listener(old_id, "mouseout") {
                            let effects = self.script_state.fire_event(old_id, "mouseout", None);
                            self.apply_script_effects(effects, ctx);
                        }
                    }
                    if let Some(ref new_id) = hovered_element_id {
                        if self.script_state.has_listener(new_id, "mouseover") {
                            let effects = self.script_state.fire_event(new_id, "mouseover", None);
                            self.apply_script_effects(effects, ctx);
                        }
                    }
                    self.last_hovered_element_id = hovered_element_id;
                }
                if let Some(target) = response.clicked {
                    match target {
                        HitTarget::Link { href } => {
                            self.telemetry
                                .emit("hit_test.clicked", &[("target", "link"), ("href", &href)]);
                            self.open_link(&href, ctx);
                        }
                        HitTarget::Button { text, element_id } => {
                            self.telemetry
                                .emit("hit_test.clicked", &[("target", "button"), ("text", &text)]);
                            self.status = format!("Clicked button {text}");
                            if let Some(id) = element_id {
                                if self.script_state.has_listener(&id, "click") {
                                    let effects = self.script_state.fire_event(&id, "click", None);
                                    self.apply_script_effects(effects, ctx);
                                }
                            }
                        }
                        HitTarget::Input { label, element_id } => {
                            self.telemetry.emit(
                                "hit_test.clicked",
                                &[("target", "input"), ("label", &label)],
                            );
                            if let Some(id) = element_id {
                                if self.script_state.has_listener(&id, "click") {
                                    let effects = self.script_state.fire_event(&id, "click", None);
                                    self.apply_script_effects(effects, ctx);
                                }
                            }
                        }
                    }
                }
                // Fire keydown for any registered listeners.
                let key_presses: Vec<String> = ctx.input(|state| {
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
                });
                if !key_presses.is_empty() {
                    let keydown_ids = self.script_state.all_element_ids_with_listener("keydown");
                    for key in &key_presses {
                        for id in &keydown_ids {
                            let effects = self.script_state.fire_event(id, "keydown", Some(key));
                            self.apply_script_effects(effects, ctx);
                        }
                    }
                }
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.telemetry.emit("app.window_closed", &[]);
    }
}

fn resolve_navigation_url(source: &str, href: &str) -> String {
    if href.starts_with('#') {
        return format!("{}{}", strip_url_fragment(source), href);
    }
    resolve_resource_url(source, href)
}

fn form_get_url_for_inputs(
    inputs: &[rich_canvas::InputSubmit],
    page_source: &str,
) -> Option<String> {
    let action = inputs.iter().find_map(|i| i.form_action.as_deref())?;
    let base_url = resolve_resource_url(page_source, action);
    let query: String = inputs
        .iter()
        .filter(|i| !i.value.trim().is_empty())
        .map(|i| {
            let key = i.name.as_deref().unwrap_or("q");
            format!(
                "{}={}",
                percent_encode_query(key),
                percent_encode_query(i.value.trim())
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    if query.is_empty() {
        return None;
    }
    let base = if base_url.contains('?') {
        format!("{base_url}&{query}")
    } else {
        format!("{base_url}?{query}")
    };
    Some(base)
}

fn percent_encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn same_document_url(current: &str, target: &str) -> bool {
    strip_url_fragment(current) == strip_url_fragment(target)
}

fn strip_url_fragment(input: &str) -> &str {
    input.split_once('#').map(|(base, _)| base).unwrap_or(input)
}

fn url_fragment(input: &str) -> Option<String> {
    input
        .split_once('#')
        .map(|(_, fragment)| fragment.to_owned())
        .filter(|fragment| !fragment.is_empty())
}

fn estimated_fragment_scroll_y(document: &BrowserDocument, fragment: &str) -> f32 {
    if fragment.is_empty() {
        return 0.0;
    }
    let fragment_text = fragment.replace(['-', '_'], " ").to_ascii_lowercase();
    document
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| {
            block_search_text(block)
                .to_ascii_lowercase()
                .contains(&fragment_text)
                .then_some(index as f32 * 150.0)
        })
        .unwrap_or(360.0)
}

fn block_search_text(block: &CanvasBlock) -> String {
    match block {
        CanvasBlock::Heading { text, .. }
        | CanvasBlock::Paragraph { text }
        | CanvasBlock::Link { text, .. }
        | CanvasBlock::ListItem { text, .. }
        | CanvasBlock::Quote { text }
        | CanvasBlock::Preformatted { text }
        | CanvasBlock::Media { label: text }
        | CanvasBlock::Button { text } => text.clone(),
        CanvasBlock::InlineText { spans } => spans.iter().map(|span| span.text.as_str()).collect(),
        CanvasBlock::Input { label, value } => format!("{label} {value}"),
        CanvasBlock::Panel { children } => children
            .iter()
            .map(block_search_text)
            .collect::<Vec<_>>()
            .join(" "),
        CanvasBlock::Table { caption, rows } => {
            let mut text = caption.clone();
            for row in rows {
                text.push(' ');
                text.push_str(&row.join(" "));
            }
            text
        }
        CanvasBlock::Image { alt, .. } => alt.clone(),
        CanvasBlock::Svg { .. } => String::new(),
        CanvasBlock::EcosiaHero { .. } | CanvasBlock::SearchResultsPage { .. } => String::new(),
        CanvasBlock::Box { children, .. } => children
            .iter()
            .map(block_search_text)
            .collect::<Vec<_>>()
            .join(" "),
        CanvasBlock::StyledBox { children, .. } => children
            .iter()
            .map(block_search_text)
            .collect::<Vec<_>>()
            .join(" "),
        CanvasBlock::Rule => String::new(),
    }
}

fn load_html_document(path: &Path) -> io::Result<BrowserDocument> {
    load_html_document_with_text_metrics(path, None)
}

fn load_html_document_with_text_metrics(
    path: &Path,
    text_metrics: Option<&egui::Context>,
) -> io::Result<BrowserDocument> {
    let html = fs::read_to_string(path)?;
    Ok(parse_html_document_with_text_metrics(
        &html,
        &path_to_file_url(path),
        text_metrics,
    ))
}

fn load_url_document(input: &str) -> io::Result<BrowserDocument> {
    load_url_document_with_optional_text_metrics(input, None)
}

fn load_url_document_with_text_metrics(
    input: &str,
    text_metrics: &egui::Context,
) -> io::Result<BrowserDocument> {
    load_url_document_with_optional_text_metrics(input, Some(text_metrics))
}

fn load_url_document_with_optional_text_metrics(
    input: &str,
    text_metrics: Option<&egui::Context>,
) -> io::Result<BrowserDocument> {
    let input = input.trim();
    if input.starts_with("http://") || input.starts_with("https://") {
        return load_http_document_with_text_metrics(input, text_metrics);
    }

    let path = input_to_path(input);
    load_html_document_with_text_metrics(&path, text_metrics)
}

fn load_url_source(input: &str) -> io::Result<LoadedPageSource> {
    let input = input.trim();
    if input.starts_with("http://") || input.starts_with("https://") {
        let response = http_client()?.get(input).send().map_err(io::Error::other)?;
        let final_url = response.url().to_string();
        let response = response.error_for_status().map_err(io::Error::other)?;
        let html = response.text().map_err(io::Error::other)?;
        return Ok(LoadedPageSource {
            html,
            source: final_url,
        });
    }

    let path = input_to_path(input);
    Ok(LoadedPageSource {
        html: fs::read_to_string(&path)?,
        source: path_to_file_url(&path),
    })
}

fn load_render_graph_debug_dump(input: &str) -> io::Result<String> {
    let input = input.trim();
    if input.starts_with("http://") || input.starts_with("https://") {
        let response = http_client()?.get(input).send().map_err(io::Error::other)?;
        let final_url = response.url().to_string();
        let response = response.error_for_status().map_err(io::Error::other)?;
        let html = response.text().map_err(io::Error::other)?;
        return Ok(parse_render_graph_debug_dump(&html, &final_url));
    }

    let path = input_to_path(input);
    let html = fs::read_to_string(&path)?;
    Ok(parse_render_graph_debug_dump(
        &html,
        &path_to_file_url(&path),
    ))
}

fn load_http_document(url: &str) -> io::Result<BrowserDocument> {
    load_http_document_with_text_metrics(url, None)
}

fn load_http_document_with_text_metrics(
    url: &str,
    text_metrics: Option<&egui::Context>,
) -> io::Result<BrowserDocument> {
    let response = http_client()?.get(url).send().map_err(io::Error::other)?;
    let final_url = response.url().to_string();
    let response = response.error_for_status().map_err(io::Error::other)?;
    let html = response.text().map_err(io::Error::other)?;

    Ok(parse_html_document_with_text_metrics(
        &html,
        &final_url,
        text_metrics,
    ))
}

fn http_client() -> io::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("{APP_TITLE}/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(io::Error::other)
}

fn load_linked_stylesheets(html: &str, source: &str) -> io::Result<String> {
    let mut css = String::new();
    for href in extract_stylesheet_hrefs(html) {
        let resolved = resolve_resource_url(source, &href);
        if !resource_allowed_for_document(source, &resolved) {
            continue;
        }
        let stylesheet = if resolved.starts_with("http://") || resolved.starts_with("https://") {
            http_client()?
                .get(&resolved)
                .send()
                .map_err(io::Error::other)?
                .error_for_status()
                .map_err(io::Error::other)?
                .text()
                .map_err(io::Error::other)?
        } else {
            fs::read_to_string(stylesheet_path_with_local_fallback(&resolved))?
        };
        css.push('\n');
        css.push_str(&stylesheet);
    }
    Ok(css)
}

fn stylesheet_path_with_local_fallback(resolved: &str) -> PathBuf {
    let path = input_to_path(resolved);
    if path.exists() {
        return path;
    }
    if path.file_name().and_then(|name| name.to_str()) == Some("style.css") {
        if let Some(parent) = path.parent() {
            let cached_latex_style = parent.join("latex_style.css");
            if cached_latex_style.exists() {
                return cached_latex_style;
            }
        }
    }
    path
}

fn extract_stylesheet_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut remaining = html;
    while let Some(index) = remaining.find("<link") {
        remaining = &remaining[index..];
        let Some(end) = remaining.find('>') else {
            break;
        };
        let tag = &remaining[..end + 1];
        if tag.contains("stylesheet") {
            if let Some(href) = extract_attr(tag, "href") {
                hrefs.push(href);
            }
        }
        remaining = &remaining[end + 1..];
    }
    hrefs
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
    if let Some(parent) = Path::new(source).parent() {
        return parent.join(href).display().to_string();
    }
    href.to_owned()
}

fn resource_allowed_for_document(source: &str, resource: &str) -> bool {
    !(is_local_document_source(source) && is_remote_url(resource))
}

fn is_local_document_source(source: &str) -> bool {
    source.starts_with("file://") || !is_remote_url(source)
}

fn is_remote_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DomDocument {
    children: Vec<DomNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DomNode {
    Element(DomElement),
    Text(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DomElement {
    tag_name: String,
    attributes: Vec<DomAttribute>,
    children: Vec<DomNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DomAttribute {
    name: String,
    value: String,
}

impl DomDocument {
    fn first_descendant_by_tag(&self, tag_name: &str) -> Option<&DomElement> {
        self.children
            .iter()
            .find_map(|child| child.first_descendant_by_tag(tag_name))
    }

    fn first_element_by_test_id(&self, test_id: &str) -> Option<&DomElement> {
        self.children
            .iter()
            .find_map(|child| child.first_element_by_test_id(test_id))
    }

    fn elements_by_test_id<'a>(&'a self, test_id: &str, out: &mut Vec<&'a DomElement>) {
        for child in &self.children {
            child.elements_by_test_id(test_id, out);
        }
    }
}

impl DomNode {
    fn first_descendant_by_tag(&self, tag_name: &str) -> Option<&DomElement> {
        match self {
            DomNode::Element(element) => element.first_descendant_by_tag(tag_name),
            DomNode::Text(_) => None,
        }
    }

    fn first_element_by_test_id(&self, test_id: &str) -> Option<&DomElement> {
        match self {
            DomNode::Element(element) => element.first_element_by_test_id(test_id),
            DomNode::Text(_) => None,
        }
    }

    fn elements_by_test_id<'a>(&'a self, test_id: &str, out: &mut Vec<&'a DomElement>) {
        if let DomNode::Element(element) = self {
            element.elements_by_test_id(test_id, out);
        }
    }
}

impl DomElement {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attr| attr.name.eq_ignore_ascii_case(name))
            .map(|attr| attr.value.as_str())
    }

    fn has_attr(&self, name: &str) -> bool {
        self.attr(name).is_some()
    }

    fn first_descendant_by_tag(&self, tag_name: &str) -> Option<&DomElement> {
        if self.tag_name.eq_ignore_ascii_case(tag_name) {
            return Some(self);
        }
        self.children
            .iter()
            .find_map(|child| child.first_descendant_by_tag(tag_name))
    }

    fn first_element_by_test_id(&self, test_id: &str) -> Option<&DomElement> {
        if self
            .attr("data-test-id")
            .is_some_and(|value| value.eq_ignore_ascii_case(test_id))
        {
            return Some(self);
        }
        self.children
            .iter()
            .find_map(|child| child.first_element_by_test_id(test_id))
    }

    fn elements_by_test_id<'a>(&'a self, test_id: &str, out: &mut Vec<&'a DomElement>) {
        if self
            .attr("data-test-id")
            .is_some_and(|value| value.eq_ignore_ascii_case(test_id))
        {
            out.push(self);
        }
        for child in &self.children {
            child.elements_by_test_id(test_id, out);
        }
    }

    fn first_descendant_by_tag_with_attr(&self, tag_name: &str, attr: &str) -> Option<&DomElement> {
        if self.tag_name.eq_ignore_ascii_case(tag_name) && self.has_attr(attr) {
            return Some(self);
        }
        self.children.iter().find_map(|child| match child {
            DomNode::Element(element) => element.first_descendant_by_tag_with_attr(tag_name, attr),
            DomNode::Text(_) => None,
        })
    }

    fn text_content(&self) -> String {
        let mut out = String::new();
        push_dom_text(&self.children, &mut out);
        normalize_ws(&decode_basic_entities(&out))
    }
}

fn push_dom_text(nodes: &[DomNode], out: &mut String) {
    for node in nodes {
        match node {
            DomNode::Text(text) => {
                out.push_str(text);
                out.push(' ');
            }
            DomNode::Element(element) if dom_element_text_is_visible(element) => {
                push_dom_text(&element.children, out);
            }
            DomNode::Element(_) => {}
        }
    }
}

fn dom_element_text_is_visible(element: &DomElement) -> bool {
    if element.has_attr("hidden") {
        return false;
    }
    if element
        .attr("aria-hidden")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    {
        return false;
    }
    if element.attr("class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|class| class == "sr-only" || class == "visually-hidden")
    }) {
        return false;
    }
    !element.attr("style").is_some_and(|style| {
        let style = style.to_ascii_lowercase().replace(' ', "");
        style.contains("display:none") || style.contains("visibility:hidden")
    })
}

fn parse_dom_document(html: &str) -> DomDocument {
    let (children, _) = parse_dom_nodes(html, None);
    DomDocument { children }
}

fn parse_dom_nodes<'a>(mut html: &'a str, closing_tag: Option<&str>) -> (Vec<DomNode>, &'a str) {
    let mut children = Vec::new();

    while !html.is_empty() {
        if let Some(text_end) = html.find('<') {
            if text_end > 0 {
                children.push(DomNode::Text(html[..text_end].to_owned()));
                html = &html[text_end..];
            }
        } else {
            children.push(DomNode::Text(html.to_owned()));
            return (children, "");
        }

        if html.starts_with("<!--") {
            let Some(end) = html.find("-->") else {
                return (children, "");
            };
            html = &html[end + 3..];
            continue;
        }

        if html.starts_with("<!") || html.starts_with("<?") {
            let Some(end) = html.find('>') else {
                return (children, "");
            };
            html = &html[end + 1..];
            continue;
        }

        if let Some(after_close) = html.strip_prefix("</") {
            let Some(end) = after_close.find('>') else {
                return (children, "");
            };
            let found_tag = after_close[..end].trim();
            html = &after_close[end + 1..];
            if closing_tag.is_some_and(|tag| found_tag.eq_ignore_ascii_case(tag)) {
                return (children, html);
            }
            continue;
        }

        let Some((open_tag, after_open)) = parse_dom_open_tag(html) else {
            children.push(DomNode::Text("<".to_owned()));
            html = &html[1..];
            continue;
        };
        html = after_open;

        let tag_name = open_tag.tag_name.clone();
        let mut element = DomElement {
            tag_name: tag_name.clone(),
            attributes: open_tag.attributes,
            children: Vec::new(),
        };

        if !open_tag.self_closing && !is_void_tag(&tag_name) {
            let (nested, after_nested) = parse_dom_nodes(html, Some(&tag_name));
            element.children = nested;
            html = after_nested;
        }

        children.push(DomNode::Element(element));
    }

    (children, html)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DomOpenTag {
    tag_name: String,
    attributes: Vec<DomAttribute>,
    self_closing: bool,
}

fn parse_dom_open_tag(html: &str) -> Option<(DomOpenTag, &str)> {
    let after_open = html.strip_prefix('<')?;
    let end = find_tag_end(after_open)?;
    let raw = &after_open[..end];
    let after_tag = &after_open[end + 1..];
    let self_closing = raw.trim_end().ends_with('/');
    let raw = raw.trim().trim_end_matches('/').trim_end();
    let name_end = raw
        .find(|ch: char| ch.is_whitespace() || ch == '/')
        .unwrap_or(raw.len());
    let tag_name = raw[..name_end].trim().to_ascii_lowercase();
    if tag_name.is_empty() {
        return None;
    }
    let attributes = parse_dom_attributes(&raw[name_end..]);
    Some((
        DomOpenTag {
            tag_name,
            attributes,
            self_closing,
        },
        after_tag,
    ))
}

fn find_tag_end(input: &str) -> Option<usize> {
    let mut quote = None;
    for (index, ch) in input.char_indices() {
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '>' => return Some(index),
            None => {}
        }
    }
    None
}

fn parse_dom_attributes(mut input: &str) -> Vec<DomAttribute> {
    let mut attributes = Vec::new();

    loop {
        input = input.trim_start();
        if input.is_empty() {
            return attributes;
        }

        let name_end = input
            .find(|ch: char| ch.is_whitespace() || ch == '=' || ch == '/' || ch == '>')
            .unwrap_or(input.len());
        let name = input[..name_end].trim().to_ascii_lowercase();
        input = &input[name_end..];
        if name.is_empty() {
            return attributes;
        }

        input = input.trim_start();
        let value = if let Some(after_equals) = input.strip_prefix('=') {
            input = after_equals.trim_start();
            if let Some(quote) = input.chars().next().filter(|ch| *ch == '"' || *ch == '\'') {
                let after_quote = &input[quote.len_utf8()..];
                if let Some(value_end) = after_quote.find(quote) {
                    input = &after_quote[value_end + quote.len_utf8()..];
                    decode_basic_entities(&after_quote[..value_end])
                } else {
                    let value = decode_basic_entities(after_quote);
                    input = "";
                    value
                }
            } else {
                let value_end = input
                    .find(|ch: char| ch.is_whitespace() || ch == '>')
                    .unwrap_or(input.len());
                let value = decode_basic_entities(&input[..value_end]);
                input = &input[value_end..];
                value
            }
        } else {
            String::new()
        };

        attributes.push(DomAttribute { name, value });
    }
}

fn parse_html_document(html: &str, source: &str) -> BrowserDocument {
    parse_html_document_with_text_metrics(html, source, None)
}

fn parse_html_document_with_text_metrics(
    html: &str,
    source: &str,
    text_metrics: Option<&egui::Context>,
) -> BrowserDocument {
    let html = remove_html_comments(html);
    let script_result = apply_safe_script_browser_effects_detailed(&html, Some(source));
    let reveal_hydration_hidden_content = script_result.hydration_failed;
    let html = script_result.html;
    let mut css = extract_tag_inner(&html, "style").unwrap_or("").to_owned();
    css.push_str(&load_linked_stylesheets(&html, source).unwrap_or_default());
    let html = remove_non_visual_metadata_elements(&html);
    let dom = parse_dom_document(&html);
    let title = dom
        .first_descendant_by_tag("title")
        .map(DomElement::text_content)
        .or_else(|| extract_tag_text(&html, "title"))
        .unwrap_or_else(|| "Untitled".to_owned())
        .trim()
        .to_owned();
    let root_classes = document_theme_root_classes(&dom, text_metrics);
    let style = parse_basic_css_for_viewport_with_root_classes(&css, 1280.0, &root_classes);
    let render_graph = build_render_graph(&dom, &style);
    let canvas_graph = render_graph_to_canvas_graph(
        &render_graph,
        source,
        style.image_height_auto,
        text_metrics,
        reveal_hydration_hidden_content,
    );
    let blocks = render_graph_to_blocks(&render_graph, source, style.image_height_auto);

    BrowserDocument {
        title,
        source: source.to_owned(),
        style,
        canvas_graph,
        blocks,
    }
}

fn parse_render_graph_debug_dump(html: &str, source: &str) -> String {
    let html = remove_html_comments(html);
    let html = apply_safe_script_browser_effects_with_source(&html, Some(source));
    let mut css = extract_tag_inner(&html, "style").unwrap_or("").to_owned();
    css.push_str(&load_linked_stylesheets(&html, source).unwrap_or_default());
    let html = remove_non_visual_metadata_elements(&html);
    let dom = parse_dom_document(&html);
    let root_classes = document_theme_root_classes(&dom, None);
    let style = parse_basic_css_for_viewport_with_root_classes(&css, 1280.0, &root_classes);
    let render_graph = build_render_graph(&dom, &style);
    render_graph_debug_string(&render_graph)
}

fn document_theme_root_classes(
    dom: &DomDocument,
    text_metrics: Option<&egui::Context>,
) -> Vec<String> {
    let mut classes: Vec<String> = dom
        .first_descendant_by_tag("html")
        .and_then(|element| element.attr("class"))
        .map(|classes| {
            classes
                .split_ascii_whitespace()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let has_explicit_theme = classes
        .iter()
        .any(|class| matches!(class.as_str(), "dark" | "light"));
    if !has_explicit_theme {
        if let Some(ctx) = text_metrics {
            classes.push(if ctx.style().visuals.dark_mode {
                "dark".to_owned()
            } else {
                "light".to_owned()
            });
        }
    }
    classes
}

#[derive(Clone, Debug)]
struct RenderGraph {
    root: RenderNode,
}

#[derive(Clone, Debug)]
struct RenderNode {
    kind: RenderNodeKind,
    style: ResolvedBoxStyle,
    children: Vec<RenderNode>,
}

#[derive(Clone, Debug)]
enum RenderNodeKind {
    Document,
    Element(DomElement),
    Text(String),
}

fn build_render_graph(dom: &DomDocument, document_style: &BrowserStyle) -> RenderGraph {
    let root_style = root_resolved_style(document_style);
    let root_children = dom
        .first_descendant_by_tag("body")
        .map(|body| {
            vec![build_render_element(
                body,
                None,
                None,
                &root_style,
                document_style,
            )]
        })
        .unwrap_or_else(|| build_render_children(&dom.children, None, &root_style, document_style));

    RenderGraph {
        root: RenderNode {
            kind: RenderNodeKind::Document,
            style: root_style,
            children: root_children,
        },
    }
}

fn build_render_node(
    node: &DomNode,
    parent_element: Option<&DomElement>,
    previous_element_sibling: Option<&DomElement>,
    parent_style: &ResolvedBoxStyle,
    document_style: &BrowserStyle,
) -> Option<RenderNode> {
    match node {
        DomNode::Text(text) => Some(RenderNode {
            kind: RenderNodeKind::Text(text.clone()),
            style: inherited_style_for_text(parent_style),
            children: Vec::new(),
        }),
        DomNode::Element(element) => Some(build_render_element(
            element,
            parent_element,
            previous_element_sibling,
            parent_style,
            document_style,
        )),
    }
}

fn inherited_style_for_text(parent: &ResolvedBoxStyle) -> ResolvedBoxStyle {
    ResolvedBoxStyle {
        display: CssDisplay::Inline,
        color: parent.color,
        font_size: parent.font_size,
        font_weight_bold: parent.font_weight_bold,
        font_style_italic: parent.font_style_italic,
        text_decoration_underline: parent.text_decoration_underline,
        text_decoration_strikethrough: parent.text_decoration_strikethrough,
        text_background: parent.text_background,
        text_align: parent.text_align,
        ..ResolvedBoxStyle::default()
    }
}

fn build_render_element(
    element: &DomElement,
    parent_element: Option<&DomElement>,
    previous_element_sibling: Option<&DomElement>,
    parent_style: &ResolvedBoxStyle,
    document_style: &BrowserStyle,
) -> RenderNode {
    let style = compute_render_style(
        element,
        parent_element,
        previous_element_sibling,
        parent_style,
        document_style,
    );
    let children = build_render_children(&element.children, Some(element), &style, document_style);

    RenderNode {
        kind: RenderNodeKind::Element(element.clone()),
        style,
        children,
    }
}

fn build_render_children(
    children: &[DomNode],
    parent_element: Option<&DomElement>,
    parent_style: &ResolvedBoxStyle,
    document_style: &BrowserStyle,
) -> Vec<RenderNode> {
    let mut out = Vec::new();
    let mut previous_element_sibling = None;
    for child in children {
        if let Some(node) = build_render_node(
            child,
            parent_element,
            previous_element_sibling,
            parent_style,
            document_style,
        ) {
            out.push(node);
        }
        if let DomNode::Element(element) = child {
            previous_element_sibling = Some(element);
        }
    }
    out
}

fn root_resolved_style(style: &BrowserStyle) -> ResolvedBoxStyle {
    ResolvedBoxStyle {
        color: style.text_color,
        background: style.page_background,
        font_size: style.body_font_size,
        ..ResolvedBoxStyle::default()
    }
}

fn compute_render_style(
    element: &DomElement,
    parent_element: Option<&DomElement>,
    previous_element_sibling: Option<&DomElement>,
    parent: &ResolvedBoxStyle,
    document_style: &BrowserStyle,
) -> ResolvedBoxStyle {
    let mut out = inherited_style_for_element(element, parent, document_style);
    let key = element_style_key_with_context(element, parent_element, previous_element_sibling);
    let matched = computed_box_style(document_style, &key);
    apply_css_box_style(&mut out, &matched, parent, document_style);
    if let Some(inline) = element.attr("style").and_then(parse_inline_box_style) {
        apply_css_box_style(&mut out, &inline, parent, document_style);
    }
    if (!dom_element_text_is_visible(element) && !dom_element_is_visual_replaced_content(element))
        || matches!(
            element.tag_name.as_str(),
            "script" | "style" | "template" | "noscript"
        )
    {
        out.display = CssDisplay::None;
    }
    normalize_heading_margins(&element.tag_name, &mut out);
    out
}

fn dom_element_is_visual_replaced_content(element: &DomElement) -> bool {
    matches!(
        element.tag_name.as_str(),
        "img" | "svg" | "audio" | "video" | "canvas" | "meter" | "progress" | "iframe"
    )
}

fn normalize_heading_margins(tag_name: &str, style: &mut ResolvedBoxStyle) {
    if !matches!(tag_name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
        return;
    }
    let max_top = style.font_size.max(1.0);
    let max_bottom = (style.font_size * 0.5).max(1.0);
    if style.margin.top > max_top {
        style.margin.top = max_top;
    }
    if style.margin.bottom > max_bottom {
        style.margin.bottom = max_bottom;
    }
}

fn inherited_style_for_element(
    element: &DomElement,
    parent: &ResolvedBoxStyle,
    document_style: &BrowserStyle,
) -> ResolvedBoxStyle {
    let mut style = ResolvedBoxStyle {
        display: default_display_for_tag(&element.tag_name),
        color: parent.color,
        background: egui::Color32::TRANSPARENT,
        font_size: parent.font_size,
        font_weight_bold: parent.font_weight_bold,
        font_style_italic: parent.font_style_italic,
        text_decoration_underline: parent.text_decoration_underline,
        text_decoration_strikethrough: parent.text_decoration_strikethrough,
        text_background: parent.text_background,
        text_align: parent.text_align,
        ..ResolvedBoxStyle::default()
    };

    match element.tag_name.as_str() {
        "body" => {
            style.background = document_style.page_background;
            style.padding = CssEdges {
                top: document_style.main_padding_y,
                right: document_style.main_padding_x,
                bottom: document_style.main_padding_y,
                left: document_style.main_padding_x,
            };
        }
        "h1" => {
            style.font_size = document_style.h1_font_size;
            style.font_weight_bold = true;
            style.margin = CssEdges {
                top: style.font_size * 0.67,
                right: 0.0,
                bottom: style.font_size * 0.67,
                left: 0.0,
            };
        }
        "h2" => {
            style.font_size = document_style.h2_font_size;
            style.font_weight_bold = true;
            style.margin = CssEdges {
                top: style.font_size * 0.83,
                right: 0.0,
                bottom: style.font_size * 0.83,
                left: 0.0,
            };
        }
        "h3" => {
            style.font_size = 20.0;
            style.font_weight_bold = true;
            style.margin = CssEdges {
                top: style.font_size,
                right: 0.0,
                bottom: style.font_size * 0.5,
                left: 0.0,
            };
        }
        "h4" => {
            style.font_size = 18.0;
            style.font_weight_bold = true;
            style.margin = CssEdges {
                top: style.font_size * 1.33,
                right: 0.0,
                bottom: style.font_size * 0.5,
                left: 0.0,
            };
        }
        "h5" | "h6" | "strong" | "b" | "th" => {
            style.font_weight_bold = true;
            if element.tag_name == "h5" || element.tag_name == "h6" {
                style.margin = CssEdges {
                    top: style.font_size * 1.67,
                    right: 0.0,
                    bottom: style.font_size * 0.5,
                    left: 0.0,
                };
            }
        }
        "p" => {
            style.margin = CssEdges {
                top: style.font_size,
                right: 0.0,
                bottom: style.font_size,
                left: 0.0,
            };
        }
        "em" | "i" | "cite" | "dfn" | "var" => style.font_style_italic = true,
        "u" | "ins" => style.text_decoration_underline = true,
        "del" | "s" => style.text_decoration_strikethrough = true,
        "mark" => style.text_background = egui::Color32::from_rgb(255, 245, 157),
        "a" => {
            style.color = document_style.link_color;
            style.text_decoration_underline = true;
        }
        "fieldset" => {
            style.border_width = 2.0;
            style.border_color = egui::Color32::from_rgb(160, 160, 160);
            style.padding = CssEdges {
                top: 6.0,
                right: 12.0,
                bottom: 10.0,
                left: 12.0,
            };
            style.margin = CssEdges {
                top: 0.0,
                right: 2.0,
                bottom: 4.0,
                left: 2.0,
            };
        }
        "button" => {
            style.background = egui::Color32::from_rgb(224, 224, 224);
            style.border_width = 1.0;
            style.border_color = egui::Color32::from_rgb(118, 118, 118);
            style.border_radius = 3;
            style.padding = CssEdges {
                top: 2.0,
                right: 8.0,
                bottom: 2.0,
                left: 8.0,
            };
            style.margin = CssEdges {
                top: 0.0,
                right: 4.0,
                bottom: 0.0,
                left: 0.0,
            };
        }
        "input" => {
            let input_type = element.attr("type").unwrap_or("text").to_ascii_lowercase();
            if matches!(input_type.as_str(), "submit" | "button" | "reset") {
                style.background = egui::Color32::from_rgb(224, 224, 224);
                style.border_width = 1.0;
                style.border_color = egui::Color32::from_rgb(118, 118, 118);
                style.border_radius = 3;
                style.padding = CssEdges {
                    top: 2.0,
                    right: 8.0,
                    bottom: 2.0,
                    left: 8.0,
                };
                style.margin = CssEdges {
                    top: 0.0,
                    right: 4.0,
                    bottom: 0.0,
                    left: 0.0,
                };
            }
        }
        _ => {}
    }

    style
}

fn default_display_for_tag(tag: &str) -> CssDisplay {
    match tag {
        "span" | "a" | "strong" | "b" | "em" | "i" | "small" | "code" | "kbd" | "samp" | "var"
        | "abbr" | "time" | "sub" | "sup" | "mark" | "del" | "s" | "ins" => CssDisplay::Inline,
        "img" | "input" | "textarea" | "select" | "button" | "svg" => CssDisplay::InlineBlock,
        "li" => CssDisplay::ListItem,
        "table" => CssDisplay::Table,
        "script" | "style" | "template" | "noscript" => CssDisplay::None,
        _ => CssDisplay::Block,
    }
}

fn apply_css_box_style(
    target: &mut ResolvedBoxStyle,
    source: &CssBoxStyle,
    parent: &ResolvedBoxStyle,
    document_style: &BrowserStyle,
) {
    let parent_width = parent
        .width
        .or(parent.max_width)
        .unwrap_or(document_style.main_max_width);

    if let Some(display) = source.display {
        target.display = display;
    }
    if let Some(color) = source.color {
        target.color = color;
    }
    if let Some(background) = source.background {
        target.background = background;
    }
    if let Some(margin) = source.margin {
        target.margin = margin;
    }
    if let Some(auto) = source.margin_auto.top {
        target.margin_auto.top = auto;
    }
    if let Some(auto) = source.margin_auto.right {
        target.margin_auto.right = auto;
    }
    if let Some(auto) = source.margin_auto.bottom {
        target.margin_auto.bottom = auto;
    }
    if let Some(auto) = source.margin_auto.left {
        target.margin_auto.left = auto;
    }
    if let Some(margin_top) = source.margin_top {
        target.margin.top = margin_top;
    }
    if let Some(margin_right) = source.margin_right {
        target.margin.right = margin_right;
    }
    if let Some(margin_bottom) = source.margin_bottom {
        target.margin.bottom = margin_bottom;
    }
    if let Some(margin_left) = source.margin_left {
        target.margin.left = margin_left;
    }
    if let Some(padding) = source.padding {
        target.padding = padding;
    }
    if let Some(padding_top) = source.padding_top {
        target.padding.top = padding_top;
    }
    if let Some(padding_right) = source.padding_right {
        target.padding.right = padding_right;
    }
    if let Some(padding_bottom) = source.padding_bottom {
        target.padding.bottom = padding_bottom;
    }
    if let Some(padding_left) = source.padding_left {
        target.padding.left = padding_left;
    }
    if let Some(width) = source.border_width {
        target.border_width = width;
    }
    if let Some(color) = source.border_color {
        target.border_color = color;
    }
    if let Some(radius) = source.border_radius {
        target.border_radius = radius;
    }
    if let Some(width) = source.width {
        target.width_percent = match width {
            CssLength::Percent(percent) => Some(percent),
            _ => None,
        };
        target.width = resolve_css_width(width, parent_width);
    }
    if let Some(max_width) = source.max_width {
        target.max_width = match max_width {
            CssLength::Auto => None,
            CssLength::Percent(percent) if percent >= 99.0 => None,
            _ => Some(resolve_css_length(max_width, parent_width)),
        };
    }
    if let Some(min_width) = source.min_width {
        target.min_width = resolve_optional_css_length(min_width, parent_width);
    }
    if let Some(height) = source.height {
        target.height = match height {
            CssLength::Auto => None,
            _ => Some(height),
        };
    }
    if let Some(min_height) = source.min_height {
        target.min_height = match min_height {
            CssLength::Auto => None,
            _ => Some(min_height),
        };
    }
    if let Some(font_size) = source.font_size {
        target.font_size = font_size;
    }
    if let Some(font_weight_bold) = source.font_weight_bold {
        target.font_weight_bold = font_weight_bold;
    }
    if let Some(font_style_italic) = source.font_style_italic {
        target.font_style_italic = font_style_italic;
    }
    if let Some(text_decoration_underline) = source.text_decoration_underline {
        target.text_decoration_underline = text_decoration_underline;
    }
    if let Some(text_decoration_strikethrough) = source.text_decoration_strikethrough {
        target.text_decoration_strikethrough = text_decoration_strikethrough;
    }
    if let Some(text_background) = source.text_background {
        target.text_background = text_background;
    }
    if let Some(text_align) = source.text_align {
        target.text_align = text_align;
    }
    if let Some(flex_grow) = source.flex_grow {
        target.flex_grow = flex_grow;
    }
    if let Some(flex_direction) = source.flex_direction {
        target.flex_direction = flex_direction;
    }
    if let Some(justify_content) = source.justify_content {
        target.justify_content = justify_content;
    }
    if let Some(align_items) = source.align_items {
        target.align_items = align_items;
    }
    if let Some(grid_template_columns) = source.grid_template_columns {
        target.grid_template_columns = Some(grid_template_columns);
    }
    if let Some(grid_template_areas) = &source.grid_template_areas {
        target.grid_template_areas = Some(grid_template_areas.clone());
    }
    if let Some(grid_area) = &source.grid_area {
        target.grid_area = Some(grid_area.clone());
    }
    if let Some(gap) = source.gap {
        target.gap = gap;
    }
    if let Some(visibility_visible) = source.visibility_visible {
        target.visibility_visible = visibility_visible;
    }
    if let Some(opacity) = source.opacity {
        target.opacity = opacity;
    }
    if let Some(overflow_hidden) = source.overflow_hidden {
        target.overflow_hidden = overflow_hidden;
    }
    if let Some(position) = source.position {
        target.position = position;
    }
    if let Some(z_index) = source.z_index {
        target.z_index = Some(z_index);
    }
    if let Some(inset) = source.inset {
        target.inset = Some(inset);
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
    if let Some(object_fit) = source.object_fit {
        target.object_fit = object_fit;
    }
}

fn resolve_css_length(length: CssLength, parent_width: f32) -> f32 {
    match length {
        CssLength::Auto => parent_width,
        CssLength::Px(px) => px,
        CssLength::Percent(percent) => parent_width * percent / 100.0,
    }
}

fn resolve_optional_css_length(length: CssLength, parent_width: f32) -> Option<f32> {
    match length {
        CssLength::Auto => None,
        _ => Some(resolve_css_length(length, parent_width)),
    }
}

fn resolve_css_width(length: CssLength, parent_width: f32) -> Option<f32> {
    match length {
        CssLength::Auto => None,
        _ => Some(resolve_css_length(length, parent_width)),
    }
}

fn render_graph_to_blocks(
    graph: &RenderGraph,
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    render_children_to_blocks(&graph.root.children, source, image_height_auto)
}

#[derive(Clone, Debug)]
struct CanvasLayoutCursor {
    x: f32,
    y: f32,
    width: f32,
    list_depth: usize,
    list_stack: Vec<CanvasListContext>,
    form_stack: Vec<CanvasFormContext>,
    next_form_index: usize,
}

#[derive(Clone, Debug)]
struct CanvasListContext {
    ordered: bool,
    next_index: usize,
}

#[derive(Clone, Debug)]
struct CanvasFormContext {
    id: String,
    action: Option<String>,
}

#[derive(Clone, Debug)]
struct CanvasInlineRun {
    text: String,
    style: ResolvedBoxStyle,
    href: Option<String>,
}

#[derive(Clone, Debug)]
struct CanvasLineFragment {
    text: String,
    style: ResolvedBoxStyle,
    href: Option<String>,
    size: egui::Vec2,
    x_offset: f32,
}

#[derive(Clone, Debug, Default)]
struct CanvasLineBox {
    fragments: Vec<CanvasLineFragment>,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssLayoutKind {
    Document,
    Block,
    AnonymousBlock,
    Inline,
    Text,
}

#[derive(Clone, Debug)]
struct CssLayoutBox<'a> {
    kind: CssLayoutKind,
    node: Option<&'a RenderNode>,
    style: ResolvedBoxStyle,
    children: Vec<CssLayoutBox<'a>>,
    dimensions: CssLayoutDimensions,
    text: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct CssLayoutDimensions {
    content: egui::Rect,
    margin: CssEdges,
    border: CssEdges,
    padding: CssEdges,
}

impl Default for CssLayoutDimensions {
    fn default() -> Self {
        Self {
            content: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
            margin: CssEdges::default(),
            border: CssEdges::default(),
            padding: CssEdges::default(),
        }
    }
}

fn build_css_layout_tree(root: &RenderNode) -> CssLayoutBox<'_> {
    let mut root_box = CssLayoutBox {
        kind: CssLayoutKind::Document,
        node: Some(root),
        style: root.style.clone(),
        children: root
            .children
            .iter()
            .flat_map(build_css_layout_boxes)
            .collect(),
        dimensions: CssLayoutDimensions::default(),
        text: None,
    };
    root_box.children = fix_css_anonymous_blocks(root_box.children, &root_box.style);
    root_box
}

fn build_css_layout_boxes(node: &RenderNode) -> Vec<CssLayoutBox<'_>> {
    if node.style.display == CssDisplay::None {
        return Vec::new();
    }

    match &node.kind {
        RenderNodeKind::Document => node
            .children
            .iter()
            .flat_map(build_css_layout_boxes)
            .collect(),
        RenderNodeKind::Text(text) => {
            let text = decode_basic_entities(text);
            if normalize_ws(&text).is_empty() {
                Vec::new()
            } else {
                vec![CssLayoutBox {
                    kind: CssLayoutKind::Text,
                    node: Some(node),
                    style: node.style.clone(),
                    children: Vec::new(),
                    dimensions: CssLayoutDimensions::default(),
                    text: Some(text),
                }]
            }
        }
        RenderNodeKind::Element(_) => {
            let kind = css_layout_kind_from_display(node.style.display);
            let raw_children = node
                .children
                .iter()
                .flat_map(build_css_layout_boxes)
                .collect();
            let children = if matches!(node.style.display, CssDisplay::Flex | CssDisplay::Grid) {
                raw_children
            } else if css_layout_box_is_block_container(kind) {
                fix_css_anonymous_blocks(raw_children, &node.style)
            } else {
                raw_children
            };
            vec![CssLayoutBox {
                kind,
                node: Some(node),
                style: node.style.clone(),
                children,
                dimensions: CssLayoutDimensions::default(),
                text: None,
            }]
        }
    }
}

fn css_layout_kind_from_display(display: CssDisplay) -> CssLayoutKind {
    match display {
        CssDisplay::Inline | CssDisplay::InlineBlock => CssLayoutKind::Inline,
        CssDisplay::None => CssLayoutKind::Block,
        CssDisplay::Block
        | CssDisplay::Flex
        | CssDisplay::Grid
        | CssDisplay::Table
        | CssDisplay::ListItem => CssLayoutKind::Block,
    }
}

fn css_layout_box_is_block_container(kind: CssLayoutKind) -> bool {
    matches!(
        kind,
        CssLayoutKind::Document | CssLayoutKind::Block | CssLayoutKind::AnonymousBlock
    )
}

fn fix_css_anonymous_blocks<'a>(
    children: Vec<CssLayoutBox<'a>>,
    parent_style: &ResolvedBoxStyle,
) -> Vec<CssLayoutBox<'a>> {
    let has_block = children.iter().any(css_layout_box_is_block_level);
    let has_inline = children.iter().any(css_layout_box_is_inline_level);
    if !(has_block && has_inline) {
        return children;
    }

    let mut result = Vec::new();
    let mut inline_run = Vec::new();
    for child in children {
        if css_layout_box_is_block_level(&child) {
            flush_css_anonymous_inline_run(&mut result, &mut inline_run, parent_style);
            result.push(child);
        } else {
            inline_run.push(child);
        }
    }
    flush_css_anonymous_inline_run(&mut result, &mut inline_run, parent_style);
    result
}

fn flush_css_anonymous_inline_run<'a>(
    result: &mut Vec<CssLayoutBox<'a>>,
    inline_run: &mut Vec<CssLayoutBox<'a>>,
    parent_style: &ResolvedBoxStyle,
) {
    if inline_run.is_empty() {
        return;
    }
    result.push(CssLayoutBox {
        kind: CssLayoutKind::AnonymousBlock,
        node: None,
        style: parent_style.clone(),
        children: std::mem::take(inline_run),
        dimensions: CssLayoutDimensions::default(),
        text: None,
    });
}

fn css_layout_box_is_block_level(box_: &CssLayoutBox<'_>) -> bool {
    matches!(
        box_.kind,
        CssLayoutKind::Document | CssLayoutKind::Block | CssLayoutKind::AnonymousBlock
    )
}

fn css_layout_box_is_inline_level(box_: &CssLayoutBox<'_>) -> bool {
    matches!(box_.kind, CssLayoutKind::Inline | CssLayoutKind::Text)
}

fn layout_css_layout_tree(
    root: &mut CssLayoutBox<'_>,
    viewport_width: f32,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) {
    root.dimensions.content =
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(viewport_width.max(1.0), 0.0));
    let height = layout_css_block_children(root, source, image_height_auto, text_metrics);
    root.dimensions.content.max.y = root.dimensions.content.min.y + height;
}

fn layout_css_block_children(
    parent: &mut CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    let mut cursor_y = parent.dimensions.content.top();
    for child in &mut parent.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        layout_css_box(
            child,
            parent.dimensions.content.left(),
            cursor_y,
            parent.dimensions.content.width().max(1.0),
            parent.dimensions.content.height().max(0.0),
            source,
            image_height_auto,
            text_metrics,
        );
        cursor_y = css_margin_box(child).bottom();
    }
    (cursor_y - parent.dimensions.content.top()).max(0.0)
}

fn layout_css_box(
    box_: &mut CssLayoutBox<'_>,
    containing_x: f32,
    cursor_y: f32,
    containing_width: f32,
    containing_height: f32,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) {
    // Anonymous blocks are synthetic wrappers; they must not apply their parent's box model.
    if box_.kind == CssLayoutKind::AnonymousBlock {
        box_.dimensions.margin = CssEdges::default();
        box_.dimensions.padding = CssEdges::default();
        box_.dimensions.border = CssEdges::default();
    } else {
        box_.dimensions.margin = box_.style.margin;
        box_.dimensions.padding = box_.style.padding;
        box_.dimensions.border = CssEdges {
            top: box_.style.border_width,
            right: box_.style.border_width,
            bottom: box_.style.border_width,
            left: box_.style.border_width,
        };
    }

    let horizontal_non_content = box_.dimensions.margin.left
        + box_.dimensions.margin.right
        + box_.dimensions.border.left
        + box_.dimensions.border.right
        + box_.dimensions.padding.left
        + box_.dimensions.padding.right;
    let percent_width = box_
        .style
        .width_percent
        .map(|percent| (containing_width * percent / 100.0 - horizontal_non_content).max(1.0));
    let mut content_width =
        if let Some(width) = percent_width.or(box_.style.width).or(box_.style.max_width) {
            width
        } else if matches!(box_.kind, CssLayoutKind::Inline)
            && (box_.style.flex_grow > 0.0 || css_layout_box_contains_text_form_control(box_))
        {
            (containing_width - horizontal_non_content).max(1.0)
        } else if matches!(box_.kind, CssLayoutKind::Inline) {
            css_layout_preferred_content_width(box_, text_metrics)
        } else {
            (containing_width - horizontal_non_content).max(1.0)
        }
        .min(containing_width)
        .max(box_.style.min_width.unwrap_or(1.0));
    if box_.style.width.is_none()
        && box_.style.max_width.is_none()
        && let Some(node) = box_.node
        && let RenderNodeKind::Element(element) = &node.kind
        && element.tag_name == "svg"
    {
        let block = replaced_content_from_dom_element(element, source, image_height_auto);
        content_width = replaced_content_size(&block, content_width, box_.style.font_size)
            .x
            .min((containing_width - horizontal_non_content).max(1.0))
            .max(box_.style.min_width.unwrap_or(1.0));
    }

    let mut content_x = containing_x
        + box_.dimensions.margin.left
        + box_.dimensions.border.left
        + box_.dimensions.padding.left;
    if box_.style.width.is_none()
        && box_.style.max_width.is_some()
        && content_width + horizontal_non_content < containing_width
    {
        content_x += ((containing_width - content_width - horizontal_non_content) * 0.5).max(0.0);
    }
    let content_y = cursor_y
        + box_.dimensions.margin.top
        + box_.dimensions.border.top
        + box_.dimensions.padding.top;
    box_.dimensions.content = egui::Rect::from_min_size(
        egui::pos2(content_x, content_y),
        egui::vec2(content_width, 0.0),
    );

    let intrinsic_height = match box_.kind {
        CssLayoutKind::Document => {
            layout_css_block_children(box_, source, image_height_auto, text_metrics)
        }
        CssLayoutKind::AnonymousBlock => {
            if css_layout_box_contains_visual_replaced_content(box_) {
                layout_css_inline_visual_children(
                    box_,
                    source,
                    image_height_auto,
                    text_metrics,
                    content_width,
                )
            } else if css_layout_box_contains_replaced_or_special(box_) {
                layout_css_block_children(box_, source, image_height_auto, text_metrics)
            } else {
                measure_css_inline_children_height(&box_.children, content_width, text_metrics)
            }
        }
        CssLayoutKind::Text => box_
            .text
            .as_deref()
            .map(|text| {
                wrap_browser_textboxes(text_metrics, text, content_width, &box_.style)
                    .iter()
                    .map(|line| line.size.y.max((box_.style.font_size * 1.35).max(1.0)))
                    .sum::<f32>()
            })
            .unwrap_or(0.0),
        CssLayoutKind::Inline => {
            if let Some(node) = box_.node {
                if css_layout_node_is_button(node) {
                    layout_css_block_children(box_, source, image_height_auto, text_metrics)
                        .max((box_.style.font_size * 1.35).max(1.0))
                } else if css_layout_node_is_replaced_or_special(node) {
                    estimate_special_css_box_height(
                        node,
                        content_width,
                        source,
                        image_height_auto,
                        text_metrics,
                    )
                } else {
                    measure_css_inline_children_height(&box_.children, content_width, text_metrics)
                        .max((box_.style.font_size * 1.35).max(1.0))
                }
            } else {
                measure_css_inline_children_height(&box_.children, content_width, text_metrics)
                    .max((box_.style.font_size * 1.35).max(1.0))
            }
        }
        CssLayoutKind::Block => {
            if box_.style.display == CssDisplay::Flex {
                layout_css_flex_children(box_, source, image_height_auto, text_metrics)
            } else if box_.style.display == CssDisplay::Grid {
                layout_css_grid_children(box_, source, image_height_auto, text_metrics)
            } else if let Some(node) = box_.node {
                if css_layout_node_is_button(node) {
                    layout_css_block_children(box_, source, image_height_auto, text_metrics)
                        .max((box_.style.font_size * 1.35).max(1.0))
                } else if css_layout_node_is_replaced_or_special(node) {
                    estimate_special_css_box_height(
                        node,
                        content_width,
                        source,
                        image_height_auto,
                        text_metrics,
                    )
                } else if !box_.children.is_empty()
                    && box_.children.iter().all(css_layout_box_is_inline_level)
                    && !box_
                        .children
                        .iter()
                        .any(css_layout_box_contains_replaced_or_special)
                {
                    measure_css_inline_children_height(&box_.children, content_width, text_metrics)
                } else {
                    layout_css_block_children(box_, source, image_height_auto, text_metrics)
                }
            } else {
                layout_css_block_children(box_, source, image_height_auto, text_metrics)
            }
        }
    };

    let content_height = css_resolve_used_height(
        intrinsic_height,
        &box_.style,
        containing_height,
        content_width,
    );
    box_.dimensions.content.max.y = box_.dimensions.content.min.y + content_height.max(0.0);
    if box_.node.is_some_and(css_layout_node_is_button) {
        center_css_button_children(box_);
    }
    layout_css_out_of_flow_children(box_, source, image_height_auto, text_metrics);
}

fn layout_css_inline_visual_children(
    box_: &mut CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    containing_width: f32,
) -> f32 {
    let mut cursor_x = box_.dimensions.content.left();
    let cursor_y = box_.dimensions.content.top();
    let max_x = box_.dimensions.content.right();
    let mut line_height: f32 = 0.0;
    let mut max_bottom = cursor_y;

    for child in &mut box_.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        let child_size = layout_css_inline_visual_box(
            child,
            cursor_x,
            cursor_y,
            (max_x - cursor_x).max(1.0).min(containing_width.max(1.0)),
            source,
            image_height_auto,
            text_metrics,
        );
        cursor_x += child_size.x;
        line_height = line_height.max(child_size.y);
        max_bottom = max_bottom.max(css_margin_box(child).bottom());
    }

    (max_bottom - cursor_y).max(line_height).max(0.0)
}

fn layout_css_inline_visual_box(
    box_: &mut CssLayoutBox<'_>,
    containing_x: f32,
    cursor_y: f32,
    containing_width: f32,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) -> egui::Vec2 {
    box_.dimensions.margin = box_.style.margin;
    box_.dimensions.padding = box_.style.padding;
    box_.dimensions.border = CssEdges {
        top: box_.style.border_width,
        right: box_.style.border_width,
        bottom: box_.style.border_width,
        left: box_.style.border_width,
    };

    let horizontal_non_content = box_.dimensions.margin.left
        + box_.dimensions.margin.right
        + box_.dimensions.border.left
        + box_.dimensions.border.right
        + box_.dimensions.padding.left
        + box_.dimensions.padding.right;
    let content_x = containing_x
        + box_.dimensions.margin.left
        + box_.dimensions.border.left
        + box_.dimensions.padding.left;
    let content_y = cursor_y
        + box_.dimensions.margin.top
        + box_.dimensions.border.top
        + box_.dimensions.padding.top;
    let available_content_width = (containing_width - horizontal_non_content).max(1.0);

    if let Some(size) =
        css_visual_replaced_content_size(box_, available_content_width, source, image_height_auto)
    {
        box_.dimensions.content = egui::Rect::from_min_size(egui::pos2(content_x, content_y), size);
        return css_margin_box(box_).size();
    }

    if box_.kind == CssLayoutKind::Text {
        let text_width = box_
            .text
            .as_deref()
            .map(|text| measure_canvas_text_run(text_metrics, &normalize_ws(text), &box_.style).x)
            .unwrap_or(1.0)
            .min(available_content_width)
            .max(1.0);
        let text_height = (box_.style.font_size * 1.35).max(1.0);
        box_.dimensions.content = egui::Rect::from_min_size(
            egui::pos2(content_x, content_y),
            egui::vec2(text_width, text_height),
        );
        return css_margin_box(box_).size();
    }

    let mut child_x = content_x;
    let mut content_width: f32 = 0.0;
    let mut content_height: f32 = 0.0;
    for child in &mut box_.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        let child_size = layout_css_inline_visual_box(
            child,
            child_x,
            content_y,
            (available_content_width - content_width).max(1.0),
            source,
            image_height_auto,
            text_metrics,
        );
        child_x += child_size.x;
        content_width += child_size.x;
        content_height = content_height.max(child_size.y);
    }
    box_.dimensions.content = egui::Rect::from_min_size(
        egui::pos2(content_x, content_y),
        egui::vec2(
            content_width.max(1.0),
            content_height.max((box_.style.font_size * 1.35).max(1.0)),
        ),
    );
    css_margin_box(box_).size()
}

fn css_visual_replaced_content_size(
    box_: &CssLayoutBox<'_>,
    containing_width: f32,
    source: &str,
    image_height_auto: bool,
) -> Option<egui::Vec2> {
    let node = box_.node?;
    if !css_layout_node_is_visual_replaced_content(node) {
        return None;
    }
    let RenderNodeKind::Element(element) = &node.kind else {
        return None;
    };
    let content = replaced_content_from_dom_element(element, source, image_height_auto);
    Some(replaced_content_size(
        &content,
        containing_width,
        box_.style.font_size,
    ))
}

fn css_layout_node_is_button(node: &RenderNode) -> bool {
    matches!(
        &node.kind,
        RenderNodeKind::Element(element) if element.tag_name == "button"
    )
}

fn center_css_button_children(box_: &mut CssLayoutBox<'_>) {
    if box_.children.is_empty() {
        return;
    }
    let mut bounds: Option<egui::Rect> = None;
    for child in &box_.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        let child_rect = css_margin_box(child);
        bounds = Some(match bounds {
            Some(bounds) => bounds.union(child_rect),
            None => child_rect,
        });
    }
    let Some(bounds) = bounds else {
        return;
    };
    let content = box_.dimensions.content;
    let delta = egui::vec2(
        if box_.style.text_align == CssTextAlign::Center {
            content.center().x - bounds.center().x
        } else {
            0.0
        },
        content.center().y - bounds.center().y,
    );
    if delta == egui::Vec2::ZERO {
        return;
    }
    for child in &mut box_.children {
        if !css_layout_box_is_out_of_flow(child) {
            translate_css_layout_box(child, delta);
        }
    }
}

fn layout_css_flex_children(
    container: &mut CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    let is_row = container.style.flex_direction == CssFlexDirection::Row;
    let gap = container.style.gap.max(0.0);
    let content = container.dimensions.content;
    let flow_indices = container
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| (!css_layout_box_is_out_of_flow(child)).then_some(index))
        .collect::<Vec<_>>();
    let child_count = flow_indices.len();
    if child_count == 0 {
        layout_css_out_of_flow_children(container, source, image_height_auto, text_metrics);
        return 0.0;
    }

    let item_widths = if is_row {
        css_flex_row_item_widths(
            &flow_indices
                .iter()
                .map(|index| &container.children[*index])
                .collect::<Vec<_>>(),
            content.width(),
            gap,
            text_metrics,
        )
    } else {
        flow_indices
            .iter()
            .map(|index| {
                let child = &container.children[*index];
                child
                    .style
                    .width
                    .or(child.style.max_width)
                    .unwrap_or(content.width())
                    .min(content.width())
                    .max(child.style.min_width.unwrap_or(1.0))
            })
            .collect::<Vec<_>>()
    };

    let mut item_sizes = Vec::with_capacity(child_count);
    for (index, item_width) in flow_indices.iter().zip(item_widths) {
        let child = &mut container.children[*index];
        layout_css_box(
            child,
            0.0,
            0.0,
            item_width.max(1.0),
            content.height().max(0.0),
            source,
            image_height_auto,
            text_metrics,
        );
        item_sizes.push(css_margin_box(child).size());
    }

    let total_main = item_sizes
        .iter()
        .map(|size| if is_row { size.x } else { size.y })
        .sum::<f32>()
        + gap * child_count.saturating_sub(1) as f32;
    let cross_size = item_sizes
        .iter()
        .map(|size| if is_row { size.y } else { size.x })
        .fold(0.0, f32::max);
    let available_cross = if is_row {
        css_definite_box_height(&container.style, content.width())
            .unwrap_or(cross_size)
            .max(cross_size)
    } else {
        content.width().max(cross_size)
    };
    let available_main = if is_row {
        content.width()
    } else {
        css_definite_box_height(&container.style, content.width())
            .unwrap_or(total_main)
            .max(total_main)
    };
    let free_space = (available_main - total_main).max(0.0);
    let main_auto_margin_count = if is_row {
        flow_indices
            .iter()
            .map(|index| {
                let auto = container.children[*index].style.margin_auto;
                auto.left as usize + auto.right as usize
            })
            .sum::<usize>()
    } else {
        flow_indices
            .iter()
            .map(|index| {
                let auto = container.children[*index].style.margin_auto;
                auto.top as usize + auto.bottom as usize
            })
            .sum::<usize>()
    };
    let auto_margin_share = if main_auto_margin_count > 0 {
        free_space / main_auto_margin_count as f32
    } else {
        0.0
    };
    let mut main_cursor = if main_auto_margin_count > 0 {
        0.0
    } else {
        match container.style.justify_content {
            CssJustifyContent::Center => free_space * 0.5,
            CssJustifyContent::FlexStart | CssJustifyContent::SpaceBetween => 0.0,
        }
    };
    let distributed_gap = if main_auto_margin_count == 0
        && container.style.justify_content == CssJustifyContent::SpaceBetween
        && child_count > 1
    {
        gap + free_space / (child_count - 1) as f32
    } else {
        gap
    };

    for (slot, index) in flow_indices.iter().enumerate() {
        let child = &mut container.children[*index];
        let size = item_sizes[slot];
        let child_margin = child.dimensions.margin;
        let child_border = child.dimensions.border;
        let child_padding = child.dimensions.padding;
        let main_auto_before = if is_row && child.style.margin_auto.left {
            auto_margin_share
        } else if !is_row && child.style.margin_auto.top {
            auto_margin_share
        } else {
            0.0
        };
        let main_auto_after = if is_row && child.style.margin_auto.right {
            auto_margin_share
        } else if !is_row && child.style.margin_auto.bottom {
            auto_margin_share
        } else {
            0.0
        };
        let cross_offset = match container.style.align_items {
            CssAlignItems::Center => {
                ((available_cross - if is_row { size.y } else { size.x }) * 0.5).max(0.0)
            }
            CssAlignItems::Stretch | CssAlignItems::FlexStart => 0.0,
        };

        let content_x = if is_row {
            content.left()
                + main_cursor
                + main_auto_before
                + child_margin.left
                + child_border.left
                + child_padding.left
        } else {
            content.left()
                + cross_offset
                + child_margin.left
                + child_border.left
                + child_padding.left
        };
        let content_y = if is_row {
            content.top() + cross_offset + child_margin.top + child_border.top + child_padding.top
        } else {
            content.top()
                + main_cursor
                + main_auto_before
                + child_margin.top
                + child_border.top
                + child_padding.top
        };
        let current_min = child.dimensions.content.min;
        let new_min = egui::pos2(content_x, content_y);
        translate_css_layout_box(child, new_min - current_min);
        main_cursor += main_auto_before
            + if is_row { size.x } else { size.y }
            + main_auto_after
            + distributed_gap;
    }

    layout_css_out_of_flow_children(container, source, image_height_auto, text_metrics);
    if is_row { available_cross } else { total_main }
}

fn css_flex_row_item_widths(
    children: &[&CssLayoutBox<'_>],
    available_width: f32,
    gap: f32,
    text_metrics: Option<&egui::Context>,
) -> Vec<f32> {
    let child_count = children.len();
    let gap_total = gap * child_count.saturating_sub(1) as f32;
    let available_for_items = (available_width - gap_total).max(1.0);
    let total_grow = children
        .iter()
        .map(|child| css_flex_item_effective_grow(child))
        .sum::<f32>();
    let base_widths = children
        .iter()
        .map(|child| css_flex_item_base_width(child, available_width, text_metrics))
        .collect::<Vec<_>>();

    if total_grow <= 0.0 {
        let mut widths = children
            .iter()
            .zip(base_widths.iter())
            .map(|(child, width)| {
                width
                    .min(available_for_items)
                    .max(css_flex_item_min_width(child))
            })
            .collect::<Vec<_>>();
        shrink_css_flex_row_item_widths(&mut widths, children, available_for_items);
        return widths;
    }

    let base_total = base_widths.iter().sum::<f32>();
    let grow_available = (available_for_items - base_total).max(0.0);

    let mut widths = children
        .iter()
        .zip(base_widths.iter())
        .map(|(child, base_width)| {
            let grow = css_flex_item_effective_grow(child);
            if grow > 0.0 {
                (base_width + grow_available * grow / total_grow)
                    .max(css_flex_item_min_width(child))
            } else {
                base_width.max(css_flex_item_min_width(child))
            }
        })
        .collect::<Vec<_>>();
    shrink_css_flex_row_item_widths(&mut widths, children, available_for_items);
    widths
}

fn shrink_css_flex_row_item_widths(
    widths: &mut [f32],
    children: &[&CssLayoutBox<'_>],
    available_for_items: f32,
) {
    let total = widths.iter().sum::<f32>();
    if total <= available_for_items {
        return;
    }
    let shrinkable_total = widths
        .iter()
        .zip(children)
        .map(|(width, child)| (*width - css_flex_item_shrink_floor(child, *width)).max(0.0))
        .sum::<f32>();
    if shrinkable_total <= 0.0 {
        return;
    }
    let overflow = total - available_for_items;
    for (width, child) in widths.iter_mut().zip(children) {
        let min_width = css_flex_item_shrink_floor(child, *width);
        let shrinkable = (*width - min_width).max(0.0);
        *width = (*width - overflow * shrinkable / shrinkable_total)
            .max(min_width)
            .max(1.0);
    }
}

fn css_flex_item_effective_grow(child: &CssLayoutBox<'_>) -> f32 {
    if child.style.flex_grow > 0.0 {
        return child.style.flex_grow;
    }
    if child.style.width.is_none()
        && child.style.max_width.is_none()
        && css_layout_box_contains_text_form_control(child)
    {
        1.0
    } else {
        0.0
    }
}

fn css_flex_item_min_width(child: &CssLayoutBox<'_>) -> f32 {
    if css_layout_box_contains_text_form_control(child) {
        child.style.min_width.unwrap_or(0.0)
    } else {
        child.style.min_width.unwrap_or(1.0)
    }
}

fn css_flex_item_shrink_floor(child: &CssLayoutBox<'_>, base_width: f32) -> f32 {
    if css_layout_box_contains_text_form_control(child) {
        css_flex_item_min_width(child)
    } else {
        base_width.max(css_flex_item_min_width(child))
    }
}

fn css_flex_item_base_width(
    child: &CssLayoutBox<'_>,
    available_width: f32,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    if let Some(width) = child.style.width.or(child.style.max_width) {
        return width.min(available_width).max(1.0);
    }

    let content_width = css_layout_preferred_content_width(child, text_metrics)
        .min(available_width)
        .max(1.0);
    let horizontal_non_content = child.style.margin.left
        + child.style.margin.right
        + child.style.border_width * 2.0
        + child.style.padding.left
        + child.style.padding.right;
    (content_width + horizontal_non_content)
        .min(available_width)
        .max(1.0)
}

fn css_layout_preferred_content_width(
    box_: &CssLayoutBox<'_>,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    match box_.kind {
        CssLayoutKind::Text => box_
            .text
            .as_deref()
            .map(|text| measure_canvas_text_run(text_metrics, &normalize_ws(text), &box_.style).x)
            .unwrap_or(1.0),
        CssLayoutKind::Inline | CssLayoutKind::AnonymousBlock | CssLayoutKind::Block => {
            if box_
                .node
                .is_some_and(css_layout_node_is_replaced_or_special)
            {
                if let Some(width) = box_.style.width.or(box_.style.max_width) {
                    return width.max(box_.style.min_width.unwrap_or(1.0)).max(1.0);
                }
                let text_width = if let Some(node) = box_.node {
                    if let RenderNodeKind::Element(element) = &node.kind {
                        if element.tag_name == "input" {
                            let input_type =
                                element.attr("type").unwrap_or("text").to_ascii_lowercase();
                            if matches!(input_type.as_str(), "submit" | "button" | "reset") {
                                let label = canvas_button_label(element, node);
                                measure_canvas_text_run(text_metrics, &label, &box_.style).x
                            } else {
                                0.0
                            }
                        } else {
                            let text = render_node_text_content(node);
                            if text.is_empty() {
                                0.0
                            } else {
                                measure_canvas_text_run(text_metrics, &text, &box_.style).x
                            }
                        }
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                return text_width.max(box_.style.font_size * 1.35).max(1.0);
            }
            box_.children
                .iter()
                .map(|child| css_layout_preferred_outer_width(child, text_metrics))
                .sum::<f32>()
                .max(1.0)
        }
        CssLayoutKind::Document => box_
            .children
            .iter()
            .map(|child| css_layout_preferred_outer_width(child, text_metrics))
            .fold(0.0, f32::max)
            .max(1.0),
    }
}

fn css_layout_preferred_outer_width(
    box_: &CssLayoutBox<'_>,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    css_layout_preferred_content_width(box_, text_metrics)
        + box_.style.margin.left
        + box_.style.margin.right
        + box_.style.border_width * 2.0
        + box_.style.padding.left
        + box_.style.padding.right
}

fn css_definite_box_height(style: &ResolvedBoxStyle, containing_width: f32) -> Option<f32> {
    let _ = containing_width;
    [style.height, style.min_height]
        .into_iter()
        .flatten()
        .find_map(|height| match height {
            CssLength::Px(px) => Some(px),
            CssLength::Auto | CssLength::Percent(_) => None,
        })
}

fn translate_css_layout_box(box_: &mut CssLayoutBox<'_>, delta: egui::Vec2) {
    if delta == egui::Vec2::ZERO {
        return;
    }
    box_.dimensions.content = box_.dimensions.content.translate(delta);
    for child in &mut box_.children {
        translate_css_layout_box(child, delta);
    }
}

fn layout_css_grid_children(
    container: &mut CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    let flow_count = container
        .children
        .iter()
        .filter(|child| !css_layout_box_is_out_of_flow(child))
        .count();
    if flow_count == 0 {
        return 0.0;
    }

    let gap = container.style.gap.max(0.0);
    let content = container.dimensions.content;
    if let Some(height) =
        layout_named_css_grid_children(container, source, image_height_auto, text_metrics, gap)
    {
        return height;
    }

    let columns = container
        .style
        .grid_template_columns
        .unwrap_or(flow_count)
        .clamp(1, flow_count);
    let auto_width =
        ((content.width() - gap * columns.saturating_sub(1) as f32) / columns as f32).max(1.0);
    let mut column = 0usize;
    let mut cursor_y = content.top();
    let mut row_height: f32 = 0.0;
    let mut max_bottom = content.top();

    for child in &mut container.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        if column >= columns {
            cursor_y += row_height + gap;
            column = 0;
            row_height = 0.0;
        }
        let cursor_x = content.left() + column as f32 * (auto_width + gap);
        let item_width = child
            .style
            .width
            .or(child.style.max_width)
            .unwrap_or(auto_width)
            .min(auto_width)
            .max(child.style.min_width.unwrap_or(1.0));
        layout_css_box(
            child,
            cursor_x,
            cursor_y,
            item_width,
            content.height().max(0.0),
            source,
            image_height_auto,
            text_metrics,
        );
        let margin_box = css_margin_box(child);
        row_height = row_height.max(margin_box.height());
        max_bottom = max_bottom.max(margin_box.bottom());
        column += 1;
    }

    (max_bottom - content.top()).max(0.0)
}

fn layout_named_css_grid_children(
    container: &mut CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    gap: f32,
) -> Option<f32> {
    let areas = container.style.grid_template_areas.clone()?;
    if areas.is_empty() {
        return None;
    }

    let content = container.dimensions.content;
    let explicit_columns = container.style.grid_template_columns.unwrap_or(0);
    let area_columns = areas.iter().map(Vec::len).max().unwrap_or(0);
    let columns = explicit_columns.max(area_columns);
    if columns == 0 {
        return None;
    }

    let auto_width =
        ((content.width() - gap * columns.saturating_sub(1) as f32) / columns as f32).max(1.0);
    let mut placed_named_child = false;
    let mut row_heights: Vec<f32> = vec![0.0; areas.len()];

    for child in &mut container.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        let Some(bounds) = child
            .style
            .grid_area
            .as_deref()
            .and_then(|name| css_grid_area_bounds(&areas, name))
        else {
            continue;
        };
        placed_named_child = true;
        let item_width = css_grid_area_width(auto_width, gap, bounds);
        layout_css_box(
            child,
            content.left() + bounds.2 as f32 * (auto_width + gap),
            content.top(),
            item_width,
            content.height().max(0.0),
            source,
            image_height_auto,
            text_metrics,
        );
        let row_span = (bounds.1 - bounds.0 + 1).max(1);
        let height = (css_margin_box(child).height() - gap * row_span.saturating_sub(1) as f32)
            .max(0.0)
            / row_span as f32;
        for row_height in &mut row_heights[bounds.0..=bounds.1] {
            *row_height = (*row_height).max(height);
        }
    }

    if !placed_named_child {
        return None;
    }

    let mut row_tops = Vec::with_capacity(row_heights.len());
    let mut cursor_y = content.top();
    for row_height in &row_heights {
        row_tops.push(cursor_y);
        cursor_y += *row_height + gap;
    }
    let named_grid_bottom = row_heights
        .last()
        .and_then(|last_height| row_tops.last().map(|top| top + *last_height))
        .unwrap_or(content.top());
    let mut max_bottom = named_grid_bottom;

    for child in &mut container.children {
        if css_layout_box_is_out_of_flow(child) {
            continue;
        }
        let Some(bounds) = child
            .style
            .grid_area
            .as_deref()
            .and_then(|name| css_grid_area_bounds(&areas, name))
        else {
            continue;
        };
        let item_width = css_grid_area_width(auto_width, gap, bounds);
        layout_css_box(
            child,
            content.left() + bounds.2 as f32 * (auto_width + gap),
            row_tops[bounds.0],
            item_width,
            content.height().max(0.0),
            source,
            image_height_auto,
            text_metrics,
        );
        max_bottom = max_bottom.max(css_margin_box(child).bottom());
    }

    let mut column = 0usize;
    let mut fallback_y = if max_bottom > content.top() {
        max_bottom + gap
    } else {
        content.top()
    };
    let mut fallback_row_height: f32 = 0.0;

    for child in &mut container.children {
        if css_layout_box_is_out_of_flow(child)
            || child
                .style
                .grid_area
                .as_deref()
                .and_then(|name| css_grid_area_bounds(&areas, name))
                .is_some()
        {
            continue;
        }
        if column >= columns {
            fallback_y += fallback_row_height + gap;
            column = 0;
            fallback_row_height = 0.0;
        }
        let cursor_x = content.left() + column as f32 * (auto_width + gap);
        let item_width = child
            .style
            .width
            .or(child.style.max_width)
            .unwrap_or(auto_width)
            .min(auto_width)
            .max(child.style.min_width.unwrap_or(1.0));
        layout_css_box(
            child,
            cursor_x,
            fallback_y,
            item_width,
            content.height().max(0.0),
            source,
            image_height_auto,
            text_metrics,
        );
        let margin_box = css_margin_box(child);
        fallback_row_height = fallback_row_height.max(margin_box.height());
        max_bottom = max_bottom.max(margin_box.bottom());
        column += 1;
    }

    Some((max_bottom - content.top()).max(0.0))
}

fn css_grid_area_width(column_width: f32, gap: f32, bounds: (usize, usize, usize, usize)) -> f32 {
    let column_span = (bounds.3 - bounds.2 + 1).max(1);
    (column_width * column_span as f32 + gap * column_span.saturating_sub(1) as f32).max(1.0)
}

fn css_grid_area_bounds(areas: &[Vec<String>], name: &str) -> Option<(usize, usize, usize, usize)> {
    if name.is_empty() || name == "." {
        return None;
    }
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (row_index, row) in areas.iter().enumerate() {
        for (column_index, area_name) in row.iter().enumerate() {
            if area_name != name {
                continue;
            }
            bounds = Some(match bounds {
                Some((min_row, max_row, min_column, max_column)) => (
                    min_row.min(row_index),
                    max_row.max(row_index),
                    min_column.min(column_index),
                    max_column.max(column_index),
                ),
                None => (row_index, row_index, column_index, column_index),
            });
        }
    }
    bounds
}

fn css_layout_box_is_out_of_flow(box_: &CssLayoutBox<'_>) -> bool {
    if matches!(
        box_.style.position,
        CssPosition::Absolute | CssPosition::Fixed
    ) {
        return true;
    }
    false
}

fn css_resolve_used_height(
    intrinsic_height: f32,
    style: &ResolvedBoxStyle,
    containing_height: f32,
    containing_width: f32,
) -> f32 {
    let _ = containing_width;
    let mut height = style
        .height
        .and_then(|height| resolve_css_used_height_length(height, containing_height))
        .unwrap_or(intrinsic_height);
    if let Some(min_height) = style
        .min_height
        .and_then(|height| resolve_css_used_height_length(height, containing_height))
    {
        height = height.max(min_height);
    }
    height.max(0.0)
}

fn resolve_css_used_height_length(length: CssLength, percent_basis: f32) -> Option<f32> {
    match length {
        CssLength::Auto => None,
        CssLength::Px(px) => Some(px),
        CssLength::Percent(percent) if percent_basis > 0.0 => Some(percent_basis * percent / 100.0),
        CssLength::Percent(_) => None,
    }
}

fn layout_css_out_of_flow_children(
    parent: &mut CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) {
    let containing = css_padding_box(&parent.dimensions);
    if containing.width() <= 0.0 {
        return;
    }

    for child in &mut parent.children {
        if !css_layout_box_is_out_of_flow(child) {
            if child.style.position == CssPosition::Static {
                layout_css_out_of_flow_descendants_for_containing_block(
                    child,
                    containing,
                    source,
                    image_height_auto,
                    text_metrics,
                );
            }
            continue;
        }
        layout_css_out_of_flow_child(child, containing, source, image_height_auto, text_metrics);
    }
}

fn layout_css_out_of_flow_descendants_for_containing_block(
    box_: &mut CssLayoutBox<'_>,
    containing: egui::Rect,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) {
    for child in &mut box_.children {
        if css_layout_box_is_out_of_flow(child) {
            layout_css_out_of_flow_child(
                child,
                containing,
                source,
                image_height_auto,
                text_metrics,
            );
        } else if child.style.position == CssPosition::Static {
            layout_css_out_of_flow_descendants_for_containing_block(
                child,
                containing,
                source,
                image_height_auto,
                text_metrics,
            );
        }
    }
}

fn layout_css_out_of_flow_child(
    child: &mut CssLayoutBox<'_>,
    containing: egui::Rect,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) {
    let inset = child.style.inset.unwrap_or_default();
    let left = child.style.inset_sides.left;
    let right = child.style.inset_sides.right;
    let top = child.style.inset_sides.top;
    let bottom = child.style.inset_sides.bottom;
    let containing_width = if left.is_some() && right.is_some() {
        (containing.width() - inset.left - inset.right).max(1.0)
    } else {
        containing.width().max(1.0)
    };
    let containing_height = (containing.height() - inset.top - inset.bottom).max(0.0);
    layout_css_box(
        child,
        containing.left() + left.unwrap_or(0.0),
        containing.top() + top.unwrap_or(0.0),
        containing_width,
        containing_height,
        source,
        image_height_auto,
        text_metrics,
    );
    let child_margin_box = css_margin_box(child);
    let mut delta = egui::Vec2::ZERO;
    if let (Some(right), None) = (right, left) {
        delta.x = containing.right() - right - child_margin_box.right();
    }
    if let (Some(bottom), None) = (bottom, top) {
        delta.y = containing.bottom() - bottom - child_margin_box.bottom();
    }
    if delta != egui::Vec2::ZERO {
        translate_css_layout_box(child, delta);
    }
}

fn measure_css_inline_children_height(
    children: &[CssLayoutBox<'_>],
    width: f32,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    let mut runs = Vec::new();
    let mut pending_space = false;
    for child in children {
        collect_canvas_layout_inline_runs(child, None, &mut pending_space, &mut runs);
    }
    build_canvas_line_boxes(runs, text_metrics, width)
        .iter()
        .map(|line| line.height.max(1.0))
        .sum::<f32>()
}

fn css_layout_node_is_replaced_or_special(node: &RenderNode) -> bool {
    matches!(
        &node.kind,
        RenderNodeKind::Element(element)
            if matches!(
                element.tag_name.as_str(),
                "img"
                    | "input"
                    | "textarea"
                    | "select"
                    | "button"
                    | "table"
                    | "svg"
                    | "audio"
                    | "video"
                    | "canvas"
                    | "meter"
                    | "progress"
                    | "iframe"
            )
    )
}

fn estimate_special_css_box_height(
    node: &RenderNode,
    width: f32,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    let RenderNodeKind::Element(element) = &node.kind else {
        return (node.style.font_size * 1.35).max(1.0);
    };
    match element.tag_name.as_str() {
        "img" => match image_block_from_dom_element(element, source, image_height_auto) {
            CanvasBlock::Image { image, .. } => image.size.y,
            _ => (node.style.font_size * 3.0).max(48.0),
        },
        "table" => collect_table_rows(node)
            .iter()
            .map(|row| {
                let columns = row.len().max(1) as f32;
                table_row_height(row, (width / columns).max(24.0), 6.0, 5.0, text_metrics)
            })
            .sum::<f32>()
            .max((node.style.font_size * 1.35).max(1.0)),
        "svg" => {
            let block = replaced_content_from_dom_element(element, source, image_height_auto);
            replaced_content_size(&block, width, node.style.font_size).y
        }
        "textarea" => {
            let rows = element
                .attr("rows")
                .and_then(|r| r.parse::<f32>().ok())
                .unwrap_or(2.0);
            (node.style.font_size * 1.35 * rows).max(1.0)
        }
        "input" | "select" | "button" => (node.style.font_size * 1.35).max(1.0),
        _ => (node.style.font_size * 3.0).max(48.0),
    }
}

fn css_padding_box(dimensions: &CssLayoutDimensions) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            dimensions.content.left() - dimensions.padding.left,
            dimensions.content.top() - dimensions.padding.top,
        ),
        egui::pos2(
            dimensions.content.right() + dimensions.padding.right,
            dimensions.content.bottom() + dimensions.padding.bottom,
        ),
    )
}

fn css_border_box(dimensions: &CssLayoutDimensions) -> egui::Rect {
    let padding = css_padding_box(dimensions);
    egui::Rect::from_min_max(
        egui::pos2(
            padding.left() - dimensions.border.left,
            padding.top() - dimensions.border.top,
        ),
        egui::pos2(
            padding.right() + dimensions.border.right,
            padding.bottom() + dimensions.border.bottom,
        ),
    )
}

fn css_margin_box(box_: &CssLayoutBox<'_>) -> egui::Rect {
    let border = css_border_box(&box_.dimensions);
    egui::Rect::from_min_max(
        egui::pos2(
            border.left() - box_.dimensions.margin.left,
            border.top() - box_.dimensions.margin.top,
        ),
        egui::pos2(
            border.right() + box_.dimensions.margin.right,
            border.bottom() + box_.dimensions.margin.bottom,
        ),
    )
}

fn render_graph_to_canvas_graph(
    graph: &RenderGraph,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    reveal_hydration_hidden_content: bool,
) -> CanvasGraph {
    let viewport = egui::vec2(graph.root.style.max_width.unwrap_or(1280.0), 0.0);
    let mut layout_root = build_css_layout_tree(&graph.root);
    let mut canvas_graph = CanvasGraph {
        viewport,
        objects: Vec::new(),
    };
    let mut cursor = CanvasLayoutCursor {
        x: 0.0,
        y: 0.0,
        width: viewport.x,
        list_depth: 0,
        list_stack: Vec::new(),
        form_stack: Vec::new(),
        next_form_index: 0,
    };
    layout_css_layout_tree(
        &mut layout_root,
        viewport.x,
        source,
        image_height_auto,
        text_metrics,
    );
    if reveal_hydration_hidden_content {
        apply_hydration_visibility_fallback(&mut layout_root);
    }

    for child in &layout_root.children {
        push_canvas_graph_layout_box(
            child,
            source,
            image_height_auto,
            text_metrics,
            None,
            &mut cursor,
            &mut canvas_graph,
        );
    }
    canvas_graph.viewport.y = cursor.y.max(layout_root.dimensions.content.height());
    canvas_graph
}

fn apply_hydration_visibility_fallback(box_: &mut CssLayoutBox<'_>) {
    if css_layout_box_should_reveal_for_hydration_fallback(box_) {
        set_css_layout_subtree_visibility_visible(box_);
        return;
    }

    for child in &mut box_.children {
        apply_hydration_visibility_fallback(child);
    }
}

fn set_css_layout_subtree_visibility_visible(box_: &mut CssLayoutBox<'_>) {
    box_.style.visibility_visible = true;
    for child in &mut box_.children {
        set_css_layout_subtree_visibility_visible(child);
    }
}

fn css_layout_box_should_reveal_for_hydration_fallback(box_: &CssLayoutBox<'_>) -> bool {
    !box_.style.visibility_visible
        && box_.style.opacity > 0.0
        && matches!(
            box_.style.display,
            CssDisplay::Block | CssDisplay::Flex | CssDisplay::Grid | CssDisplay::ListItem
        )
        && box_.style.position == CssPosition::Static
        && css_layout_box_has_meaningful_ssr_content(box_)
}

fn css_layout_box_has_meaningful_ssr_content(box_: &CssLayoutBox<'_>) -> bool {
    let text_len = css_layout_box_visible_text_len(box_);
    let link_count = css_layout_box_link_count(box_);
    css_layout_box_contains_tag(box_, &["article", "main"]) || (text_len >= 24 && link_count > 0)
}

fn css_layout_box_visible_text_len(box_: &CssLayoutBox<'_>) -> usize {
    let mut total = box_
        .text
        .as_deref()
        .map(str::trim)
        .map(str::len)
        .unwrap_or(0);
    for child in &box_.children {
        total += css_layout_box_visible_text_len(child);
    }
    total
}

fn css_layout_box_link_count(box_: &CssLayoutBox<'_>) -> usize {
    let own = match &box_.node {
        Some(RenderNode {
            kind: RenderNodeKind::Element(element),
            ..
        }) if element.tag_name.eq_ignore_ascii_case("a") && element.attr("href").is_some() => 1,
        _ => 0,
    };
    own + box_
        .children
        .iter()
        .map(css_layout_box_link_count)
        .sum::<usize>()
}

fn css_layout_box_contains_tag(box_: &CssLayoutBox<'_>, tags: &[&str]) -> bool {
    if let Some(RenderNode {
        kind: RenderNodeKind::Element(element),
        ..
    }) = box_.node
    {
        if tags
            .iter()
            .any(|tag| element.tag_name.eq_ignore_ascii_case(tag))
        {
            return true;
        }
    }
    box_.children
        .iter()
        .any(|child| css_layout_box_contains_tag(child, tags))
}

fn push_canvas_graph_layout_box(
    box_: &CssLayoutBox<'_>,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let pushed_form = push_canvas_form_context_for_layout_box(box_, cursor);
    if !css_style_paints(&box_.style) {
        cursor.y = cursor.y.max(css_margin_box(box_).bottom());
        if pushed_form {
            cursor.form_stack.pop();
        }
        return;
    }
    match box_.kind {
        CssLayoutKind::Document => {
            for child in &box_.children {
                push_canvas_graph_layout_box(
                    child,
                    source,
                    image_height_auto,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            }
        }
        CssLayoutKind::AnonymousBlock => {
            if !box_.children.is_empty()
                && box_.children.iter().all(|c| {
                    css_layout_box_contains_visual_replaced_content(c)
                        || css_layout_box_contains_replaced_or_special(c)
                })
            {
                push_canvas_graph_layout_children_at_used_positions(
                    &box_.children,
                    source,
                    image_height_auto,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            } else {
                push_canvas_graph_layout_inline_children(
                    &box_.children,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            }
        }
        CssLayoutKind::Text => {
            if let Some(text) = &box_.text {
                push_canvas_graph_text(
                    text,
                    &box_.style,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            }
        }
        CssLayoutKind::Inline => {
            if let Some(node) = box_.node {
                let href = if let RenderNodeKind::Element(element) = &node.kind {
                    if element.tag_name == "a" {
                        element.attr("href").or(inherited_href)
                    } else {
                        inherited_href
                    }
                } else {
                    inherited_href
                };

                let previous_x = cursor.x;
                let previous_y = cursor.y;
                let previous_width = cursor.width;
                cursor.x = box_.dimensions.content.left();
                cursor.y = box_.dimensions.content.top();
                cursor.width = box_.dimensions.content.width().max(1.0);

                if css_layout_node_is_replaced_or_special(node) {
                    push_canvas_graph_layout_replaced_or_special(
                        box_,
                        node,
                        source,
                        image_height_auto,
                        text_metrics,
                        href,
                        cursor,
                        graph,
                    );
                } else {
                    push_canvas_graph_layout_box_background(box_, graph);
                    let clips_children = push_canvas_graph_layout_clip_start(box_, graph);
                    if !node.children.is_empty() && children_are_inline_flow(&node.children) {
                        push_canvas_graph_inline_children(
                            &node.children,
                            text_metrics,
                            href,
                            cursor,
                            graph,
                        );
                    } else {
                        for child in &node.children {
                            push_canvas_graph_node(
                                child,
                                source,
                                image_height_auto,
                                text_metrics,
                                href,
                                cursor,
                                graph,
                            );
                        }
                    }
                    if clips_children {
                        graph.objects.push(CanvasObject::ClipEnd);
                    }
                }

                cursor.x = previous_x;
                cursor.width = previous_width;
                cursor.y = previous_y.max(css_margin_box(box_).bottom());
            }
        }
        CssLayoutKind::Block => {
            let Some(node) = box_.node else {
                return;
            };
            if let RenderNodeKind::Element(element) = &node.kind {
                let href = if element.tag_name == "a" {
                    element.attr("href").or(inherited_href)
                } else {
                    inherited_href
                };
                match element.tag_name.as_str() {
                    "ul" | "ol" => {
                        cursor.list_stack.push(CanvasListContext {
                            ordered: element.tag_name == "ol",
                            next_index: 1,
                        });
                        cursor.list_depth = cursor.list_stack.len().saturating_sub(1);
                        push_canvas_graph_layout_children_at_used_positions(
                            &box_.children,
                            source,
                            image_height_auto,
                            text_metrics,
                            href,
                            cursor,
                            graph,
                        );
                        cursor.list_stack.pop();
                        cursor.list_depth = cursor.list_stack.len().saturating_sub(1);
                        cursor.y = css_margin_box(box_).bottom();
                        return;
                    }
                    "li" => {
                        let previous_x = cursor.x;
                        let previous_y = cursor.y;
                        let previous_width = cursor.width;
                        cursor.x = box_.dimensions.content.left();
                        cursor.y = box_.dimensions.content.top();
                        cursor.width = box_.dimensions.content.width().max(1.0);
                        push_canvas_graph_list_item(
                            node,
                            source,
                            image_height_auto,
                            text_metrics,
                            href,
                            cursor,
                            graph,
                        );
                        for child in &box_.children {
                            if let Some(child_node) = child.node {
                                if matches!(
                                    &child_node.kind,
                                    RenderNodeKind::Element(element)
                                        if element.tag_name == "ul" || element.tag_name == "ol"
                                ) {
                                    push_canvas_graph_layout_box(
                                        child,
                                        source,
                                        image_height_auto,
                                        text_metrics,
                                        href,
                                        cursor,
                                        graph,
                                    );
                                }
                            }
                        }
                        cursor.x = previous_x;
                        cursor.width = previous_width;
                        cursor.y = previous_y.max(css_margin_box(box_).bottom());
                        return;
                    }
                    "img" | "input" | "textarea" | "select" | "button" | "table" | "audio"
                    | "video" | "canvas" | "meter" | "progress" | "iframe" | "svg" => {
                        let previous_x = cursor.x;
                        let previous_y = cursor.y;
                        let previous_width = cursor.width;
                        cursor.x = box_.dimensions.content.left();
                        cursor.y = box_.dimensions.content.top();
                        cursor.width = box_.dimensions.content.width().max(1.0);
                        push_canvas_graph_layout_replaced_or_special(
                            box_,
                            node,
                            source,
                            image_height_auto,
                            text_metrics,
                            inherited_href,
                            cursor,
                            graph,
                        );
                        cursor.x = previous_x;
                        cursor.width = previous_width;
                        cursor.y = previous_y.max(css_margin_box(box_).bottom());
                        return;
                    }
                    _ => {
                        let previous_x = cursor.x;
                        let previous_y = cursor.y;
                        let previous_width = cursor.width;
                        cursor.x = box_.dimensions.content.left();
                        cursor.y = box_.dimensions.content.top();
                        cursor.width = box_.dimensions.content.width().max(1.0);
                        push_canvas_graph_layout_box_background(box_, graph);
                        let clips_children = push_canvas_graph_layout_clip_start(box_, graph);

                        if box_.style.display == CssDisplay::Flex {
                            push_canvas_graph_layout_children_at_used_positions(
                                &box_.children,
                                source,
                                image_height_auto,
                                text_metrics,
                                href,
                                cursor,
                                graph,
                            );
                            cursor.y = css_margin_box(box_).bottom();
                        } else if !box_.children.is_empty()
                            && box_.children.iter().all(css_layout_box_is_inline_level)
                            && !box_
                                .children
                                .iter()
                                .any(css_layout_box_contains_replaced_or_special)
                        {
                            push_canvas_graph_layout_inline_children(
                                &box_.children,
                                text_metrics,
                                href,
                                cursor,
                                graph,
                            );
                        } else {
                            push_canvas_graph_layout_children_at_used_positions(
                                &box_.children,
                                source,
                                image_height_auto,
                                text_metrics,
                                href,
                                cursor,
                                graph,
                            );
                        }

                        if clips_children {
                            graph.objects.push(CanvasObject::ClipEnd);
                        }
                        cursor.x = previous_x;
                        cursor.width = previous_width;
                        cursor.y = previous_y.max(css_margin_box(box_).bottom());
                    }
                }
            }
        }
    }
    if pushed_form {
        cursor.form_stack.pop();
    }
}

fn push_canvas_form_context_for_layout_box(
    box_: &CssLayoutBox<'_>,
    cursor: &mut CanvasLayoutCursor,
) -> bool {
    let Some(node) = box_.node else {
        return false;
    };
    let RenderNodeKind::Element(element) = &node.kind else {
        return false;
    };
    if element.tag_name != "form" {
        return false;
    }
    let id = element
        .attr("id")
        .or_else(|| element.attr("name"))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let id = format!("form-{}", cursor.next_form_index);
            cursor.next_form_index += 1;
            id
        });
    let action = element.attr("action").map(str::to_owned);
    cursor.form_stack.push(CanvasFormContext { id, action });
    true
}

fn push_canvas_graph_layout_clip_start(box_: &CssLayoutBox<'_>, graph: &mut CanvasGraph) -> bool {
    if !box_.style.overflow_hidden && box_.style.border_radius == 0 {
        return false;
    }
    graph
        .objects
        .push(CanvasObject::ClipStart(CanvasClipObject {
            rect: css_border_box(&box_.dimensions),
            border_radius: box_.style.border_radius,
        }));
    true
}

fn push_canvas_graph_layout_box_background(box_: &CssLayoutBox<'_>, graph: &mut CanvasGraph) {
    if box_.style.background != egui::Color32::TRANSPARENT || box_.style.border_width > 0.0 {
        graph.objects.push(CanvasObject::Rect(CanvasRectObject {
            rect: css_border_box(&box_.dimensions),
            fill: box_.style.background,
            border_color: box_.style.border_color,
            border_width: box_.style.border_width,
            border_radius: box_.style.border_radius,
        }));
    }
}

fn push_canvas_graph_layout_replaced_or_special(
    box_: &CssLayoutBox<'_>,
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let RenderNodeKind::Element(element) = &node.kind else {
        return;
    };
    match element.tag_name.as_str() {
        "img" | "svg" => {
            let content = replaced_content_from_dom_element(element, source, image_height_auto);
            push_canvas_graph_replaced_content_in_rect(
                &content,
                &node.style,
                box_.dimensions.content,
                graph,
            );
        }
        "button" => {
            push_canvas_graph_layout_box_background(box_, graph);
            if box_.children.is_empty() {
                let text = render_node_text_content(node);
                push_canvas_graph_text(
                    &text,
                    &node.style,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            } else {
                push_canvas_graph_layout_children_at_used_positions(
                    &box_.children,
                    source,
                    image_height_auto,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            }
            push_canvas_graph_button_hit_target(
                element,
                node,
                css_border_box(&box_.dimensions),
                cursor,
                graph,
            );
        }
        "input" => {
            let input_type = element.attr("type").unwrap_or("text").to_ascii_lowercase();
            if matches!(input_type.as_str(), "submit" | "button" | "reset") {
                push_canvas_graph_layout_box_background(box_, graph);
                let label = canvas_button_label(element, node);
                push_canvas_graph_text(&label, &node.style, text_metrics, inherited_href, cursor, graph);
                push_canvas_graph_button_hit_target(
                    element,
                    node,
                    css_border_box(&box_.dimensions),
                    cursor,
                    graph,
                );
            } else {
                push_canvas_graph_node(node, source, image_height_auto, text_metrics, inherited_href, cursor, graph);
            }
        }
        _ => push_canvas_graph_node(
            node,
            source,
            image_height_auto,
            text_metrics,
            inherited_href,
            cursor,
            graph,
        ),
    }
}

fn css_layout_box_contains_replaced_or_special(box_: &CssLayoutBox<'_>) -> bool {
    box_.node
        .is_some_and(css_layout_node_is_replaced_or_special)
        || box_
            .children
            .iter()
            .any(css_layout_box_contains_replaced_or_special)
}

fn css_layout_box_contains_text_form_control(box_: &CssLayoutBox<'_>) -> bool {
    box_.node.is_some_and(|node| {
        matches!(
            &node.kind,
            RenderNodeKind::Element(element)
                if matches!(element.tag_name.as_str(), "input" | "textarea" | "select")
        )
    }) || box_
        .children
        .iter()
        .any(css_layout_box_contains_text_form_control)
}

fn css_layout_box_contains_visual_replaced_content(box_: &CssLayoutBox<'_>) -> bool {
    box_.node
        .is_some_and(css_layout_node_is_visual_replaced_content)
        || box_
            .children
            .iter()
            .any(css_layout_box_contains_visual_replaced_content)
}

fn css_layout_node_is_visual_replaced_content(node: &RenderNode) -> bool {
    matches!(
        &node.kind,
        RenderNodeKind::Element(element)
            if matches!(
                element.tag_name.as_str(),
                "img" | "svg" | "audio" | "video" | "canvas" | "meter" | "progress" | "iframe"
            )
    )
}

fn push_canvas_graph_layout_children_at_used_positions(
    children: &[CssLayoutBox<'_>],
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let previous_x = cursor.x;
    let previous_y = cursor.y;
    let previous_width = cursor.width;
    let mut ordered_children: Vec<_> = children.iter().enumerate().collect();
    ordered_children.sort_by_key(|(index, child)| {
        let z_index = child.style.z_index.unwrap_or(0);
        let positioned_positive_z =
            child.style.position != CssPosition::Static && child.style.z_index.unwrap_or(0) > 0;
        (positioned_positive_z, z_index, *index)
    });
    for (_, child) in ordered_children {
        let margin_box = css_margin_box(child);
        if css_layout_box_is_block_level(child) {
            cursor.x = margin_box.left();
            cursor.y = margin_box.top();
            cursor.width = margin_box.width().max(1.0);
        } else {
            cursor.x = child.dimensions.content.left();
            cursor.y = child.dimensions.content.top();
            cursor.width = child.dimensions.content.width().max(1.0);
        }
        push_canvas_graph_layout_box(
            child,
            source,
            image_height_auto,
            text_metrics,
            inherited_href,
            cursor,
            graph,
        );
    }
    cursor.x = previous_x;
    cursor.y = previous_y;
    cursor.width = previous_width;
}

fn push_canvas_graph_node(
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    if node.style.display == CssDisplay::None || !css_style_paints(&node.style) {
        return;
    }

    match &node.kind {
        RenderNodeKind::Document => {
            for child in &node.children {
                push_canvas_graph_node(
                    child,
                    source,
                    image_height_auto,
                    text_metrics,
                    inherited_href,
                    cursor,
                    graph,
                );
            }
        }
        RenderNodeKind::Text(text) => {
            push_canvas_graph_text(
                text,
                &node.style,
                text_metrics,
                inherited_href,
                cursor,
                graph,
            );
        }
        RenderNodeKind::Element(element) => {
            push_canvas_graph_element(
                node,
                element,
                source,
                image_height_auto,
                text_metrics,
                inherited_href,
                cursor,
                graph,
            );
        }
    }
}

fn push_canvas_graph_element(
    node: &RenderNode,
    element: &DomElement,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let href = if element.tag_name == "a" {
        element.attr("href").or(inherited_href)
    } else {
        inherited_href
    };
    let pushed_form = push_canvas_form_context_for_element(element, cursor);

    match element.tag_name.as_str() {
        "img" => {
            push_canvas_graph_image(
                element,
                &node.style,
                source,
                image_height_auto,
                cursor,
                graph,
            );
        }
        "input" | "textarea" | "select" => {
            let input_type = element.attr("type").unwrap_or("text");
            if input_type.eq_ignore_ascii_case("hidden") {
                return;
            }
            // Button-like input types go to CanvasObject::Button.
            if element.tag_name == "input"
                && matches!(
                    input_type.to_ascii_lowercase().as_str(),
                    "submit" | "button" | "reset"
                )
            {
                let button_rect = egui::Rect::from_min_size(
                    egui::pos2(cursor.x, cursor.y),
                    egui::vec2(
                        cursor.width.max(1.0),
                        (node.style.font_size * 1.35).max(1.0),
                    ),
                );
                let label = canvas_button_label(element, node);
                push_canvas_graph_text(&label, &node.style, text_metrics, href, cursor, graph);
                push_canvas_graph_button_hit_target(element, node, button_rect, cursor, graph);
                return;
            }
            let kind = if element.tag_name == "textarea" {
                CanvasInputKind::TextArea
            } else if element.tag_name == "select" {
                let mut options = Vec::new();
                collect_select_options(&element.children, &mut options);
                CanvasInputKind::Select { options }
            } else {
                match input_type.to_ascii_lowercase().as_str() {
                    "checkbox" => CanvasInputKind::Checkbox,
                    "radio" => CanvasInputKind::Radio,
                    "password" => CanvasInputKind::Password,
                    _ => CanvasInputKind::Text,
                }
            };
            let line_height = (node.style.font_size * 1.35).max(1.0);
            let height = if element.tag_name == "textarea" {
                let rows = element
                    .attr("rows")
                    .and_then(|r| r.parse::<f32>().ok())
                    .unwrap_or(2.0);
                line_height * rows
            } else {
                line_height
            };
            let block = input_block_from_dom_element(element);
            if let CanvasBlock::Input { label, value } = block {
                // For checkboxes/radios, derive initial value from the `checked` attribute.
                let value = if matches!(kind, CanvasInputKind::Checkbox | CanvasInputKind::Radio) {
                    if element.attr("checked").is_some() {
                        "true".to_owned()
                    } else {
                        "false".to_owned()
                    }
                } else if let CanvasInputKind::Select { ref options } = kind {
                    // Use the first selected option, or the first option, or empty.
                    let selected = find_selected_option(&element.children);
                    selected.unwrap_or_else(|| options.first().cloned().unwrap_or(value))
                } else {
                    value
                };
                graph.objects.push(CanvasObject::Input(CanvasInputObject {
                    label,
                    name: element.attr("name").map(str::to_owned),
                    default_value: value.clone(),
                    value,
                    rect: egui::Rect::from_min_size(
                        egui::pos2(cursor.x, cursor.y),
                        egui::vec2(cursor.width.max(1.0), height),
                    ),
                    font_size: node.style.font_size,
                    color: node.style.color,
                    form_id: current_canvas_form_id(cursor),
                    form_action: current_canvas_form_action(cursor),
                    element_id: element.attr("id").map(str::to_owned),
                    kind,
                }));
            }
        }
        "button" => {
            let button_rect = egui::Rect::from_min_size(
                egui::pos2(cursor.x, cursor.y),
                egui::vec2(
                    cursor.width.max(1.0),
                    (node.style.font_size * 1.35).max(1.0),
                ),
            );
            if node.children.is_empty() {
                let text = render_node_text_content(node);
                push_canvas_graph_text(&text, &node.style, text_metrics, href, cursor, graph);
            } else {
                for child in &node.children {
                    push_canvas_graph_node(
                        child,
                        source,
                        image_height_auto,
                        text_metrics,
                        href,
                        cursor,
                        graph,
                    );
                }
            }
            push_canvas_graph_button_hit_target(element, node, button_rect, cursor, graph);
        }
        "table" => {
            push_canvas_graph_table(node, text_metrics, cursor, graph);
        }
        "ul" | "ol" => {
            cursor.list_stack.push(CanvasListContext {
                ordered: element.tag_name == "ol",
                next_index: 1,
            });
            cursor.list_depth = cursor.list_stack.len().saturating_sub(1);
            for child in &node.children {
                push_canvas_graph_node(
                    child,
                    source,
                    image_height_auto,
                    text_metrics,
                    href,
                    cursor,
                    graph,
                );
            }
            cursor.list_stack.pop();
            cursor.list_depth = cursor.list_stack.len().saturating_sub(1);
        }
        "svg" => {
            push_canvas_graph_replaced_content(
                &replaced_content_from_dom_element(element, source, image_height_auto),
                &node.style,
                cursor,
                graph,
            );
        }
        "audio" | "video" | "canvas" | "meter" | "progress" | "iframe" => {
            push_canvas_graph_media(
                element
                    .attr("alt")
                    .or_else(|| element.attr("src"))
                    .unwrap_or(element.tag_name.as_str()),
                &node.style,
                cursor,
                graph,
            );
        }
        "li" => {
            push_canvas_graph_list_item(
                node,
                source,
                image_height_auto,
                text_metrics,
                href,
                cursor,
                graph,
            );
            for child in &node.children {
                if let RenderNodeKind::Element(child_element) = &child.kind {
                    if child_element.tag_name == "ul" || child_element.tag_name == "ol" {
                        cursor.list_depth += 1;
                        push_canvas_graph_node(
                            child,
                            source,
                            image_height_auto,
                            text_metrics,
                            inherited_href,
                            cursor,
                            graph,
                        );
                        cursor.list_depth = cursor.list_depth.saturating_sub(1);
                    }
                }
            }
        }
        _ => {
            push_canvas_graph_box_start(&node.style, cursor, graph);
            let previous_x = cursor.x;
            let previous_width = cursor.width;
            cursor.x += node.style.margin.left + node.style.padding.left;
            cursor.width = (cursor.width
                - node.style.margin.left
                - node.style.margin.right
                - node.style.padding.left
                - node.style.padding.right)
                .max(1.0);

            if !node.children.is_empty() && children_are_inline_flow(&node.children) {
                push_canvas_graph_inline_children(
                    &node.children,
                    text_metrics,
                    href,
                    cursor,
                    graph,
                );
            } else {
                for child in &node.children {
                    push_canvas_graph_node(
                        child,
                        source,
                        image_height_auto,
                        text_metrics,
                        href,
                        cursor,
                        graph,
                    );
                }
            }

            cursor.x = previous_x;
            cursor.width = previous_width;
            cursor.y += node.style.padding.bottom + node.style.margin.bottom;
        }
    }
    if pushed_form {
        cursor.form_stack.pop();
    }
}

fn push_canvas_form_context_for_element(
    element: &DomElement,
    cursor: &mut CanvasLayoutCursor,
) -> bool {
    if element.tag_name != "form" {
        return false;
    }
    let id = element
        .attr("id")
        .or_else(|| element.attr("name"))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let id = format!("form-{}", cursor.next_form_index);
            cursor.next_form_index += 1;
            id
        });
    let action = element.attr("action").map(str::to_owned);
    cursor.form_stack.push(CanvasFormContext { id, action });
    true
}

fn current_canvas_form_id(cursor: &CanvasLayoutCursor) -> Option<String> {
    cursor.form_stack.last().map(|form| form.id.clone())
}

fn current_canvas_form_action(cursor: &CanvasLayoutCursor) -> Option<String> {
    cursor
        .form_stack
        .last()
        .and_then(|form| form.action.clone())
}

fn push_canvas_graph_button_hit_target(
    element: &DomElement,
    node: &RenderNode,
    rect: egui::Rect,
    cursor: &CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    graph.objects.push(CanvasObject::Button(CanvasButtonObject {
        text: canvas_button_label(element, node),
        rect,
        button_type: element.attr("type").unwrap_or("submit").to_owned(),
        form_id: current_canvas_form_id(cursor),
        form_action: current_canvas_form_action(cursor),
        element_id: element.attr("id").map(str::to_owned),
    }));
}

fn canvas_button_label(element: &DomElement, node: &RenderNode) -> String {
    element
        .attr("aria-label")
        .or_else(|| element.attr("title"))
        .or_else(|| element.attr("value"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let text = render_node_text_content(node).trim().to_owned();
            if text.is_empty() {
                "Button".to_owned()
            } else {
                text
            }
        })
}

fn push_canvas_graph_box_start(
    style: &ResolvedBoxStyle,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    cursor.y += style.margin.top + style.padding.top;
    if style.background != egui::Color32::TRANSPARENT || style.border_width > 0.0 {
        let rect = egui::Rect::from_min_size(
            egui::pos2(cursor.x + style.margin.left, cursor.y - style.padding.top),
            egui::vec2(
                style
                    .width
                    .or(style.max_width)
                    .unwrap_or(cursor.width)
                    .min(cursor.width)
                    .max(1.0),
                (style.padding.top + style.padding.bottom + style.font_size * 1.4).max(1.0),
            ),
        );
        graph.objects.push(CanvasObject::Rect(CanvasRectObject {
            rect,
            fill: style.background,
            border_color: style.border_color,
            border_width: style.border_width,
            border_radius: style.border_radius,
        }));
    }
}

fn push_canvas_graph_text(
    text: &str,
    style: &ResolvedBoxStyle,
    text_metrics: Option<&egui::Context>,
    href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let text = normalize_ws(&decode_basic_entities(text));
    if text.is_empty() {
        return;
    }
    for line in wrap_browser_textboxes(text_metrics, &text, cursor.width, style) {
        let line_height = line.size.y.max((style.font_size * 1.35).max(1.0));
        let rect = egui::Rect::from_min_size(
            egui::pos2(cursor.x, cursor.y),
            egui::vec2(line.size.x.min(cursor.width).max(1.0), line_height),
        );
        push_canvas_graph_text_object(line.text, rect, style, href, graph);
        cursor.y += line_height;
    }
}

fn push_canvas_graph_text_object(
    text: String,
    rect: egui::Rect,
    style: &ResolvedBoxStyle,
    href: Option<&str>,
    graph: &mut CanvasGraph,
) {
    graph.objects.push(CanvasObject::Text(CanvasTextObject {
        text,
        rect,
        color: style.color,
        font_size: style.font_size,
        font_weight_bold: style.font_weight_bold,
        font_style_italic: style.font_style_italic,
        text_decoration_underline: style.text_decoration_underline,
        text_decoration_strikethrough: style.text_decoration_strikethrough,
        text_background: style.text_background,
        text_align: style.text_align,
        href: href.map(str::to_owned),
    }));
}

fn children_are_inline_flow(children: &[RenderNode]) -> bool {
    children.iter().all(is_inline_flow_node)
}

fn is_inline_flow_node(node: &RenderNode) -> bool {
    match &node.kind {
        RenderNodeKind::Text(text) => !normalize_ws(text).is_empty(),
        RenderNodeKind::Element(element) => {
            if matches!(
                element.tag_name.as_str(),
                "br" | "img" | "input" | "textarea" | "select" | "button" | "svg"
            ) {
                return false;
            }
            node.style.display == CssDisplay::Inline && children_are_inline_flow(&node.children)
        }
        RenderNodeKind::Document => children_are_inline_flow(&node.children),
    }
}

fn push_canvas_graph_layout_inline_children(
    children: &[CssLayoutBox<'_>],
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let mut runs = Vec::new();
    let mut pending_space = false;
    for child in children {
        collect_canvas_layout_inline_runs(child, inherited_href, &mut pending_space, &mut runs);
    }
    push_canvas_line_boxes(runs, text_metrics, cursor, graph);
}

fn collect_canvas_layout_inline_runs(
    box_: &CssLayoutBox<'_>,
    inherited_href: Option<&str>,
    pending_space: &mut bool,
    runs: &mut Vec<CanvasInlineRun>,
) {
    if !css_style_paints(&box_.style) {
        return;
    }

    match box_.kind {
        CssLayoutKind::Text => {
            let Some(text) = &box_.text else {
                return;
            };
            let has_leading_space = text.chars().next().is_some_and(char::is_whitespace);
            let has_trailing_space = text.chars().last().is_some_and(char::is_whitespace);
            let mut text = normalize_ws(text);
            if text.is_empty() {
                if has_leading_space || has_trailing_space {
                    *pending_space = true;
                }
                return;
            }
            if !runs.is_empty() && (*pending_space || has_leading_space) {
                text.insert(0, ' ');
            }
            runs.push(CanvasInlineRun {
                text,
                style: box_.style.clone(),
                href: inherited_href.map(str::to_owned),
            });
            *pending_space = has_trailing_space;
        }
        CssLayoutKind::Inline => {
            let href = box_.node.and_then(|node| {
                if let RenderNodeKind::Element(element) = &node.kind {
                    if element.tag_name == "a" {
                        return element.attr("href").or(inherited_href);
                    }
                }
                inherited_href
            });
            for child in &box_.children {
                collect_canvas_layout_inline_runs(child, href, pending_space, runs);
            }
        }
        CssLayoutKind::AnonymousBlock | CssLayoutKind::Document | CssLayoutKind::Block => {
            for child in &box_.children {
                collect_canvas_layout_inline_runs(child, inherited_href, pending_space, runs);
            }
        }
    }
}

fn push_canvas_graph_inline_children(
    children: &[RenderNode],
    text_metrics: Option<&egui::Context>,
    inherited_href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let mut runs = Vec::new();
    let mut pending_space = false;
    for child in children {
        collect_canvas_inline_runs(child, inherited_href, &mut pending_space, &mut runs);
    }
    push_canvas_line_boxes(runs, text_metrics, cursor, graph);
}

fn collect_canvas_inline_runs(
    node: &RenderNode,
    inherited_href: Option<&str>,
    pending_space: &mut bool,
    runs: &mut Vec<CanvasInlineRun>,
) {
    if node.style.display == CssDisplay::None || !css_style_paints(&node.style) {
        return;
    }

    match &node.kind {
        RenderNodeKind::Text(text) => {
            let decoded = decode_basic_entities(text);
            let has_leading_space = decoded.chars().next().is_some_and(char::is_whitespace);
            let has_trailing_space = decoded.chars().last().is_some_and(char::is_whitespace);
            let mut text = normalize_ws(&decoded);
            if text.is_empty() {
                if decoded.chars().any(char::is_whitespace) && !runs.is_empty() {
                    *pending_space = true;
                }
                return;
            }
            if !runs.is_empty() && (*pending_space || has_leading_space) {
                text.insert(0, ' ');
            }
            runs.push(CanvasInlineRun {
                text,
                style: node.style.clone(),
                href: inherited_href.map(str::to_owned),
            });
            *pending_space = has_trailing_space;
        }
        RenderNodeKind::Element(element) => {
            let href = if element.tag_name == "a" {
                element.attr("href").or(inherited_href)
            } else {
                inherited_href
            };

            if is_inline_flow_node(node) {
                for child in &node.children {
                    collect_canvas_inline_runs(child, href, pending_space, runs);
                }
            }
        }
        RenderNodeKind::Document => {
            for child in &node.children {
                collect_canvas_inline_runs(child, inherited_href, pending_space, runs);
            }
        }
    }
}

fn css_style_paints(style: &ResolvedBoxStyle) -> bool {
    style.visibility_visible && style.opacity > 0.0
}

fn push_canvas_line_boxes(
    runs: Vec<CanvasInlineRun>,
    text_metrics: Option<&egui::Context>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let line_boxes = build_canvas_line_boxes(runs, text_metrics, cursor.width);
    if line_boxes.is_empty() {
        return;
    }

    for mut line in line_boxes {
        coalesce_canvas_line_fragments(&mut line);
        let align_offset = line_align_offset(&line, cursor.width);
        for fragment in line.fragments {
            let rect = egui::Rect::from_min_size(
                egui::pos2(cursor.x + align_offset + fragment.x_offset, cursor.y),
                egui::vec2(fragment.size.x.max(1.0), line.height.max(1.0)),
            );
            push_canvas_graph_text_object(
                fragment.text,
                rect,
                &fragment.style,
                fragment.href.as_deref(),
                graph,
            );
        }
        cursor.y += line.height.max(1.0);
    }
}

fn coalesce_canvas_line_fragments(line: &mut CanvasLineBox) {
    let mut coalesced: Vec<CanvasLineFragment> = Vec::new();
    for fragment in line.fragments.drain(..) {
        if let Some(last) = coalesced.last_mut() {
            if canvas_line_fragments_can_merge(last, &fragment) {
                last.text.push_str(&fragment.text);
                last.size.x += fragment.size.x;
                last.size.y = last.size.y.max(fragment.size.y);
                continue;
            }
        }
        coalesced.push(fragment);
    }
    line.fragments = coalesced;
}

fn canvas_line_fragments_can_merge(a: &CanvasLineFragment, b: &CanvasLineFragment) -> bool {
    a.href == b.href
        && a.style.color == b.style.color
        && a.style.font_size == b.style.font_size
        && a.style.font_weight_bold == b.style.font_weight_bold
        && a.style.font_style_italic == b.style.font_style_italic
        && a.style.text_decoration_underline == b.style.text_decoration_underline
        && a.style.text_decoration_strikethrough == b.style.text_decoration_strikethrough
        && a.style.text_background == b.style.text_background
        && a.style.text_align == b.style.text_align
}

fn build_canvas_line_boxes(
    runs: Vec<CanvasInlineRun>,
    text_metrics: Option<&egui::Context>,
    max_width: f32,
) -> Vec<CanvasLineBox> {
    let max_width = max_width.max(1.0);
    let mut lines = Vec::new();
    let mut current = CanvasLineBox::default();

    for run in runs {
        push_line_run(&mut lines, &mut current, &run, text_metrics, max_width);
    }

    if !current.fragments.is_empty() {
        lines.push(current);
    }
    lines
}

fn push_line_run(
    lines: &mut Vec<CanvasLineBox>,
    current: &mut CanvasLineBox,
    run: &CanvasInlineRun,
    text_metrics: Option<&egui::Context>,
    max_width: f32,
) {
    let text = if current.fragments.is_empty() {
        run.text.trim_start().to_owned()
    } else {
        run.text.clone()
    };
    if text.is_empty() {
        return;
    }

    let size = measure_canvas_text_run(text_metrics, &text, &run.style);
    if current.fragments.is_empty() {
        if size.x <= max_width {
            push_line_fragment(current, run, text, size);
            return;
        }
        push_line_run_tokens(lines, current, run, text, text_metrics, max_width);
        return;
    }

    if current.width + size.x <= max_width {
        push_line_fragment(current, run, text, size);
        return;
    }

    lines.push(std::mem::take(current));
    let text = text.trim_start().to_owned();
    if text.is_empty() {
        return;
    }
    let size = measure_canvas_text_run(text_metrics, &text, &run.style);
    if size.x <= max_width {
        push_line_fragment(current, run, text, size);
    } else {
        push_line_run_tokens(lines, current, run, text, text_metrics, max_width);
    }
}

fn push_line_run_tokens(
    lines: &mut Vec<CanvasLineBox>,
    current: &mut CanvasLineBox,
    run: &CanvasInlineRun,
    text: String,
    text_metrics: Option<&egui::Context>,
    max_width: f32,
) {
    for token in split_inline_run_tokens(&text) {
        if token.is_empty() {
            continue;
        }
        push_line_token(lines, current, run, token, text_metrics, max_width);
    }
}

fn push_line_token(
    lines: &mut Vec<CanvasLineBox>,
    current: &mut CanvasLineBox,
    run: &CanvasInlineRun,
    token: String,
    text_metrics: Option<&egui::Context>,
    max_width: f32,
) {
    let token = if current.fragments.is_empty() {
        token.trim_start().to_owned()
    } else {
        token
    };
    if token.is_empty() {
        return;
    }

    let size = measure_canvas_text_run(text_metrics, &token, &run.style);
    if !current.fragments.is_empty() && current.width + size.x > max_width {
        lines.push(std::mem::take(current));
        push_line_token(
            lines,
            current,
            run,
            token.trim_start().to_owned(),
            text_metrics,
            max_width,
        );
        return;
    }

    if size.x <= max_width || token.chars().count() <= 1 {
        push_line_fragment(current, run, token, size);
        return;
    }

    for character in token.chars() {
        let fragment = character.to_string();
        let size = measure_canvas_text_run(text_metrics, &fragment, &run.style);
        if !current.fragments.is_empty() && current.width + size.x > max_width {
            lines.push(std::mem::take(current));
        }
        push_line_fragment(current, run, fragment, size);
    }
}

fn push_line_fragment(
    line: &mut CanvasLineBox,
    run: &CanvasInlineRun,
    text: String,
    size: egui::Vec2,
) {
    let height = size.y.max((run.style.font_size * 1.35).max(1.0));
    line.fragments.push(CanvasLineFragment {
        text,
        style: run.style.clone(),
        href: run.href.clone(),
        size,
        x_offset: line.width,
    });
    line.width += size.x;
    line.height = line.height.max(height);
}

fn split_inline_run_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut previous_was_space = false;

    for character in text.chars() {
        if character.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            previous_was_space = true;
        } else {
            if previous_was_space && !tokens.is_empty() {
                current.push(' ');
            }
            current.push(character);
            previous_was_space = false;
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn line_align_offset(line: &CanvasLineBox, width: f32) -> f32 {
    let align = line
        .fragments
        .first()
        .map(|fragment| fragment.style.text_align)
        .unwrap_or(CssTextAlign::Left);
    match align {
        CssTextAlign::Left => 0.0,
        CssTextAlign::Center => ((width - line.width) * 0.5).max(0.0),
        CssTextAlign::Right => (width - line.width).max(0.0),
    }
}

fn push_canvas_graph_list_item(
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
    text_metrics: Option<&egui::Context>,
    href: Option<&str>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let previous_x = cursor.x;
    let previous_width = cursor.width;
    let depth_indent = cursor.list_depth as f32 * 22.0;
    let marker_width = 18.0;
    let content_x = cursor.x + depth_indent + marker_width;
    let content_width = (cursor.width - depth_indent - marker_width).max(1.0);
    let line_height = (node.style.font_size * 1.35).max(1.0);

    let marker_rect = egui::Rect::from_min_size(
        egui::pos2(cursor.x + depth_indent, cursor.y),
        egui::vec2(marker_width, line_height),
    );
    let marker = next_canvas_list_marker(cursor);
    push_canvas_graph_text_object(marker, marker_rect, &node.style, None, graph);

    cursor.x = content_x;
    cursor.width = content_width;
    let inline_children: Vec<RenderNode> = node
        .children
        .iter()
        .filter(|child| {
            !matches!(
                &child.kind,
                RenderNodeKind::Element(element)
                    if element.tag_name == "ul" || element.tag_name == "ol"
            )
        })
        .cloned()
        .collect();
    if inline_children.is_empty() {
        cursor.y += line_height;
    } else if children_are_inline_flow(&inline_children) {
        push_canvas_graph_inline_children(&inline_children, text_metrics, href, cursor, graph);
    } else {
        for child in &inline_children {
            push_canvas_graph_node(
                child,
                source,
                image_height_auto,
                text_metrics,
                href,
                cursor,
                graph,
            );
        }
    }
    cursor.x = previous_x;
    cursor.width = previous_width;
}

fn next_canvas_list_marker(cursor: &mut CanvasLayoutCursor) -> String {
    if let Some(context) = cursor.list_stack.last_mut() {
        if context.ordered {
            let marker = format!("{}.", context.next_index);
            context.next_index += 1;
            marker
        } else {
            "•".to_owned()
        }
    } else {
        "•".to_owned()
    }
}

fn measure_canvas_text_run(
    text_metrics: Option<&egui::Context>,
    text: &str,
    style: &ResolvedBoxStyle,
) -> egui::Vec2 {
    wrap_browser_textboxes(text_metrics, text, f32::INFINITY, style)
        .first()
        .map(|line| line.size)
        .unwrap_or_else(|| egui::vec2(1.0, (style.font_size * 1.35).max(1.0)))
}

fn push_canvas_graph_table(
    node: &RenderNode,
    text_metrics: Option<&egui::Context>,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let rows = collect_table_rows(node);
    if rows.is_empty() {
        return;
    }

    let table_width = cursor.width.max(1.0);
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let cell_width = (table_width / column_count as f32).max(24.0);
    let cell_padding_x = 6.0;
    let cell_padding_y = 5.0;
    let border_color = if node.style.border_color == egui::Color32::TRANSPARENT {
        egui::Color32::from_rgb(178, 187, 196)
    } else {
        node.style.border_color
    };
    let border_width = node.style.border_width.max(1.0);

    cursor.y += node.style.margin.top + node.style.padding.top;

    if let Some(caption) = table_caption_text(node) {
        push_canvas_graph_text(&caption, &node.style, text_metrics, None, cursor, graph);
        cursor.y += 4.0;
    }

    for (row_index, row) in rows.iter().enumerate() {
        let row_height = table_row_height(
            row,
            cell_width,
            cell_padding_x,
            cell_padding_y,
            text_metrics,
        );
        for column_index in 0..column_count {
            let cell = row.get(column_index);
            let cell_style = cell.map(|cell| &cell.style).unwrap_or(&node.style);
            let is_header = cell.is_some_and(|cell| is_table_header_cell(cell));
            let fill = table_cell_fill(cell_style, row_index, is_header);
            let rect = egui::Rect::from_min_size(
                egui::pos2(cursor.x + column_index as f32 * cell_width, cursor.y),
                egui::vec2(cell_width, row_height),
            );
            graph.objects.push(CanvasObject::Rect(CanvasRectObject {
                rect,
                fill,
                border_color,
                border_width,
                border_radius: 0,
            }));

            let Some(cell) = cell else {
                continue;
            };
            let text = render_node_text_content(cell);
            let text_width = (cell_width - cell_padding_x * 2.0).max(1.0);
            let line_height = (cell.style.font_size * 1.35).max(1.0);
            for (line_index, line) in
                wrap_browser_textboxes(text_metrics, &text, text_width, &cell.style)
                    .into_iter()
                    .enumerate()
            {
                let line_height = line.size.y.max(line_height);
                let text_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        rect.left() + cell_padding_x,
                        rect.top() + cell_padding_y + line_index as f32 * line_height,
                    ),
                    egui::vec2(text_width, line_height),
                );
                push_canvas_graph_text_object(line.text, text_rect, &cell.style, None, graph);
            }
        }
        cursor.y += row_height;
    }

    cursor.y += node.style.padding.bottom + node.style.margin.bottom;
}

fn collect_table_rows(node: &RenderNode) -> Vec<Vec<&RenderNode>> {
    let mut rows = Vec::new();
    collect_table_rows_inner(node, &mut rows);
    rows
}

fn collect_table_rows_inner<'a>(node: &'a RenderNode, rows: &mut Vec<Vec<&'a RenderNode>>) {
    if let RenderNodeKind::Element(element) = &node.kind {
        if element.tag_name == "tr" {
            let cells = node
                .children
                .iter()
                .filter(|child| {
                    matches!(
                        &child.kind,
                        RenderNodeKind::Element(element)
                            if element.tag_name == "th" || element.tag_name == "td"
                    )
                })
                .collect::<Vec<_>>();
            if !cells.is_empty() {
                rows.push(cells);
            }
            return;
        }
    }

    for child in &node.children {
        collect_table_rows_inner(child, rows);
    }
}

fn table_caption_text(node: &RenderNode) -> Option<String> {
    node.children.iter().find_map(|child| {
        if matches!(
            &child.kind,
            RenderNodeKind::Element(element) if element.tag_name == "caption"
        ) {
            let text = render_node_text_content(child);
            if text.is_empty() { None } else { Some(text) }
        } else {
            None
        }
    })
}

fn table_row_height(
    row: &[&RenderNode],
    cell_width: f32,
    padding_x: f32,
    padding_y: f32,
    text_metrics: Option<&egui::Context>,
) -> f32 {
    row.iter()
        .map(|cell| {
            let text_width = (cell_width - padding_x * 2.0).max(1.0);
            let lines = wrap_browser_textboxes(
                text_metrics,
                &render_node_text_content(cell),
                text_width,
                &cell.style,
            );
            let text_height = lines
                .iter()
                .map(|line| line.size.y.max((cell.style.font_size * 1.35).max(1.0)))
                .sum::<f32>()
                .max((cell.style.font_size * 1.35).max(1.0));
            padding_y * 2.0 + text_height
        })
        .fold(0.0, f32::max)
        .max(28.0)
}

fn is_table_header_cell(node: &RenderNode) -> bool {
    matches!(
        &node.kind,
        RenderNodeKind::Element(element) if element.tag_name == "th"
    )
}

fn table_cell_fill(style: &ResolvedBoxStyle, row_index: usize, is_header: bool) -> egui::Color32 {
    if style.background != egui::Color32::TRANSPARENT {
        style.background
    } else if is_header {
        egui::Color32::from_rgb(231, 236, 242)
    } else if row_index % 2 == 1 {
        egui::Color32::from_rgb(248, 250, 252)
    } else {
        egui::Color32::TRANSPARENT
    }
}

fn push_canvas_graph_image(
    element: &DomElement,
    style: &ResolvedBoxStyle,
    source: &str,
    image_height_auto: bool,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let content = replaced_content_from_dom_element(element, source, image_height_auto);
    push_canvas_graph_replaced_content(&content, style, cursor, graph);
}

fn push_canvas_graph_replaced_content(
    content: &CanvasBlock,
    style: &ResolvedBoxStyle,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    match content {
        CanvasBlock::Image { alt, src, image } => {
            let rect = egui::Rect::from_min_size(egui::pos2(cursor.x, cursor.y), image.size);
            cursor.y += image.size.y.max(style.font_size * 1.35);
            graph.objects.push(CanvasObject::Image(CanvasImageObject {
                rect,
                src: src.clone(),
                alt: alt.clone(),
                image: image.clone(),
                object_fit: style.object_fit,
            }));
        }
        CanvasBlock::Svg { svg } => {
            let rect = egui::Rect::from_min_size(egui::pos2(cursor.x, cursor.y), svg.size);
            cursor.y += svg.size.y.max(style.font_size * 1.35);
            graph.objects.push(CanvasObject::Svg(CanvasSvgObject {
                rect,
                svg: svg.clone(),
            }));
        }
        CanvasBlock::Media { label } => push_canvas_graph_media(label, style, cursor, graph),
        _ => {}
    }
}

fn push_canvas_graph_replaced_content_in_rect(
    content: &CanvasBlock,
    style: &ResolvedBoxStyle,
    rect: egui::Rect,
    graph: &mut CanvasGraph,
) {
    match content {
        CanvasBlock::Image { alt, src, image } => {
            let rect = expand_zero_replaced_rect(rect, image.size);
            graph.objects.push(CanvasObject::Image(CanvasImageObject {
                rect,
                src: src.clone(),
                alt: alt.clone(),
                image: image.clone(),
                object_fit: style.object_fit,
            }));
        }
        CanvasBlock::Svg { svg } => {
            let rect = expand_zero_replaced_rect(rect, svg.size);
            graph.objects.push(CanvasObject::Svg(CanvasSvgObject {
                rect,
                svg: svg.clone(),
            }));
        }
        CanvasBlock::Media { label } => {
            graph.objects.push(CanvasObject::Media(CanvasMediaObject {
                rect,
                label: label.clone(),
            }));
        }
        _ => {}
    }
}

fn push_canvas_graph_media(
    label: &str,
    style: &ResolvedBoxStyle,
    cursor: &mut CanvasLayoutCursor,
    graph: &mut CanvasGraph,
) {
    let height = (style.font_size * 3.0).max(48.0);
    let rect = egui::Rect::from_min_size(
        egui::pos2(cursor.x, cursor.y),
        egui::vec2(cursor.width.max(1.0), height),
    );
    graph.objects.push(CanvasObject::Media(CanvasMediaObject {
        rect,
        label: label.to_owned(),
    }));
    cursor.y += height;
}

fn render_graph_debug_string(graph: &RenderGraph) -> String {
    let mut out = String::new();
    push_render_node_debug(&graph.root, 0, &mut out);
    out
}

fn push_render_node_debug(node: &RenderNode, depth: usize, out: &mut String) {
    use std::fmt::Write as _;

    let indent = "  ".repeat(depth);
    match &node.kind {
        RenderNodeKind::Document => {
            let _ = writeln!(
                out,
                "{indent}Document {} children={}",
                resolved_style_debug(&node.style),
                node.children.len()
            );
        }
        RenderNodeKind::Element(element) => {
            let _ = writeln!(
                out,
                "{indent}Element <{}{}> {} children={}",
                element.tag_name,
                element_attr_debug(element),
                resolved_style_debug(&node.style),
                node.children.len()
            );
        }
        RenderNodeKind::Text(text) => {
            let _ = writeln!(
                out,
                "{indent}Text \"{}\" {}",
                shorten_debug_text(&decode_basic_entities(text)),
                resolved_style_debug(&node.style)
            );
        }
    }

    for child in &node.children {
        push_render_node_debug(child, depth + 1, out);
    }
}

fn resolved_style_debug(style: &ResolvedBoxStyle) -> String {
    format!(
        "style(display={:?}, color={}, bg={}, margin={}, padding={}, border_width={:.1}, border_color={}, radius={:.1}, width={}, min_width={}, max_width={}, font_size={:.1}, bold={}, align={:?}, visible={}, opacity={:.2}, overflow_hidden={}, position={:?}, z_index={})",
        style.display,
        color_debug(style.color),
        color_debug(style.background),
        edges_debug(style.margin),
        edges_debug(style.padding),
        style.border_width,
        color_debug(style.border_color),
        style.border_radius,
        optional_f32_debug(style.width),
        optional_f32_debug(style.min_width),
        optional_f32_debug(style.max_width),
        style.font_size,
        style.font_weight_bold,
        style.text_align,
        style.visibility_visible,
        style.opacity,
        style.overflow_hidden,
        style.position,
        style
            .z_index
            .map(|z_index| z_index.to_string())
            .unwrap_or_else(|| "auto".to_owned())
    )
}

fn element_attr_debug(element: &DomElement) -> String {
    let mut attrs = Vec::new();
    for name in ["id", "class", "href", "src", "type", "name", "role"] {
        if let Some(value) = element.attr(name) {
            attrs.push(format!(r#" {name}="{}""#, shorten_debug_text(value)));
        }
    }
    attrs.concat()
}

fn edges_debug(edges: CssEdges) -> String {
    format!(
        "{:.1}/{:.1}/{:.1}/{:.1}",
        edges.top, edges.right, edges.bottom, edges.left
    )
}

fn color_debug(color: egui::Color32) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.r(),
        color.g(),
        color.b(),
        color.a()
    )
}

fn optional_f32_debug(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "auto".to_owned())
}

fn shorten_debug_text(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 120 {
        compact.chars().take(117).collect::<String>() + "..."
    } else {
        compact
    }
}

fn render_children_to_blocks(
    children: &[RenderNode],
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    for child in children {
        blocks.extend(render_node_to_blocks(child, source, image_height_auto));
    }
    blocks
}

fn render_node_to_blocks(
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    if node.style.display == CssDisplay::None {
        return Vec::new();
    }

    match &node.kind {
        RenderNodeKind::Document => {
            render_children_to_blocks(&node.children, source, image_height_auto)
        }
        RenderNodeKind::Text(text) => {
            let text = normalize_ws(&decode_basic_entities(text));
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Paragraph { text }]
            }
        }
        RenderNodeKind::Element(element) => {
            render_element_to_blocks(node, element, source, image_height_auto)
        }
    }
}

fn render_element_to_blocks(
    node: &RenderNode,
    element: &DomElement,
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    match element.tag_name.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let text = render_node_text_content(node);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Heading {
                    level: element.tag_name[1..].parse::<u8>().unwrap_or(1),
                    text,
                }]
            }
        }
        "p" => paragraph_blocks_from_render_node(node, source, image_height_auto),
        "a" => {
            let text = render_node_text_content(node);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Link {
                    text,
                    href: element.attr("href").unwrap_or_default().to_owned(),
                }]
            }
        }
        "ul" => render_list_blocks(node, source, image_height_auto, false, 0),
        "ol" => render_list_blocks(node, source, image_height_auto, true, 0),
        "blockquote" => {
            let text = render_node_text_content(node);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Quote { text }]
            }
        }
        "pre" => {
            let text = render_node_text_content(node);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Preformatted { text }]
            }
        }
        "hr" => vec![CanvasBlock::Rule],
        "img" => vec![image_block_from_dom_element(
            element,
            source,
            image_height_auto,
        )],
        "input"
            if element
                .attr("type")
                .is_some_and(|value| value.eq_ignore_ascii_case("hidden")) =>
        {
            Vec::new()
        }
        "input" | "textarea" | "select" => vec![input_block_from_dom_element(element)],
        "button" => {
            let text = render_node_text_content(node);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Button { text }]
            }
        }
        "svg" => vec![svg_block_from_dom_element(element)],
        "audio" | "video" | "canvas" | "meter" | "progress" | "iframe" => {
            vec![CanvasBlock::Media {
                label: element
                    .attr("alt")
                    .or_else(|| element.attr("src"))
                    .unwrap_or(element.tag_name.as_str())
                    .to_owned(),
            }]
        }
        "table" => dom_table_block(element).into_iter().collect(),
        "li" => {
            let text = render_node_own_inline_text(node);
            let mut blocks = if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::ListItem {
                    depth: 0,
                    ordered: false,
                    text,
                    href: element_href(element),
                }]
            };
            blocks.extend(render_inline_embedded_blocks_from_render_node(
                node,
                source,
                image_height_auto,
            ));
            blocks.extend(render_children_to_blocks(
                &node.children,
                source,
                image_height_auto,
            ));
            blocks
        }
        _ => {
            let children = render_children_to_blocks(&node.children, source, image_height_auto);
            if children.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::StyledBox {
                    style: node.style.clone(),
                    children,
                }]
            }
        }
    }
}

fn render_node_text_content(node: &RenderNode) -> String {
    let mut out = String::new();
    push_render_node_text(node, &mut out);
    normalize_ws(&decode_basic_entities(&out))
}

fn render_node_own_inline_text(node: &RenderNode) -> String {
    let mut out = String::new();
    push_render_node_own_inline_text(node, &mut out);
    normalize_ws(&decode_basic_entities(&out))
}

fn push_render_node_text(node: &RenderNode, out: &mut String) {
    if node.style.display == CssDisplay::None {
        return;
    }
    match &node.kind {
        RenderNodeKind::Text(text) => {
            out.push_str(text);
            out.push(' ');
        }
        RenderNodeKind::Document | RenderNodeKind::Element(_) => {
            for child in &node.children {
                push_render_node_text(child, out);
            }
        }
    }
}

fn push_render_node_own_inline_text(node: &RenderNode, out: &mut String) {
    if node.style.display == CssDisplay::None {
        return;
    }
    match &node.kind {
        RenderNodeKind::Text(text) => {
            out.push_str(text);
            out.push(' ');
        }
        RenderNodeKind::Document => {
            for child in &node.children {
                push_render_node_own_inline_text(child, out);
            }
        }
        RenderNodeKind::Element(_) if is_block_boundary(node) => {}
        RenderNodeKind::Element(_) => {
            for child in &node.children {
                push_render_node_own_inline_text(child, out);
            }
        }
    }
}

fn is_block_boundary(node: &RenderNode) -> bool {
    matches!(
        node.style.display,
        CssDisplay::Block | CssDisplay::Flex | CssDisplay::Grid | CssDisplay::Table
    )
}

fn paragraph_blocks_from_render_node(
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    let links = render_node_links(node);
    let paragraph = render_node_text_content(node);
    let embedded = render_inline_embedded_blocks_from_render_node(node, source, image_height_auto);
    if paragraph.is_empty() {
        return embedded;
    }
    if links.len() == 1 && links[0].0 == paragraph {
        let mut blocks = vec![CanvasBlock::Link {
            text: links[0].0.clone(),
            href: links[0].1.clone(),
        }];
        blocks.extend(embedded);
        return blocks;
    }
    let mut blocks = vec![CanvasBlock::Paragraph { text: paragraph }];
    blocks.extend(embedded);
    blocks
}

fn render_inline_embedded_blocks_from_render_node(
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    collect_inline_embedded_blocks(node, true, source, image_height_auto, &mut blocks);
    blocks
}

fn collect_inline_embedded_blocks(
    node: &RenderNode,
    is_root: bool,
    source: &str,
    image_height_auto: bool,
    blocks: &mut Vec<CanvasBlock>,
) {
    if node.style.display == CssDisplay::None {
        return;
    }
    let RenderNodeKind::Element(element) = &node.kind else {
        return;
    };

    if !is_root && is_block_boundary(node) {
        return;
    }

    if is_embedded_inline_element(element) {
        blocks.extend(render_element_to_blocks(
            node,
            element,
            source,
            image_height_auto,
        ));
        return;
    }

    for child in &node.children {
        collect_inline_embedded_blocks(child, false, source, image_height_auto, blocks);
    }
}

fn is_embedded_inline_element(element: &DomElement) -> bool {
    matches!(
        element.tag_name.as_str(),
        "img"
            | "input"
            | "textarea"
            | "select"
            | "button"
            | "svg"
            | "audio"
            | "video"
            | "canvas"
            | "meter"
            | "progress"
            | "iframe"
    )
}

fn render_node_links(node: &RenderNode) -> Vec<(String, String)> {
    let mut links = Vec::new();
    collect_render_node_links(node, &mut links);
    links
}

fn collect_render_node_links(node: &RenderNode, links: &mut Vec<(String, String)>) {
    if node.style.display == CssDisplay::None {
        return;
    }
    if let RenderNodeKind::Element(element) = &node.kind {
        if element.tag_name.eq_ignore_ascii_case("a") {
            let text = render_node_text_content(node);
            if !text.is_empty() {
                links.push((text, element.attr("href").unwrap_or_default().to_owned()));
            }
        }
    }
    for child in &node.children {
        collect_render_node_links(child, links);
    }
}

fn render_list_blocks(
    node: &RenderNode,
    source: &str,
    image_height_auto: bool,
    ordered: bool,
    depth: usize,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    for child in &node.children {
        let RenderNodeKind::Element(item) = &child.kind else {
            continue;
        };
        if item.tag_name != "li" {
            blocks.extend(render_node_to_blocks(child, source, image_height_auto));
            continue;
        }
        let text = render_node_own_inline_text(child);
        if !text.is_empty() {
            blocks.push(CanvasBlock::ListItem {
                depth,
                ordered,
                text,
                href: element_href(item),
            });
        }
        blocks.extend(render_inline_embedded_blocks_from_render_node(
            child,
            source,
            image_height_auto,
        ));
        for nested in &child.children {
            if let RenderNodeKind::Element(nested_element) = &nested.kind {
                if nested_element.tag_name == "ul" || nested_element.tag_name == "ol" {
                    blocks.extend(render_list_blocks(
                        nested,
                        source,
                        image_height_auto,
                        nested_element.tag_name == "ol",
                        depth + 1,
                    ));
                }
            }
        }
    }
    blocks
}

fn dom_children_to_blocks(
    children: &[DomNode],
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    for child in children {
        match child {
            DomNode::Text(text) => {
                let text = normalize_ws(&decode_basic_entities(text));
                if !text.is_empty() {
                    blocks.push(CanvasBlock::Paragraph { text });
                }
            }
            DomNode::Element(element) => {
                blocks.extend(dom_element_to_blocks(element, source, image_height_auto));
            }
        }
    }
    blocks
}

fn dom_element_to_blocks(
    element: &DomElement,
    source: &str,
    image_height_auto: bool,
) -> Vec<CanvasBlock> {
    if !dom_element_text_is_visible(element)
        || matches!(
            element.tag_name.as_str(),
            "script" | "style" | "template" | "noscript"
        )
    {
        return Vec::new();
    }

    match element.tag_name.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let text = element.text_content();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Heading {
                    level: element.tag_name[1..].parse::<u8>().unwrap_or(1),
                    text,
                }]
            }
        }
        "p" => paragraph_blocks_from_dom(element),
        "a" => {
            let text = element.text_content();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Link {
                    text,
                    href: element.attr("href").unwrap_or_default().to_owned(),
                }]
            }
        }
        "ul" => dom_list_blocks(element, source, image_height_auto, false, 0),
        "ol" => dom_list_blocks(element, source, image_height_auto, true, 0),
        "blockquote" => {
            let text = element.text_content();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Quote { text }]
            }
        }
        "pre" => {
            let text = element.text_content();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Preformatted { text }]
            }
        }
        "hr" => vec![CanvasBlock::Rule],
        "img" => vec![image_block_from_dom_element(
            element,
            source,
            image_height_auto,
        )],
        "input"
            if element
                .attr("type")
                .is_some_and(|value| value.eq_ignore_ascii_case("hidden")) =>
        {
            Vec::new()
        }
        "input" | "textarea" | "select" => vec![input_block_from_dom_element(element)],
        "button" => {
            let text = element.text_content();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Button { text }]
            }
        }
        "svg" => vec![svg_block_from_dom_element(element)],
        "audio" | "video" | "canvas" | "meter" | "progress" | "iframe" => {
            vec![CanvasBlock::Media {
                label: element
                    .attr("alt")
                    .or_else(|| element.attr("src"))
                    .unwrap_or(element.tag_name.as_str())
                    .to_owned(),
            }]
        }
        "table" => dom_table_block(element).into_iter().collect(),
        "li" => {
            let text = element.text_content();
            let mut blocks = if text.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::ListItem {
                    depth: 0,
                    ordered: false,
                    text,
                    href: element_href(element),
                }]
            };
            blocks.extend(dom_children_to_blocks(
                &element.children,
                source,
                image_height_auto,
            ));
            blocks
        }
        _ => {
            let children = dom_children_to_blocks(&element.children, source, image_height_auto);
            if children.is_empty() {
                Vec::new()
            } else {
                vec![CanvasBlock::Box {
                    style_key: element_style_key(element),
                    children,
                }]
            }
        }
    }
}

fn paragraph_blocks_from_dom(element: &DomElement) -> Vec<CanvasBlock> {
    let links = element_links(element);
    let paragraph = element.text_content();
    if paragraph.is_empty() {
        return Vec::new();
    }
    if links.len() == 1 && links[0].0 == paragraph {
        return vec![CanvasBlock::Link {
            text: links[0].0.clone(),
            href: links[0].1.clone(),
        }];
    }
    vec![CanvasBlock::Paragraph { text: paragraph }]
}

fn dom_list_blocks(
    element: &DomElement,
    source: &str,
    image_height_auto: bool,
    ordered: bool,
    depth: usize,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    for child in &element.children {
        let DomNode::Element(item) = child else {
            continue;
        };
        if item.tag_name != "li" {
            blocks.extend(dom_element_to_blocks(item, source, image_height_auto));
            continue;
        }
        let text = item.text_content();
        if !text.is_empty() {
            blocks.push(CanvasBlock::ListItem {
                depth,
                ordered,
                text,
                href: element_href(item),
            });
        }
        for nested in &item.children {
            if let DomNode::Element(nested) = nested {
                if nested.tag_name == "ul" || nested.tag_name == "ol" {
                    blocks.extend(dom_list_blocks(
                        nested,
                        source,
                        image_height_auto,
                        nested.tag_name == "ol",
                        depth + 1,
                    ));
                }
            }
        }
    }
    blocks
}

fn element_style_key(element: &DomElement) -> ElementStyleKey {
    element_style_key_with_context(element, None, None)
}

fn element_style_key_with_context(
    element: &DomElement,
    parent: Option<&DomElement>,
    previous_sibling: Option<&DomElement>,
) -> ElementStyleKey {
    ElementStyleKey {
        tag: element.tag_name.clone(),
        id: element.attr("id").map(str::to_owned),
        classes: element
            .attr("class")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        attributes: element
            .attributes
            .iter()
            .map(|attribute| attribute.name.to_ascii_lowercase())
            .collect(),
        parent: parent.map(|parent| Box::new(element_style_key(parent))),
        previous_sibling: previous_sibling.map(|previous| Box::new(element_style_key(previous))),
    }
}

fn image_block_from_dom_element(
    element: &DomElement,
    source: &str,
    image_height_auto: bool,
) -> CanvasBlock {
    replaced_content_from_dom_element(element, source, image_height_auto)
}

fn replaced_content_from_dom_element(
    element: &DomElement,
    source: &str,
    image_height_auto: bool,
) -> CanvasBlock {
    if element.tag_name == "svg" {
        return svg_block_from_dom_element(element);
    }

    let label = element
        .attr("alt")
        .or_else(|| element.attr("src"))
        .unwrap_or("image")
        .to_owned();
    let Some(src) = element.attr("src").filter(|src| !src.is_empty()) else {
        return CanvasBlock::Media { label };
    };
    let resolved = resolve_resource_url(source, src);
    if !resource_allowed_for_document(source, &resolved) {
        return CanvasBlock::Media { label };
    }
    let requested_size = requested_image_size_from_dom(element);
    match load_image_resource(&resolved, requested_size, image_height_auto) {
        Ok(image) => CanvasBlock::Image {
            alt: element.attr("alt").unwrap_or_default().to_owned(),
            src: resolved,
            image,
        },
        Err(_) => CanvasBlock::Media { label },
    }
}

fn replaced_content_size(
    block: &CanvasBlock,
    fallback_width: f32,
    fallback_font_size: f32,
) -> egui::Vec2 {
    match block {
        CanvasBlock::Image { image, .. } => image.size,
        CanvasBlock::Svg { svg } => svg.size,
        CanvasBlock::Media { .. } => egui::vec2(
            fallback_width.max(1.0),
            (fallback_font_size * 3.0).max(48.0),
        ),
        _ => egui::vec2(
            fallback_width.max(1.0),
            (fallback_font_size * 1.35).max(1.0),
        ),
    }
}

fn expand_zero_replaced_rect(rect: egui::Rect, size: egui::Vec2) -> egui::Rect {
    if rect.width() > 0.0 && rect.height() > 0.0 {
        return rect;
    }
    egui::Rect::from_min_size(rect.min, egui::vec2(size.x.max(1.0), size.y.max(1.0)))
}

fn requested_image_size_from_dom(element: &DomElement) -> Option<egui::Vec2> {
    let width = element.attr("width").and_then(parse_html_dimension);
    let height = element.attr("height").and_then(parse_html_dimension);
    match (width, height) {
        (Some(width), Some(height)) => Some(egui::vec2(width, height)),
        _ => None,
    }
}

fn svg_block_from_dom_element(element: &DomElement) -> CanvasBlock {
    let (width, height) = svg_size_from_dom_element(element);
    if let Some(image) = rasterized_svg_image_from_dom_element(element, width, height) {
        return image;
    }

    let mut shapes = Vec::new();
    collect_dom_svg_shapes(element, &mut shapes);
    if shapes.is_empty() && dom_svg_contains_path(element) {
        shapes.push(SvgShape::PathFallback {
            fill: element_style_current_color(element).unwrap_or(egui::Color32::WHITE),
        });
    }
    if shapes.is_empty() {
        CanvasBlock::Media {
            label: element.attr("aria-label").unwrap_or("svg").to_owned(),
        }
    } else {
        CanvasBlock::Svg {
            svg: SvgBlock::new(egui::vec2(width, height), shapes),
        }
    }
}

fn svg_size_from_dom_element(element: &DomElement) -> (f32, f32) {
    let width = element.attr("width").and_then(parse_html_dimension);
    let height = element.attr("height").and_then(parse_html_dimension);
    if let (Some(width), Some(height)) = (width, height) {
        return (width, height);
    }
    if let Some((_, _, view_width, view_height)) =
        element.attr("viewBox").and_then(parse_svg_view_box)
    {
        return (
            width.unwrap_or(view_width.max(1.0)),
            height.unwrap_or(view_height.max(1.0)),
        );
    }
    (width.unwrap_or(100.0), height.unwrap_or(100.0))
}

fn parse_svg_view_box(value: &str) -> Option<(f32, f32, f32, f32)> {
    let values: Vec<f32> = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f32>().ok())
        .collect();
    match values.as_slice() {
        [min_x, min_y, width, height] if *width > 0.0 && *height > 0.0 => {
            Some((*min_x, *min_y, *width, *height))
        }
        _ => None,
    }
}

fn rasterized_svg_image_from_dom_element(
    element: &DomElement,
    width: f32,
    height: f32,
) -> Option<CanvasBlock> {
    let mut paths = Vec::new();
    collect_dom_svg_paths(
        element,
        element_style_current_color(element).unwrap_or(egui::Color32::WHITE),
        &mut paths,
    );
    if paths.is_empty() {
        return None;
    }

    let raster_width = width.round().clamp(1.0, 512.0) as u32;
    let raster_height = height.round().clamp(1.0, 512.0) as u32;
    let mut pixmap = Pixmap::new(raster_width, raster_height)?;
    let (min_x, min_y, view_width, view_height) = element
        .attr("viewBox")
        .and_then(parse_svg_view_box)
        .unwrap_or((0.0, 0.0, width.max(1.0), height.max(1.0)));
    let transform = Transform::from_row(
        raster_width as f32 / view_width.max(1.0),
        0.0,
        0.0,
        raster_height as f32 / view_height.max(1.0),
        -min_x * raster_width as f32 / view_width.max(1.0),
        -min_y * raster_height as f32 / view_height.max(1.0),
    );

    let mut painted = false;
    for path in paths {
        let Some(tiny_path) = parse_svg_path_to_tiny_path(&path.data) else {
            continue;
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(path.fill.r(), path.fill.g(), path.fill.b(), path.fill.a());
        let fill_rule = if path.even_odd {
            FillRule::EvenOdd
        } else {
            FillRule::Winding
        };
        pixmap.fill_path(&tiny_path, &paint, fill_rule, transform, None);
        painted = true;
    }
    if !painted {
        return None;
    }

    let color_image = egui::ColorImage::from_rgba_premultiplied(
        [raster_width as usize, raster_height as usize],
        pixmap.data(),
    );
    let image = ImageBlock::from_color_image(
        PathBuf::from(format!(
            "inline-svg-raster-{}x{}.png",
            raster_width, raster_height
        )),
        egui::vec2(width, height),
        color_image,
    );
    Some(CanvasBlock::Image {
        alt: element.attr("aria-label").unwrap_or("svg").to_owned(),
        src: "inline-svg-raster".to_owned(),
        image,
    })
}

#[derive(Clone, Debug)]
struct SvgPathPaint {
    data: String,
    fill: egui::Color32,
    even_odd: bool,
}

fn collect_dom_svg_paths(
    element: &DomElement,
    inherited_fill: egui::Color32,
    paths: &mut Vec<SvgPathPaint>,
) {
    let fill = element
        .attr("fill")
        .and_then(|value| parse_svg_fill_color(value, inherited_fill))
        .unwrap_or(inherited_fill);
    if element.tag_name == "path" {
        if let Some(data) = element.attr("d").filter(|data| !data.trim().is_empty()) {
            paths.push(SvgPathPaint {
                data: data.to_owned(),
                fill,
                even_odd: element
                    .attr("fill-rule")
                    .is_some_and(|rule| rule.eq_ignore_ascii_case("evenodd")),
            });
        }
    }
    for child in &element.children {
        if let DomNode::Element(child) = child {
            collect_dom_svg_paths(child, fill, paths);
        }
    }
}

fn parse_svg_fill_color(value: &str, current_color: egui::Color32) -> Option<egui::Color32> {
    if value.trim().eq_ignore_ascii_case("currentColor") {
        return Some(current_color);
    }
    parse_svg_color(value)
}

fn element_style_current_color(element: &DomElement) -> Option<egui::Color32> {
    element.attr("color").and_then(parse_svg_color).or_else(|| {
        element.attr("style").and_then(|style| {
            style.split(';').find_map(|declaration| {
                let (name, value) = declaration.split_once(':')?;
                name.trim()
                    .eq_ignore_ascii_case("color")
                    .then(|| parse_svg_color(value))
                    .flatten()
            })
        })
    })
}

#[derive(Clone, Copy, Debug)]
enum SvgPathToken {
    Command(char),
    Number(f32),
}

fn parse_svg_path_to_tiny_path(data: &str) -> Option<tiny_skia::Path> {
    let tokens = tokenize_svg_path(data);
    let mut index = 0;
    let mut command = None;
    let mut builder = PathBuilder::new();
    let mut current = (0.0, 0.0);
    let mut subpath_start = (0.0, 0.0);
    let mut last_cubic_control = None;
    let mut last_quad_control = None;

    while index < tokens.len() {
        if let Some(next) = svg_path_command_at(&tokens, index) {
            command = Some(next);
            index += 1;
        }
        let command = command?;
        let relative = command.is_ascii_lowercase();
        match command.to_ascii_uppercase() {
            'M' => {
                let mut first = true;
                while svg_path_number_at(&tokens, index).is_some() {
                    let x = next_svg_path_number(&tokens, &mut index)?;
                    let y = next_svg_path_number(&tokens, &mut index)?;
                    let point = svg_path_point(x, y, relative, current);
                    if first {
                        builder.move_to(point.0, point.1);
                        subpath_start = point;
                        first = false;
                    } else {
                        builder.line_to(point.0, point.1);
                    }
                    current = point;
                    last_cubic_control = None;
                    last_quad_control = None;
                }
            }
            'L' => {
                while svg_path_number_at(&tokens, index).is_some() {
                    let x = next_svg_path_number(&tokens, &mut index)?;
                    let y = next_svg_path_number(&tokens, &mut index)?;
                    current = svg_path_point(x, y, relative, current);
                    builder.line_to(current.0, current.1);
                    last_cubic_control = None;
                    last_quad_control = None;
                }
            }
            'H' => {
                while let Some(x) = svg_path_number_at(&tokens, index) {
                    index += 1;
                    current.0 = if relative { current.0 + x } else { x };
                    builder.line_to(current.0, current.1);
                    last_cubic_control = None;
                    last_quad_control = None;
                }
            }
            'V' => {
                while let Some(y) = svg_path_number_at(&tokens, index) {
                    index += 1;
                    current.1 = if relative { current.1 + y } else { y };
                    builder.line_to(current.0, current.1);
                    last_cubic_control = None;
                    last_quad_control = None;
                }
            }
            'C' => {
                while svg_path_number_at(&tokens, index).is_some() {
                    let c1 = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    let c2 = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    let point = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    builder.cubic_to(c1.0, c1.1, c2.0, c2.1, point.0, point.1);
                    current = point;
                    last_cubic_control = Some(c2);
                    last_quad_control = None;
                }
            }
            'S' => {
                while svg_path_number_at(&tokens, index).is_some() {
                    let c1 = last_cubic_control
                        .map(|control| (current.0 * 2.0 - control.0, current.1 * 2.0 - control.1))
                        .unwrap_or(current);
                    let c2 = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    let point = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    builder.cubic_to(c1.0, c1.1, c2.0, c2.1, point.0, point.1);
                    current = point;
                    last_cubic_control = Some(c2);
                    last_quad_control = None;
                }
            }
            'Q' => {
                while svg_path_number_at(&tokens, index).is_some() {
                    let control = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    let point = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    builder.quad_to(control.0, control.1, point.0, point.1);
                    current = point;
                    last_quad_control = Some(control);
                    last_cubic_control = None;
                }
            }
            'T' => {
                while svg_path_number_at(&tokens, index).is_some() {
                    let control = last_quad_control
                        .map(|control| (current.0 * 2.0 - control.0, current.1 * 2.0 - control.1))
                        .unwrap_or(current);
                    let point = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    builder.quad_to(control.0, control.1, point.0, point.1);
                    current = point;
                    last_quad_control = Some(control);
                    last_cubic_control = None;
                }
            }
            'A' => {
                while svg_path_number_at(&tokens, index).is_some() {
                    for _ in 0..5 {
                        next_svg_path_number(&tokens, &mut index)?;
                    }
                    let point = svg_path_point(
                        next_svg_path_number(&tokens, &mut index)?,
                        next_svg_path_number(&tokens, &mut index)?,
                        relative,
                        current,
                    );
                    current = point;
                    builder.line_to(current.0, current.1);
                    last_cubic_control = None;
                    last_quad_control = None;
                }
            }
            'Z' => {
                builder.close();
                current = subpath_start;
                last_cubic_control = None;
                last_quad_control = None;
            }
            _ => return None,
        }
    }

    builder.finish()
}

fn tokenize_svg_path(data: &str) -> Vec<SvgPathToken> {
    let mut tokens = Vec::new();
    let bytes = data.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let character = data[index..].chars().next().unwrap_or_default();
        if character.is_ascii_whitespace() || character == ',' {
            index += character.len_utf8();
        } else if is_svg_path_command(character) {
            tokens.push(SvgPathToken::Command(character));
            index += character.len_utf8();
        } else {
            let start = index;
            index += character.len_utf8();
            while index < bytes.len() {
                let next = data[index..].chars().next().unwrap_or_default();
                if next == '.' && data[start..index].chars().any(|character| character == '.') {
                    break;
                }
                if next.is_ascii_digit()
                    || next == '.'
                    || next == '-'
                    || next == '+'
                    || next == 'e'
                    || next == 'E'
                {
                    let previous = data[..index].chars().next_back().unwrap_or_default();
                    if (next == '-' || next == '+') && previous != 'e' && previous != 'E' {
                        break;
                    }
                    index += next.len_utf8();
                } else {
                    break;
                }
            }
            if let Ok(number) = data[start..index].parse::<f32>() {
                tokens.push(SvgPathToken::Number(number));
            }
        }
    }
    tokens
}

fn is_svg_path_command(character: char) -> bool {
    matches!(
        character,
        'M' | 'm'
            | 'L'
            | 'l'
            | 'H'
            | 'h'
            | 'V'
            | 'v'
            | 'C'
            | 'c'
            | 'S'
            | 's'
            | 'Q'
            | 'q'
            | 'T'
            | 't'
            | 'A'
            | 'a'
            | 'Z'
            | 'z'
    )
}

fn svg_path_command_at(tokens: &[SvgPathToken], index: usize) -> Option<char> {
    match tokens.get(index) {
        Some(SvgPathToken::Command(command)) => Some(*command),
        _ => None,
    }
}

fn svg_path_number_at(tokens: &[SvgPathToken], index: usize) -> Option<f32> {
    match tokens.get(index) {
        Some(SvgPathToken::Number(number)) => Some(*number),
        _ => None,
    }
}

fn next_svg_path_number(tokens: &[SvgPathToken], index: &mut usize) -> Option<f32> {
    let number = svg_path_number_at(tokens, *index)?;
    *index += 1;
    Some(number)
}

fn svg_path_point(x: f32, y: f32, relative: bool, current: (f32, f32)) -> (f32, f32) {
    if relative {
        (current.0 + x, current.1 + y)
    } else {
        (x, y)
    }
}

fn collect_dom_svg_shapes(element: &DomElement, shapes: &mut Vec<SvgShape>) {
    if element.tag_name == "circle" {
        if let (Some(cx), Some(cy), Some(r)) = (
            element.attr("cx").and_then(parse_html_dimension),
            element.attr("cy").and_then(parse_html_dimension),
            element.attr("r").and_then(parse_html_dimension),
        ) {
            shapes.push(SvgShape::Circle {
                cx,
                cy,
                r,
                fill: element
                    .attr("fill")
                    .and_then(parse_svg_color)
                    .unwrap_or(egui::Color32::BLACK),
                stroke: element.attr("stroke").and_then(parse_svg_color),
                stroke_width: element
                    .attr("stroke-width")
                    .and_then(parse_html_dimension)
                    .unwrap_or(1.0),
            });
        }
    }
    for child in &element.children {
        if let DomNode::Element(child) = child {
            collect_dom_svg_shapes(child, shapes);
        }
    }
}

fn dom_svg_contains_path(element: &DomElement) -> bool {
    element.tag_name == "path"
        || element.children.iter().any(|child| match child {
            DomNode::Element(child) => dom_svg_contains_path(child),
            DomNode::Text(_) => false,
        })
}

fn input_block_from_dom_element(element: &DomElement) -> CanvasBlock {
    let value = element
        .attr("value")
        .map(ToOwned::to_owned)
        .or_else(|| {
            if element.tag_name.eq_ignore_ascii_case("textarea") {
                let text = element.text_content();
                (!text.is_empty()).then_some(text)
            } else {
                None
            }
        })
        .unwrap_or_default();

    CanvasBlock::Input {
        label: element
            .attr("aria-label")
            .or_else(|| element.attr("placeholder"))
            .or_else(|| element.attr("name"))
            .or_else(|| element.attr("id"))
            .map(labelize_input_name)
            .unwrap_or_else(|| "Input".to_owned()),
        value,
    }
}

fn collect_select_options(nodes: &[DomNode], options: &mut Vec<String>) {
    for child in nodes {
        if let DomNode::Element(el) = child {
            if el.tag_name == "option" {
                let text = el.text_content();
                let text = text.trim();
                if !text.is_empty() {
                    options.push(text.to_owned());
                }
            } else if el.tag_name == "optgroup" {
                collect_select_options(&el.children, options);
            }
        }
    }
}

fn find_selected_option(nodes: &[DomNode]) -> Option<String> {
    for child in nodes {
        if let DomNode::Element(el) = child {
            if el.tag_name == "option" && el.attr("selected").is_some() {
                return Some(el.text_content().trim().to_owned());
            } else if el.tag_name == "optgroup" {
                if let Some(found) = find_selected_option(&el.children) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn labelize_input_name(value: &str) -> String {
    let value = value
        .split_once(|ch: char| ch == '-' || ch == '_')
        .map(|(first, _)| first)
        .unwrap_or(value);
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return "Input".to_owned();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

fn dom_table_block(element: &DomElement) -> Option<CanvasBlock> {
    let mut rows = Vec::new();
    collect_dom_table_rows(element, &mut rows);
    (!rows.is_empty()).then_some(CanvasBlock::Table {
        caption: element
            .first_descendant_by_tag("caption")
            .map(DomElement::text_content)
            .unwrap_or_default(),
        rows,
    })
}

fn collect_dom_table_rows(element: &DomElement, rows: &mut Vec<Vec<String>>) {
    if element.tag_name == "tr" {
        let row = element
            .children
            .iter()
            .filter_map(|child| match child {
                DomNode::Element(cell) if cell.tag_name == "td" || cell.tag_name == "th" => {
                    Some(cell.text_content())
                }
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        if !row.is_empty() {
            rows.push(row);
        }
    }
    for child in &element.children {
        if let DomNode::Element(child) = child {
            collect_dom_table_rows(child, rows);
        }
    }
}

fn element_href(element: &DomElement) -> Option<String> {
    element
        .first_descendant_by_tag_with_attr("a", "href")
        .and_then(|link| link.attr("href"))
        .map(str::to_owned)
}

fn element_links(element: &DomElement) -> Vec<(String, String)> {
    let mut links = Vec::new();
    collect_element_links(element, &mut links);
    links
}

fn collect_element_links(element: &DomElement, links: &mut Vec<(String, String)>) {
    if element.tag_name.eq_ignore_ascii_case("a") {
        let text = element.text_content();
        if !text.is_empty() {
            links.push((text, element.attr("href").unwrap_or_default().to_owned()));
        }
    }
    for child in &element.children {
        if let DomNode::Element(child) = child {
            collect_element_links(child, links);
        }
    }
}

#[derive(Default)]
struct ParseState {
    source: String,
    image_height_auto: bool,
    last_label: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct InlineStyleState {
    href: Option<String>,
    strong: bool,
    emphasis: bool,
    underline: bool,
    strikethrough: bool,
    code: bool,
    small: bool,
    raised: bool,
    lowered: bool,
    highlight: bool,
}

fn inline_span_has_style(span: &InlineSpan) -> bool {
    span.href.is_some()
        || span.strong
        || span.emphasis
        || span.underline
        || span.strikethrough
        || span.code
        || span.small
        || span.raised
        || span.lowered
        || span.highlight
}

fn parse_inline_spans(html: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut style = InlineStyleState::default();
    parse_inline_into(html, &mut style, &mut spans);
    merge_inline_spans(spans)
}

fn parse_inline_into(html: &str, style: &mut InlineStyleState, spans: &mut Vec<InlineSpan>) {
    let mut remaining = html;
    while let Some(open_index) = remaining.find('<') {
        push_inline_text(&remaining[..open_index], style, spans);
        let after_open = &remaining[open_index..];
        let Some(open_end) = after_open.find('>') else {
            push_inline_text(after_open, style, spans);
            return;
        };
        let open_tag = &after_open[..open_end + 1];
        let Some(tag) = tag_name(open_tag) else {
            remaining = &after_open[open_end + 1..];
            continue;
        };
        let after_content = &after_open[open_end + 1..];

        if is_void_tag(tag) {
            remaining = after_content;
            continue;
        }

        let Some(close_index) = find_matching_close(after_content, tag) else {
            remaining = after_content;
            continue;
        };
        let content = &after_content[..close_index];
        let mut nested = style.clone();
        apply_inline_tag(&mut nested, tag, open_tag);
        parse_inline_into(content, &mut nested, spans);
        remaining = &after_content[close_index + tag.len() + 3..];
    }
    push_inline_text(remaining, style, spans);
}

fn push_inline_text(text: &str, style: &InlineStyleState, spans: &mut Vec<InlineSpan>) {
    let text = normalize_inline_ws(&decode_basic_entities(text));
    if text.is_empty() {
        return;
    }
    spans.push(InlineSpan {
        text,
        href: style.href.clone(),
        strong: style.strong,
        emphasis: style.emphasis,
        underline: style.underline,
        strikethrough: style.strikethrough,
        code: style.code,
        small: style.small,
        raised: style.raised,
        lowered: style.lowered,
        highlight: style.highlight,
    });
}

fn normalize_inline_ws(text: &str) -> String {
    let has_leading_ws = text.chars().next().is_some_and(char::is_whitespace);
    let has_trailing_ws = text.chars().last().is_some_and(char::is_whitespace);
    let core = normalize_ws(text);
    if core.is_empty() {
        return if has_leading_ws || has_trailing_ws {
            " ".to_owned()
        } else {
            String::new()
        };
    }

    let mut out = String::new();
    if has_leading_ws {
        out.push(' ');
    }
    out.push_str(&core);
    if has_trailing_ws {
        out.push(' ');
    }
    out
}

fn apply_inline_tag(style: &mut InlineStyleState, tag: &str, open_tag: &str) {
    match tag {
        "a" => style.href = extract_attr(open_tag, "href"),
        "strong" | "b" => style.strong = true,
        "em" | "i" | "cite" | "dfn" | "var" => style.emphasis = true,
        "u" | "ins" => style.underline = true,
        "del" | "s" => style.strikethrough = true,
        "code" | "kbd" | "samp" => style.code = true,
        "small" => style.small = true,
        "sup" => style.raised = true,
        "sub" => style.lowered = true,
        "mark" => style.highlight = true,
        "abbr" | "q" | "time" => {}
        _ => {}
    }
}

fn merge_inline_spans(spans: Vec<InlineSpan>) -> Vec<InlineSpan> {
    let mut merged: Vec<InlineSpan> = Vec::new();
    for span in spans {
        if let Some(last) = merged.last_mut() {
            if same_inline_style(last, &span) {
                last.text.push_str(&span.text);
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

fn same_inline_style(a: &InlineSpan, b: &InlineSpan) -> bool {
    a.href == b.href
        && a.strong == b.strong
        && a.emphasis == b.emphasis
        && a.underline == b.underline
        && a.strikethrough == b.strikethrough
        && a.code == b.code
        && a.small == b.small
        && a.raised == b.raised
        && a.lowered == b.lowered
        && a.highlight == b.highlight
}

fn tag_name(open_tag: &str) -> Option<&str> {
    let tag = open_tag.trim_start_matches('<').trim_start_matches('/');
    let end = tag
        .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
        .unwrap_or(tag.len());
    let tag = &tag[..end];
    if tag.is_empty() { None } else { Some(tag) }
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn parse_container_blocks(
    html: &str,
    state: &mut ParseState,
    list_depth: usize,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    let mut remaining = html;

    while let Some((tag, index)) = next_render_tag(remaining) {
        remaining = &remaining[index..];
        let Some(open_end) = remaining.find('>') else {
            break;
        };
        let open_tag = &remaining[..open_end + 1];

        if tag == "hr" {
            blocks.push(CanvasBlock::Rule);
            remaining = &remaining[open_end + 1..];
            continue;
        }

        if tag == "img" {
            blocks.push(image_block_from_tag(open_tag, state));
            remaining = &remaining[open_end + 1..];
            continue;
        }

        if tag == "input" {
            push_input_block(open_tag, state, &mut blocks);
            remaining = &remaining[open_end + 1..];
            continue;
        }

        let close = format!("</{tag}>");
        let content_start = open_end + 1;
        let after_content_start = &remaining[content_start..];
        let Some(close_index) = find_matching_close(after_content_start, tag) else {
            remaining = &remaining[open_end + 1..];
            continue;
        };
        let content = &after_content_start[..close_index];

        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let text = strip_tags(content);
                if !text.is_empty() {
                    blocks.push(CanvasBlock::Heading {
                        level: tag[1..].parse::<u8>().unwrap_or(1),
                        text,
                    });
                }
            }
            "p" => {
                let paragraph = strip_tags(content);
                let links = extract_links(content);
                let link_text = normalize_ws(
                    &links
                        .iter()
                        .map(|(text, _)| text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                if !paragraph.is_empty() && (links.is_empty() || paragraph != link_text) {
                    let spans = parse_inline_spans(content);
                    if spans.len() > 1 || spans.iter().any(inline_span_has_style) {
                        blocks.push(CanvasBlock::InlineText { spans });
                    } else {
                        blocks.push(CanvasBlock::Paragraph {
                            text: paragraph.clone(),
                        });
                    }
                }
                if paragraph == link_text {
                    for (text, href) in links {
                        blocks.push(CanvasBlock::Link { text, href });
                    }
                }
                blocks.extend(parse_media_only_blocks(content, state, list_depth));
            }
            "a" => {
                let text = strip_tags(content);
                if !text.is_empty() {
                    blocks.push(CanvasBlock::Link {
                        text,
                        href: extract_attr(open_tag, "href").unwrap_or_default(),
                    });
                }
            }
            "ul" | "ol" => {
                blocks.extend(parse_list_blocks(content, state, list_depth, tag == "ol"));
            }
            "li" => {
                let text = strip_tags_without_nested_lists(content);
                if !text.is_empty() {
                    blocks.push(CanvasBlock::ListItem {
                        depth: list_depth,
                        ordered: false,
                        href: list_item_href(content),
                        text,
                    });
                }
                blocks.extend(parse_container_blocks(content, state, list_depth + 1));
            }
            "blockquote" => {
                let text = strip_tags(content);
                if !text.is_empty() {
                    blocks.push(CanvasBlock::Quote { text });
                }
            }
            "pre" => {
                let text = decode_basic_entities(content).trim().to_owned();
                if !text.is_empty() {
                    blocks.push(CanvasBlock::Preformatted { text });
                }
            }
            "table" => {
                if let Some(table) = parse_table(content) {
                    blocks.push(table);
                }
            }
            "label" => {
                let text = strip_tags(content);
                if !text.is_empty() {
                    state.last_label = Some(text.clone());
                    if !content.contains("<input") {
                        blocks.push(CanvasBlock::Paragraph { text });
                    }
                }
                blocks.extend(parse_container_blocks(content, state, list_depth));
            }
            "button" => {
                let text = strip_tags(content);
                if !text.is_empty() {
                    blocks.push(CanvasBlock::Button { text });
                }
            }
            "select" => {
                let value = extract_tag_text(content, "option").unwrap_or_default();
                blocks.push(CanvasBlock::Input {
                    label: state
                        .last_label
                        .clone()
                        .unwrap_or_else(|| "Select".to_owned()),
                    value,
                });
            }
            "span" | "abbr" | "q" | "time" | "mark" | "del" | "s" | "ins" | "var" | "sup"
            | "sub" => {
                let spans = parse_inline_spans(content);
                if !spans.is_empty() {
                    if spans.len() > 1 || spans.iter().any(inline_span_has_style) {
                        blocks.push(CanvasBlock::InlineText { spans });
                    } else {
                        blocks.push(CanvasBlock::Paragraph {
                            text: strip_tags(content),
                        });
                    }
                }
                blocks.extend(parse_media_only_blocks(content, state, list_depth));
            }
            "textarea" => {
                let value = extract_attr(open_tag, "placeholder")
                    .or_else(|| Some(strip_tags(content)))
                    .unwrap_or_default();
                blocks.push(CanvasBlock::Input {
                    label: state
                        .last_label
                        .clone()
                        .unwrap_or_else(|| "Textarea".to_owned()),
                    value,
                });
            }
            "svg" => {
                blocks.push(svg_block_from_tag(open_tag, content));
            }
            "audio" | "video" | "canvas" | "meter" | "progress" | "iframe" => {
                blocks.push(CanvasBlock::Media {
                    label: media_label(open_tag, tag),
                });
            }
            "section" if open_tag.contains("panel") => {
                let children = parse_container_blocks(content, state, list_depth);
                if !children.is_empty() {
                    blocks.push(CanvasBlock::Panel { children });
                }
            }
            "header" | "nav" | "main" | "section" | "article" | "div" | "footer" | "form"
            | "fieldset" | "figure" | "figcaption" | "dl" | "dt" | "dd" | "strong" | "em" | "b"
            | "i" | "u" | "small" | "cite" | "code" | "kbd" | "samp" | "legend" => {
                blocks.extend(parse_container_blocks(content, state, list_depth));
            }
            _ => {}
        }

        remaining = &after_content_start[close_index + close.len()..];
    }

    if blocks.is_empty() {
        let text = strip_tags(html);
        if !text.is_empty() {
            blocks.push(CanvasBlock::Paragraph { text });
        }
    }

    blocks
}

fn parse_list_blocks(
    html: &str,
    state: &mut ParseState,
    list_depth: usize,
    ordered: bool,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    let mut remaining = html;

    while let Some(index) = remaining.find("<li") {
        remaining = &remaining[index..];
        let Some(open_end) = remaining.find('>') else {
            break;
        };
        let content_start = open_end + 1;
        let after_content_start = &remaining[content_start..];
        let Some(close_index) = find_matching_close(after_content_start, "li") else {
            break;
        };
        let content = &after_content_start[..close_index];
        let text = strip_tags_without_nested_lists(content);
        if !text.is_empty() {
            blocks.push(CanvasBlock::ListItem {
                depth: list_depth,
                ordered,
                href: list_item_href(content),
                text,
            });
        }
        blocks.extend(parse_nested_lists(content, state, list_depth + 1));

        remaining = &after_content_start[close_index + "</li>".len()..];
    }

    blocks
}

fn parse_nested_lists(html: &str, state: &mut ParseState, list_depth: usize) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    let mut remaining = html;
    while let Some((tag, index)) = ["ul", "ol"]
        .into_iter()
        .filter_map(|tag| remaining.find(&format!("<{tag}")).map(|index| (tag, index)))
        .min_by_key(|(_, index)| *index)
    {
        remaining = &remaining[index..];
        let Some(open_end) = remaining.find('>') else {
            break;
        };
        let close = format!("</{tag}>");
        let after_open = &remaining[open_end + 1..];
        let Some(close_index) = find_matching_close(after_open, tag) else {
            break;
        };
        blocks.extend(parse_list_blocks(
            &after_open[..close_index],
            state,
            list_depth,
            tag == "ol",
        ));
        remaining = &after_open[close_index + close.len()..];
    }
    blocks
}

fn list_item_href(content: &str) -> Option<String> {
    let first_link = extract_links(strip_nested_lists_raw(content))
        .into_iter()
        .next()?;
    let item_text = strip_tags_without_nested_lists(content);
    if first_link.0 == item_text {
        Some(first_link.1)
    } else {
        None
    }
}

fn strip_nested_lists_raw(html: &str) -> &str {
    let first_nested = ["<ul", "<ol"]
        .into_iter()
        .filter_map(|pattern| html.find(pattern))
        .min()
        .unwrap_or(html.len());
    &html[..first_nested]
}

fn find_matching_close(html_after_open: &str, tag: &str) -> Option<usize> {
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    let mut depth = 1usize;
    let mut offset = 0usize;

    loop {
        let rest = &html_after_open[offset..];
        let next_open = rest.find(&open_pattern);
        let next_close = rest.find(&close_pattern)?;

        if let Some(open_index) = next_open {
            if open_index < next_close {
                depth += 1;
                offset += open_index + open_pattern.len();
                continue;
            }
        }

        depth -= 1;
        if depth == 0 {
            return Some(offset + next_close);
        }
        offset += next_close + close_pattern.len();
    }
}

fn parse_media_only_blocks(
    html: &str,
    state: &mut ParseState,
    list_depth: usize,
) -> Vec<CanvasBlock> {
    let mut blocks = Vec::new();
    for tag in ["img", "input"] {
        if html.contains(&format!("<{tag}")) {
            blocks.extend(parse_container_blocks(html, state, list_depth));
            break;
        }
    }
    blocks
}

fn next_render_tag(html: &str) -> Option<(&'static str, usize)> {
    [
        "blockquote",
        "fieldset",
        "figcaption",
        "figure",
        "progress",
        "section",
        "article",
        "header",
        "footer",
        "button",
        "select",
        "textarea",
        "strong",
        "legend",
        "audio",
        "video",
        "canvas",
        "meter",
        "iframe",
        "main",
        "form",
        "nav",
        "div",
        "pre",
        "table",
        "label",
        "input",
        "img",
        "code",
        "kbd",
        "samp",
        "cite",
        "small",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "ul",
        "ol",
        "li",
        "dl",
        "dt",
        "dd",
        "hr",
        "svg",
        "p",
        "a",
        "span",
        "abbr",
        "time",
        "mark",
        "del",
        "s",
        "ins",
        "var",
        "sup",
        "sub",
        "q",
        "em",
        "b",
        "i",
        "u",
    ]
    .into_iter()
    .filter_map(|tag| html.find(&format!("<{tag}")).map(|index| (tag, index)))
    .min_by_key(|(_, index)| *index)
}

fn push_input_block(open_tag: &str, state: &mut ParseState, blocks: &mut Vec<CanvasBlock>) {
    let input_type = extract_attr(open_tag, "type").unwrap_or_else(|| "text".to_owned());
    let value = extract_attr(open_tag, "value")
        .or_else(|| extract_attr(open_tag, "placeholder"))
        .unwrap_or_default();

    match input_type.as_str() {
        "hidden" => {}
        "submit" | "button" | "reset" => blocks.push(CanvasBlock::Button {
            text: if value.is_empty() { input_type } else { value },
        }),
        "checkbox" | "radio" => blocks.push(CanvasBlock::ListItem {
            depth: 1,
            ordered: false,
            href: None,
            text: state
                .last_label
                .clone()
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| input_type.clone()),
        }),
        "range" => blocks.push(CanvasBlock::Media {
            label: format!("{} range", current_label(state, "Range input")),
        }),
        "color" => blocks.push(CanvasBlock::Media {
            label: format!("{} {}", current_label(state, "Color input"), value),
        }),
        _ => blocks.push(CanvasBlock::Input {
            label: current_label(state, "Input"),
            value,
        }),
    }
}

fn current_label(state: &ParseState, fallback: &str) -> String {
    state
        .last_label
        .clone()
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn media_label(open_tag: &str, fallback: &str) -> String {
    extract_attr(open_tag, "alt")
        .or_else(|| extract_attr(open_tag, "src"))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn image_block_from_tag(open_tag: &str, state: &ParseState) -> CanvasBlock {
    let label = media_label(open_tag, "image");
    let Some(src) = extract_attr(open_tag, "src").filter(|src| !src.is_empty()) else {
        return CanvasBlock::Media { label };
    };
    let resolved = resolve_resource_url(&state.source, &src);
    if !resource_allowed_for_document(&state.source, &resolved) {
        return CanvasBlock::Media { label };
    }
    let requested_size = requested_image_size(open_tag);

    match load_image_resource(&resolved, requested_size, state.image_height_auto) {
        Ok(image) => CanvasBlock::Image {
            alt: extract_attr(open_tag, "alt").unwrap_or_default(),
            src: resolved,
            image,
        },
        Err(_) => CanvasBlock::Media { label },
    }
}

fn svg_block_from_tag(open_tag: &str, content: &str) -> CanvasBlock {
    let width = extract_attr(open_tag, "width")
        .and_then(|value| parse_html_dimension(&value))
        .unwrap_or(100.0);
    let height = extract_attr(open_tag, "height")
        .and_then(|value| parse_html_dimension(&value))
        .unwrap_or(100.0);
    let mut shapes = parse_svg_shapes(content);
    if shapes.is_empty() && content.contains("<path") {
        shapes.push(SvgShape::Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
            fill: egui::Color32::WHITE,
        });
    }

    if shapes.is_empty() {
        CanvasBlock::Media {
            label: media_label(open_tag, "svg"),
        }
    } else {
        CanvasBlock::Svg {
            svg: SvgBlock::new(egui::vec2(width, height), shapes),
        }
    }
}

fn parse_svg_shapes(content: &str) -> Vec<SvgShape> {
    let mut shapes = Vec::new();
    let mut remaining = content;

    while let Some(index) = remaining.find("<circle") {
        remaining = &remaining[index..];
        let Some(end) = remaining.find('>') else {
            break;
        };
        let tag = &remaining[..end + 1];
        if let Some(shape) = parse_svg_circle(tag) {
            shapes.push(shape);
        }
        remaining = &remaining[end + 1..];
    }

    shapes
}

fn parse_svg_circle(tag: &str) -> Option<SvgShape> {
    let cx = extract_attr(tag, "cx").and_then(|value| parse_html_dimension(&value))?;
    let cy = extract_attr(tag, "cy").and_then(|value| parse_html_dimension(&value))?;
    let r = extract_attr(tag, "r").and_then(|value| parse_html_dimension(&value))?;
    let fill = extract_attr(tag, "fill")
        .and_then(|value| parse_svg_color(&value))
        .unwrap_or(egui::Color32::BLACK);
    let stroke = extract_attr(tag, "stroke").and_then(|value| parse_svg_color(&value));
    let stroke_width = extract_attr(tag, "stroke-width")
        .and_then(|value| parse_html_dimension(&value))
        .unwrap_or(1.0);

    Some(SvgShape::Circle {
        cx,
        cy,
        r,
        fill,
        stroke,
        stroke_width,
    })
}

fn parse_svg_color(value: &str) -> Option<egui::Color32> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let mut chars = hex.chars();
            let r = chars.next()?.to_digit(16)? as u8 * 17;
            let g = chars.next()?.to_digit(16)? as u8 * 17;
            let b = chars.next()?.to_digit(16)? as u8 * 17;
            Some(egui::Color32::from_rgb(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(egui::Color32::from_rgb(r, g, b))
        }
        _ => None,
    }
}

fn requested_image_size(open_tag: &str) -> Option<egui::Vec2> {
    let width = extract_attr(open_tag, "width").and_then(|value| parse_html_dimension(&value));
    let height = extract_attr(open_tag, "height").and_then(|value| parse_html_dimension(&value));
    match (width, height) {
        (Some(width), Some(height)) => Some(egui::vec2(width, height)),
        _ => None,
    }
}

fn parse_html_dimension(value: &str) -> Option<f32> {
    let value = value.trim().trim_end_matches("px");
    value.parse::<f32>().ok().filter(|value| *value > 0.0)
}

fn load_image_resource(
    url: &str,
    requested_size: Option<egui::Vec2>,
    preserve_aspect: bool,
) -> io::Result<ImageBlock> {
    let bytes = if url.starts_with("http://") || url.starts_with("https://") {
        http_client()?
            .get(url)
            .send()
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .bytes()
            .map_err(io::Error::other)?
            .to_vec()
    } else {
        fs::read(input_to_path(url))?
    };

    ImageBlock::from_encoded_bytes_with_aspect(
        PathBuf::from(url),
        &bytes,
        requested_size,
        preserve_aspect,
    )
    .map_err(io::Error::other)
}

fn parse_table(html: &str) -> Option<CanvasBlock> {
    let caption = extract_tag_text(html, "caption").unwrap_or_default();
    let mut rows = Vec::new();
    let mut remaining = html;
    while let Some(index) = remaining.find("<tr") {
        remaining = &remaining[index..];
        let open_end = remaining.find('>')?;
        let after_open = &remaining[open_end + 1..];
        let close_index = after_open.find("</tr>")?;
        let row_html = &after_open[..close_index];
        let row = extract_table_cells(row_html);
        if !row.is_empty() {
            rows.push(row);
        }
        remaining = &after_open[close_index + "</tr>".len()..];
    }

    if rows.is_empty() {
        None
    } else {
        Some(CanvasBlock::Table { caption, rows })
    }
}

fn extract_table_cells(row_html: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut remaining = row_html;
    while let Some((tag, index)) = ["th", "td"]
        .into_iter()
        .filter_map(|tag| remaining.find(&format!("<{tag}")).map(|index| (tag, index)))
        .min_by_key(|(_, index)| *index)
    {
        remaining = &remaining[index..];
        let Some(open_end) = remaining.find('>') else {
            break;
        };
        let close = format!("</{tag}>");
        let after_open = &remaining[open_end + 1..];
        let Some(close_index) = after_open.find(&close) else {
            break;
        };
        let cell = strip_tags(&after_open[..close_index]);
        if !cell.is_empty() {
            cells.push(cell);
        }
        remaining = &after_open[close_index + close.len()..];
    }
    cells
}

fn strip_tags_without_nested_lists(html: &str) -> String {
    let mut text_html = html.to_owned();
    for tag in ["ul", "ol"] {
        while let Some(start) = text_html.find(&format!("<{tag}")) {
            let Some(after_open) = text_html[start..].find('>') else {
                break;
            };
            let content_start = start + after_open + 1;
            let Some(close_rel) = text_html[content_start..].find(&format!("</{tag}>")) else {
                break;
            };
            let end = content_start + close_rel + tag.len() + 3;
            text_html.replace_range(start..end, "");
        }
    }
    strip_tags(&text_html)
}

fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    extract_tag_inner(html, tag).map(strip_tags)
}

fn extract_tag_inner<'a>(html: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let open_index = html.find(&open)?;
    let after_open = &html[open_index..];
    let open_end = after_open.find('>')?;
    let content_start = open_index + open_end + 1;
    let after_content_start = &html[content_start..];
    let close_index = after_content_start.find(&close)?;
    Some(&after_content_start[..close_index])
}

fn extract_links(html: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut remaining = html;

    while let Some(open_index) = remaining.find("<a") {
        let after_open = &remaining[open_index..];
        let Some(open_end) = after_open.find('>') else {
            break;
        };
        let tag = &after_open[..open_end + 1];
        let href = extract_attr(tag, "href").unwrap_or_default();
        let content_start = open_index + open_end + 1;
        let after_content_start = &remaining[content_start..];
        let Some(close_index) = after_content_start.find("</a>") else {
            break;
        };
        let text = normalize_ws(&strip_tags(&after_content_start[..close_index]));
        if !text.is_empty() {
            out.push((text, href));
        }
        remaining = &after_content_start[close_index + "</a>".len()..];
    }

    out
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let mut remaining = tag.trim_start_matches('<');
    if let Some(end) = remaining.find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/') {
        remaining = &remaining[end..];
    }

    loop {
        remaining = remaining.trim_start();
        if remaining.is_empty() || remaining.starts_with('>') || remaining.starts_with("/>") {
            return None;
        }

        let name_end = remaining
            .find(|ch: char| ch.is_whitespace() || ch == '=' || ch == '>' || ch == '/')
            .unwrap_or(remaining.len());
        let attr_name = &remaining[..name_end];
        remaining = &remaining[name_end..];
        if attr_name.is_empty() {
            return None;
        }

        remaining = remaining.trim_start();
        let value = if let Some(after_equals) = remaining.strip_prefix('=') {
            let after_equals = after_equals.trim_start();
            if let Some(quote) = after_equals
                .chars()
                .next()
                .filter(|ch| *ch == '"' || *ch == '\'')
            {
                let after_quote = &after_equals[quote.len_utf8()..];
                let end = after_quote.find(quote)?;
                remaining = &after_quote[end + quote.len_utf8()..];
                after_quote[..end].to_owned()
            } else {
                let end = after_equals
                    .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
                    .unwrap_or(after_equals.len());
                remaining = &after_equals[end..];
                after_equals[..end].to_owned()
            }
        } else {
            String::new()
        };

        if attr_name.eq_ignore_ascii_case(name) {
            return Some(value);
        }
    }
}

fn has_attr(tag: &str, name: &str) -> bool {
    extract_attr(tag, name).is_some()
}

fn find_tag_with_class<'a>(html: &'a str, tag: &str, class_name: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let mut remaining = html;
    while let Some(index) = remaining.find(&open) {
        remaining = &remaining[index..];
        let Some(end) = remaining.find('>') else {
            return None;
        };
        let candidate = &remaining[..end + 1];
        if extract_attr(candidate, "class")
            .is_some_and(|classes| classes.split_whitespace().any(|class| class == class_name))
        {
            return Some(candidate);
        }
        remaining = &remaining[end + 1..];
    }
    None
}

fn extract_search_placeholder(html: &str) -> Option<String> {
    let marker = html.find("omnibox-search-form-input")?;
    let prefix = &html[..marker];
    let tag_start = prefix.rfind("<textarea")?;
    let tag_end = html[marker..].find('>')? + marker;
    extract_attr(&html[tag_start..=tag_end], "placeholder")
}

fn extract_counter(html: &str, test_id: &str) -> Option<(String, String)> {
    let start = html.find(&format!("data-test-id=\"{test_id}\""))?;
    let segment = &html[start..html.len().min(start + 6_000)];
    let count = extract_text_near_test_id(segment, "counter-count")?;
    let description = extract_text_near_test_id(segment, "counter-description")?;
    Some((count, description))
}

fn extract_text_near_test_id(html: &str, test_id: &str) -> Option<String> {
    let start = html.find(&format!("data-test-id=\"{test_id}\""))?;
    let after_marker = &html[start..];
    let tag_end = after_marker.find('>')? + 1;
    let content = &after_marker[tag_end..];
    let end = content
        .find("</")
        .or_else(|| content.find('<'))
        .unwrap_or(content.len());
    let text = strip_tags(&content[..end]);
    if text.is_empty() { None } else { Some(text) }
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

fn remove_html_comments(html: &str) -> String {
    let mut out = String::new();
    let mut remaining = html;
    while let Some(start) = remaining.find("<!--") {
        out.push_str(&remaining[..start]);
        let after_start = &remaining[start + 4..];
        let Some(end) = after_start.find("-->") else {
            return out;
        };
        remaining = &after_start[end + 3..];
    }
    out.push_str(remaining);
    out
}

fn remove_non_rendered_elements(html: &str) -> String {
    let mut cleaned = html.to_owned();
    for tag in ["script", "style", "template", "noscript"] {
        cleaned = remove_elements_by_tag(&cleaned, tag);
    }
    remove_hidden_elements(&cleaned)
}

fn remove_non_visual_metadata_elements(html: &str) -> String {
    let mut cleaned = html.to_owned();
    for tag in ["script", "style", "template", "noscript"] {
        cleaned = remove_elements_by_tag(&cleaned, tag);
    }
    cleaned
}

fn remove_elements_by_tag(html: &str, tag: &str) -> String {
    let mut out = String::new();
    let mut remaining = html;
    let open = format!("<{tag}");

    while let Some(index) = remaining.find(&open) {
        if !is_tag_boundary(remaining, index + open.len()) {
            out.push_str(&remaining[..index + open.len()]);
            remaining = &remaining[index + open.len()..];
            continue;
        }

        out.push_str(&remaining[..index]);
        let candidate = &remaining[index..];
        let Some(open_end) = candidate.find('>') else {
            return out;
        };
        let after_open = &candidate[open_end + 1..];
        let Some(close_index) = find_matching_close(after_open, tag) else {
            remaining = after_open;
            continue;
        };
        let close = format!("</{tag}>");
        remaining = &after_open[close_index + close.len()..];
    }

    out.push_str(remaining);
    out
}

fn remove_hidden_elements(html: &str) -> String {
    let mut out = String::new();
    let mut remaining = html;

    while let Some(index) = remaining.find('<') {
        out.push_str(&remaining[..index]);
        let candidate = &remaining[index..];
        let Some(open_end) = candidate.find('>') else {
            out.push_str(candidate);
            return out;
        };
        let open_tag = &candidate[..open_end + 1];
        let Some(tag) = tag_name(open_tag).map(str::to_owned) else {
            out.push_str(open_tag);
            remaining = &candidate[open_end + 1..];
            continue;
        };

        if is_void_tag(&tag) {
            if !tag_is_hidden(open_tag) {
                out.push_str(open_tag);
            }
            remaining = &candidate[open_end + 1..];
            continue;
        }

        if tag_is_hidden(open_tag) {
            let after_open = &candidate[open_end + 1..];
            let Some(close_index) = find_matching_close(after_open, &tag) else {
                remaining = after_open;
                continue;
            };
            let close = format!("</{tag}>");
            remaining = &after_open[close_index + close.len()..];
        } else {
            out.push_str(open_tag);
            remaining = &candidate[open_end + 1..];
        }
    }

    out.push_str(remaining);
    out
}

fn tag_is_hidden(open_tag: &str) -> bool {
    if has_attr(open_tag, "hidden") {
        return true;
    }
    if extract_attr(open_tag, "aria-hidden").is_some_and(|value| value.eq_ignore_ascii_case("true"))
    {
        return true;
    }
    extract_attr(open_tag, "style").is_some_and(|style| {
        let style = style.to_ascii_lowercase().replace(' ', "");
        style.contains("display:none") || style.contains("visibility:hidden")
    })
}

fn is_tag_boundary(text: &str, index: usize) -> bool {
    text[index..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '>' || ch == '/')
}

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn path_to_file_url(path: &Path) -> String {
    let absolute = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace(' ', "%20");
    format!("file://{absolute}")
}

fn percent_decode_file_path(path: &str) -> String {
    let mut out = String::new();
    let mut chars = path.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let high = chars.next();
            let low = chars.next();
            if let (Some(high), Some(low)) = (high, low) {
                let hex = [high, low].iter().collect::<String>();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    out.push(byte as char);
                    continue;
                }
                out.push('%');
                out.push(high);
                out.push(low);
            } else {
                out.push('%');
                if let Some(high) = high {
                    out.push(high);
                }
                if let Some(low) = low {
                    out.push(low);
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn telemetry_event_kind_and_detail(event: &egui::Event) -> (&'static str, String) {
    match event {
        egui::Event::Copy => ("copy", String::new()),
        egui::Event::Cut => ("cut", String::new()),
        egui::Event::Paste(text) => ("paste", format!("chars={}", text.chars().count())),
        egui::Event::Text(text) => ("text", format!("chars={}", text.chars().count())),
        egui::Event::Key {
            key,
            physical_key: _,
            pressed,
            repeat,
            modifiers,
        } => (
            "key",
            format!("key={key:?} pressed={pressed} repeat={repeat} modifiers={modifiers:?}"),
        ),
        egui::Event::PointerMoved(pos) => {
            ("pointer_moved", format!("x={:.1} y={:.1}", pos.x, pos.y))
        }
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        } => (
            "pointer_button",
            format!(
                "x={:.1} y={:.1} button={button:?} pressed={pressed} modifiers={modifiers:?}",
                pos.x, pos.y
            ),
        ),
        egui::Event::PointerGone => ("pointer_gone", String::new()),
        egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
        } => (
            "mouse_wheel",
            format!(
                "unit={unit:?} dx={:.1} dy={:.1} modifiers={modifiers:?}",
                delta.x, delta.y
            ),
        ),
        egui::Event::Zoom(value) => ("zoom", format!("value={value:.3}")),
        egui::Event::Touch { phase, pos, .. } => (
            "touch",
            format!("phase={phase:?} x={:.1} y={:.1}", pos.x, pos.y),
        ),
        egui::Event::WindowFocused(focused) => ("window_focused", format!("focused={focused}")),
        _ => ("other", String::new()),
    }
}

#[derive(Clone, Debug)]
struct TelemetrySink {
    session_id: String,
    session_path: PathBuf,
}

static GLOBAL_TELEMETRY: OnceLock<Mutex<Option<TelemetrySink>>> = OnceLock::new();
static PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();
static SCRIPT_RESOURCE_CACHE: OnceLock<Mutex<HashMap<String, Result<String, String>>>> =
    OnceLock::new();

fn install_global_telemetry(session: &TelemetrySession) {
    let Some(sink) = session.sink() else {
        return;
    };
    let slot = GLOBAL_TELEMETRY.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(sink);
    }
}

fn emit_global_telemetry(event: &str, fields: &[(&str, &str)]) {
    let Some(slot) = GLOBAL_TELEMETRY.get() else {
        return;
    };
    let Ok(guard) = slot.lock() else {
        return;
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };
    write_telemetry_line(sink, event, fields);
}

fn install_telemetry_panic_hook() {
    PANIC_HOOK_INSTALLED.get_or_init(|| {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|value| (*value).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic payload unavailable".to_owned());
            let location = info
                .location()
                .map(|location| {
                    format!(
                        "{}:{}:{}",
                        location.file(),
                        location.line(),
                        location.column()
                    )
                })
                .unwrap_or_else(|| "unknown".to_owned());
            emit_global_telemetry(
                "app.panic",
                &[("message", &message), ("location", &location)],
            );
            previous_hook(info);
        }));
    });
}

fn write_telemetry_line(sink: &TelemetrySink, event: &str, fields: &[(&str, &str)]) {
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(sink.session_path.join("session.jsonl"))
    else {
        return;
    };

    let mut line = format!(
        "{{\"schema_version\":1,\"session_id\":\"{}\",\"timestamp_ms\":{},\"event\":\"{}\"",
        json_escape(&sink.session_id),
        unix_ms(),
        json_escape(event)
    );
    for (key, value) in fields {
        line.push_str(&format!(
            ",\"{}\":\"{}\"",
            json_escape(key),
            json_escape(value)
        ));
    }
    line.push_str("}\n");
    let _ = file.write_all(line.as_bytes());
}

struct TelemetrySession {
    session_id: String,
    session_path: Option<PathBuf>,
}

impl TelemetrySession {
    fn start() -> io::Result<Self> {
        let now = unix_ms();
        let session_id = format!("{now}_{}", std::process::id());
        let session_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../appdata/telemetry/sessions")
            .join(&session_id);
        fs::create_dir_all(&session_path)?;
        File::create(session_path.join("session.jsonl"))?;
        fs::write(
            session_path.join("summary.json"),
            format!(
                "{{\"schema_version\":1,\"session_id\":\"{}\",\"status\":\"started\"}}\n",
                json_escape(&session_id)
            ),
        )?;
        Ok(Self {
            session_id,
            session_path: Some(session_path),
        })
    }

    fn disabled() -> Self {
        Self {
            session_id: "disabled".to_owned(),
            session_path: None,
        }
    }

    fn emit(&self, event: &str, fields: &[(&str, &str)]) {
        if let Some(sink) = self.sink() {
            write_telemetry_line(&sink, event, fields);
        }
    }

    fn display_path(&self) -> String {
        self.session_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "disabled".to_owned())
    }

    fn sink(&self) -> Option<TelemetrySink> {
        self.session_path.as_ref().map(|path| TelemetrySink {
            session_id: self.session_id.clone(),
            session_path: path.clone(),
        })
    }
}

impl Drop for TelemetrySession {
    fn drop(&mut self) {
        self.emit("session.ended", &[]);
        if let Some(path) = &self.session_path {
            let _ = fs::write(
                path.join("summary.json"),
                format!(
                    "{{\"schema_version\":1,\"session_id\":\"{}\",\"status\":\"ended\"}}\n",
                    json_escape(&self.session_id)
                ),
            );
        }
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dom_parser_preserves_nested_elements_text_and_attributes() {
        let document = parse_dom_document(
            r#"<!doctype html><html><head><title>DOM &amp; Browser</title></head>
            <body><main data-test-id='main'><h1>Hello <em>world</em><span class="sr-only">noise</span></h1><img src=local.png hidden></main></body></html>"#,
        );

        let title = document.first_descendant_by_tag("title").unwrap();
        assert_eq!(title.text_content(), "DOM & Browser");

        let main = document.first_descendant_by_tag("main").unwrap();
        assert_eq!(main.attr("data-test-id"), Some("main"));
        assert_eq!(main.text_content(), "Hello world");

        let image = document.first_descendant_by_tag("img").unwrap();
        assert_eq!(image.attr("src"), Some("local.png"));
        assert!(image.has_attr("hidden"));
        assert!(image.children.is_empty());
    }

    #[test]
    fn dom_parser_keeps_angle_brackets_inside_quoted_attributes() {
        let document = parse_dom_document(
            r#"<html><body><input type="submit" value="<input type=submit>"><p>After</p></body></html>"#,
        );

        let input = document.first_descendant_by_tag("input").unwrap();
        assert_eq!(input.attr("value"), Some("<input type=submit>"));
        assert_eq!(
            document
                .first_descendant_by_tag("body")
                .unwrap()
                .text_content(),
            "After"
        );
    }

    #[test]
    fn dom_parser_treats_source_as_void_element() {
        let document = parse_dom_document(
            r#"<html><body><picture><source srcset="wide.avif"><source srcset="small.avif"><img src="fallback.avif"></picture><p>After</p></body></html>"#,
        );
        let picture = document.first_descendant_by_tag("picture").unwrap();

        assert_eq!(picture.children.len(), 3);
        assert_eq!(
            picture
                .children
                .iter()
                .filter(|child| matches!(child, DomNode::Element(element) if element.tag_name == "source"))
                .count(),
            2
        );
        assert!(document.first_descendant_by_tag("p").is_some());
    }

    #[test]
    fn dom_backed_document_title_reuses_tree_text_content() {
        let document = parse_html_document(
            "<html><head><title>Hello <span>DOM</span></title></head><body><p>Body</p></body></html>",
            "https://example.test/",
        );

        assert_eq!(document.title, "Hello DOM");
    }

    #[test]
    fn render_graph_computes_inherited_and_relative_box_styles() {
        let html = r#"
            <html>
              <head>
                <style>
                  body { color: #112233; font-size: 20px; }
                  .box { padding: 4px 8px; width: 50%; }
                  #leaf { color: #445566; }
                </style>
              </head>
              <body>
                <div class="box"><span id="leaf">Text</span></div>
              </body>
            </html>
        "#;
        let html = remove_html_comments(html);
        let dom = parse_dom_document(&html);
        let style = rich_canvas::parse_basic_css(extract_tag_inner(&html, "style").unwrap_or(""));
        let graph = build_render_graph(&dom, &style);
        let div = find_render_element_by_class(&graph.root, "box").unwrap();
        let leaf = find_render_element_by_id(&graph.root, "leaf").unwrap();

        assert_eq!(div.style.color, egui::Color32::from_rgb(17, 34, 51));
        assert_eq!(div.style.font_size, 20.0);
        assert_eq!(div.style.padding.left, 8.0);
        assert_eq!(div.style.padding.top, 4.0);
        assert_eq!(div.style.width, Some(380.0));
        assert_eq!(leaf.style.color, egui::Color32::from_rgb(68, 85, 102));
        assert_eq!(leaf.style.font_size, 20.0);
    }

    #[test]
    fn render_graph_width_auto_overrides_earlier_percentage_width() {
        let html = r#"
            <html>
              <head>
                <style>
                  .button { width: 100%; }
                  .button.compact { width: auto; }
                </style>
              </head>
              <body>
                <a id="target" class="button compact">Sign in</a>
              </body>
            </html>
        "#;
        let html = remove_html_comments(html);
        let dom = parse_dom_document(&html);
        let style = rich_canvas::parse_basic_css(extract_tag_inner(&html, "style").unwrap_or(""));
        let graph = build_render_graph(&dom, &style);
        let target = find_render_element_by_id(&graph.root, "target").unwrap();

        assert_eq!(target.style.width, None);
    }

    #[test]
    fn canvas_debug_line_index_parses_only_object_rows() {
        assert_eq!(
            parse_canvas_debug_line_index("0004 Image rect=(0,0)"),
            Some(4)
        );
        assert_eq!(
            parse_canvas_debug_line_index("CanvasGraph viewport=1x1"),
            None
        );
        assert_eq!(parse_canvas_debug_line_index("0004Image rect=(0,0)"), None);
        assert_eq!(parse_canvas_debug_line_index("abcd Image rect=(0,0)"), None);
    }

    #[test]
    fn render_graph_applies_browser_default_paragraph_margins() {
        let html = remove_html_comments(
            r##"<html><body><p id="top-link"><a href="#top">[Top]</a></p></body></html>"##,
        );
        let dom = parse_dom_document(&html);
        let style = rich_canvas::parse_basic_css("");
        let graph = build_render_graph(&dom, &style);
        let paragraph = find_render_element_by_id(&graph.root, "top-link").unwrap();

        assert!(paragraph.style.margin.top > 0.0);
        assert!(paragraph.style.margin.bottom > 0.0);
    }

    #[test]
    fn render_graph_matches_child_adjacent_sibling_margin_selectors() {
        let html = r#"
            <html>
              <head><style>article > * + * { margin-top: 1em; }</style></head>
              <body>
                <article>
                  <header id="first">First</header>
                  <footer id="second">Second</footer>
                </article>
              </body>
            </html>
        "#;
        let html = remove_html_comments(html);
        let dom = parse_dom_document(&html);
        let style = rich_canvas::parse_basic_css(extract_tag_inner(&html, "style").unwrap_or(""));
        let graph = build_render_graph(&dom, &style);
        let first = find_render_element_by_id(&graph.root, "first").unwrap();
        let second = find_render_element_by_id(&graph.root, "second").unwrap();

        assert_eq!(first.style.margin.top, 0.0);
        assert_eq!(second.style.margin.top, 16.0);
    }

    #[test]
    fn render_graph_merges_side_specific_margin_rules() {
        let html = r#"
            <html>
              <head>
                <style>
                  h2 { margin-top: 3rem; }
                  h2, h3 { margin-bottom: 0.8rem; }
                </style>
              </head>
              <body><h2 id="heading">Heading</h2></body>
            </html>
        "#;
        let html = remove_html_comments(html);
        let dom = parse_dom_document(&html);
        let style = rich_canvas::parse_basic_css(extract_tag_inner(&html, "style").unwrap_or(""));
        let graph = build_render_graph(&dom, &style);
        let heading = find_render_element_by_id(&graph.root, "heading").unwrap();

        assert_eq!(heading.style.margin.top, heading.style.font_size);
        assert_eq!(heading.style.margin.bottom, heading.style.font_size * 0.5);
    }

    #[test]
    fn cached_latex_page_uses_saved_stylesheet_when_style_css_is_missing() {
        let cache_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sample_pages/cache");
        let missing_style = cache_dir.join("style.css");

        assert!(!missing_style.exists());
        assert_eq!(
            stylesheet_path_with_local_fallback(missing_style.to_str().unwrap()),
            cache_dir.join("latex_style.css")
        );

        let document = load_html_document(&cache_dir.join("latex_elements.html")).unwrap();

        assert_eq!(document.style.main_max_width, 640.0);
        assert_eq!(
            document.style.text_color,
            egui::Color32::from_rgb(27, 24, 24)
        );
        assert_eq!(
            document.style.page_background,
            egui::Color32::from_rgb(249, 250, 251)
        );
    }

    #[test]
    fn canvas_graph_keeps_resolved_text_attributes_and_coordinates() {
        let document = parse_html_document(
            r#"
            <html>
              <head><style>.box { color: #445566; font-size: 20px; padding: 10px; }</style></head>
              <body><div class="box"><span>Hello graph</span><em>italic</em><u>under</u><del>gone</del><mark>marked</mark></div></body>
            </html>
            "#,
            "https://example.test/",
        );

        let text = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Text(text) if text.text == "Hello graph" => Some(text),
                _ => None,
            })
            .expect("expected CanvasGraph text object");

        assert_eq!(text.color, egui::Color32::from_rgb(68, 85, 102));
        assert_eq!(text.font_size, 20.0);
        assert!(text.rect.min.x >= 10.0);
        assert!(text.rect.min.y >= 10.0);

        assert!(document.canvas_graph.objects.iter().any(|object| matches!(
            object,
            CanvasObject::Text(text) if text.text == "italic" && text.font_style_italic
        )));
        assert!(document.canvas_graph.objects.iter().any(|object| matches!(
            object,
            CanvasObject::Text(text) if text.text == "under" && text.text_decoration_underline
        )));
        assert!(document.canvas_graph.objects.iter().any(|object| matches!(
            object,
            CanvasObject::Text(text) if text.text == "gone" && text.text_decoration_strikethrough
        )));
        assert!(
            document
                .canvas_graph
                .objects
                .iter()
                .any(|object| matches!(
                    object,
                    CanvasObject::Text(text) if text.text == "marked" && text.text_background != egui::Color32::TRANSPARENT
                ))
        );
    }

    #[test]
    fn render_graph_text_nodes_are_inline_and_lists_do_not_flatten_nested_text() {
        let html = r##"
            <html>
              <body>
                <ul>
                  <li><a href="#text">Text</a><ul><li><a href="#headings">Headings</a></li></ul></li>
                </ul>
              </body>
            </html>
        "##;
        let html = remove_html_comments(html);
        let dom = parse_dom_document(&html);
        let style = rich_canvas::parse_basic_css("");
        let graph = build_render_graph(&dom, &style);
        let text_node = find_render_text(&graph.root, "Text").unwrap();

        assert_eq!(text_node.style.display, CssDisplay::Inline);

        let document = parse_html_document(html.as_str(), "https://example.test/");
        let mut list_items = Vec::new();
        collect_list_item_text(&document.blocks, &mut list_items);

        assert_eq!(list_items, vec!["Text".to_owned(), "Headings".to_owned()]);
        assert!(!list_items.iter().any(|text| text.contains("Text Headings")));
    }

    #[test]
    fn canvas_graph_flows_inline_text_lists_and_tables() {
        let document = parse_html_document(
            r##"
            <html>
              <body>
                <p>One <a href="#two">two</a> three</p>
                <ul><li><a href="#item">Item link</a></li></ul>
                <table>
                  <tr><th>Head A</th><th>Head B</th></tr>
                  <tr><td>Cell A</td><td>Cell B</td></tr>
                </table>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let one = find_canvas_text(&document.canvas_graph, "One").expect("expected first run");
        let two = find_canvas_text(&document.canvas_graph, " two").expect("expected link run");
        let three = find_canvas_text(&document.canvas_graph, " three").expect("expected third run");
        assert_eq!(one.rect.top(), two.rect.top());
        assert_eq!(two.rect.top(), three.rect.top());
        assert_eq!(two.href.as_deref(), Some("#two"));

        let marker = find_canvas_text(&document.canvas_graph, "•").expect("expected list marker");
        let item =
            find_canvas_text(&document.canvas_graph, "Item link").expect("expected list text");
        assert!(marker.rect.left() < item.rect.left());
        assert_eq!(marker.rect.top(), item.rect.top());
        assert_eq!(item.href.as_deref(), Some("#item"));

        let table_rects = document
            .canvas_graph
            .objects
            .iter()
            .filter(|object| matches!(object, CanvasObject::Rect(_)))
            .count();
        assert!(table_rects >= 4);
        assert!(
            find_canvas_text(&document.canvas_graph, "Head A")
                .is_some_and(|text| text.font_weight_bold)
        );
        assert!(find_canvas_text(&document.canvas_graph, "Cell B").is_some());
    }

    #[test]
    fn canvas_graph_lays_out_nested_lists_with_real_child_widths() {
        let document = parse_html_document(
            r##"
            <html>
              <body>
                <nav>
                  <ul>
                    <li>
                      <a href="#text">Text</a>
                      <ul><li><a href="#headings">Headings</a></li></ul>
                    </li>
                  </ul>
                </nav>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let parent =
            find_canvas_text(&document.canvas_graph, "Text").expect("expected parent link");
        let child =
            find_canvas_text(&document.canvas_graph, "Headings").expect("expected nested link");

        assert_eq!(parent.href.as_deref(), Some("#text"));
        assert_eq!(child.href.as_deref(), Some("#headings"));
        assert!(child.rect.width() > 40.0);
        assert!(child.rect.top() > parent.rect.top());
    }

    #[test]
    fn canvas_graph_uses_ordered_list_markers() {
        let document = parse_html_document(
            r#"<html><body><ol><li>One</li><li>Two</li></ol></body></html>"#,
            "https://example.test/",
        );

        let one = find_canvas_text(&document.canvas_graph, "1.").expect("expected first marker");
        let two = find_canvas_text(&document.canvas_graph, "2.").expect("expected second marker");
        let one_text =
            find_canvas_text(&document.canvas_graph, "One").expect("expected first item");
        let two_text =
            find_canvas_text(&document.canvas_graph, "Two").expect("expected second item");

        assert!(one.rect.left() < one_text.rect.left());
        assert!(two.rect.left() < two_text.rect.left());
        assert!(two.rect.top() > one.rect.top());
    }

    #[test]
    fn canvas_graph_emits_inline_svg_replaced_content() {
        let document = parse_html_document(
            r##"
            <html>
              <body>
                <div>
                  <svg width="100px" height="80px" aria-label="chart">
                    <circle cx="50" cy="40" r="20" fill="#ff0000"></circle>
                  </svg>
                </div>
                <h3>After SVG</h3>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let svg_rect = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Svg(svg) => Some(svg.rect),
                _ => None,
            })
            .expect("expected CanvasGraph SVG object");
        let after =
            find_canvas_text(&document.canvas_graph, "After SVG").expect("expected text after SVG");

        assert!((svg_rect.width() - 100.0).abs() < 0.1);
        assert!((svg_rect.height() - 80.0).abs() < 0.1);
        assert!(svg_rect.top() > 0.0);
        assert!(after.rect.top() >= svg_rect.bottom());
    }

    #[test]
    fn canvas_graph_emits_path_only_svg_as_visible_icon() {
        let document = parse_html_document(
            r##"
            <html>
              <body>
                <svg width="16" height="16" aria-label="search">
                  <path fill="currentColor" d="M1 1h14v14H1z"></path>
                </svg>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let image_rect = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Image(image) if image.src == "inline-svg-raster" => Some(image.rect),
                _ => None,
            })
            .expect("expected path-only SVG to rasterize into a visible image");

        assert!(
            (image_rect.width() - 16.0).abs() < 0.1,
            "expected 16px rasterized svg width, got {}",
            image_rect.width()
        );
        assert!((image_rect.height() - 16.0).abs() < 0.1);
    }

    #[test]
    fn flex_svg_item_uses_explicit_width_for_rasterized_svg() {
        let document = parse_html_document(
            r##"
            <html>
              <head>
                <style>
                  .row { display: flex; width: 200px; }
                  .logo { display: flex; margin-right: 24px; }
                  svg { width: 70px; height: 20px; }
                </style>
              </head>
              <body>
                <div class="row">
                  <div class="logo">
                    <svg viewBox="0 0 70 20" aria-label="logo">
                      <path fill="#ffffff" d="M0 0h70v20H0z"></path>
                    </svg>
                  </div>
                  <span>Next</span>
                </div>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let image_rect = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Image(image) if image.src == "inline-svg-raster" => Some(image.rect),
                _ => None,
            })
            .expect("expected path-only SVG to rasterize into a visible image");

        assert!(
            image_rect.width() >= 69.0,
            "expected 70px rasterized svg width, got {}",
            image_rect.width()
        );
    }

    #[test]
    fn positioned_positive_z_index_paints_after_normal_siblings() {
        let document = parse_html_document(
            r##"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .header {
                    position: sticky;
                    z-index: 3;
                    width: 200px;
                    height: 48px;
                    background: #ff0000;
                  }
                  .hero {
                    margin-top: -24px;
                    width: 200px;
                    height: 96px;
                    background: #00ff00;
                  }
                </style>
              </head>
              <body>
                <div class="header"></div>
                <div class="hero"></div>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let mut header_index = None;
        let mut hero_index = None;
        for (index, object) in document.canvas_graph.objects.iter().enumerate() {
            if let CanvasObject::Rect(rect) = object {
                if rect.fill == egui::Color32::from_rgb(0xff, 0x00, 0x00) {
                    header_index = Some(index);
                } else if rect.fill == egui::Color32::from_rgb(0x00, 0xff, 0x00) {
                    hero_index = Some(index);
                }
            }
        }

        let header_index = header_index.expect("expected header background rect");
        let hero_index = hero_index.expect("expected hero background rect");
        assert!(
            header_index > hero_index,
            "expected z-index header to paint after hero, got header={header_index}, hero={hero_index}"
        );
    }

    #[test]
    fn absolute_icon_button_in_zero_height_wrapper_keeps_button_rect() {
        let document = parse_html_document(
            r##"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .search { display: flex; width: 120px; height: 56px; background: #333333; }
                  .left { position: relative; width: 40px; }
                  button {
                    position: absolute;
                    top: 12px;
                    right: 0;
                    min-width: 32px;
                    height: 32px;
                    padding-left: 8px;
                    padding-right: 8px;
                    border-radius: 9999px;
                    background: #deded9;
                    border: 0;
                  }
                  svg { width: 16px; height: 16px; }
                </style>
              </head>
              <body>
                <div class="search">
                  <div class="left">
                    <button type="submit" aria-label="Search">
                      <svg viewBox="0 0 16 16" aria-hidden="true">
                        <path fill="#ffffff" d="M1 1h14v14H1z"></path>
                      </svg>
                    </button>
                  </div>
                </div>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let button_rect = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Rect(rect)
                    if rect.fill == egui::Color32::from_rgb(0xde, 0xde, 0xd9) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("expected positioned button background");
        let icon_rect = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Image(image) if image.src == "inline-svg-raster" => Some(image.rect),
                _ => None,
            })
            .expect("expected positioned button icon");

        assert!(
            button_rect.width() >= 32.0,
            "expected non-zero button width, got {:?}",
            button_rect
        );
        assert!(
            button_rect.height() >= 32.0,
            "expected non-zero button height, got {:?}",
            button_rect
        );
        assert!(
            button_rect.contains_rect(icon_rect),
            "expected icon {:?} to be inside button {:?}",
            icon_rect,
            button_rect
        );
    }

    #[test]
    fn form_textbox_and_submit_button_share_canvas_form_id() {
        let document = parse_html_document(
            r##"
            <html>
              <body>
                <form id="search-form">
                  <input name="q" placeholder="Search the web..." value="trees">
                  <button type="submit" aria-label="Search">
                    <svg viewBox="0 0 16 16"><path d="M1 1h14v14H1z"></path></svg>
                  </button>
                </form>
              </body>
            </html>
            "##,
            "https://example.test/",
        );

        let input = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Input(input) if input.label == "Search the web..." => Some(input),
                _ => None,
            })
            .expect("expected form textbox");
        let button = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Button(button) if button.text == "Search" => Some(button),
                _ => None,
            })
            .expect("expected form submit button");

        assert_eq!(input.form_id.as_deref(), Some("search-form"));
        assert_eq!(button.form_id.as_deref(), Some("search-form"));
        assert_eq!(button.button_type, "submit");
    }

    #[test]
    fn absolute_child_positions_from_padding_box_not_content_box() {
        let document = parse_html_document(
            r##"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .hero {
                    position: relative;
                    padding-top: 64px;
                    width: 400px;
                    height: 200px;
                    overflow: hidden;
                  }
                  svg {
                    position: absolute;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    object-fit: cover;
                  }
                </style>
              </head>
              <body>
                <section class="hero">
                  <picture>
                    <svg viewBox="0 0 16 16"><path d="M0 0h16v16H0z"></path></svg>
                  </picture>
                </section>
              </body>
            </html>
            "##,
            "file:///tmp/test.html",
        );

        let image = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Image(image) if image.src == "inline-svg-raster" => Some(image),
                _ => None,
            })
            .expect("expected absolute image");

        assert_eq!(image.rect.top(), 0.0);
        assert!(
            (image.rect.width() - 400.0).abs() < 0.1,
            "expected absolute image to fill the positioned section width, got {:?}",
            image.rect
        );
        assert!(
            (image.rect.height() - 264.0).abs() < 0.1,
            "expected absolute image to fill the positioned section padding box, got {:?}",
            image.rect
        );
    }

    #[test]
    fn document_without_explicit_theme_uses_context_dark_root_class() {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::dark());
        let dom = parse_dom_document("<html><body>Dark inferred</body></html>");

        assert_eq!(document_theme_root_classes(&dom, Some(&ctx)), vec!["dark"]);
    }

    #[test]
    fn flex_textarea_wrapper_grows_between_button_sections() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .search { display: flex; align-items: center; width: 360px; gap: 8px; }
                  .left { width: 32px; }
                  .middle { display: flex; min-width: 0; }
                  .middle textarea { width: 100%; }
                  .right { width: 80px; }
                </style>
              </head>
              <body>
                <div class="search">
                  <div class="left">S</div>
                  <div class="middle"><textarea placeholder="Search the web..."></textarea></div>
                  <div class="right"><button>AI Chat</button></div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let icon = find_canvas_text(&document.canvas_graph, "S").expect("expected left section");
        let placeholder =
            find_canvas_input(&document.canvas_graph, "Search the web...").expect("expected input");
        let ai_chat =
            find_canvas_text(&document.canvas_graph, "AI Chat").expect("expected AI button");

        assert!(placeholder.rect.left() > icon.rect.right());
        assert!(ai_chat.rect.left() > placeholder.rect.right());
        assert!((icon.rect.center().y - ai_chat.rect.center().y).abs() < 6.0);
    }

    #[test]
    fn grid_template_columns_wraps_items_by_declared_column_count() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; width: 300px; }
                </style>
              </head>
              <body>
                <div class="grid">
                  <div>One</div>
                  <div>Two</div>
                  <div>Three</div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let one = find_canvas_text(&document.canvas_graph, "One").expect("expected one");
        let two = find_canvas_text(&document.canvas_graph, "Two").expect("expected two");
        let three = find_canvas_text(&document.canvas_graph, "Three").expect("expected three");

        assert_eq!(one.rect.top(), two.rect.top());
        assert!(two.rect.left() > one.rect.right());
        assert!(three.rect.top() > one.rect.top());
        assert!(three.rect.left() <= one.rect.left() + 1.0);
    }

    #[test]
    fn grid_template_areas_position_items_by_declared_names() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .grid {
                    display: grid;
                    grid-template-areas:
                      "side main"
                      "foot foot";
                    grid-template-columns: 1fr 1fr;
                    gap: 10px;
                    width: 410px;
                  }
                  .main { grid-area: main; }
                  .side { grid-area: side; }
                  .foot { grid-area: foot; }
                </style>
              </head>
              <body>
                <div class="grid">
                  <div class="main">Main</div>
                  <div class="side">Side</div>
                  <div class="foot">Foot</div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let main = find_canvas_text(&document.canvas_graph, "Main").expect("expected main");
        let side = find_canvas_text(&document.canvas_graph, "Side").expect("expected side");
        let foot = find_canvas_text(&document.canvas_graph, "Foot").expect("expected foot");

        assert_eq!(main.rect.top(), side.rect.top());
        assert!(main.rect.left() > side.rect.right());
        assert!(foot.rect.top() > side.rect.top());
        assert!(foot.rect.left() <= side.rect.left() + 1.0);
    }

    #[test]
    fn canvas_graph_uses_flex_row_positions_from_layout_tree() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  .row { display: flex; flex-direction: row; gap: 24px; }
                  .item { width: 120px; }
                </style>
              </head>
              <body>
                <div class="row">
                  <div class="item">First</div>
                  <div class="item">Second</div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let first = find_canvas_text(&document.canvas_graph, "First").expect("expected first item");
        let second =
            find_canvas_text(&document.canvas_graph, "Second").expect("expected second item");

        assert_eq!(first.rect.top(), second.rect.top());
        assert!(second.rect.left() - first.rect.left() >= 120.0);
    }

    #[test]
    fn flex_row_auto_margin_absorbs_free_space() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  .row {
                    display: flex;
                    flex-direction: row;
                    width: 500px;
                  }
                  .left { width: 40px; }
                  .spacer { margin-left: auto; width: 1px; }
                  .right { width: 80px; }
                </style>
              </head>
              <body>
                <div class="row">
                  <div class="left">Left</div>
                  <div class="spacer"></div>
                  <div class="right">Right</div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let left = find_canvas_text(&document.canvas_graph, "Left").expect("expected left text");
        let right = find_canvas_text(&document.canvas_graph, "Right").expect("expected right text");

        assert_eq!(left.rect.top(), right.rect.top());
        assert!(right.rect.left() > 410.0, "right rect was {:?}", right.rect);
    }

    #[test]
    fn flex_grow_item_uses_auto_base_before_free_space_distribution() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .row { display: flex; width: 200px; }
                  .title { flex-grow: 1; font-size: 32px; }
                  .tools { width: 160px; }
                </style>
              </head>
              <body>
                <div class="row">
                  <h1 class="title">Hello world</h1>
                  <div class="tools">Tools</div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let title =
            find_canvas_text(&document.canvas_graph, "Hello world").expect("expected full title");
        let tools = find_canvas_text(&document.canvas_graph, "Tools").expect("expected tools");

        assert!(tools.rect.left() > title.rect.left());
    }

    #[test]
    fn flex_row_width_percent_uses_actual_containing_block() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  header { padding: 0 24px; }
                  .nav {
                    display: flex;
                    flex-direction: row;
                    width: 100%;
                  }
                  .logo { width: 70px; }
                  .spacer { margin-left: auto; width: 1px; }
                  .action { width: 80px; }
                </style>
              </head>
              <body>
                <header>
                  <div class="nav">
                    <div class="logo">Logo</div>
                    <div class="spacer"></div>
                    <div class="action">Action</div>
                  </div>
                </header>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let action =
            find_canvas_text(&document.canvas_graph, "Action").expect("expected action text");

        assert!(
            action.rect.left() > 1100.0,
            "action should be near the full header right edge, got {:?}",
            action.rect
        );
    }

    #[test]
    fn flex_placeholder_span_and_button_render_horizontally() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  .hero-search {
                    display: flex;
                    align-items: center;
                    gap: 16px;
                    width: 600px;
                    height: 56px;
                    padding: 0 14px 0 20px;
                  }
                  .placeholder {
                    flex: 1;
                    font-size: 18px;
                  }
                  .hero-search button {
                    color: #ffffff;
                    font-weight: 700;
                  }
                </style>
              </head>
              <body>
                <div class="hero-search">
                  <span class="search-icon">Search icon</span>
                  <span class="placeholder">Search the web...</span>
                  <button>AI Chat</button>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let icon =
            find_canvas_text(&document.canvas_graph, "Search icon").expect("expected icon span");
        let placeholder = find_canvas_text(&document.canvas_graph, "Search the web...")
            .expect("expected placeholder span");
        let ai_chat =
            find_canvas_text(&document.canvas_graph, "AI Chat").expect("expected button text");

        assert!((icon.rect.center().y - placeholder.rect.center().y).abs() < 4.0);
        assert!((placeholder.rect.center().y - ai_chat.rect.center().y).abs() < 4.0);
        assert!(placeholder.rect.left() > icon.rect.right());
        assert!(ai_chat.rect.left() > placeholder.rect.right());
        assert_eq!(ai_chat.color, egui::Color32::WHITE);
        assert!(ai_chat.font_weight_bold);
    }

    #[test]
    fn percent_height_in_auto_height_flex_context_does_not_use_width() {
        let document = parse_html_document(
            r#"
            <html>
              <head>
                <style>
                  body { padding: 0; }
                  .search {
                    display: flex;
                    align-items: center;
                    width: 800px;
                    height: 100%;
                    background: #333333;
                    border: 1px solid #6c6c6c;
                    border-radius: 40px;
                  }
                  .left { width: 40px; }
                  .input { flex: 1; }
                  textarea { margin: 16px 16px 16px 8px; }
                </style>
              </head>
              <body>
                <div class="search">
                  <div class="left"></div>
                  <div class="input"><textarea placeholder="Search the web..."></textarea></div>
                </div>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        let search_rect = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Rect(rect)
                    if rect.fill == egui::Color32::from_rgb(0x33, 0x33, 0x33) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("expected search field background");

        assert!(
            search_rect.height() < 100.0,
            "percentage height in an indefinite container should stay intrinsic, got {:?}",
            search_rect
        );
    }

    #[test]
    fn textarea_placeholder_renders_as_placeholder_not_value() {
        let document = parse_html_document(
            r#"
            <html>
              <body>
                <textarea placeholder="Search the web..." name="q"></textarea>
              </body>
            </html>
            "#,
            "https://example.test/",
        );

        assert!(find_canvas_text(&document.canvas_graph, "Search the web...").is_some());
        assert!(
            find_canvas_text(
                &document.canvas_graph,
                "Search the web...: Search the web..."
            )
            .is_none()
        );
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Input { label, value } if label == "Search the web..." && value.is_empty()
        )));
    }

    #[test]
    fn anonymous_inline_replaced_content_is_lowered_to_canvas_graph() {
        let document = parse_html_document(
            r#"
            <html>
              <body>
                <picture>
                  <source srcset="test_2x2.avif" type="image/avif">
                  <img src="test_2x2.avif" alt="hero" width="40" height="40">
                </picture>
              </body>
            </html>
            "#,
            &path_to_file_url(Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../sample_pages/anonymous_picture.html"
            ))),
        );

        assert!(document.canvas_graph.objects.iter().any(|object| matches!(
            object,
            CanvasObject::Image(image)
                if image.alt == "hero" && image.image.color_image.size == [2, 2]
        )));
    }

    #[test]
    fn text_container_lowering_preserves_inline_form_controls() {
        let document = parse_html_document(
            r#"<html><body><form><p><label for="name">Name</label><input id="name" value="Ada"></p><p><button>Save</button></p></form></body></html>"#,
            "https://example.test/",
        );

        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Input { label, value } if label == "Name" && value == "Ada"
        )));
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Button { text } if text == "Save"
        )));
    }

    #[test]
    fn fragment_links_resolve_against_current_document_without_reloading_path_fragment() {
        let source = "file:///tmp/test_basic_page.html";

        assert_eq!(
            resolve_navigation_url(source, "#form-section"),
            "file:///tmp/test_basic_page.html#form-section"
        );
        assert!(same_document_url(
            source,
            "file:///tmp/test_basic_page.html#form-section"
        ));
        assert_eq!(
            input_to_path("file:///tmp/test_basic_page.html#form-section"),
            PathBuf::from("/tmp/test_basic_page.html")
        );
    }

    #[test]
    fn default_fixture_parses_browser_blocks() {
        let document = load_html_document(Path::new(DEFAULT_PAGE_PATH)).unwrap();

        assert_eq!(document.title, "AlmostThere Sample Page");
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Heading { text, .. } if text == "AlmostThere Sample Page"
        )));
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::StyledBox { style, .. } if style.padding.left == 16.0
        )));
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Link { href, .. } if href == "#form-section"
        )));
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Button { text } if text == "Test Button"
        )));
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Input { label, value } if label == "Name" && value == "AlmostThere"
        )));
    }

    #[test]
    fn parses_bookmark_file_lines() {
        let bookmarks = parse_bookmarks(
            "AlmostThere\tfile:///tmp/test.html\ninvalid\nDocs\thttps://example.com\n",
        );

        assert_eq!(
            bookmarks,
            vec![
                Bookmark {
                    title: "AlmostThere".to_owned(),
                    url: "file:///tmp/test.html".to_owned(),
                },
                Bookmark {
                    title: "Docs".to_owned(),
                    url: "https://example.com".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn bookmark_local_token_resolves_at_runtime() {
        let bookmarks =
            parse_bookmarks("Local\tfile:///[local]/sample_pages/test_basic_page.html\n");
        let expected = format!(
            "file:///{}/sample_pages/test_basic_page.html",
            local_bookmark_path_token()
        );

        assert_eq!(
            bookmarks,
            vec![Bookmark {
                title: "Local".to_owned(),
                url: expected,
            }]
        );
    }

    #[test]
    fn bookmark_urls_are_saved_with_local_token_when_inside_workspace() {
        let url = format!(
            "file:///{}/sample_pages/test_basic_page.html",
            local_bookmark_path_token()
        );

        assert_eq!(
            portable_bookmark_url(&url),
            "file:///[local]/sample_pages/test_basic_page.html"
        );
    }

    #[test]
    fn script_test_bookmarks_cover_generated_unit_tests() {
        let bookmarks = script_test_bookmarks();

        assert_eq!(bookmarks.len(), 41);
        assert!(bookmarks[0].title.starts_with("001 "));
        assert!(
            bookmarks[0]
                .url
                .ends_with("/JustBarelyScript/UnitTest/001-basic-script-execution/index.html")
        );
        assert!(
            bookmarks[40]
                .url
                .ends_with("/JustBarelyScript/UnitTest/041-proxy/index.html")
        );
    }

    #[test]
    fn script_console_messages_include_logs_and_parse_errors() {
        let messages = script_console_messages_from_html(
            r#"
            <script>console.log("ready");</script>
            <script>let = ;</script>
            "#,
        );

        assert!(messages.iter().any(|message| {
            message.level == justbarelyscript::ConsoleLevel::Log && message.text == "ready"
        }));
        assert!(messages.iter().any(|message| message.level
            == justbarelyscript::ConsoleLevel::Error
            && message.text.contains("Parse error")));
    }

    #[test]
    fn script_execution_seeds_navigator_and_screen_globals() {
        let document = parse_html_document(
            r#"
            <div id="ua"></div>
            <div id="screen"></div>
            <script>
            document.getElementById("ua").textContent = navigator.userAgent;
            document.getElementById("screen").textContent = screen.width;
            </script>
            "#,
            "file:///tmp/seeded-browser-globals.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "AlmostThere Browser/0.1.0").is_some());
        assert!(
            document.canvas_graph.objects.iter().any(|object| {
                matches!(object, CanvasObject::Text(text) if text.text.parse::<u32>().is_ok())
            }),
            "expected numeric screen width text in canvas graph"
        );
    }

    #[test]
    fn same_origin_external_scripts_execute_in_document_order() {
        let dir = std::env::temp_dir().join(format!(
            "almostthere-script-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp script dir");
        fs::write(dir.join("a.js"), r#"window.value = "A";"#).expect("write first script");
        fs::write(
            dir.join("b.js"),
            r#"document.getElementById("result").textContent = window.value + "B";"#,
        )
        .expect("write second script");

        let html = r#"
            <div id="result">Before</div>
            <script src="a.js"></script>
            <script src="b.js"></script>
        "#;
        let source = path_to_file_url(&dir.join("index.html"));
        let document = parse_html_document(html, &source);

        assert!(find_canvas_text(&document.canvas_graph, "AB").is_some());
        assert!(find_canvas_text(&document.canvas_graph, "Before").is_none());
    }

    #[test]
    fn external_script_parse_errors_are_reported_with_source_label() {
        let dir = std::env::temp_dir().join(format!(
            "almostthere-script-error-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp script dir");
        fs::write(dir.join("bad.js"), "let = ;").expect("write bad script");

        let html = r#"<script src="bad.js"></script>"#;
        let source = path_to_file_url(&dir.join("index.html"));
        let messages = script_console_messages_from_html_with_source(html, Some(&source));

        assert!(messages.iter().any(|message| {
            message.level == justbarelyscript::ConsoleLevel::Error
                && message.text.contains("bad.js")
                && message.text.contains("Parse error")
        }));
    }

    #[test]
    fn external_script_parse_errors_include_construct_diagnostics() {
        let dir = std::env::temp_dir().join(format!(
            "almostthere-script-diagnostic-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp script dir");
        fs::write(
            dir.join("bundle.js"),
            r#"import runtime from "./runtime.js";
            (window.webpackJsonp = window.webpackJsonp || []).push([["x"], {
                1: function(module) {
                    class Collector {}
                    const run = async () => await fetch("/fingerprint");
                    const pick = window?.navigator?.userAgent ?? "";
                    const broken = ;
                    module.exports = { Collector, run, pick };
                }
            }]);"#,
        )
        .expect("write diagnostic bundle");

        let html = r#"<script src="bundle.js"></script>"#;
        let source = path_to_file_url(&dir.join("index.html"));
        let messages = script_console_messages_from_html_with_source(html, Some(&source));
        let error = messages
            .iter()
            .find(|message| message.level == justbarelyscript::ConsoleLevel::Error)
            .expect("expected parse diagnostic");

        assert!(error.text.contains("bundle.js"));
        assert!(error.text.contains("Unsupported constructs"));
        assert!(error.text.contains("ES modules"));
        assert!(error.text.contains("Webpack runtime"));
        assert!(error.text.contains("class syntax"));
        assert!(error.text.contains("async/await"));
        assert!(error.text.contains("optional chaining"));
        assert!(error.text.contains("nullish coalescing"));
        assert!(error.text.contains("fetch"));
    }

    #[test]
    fn fingerprint_suite_values_are_exposed_to_scripts() {
        let expected = justbarelyscript::FingerprintSuite::detect();
        let document = parse_html_document(
            r#"
            <div id="timezone"></div>
            <div id="storage"></div>
            <div id="canvas"></div>
            <div id="location"></div>
            <script>
            localStorage.setItem("fp", "ok");
            sessionStorage.session = "yes";
            document.getElementById("timezone").textContent = new Date().getTimezoneOffset();
            document.getElementById("storage").textContent = localStorage.getItem("fp") + "/" + sessionStorage.session;
            var canvas = document.createElement("canvas");
            document.getElementById("canvas").textContent = canvas.toDataURL();
            document.getElementById("location").textContent = location.href;
            </script>
            "#,
            "https://www.amiunique.org/fingerprint",
        );

        assert!(
            find_canvas_text(
                &document.canvas_graph,
                &expected.timezone.offset_minutes.to_string()
            )
            .is_some()
        );
        assert!(find_canvas_text(&document.canvas_graph, "ok/yes").is_some());
        assert!(
            find_canvas_text(&document.canvas_graph, &expected.canvas.data_url).is_some(),
            "expected precomputed canvas data URL to be exposed"
        );
        assert!(
            find_canvas_text(
                &document.canvas_graph,
                "https://www.amiunique.org/fingerprint"
            )
            .is_some()
        );
    }

    #[test]
    fn synthetic_amiunique_collector_renders_precomputed_js_attributes() {
        let expected = justbarelyscript::FingerprintSuite::detect();
        let document = parse_html_document(
            r#"
            <table>
              <tbody id="js-attributes"><tr><td>No data available</td></tr></tbody>
            </table>
            <script>
            var fp = window.__almostthereFingerprint;
            document.getElementById("js-attributes").innerHTML =
              "<tr><td>Canvas</td><td>" + fp.canvas + "</td></tr>" +
              "<tr><td>Fonts</td><td>" + fp.fontsEnum + "</td></tr>" +
              "<tr><td>Audio</td><td>" + fp.audio + "</td></tr>" +
              "<tr><td>Modernizr</td><td>" + fp.modernizr + "</td></tr>" +
              "<tr><td>Touch</td><td>" + fp.touchSupport + "</td></tr>";
            </script>
            "#,
            "https://www.amiunique.org/fingerprint",
        );

        assert!(find_canvas_text(&document.canvas_graph, "No data available").is_none());
        assert!(find_canvas_text(&document.canvas_graph, "Canvas").is_some());
        assert!(find_canvas_text(&document.canvas_graph, &expected.canvas.data_url).is_some());
        assert!(find_canvas_text(&document.canvas_graph, "Fonts").is_some());
        assert!(
            document.canvas_graph.objects.iter().any(|object| {
                matches!(object, CanvasObject::Text(text) if text.text.contains("Arial--"))
            }),
            "expected rendered AMIUnique font availability tokens"
        );
        assert!(find_canvas_text(&document.canvas_graph, "Modernizr").is_some());
        assert!(
            find_canvas_text(
                &document.canvas_graph,
                &expected.modernizr.as_amiunique_string()
            )
            .is_some()
        );
    }

    #[test]
    fn oversized_external_scripts_are_not_parsed_on_render_path() {
        let dir = std::env::temp_dir().join(format!(
            "almostthere-large-script-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp script dir");
        fs::write(dir.join("large.js"), "var x = 1;\n".repeat(500_000))
            .expect("write large script");

        let html = r#"
            <div id="result">Still renders</div>
            <script src="large.js"></script>
        "#;
        let source = path_to_file_url(&dir.join("index.html"));
        let document = parse_html_document(html, &source);
        let messages = script_console_messages_from_html_with_source(html, Some(&source));

        assert!(find_canvas_text(&document.canvas_graph, "Still renders").is_some());
        assert!(messages.iter().any(|message| {
            message.level == justbarelyscript::ConsoleLevel::Error
                && message.text.contains("exceeds parser budget")
        }));
    }

    #[test]
    fn oversized_inline_scripts_are_not_parsed_on_render_path() {
        let html = format!(
            r#"
            <div id="result">SSR content remains visible</div>
            <script>{}</script>
            "#,
            "var nuxtState = 1;\n".repeat(280_000)
        );
        let document = parse_html_document(&html, "https://example.test/fingerprint");
        let messages = script_console_messages_from_html_with_source(
            &html,
            Some("https://example.test/fingerprint"),
        );

        assert!(find_canvas_text(&document.canvas_graph, "SSR content remains visible").is_some());
        assert!(messages.iter().any(|message| {
            message.level == justbarelyscript::ConsoleLevel::Error
                && message.text.contains("inline script skipped")
                && message.text.contains("exceeds parser budget")
        }));
    }

    #[test]
    fn first_script_test_applies_text_content_effect() {
        let document = parse_html_document(
            include_str!("../../JustBarelyScript/UnitTest/001-basic-script-execution/index.html"),
            "file:///tmp/001-basic-script-execution/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "After").is_some());
        assert!(find_canvas_text(&document.canvas_graph, "Before").is_none());
    }

    #[test]
    fn second_script_test_preserves_window_state_between_scripts() {
        let document = parse_html_document(
            include_str!(
                "../../JustBarelyScript/UnitTest/002-multiple-script-tags-execute-in-order/index.html"
            ),
            "file:///tmp/002-multiple-script-tags-execute-in-order/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "AB").is_some());
    }

    #[test]
    fn fourth_script_test_creates_and_appends_element() {
        let document = parse_html_document(
            include_str!("../../JustBarelyScript/UnitTest/004-element-creation/index.html"),
            "file:///tmp/004-element-creation/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "Created by script").is_some());
    }

    #[test]
    fn fifth_script_test_assigns_class_attribute() {
        let html = apply_safe_script_browser_effects(include_str!(
            "../../JustBarelyScript/UnitTest/005-css-class-assignment/index.html"
        ));

        assert!(html.contains(r#"<div id="box" class="active">Box</div>"#));
    }

    #[test]
    fn sixth_script_test_reads_back_set_attribute() {
        let document = parse_html_document(
            include_str!(
                "../../JustBarelyScript/UnitTest/006-setattribute-and-getattribute/index.html"
            ),
            "file:///tmp/006-setattribute-and-getattribute/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "ready").is_some());
    }

    #[test]
    fn seventh_script_test_replaces_inner_html() {
        let document = parse_html_document(
            include_str!(
                "../../JustBarelyScript/UnitTest/007-innerhtml-basic-replacement/index.html"
            ),
            "file:///tmp/007-innerhtml-basic-replacement/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "Hello").is_some());
    }

    #[test]
    fn eighth_script_test_query_selector_by_id_reads_existing_text() {
        let document = parse_html_document(
            include_str!("../../JustBarelyScript/UnitTest/008-query-selector-by-id/index.html"),
            "file:///tmp/008-query-selector-by-id/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "Hello").is_some());
    }

    #[test]
    fn ninth_script_test_query_selector_by_class_reads_first_match_without_id() {
        let document = parse_html_document(
            include_str!("../../JustBarelyScript/UnitTest/009-query-selector-by-class/index.html"),
            "file:///tmp/009-query-selector-by-class/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "First").is_some());
    }

    #[test]
    fn tenth_script_test_query_selector_all_length_counts_class_matches() {
        let document = parse_html_document(
            include_str!(
                "../../JustBarelyScript/UnitTest/010-queryselectorall-and-length/index.html"
            ),
            "file:///tmp/010-queryselectorall-and-length/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "3").is_some());
    }

    #[test]
    fn eleventh_script_test_for_loop_appends_list_items() {
        let html = apply_safe_script_browser_effects(&remove_html_comments(include_str!(
            "../../JustBarelyScript/UnitTest/011-for-loop-dom-update/index.html"
        )));

        assert!(html.contains("Item 0"), "html after effects: {html}");
        assert!(html.contains("Item 1"), "html after effects: {html}");
        assert!(html.contains("Item 2"), "html after effects: {html}");

        let document = parse_html_document(
            include_str!("../../JustBarelyScript/UnitTest/011-for-loop-dom-update/index.html"),
            "file:///tmp/011-for-loop-dom-update/index.html",
        );

        assert!(find_canvas_text(&document.canvas_graph, "Item 0").is_some());
        assert!(find_canvas_text(&document.canvas_graph, "Item 1").is_some());
        assert!(find_canvas_text(&document.canvas_graph, "Item 2").is_some());
    }

    #[test]
    fn twelfth_script_test_click_listener_fires_and_updates_result() {
        let html =
            include_str!("../../JustBarelyScript/UnitTest/012-event-listener-click/index.html");
        let source = "file:///tmp/012-event-listener-click/index.html";

        // Initial render shows "Not clicked".
        let document = parse_html_document(html, source);
        assert!(find_canvas_text(&document.canvas_graph, "Not clicked").is_some());
        assert!(find_canvas_text(&document.canvas_graph, "Clicked").is_none());

        // Button has element_id="button" in the canvas graph.
        let button = document.canvas_graph.objects.iter().find_map(|o| {
            if let CanvasObject::Button(b) = o {
                Some(b)
            } else {
                None
            }
        });
        assert_eq!(
            button.map(|b| b.element_id.as_deref()),
            Some(Some("button"))
        );

        // Script state registers a click listener on "button".
        let mut state = build_script_state(html);
        assert!(state.has_listener("button", "click"));

        // Firing the click produces SetTextContent for "result".
        let effects = state.fire_event("button", "click", None);
        assert_eq!(
            effects,
            vec![justbarelyscript::BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "Clicked".to_owned(),
            }]
        );

        // Applying the effect to live_html and re-parsing shows "Clicked".
        let live_html = apply_safe_script_browser_effects(&remove_html_comments(html));
        let live_html = set_element_text_content_by_id(&live_html, "result", "Clicked");
        let updated = parse_html_document_from_live_html(&live_html, source, None);
        assert!(find_canvas_text(&updated.canvas_graph, "Clicked").is_some());
        assert!(find_canvas_text(&updated.canvas_graph, "Not clicked").is_none());
    }

    #[test]
    fn safe_script_effect_replaces_element_text_by_id() {
        let html = apply_safe_script_browser_effects(
            r#"<div id="result">Before</div><script>document.getElementById("result").textContent = "After";</script>"#,
        );

        assert!(html.contains(r#"<div id="result">After</div>"#));
    }

    #[test]
    fn ensure_bookmark_adds_default_without_duplicates() {
        let mut bookmarks = vec![Bookmark {
            title: "Docs".to_owned(),
            url: "https://example.com".to_owned(),
        }];

        assert!(ensure_bookmark(
            &mut bookmarks,
            Bookmark {
                title: DEFAULT_BOOKMARK_TITLE.to_owned(),
                url: "file:///tmp/sample.html".to_owned(),
            }
        ));
        assert!(!ensure_bookmark(
            &mut bookmarks,
            Bookmark {
                title: DEFAULT_BOOKMARK_TITLE.to_owned(),
                url: "file:///tmp/sample.html".to_owned(),
            }
        ));
        assert_eq!(bookmarks.len(), 2);
    }

    #[test]
    fn default_url_bookmark_is_seeded_separately() {
        let mut bookmarks = vec![default_sample_bookmark()];

        assert!(ensure_bookmark(&mut bookmarks, default_url_bookmark()));
        assert!(!ensure_bookmark(&mut bookmarks, default_url_bookmark()));
        assert!(bookmarks.iter().any(|bookmark| bookmark.url == DEFAULT_URL));
    }

    #[test]
    fn inline_elements_parse_to_styled_spans() {
        let spans = parse_inline_spans(
            r##"<a href="#!">link</a> <strong>strong</strong> <em>em</em>
            <u>under</u> <del>deleted</del> <ins>inserted</ins> <s>strike</s>
            H<sub>2</sub>O sup<sup>R</sup> <small>small</small>
            <code>code</code> <kbd>Cmd</kbd> <samp>out</samp>
            <mark>mark</mark> <var>x</var> <time>now</time>"##,
        );

        assert!(spans.iter().any(|span| span.href.as_deref() == Some("#!")));
        assert!(
            spans
                .iter()
                .any(|span| span.text == "strong" && span.strong)
        );
        assert!(spans.iter().any(|span| span.text == "em" && span.emphasis));
        assert!(
            spans
                .iter()
                .any(|span| span.text == "under" && span.underline)
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text == "deleted" && span.strikethrough)
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text == "inserted" && span.underline)
        );
        assert!(spans.iter().any(|span| span.text == "2" && span.lowered));
        assert!(spans.iter().any(|span| span.text == "R" && span.raised));
        assert!(spans.iter().any(|span| span.text == "small" && span.small));
        assert!(
            spans
                .iter()
                .any(|span| span.text.contains("code") && span.code)
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text.contains("Cmd") && span.code)
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text.contains("out") && span.code)
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text == "mark" && span.highlight)
        );
        assert!(spans.iter().any(|span| span.text == "x" && span.emphasis));
        assert!(spans.iter().any(|span| span.text.trim() == "now"));
    }

    #[test]
    fn inline_parser_preserves_boundaries_around_script_and_variable_spans() {
        let superscript = parse_inline_spans("Superscript<sup>®</sup>.");
        assert_eq!(superscript[0].text, "Superscript");
        assert!(superscript[1].raised);
        assert_eq!(superscript[1].text, "®");
        assert_eq!(superscript[2].text, ".");

        let subscript = parse_inline_spans("Subscript for things like H<sub>2</sub>O.");
        assert_eq!(subscript[0].text, "Subscript for things like H");
        assert!(subscript[1].lowered);
        assert_eq!(subscript[1].text, "2");
        assert_eq!(subscript[2].text, "O.");

        let variables = parse_inline_spans(
            "The <var>variable element</var>, such as <var>x</var> = <var>y</var>.",
        );
        assert_eq!(variables[0].text, "The ");
        assert!(variables[1].emphasis);
        assert_eq!(variables[1].text, "variable element");
        assert_eq!(variables[2].text, ", such as ");
        assert!(variables[3].emphasis);
        assert_eq!(variables[3].text, "x");
        assert_eq!(variables[4].text, " = ");
        assert!(variables[5].emphasis);
        assert_eq!(variables[5].text, "y");
        assert_eq!(variables[6].text, ".");
    }

    #[test]
    fn img_tags_load_local_image_blocks() {
        let html = r#"
            <html>
              <body>
                <img src="../V0/sample_docs/sample-image.jpg" alt="Sample image" width="64" height="32">
              </body>
            </html>
        "#;
        let source = path_to_file_url(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sample_pages/test_image_page.html"
        )));
        let document = parse_html_document(html, &source);

        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Image { alt, src, image }
                if alt == "Sample image"
                    && src.ends_with("/V0/sample_docs/sample-image.jpg")
                    && image.size == egui::vec2(64.0, 32.0)
                    && image.color_image.size[0] > 0
                    && image.color_image.size[1] > 0
        )));
    }

    #[test]
    fn image_handler_loads_local_avif_images() {
        let html = r#"
            <html>
              <body>
                <img src="test_2x2.avif" alt="AVIF sample" width="40" height="40">
              </body>
            </html>
        "#;
        let source = path_to_file_url(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sample_pages/avif_test_page.html"
        )));
        let document = parse_html_document(html, &source);

        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Image { alt, src, image }
                if alt == "AVIF sample"
                    && src.ends_with("/sample_pages/test_2x2.avif")
                    && image.size == egui::vec2(40.0, 40.0)
                    && image.color_image.size == [2, 2]
        )));
        assert!(document.canvas_graph.objects.iter().any(|object| matches!(
        object,
        CanvasObject::Image(image)
            if image.alt == "AVIF sample"
                && image.src.ends_with("/sample_pages/test_2x2.avif")
                && image.image.color_image.size == [2, 2]
        )));
    }

    #[test]
    fn linked_inline_image_inside_block_gets_used_position() {
        let html = r#"
            <html>
              <body>
                <p>Before image.</p>
                <figure>
                  <a href="https://example.test/image">
                    <img src="test_2x2.avif" alt="Linked image" width="40" height="40">
                  </a>
                  <figcaption>Caption</figcaption>
                </figure>
              </body>
            </html>
        "#;
        let source = path_to_file_url(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sample_pages/avif_test_page.html"
        )));
        let document = parse_html_document(html, &source);

        let image = document
            .canvas_graph
            .objects
            .iter()
            .find_map(|object| match object {
                CanvasObject::Image(image) if image.alt == "Linked image" => Some(image),
                _ => None,
            })
            .expect("expected linked image canvas object");

        assert_eq!(image.rect.size(), egui::vec2(40.0, 40.0));
        assert!(
            image.rect.left() > 0.0 && image.rect.top() > 0.0,
            "linked inline image should use its layout position, got {:?}",
            image.rect
        );
    }

    #[test]
    fn css_height_auto_preserves_image_aspect_ratio() {
        let html = r#"
            <html>
              <head><style>img { max-width: 100%; height: auto; display: block; }</style></head>
              <body>
                <img src="../V0/sample_docs/sample-image.jpg" alt="Sample image" width="64" height="32">
              </body>
            </html>
        "#;
        let source = path_to_file_url(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sample_pages/test_image_page.html"
        )));
        let document = parse_html_document(html, &source);

        let Some(image) = find_image_block(&document.blocks) else {
            panic!("expected decoded image block");
        };
        assert_eq!(image.size.x, 64.0);
        assert!((41.0..=42.0).contains(&image.size.y));
    }

    #[test]
    fn hidden_inputs_do_not_render_as_text_fields() {
        let html = r#"
            <html>
              <body>
                <input type="hidden" name="method" value="index">
                <input type="text" value="visible">
              </body>
            </html>
        "#;
        let document = parse_html_document(html, "https://example.test/");

        assert_eq!(count_inputs(&document.blocks), 1);
    }

    #[test]
    fn parser_skips_non_rendered_and_hidden_content() {
        let html = r#"
            <html>
              <body>
                <script>document.write("script text")</script>
                <style>.hidden { display: none; }</style>
                <template>template text</template>
                <div hidden>hidden attr text</div>
                <div style="display: none">display none text</div>
                <div aria-hidden="true">aria hidden text</div>
                <p>Visible text</p>
              </body>
            </html>
        "#;
        let document = parse_html_document(html, "https://example.test/");
        let rendered_text = format!("{:?}", document.blocks);

        assert!(rendered_text.contains("Visible text"));
        assert!(!rendered_text.contains("script text"));
        assert!(!rendered_text.contains("template text"));
        assert!(!rendered_text.contains("hidden attr text"));
        assert!(!rendered_text.contains("display none text"));
        assert!(!rendered_text.contains("aria hidden text"));
    }

    #[test]
    fn render_graph_dump_excludes_non_visual_metadata_text() {
        let html = r#"
            <html>
              <head>
                <style>p { color: rgb(10, 20, 30); }</style>
              </head>
              <body>
                <script>window.__NUXT__ = "large state payload";</script>
                <template>template payload</template>
                <noscript>noscript payload</noscript>
                <p>Visible text</p>
              </body>
            </html>
        "#;
        let dump = parse_render_graph_debug_dump(html, "https://example.test/");

        assert!(dump.contains("Visible text"));
        assert!(!dump.contains("window.__NUXT__"));
        assert!(!dump.contains("template payload"));
        assert!(!dump.contains("noscript payload"));
    }

    #[test]
    fn live_js_debug_report_marks_budget_exhaustion() {
        let html = r#"
            <html>
              <body>
                <script>while (true) { var x = 1; }</script>
              </body>
            </html>
        "#;
        let report = live_js_debug_report(html, Some("https://example.test/"));

        assert!(report.contains("Live JS Debug"));
        assert!(report.contains("status: executed"));
        assert!(report.contains("statement budget exhausted"));
        assert!(report.contains("budget_exhausted: 1"));
    }

    #[test]
    fn page_load_continues_when_script_budget_is_exhausted() {
        let html = r#"
            <html>
              <body>
                <p>Still visible</p>
                <script>while (true) { var x = 1; }</script>
              </body>
            </html>
        "#;
        let document = parse_html_document(html, "https://example.test/");
        let messages =
            script_console_messages_from_html_with_source(html, Some("https://example.test/"));

        assert!(find_canvas_text(&document.canvas_graph, "Still visible").is_some());
        assert!(messages.iter().any(|message| {
            message.level == justbarelyscript::ConsoleLevel::Error
                && message.text.contains("execution stopped")
        }));
    }

    #[test]
    fn hydration_fallback_reveals_hidden_ssr_article_content() {
        let dir = std::env::temp_dir().join(format!(
            "almostthere-hydration-fallback-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp script dir");
        fs::write(dir.join("hydrate.js"), "function () {").expect("write hydration script");

        let html = r#"
            <style>
              .ssr-card { visibility: hidden; }
            </style>
            <main>
              <div class="ssr-card">
                <article>
                  <a href="https://example.test/result">Server rendered result title</a>
                  <p>Server rendered result body that should remain available without hydration.</p>
                </article>
              </div>
            </main>
            <script src="hydrate.js" type="module"></script>
        "#;
        let source = path_to_file_url(&dir.join("index.html"));
        let document = parse_html_document(html, &source);

        assert!(document.canvas_graph.objects.iter().any(|object| matches!(
            object,
            CanvasObject::Text(text) if text.text.contains("Server rendered result title")
        )));
    }

    #[test]
    fn hydration_fallback_does_not_reveal_absolute_hidden_menus() {
        let dir = std::env::temp_dir().join(format!(
            "almostthere-hydration-menu-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp script dir");
        fs::write(dir.join("hydrate.js"), "function () {").expect("write hydration script");

        let html = r#"
            <style>
              .menu { visibility: hidden; position: absolute; }
            </style>
            <nav class="menu">
              <a href="/settings">Hidden menu link with enough text to look meaningful</a>
            </nav>
            <p>Visible page text</p>
            <script src="hydrate.js" type="module"></script>
        "#;
        let source = path_to_file_url(&dir.join("index.html"));
        let document = parse_html_document(html, &source);

        assert!(find_canvas_text(&document.canvas_graph, "Visible page text").is_some());
        assert!(!document.canvas_graph.objects.iter().any(|object| matches!(
            object,
            CanvasObject::Text(text)
                if text.text.contains("Hidden menu link with enough text to look meaningful")
        )));
    }

    #[test]
    fn attributes_parse_single_quoted_unquoted_and_boolean_forms() {
        assert_eq!(
            extract_attr("<a href='/local'>", "href").as_deref(),
            Some("/local")
        );
        assert_eq!(
            extract_attr("<input type=hidden value=index>", "value").as_deref(),
            Some("index")
        );
        assert!(has_attr("<div hidden>", "hidden"));
    }

    #[test]
    fn local_documents_do_not_fetch_remote_subresources() {
        let html = r#"
            <html>
              <head>
                <link rel="stylesheet" href="https://www.ecosia.org/remote.css">
              </head>
              <body>
                <img src="https://www.ecosia.org/image.jpg" alt="remote image">
              </body>
            </html>
        "#;
        let source = path_to_file_url(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sample_pages/offline_debug.html"
        )));
        let document = parse_html_document(html, &source);

        assert_eq!(
            document.style.body_font_size,
            rich_canvas::BrowserStyle::default().body_font_size
        );
        assert!(contains_block(&document.blocks, |block| matches!(
            block,
            CanvasBlock::Media { label } if label == "remote image"
        )));
    }

    #[test]
    fn generic_inline_wrappers_render_text_without_page_specific_rules() {
        let html = r#"
            <html>
              <body>
                <div><span>Plain span text</span><mark>marked</mark><time>now</time></div>
              </body>
            </html>
        "#;
        let document = parse_html_document(html, "https://example.test/");
        let rendered_text = format!("{:?}", document.blocks);

        assert!(rendered_text.contains("Plain span text"));
        assert!(rendered_text.contains("marked"));
        assert!(rendered_text.contains("now"));
    }

    #[test]
    fn inline_svg_circle_parses_to_svg_block() {
        let html = r##"
            <html>
              <body>
                <svg width="100px" height="100px">
                  <circle cx="100" cy="100" r="100" fill="#1fa3ec"></circle>
                </svg>
              </body>
            </html>
        "##;
        let document = parse_html_document(html, "https://latex.vercel.app/elements");

        let Some(svg) = find_svg_block(&document.blocks) else {
            panic!("expected parsed SVG block");
        };
        assert_eq!(svg.size, egui::vec2(100.0, 100.0));
        assert_eq!(svg.shapes.len(), 1);
    }

    #[test]
    fn file_url_input_decodes_spaces_for_local_paths() {
        assert_eq!(
            input_to_path("file:///tmp/AlmostThere%20Browser/page.html"),
            PathBuf::from("/tmp/AlmostThere Browser/page.html")
        );
    }

    fn contains_block(
        blocks: &[CanvasBlock],
        predicate: impl Copy + Fn(&CanvasBlock) -> bool,
    ) -> bool {
        blocks.iter().any(|block| {
            predicate(block)
                || matches!(
                    block,
                    CanvasBlock::Panel { children } if contains_block(children, predicate)
                )
                || matches!(
                    block,
                    CanvasBlock::Box { children, .. } if contains_block(children, predicate)
                )
                || matches!(
                    block,
                    CanvasBlock::StyledBox { children, .. } if contains_block(children, predicate)
                )
        })
    }

    fn find_image_block(blocks: &[CanvasBlock]) -> Option<&ImageBlock> {
        blocks.iter().find_map(|block| match block {
            CanvasBlock::Image { image, .. } => Some(image),
            CanvasBlock::Panel { children }
            | CanvasBlock::Box { children, .. }
            | CanvasBlock::StyledBox { children, .. } => find_image_block(children),
            _ => None,
        })
    }

    fn find_svg_block(blocks: &[CanvasBlock]) -> Option<&SvgBlock> {
        blocks.iter().find_map(|block| match block {
            CanvasBlock::Svg { svg } => Some(svg),
            CanvasBlock::Panel { children }
            | CanvasBlock::Box { children, .. }
            | CanvasBlock::StyledBox { children, .. } => find_svg_block(children),
            _ => None,
        })
    }

    fn find_canvas_text<'a>(graph: &'a CanvasGraph, value: &str) -> Option<&'a CanvasTextObject> {
        graph.objects.iter().find_map(|object| match object {
            CanvasObject::Text(text) if text.text == value => Some(text),
            _ => None,
        })
    }

    fn find_canvas_input<'a>(graph: &'a CanvasGraph, label: &str) -> Option<&'a CanvasInputObject> {
        graph.objects.iter().find_map(|object| match object {
            CanvasObject::Input(input) if input.label == label => Some(input),
            _ => None,
        })
    }

    fn count_inputs(blocks: &[CanvasBlock]) -> usize {
        blocks
            .iter()
            .map(|block| match block {
                CanvasBlock::Input { .. } => 1,
                CanvasBlock::Panel { children }
                | CanvasBlock::Box { children, .. }
                | CanvasBlock::StyledBox { children, .. } => count_inputs(children),
                _ => 0,
            })
            .sum()
    }

    fn collect_list_item_text(blocks: &[CanvasBlock], out: &mut Vec<String>) {
        for block in blocks {
            match block {
                CanvasBlock::ListItem { text, .. } => out.push(text.clone()),
                CanvasBlock::Panel { children }
                | CanvasBlock::Box { children, .. }
                | CanvasBlock::StyledBox { children, .. } => collect_list_item_text(children, out),
                _ => {}
            }
        }
    }

    fn find_render_text<'a>(node: &'a RenderNode, text: &str) -> Option<&'a RenderNode> {
        if let RenderNodeKind::Text(value) = &node.kind {
            if normalize_ws(value) == text {
                return Some(node);
            }
        }
        node.children
            .iter()
            .find_map(|child| find_render_text(child, text))
    }

    fn find_render_element_by_id<'a>(node: &'a RenderNode, id: &str) -> Option<&'a RenderNode> {
        if let RenderNodeKind::Element(element) = &node.kind {
            if element.attr("id") == Some(id) {
                return Some(node);
            }
        }
        node.children
            .iter()
            .find_map(|child| find_render_element_by_id(child, id))
    }

    fn find_render_element_by_class<'a>(
        node: &'a RenderNode,
        class_name: &str,
    ) -> Option<&'a RenderNode> {
        if let RenderNodeKind::Element(element) = &node.kind {
            if element
                .attr("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == class_name))
            {
                return Some(node);
            }
        }
        node.children
            .iter()
            .find_map(|child| find_render_element_by_class(child, class_name))
    }
}
