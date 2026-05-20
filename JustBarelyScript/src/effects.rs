use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    Program,
    ast::{
        BinaryOperator, Binding, BlockStatement, Expression, FunctionBody, MemberProperty,
        ObjectProperty, Param, Statement, SwitchStatement, UnaryOperator, VarKind,
        VariableDeclaration,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserEffect {
    SetTextContent {
        element_id: String,
        value: String,
    },
    SetAttribute {
        element_id: String,
        name: String,
        value: String,
    },
    SetInnerHtml {
        element_id: String,
        value: String,
    },
    AppendChild {
        parent_id: String,
        child: DomElementSnapshot,
    },
    ConsoleLog {
        level: String,
        text: String,
    },
    NetworkRequest {
        method: String,
        url: String,
        body: String,
    },
    RuntimeTrace {
        kind: String,
        detail: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomElementSnapshot {
    pub tag_name: String,
    pub text_content: String,
    pub inner_html: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<DomElementSnapshot>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomExecutionState {
    pub text_content_by_id: HashMap<String, String>,
    pub inner_html_by_id: HashMap<String, String>,
    pub attributes_by_id: HashMap<String, HashMap<String, String>>,
    pub computed_styles_by_id: HashMap<String, HashMap<String, String>>,
    query_selector_all_by_class: HashMap<String, Vec<String>>,
    query_selector_by_id: HashMap<String, String>,
    query_selector_by_class: HashMap<String, String>,
    created_elements: HashMap<String, DomElementSnapshot>,
    next_created_id: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<Param>,
    pub body: FunctionBody,
    pub captured: Vec<StackFrame>,
    properties: HashMap<String, JsValue>,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingTimer {
    fires_at_ms: u64,
    params: Vec<String>,
    body: crate::ast::BlockStatement,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingMicrotask {
    params: Vec<String>,
    body: crate::ast::BlockStatement,
}

#[derive(Clone, Debug, PartialEq)]
enum EarlyExit {
    Return(JsValue),
    Throw(JsValue),
    Break,
    Continue,
}

#[derive(Clone, Debug, Default)]
pub struct BrowserExecutionState {
    pub dom: DomExecutionState,
    globals: HashMap<String, JsValue>,
    local_storage: HashMap<String, String>,
    session_storage: HashMap<String, String>,
    fingerprint_suite: Option<crate::specs_placeholder::FingerprintSuite>,
    stack: Vec<StackFrame>,
    effects: Vec<BrowserEffect>,
    event_handlers: Vec<EventHandler>,
    pending_timers: Vec<PendingTimer>,
    pending_microtasks: Vec<PendingMicrotask>,
    pub current_time_ms: u64,
    early_exit: Option<EarlyExit>,
    execution_budget_remaining: Option<usize>,
    execution_budget_exhausted: bool,
    array_method_overrides: HashMap<String, JsValue>,
    symbol_counter: u32,
}

#[derive(Clone, Debug)]
struct StackFrame {
    locals: Rc<RefCell<HashMap<String, JsValue>>>,
    is_function_scope: bool,
}

impl Default for StackFrame {
    fn default() -> Self {
        StackFrame {
            locals: Rc::new(RefCell::new(HashMap::new())),
            is_function_scope: false,
        }
    }
}

impl StackFrame {
    fn function_scope() -> Self {
        StackFrame {
            locals: Rc::new(RefCell::new(HashMap::new())),
            is_function_scope: true,
        }
    }
}

impl PartialEq for StackFrame {
    fn eq(&self, other: &Self) -> bool {
        *self.locals.borrow() == *other.locals.borrow()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EventHandler {
    element_id: String,
    event_type: String,
    params: Vec<String>,
    body: BlockStatement,
    captured: Vec<StackFrame>,
}

#[derive(Clone, Debug, PartialEq)]
enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Object(HashMap<String, JsValue>),
    Array(Vec<JsValue>),
    Function(JsFunction),
    ElementRef(String),
    NodeList(Vec<String>),
    StyleRef(String),
    StorageRef(StorageKind),
    DocumentRef,
    WindowRef,
    NavigatorRef,
    HostFunction(String),
    BoundHostFunction {
        name: String,
        this_arg: Box<JsValue>,
        bound_args: Vec<JsValue>,
    },
    HostObject(String),
    RegExp {
        pattern: String,
        flags: String,
    },
    CanvasContextRef(String),
    DateInstance,
    ResolvedPromise,
    XhrInstance {
        method: String,
        url: String,
        headers: HashMap<String, String>,
    },
    Proxy {
        target: Box<JsValue>,
        get: Option<JsFunction>,
    },
    WeakMap(HashMap<String, JsValue>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StorageKind {
    Local,
    Session,
}

pub fn collect_browser_effects(program: &Program) -> Vec<BrowserEffect> {
    let mut state = BrowserExecutionState::default();
    state.execute_program(program);
    state.drain_effects()
}

impl BrowserExecutionState {
    pub fn set_execution_budget(&mut self, statement_budget: usize) {
        self.execution_budget_remaining = Some(statement_budget);
        self.execution_budget_exhausted = false;
    }

    pub fn execution_budget_exhausted(&self) -> bool {
        self.execution_budget_exhausted
    }

    pub fn pending_timer_count(&self) -> usize {
        self.pending_timers.len()
    }

    pub fn listener_count(&self) -> usize {
        self.event_handlers.len()
    }

    pub fn seed_existing_element(
        &mut self,
        id: &str,
        text_content: String,
        attributes: HashMap<String, String>,
    ) {
        self.dom
            .text_content_by_id
            .insert(id.to_owned(), text_content);
        self.dom
            .query_selector_by_id
            .entry(id.to_owned())
            .or_insert_with(|| id.to_owned());
        if let Some(classes) = attributes.get("class") {
            for class_name in classes.split_ascii_whitespace() {
                self.dom
                    .query_selector_by_class
                    .entry(class_name.to_owned())
                    .or_insert_with(|| id.to_owned());
                self.dom
                    .query_selector_all_by_class
                    .entry(class_name.to_owned())
                    .or_default()
                    .push(id.to_owned());
            }
        }
        self.dom.attributes_by_id.insert(id.to_owned(), attributes);
    }

    pub fn seed_computed_style(&mut self, id: &str, properties: HashMap<String, String>) {
        self.dom
            .computed_styles_by_id
            .insert(id.to_owned(), properties);
    }

    /// Seed the global `navigator` object so that scripts can read
    /// `navigator.platform`, `navigator.languages`, etc.
    pub fn seed_navigator(&mut self, info: &crate::navigator::NavigatorInfo) {
        let mut obj: HashMap<String, JsValue> = HashMap::new();

        obj.insert("platform".into(), JsValue::String(info.platform.clone()));
        obj.insert("userAgent".into(), JsValue::String(info.user_agent.clone()));
        obj.insert(
            "appVersion".into(),
            JsValue::String(info.app_version.clone()),
        );
        obj.insert("appName".into(), JsValue::String(info.app_name.into()));
        obj.insert(
            "appCodeName".into(),
            JsValue::String(info.app_code_name.into()),
        );
        obj.insert("product".into(), JsValue::String(info.product.into()));
        obj.insert(
            "productSub".into(),
            JsValue::String(info.product_sub.into()),
        );
        obj.insert("vendor".into(), JsValue::String(info.vendor.into()));
        obj.insert("vendorSub".into(), JsValue::String(info.vendor_sub.into()));
        obj.insert(
            "hardwareConcurrency".into(),
            JsValue::Number(info.hardware_concurrency as f64),
        );
        obj.insert(
            "maxTouchPoints".into(),
            JsValue::Number(info.max_touch_points as f64),
        );
        obj.insert(
            "cookieEnabled".into(),
            JsValue::Boolean(info.cookie_enabled),
        );
        obj.insert(
            "doNotTrack".into(),
            match info.do_not_track {
                Some(true) => JsValue::String("1".into()),
                Some(false) => JsValue::String("0".into()),
                None => JsValue::String("unspecified".into()),
            },
        );

        // languages array
        let langs: Vec<JsValue> = info
            .languages
            .iter()
            .map(|l| JsValue::String(l.clone()))
            .collect();
        obj.insert("languages".into(), JsValue::Array(langs));
        // language (first entry, or empty string)
        obj.insert(
            "language".into(),
            JsValue::String(info.languages.first().cloned().unwrap_or_default()),
        );

        // Firefox-only; undefined in Chrome — we expose as undefined when absent
        if let Some(ref oscpu) = info.oscpu {
            obj.insert("oscpu".into(), JsValue::String(oscpu.clone()));
        }
        // IE-only
        if let Some(ref cpu) = info.cpu_class {
            obj.insert("cpuClass".into(), JsValue::String(cpu.clone()));
        }
        // Firefox-only buildID
        if let Some(ref bid) = info.build_id {
            obj.insert("buildID".into(), JsValue::String(bid.clone()));
        }

        // Stub out plugin-related properties as empty arrays / zero
        obj.insert("plugins".into(), JsValue::Array(vec![]));
        obj.insert("mimeTypes".into(), JsValue::Array(vec![]));

        self.globals
            .insert("navigator".into(), JsValue::NavigatorRef);
        self.globals
            .insert("__navigatorData".into(), JsValue::Object(obj));
    }

    /// Seed the global `screen` object so scripts can read
    /// `screen.width`, `screen.height`, `screen.colorDepth`, etc.
    pub fn seed_screen(&mut self, info: &crate::screen::ScreenInfo) {
        let mut obj: HashMap<String, JsValue> = HashMap::new();
        obj.insert("width".into(), JsValue::Number(info.width as f64));
        obj.insert("height".into(), JsValue::Number(info.height as f64));
        obj.insert(
            "colorDepth".into(),
            JsValue::Number(info.color_depth as f64),
        );
        obj.insert(
            "pixelDepth".into(),
            JsValue::Number(info.pixel_depth() as f64),
        );
        obj.insert(
            "availWidth".into(),
            JsValue::Number(info.avail_width as f64),
        );
        obj.insert(
            "availHeight".into(),
            JsValue::Number(info.avail_height as f64),
        );
        self.globals.insert("screen".into(), JsValue::Object(obj));
    }

    /// Seed browser globals that are independent of OS detection.
    pub fn seed_browser_basics(&mut self) {
        self.globals.insert(
            "localStorage".into(),
            JsValue::StorageRef(StorageKind::Local),
        );
        self.globals.insert(
            "sessionStorage".into(),
            JsValue::StorageRef(StorageKind::Session),
        );
        self.globals.insert("document".into(), JsValue::DocumentRef);
        self.globals.insert("window".into(), JsValue::WindowRef);
        self.globals.insert("globalThis".into(), JsValue::WindowRef);
        self.globals.insert(
            "ActiveXObject".into(),
            JsValue::HostFunction("ActiveXObject".into()),
        );
        self.globals.insert(
            "Symbol".into(),
            JsValue::HostFunction("Symbol".into()),
        );
        let mut perf = HashMap::new();
        perf.insert(
            "now".to_owned(),
            JsValue::HostFunction("performance.now".into()),
        );
        self.globals.insert("performance".into(), JsValue::Object(perf));
    }

    /// Seed the precomputed browser fingerprint suite into JS-facing APIs.
    pub fn seed_fingerprint_suite(&mut self, suite: crate::specs_placeholder::FingerprintSuite) {
        if suite.storage.local_storage {
            self.globals.insert(
                "localStorage".into(),
                JsValue::StorageRef(StorageKind::Local),
            );
        }
        if suite.storage.session_storage {
            self.globals.insert(
                "sessionStorage".into(),
                JsValue::StorageRef(StorageKind::Session),
            );
        }
        let suite_object = Self::fingerprint_suite_js_object(&suite);
        self.globals
            .insert("__almostthereFingerprint".into(), suite_object.clone());
        self.globals
            .insert("almostthereFingerprint".into(), suite_object);
        self.fingerprint_suite = Some(suite);
    }

    fn fingerprint_suite_js_object(suite: &crate::specs_placeholder::FingerprintSuite) -> JsValue {
        let mut obj = HashMap::new();
        obj.insert(
            "canvas".into(),
            JsValue::String(suite.canvas.data_url.clone()),
        );
        obj.insert(
            "webGLVendor".into(),
            JsValue::String(suite.webgl.vendor.clone()),
        );
        obj.insert(
            "webGLRenderer".into(),
            JsValue::String(suite.webgl.renderer.clone()),
        );
        obj.insert(
            "webGLData".into(),
            JsValue::String(
                suite
                    .webgl
                    .parameters
                    .iter()
                    .map(|(key, value)| format!("{key}:{value}"))
                    .collect::<Vec<_>>()
                    .join(";"),
            ),
        );
        obj.insert(
            "audio".into(),
            JsValue::String(Self::audio_fingerprint_string(&suite.audio)),
        );
        obj.insert(
            "fontsEnum".into(),
            JsValue::String(suite.fonts.as_amiunique_string()),
        );
        obj.insert(
            "touchSupport".into(),
            JsValue::String(suite.touch.as_amiunique_string()),
        );
        obj.insert(
            "overwrittenObjects".into(),
            JsValue::String(format!(
                "screen.width={};canvas.toDataURL={};Date.getTimezoneOffset={}",
                suite.overwrite.screen_width_getter,
                suite.overwrite.canvas_to_data_url,
                suite.overwrite.date_get_timezone_offset
            )),
        );
        obj.insert(
            "navigatorPrototype".into(),
            JsValue::String(suite.nav_prototype.properties.join(";")),
        );
        obj.insert(
            "mathsConstants".into(),
            JsValue::String(Self::math_constants_string(&suite.math)),
        );
        obj.insert(
            "errorsGenerated".into(),
            JsValue::String(Self::error_shape_string(&suite.errors)),
        );
        obj.insert(
            "resOverflow".into(),
            JsValue::String(format!(
                "{};{};{}",
                suite.stack.depth, suite.stack.error_name, suite.stack.error_message
            )),
        );
        obj.insert(
            "modernizr".into(),
            JsValue::String(suite.modernizr.as_amiunique_string()),
        );
        obj.insert(
            "osMediaqueries".into(),
            JsValue::String(suite.os_queries.as_amiunique_string()),
        );
        obj.insert(
            "unknownImageError".into(),
            JsValue::String(suite.unknown_image.as_amiunique_string()),
        );
        obj.insert(
            "timezone".into(),
            JsValue::Number(suite.timezone.offset_minutes as f64),
        );
        obj.insert(
            "timezoneName".into(),
            suite
                .timezone
                .iana_name
                .clone()
                .map(JsValue::String)
                .unwrap_or(JsValue::Null),
        );
        obj.insert(
            "localStorage".into(),
            JsValue::Boolean(suite.storage.local_storage),
        );
        obj.insert(
            "sessionStorage".into(),
            JsValue::Boolean(suite.storage.session_storage),
        );
        obj.insert("adBlock".into(), JsValue::Boolean(suite.adblock));
        JsValue::Object(obj)
    }

    fn audio_fingerprint_string(audio: &crate::specs_placeholder::AudioFingerprint) -> String {
        let bins = audio
            .cc_bins
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "pxi={};nt_vc={};cc={};hybrid={};supported={}",
            audio.pxi_sum, audio.nt_vc_props, bins, audio.hybrid_sum, audio.is_supported
        )
    }

    fn math_constants_string(math: &crate::specs_placeholder::MathConstants) -> String {
        format!(
            "asinh(1)={};acosh(1e300)={};atanh(0.5)={};expm1(1)={};cbrt(100)={};log1p(10)={};sinh(1)={};cosh(10)={};tanh(1)={}",
            math.asinh_1,
            math.acosh_1e300,
            math.atanh_half,
            math.expm1_1,
            math.cbrt_100,
            math.log1p_10,
            math.sinh_1,
            math.cosh_10,
            math.tanh_1
        )
    }

    fn error_shape_string(errors: &crate::specs_placeholder::ErrorShapeInfo) -> String {
        format!(
            "{};{};{};{};{};{};{};{};{}",
            errors.ref_name,
            errors.ref_message,
            errors.ref_file_name.clone().unwrap_or_default(),
            errors
                .ref_line_number
                .map(|value| value.to_string())
                .unwrap_or_default(),
            errors.ref_description.clone().unwrap_or_default(),
            errors.ref_to_source.clone().unwrap_or_default(),
            errors.ws_name,
            errors.ws_message,
            "chrome-like"
        )
    }

    /// Seed a minimal `location` object for scripts that inspect the current URL.
    pub fn seed_location(&mut self, href: &str) {
        let mut obj: HashMap<String, JsValue> = HashMap::new();
        obj.insert("href".into(), JsValue::String(href.to_owned()));

        if let Some((protocol, rest)) = href.split_once("://") {
            obj.insert("protocol".into(), JsValue::String(format!("{protocol}:")));
            let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let host = &rest[..host_end];
            obj.insert("host".into(), JsValue::String(host.to_owned()));
            let hostname = host.split(':').next().unwrap_or("").to_owned();
            let port = host
                .split(':')
                .nth(1)
                .unwrap_or("")
                .to_owned();
            obj.insert("hostname".into(), JsValue::String(hostname.clone()));
            obj.insert("port".into(), JsValue::String(port));
            let after_host = &rest[host_end..];
            let (path_part, rest_after_path) = after_host
                .split_once('?')
                .map(|(p, r)| (p, format!("?{r}")))
                .unwrap_or_else(|| {
                    after_host
                        .split_once('#')
                        .map(|(p, r)| (p, format!("#{r}")))
                        .unwrap_or((after_host, String::new()))
                });
            let pathname = if path_part.is_empty() {
                "/".to_owned()
            } else {
                path_part.to_owned()
            };
            obj.insert("pathname".into(), JsValue::String(pathname));
            let (search_part, hash_part) = if let Some(q_rest) = rest_after_path.strip_prefix('?') {
                let (s, h) = q_rest
                    .split_once('#')
                    .map(|(s, h)| (format!("?{s}"), format!("#{h}")))
                    .unwrap_or_else(|| (format!("?{q_rest}"), String::new()));
                (s, h)
            } else if let Some(h_rest) = rest_after_path.strip_prefix('#') {
                (String::new(), format!("#{h_rest}"))
            } else {
                (String::new(), String::new())
            };
            obj.insert("search".into(), JsValue::String(search_part));
            obj.insert("hash".into(), JsValue::String(hash_part));
            obj.insert(
                "origin".into(),
                JsValue::String(format!("{protocol}://{hostname}")),
            );
        } else {
            obj.insert("protocol".into(), JsValue::String(String::new()));
            obj.insert("host".into(), JsValue::String(String::new()));
            obj.insert("hostname".into(), JsValue::String(String::new()));
            obj.insert("pathname".into(), JsValue::String(href.to_owned()));
            obj.insert("port".into(), JsValue::String(String::new()));
            obj.insert("search".into(), JsValue::String(String::new()));
            obj.insert("hash".into(), JsValue::String(String::new()));
            obj.insert("origin".into(), JsValue::String("null".to_owned()));
        }

        self.globals.insert("location".into(), JsValue::Object(obj.clone()));
        // Mirror as window.location for scripts that read window.location.*
        self.globals.insert("__location__".into(), JsValue::Object(obj));
    }

    pub fn execute_program(&mut self, program: &Program) {
        self.ensure_global_frame();
        self.hoist_function_declarations(&program.body);
        for statement in &program.body {
            self.execute_statement(statement);
            if self.early_exit.is_some() {
                break;
            }
        }
        self.drain_and_run_microtasks();
    }

    fn drain_and_run_microtasks(&mut self) {
        while !self.pending_microtasks.is_empty() {
            let task = self.pending_microtasks.remove(0);
            self.stack.push(StackFrame::function_scope());
            self.execute_block(&task.body);
            self.stack.pop();
            self.ensure_global_frame();
        }
    }

    pub fn drain_effects(&mut self) -> Vec<BrowserEffect> {
        self.effects.drain(..).collect()
    }

    fn trace_runtime(&mut self, kind: &str, detail: impl Into<String>) {
        self.effects.push(BrowserEffect::RuntimeTrace {
            kind: kind.to_owned(),
            detail: detail.into(),
        });
    }

    fn trace_member_read(
        &mut self,
        object: &Expression,
        property: &str,
        receiver: &JsValue,
        result: &JsValue,
        prototype_attempted: bool,
    ) {
        let receiver_missing = matches!(receiver, JsValue::Undefined | JsValue::Null);
        if receiver_missing {
            self.trace_runtime(
                "member.receiver.warning",
                format!(
                    "property={} receiver_tag={} result_tag={} prototype_attempted={} object={:?}",
                    property,
                    Self::object_tag(receiver),
                    Self::object_tag(result),
                    prototype_attempted,
                    object
                ),
            );
        } else if matches!(result, JsValue::Undefined) && Self::diagnostic_member_property(property)
        {
            self.trace_runtime(
                "member.read.undefined",
                format!(
                    "property={} receiver_tag={} result_tag={} prototype_attempted={} object={:?}",
                    property,
                    Self::object_tag(receiver),
                    Self::object_tag(result),
                    prototype_attempted,
                    object
                ),
            );
        }
    }

    fn emit_network_request(&mut self, method: &str, url: String, body: String) {
        self.effects.push(BrowserEffect::RuntimeTrace {
            kind: "network.request".to_owned(),
            detail: format!("{} {} body_bytes={}", method, url, body.len()),
        });
        self.effects.push(BrowserEffect::NetworkRequest {
            method: method.to_owned(),
            url,
            body,
        });
    }

    fn execute_statement(&mut self, statement: &Statement) {
        if !self.consume_execution_budget() {
            return;
        }
        match statement {
            Statement::VariableDeclaration(declaration) => {
                self.execute_variable_declaration(declaration)
            }
            Statement::Expression(expression) => {
                self.execute_expression(expression);
            }
            Statement::Block(block) => self.execute_block(block),
            Statement::If(statement) => {
                let condition = self.execute_expression(&statement.test);
                if Self::is_truthy(&condition) {
                    self.execute_statement(&statement.consequent);
                } else if let Some(alternate) = &statement.alternate {
                    self.execute_statement(alternate);
                }
            }
            Statement::While(statement) => {
                let statement = statement.clone();
                loop {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    let condition = self.execute_expression(&statement.test);
                    if !Self::is_truthy(&condition) {
                        break;
                    }
                    self.execute_statement(&statement.body);
                    match self.early_exit {
                        Some(EarlyExit::Break) => {
                            self.early_exit = None;
                            break;
                        }
                        Some(EarlyExit::Continue) => {
                            self.early_exit = None;
                        }
                        Some(_) => break,
                        None => {}
                    }
                }
            }
            Statement::DoWhile(statement) => {
                let statement = statement.clone();
                loop {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    self.execute_statement(&statement.body);
                    match self.early_exit {
                        Some(EarlyExit::Break) => {
                            self.early_exit = None;
                            break;
                        }
                        Some(EarlyExit::Continue) => {
                            self.early_exit = None;
                        }
                        Some(_) => break,
                        None => {}
                    }
                    let condition = self.execute_expression(&statement.test);
                    if !Self::is_truthy(&condition) {
                        break;
                    }
                }
            }
            Statement::For(statement) => {
                let statement = statement.clone();
                self.stack.push(StackFrame::default());
                if let Some(init) = &statement.init {
                    self.execute_statement(init);
                }
                loop {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    if let Some(test) = &statement.test {
                        let cond = self.execute_expression(test);
                        if !Self::is_truthy(&cond) {
                            break;
                        }
                    }
                    self.execute_statement(&statement.body);
                    match self.early_exit {
                        Some(EarlyExit::Break) => {
                            self.early_exit = None;
                            break;
                        }
                        Some(EarlyExit::Continue) => {
                            self.early_exit = None;
                        }
                        Some(_) => break,
                        None => {}
                    }
                    if let Some(update) = &statement.update {
                        self.execute_expression(update);
                    }
                }
                self.stack.pop();
                self.ensure_global_frame();
            }
            Statement::FunctionDeclaration(decl) => {
                let func = JsFunction {
                    name: Some(decl.name.clone()),
                    params: decl.params.clone(),
                    body: FunctionBody::Block(decl.body.clone()),
                    captured: self.stack.clone(),
                    properties: HashMap::new(),
                };
                let name = decl.name.clone();
                self.set_local(&name, JsValue::Function(func.clone()));
                // If this was previously hoisted (empty-closure version stored in an
                // override), refresh those overrides with the now-complete closure.
                self.refresh_overrides_for_named_func(&name, JsValue::Function(func));
            }
            Statement::ClassDeclaration(_) => {}
            Statement::Return(stmt) => {
                let value = stmt
                    .argument
                    .as_ref()
                    .map(|e| self.execute_expression(e))
                    .unwrap_or(JsValue::Undefined);
                self.early_exit = Some(EarlyExit::Return(value));
            }
            Statement::Throw(stmt) => {
                let value = self.execute_expression(&stmt.argument.clone());
                self.early_exit = Some(EarlyExit::Throw(value));
            }
            Statement::Break(_) => {
                self.early_exit = Some(EarlyExit::Break);
            }
            Statement::Continue(_) => {
                self.early_exit = Some(EarlyExit::Continue);
            }
            Statement::TryCatch(tc) => {
                let tc = tc.clone();
                // try body
                self.stack.push(StackFrame::default());
                for stmt in &tc.body.body {
                    self.execute_statement(stmt);
                    if self.early_exit.is_some() {
                        break;
                    }
                }
                self.stack.pop();
                self.ensure_global_frame();
                // catch
                if let Some(EarlyExit::Throw(err_val)) = self.early_exit.take() {
                    if let Some(catch_body) = &tc.catch_body.clone() {
                        self.stack.push(StackFrame::default());
                        if let Some(param) = &tc.catch_param {
                            self.set_local(param, err_val);
                        }
                        for stmt in &catch_body.body {
                            self.execute_statement(stmt);
                            if self.early_exit.is_some() {
                                break;
                            }
                        }
                        self.stack.pop();
                        self.ensure_global_frame();
                    }
                }
                // finally — always runs; preserves outer early_exit if finally doesn't set one
                if let Some(finally_body) = tc.finally_body.clone() {
                    let saved = self.early_exit.take();
                    self.stack.push(StackFrame::default());
                    for stmt in &finally_body.body {
                        self.execute_statement(stmt);
                        if self.early_exit.is_some() {
                            break;
                        }
                    }
                    self.stack.pop();
                    self.ensure_global_frame();
                    if self.early_exit.is_none() {
                        self.early_exit = saved;
                    }
                }
            }
            Statement::ForOf(stmt) => {
                let stmt = stmt.clone();
                let iterable = self.execute_expression(&stmt.iterable);
                let items: Vec<JsValue> = match iterable {
                    JsValue::Array(arr) => arr,
                    JsValue::String(s) => {
                        s.chars().map(|c| JsValue::String(c.to_string())).collect()
                    }
                    JsValue::NodeList(ids) => ids
                        .into_iter()
                        .map(|id| JsValue::ElementRef(existing_element_ref(&id)))
                        .collect(),
                    _ => vec![],
                };
                let for_of_is_var = stmt.binding_kind == VarKind::Var;
                for item in items {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    self.stack.push(StackFrame::default());
                    if for_of_is_var {
                        self.execute_var_binding(&stmt.binding, item);
                    } else {
                        self.execute_binding(&stmt.binding, item);
                    }
                    self.execute_statement(&stmt.body);
                    self.stack.pop();
                    self.ensure_global_frame();
                    match self.early_exit {
                        Some(EarlyExit::Break) => {
                            self.early_exit = None;
                            break;
                        }
                        Some(EarlyExit::Continue) => {
                            self.early_exit = None;
                        }
                        Some(_) => break,
                        None => {}
                    }
                }
            }
            Statement::ForIn(stmt) => {
                let stmt = stmt.clone();
                let object = self.execute_expression(&stmt.object);
                let keys: Vec<String> = match object {
                    JsValue::Object(map) => map.keys().cloned().collect(),
                    _ => vec![],
                };
                let for_in_is_var = stmt.binding_kind == VarKind::Var;
                for key in keys {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    self.stack.push(StackFrame::default());
                    if for_in_is_var {
                        self.execute_var_binding(&stmt.binding, JsValue::String(key));
                    } else {
                        self.execute_binding(&stmt.binding, JsValue::String(key));
                    }
                    self.execute_statement(&stmt.body);
                    self.stack.pop();
                    self.ensure_global_frame();
                    match self.early_exit {
                        Some(EarlyExit::Break) => {
                            self.early_exit = None;
                            break;
                        }
                        Some(EarlyExit::Continue) => {
                            self.early_exit = None;
                        }
                        Some(_) => break,
                        None => {}
                    }
                }
            }
            Statement::Switch(stmt) => {
                self.execute_switch(stmt);
            }
            Statement::Empty => {}
        }
    }

    fn consume_execution_budget(&mut self) -> bool {
        if self.execution_budget_exhausted {
            return false;
        }
        let Some(remaining) = self.execution_budget_remaining.as_mut() else {
            return true;
        };
        if *remaining == 0 {
            self.execution_budget_exhausted = true;
            return false;
        }
        *remaining -= 1;
        true
    }

    fn execute_switch(&mut self, stmt: &SwitchStatement) {
        let stmt = stmt.clone();
        let discriminant = self.execute_expression(&stmt.discriminant);

        // Find the first matching case index; record default position.
        let mut start_idx: Option<usize> = None;
        let mut default_idx: Option<usize> = None;
        for (i, case) in stmt.cases.iter().enumerate() {
            match &case.test {
                Some(test_expr) => {
                    if start_idx.is_none() {
                        let test_val = self.execute_expression(test_expr);
                        if Self::js_equal(&discriminant, &test_val) {
                            start_idx = Some(i);
                        }
                    }
                }
                None => {
                    default_idx = Some(i);
                }
            }
        }

        let run_from = start_idx.or(default_idx);
        if let Some(from) = run_from {
            'switch_body: for i in from..stmt.cases.len() {
                for body_stmt in &stmt.cases[i].body {
                    self.execute_statement(body_stmt);
                    if self.early_exit.is_some() {
                        break 'switch_body;
                    }
                }
            }
        }

        // `break` inside switch exits the switch, not an outer loop.
        if matches!(self.early_exit, Some(EarlyExit::Break)) {
            self.early_exit = None;
        }
    }

    fn hoist_function_declarations(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            if let Statement::FunctionDeclaration(decl) = stmt {
                let func = JsFunction {
                    name: Some(decl.name.clone()),
                    params: decl.params.clone(),
                    body: FunctionBody::Block(decl.body.clone()),
                    captured: self.stack.clone(),
                    properties: HashMap::new(),
                };
                self.set_local(&decl.name, JsValue::Function(func));
            }
        }
    }

    fn execute_block(&mut self, block: &BlockStatement) {
        self.stack.push(StackFrame::default());
        self.hoist_function_declarations(&block.body);
        for statement in &block.body {
            self.execute_statement(statement);
            if self.early_exit.is_some() {
                break;
            }
        }
        self.stack.pop();
        self.ensure_global_frame();
    }

    fn execute_variable_declaration(&mut self, declaration: &VariableDeclaration) {
        for declarator in &declaration.declarations {
            let value = declarator
                .init
                .as_ref()
                .map(|expression| self.execute_expression(expression))
                .unwrap_or(JsValue::Undefined);
            let binding = declarator.id.clone();
            if declaration.kind == VarKind::Var {
                self.execute_var_binding(&binding, value);
            } else {
                self.execute_binding(&binding, value);
            }
        }
    }

    fn execute_binding(&mut self, binding: &Binding, value: JsValue) {
        match binding {
            Binding::Name(name) => {
                self.set_local(name, value);
            }
            Binding::Object(props) => {
                for prop in props {
                    let extracted = match &value {
                        JsValue::Object(map) => {
                            map.get(&prop.key).cloned().unwrap_or(JsValue::Undefined)
                        }
                        _ => JsValue::Undefined,
                    };
                    let extracted = if extracted == JsValue::Undefined {
                        if let Some(default_expr) = &prop.default {
                            self.execute_expression(default_expr)
                        } else {
                            JsValue::Undefined
                        }
                    } else {
                        extracted
                    };
                    let sub = prop.binding.clone();
                    self.execute_binding(&sub, extracted);
                }
            }
            Binding::Array(items) => {
                let arr = match &value {
                    JsValue::Array(a) => a.clone(),
                    _ => Vec::new(),
                };
                for (i, item) in items.iter().enumerate() {
                    if let Some(sub_binding) = item {
                        let elem = arr.get(i).cloned().unwrap_or(JsValue::Undefined);
                        let sub = sub_binding.clone();
                        self.execute_binding(&sub, elem);
                    }
                }
            }
        }
    }

    // Like execute_binding but uses set_var so names land in the nearest function scope.
    fn execute_var_binding(&mut self, binding: &Binding, value: JsValue) {
        match binding {
            Binding::Name(name) => {
                self.set_var(name, value);
            }
            Binding::Object(props) => {
                for prop in props {
                    let extracted = match &value {
                        JsValue::Object(map) => {
                            map.get(&prop.key).cloned().unwrap_or(JsValue::Undefined)
                        }
                        _ => JsValue::Undefined,
                    };
                    let extracted = if extracted == JsValue::Undefined {
                        if let Some(default_expr) = &prop.default {
                            self.execute_expression(default_expr)
                        } else {
                            JsValue::Undefined
                        }
                    } else {
                        extracted
                    };
                    let sub = prop.binding.clone();
                    self.execute_var_binding(&sub, extracted);
                }
            }
            Binding::Array(items) => {
                let arr = match &value {
                    JsValue::Array(a) => a.clone(),
                    _ => Vec::new(),
                };
                for (i, item) in items.iter().enumerate() {
                    if let Some(sub_binding) = item {
                        let elem = arr.get(i).cloned().unwrap_or(JsValue::Undefined);
                        let sub = sub_binding.clone();
                        self.execute_var_binding(&sub, elem);
                    }
                }
            }
        }
    }

    fn execute_expression(&mut self, expression: &Expression) -> JsValue {
        match expression {
            Expression::Assignment { target, value } => {
                let value = self.execute_expression(value);
                self.assign_target(target, value.clone());
                value
            }
            Expression::Ternary {
                test,
                consequent,
                alternate,
            } => {
                if Self::is_truthy(&self.execute_expression(test)) {
                    self.execute_expression(consequent)
                } else {
                    self.execute_expression(alternate)
                }
            }
            Expression::Sequence(exprs) => {
                let exprs = exprs.clone();
                let mut last = JsValue::Undefined;
                for expr in &exprs {
                    last = self.execute_expression(expr);
                }
                last
            }
            Expression::Call { callee, arguments } => self.execute_call(callee, arguments),
            Expression::Member { .. } => self.eval_member(expression),
            Expression::Binary { op, left, right } => self.execute_binary(op, left, right),
            Expression::Unary { op, expr } => {
                let value = self.execute_expression(expr);
                match op {
                    UnaryOperator::Not => JsValue::Boolean(!Self::is_truthy(&value)),
                    UnaryOperator::Negate => JsValue::Number(-Self::value_to_number(&value)),
                    UnaryOperator::Plus => JsValue::Number(Self::value_to_number(&value)),
                    UnaryOperator::BitNot => {
                        JsValue::Number((!(Self::value_to_number(&value) as i32)) as f64)
                    }
                    UnaryOperator::Typeof => JsValue::String(Self::value_type_str(&value)),
                    UnaryOperator::Void => JsValue::Undefined,
                    UnaryOperator::Delete => JsValue::Boolean(true),
                }
            }
            Expression::Array(items) => {
                let items = items.clone();
                let mut values: Vec<JsValue> = Vec::new();
                for item in &items {
                    if let Expression::Spread(inner) = item {
                        let val = self.execute_expression(inner);
                        if let JsValue::Array(arr) = val {
                            values.extend(arr);
                        } else {
                            values.push(val);
                        }
                    } else {
                        values.push(self.execute_expression(item));
                    }
                }
                JsValue::Array(values)
            }
            Expression::Object(properties) => {
                JsValue::Object(self.object_from_properties(properties))
            }
            Expression::Function(fe) => JsValue::Function(JsFunction {
                name: None,
                params: fe.params.clone(),
                body: FunctionBody::Block(fe.body.clone()),
                captured: self.stack.clone(),
                properties: HashMap::new(),
            }),
            Expression::ArrowFunction { params, body, .. } => JsValue::Function(JsFunction {
                name: None,
                params: params.clone(),
                body: *body.clone(),
                captured: self.stack.clone(),
                properties: HashMap::new(),
            }),
            Expression::TemplateLiteral(parts) => {
                let parts = parts.clone();
                let mut s = String::new();
                for part in &parts {
                    match part {
                        crate::ast::TemplateElement::Str(text) => s.push_str(text),
                        crate::ast::TemplateElement::Expr(expr) => {
                            let val = self.execute_expression(expr);
                            s.push_str(&Self::value_to_string(&val));
                        }
                    }
                }
                JsValue::String(s)
            }
            Expression::Typeof(expr) => {
                let val = if let Expression::Identifier(name) = expr.as_ref() {
                    self.get_identifier_value(name)
                } else {
                    self.execute_expression(expr)
                };
                JsValue::String(Self::value_type_str(&val))
            }
            Expression::Void(expr) => {
                self.execute_expression(expr);
                JsValue::Undefined
            }
            Expression::Delete(_) => JsValue::Boolean(true),
            Expression::Await(expr) => self.execute_expression(expr),
            Expression::New { callee, arguments } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Date") {
                    JsValue::DateInstance
                } else if let Some(name) =
                    Self::soft_failure_constructor_name_from_expr(callee.as_ref())
                {
                    for argument in arguments {
                        self.execute_expression(argument);
                    }
                    JsValue::HostObject(name)
                } else if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Function")
                {
                    JsValue::HostFunction("Function".into())
                } else if matches!(callee.as_ref(), Expression::Identifier(name) if name == "RegExp")
                {
                    let pattern = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    let flags = arguments
                        .get(1)
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    JsValue::RegExp { pattern, flags }
                } else if matches!(callee.as_ref(), Expression::Identifier(name) if name == "XMLHttpRequest")
                {
                    self.trace_runtime("xhr.new", "XMLHttpRequest");
                    JsValue::XhrInstance {
                        method: "GET".to_owned(),
                        url: String::new(),
                        headers: HashMap::new(),
                    }
                } else if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise")
                {
                    self.trace_runtime(
                        "promise.new",
                        "Promise constructor approximated as resolved",
                    );
                    for argument in arguments {
                        self.execute_expression(argument);
                    }
                    JsValue::ResolvedPromise
                } else if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Proxy")
                {
                    let mut args = self.eval_args(arguments);
                    let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
                    let get = args.get_mut(1).and_then(|handler| {
                        if let JsValue::Object(map) = handler {
                            match map.get("get") {
                                Some(JsValue::Function(func)) => Some(func.clone()),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    });
                    JsValue::Proxy {
                        target: Box::new(target),
                        get,
                    }
                } else if matches!(callee.as_ref(), Expression::Identifier(name) if name == "WeakMap" || name == "Map")
                {
                    JsValue::WeakMap(HashMap::new())
                } else if let JsValue::Function(func) = self.execute_expression(callee) {
                    let this_obj = func
                        .properties
                        .get("prototype")
                        .cloned()
                        .unwrap_or_else(|| JsValue::Object(HashMap::new()));
                    let args = self.eval_args(arguments);
                    let (result, this_after) = self.call_function_with_this(func, args, this_obj);
                    if matches!(result, JsValue::Object(_)) {
                        result
                    } else {
                        this_after
                    }
                } else if let JsValue::HostFunction(name) = self.execute_expression(callee) {
                    JsValue::HostObject(name)
                } else if let Some(name) = constructor_like_member_name(callee) {
                    for argument in arguments {
                        self.execute_expression(argument);
                    }
                    if Self::soft_failure_constructor_name(&name) {
                        JsValue::HostObject(Self::soft_failure_host_name(callee.as_ref()))
                    } else {
                        JsValue::HostObject(name)
                    }
                } else {
                    self.trace_runtime("unsupported.constructor", format!("{:?}", callee.as_ref()));
                    JsValue::Undefined
                }
            }
            Expression::Spread(_) | Expression::Super => JsValue::Undefined,
            Expression::Identifier(name) => self.get_identifier_value(name),
            Expression::Number(value) => JsValue::Number(*value),
            Expression::String(value) => JsValue::String(value.clone()),
            Expression::Regex(value) => {
                let (pattern, flags) = parse_regex_literal(value);
                JsValue::RegExp { pattern, flags }
            }
            Expression::Boolean(value) => JsValue::Boolean(*value),
            Expression::Null => JsValue::Null,
            Expression::Undefined => JsValue::Undefined,
            Expression::This => self.get_identifier_value("this"),
        }
    }

    fn execute_call(&mut self, callee: &Expression, arguments: &[Expression]) -> JsValue {
        if matches!(callee, Expression::Identifier(name) if name == "String") {
            return arguments
                .first()
                .map(|argument| self.execute_expression(argument))
                .map(|value| JsValue::String(Self::value_to_string(&value)))
                .unwrap_or_else(|| JsValue::String(String::new()));
        }

        if matches!(callee, Expression::Identifier(name) if name == "RegExp") {
            let pattern = arguments
                .first()
                .map(|argument| self.execute_expression(argument))
                .map(|value| Self::value_to_string(&value))
                .unwrap_or_default();
            let flags = arguments
                .get(1)
                .map(|argument| self.execute_expression(argument))
                .map(|value| Self::value_to_string(&value))
                .unwrap_or_default();
            return JsValue::RegExp { pattern, flags };
        }

        if matches!(callee, Expression::Identifier(name) if name == "Function") {
            for argument in arguments {
                self.execute_expression(argument);
            }
            return JsValue::HostFunction("Function".into());
        }

        if let Some(method) = method_call(callee) {
            if matches!(&method.object, Expression::Identifier(n) if n == "JSON") {
                match method.name.as_str() {
                    "parse" => {
                        let arg = arguments
                            .first()
                            .map(|a| self.execute_expression(a))
                            .unwrap_or(JsValue::Null);
                        let s = Self::value_to_string(&arg);
                        return json_parse_str(&s);
                    }
                    "stringify" => {
                        let arg = arguments
                            .first()
                            .map(|a| self.execute_expression(a))
                            .unwrap_or(JsValue::Undefined);
                        return JsValue::String(json_stringify(&arg));
                    }
                    _ => {}
                }
            }
            if method.name == "push" {
                if let Expression::Identifier(var_name) = &method.object {
                    let val = arguments
                        .first()
                        .map(|a| self.execute_expression(a))
                        .unwrap_or(JsValue::Undefined);
                    let var_name = var_name.clone();
                    if let Some(JsValue::Array(mut arr)) = self.get_binding(&var_name) {
                        arr.push(val);
                        self.set_binding(&var_name, JsValue::Array(arr));
                    }
                    return JsValue::Undefined;
                }
            }
        }

        if matches!(callee, Expression::Identifier(name) if name == "fetch") {
            let url = arguments
                .first()
                .map(|argument| self.execute_expression(argument))
                .map(|value| Self::value_to_string(&value))
                .unwrap_or_default();
            let mut method = "GET".to_owned();
            let mut body = String::new();
            if let Some(options) = arguments
                .get(1)
                .map(|argument| self.execute_expression(argument))
            {
                if let JsValue::Object(map) = options {
                    if let Some(value) = map.get("method") {
                        method = Self::value_to_string(value).to_ascii_uppercase();
                    }
                    if let Some(value) = map.get("body") {
                        body = Self::value_to_string(value);
                    }
                }
            }
            self.emit_network_request(&method, url, body);
            return JsValue::ResolvedPromise;
        }

        if matches!(callee, Expression::Identifier(name) if name == "setTimeout") {
            let delay_ms = arguments
                .get(1)
                .map(|a| self.execute_expression(a))
                .map(|v| Self::value_to_number(&v).max(0.0) as u64)
                .unwrap_or(0);
            if let Some(Expression::Function(func)) = arguments.first() {
                self.pending_timers.push(PendingTimer {
                    fires_at_ms: self.current_time_ms + delay_ms,
                    params: func.params.iter().map(|p| p.name().to_owned()).collect(),
                    body: func.body.clone(),
                });
            }
            return JsValue::Undefined;
        }

        if matches!(callee, Expression::Identifier(name) if name == "getComputedStyle") {
            let element = arguments
                .first()
                .map(|a| self.execute_expression(a))
                .unwrap_or(JsValue::Undefined);
            if let JsValue::ElementRef(element_ref) = element {
                if let Some(element_id) = existing_id_from_ref(&element_ref) {
                    let mut props: HashMap<String, JsValue> = self
                        .dom
                        .computed_styles_by_id
                        .get(&element_id)
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), JsValue::String(v.clone())))
                                .collect()
                        })
                        .unwrap_or_default();
                    if let Some(inline) = self.get_element_attribute(&element_ref, "style") {
                        for (prop, val) in parse_inline_style_map(&inline) {
                            props.insert(prop, JsValue::String(val));
                        }
                    }
                    return JsValue::Object(props);
                }
            }
            return JsValue::Undefined;
        }

        if let Some(method) = method_call(callee) {
            match method.name.as_str() {
                "resolve" if matches!(&method.object, Expression::Identifier(n) if n == "Promise") =>
                {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::ResolvedPromise;
                }
                "reject" if matches!(&method.object, Expression::Identifier(n) if n == "Promise") =>
                {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::ResolvedPromise;
                }
                "all" | "allSettled" | "race" | "any"
                    if matches!(&method.object, Expression::Identifier(n) if n == "Promise") =>
                {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::ResolvedPromise;
                }
                "then" => {
                    let receiver = self.execute_expression(&method.object);
                    if matches!(receiver, JsValue::ResolvedPromise) {
                        if let Some(Expression::Function(func)) = arguments.first() {
                            self.pending_microtasks.push(PendingMicrotask {
                                params: func.params.iter().map(|p| p.name().to_owned()).collect(),
                                body: func.body.clone(),
                            });
                        }
                    }
                    return JsValue::Undefined;
                }
                "log" | "info" | "warn" | "error" if matches!(&method.object, Expression::Identifier(n) if n == "console") =>
                {
                    let text = arguments
                        .iter()
                        .map(|a| {
                            let v = self.execute_expression(a);
                            Self::value_to_string(&v)
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.effects.push(BrowserEffect::ConsoleLog {
                        level: method.name.clone(),
                        text,
                    });
                    return JsValue::Undefined;
                }
                "createElement" if method.receiver == MethodReceiver::Document => {
                    let tag_name = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_else(|| "div".to_owned());
                    return self.create_element(tag_name);
                }
                "createTextNode" if method.receiver == MethodReceiver::Document => {
                    let text = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    return self.create_text_node(text);
                }
                "createComment" if method.receiver == MethodReceiver::Document => {
                    let text = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    return self.create_comment_node(text);
                }
                "getElementById" if method.receiver == MethodReceiver::Document => {
                    let id = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    return JsValue::ElementRef(existing_element_ref(&id));
                }
                "querySelector" if method.receiver == MethodReceiver::Document => {
                    let selector = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    if let Some(id) = self.query_selector_first_id(&selector) {
                        return JsValue::ElementRef(existing_element_ref(&id));
                    }
                    return JsValue::Undefined;
                }
                "querySelectorAll" if method.receiver == MethodReceiver::Document => {
                    let selector = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    return JsValue::NodeList(self.query_selector_all_ids(&selector));
                }
                "appendChild" => {
                    let parent = self.execute_expression(&method.object);
                    let child = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument));
                    if let (JsValue::ElementRef(parent_ref), Some(JsValue::ElementRef(child_ref))) =
                        (parent, child)
                    {
                        self.append_child(&parent_ref, &child_ref);
                    }
                    return JsValue::Undefined;
                }
                "insertBefore" => {
                    let parent = self.execute_expression(&method.object);
                    let child = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument));
                    if let (JsValue::ElementRef(parent_ref), Some(JsValue::ElementRef(child_ref))) =
                        (parent, child)
                    {
                        self.insert_before(&parent_ref, &child_ref);
                    }
                    return JsValue::Undefined;
                }
                "removeChild" => {
                    let parent = self.execute_expression(&method.object);
                    let child = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument));
                    if let (JsValue::ElementRef(parent_ref), Some(JsValue::ElementRef(child_ref))) =
                        (parent, child)
                    {
                        self.remove_child(&parent_ref, &child_ref);
                    }
                    return JsValue::Undefined;
                }
                "setAttribute" => {
                    let target = self.execute_expression(&method.object);
                    let name = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    let value = arguments
                        .get(1)
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    if let JsValue::ElementRef(element_ref) = target {
                        self.set_element_attribute(&element_ref, &name, value);
                    }
                    return JsValue::Undefined;
                }
                "getAttribute" => {
                    let target = self.execute_expression(&method.object);
                    let name = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    if let JsValue::ElementRef(element_ref) = target {
                        return JsValue::String(
                            self.get_element_attribute(&element_ref, &name)
                                .unwrap_or_default(),
                        );
                    }
                    return JsValue::Undefined;
                }
                "addEventListener" | "attachEvent" | "detachEvent" => {
                    let receiver = self.execute_expression(&method.object);
                    let mut event_type = arguments
                        .first()
                        .map(|a| self.execute_expression(a))
                        .map(|v| Self::value_to_string(&v))
                        .unwrap_or_default();
                    if method.name == "detachEvent" {
                        return JsValue::Undefined;
                    }
                    if let Some(stripped) = event_type.strip_prefix("on") {
                        event_type = stripped.to_owned();
                    }
                    // DOM is already parsed and page is loaded by the time scripts run, so
                    // DOMContentLoaded and load fire as immediate microtasks.
                    let is_document =
                        matches!(&method.object, Expression::Identifier(n) if n == "document");
                    let is_window = matches!(&method.object, Expression::Identifier(n) if n == "window")
                        || matches!(receiver, JsValue::WindowRef);
                    if (is_document && event_type == "DOMContentLoaded")
                        || (is_window && (event_type == "load" || event_type == "DOMContentLoaded"))
                    {
                        if let Some(Expression::Function(func)) = arguments.get(1) {
                            self.pending_microtasks.push(PendingMicrotask {
                                params: func.params.iter().map(|p| p.name().to_owned()).collect(),
                                body: func.body.clone(),
                            });
                        }
                        return JsValue::Undefined;
                    }
                    if let JsValue::ElementRef(element_ref) = receiver {
                        if let Some(element_id) = existing_id_from_ref(&element_ref) {
                            if let Some(Expression::Function(func)) = arguments.get(1) {
                                self.event_handlers.push(EventHandler {
                                    element_id,
                                    event_type,
                                    params: func
                                        .params
                                        .iter()
                                        .map(|p| p.name().to_owned())
                                        .collect(),
                                    body: func.body.clone(),
                                    captured: self.stack.clone(),
                                });
                            }
                        }
                    }
                    return JsValue::Undefined;
                }
                _ => {}
            }
        }

        if let Expression::Member {
            object,
            property: MemberProperty::Named(method_name),
            ..
        } = callee
        {
            if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                && method_name == "defineProperty"
            {
                return self.call_object_define_property(arguments);
            }
        }

        // Static namespace calls: Math.*, Object.*, Array.*, Number.*, parseInt, parseFloat
        if let Expression::Member {
            object,
            property: MemberProperty::Named(method_name),
            ..
        } = callee
        {
            if let Expression::Identifier(obj_name) = object.as_ref() {
                match obj_name.as_str() {
                    "Math" => {
                        let args = self.eval_args(arguments);
                        return self.call_math_method(method_name, &args);
                    }
                    "Object" => {
                        let args = self.eval_args(arguments);
                        return self.call_object_static(method_name, args);
                    }
                    "Array" => {
                        let args = self.eval_args(arguments);
                        return self.call_array_static(method_name, args);
                    }
                    "Number" => {
                        let args = self.eval_args(arguments);
                        return self.call_number_static(method_name, &args);
                    }
                    "String" => {
                        // String.fromCharCode
                        if method_name == "fromCharCode" {
                            let args = self.eval_args(arguments);
                            let s: String = args
                                .iter()
                                .map(|v| {
                                    char::from_u32(Self::value_to_number(v) as u32).unwrap_or('\0')
                                })
                                .collect();
                            return JsValue::String(s);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Free-standing global functions
        if let Expression::Identifier(fn_name) = callee {
            match fn_name.as_str() {
                "parseInt" => {
                    let args = self.eval_args(arguments);
                    let s = Self::value_to_string(args.first().unwrap_or(&JsValue::Undefined));
                    let radix = args
                        .get(1)
                        .map(|v| Self::value_to_number(v) as u32)
                        .unwrap_or(10);
                    let radix = if radix < 2 || radix > 36 { 10 } else { radix };
                    // consume only valid chars for the given radix (like JS parseInt)
                    let trimmed = s.trim();
                    let (sign, rest) = if trimmed.starts_with('-') {
                        (-1i64, &trimmed[1..])
                    } else if trimmed.starts_with('+') {
                        (1, &trimmed[1..])
                    } else {
                        (1, trimmed)
                    };
                    let digits: String = rest
                        .chars()
                        .take_while(|c| c.to_digit(radix).is_some())
                        .collect();
                    return if digits.is_empty() {
                        JsValue::Number(f64::NAN)
                    } else {
                        JsValue::Number(
                            sign as f64 * i64::from_str_radix(&digits, radix).unwrap_or(0) as f64,
                        )
                    };
                }
                "parseFloat" => {
                    let args = self.eval_args(arguments);
                    let s = Self::value_to_string(args.first().unwrap_or(&JsValue::Undefined));
                    // consume valid float prefix
                    let trimmed = s.trim();
                    let valid: String = trimmed
                        .chars()
                        .scan(false, |saw_dot, c| {
                            if c.is_ascii_digit() {
                                Some(c)
                            } else if c == '-' || c == '+' {
                                Some(c)
                            } else if c == '.' && !*saw_dot {
                                *saw_dot = true;
                                Some(c)
                            } else if c == 'e' || c == 'E' {
                                Some(c)
                            } else {
                                None
                            }
                        })
                        .collect();
                    return match valid.parse::<f64>() {
                        Ok(n) => JsValue::Number(n),
                        Err(_) => JsValue::Number(f64::NAN),
                    };
                }
                "isNaN" => {
                    let args = self.eval_args(arguments);
                    let n = Self::value_to_number(args.first().unwrap_or(&JsValue::Undefined));
                    return JsValue::Boolean(n.is_nan());
                }
                "isFinite" => {
                    let args = self.eval_args(arguments);
                    let n = Self::value_to_number(args.first().unwrap_or(&JsValue::Undefined));
                    return JsValue::Boolean(n.is_finite());
                }
                "encodeURIComponent" => {
                    let args = self.eval_args(arguments);
                    let s = Self::value_to_string(args.first().unwrap_or(&JsValue::Undefined));
                    let encoded: String = s
                        .bytes()
                        .flat_map(|b| {
                            if b.is_ascii_alphanumeric() || b"_.-!~*'()".contains(&b) {
                                vec![b as char]
                            } else {
                                format!("%{b:02X}").chars().collect()
                            }
                        })
                        .collect();
                    return JsValue::String(encoded);
                }
                "decodeURIComponent" => {
                    let args = self.eval_args(arguments);
                    let s = Self::value_to_string(args.first().unwrap_or(&JsValue::Undefined));
                    return JsValue::String(s); // passthrough approximation
                }
                _ => {}
            }
        }

        // Generic dispatch: evaluate callee, call if it's a function.
        // For method calls, also pass receiver so object methods work.
        if let Expression::Member {
            object,
            property: MemberProperty::Named(method_name),
            optional,
        } = callee
        {
            let receiver = self.execute_expression(object);
            if *optional && matches!(receiver, JsValue::Null | JsValue::Undefined) {
                return JsValue::Undefined;
            }
            let method_name = method_name.clone();

            // Built-in number instance methods (.toFixed, .toString, etc.)
            if let JsValue::Number(n) = receiver {
                let args_vals = self.eval_args(arguments);
                let result = match method_name.as_str() {
                    "toFixed" => {
                        let digits = args_vals
                            .first()
                            .map(|v| Self::value_to_number(v) as usize)
                            .unwrap_or(0);
                        JsValue::String(format!("{n:.digits$}"))
                    }
                    "toPrecision" => {
                        let p = args_vals
                            .first()
                            .map(|v| Self::value_to_number(v) as usize)
                            .unwrap_or(1);
                        JsValue::String(format!("{n:.p$}"))
                    }
                    "toString" => {
                        let radix = args_vals
                            .first()
                            .map(|v| Self::value_to_number(v) as u32)
                            .unwrap_or(10);
                        if radix == 10 || radix < 2 || radix > 36 {
                            JsValue::String(Self::value_to_string(&JsValue::Number(n)))
                        } else {
                            JsValue::String(format!("{}", n as i64)) // simplified non-base-10
                        }
                    }
                    "valueOf" => JsValue::Number(n),
                    _ => JsValue::Undefined,
                };
                return result;
            }

            // Built-in array instance methods
            if let JsValue::Array(arr) = receiver.clone() {
                // Check for an overridden method before falling back to native dispatch.
                // Covers: window.X.method(args), (window.X = ...).method(args),
                //         and localVar.method(args) where the override was stored via
                //         localVar.method = fn assignment.
                let override_key = if let Some(global_name) = extract_window_global_name(object) {
                    Some(format!("{global_name}:{method_name}"))
                } else if let Expression::Identifier(varname) = object.as_ref() {
                    Some(format!("{varname}:{method_name}"))
                } else {
                    None
                };
                if let Some(key) = override_key {
                    if let Some(JsValue::Function(func)) =
                        self.array_method_overrides.get(&key).cloned()
                    {
                        let args = self.eval_args(arguments);
                        return self.call_function(func, args);
                    }
                }
                // Value-based fallback: when the expression form yields no key (or the key has
                // no entry), scan globals for any that currently holds this exact array value and
                // has an override registered under "{global_name}:{method_name}".  This covers
                // patterns like `(window["webpackJsonp"] = window["webpackJsonp"] || []).push(…)`
                // where the receiver is an assignment expression whose target identity we can
                // resolve through the globals map rather than through the AST node form.
                {
                    let arr_val = JsValue::Array(arr.clone());
                    let func_opt: Option<JsFunction> = self.globals.iter().find_map(|(gname, gval)| {
                        if gval == &arr_val {
                            let key = format!("{gname}:{method_name}");
                            if let Some(JsValue::Function(f)) = self.array_method_overrides.get(&key) {
                                return Some(f.clone());
                            }
                        }
                        None
                    });
                    if let Some(func) = func_opt {
                        let args = self.eval_args(arguments);
                        return self.call_function(func, args);
                    }
                }
                // Mutating array methods: compute new array state and write back to the
                // receiver expression so value-semantics arrays stay consistent.
                match method_name.as_str() {
                    "push" => {
                        let mut new_arr = arr;
                        for arg in arguments {
                            match arg {
                                Expression::Spread(inner) => {
                                    if let JsValue::Array(spread_items) = self.execute_expression(inner) {
                                        new_arr.extend(spread_items);
                                    }
                                }
                                _ => new_arr.push(self.execute_expression(arg)),
                            }
                        }
                        let len = new_arr.len();
                        self.assign_target(object, JsValue::Array(new_arr));
                        return JsValue::Number(len as f64);
                    }
                    "pop" => {
                        let mut new_arr = arr;
                        let result = new_arr.pop().unwrap_or(JsValue::Undefined);
                        self.assign_target(object, JsValue::Array(new_arr));
                        return result;
                    }
                    "shift" => {
                        let mut new_arr = arr;
                        let result = if new_arr.is_empty() {
                            JsValue::Undefined
                        } else {
                            new_arr.remove(0)
                        };
                        self.assign_target(object, JsValue::Array(new_arr));
                        return result;
                    }
                    "unshift" => {
                        let new_items: Vec<JsValue> = arguments
                            .iter()
                            .map(|a| self.execute_expression(a))
                            .collect();
                        let len = new_items.len() + arr.len();
                        let mut new_arr = new_items;
                        new_arr.extend(arr);
                        self.assign_target(object, JsValue::Array(new_arr));
                        return JsValue::Number(len as f64);
                    }
                    "splice" => {
                        let mut new_arr = arr;
                        let start = arguments
                            .first()
                            .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                            .unwrap_or(0)
                            .min(new_arr.len());
                        let delete_count = arguments
                            .get(1)
                            .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                            .unwrap_or(new_arr.len() - start)
                            .min(new_arr.len() - start);
                        let removed: Vec<JsValue> = new_arr.drain(start..start + delete_count).collect();
                        for (i, arg) in arguments.iter().skip(2).enumerate() {
                            new_arr.insert(start + i, self.execute_expression(arg));
                        }
                        self.assign_target(object, JsValue::Array(new_arr));
                        return JsValue::Array(removed);
                    }
                    _ => {}
                }
                if let Some(v) = self.call_array_method(&method_name, arr, arguments) {
                    return v;
                }
            }

            // Built-in string instance methods
            if let JsValue::String(ref s) = receiver {
                let s = s.clone();
                if let Some(v) = self.call_string_method(&method_name, &s, arguments) {
                    return v;
                }
            }

            if let JsValue::StorageRef(kind) = receiver.clone() {
                return self.call_storage_method(kind, &method_name, arguments);
            }

            if matches!(receiver, JsValue::DocumentRef) {
                match method_name.as_str() {
                    "createElement" => {
                        let tag_name = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_else(|| "div".to_owned());
                        return self.create_element(tag_name);
                    }
                    "createTextNode" => {
                        let text = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        return self.create_text_node(text);
                    }
                    "createComment" => {
                        let text = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        return self.create_comment_node(text);
                    }
                    "getElementById" => {
                        let id = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        return JsValue::ElementRef(existing_element_ref(&id));
                    }
                    "getElementsByTagName" => {
                        let tag = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value).to_ascii_lowercase())
                            .unwrap_or_default();
                        let ids = match tag.as_str() {
                            "body" => vec![existing_element_ref("body")],
                            "head" => vec![existing_element_ref("head")],
                            "html" => vec![existing_element_ref("html")],
                            _ => Vec::new(),
                        };
                        return JsValue::Array(ids.into_iter().map(JsValue::ElementRef).collect());
                    }
                    "addEventListener" | "attachEvent" | "detachEvent" => {
                        let mut event_type = arguments
                            .first()
                            .map(|a| self.execute_expression(a))
                            .map(|v| Self::value_to_string(&v))
                            .unwrap_or_default();
                        if method_name == "detachEvent" {
                            return JsValue::Undefined;
                        }
                        if let Some(stripped) = event_type.strip_prefix("on") {
                            event_type = stripped.to_owned();
                        }
                        if event_type == "DOMContentLoaded" {
                            if let Some(Expression::Function(func)) = arguments.get(1) {
                                self.pending_microtasks.push(PendingMicrotask {
                                    params: func
                                        .params
                                        .iter()
                                        .map(|p| p.name().to_owned())
                                        .collect(),
                                    body: func.body.clone(),
                                });
                            }
                        }
                        return JsValue::Undefined;
                    }
                    _ => {}
                }
            }

            if matches!(receiver, JsValue::NavigatorRef) {
                match method_name.as_str() {
                    "javaEnabled" => {
                        for arg in arguments {
                            self.execute_expression(arg);
                        }
                        return JsValue::Boolean(false);
                    }
                    "getBattery" => {
                        for arg in arguments {
                            self.execute_expression(arg);
                        }
                        return JsValue::ResolvedPromise;
                    }
                    _ => {}
                }
            }

            if let JsValue::HostObject(name) = receiver.clone() {
                for argument in arguments {
                    self.execute_expression(argument);
                }
                return Self::host_object_method_return(&name, &method_name);
            }

            if let JsValue::XhrInstance {
                mut method,
                mut url,
                mut headers,
            } = receiver.clone()
            {
                match method_name.as_str() {
                    "open" => {
                        method = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value).to_ascii_uppercase())
                            .unwrap_or_else(|| "GET".to_owned());
                        url = arguments
                            .get(1)
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        self.trace_runtime("xhr.open", format!("{method} {url}"));
                        self.assign_target(
                            object,
                            JsValue::XhrInstance {
                                method,
                                url,
                                headers,
                            },
                        );
                        return JsValue::Undefined;
                    }
                    "setRequestHeader" => {
                        let name = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        let value = arguments
                            .get(1)
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        headers.insert(name.clone(), value);
                        self.trace_runtime("xhr.header", name);
                        self.assign_target(
                            object,
                            JsValue::XhrInstance {
                                method,
                                url,
                                headers,
                            },
                        );
                        return JsValue::Undefined;
                    }
                    "send" => {
                        let body = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        self.emit_network_request(&method, url, body);
                        return JsValue::Undefined;
                    }
                    _ => {}
                }
            }

            if let JsValue::WeakMap(mut map) = receiver.clone() {
                match method_name.as_str() {
                    "set" => {
                        let key = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .unwrap_or(JsValue::Undefined);
                        let value = arguments
                            .get(1)
                            .map(|argument| self.execute_expression(argument))
                            .unwrap_or(JsValue::Undefined);
                        map.insert(Self::weak_map_key(&key), value);
                        self.assign_target(object, JsValue::WeakMap(map));
                        return receiver;
                    }
                    "get" => {
                        let key = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .unwrap_or(JsValue::Undefined);
                        return map
                            .get(&Self::weak_map_key(&key))
                            .cloned()
                            .unwrap_or(JsValue::Undefined);
                    }
                    "has" => {
                        let key = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .unwrap_or(JsValue::Undefined);
                        return JsValue::Boolean(map.contains_key(&Self::weak_map_key(&key)));
                    }
                    "delete" => {
                        let key = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .unwrap_or(JsValue::Undefined);
                        let removed = map.remove(&Self::weak_map_key(&key)).is_some();
                        self.assign_target(object, JsValue::WeakMap(map));
                        return JsValue::Boolean(removed);
                    }
                    _ => {}
                }
            }

            if matches!(receiver, JsValue::DateInstance) {
                if method_name == "getTimezoneOffset" {
                    let offset = self
                        .fingerprint_suite
                        .as_ref()
                        .map(|suite| suite.timezone.offset_minutes)
                        .unwrap_or_else(|| {
                            crate::specs_placeholder::TimezoneInfo::detect().offset_minutes
                        });
                    return JsValue::Number(offset as f64);
                }
                if method_name == "toString" {
                    return JsValue::String("[object Date]".to_owned());
                }
            }

            if let JsValue::RegExp { pattern, flags } = receiver.clone() {
                match method_name.as_str() {
                    "test" => {
                        let haystack = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        return JsValue::Boolean(Self::simple_regex_test(
                            &pattern, &flags, &haystack,
                        ));
                    }
                    "exec" => {
                        let haystack = arguments
                            .first()
                            .map(|argument| self.execute_expression(argument))
                            .map(|value| Self::value_to_string(&value))
                            .unwrap_or_default();
                        if Self::simple_regex_test(&pattern, &flags, &haystack) {
                            return JsValue::Array(vec![JsValue::String(pattern)]);
                        }
                        return JsValue::Null;
                    }
                    "toString" => {
                        return JsValue::String(format!("/{pattern}/{flags}"));
                    }
                    _ => {}
                }
            }

            if let JsValue::ElementRef(element_ref) = receiver.clone() {
                if method_name == "toDataURL"
                    && self.element_tag_name(&element_ref) == Some("canvas")
                {
                    return JsValue::String(
                        self.fingerprint_suite
                            .as_ref()
                            .map(|suite| suite.canvas.data_url.clone())
                            .unwrap_or_else(|| {
                                crate::specs_placeholder::CanvasFingerprint::detect().data_url
                            }),
                    );
                }
                if method_name == "getContext"
                    && self.element_tag_name(&element_ref) == Some("canvas")
                {
                    let context_name = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    return self.canvas_context_ref(&context_name);
                }
                if method_name == "getBoundingClientRect" {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    let mut rect = HashMap::new();
                    for key in &["top", "left", "right", "bottom", "width", "height", "x", "y"] {
                        rect.insert((*key).to_owned(), JsValue::Number(0.0));
                    }
                    return JsValue::Object(rect);
                }
                if matches!(
                    method_name.as_str(),
                    "matches" | "webkitMatchesSelector" | "mozMatchesSelector"
                ) {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Boolean(false);
                }
                if matches!(method_name.as_str(), "closest") {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Null;
                }
                if matches!(method_name.as_str(), "contains") {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Boolean(false);
                }
                if matches!(method_name.as_str(), "hasAttribute" | "hasAttributes") {
                    let name = arguments
                        .first()
                        .map(|a| Self::value_to_string(&self.execute_expression(a)))
                        .unwrap_or_default();
                    return JsValue::Boolean(
                        self.get_element_attribute(&element_ref, &name).is_some(),
                    );
                }
                if method_name == "removeAttribute" {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Undefined;
                }
                if method_name == "querySelector" {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Null;
                }
                if method_name == "querySelectorAll" {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Array(vec![]);
                }
                if method_name == "getElementsByTagName" || method_name == "getElementsByClassName" {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Array(vec![]);
                }
                if method_name == "focus" || method_name == "blur" || method_name == "click" {
                    for arg in arguments {
                        self.execute_expression(arg);
                    }
                    return JsValue::Undefined;
                }
                if Self::element_plugin_method_returns_empty(&method_name) {
                    for argument in arguments {
                        self.execute_expression(argument);
                    }
                    return JsValue::String(String::new());
                }
            }

            if let JsValue::CanvasContextRef(context_name) = receiver.clone() {
                if matches!(
                    method_name.as_str(),
                    "fillRect"
                        | "fillText"
                        | "strokeText"
                        | "beginPath"
                        | "closePath"
                        | "stroke"
                        | "fill"
                        | "rect"
                        | "moveTo"
                        | "lineTo"
                ) {
                    return JsValue::Undefined;
                }
                if method_name == "getParameter" {
                    let key = arguments
                        .first()
                        .map(|argument| self.execute_expression(argument))
                        .map(|value| Self::value_to_string(&value))
                        .unwrap_or_default();
                    return self.webgl_parameter_value(&context_name, &key);
                }
            }

            if method_name == "item" {
                let index = arguments
                    .first()
                    .map(|argument| self.execute_expression(argument))
                    .map(|value| Self::value_to_number(&value))
                    .unwrap_or(0.0);
                if index >= 0.0 && index.fract() == 0.0 {
                    let index = index as usize;
                    return match receiver.clone() {
                        JsValue::Array(items) => items.get(index).cloned().unwrap_or(JsValue::Null),
                        JsValue::NodeList(ids) => ids
                            .get(index)
                            .map(|id| JsValue::ElementRef(existing_element_ref(id)))
                            .unwrap_or(JsValue::Null),
                        _ => JsValue::Undefined,
                    };
                }
                return JsValue::Null;
            }

            if let JsValue::HostFunction(name) = receiver.clone() {
                // Dispatch static method calls such as Symbol.for(...) via compound name.
                let compound = format!("{name}.{method_name}");
                if !matches!(method_name.as_str(), "call" | "apply" | "bind") {
                    let args = self.eval_args(arguments);
                    let result = self.call_host_function(&compound, JsValue::Undefined, args);
                    if !matches!(result, JsValue::Undefined) {
                        return result;
                    }
                }
                match method_name.as_str() {
                    "call" => {
                        let mut args = self.eval_args(arguments);
                        let this_arg = if args.is_empty() {
                            JsValue::Undefined
                        } else {
                            args.remove(0)
                        };
                        return self.call_host_function(&name, this_arg, args);
                    }
                    "apply" => {
                        let mut args = self.eval_args(arguments);
                        let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                        let real_args = match args.get_mut(1) {
                            Some(JsValue::Array(items)) => std::mem::take(items),
                            _ => vec![],
                        };
                        // For array-mutating host methods, write the updated array back to
                        // the first argument expression so `f.push.apply(f, items)` reflects
                        // the change on `f` (value-semantics arrays are not shared by reference).
                        if let JsValue::Array(mut arr) = this_arg.clone() {
                            let mut wrote_back = false;
                            match name.as_str() {
                                "Array.prototype.push" => {
                                    arr.extend(real_args.clone());
                                    wrote_back = true;
                                }
                                "Array.prototype.unshift" => {
                                    let old = std::mem::take(&mut arr);
                                    arr = real_args.clone();
                                    arr.extend(old);
                                    wrote_back = true;
                                }
                                _ => {}
                            }
                            if wrote_back {
                                if let Some(expr) = arguments.first() {
                                    self.assign_target(expr, JsValue::Array(arr));
                                }
                            }
                        }
                        return self.call_host_function(&name, this_arg, real_args);
                    }
                    "bind" => {
                        let mut args = self.eval_args(arguments);
                        let this_arg = if args.is_empty() {
                            JsValue::Undefined
                        } else {
                            args.remove(0)
                        };
                        return JsValue::BoundHostFunction {
                            name,
                            this_arg: Box::new(this_arg),
                            bound_args: args,
                        };
                    }
                    _ => {}
                }
            }

            if let JsValue::BoundHostFunction {
                name,
                this_arg,
                bound_args,
            } = receiver.clone()
            {
                match method_name.as_str() {
                    "call" => {
                        let mut args = self.eval_args(arguments);
                        let call_this = if args.is_empty() {
                            *this_arg
                        } else {
                            args.remove(0)
                        };
                        let mut real_args = bound_args;
                        real_args.extend(args);
                        return self.call_host_function(&name, call_this, real_args);
                    }
                    "apply" => {
                        let mut args = self.eval_args(arguments);
                        let call_this = args.first().cloned().unwrap_or(*this_arg);
                        let mut real_args = bound_args;
                        if let Some(JsValue::Array(items)) = args.get_mut(1) {
                            real_args.extend(std::mem::take(items));
                        }
                        return self.call_host_function(&name, call_this, real_args);
                    }
                    "bind" => {
                        let mut args = self.eval_args(arguments);
                        let next_this = if args.is_empty() {
                            *this_arg
                        } else {
                            args.remove(0)
                        };
                        let mut next_args = bound_args;
                        next_args.extend(args);
                        return JsValue::BoundHostFunction {
                            name,
                            this_arg: Box::new(next_this),
                            bound_args: next_args,
                        };
                    }
                    _ => {}
                }
            }

            // Function.prototype.call / apply / bind
            if let JsValue::Function(func) = receiver.clone() {
                match method_name.as_str() {
                    "call" => {
                        // func.call(thisArg, arg1, arg2, ...)
                        let args = self.eval_args(arguments);
                        let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                        let real_args = if args.is_empty() {
                            vec![]
                        } else {
                            args[1..].to_vec()
                        };
                        let real_exprs: Vec<Expression> =
                            arguments.iter().skip(1).cloned().collect();
                        return self.call_function_with_writeback(
                            func,
                            real_args,
                            &real_exprs,
                            this_arg,
                        );
                    }
                    "apply" => {
                        // func.apply(thisArg, [arg1, arg2, ...]) — spread second arg
                        let mut iter = arguments.iter();
                        let this_arg = iter
                            .next()
                            .map(|a| self.execute_expression(a))
                            .unwrap_or(JsValue::Undefined);
                        let args_val = iter
                            .next()
                            .map(|a| self.execute_expression(a))
                            .unwrap_or(JsValue::Undefined);
                        let real_args = match args_val {
                            JsValue::Array(arr) => arr,
                            _ => vec![],
                        };
                        return self.call_function_with_this(func, real_args, this_arg).0;
                    }
                    "bind" => {
                        // func.bind(thisArg) — return the same function (ignore this)
                        for arg in arguments {
                            self.execute_expression(arg);
                        }
                        return JsValue::Function(func);
                    }
                    _ => {}
                }
            }

            if let JsValue::Object(ref map) = receiver {
                if let Some(JsValue::Function(func)) = map.get(&method_name).cloned() {
                    let args = self.eval_args(arguments);
                    let (result, this_after) =
                        self.call_function_with_this(func, args, receiver.clone());
                    if matches!(this_after, JsValue::Object(_) | JsValue::Array(_)) {
                        self.assign_target(object, this_after);
                    }
                    return result;
                }
                if let Some(JsValue::HostFunction(name)) = map.get(&method_name).cloned() {
                    let args = self.eval_args(arguments);
                    return self.call_host_function(&name, receiver.clone(), args);
                }
                // hasOwnProperty on any object
                if method_name == "hasOwnProperty" {
                    let key = arguments
                        .first()
                        .map(|a| Self::value_to_string(&self.execute_expression(a)))
                        .unwrap_or_default();
                    return JsValue::Boolean(map.contains_key(&key));
                }
            }
            // Evaluated receiver but method not found; trace it and still evaluate args for side effects.
            self.trace_runtime(
                "unsupported.method",
                format!(
                    "{} on {} via {:?}",
                    method_name,
                    Self::value_to_string(&receiver),
                    callee
                ),
            );
            for arg in arguments {
                self.execute_expression(arg);
            }
            return JsValue::Undefined;
        }

        let func_val = self.execute_expression(callee);
        let args = self.eval_args(arguments);
        if let JsValue::Function(func) = func_val {
            return self.call_function(func, args);
        }
        if let JsValue::HostFunction(name) = func_val {
            let value = self.call_host_function(&name, JsValue::Undefined, args);
            return if matches!(value, JsValue::Undefined) {
                JsValue::HostObject(name)
            } else {
                value
            };
        }
        if let JsValue::BoundHostFunction {
            name,
            this_arg,
            mut bound_args,
        } = func_val
        {
            bound_args.extend(args);
            return self.call_host_function(&name, *this_arg, bound_args);
        }
        if !matches!(func_val, JsValue::Undefined | JsValue::Null) {
            self.trace_runtime(
                "unsupported.call",
                format!("callee={}", Self::value_to_string(&func_val)),
            );
        }
        JsValue::Undefined
    }

    fn call_storage_method(
        &mut self,
        kind: StorageKind,
        name: &str,
        arguments: &[Expression],
    ) -> JsValue {
        match name {
            "setItem" => {
                let key = arguments
                    .first()
                    .map(|argument| self.execute_expression(argument))
                    .map(|value| Self::value_to_string(&value))
                    .unwrap_or_default();
                let value = arguments
                    .get(1)
                    .map(|argument| self.execute_expression(argument))
                    .map(|value| Self::value_to_string(&value))
                    .unwrap_or_default();
                self.storage_map_mut(kind).insert(key, value);
                JsValue::Undefined
            }
            "getItem" => {
                let key = arguments
                    .first()
                    .map(|argument| self.execute_expression(argument))
                    .map(|value| Self::value_to_string(&value))
                    .unwrap_or_default();
                self.storage_map(&kind)
                    .get(&key)
                    .cloned()
                    .map(JsValue::String)
                    .unwrap_or(JsValue::Null)
            }
            "removeItem" => {
                let key = arguments
                    .first()
                    .map(|argument| self.execute_expression(argument))
                    .map(|value| Self::value_to_string(&value))
                    .unwrap_or_default();
                self.storage_map_mut(kind).remove(&key);
                JsValue::Undefined
            }
            "clear" => {
                self.storage_map_mut(kind).clear();
                JsValue::Undefined
            }
            "key" => {
                let index = arguments
                    .first()
                    .map(|argument| self.execute_expression(argument))
                    .map(|value| Self::value_to_number(&value) as usize)
                    .unwrap_or(0);
                self.storage_map(&kind)
                    .keys()
                    .nth(index)
                    .cloned()
                    .map(JsValue::String)
                    .unwrap_or(JsValue::Null)
            }
            _ => JsValue::Undefined,
        }
    }

    fn canvas_context_ref(&self, context_name: &str) -> JsValue {
        match context_name {
            "2d" => JsValue::CanvasContextRef(context_name.to_owned()),
            "webgl" | "experimental-webgl" | "webgl2" => {
                if self
                    .fingerprint_suite
                    .as_ref()
                    .map(|suite| suite.webgl.is_supported)
                    .unwrap_or(true)
                {
                    JsValue::CanvasContextRef(context_name.to_owned())
                } else {
                    JsValue::Null
                }
            }
            _ => JsValue::Null,
        }
    }

    fn element_plugin_method_returns_empty(method_name: &str) -> bool {
        matches!(
            method_name,
            "getComponentVersion" | "getVariable" | "GetVariable" | "IsVersionSupported"
        )
    }

    fn storage_map(&self, kind: &StorageKind) -> &HashMap<String, String> {
        match kind {
            StorageKind::Local => &self.local_storage,
            StorageKind::Session => &self.session_storage,
        }
    }

    fn storage_map_mut(&mut self, kind: StorageKind) -> &mut HashMap<String, String> {
        match kind {
            StorageKind::Local => &mut self.local_storage,
            StorageKind::Session => &mut self.session_storage,
        }
    }

    fn webgl_parameter_value(&self, context_name: &str, key: &str) -> JsValue {
        if !matches!(context_name, "webgl" | "experimental-webgl" | "webgl2") {
            return JsValue::Undefined;
        }
        let Some(suite) = self.fingerprint_suite.as_ref() else {
            return JsValue::Undefined;
        };
        match key {
            "VENDOR" => JsValue::String(suite.webgl.vendor.clone()),
            "RENDERER" => JsValue::String(suite.webgl.renderer.clone()),
            "VERSION" => JsValue::String(suite.webgl.version.clone()),
            "SHADING_LANGUAGE_VERSION" => {
                JsValue::String(suite.webgl.shading_language_version.clone())
            }
            _ => suite
                .webgl
                .parameters
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| JsValue::String(value.clone()))
                .unwrap_or(JsValue::Undefined),
        }
    }

    fn call_math_method(&self, name: &str, args: &[JsValue]) -> JsValue {
        let a0 = || Self::value_to_number(args.first().unwrap_or(&JsValue::Undefined));
        let a1 = || Self::value_to_number(args.get(1).unwrap_or(&JsValue::Undefined));
        match name {
            "floor" => JsValue::Number(a0().floor()),
            "ceil" => JsValue::Number(a0().ceil()),
            "round" => JsValue::Number(a0().round()),
            "abs" => JsValue::Number(a0().abs()),
            "sqrt" => JsValue::Number(a0().sqrt()),
            "log" => JsValue::Number(a0().ln()),
            "log2" => JsValue::Number(a0().log2()),
            "log10" => JsValue::Number(a0().log10()),
            "exp" => JsValue::Number(a0().exp()),
            "pow" => JsValue::Number(a0().powf(a1())),
            "random" => JsValue::Number(0.5), // deterministic stub
            "sign" => JsValue::Number(a0().signum()),
            "trunc" => JsValue::Number(a0().trunc()),
            "sin" => JsValue::Number(a0().sin()),
            "cos" => JsValue::Number(a0().cos()),
            "tan" => JsValue::Number(a0().tan()),
            "atan" => JsValue::Number(a0().atan()),
            "atan2" => JsValue::Number(a0().atan2(a1())),
            "min" => {
                let v = args
                    .iter()
                    .map(Self::value_to_number)
                    .fold(f64::INFINITY, f64::min);
                JsValue::Number(v)
            }
            "max" => {
                let v = args
                    .iter()
                    .map(Self::value_to_number)
                    .fold(f64::NEG_INFINITY, f64::max);
                JsValue::Number(v)
            }
            "hypot" => {
                let v = args
                    .iter()
                    .map(|a| Self::value_to_number(a).powi(2))
                    .sum::<f64>()
                    .sqrt();
                JsValue::Number(v)
            }
            _ => JsValue::Undefined,
        }
    }

    fn call_object_define_property(&mut self, arguments: &[Expression]) -> JsValue {
        let Some(target_expr) = arguments.first() else {
            return JsValue::Undefined;
        };
        let mut target = self.execute_expression(target_expr);
        let key = arguments
            .get(1)
            .map(|argument| self.execute_expression(argument))
            .map(|value| Self::value_to_string(&value))
            .unwrap_or_default();
        let descriptor = arguments
            .get(2)
            .map(|argument| self.execute_expression(argument))
            .unwrap_or(JsValue::Undefined);

        if let (JsValue::Object(map), JsValue::Object(desc)) = (&mut target, descriptor) {
            if let Some(val) = desc.get("value") {
                map.insert(key, val.clone());
            } else if let Some(JsValue::Function(getter)) = desc.get("get").cloned() {
                let value = self.call_function(getter, vec![]);
                map.insert(key, value);
            }
            self.assign_target(target_expr, target.clone());
            target
        } else {
            JsValue::Undefined
        }
    }

    fn call_object_static(&mut self, name: &str, args: Vec<JsValue>) -> JsValue {
        match name {
            "keys" => {
                if let Some(JsValue::Object(map)) = args.into_iter().next() {
                    let mut keys: Vec<JsValue> =
                        map.keys().map(|k| JsValue::String(k.clone())).collect();
                    keys.sort_by(|a, b| Self::value_to_string(a).cmp(&Self::value_to_string(b)));
                    JsValue::Array(keys)
                } else {
                    JsValue::Array(vec![])
                }
            }
            "values" => {
                if let Some(JsValue::Object(map)) = args.into_iter().next() {
                    let mut pairs: Vec<(String, JsValue)> = map.into_iter().collect();
                    pairs.sort_by(|a, b| a.0.cmp(&b.0));
                    JsValue::Array(pairs.into_iter().map(|(_, v)| v).collect())
                } else {
                    JsValue::Array(vec![])
                }
            }
            "entries" => {
                if let Some(JsValue::Object(map)) = args.into_iter().next() {
                    let mut pairs: Vec<(String, JsValue)> = map.into_iter().collect();
                    pairs.sort_by(|a, b| a.0.cmp(&b.0));
                    JsValue::Array(
                        pairs
                            .into_iter()
                            .map(|(k, v)| JsValue::Array(vec![JsValue::String(k), v]))
                            .collect(),
                    )
                } else {
                    JsValue::Array(vec![])
                }
            }
            "assign" => {
                let mut iter = args.into_iter();
                let mut target = match iter.next() {
                    Some(JsValue::Object(m)) => m,
                    _ => return JsValue::Undefined,
                };
                for src in iter {
                    if let JsValue::Object(m) = src {
                        for (k, v) in m {
                            target.insert(k, v);
                        }
                    }
                }
                JsValue::Object(target)
            }
            "fromEntries" => {
                let mut map = HashMap::new();
                if let Some(JsValue::Array(entries)) = args.into_iter().next() {
                    for entry in entries {
                        if let JsValue::Array(pair) = entry {
                            let k =
                                Self::value_to_string(pair.first().unwrap_or(&JsValue::Undefined));
                            let v = pair.get(1).cloned().unwrap_or(JsValue::Undefined);
                            map.insert(k, v);
                        }
                    }
                }
                JsValue::Object(map)
            }
            "defineProperty" => {
                // Object.defineProperty(obj, key, descriptor) — apply value if present
                let mut iter = args.into_iter();
                let obj = iter.next().unwrap_or(JsValue::Undefined);
                let key = Self::value_to_string(&iter.next().unwrap_or(JsValue::Undefined));
                let descriptor = iter.next().unwrap_or(JsValue::Undefined);
                if let (JsValue::Object(mut map), JsValue::Object(desc)) = (obj, descriptor) {
                    if let Some(val) = desc.get("value") {
                        map.insert(key, val.clone());
                    } else if let Some(JsValue::Function(getter)) = desc.get("get").cloned() {
                        let v = self.call_function(getter, vec![]);
                        map.insert(key, v);
                    }
                    JsValue::Object(map)
                } else {
                    JsValue::Undefined
                }
            }
            "defineProperties"
            | "getOwnPropertyDescriptor"
            | "getOwnPropertyNames"
            | "getOwnPropertySymbols"
            | "getPrototypeOf"
            | "setPrototypeOf" => args.into_iter().next().unwrap_or(JsValue::Undefined),
            "create" => JsValue::Object(HashMap::new()), // ignore prototype arg
            "freeze" | "seal" | "preventExtensions" => {
                args.into_iter().next().unwrap_or(JsValue::Undefined)
            }
            "isFrozen" | "isSealed" => JsValue::Boolean(false),
            "hasOwn" => {
                if let (Some(JsValue::Object(m)), Some(k)) = (args.first(), args.get(1)) {
                    JsValue::Boolean(m.contains_key(Self::value_to_string(k).as_str()))
                } else {
                    JsValue::Boolean(false)
                }
            }
            _ => JsValue::Undefined,
        }
    }

    fn call_array_static(&self, name: &str, args: Vec<JsValue>) -> JsValue {
        match name {
            "isArray" => JsValue::Boolean(matches!(args.first(), Some(JsValue::Array(_)))),
            "from" => match args.into_iter().next() {
                Some(JsValue::Array(a)) => JsValue::Array(a),
                Some(JsValue::String(s)) => {
                    JsValue::Array(s.chars().map(|c| JsValue::String(c.to_string())).collect())
                }
                Some(JsValue::NodeList(ids)) => {
                    JsValue::Array(ids.into_iter().map(JsValue::ElementRef).collect())
                }
                _ => JsValue::Array(vec![]),
            },
            "of" => JsValue::Array(args),
            _ => JsValue::Undefined,
        }
    }

    fn call_number_static(&self, name: &str, args: &[JsValue]) -> JsValue {
        let a0 = || Self::value_to_number(args.first().unwrap_or(&JsValue::Undefined));
        match name {
            "isNaN" => JsValue::Boolean(a0().is_nan()),
            "isFinite" => JsValue::Boolean(a0().is_finite()),
            "isInteger" => {
                let n = a0();
                JsValue::Boolean(n.is_finite() && n.fract() == 0.0)
            }
            "parseInt" => {
                let s = Self::value_to_string(args.first().unwrap_or(&JsValue::Undefined));
                match s.trim().parse::<i64>() {
                    Ok(n) => JsValue::Number(n as f64),
                    Err(_) => JsValue::Number(f64::NAN),
                }
            }
            "parseFloat" => {
                let s = Self::value_to_string(args.first().unwrap_or(&JsValue::Undefined));
                match s.trim().parse::<f64>() {
                    Ok(n) => JsValue::Number(n),
                    Err(_) => JsValue::Number(f64::NAN),
                }
            }
            "toFixed" => JsValue::String(format!("{:.0}", a0())),
            _ => JsValue::Undefined,
        }
    }

    fn call_array_method(
        &mut self,
        name: &str,
        arr: Vec<JsValue>,
        arguments: &[Expression],
    ) -> Option<JsValue> {
        match name {
            "join" => {
                let sep = arguments
                    .first()
                    .map(|a| Self::value_to_string(&self.execute_expression(a)))
                    .unwrap_or_else(|| ",".to_owned());
                let s = arr
                    .iter()
                    .map(Self::value_to_string)
                    .collect::<Vec<_>>()
                    .join(&sep);
                Some(JsValue::String(s))
            }
            "includes" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                Some(JsValue::Boolean(
                    arr.iter().any(|v| Self::js_equal(v, &needle)),
                ))
            }
            "indexOf" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                let idx = arr.iter().position(|v| Self::js_equal(v, &needle));
                Some(JsValue::Number(idx.map(|i| i as f64).unwrap_or(-1.0)))
            }
            "lastIndexOf" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                let idx = arr.iter().rposition(|v| Self::js_equal(v, &needle));
                Some(JsValue::Number(idx.map(|i| i as f64).unwrap_or(-1.0)))
            }
            "slice" => {
                let len = arr.len() as i64;
                let start = arguments
                    .first()
                    .map(|a| {
                        let n = Self::value_to_number(&self.execute_expression(a)) as i64;
                        if n < 0 {
                            (len + n).max(0) as usize
                        } else {
                            n.min(len) as usize
                        }
                    })
                    .unwrap_or(0);
                let end = arguments
                    .get(1)
                    .map(|a| {
                        let n = Self::value_to_number(&self.execute_expression(a)) as i64;
                        if n < 0 {
                            (len + n).max(0) as usize
                        } else {
                            n.min(len) as usize
                        }
                    })
                    .unwrap_or(arr.len());
                let end = end.max(start);
                Some(JsValue::Array(arr[start..end.min(arr.len())].to_vec()))
            }
            "concat" => {
                let mut result = arr;
                for arg in arguments {
                    let v = self.execute_expression(arg);
                    match v {
                        JsValue::Array(a) => result.extend(a),
                        other => result.push(other),
                    }
                }
                Some(JsValue::Array(result))
            }
            "reverse" => {
                let mut r = arr;
                r.reverse();
                Some(JsValue::Array(r))
            }
            "flat" => {
                let depth = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                    .unwrap_or(1);
                fn flat_arr(arr: Vec<JsValue>, depth: usize) -> Vec<JsValue> {
                    if depth == 0 {
                        return arr;
                    }
                    let mut out = Vec::new();
                    for v in arr {
                        if let JsValue::Array(inner) = v {
                            out.extend(flat_arr(inner, depth - 1));
                        } else {
                            out.push(v);
                        }
                    }
                    out
                }
                Some(JsValue::Array(flat_arr(arr, depth)))
            }
            "at" => {
                let idx = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as i64)
                    .unwrap_or(0);
                let len = arr.len() as i64;
                let i = if idx < 0 { len + idx } else { idx };
                Some(if i >= 0 && (i as usize) < arr.len() {
                    arr[i as usize].clone()
                } else {
                    JsValue::Undefined
                })
            }
            "forEach" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        self.call_function(func.clone(), vec![item, JsValue::Number(i as f64)]);
                        if matches!(
                            self.early_exit,
                            Some(EarlyExit::Break) | Some(EarlyExit::Continue)
                        ) {
                            self.early_exit = None;
                        }
                        if self.early_exit.is_some() {
                            break;
                        }
                    }
                }
                Some(JsValue::Undefined)
            }
            "map" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    let mut result = Vec::new();
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let v =
                            self.call_function(func.clone(), vec![item, JsValue::Number(i as f64)]);
                        if self.early_exit.is_some() {
                            break;
                        }
                        result.push(v);
                    }
                    Some(JsValue::Array(result))
                } else {
                    Some(JsValue::Array(arr))
                }
            }
            "filter" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    let mut result = Vec::new();
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let keep = self.call_function(
                            func.clone(),
                            vec![item.clone(), JsValue::Number(i as f64)],
                        );
                        if self.early_exit.is_some() {
                            break;
                        }
                        if Self::is_truthy(&keep) {
                            result.push(item);
                        }
                    }
                    Some(JsValue::Array(result))
                } else {
                    Some(JsValue::Array(arr))
                }
            }
            "reduce" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    let has_init = arguments.len() > 1;
                    let (mut acc, start) = if has_init {
                        (self.execute_expression(&arguments[1]), 0)
                    } else if arr.is_empty() {
                        return Some(JsValue::Undefined);
                    } else {
                        (arr[0].clone(), 1)
                    };
                    for (i, item) in arr.into_iter().enumerate().skip(start) {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        acc = self.call_function(
                            func.clone(),
                            vec![acc, item, JsValue::Number(i as f64)],
                        );
                        if self.early_exit.is_some() {
                            break;
                        }
                    }
                    Some(acc)
                } else {
                    Some(JsValue::Undefined)
                }
            }
            "reduceRight" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    let len = arr.len();
                    let has_init = arguments.len() > 1;
                    let (mut acc, end) = if has_init {
                        (self.execute_expression(&arguments[1]), len)
                    } else if arr.is_empty() {
                        return Some(JsValue::Undefined);
                    } else {
                        (arr[len - 1].clone(), len - 1)
                    };
                    for i in (0..end).rev() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        acc = self.call_function(
                            func.clone(),
                            vec![acc, arr[i].clone(), JsValue::Number(i as f64)],
                        );
                        if self.early_exit.is_some() {
                            break;
                        }
                    }
                    Some(acc)
                } else {
                    Some(JsValue::Undefined)
                }
            }
            "find" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let found = self.call_function(
                            func.clone(),
                            vec![item.clone(), JsValue::Number(i as f64)],
                        );
                        if self.early_exit.is_some() {
                            break;
                        }
                        if Self::is_truthy(&found) {
                            return Some(item);
                        }
                    }
                }
                Some(JsValue::Undefined)
            }
            "findIndex" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let found =
                            self.call_function(func.clone(), vec![item, JsValue::Number(i as f64)]);
                        if self.early_exit.is_some() {
                            break;
                        }
                        if Self::is_truthy(&found) {
                            return Some(JsValue::Number(i as f64));
                        }
                    }
                }
                Some(JsValue::Number(-1.0))
            }
            "some" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let v =
                            self.call_function(func.clone(), vec![item, JsValue::Number(i as f64)]);
                        if self.early_exit.is_some() {
                            break;
                        }
                        if Self::is_truthy(&v) {
                            return Some(JsValue::Boolean(true));
                        }
                    }
                }
                Some(JsValue::Boolean(false))
            }
            "every" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let v =
                            self.call_function(func.clone(), vec![item, JsValue::Number(i as f64)]);
                        if self.early_exit.is_some() {
                            break;
                        }
                        if !Self::is_truthy(&v) {
                            return Some(JsValue::Boolean(false));
                        }
                    }
                }
                Some(JsValue::Boolean(true))
            }
            "flatMap" => {
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    let mut result = Vec::new();
                    for (i, item) in arr.into_iter().enumerate() {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let v =
                            self.call_function(func.clone(), vec![item, JsValue::Number(i as f64)]);
                        if self.early_exit.is_some() {
                            break;
                        }
                        match v {
                            JsValue::Array(a) => result.extend(a),
                            other => result.push(other),
                        }
                    }
                    Some(JsValue::Array(result))
                } else {
                    Some(JsValue::Array(arr))
                }
            }
            "sort" => {
                let mut r = arr;
                let cb = arguments.first().map(|a| self.execute_expression(a));
                if let Some(JsValue::Function(func)) = cb {
                    // Insertion sort to avoid borrow issues with &mut self in closure
                    let len = r.len();
                    for i in 1..len {
                        if self.execution_budget_exhausted {
                            break;
                        }
                        let mut j = i;
                        while j > 0 {
                            if self.execution_budget_exhausted {
                                break;
                            }
                            let cmp = self
                                .call_function(func.clone(), vec![r[j - 1].clone(), r[j].clone()]);
                            if Self::value_to_number(&cmp) > 0.0 {
                                r.swap(j - 1, j);
                                j -= 1;
                            } else {
                                break;
                            }
                        }
                    }
                } else {
                    r.sort_by(|a, b| Self::value_to_string(a).cmp(&Self::value_to_string(b)));
                }
                Some(JsValue::Array(r))
            }
            "keys" => {
                let keys = (0..arr.len()).map(|i| JsValue::Number(i as f64)).collect();
                Some(JsValue::Array(keys))
            }
            "entries" => {
                let entries = arr
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| JsValue::Array(vec![JsValue::Number(i as f64), v]))
                    .collect();
                Some(JsValue::Array(entries))
            }
            "values" => Some(JsValue::Array(arr)),
            "fill" => {
                let val = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                Some(JsValue::Array(
                    arr.into_iter().map(|_| val.clone()).collect(),
                ))
            }
            _ => None,
        }
    }

    fn call_string_method(
        &mut self,
        name: &str,
        s: &str,
        arguments: &[Expression],
    ) -> Option<JsValue> {
        match name {
            "split" => {
                let sep = arguments.first().map(|a| self.execute_expression(a));
                let parts = match sep {
                    None | Some(JsValue::Undefined) => vec![s.to_owned()],
                    Some(JsValue::String(ref d)) if d.is_empty() => {
                        s.chars().map(|c| c.to_string()).collect()
                    }
                    Some(ref d) => {
                        let d = Self::value_to_string(d);
                        s.split(d.as_str()).map(str::to_owned).collect()
                    }
                };
                Some(JsValue::Array(
                    parts.into_iter().map(JsValue::String).collect(),
                ))
            }
            "trim" => Some(JsValue::String(s.trim().to_owned())),
            "trimStart" | "trimLeft" => Some(JsValue::String(s.trim_start().to_owned())),
            "trimEnd" | "trimRight" => Some(JsValue::String(s.trim_end().to_owned())),
            "toUpperCase" | "toLocaleUpperCase" => Some(JsValue::String(s.to_uppercase())),
            "toLowerCase" | "toLocaleLowerCase" => Some(JsValue::String(s.to_lowercase())),
            "includes" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                Some(JsValue::Boolean(
                    s.contains(Self::value_to_string(&needle).as_str()),
                ))
            }
            "startsWith" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                Some(JsValue::Boolean(
                    s.starts_with(Self::value_to_string(&needle).as_str()),
                ))
            }
            "endsWith" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                Some(JsValue::Boolean(
                    s.ends_with(Self::value_to_string(&needle).as_str()),
                ))
            }
            "indexOf" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                let n = Self::value_to_string(&needle);
                Some(JsValue::Number(
                    s.find(n.as_str()).map(|i| i as f64).unwrap_or(-1.0),
                ))
            }
            "lastIndexOf" => {
                let needle = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                let n = Self::value_to_string(&needle);
                Some(JsValue::Number(
                    s.rfind(n.as_str()).map(|i| i as f64).unwrap_or(-1.0),
                ))
            }
            "slice" | "substring" => {
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let raw_start = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as i64)
                    .unwrap_or(0);
                let start = if name == "slice" && raw_start < 0 {
                    (len + raw_start).max(0) as usize
                } else {
                    raw_start.max(0) as usize
                };
                let end = arguments
                    .get(1)
                    .map(|a| {
                        let n = Self::value_to_number(&self.execute_expression(a)) as i64;
                        if name == "slice" && n < 0 {
                            (len + n).max(0) as usize
                        } else {
                            n.max(0) as usize
                        }
                    })
                    .unwrap_or(chars.len());
                let start = start.min(chars.len());
                let end = end.min(chars.len()).max(start);
                Some(JsValue::String(chars[start..end].iter().collect()))
            }
            "replace" => {
                let pat = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                let rep = arguments
                    .get(1)
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::String(String::new()));
                let pat_s = Self::value_to_string(&pat);
                let rep_s = Self::value_to_string(&rep);
                Some(JsValue::String(s.replacen(pat_s.as_str(), &rep_s, 1)))
            }
            "replaceAll" => {
                let pat = arguments
                    .first()
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::Undefined);
                let rep = arguments
                    .get(1)
                    .map(|a| self.execute_expression(a))
                    .unwrap_or(JsValue::String(String::new()));
                Some(JsValue::String(s.replace(
                    Self::value_to_string(&pat).as_str(),
                    &Self::value_to_string(&rep),
                )))
            }
            "repeat" => {
                let n = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                    .unwrap_or(0);
                Some(JsValue::String(s.repeat(n)))
            }
            "padStart" => {
                let target_len = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                    .unwrap_or(0);
                let fill = arguments
                    .get(1)
                    .map(|a| Self::value_to_string(&self.execute_expression(a)))
                    .unwrap_or_else(|| " ".to_owned());
                let chars: Vec<char> = s.chars().collect();
                if chars.len() >= target_len {
                    return Some(JsValue::String(s.to_owned()));
                }
                let needed = target_len - chars.len();
                let fill_chars: Vec<char> = fill.chars().collect();
                let pad: String = fill_chars.iter().cycle().take(needed).collect();
                Some(JsValue::String(pad + s))
            }
            "padEnd" => {
                let target_len = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                    .unwrap_or(0);
                let fill = arguments
                    .get(1)
                    .map(|a| Self::value_to_string(&self.execute_expression(a)))
                    .unwrap_or_else(|| " ".to_owned());
                let chars: Vec<char> = s.chars().collect();
                if chars.len() >= target_len {
                    return Some(JsValue::String(s.to_owned()));
                }
                let needed = target_len - chars.len();
                let fill_chars: Vec<char> = fill.chars().collect();
                let pad: String = fill_chars.iter().cycle().take(needed).collect();
                Some(JsValue::String(s.to_owned() + &pad))
            }
            "charAt" => {
                let i = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                    .unwrap_or(0);
                let c = s.chars().nth(i).map(|c| c.to_string()).unwrap_or_default();
                Some(JsValue::String(c))
            }
            "charCodeAt" | "codePointAt" => {
                let i = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as usize)
                    .unwrap_or(0);
                let code = s
                    .chars()
                    .nth(i)
                    .map(|c| JsValue::Number(c as u32 as f64))
                    .unwrap_or(JsValue::Number(f64::NAN));
                Some(code)
            }
            "concat" => {
                let mut result = s.to_owned();
                for arg in arguments {
                    result.push_str(&Self::value_to_string(&self.execute_expression(arg)));
                }
                Some(JsValue::String(result))
            }
            "at" => {
                let chars: Vec<char> = s.chars().collect();
                let idx = arguments
                    .first()
                    .map(|a| Self::value_to_number(&self.execute_expression(a)) as i64)
                    .unwrap_or(0);
                let len = chars.len() as i64;
                let i = if idx < 0 { len + idx } else { idx };
                Some(if i >= 0 && (i as usize) < chars.len() {
                    JsValue::String(chars[i as usize].to_string())
                } else {
                    JsValue::Undefined
                })
            }
            "match" | "matchAll" => {
                // Simplified: return null (no regex engine)
                Some(JsValue::Null)
            }
            "search" => Some(JsValue::Number(-1.0)),
            "normalize" => Some(JsValue::String(s.to_owned())),
            _ => None,
        }
    }

    fn object_from_properties(
        &mut self,
        properties: &[ObjectProperty],
    ) -> HashMap<String, JsValue> {
        let mut object = HashMap::new();
        for property in properties {
            object.insert(
                property.key.clone(),
                self.execute_expression(&property.value),
            );
        }
        object
    }

    fn query_selector_first_id(&self, selector: &str) -> Option<String> {
        if let Some(id) = selector.strip_prefix('#') {
            self.dom.query_selector_by_id.get(id).cloned()
        } else if let Some(class_name) = selector.strip_prefix('.') {
            self.dom.query_selector_by_class.get(class_name).cloned()
        } else {
            None
        }
    }

    fn query_selector_all_ids(&self, selector: &str) -> Vec<String> {
        if let Some(id) = selector.strip_prefix('#') {
            self.dom
                .query_selector_by_id
                .get(id)
                .cloned()
                .into_iter()
                .collect()
        } else if let Some(class_name) = selector.strip_prefix('.') {
            self.dom
                .query_selector_all_by_class
                .get(class_name)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn assign_target(&mut self, target: &Expression, value: JsValue) {
        if let Some((element_id, property)) = document_get_element_member(target) {
            let element_ref = existing_element_ref(&element_id);
            self.assign_element_property(&element_ref, &property, value);
            return;
        }

        // Computed member assignment: obj[key] = val, arr[idx] = val
        if let Expression::Member {
            object,
            property: MemberProperty::Computed(key_expr),
            ..
        } = target
        {
            let key = Self::value_to_string(&self.execute_expression(key_expr));
            let receiver = self.execute_expression(object);
            match receiver {
                JsValue::Object(mut map) => {
                    map.insert(key, value);
                    self.assign_target(object, JsValue::Object(map));
                }
                JsValue::Array(mut arr) => {
                    if let Ok(idx) = key.parse::<usize>() {
                        if idx >= arr.len() {
                            arr.resize(idx + 1, JsValue::Undefined);
                        }
                        arr[idx] = value;
                        self.assign_target(object, JsValue::Array(arr));
                    }
                }
                JsValue::WindowRef => {
                    self.globals.insert(key, value);
                }
                _ => {}
            }
            return;
        }

        if let Some((object, property)) = member_assignment_target(target) {
            let receiver = self.execute_expression(object);
            match receiver {
                JsValue::ElementRef(element_ref) => {
                    self.assign_element_property(&element_ref, &property, value);
                    return;
                }
                JsValue::StyleRef(element_id) => {
                    self.assign_style_property(&element_id, &property, value);
                    return;
                }
                JsValue::StorageRef(kind) => {
                    let value = Self::value_to_string(&value);
                    self.storage_map_mut(kind).insert(property, value);
                    return;
                }
                JsValue::Object(mut map) => {
                    map.insert(property, value);
                    self.assign_target(object, JsValue::Object(map));
                    return;
                }
                JsValue::Function(mut func) => {
                    func.properties.insert(property, value);
                    self.assign_target(object, JsValue::Function(func));
                    return;
                }
                _ => {}
            }
        }

        if let Some(global_name) = extract_window_global_name(target) {
            self.globals.insert(global_name, value);
            return;
        }

        // anyExpr.method = fn  →  store as array method override
        // Handles three cases:
        //   window.X.method = fn          → keyed "X:method"  (named dot access)
        //   window["X"].method = fn       → keyed "X:method"  (computed bracket access)
        //   (window["X"] = ...).method    → keyed "X:method"  (assignment expression)
        //   localVar.method = fn          → keyed "localVar:method", and also propagated to any
        //                                   global that currently holds the same array value
        //                                   (bridges the var d = window.X = []; d.push = r pattern)
        if let Expression::Member {
            object,
            property: MemberProperty::Named(method_name),
            ..
        } = target
        {
            if let Some(global_name) = extract_window_global_name(object) {
                self.array_method_overrides
                    .insert(format!("{global_name}:{method_name}"), value);
                return;
            }
            if let Expression::Identifier(varname) = object.as_ref() {
                if let Some(arr_val @ JsValue::Array(_)) = self.get_binding(varname) {
                    // Store by local var name (for direct calls on the same variable)
                    self.array_method_overrides
                        .insert(format!("{varname}:{method_name}"), value.clone());
                    // Propagate to every global currently holding the same array value
                    let matching_globals: Vec<String> = self
                        .globals
                        .iter()
                        .filter(|(_, v)| *v == &arr_val)
                        .map(|(k, _)| k.clone())
                        .collect();
                    for global_name in matching_globals {
                        self.array_method_overrides
                            .insert(format!("{global_name}:{method_name}"), value.clone());
                    }
                    return;
                }
            }
        }

        if let Expression::Identifier(name) = target {
            // Before overwriting, capture the old value so we can propagate the update
            // to any Object in scope that holds it as an entry (handles the Webpack pattern:
            // `var module = installedModules[id] = {...}` where both sides are aliases).
            let old_val = self.get_binding(name).unwrap_or(JsValue::Undefined);
            self.set_binding(name, value.clone());
            if matches!(&old_val, JsValue::Object(_)) {
                self.propagate_object_alias_update(old_val, value);
            }
        } else if matches!(target, Expression::This) {
            self.set_binding("this", value);
        }
    }

    /// When an Object identifier is overwritten with a new value, scan all accessible
    /// Object bindings for entries equal to the old value and update those entries to
    /// the new value. This gives shallow alias semantics for the Webpack pattern:
    ///   `var module = installedModules[id] = {exports:{}}` — both sides start equal;
    /// when writeback updates `module`, `installedModules[id]` must reflect the change.
    fn propagate_object_alias_update(&mut self, old_val: JsValue, new_val: JsValue) {
        // Scan globals
        let global_keys: Vec<String> = self.globals.keys().cloned().collect();
        for key in global_keys {
            if let Some(JsValue::Object(mut map)) = self.globals.get(&key).cloned() {
                let mut changed = false;
                for v in map.values_mut() {
                    if Self::safe_objects_equal(v, &old_val) {
                        *v = new_val.clone();
                        changed = true;
                    }
                }
                if changed {
                    self.globals.insert(key, JsValue::Object(map));
                }
            }
        }
        // Scan all stack frames (including captured closure frames)
        for frame in &self.stack {
            let frame_keys: Vec<String> = frame.locals.borrow().keys().cloned().collect();
            for key in frame_keys {
                let current = frame.locals.borrow().get(&key).cloned();
                if let Some(JsValue::Object(mut map)) = current {
                    let mut changed = false;
                    for v in map.values_mut() {
                        if Self::safe_objects_equal(v, &old_val) {
                            *v = new_val.clone();
                            changed = true;
                        }
                    }
                    if changed {
                        frame.locals.borrow_mut().insert(key, JsValue::Object(map));
                    }
                }
            }
        }
    }

    /// Structural equality check that never recurses into Function values.
    /// When a named function declaration runs in statement order, any method override
    /// that was stored with the earlier hoisted (empty-closure) version of the same
    /// function is refreshed with the now-complete closure. Matching is done by
    /// `JsFunction::name` to avoid comparing closures (which can cause stack overflows).
    fn refresh_overrides_for_named_func(&mut self, func_name: &str, new_val: JsValue) {
        for val in self.array_method_overrides.values_mut() {
            if let JsValue::Function(f) = val {
                if f.name.as_deref() == Some(func_name) {
                    *val = new_val.clone();
                }
            }
        }
    }

    /// Any comparison involving a Function (or other complex host type) returns false,
    /// preventing the PartialEq stack overflow that occurs when closures contain
    /// self-referential captured frames (e.g. a function stored in exports that
    /// captures the same globals map containing it).
    fn safe_objects_equal(a: &JsValue, b: &JsValue) -> bool {
        match (a, b) {
            (JsValue::Undefined, JsValue::Undefined) | (JsValue::Null, JsValue::Null) => true,
            (JsValue::Boolean(x), JsValue::Boolean(y)) => x == y,
            (JsValue::Number(x), JsValue::Number(y)) => x == y,
            (JsValue::String(x), JsValue::String(y)) => x == y,
            (JsValue::Object(a), JsValue::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|bv| Self::safe_objects_equal(v, bv)))
            }
            (JsValue::Array(a), JsValue::Array(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(av, bv)| Self::safe_objects_equal(av, bv))
            }
            _ => false,
        }
    }

    fn assign_element_property(&mut self, element_ref: &str, property: &str, value: JsValue) {
        let value = Self::value_to_string(&value);
        if dom_property_is_text_content(property) {
            self.set_element_text_content(element_ref, value);
        } else if dom_property_is_inner_html(property) {
            self.set_element_inner_html(element_ref, value);
        } else {
            self.set_element_attribute(
                element_ref,
                dom_property_to_attribute_name(property),
                value,
            );
        }
    }

    fn assign_style_property(&mut self, element_id: &str, js_prop: &str, value: JsValue) {
        let css_prop = js_style_prop_to_css(js_prop);
        let css_value = Self::value_to_string(&value);
        let element_ref = existing_element_ref(element_id);
        let existing = self
            .get_element_attribute(&element_ref, "style")
            .unwrap_or_default();
        let merged = merge_inline_style(&existing, &css_prop, &css_value);
        self.set_element_attribute(&element_ref, "style", merged);
    }

    fn eval_member(&mut self, expression: &Expression) -> JsValue {
        if let Some(global_name) = extract_window_global_name(expression) {
            return self.globals.get(&global_name).cloned().unwrap_or_else(|| {
                match global_name.as_str() {
                    "ActiveXObject" => JsValue::HostFunction("ActiveXObject".into()),
                    "external" => {
                        let mut map = HashMap::new();
                        map.insert(
                            "msActiveXFilteringEnabled".to_owned(),
                            JsValue::HostFunction("msActiveXFilteringEnabled".into()),
                        );
                        JsValue::Object(map)
                    }
                    _ => JsValue::Undefined,
                }
            });
        }
        let Expression::Member {
            object,
            property,
            optional,
        } = expression
        else {
            return JsValue::Undefined;
        };
        // Optional chaining: null?.foo → undefined
        if *optional {
            let receiver = self.execute_expression(object);
            if matches!(receiver, JsValue::Null | JsValue::Undefined) {
                return JsValue::Undefined;
            }
        }
        match property {
            MemberProperty::Computed(index_expr) => {
                let receiver = self.execute_expression(object);
                let index = self.execute_expression(index_expr);
                if matches!(receiver, JsValue::Undefined | JsValue::Null) {
                    self.trace_member_read(
                        object,
                        "[computed]",
                        &receiver,
                        &JsValue::Undefined,
                        false,
                    );
                }
                match receiver {
                    JsValue::Proxy { target, get } => {
                        let key = Self::value_to_string(&index);
                        self.proxy_get_property(*target, get, &key)
                    }
                    JsValue::Array(items) => {
                        let idx = Self::value_to_number(&index);
                        if idx >= 0.0 && idx.fract() == 0.0 {
                            items
                                .get(idx as usize)
                                .cloned()
                                .unwrap_or(JsValue::Undefined)
                        } else {
                            JsValue::Undefined
                        }
                    }
                    JsValue::NodeList(ids) => {
                        let idx = Self::value_to_number(&index);
                        if idx >= 0.0 && idx.fract() == 0.0 {
                            ids.get(idx as usize)
                                .map(|id| JsValue::ElementRef(existing_element_ref(id)))
                                .unwrap_or(JsValue::Undefined)
                        } else {
                            JsValue::Undefined
                        }
                    }
                    JsValue::Object(map) => {
                        let key = Self::value_to_string(&index);
                        map.get(&key).cloned().unwrap_or(JsValue::Undefined)
                    }
                    JsValue::String(s) => {
                        let idx = Self::value_to_number(&index);
                        if idx >= 0.0 && idx.fract() == 0.0 {
                            s.chars()
                                .nth(idx as usize)
                                .map(|c| JsValue::String(c.to_string()))
                                .unwrap_or(JsValue::Undefined)
                        } else {
                            JsValue::Undefined
                        }
                    }
                    JsValue::WindowRef => {
                        let key = Self::value_to_string(&index);
                        self.globals.get(&key).cloned().unwrap_or(JsValue::Undefined)
                    }
                    _ => JsValue::Undefined,
                }
            }
            MemberProperty::Named(property) => {
                // Static namespace constants (Math.PI, etc.) — check raw expression before eval
                if let Expression::Identifier(obj_name) = object.as_ref() {
                    match obj_name.as_str() {
                        "document" if property == "body" => {
                            return JsValue::ElementRef(existing_element_ref("body"));
                        }
                        "document" if property == "head" => {
                            return JsValue::ElementRef(existing_element_ref("head"));
                        }
                        "document" if property == "documentElement" => {
                            return JsValue::ElementRef(existing_element_ref("html"));
                        }
                        "document" if property == "readyState" => {
                            return JsValue::String("complete".to_owned());
                        }
                        "window" if property == "ActiveXObject" => {
                            return JsValue::HostFunction("ActiveXObject".into());
                        }
                        "Object" if property == "prototype" => {
                            return Self::native_prototype_object("Object");
                        }
                        "Array" if property == "prototype" => {
                            return Self::native_prototype_object("Array");
                        }
                        "String" if property == "prototype" => {
                            return Self::native_prototype_object("String");
                        }
                        "Function" if property == "prototype" => {
                            return Self::native_prototype_object("Function");
                        }
                        "Symbol" => {
                            return match property.as_str() {
                                "iterator" => JsValue::String("Symbol(Symbol.iterator)".into()),
                                "toPrimitive" => {
                                    JsValue::String("Symbol(Symbol.toPrimitive)".into())
                                }
                                "toStringTag" => {
                                    JsValue::String("Symbol(Symbol.toStringTag)".into())
                                }
                                "hasInstance" => {
                                    JsValue::String("Symbol(Symbol.hasInstance)".into())
                                }
                                "species" => JsValue::String("Symbol(Symbol.species)".into()),
                                "asyncIterator" => {
                                    JsValue::String("Symbol(Symbol.asyncIterator)".into())
                                }
                                "for" => JsValue::HostFunction("Symbol.for".into()),
                                "keyFor" => JsValue::HostFunction("Symbol.keyFor".into()),
                                _ => JsValue::Undefined,
                            };
                        }
                        "Math" => {
                            return match property.as_str() {
                                "PI" => JsValue::Number(std::f64::consts::PI),
                                "E" => JsValue::Number(std::f64::consts::E),
                                "LN2" => JsValue::Number(std::f64::consts::LN_2),
                                "LN10" => JsValue::Number(std::f64::consts::LN_10),
                                "LOG2E" => JsValue::Number(std::f64::consts::LOG2_E),
                                "LOG10E" => JsValue::Number(std::f64::consts::LOG10_E),
                                "SQRT2" => JsValue::Number(std::f64::consts::SQRT_2),
                                "SQRT1_2" => JsValue::Number(1.0 / std::f64::consts::SQRT_2),
                                _ => JsValue::Undefined,
                            };
                        }
                        "Number" => {
                            return match property.as_str() {
                                "MAX_SAFE_INTEGER" => JsValue::Number(9007199254740991.0),
                                "MIN_SAFE_INTEGER" => JsValue::Number(-9007199254740991.0),
                                "MAX_VALUE" => JsValue::Number(f64::MAX),
                                "MIN_VALUE" => JsValue::Number(f64::MIN_POSITIVE),
                                "POSITIVE_INFINITY" => JsValue::Number(f64::INFINITY),
                                "NEGATIVE_INFINITY" => JsValue::Number(f64::NEG_INFINITY),
                                "NaN" => JsValue::Number(f64::NAN),
                                "EPSILON" => JsValue::Number(f64::EPSILON),
                                _ => JsValue::Undefined,
                            };
                        }
                        _ => {}
                    }
                }

                let receiver = self.execute_expression(object);
                let result = match receiver.clone() {
                    JsValue::Proxy { target, get } => {
                        self.proxy_get_property(*target, get, property)
                    }
                    JsValue::String(ref s) if property == "length" => {
                        JsValue::Number(s.chars().count() as f64)
                    }
                    JsValue::String(_) => self
                        .native_prototype_property("String", property)
                        .unwrap_or(JsValue::Undefined),
                    JsValue::ElementRef(element_ref) => {
                        if let Some(value) = self.native_prototype_property("Object", property) {
                            return value;
                        }
                        if property == "style" {
                            return if let Some(id) = existing_id_from_ref(&element_ref) {
                                JsValue::StyleRef(id)
                            } else {
                                JsValue::Undefined
                            };
                        }
                        // Layout/position dimensions — safe zero stubs (no layout engine).
                        if matches!(
                            property.as_str(),
                            "offsetWidth"
                                | "offsetHeight"
                                | "clientWidth"
                                | "clientHeight"
                                | "scrollWidth"
                                | "scrollHeight"
                                | "offsetTop"
                                | "offsetLeft"
                                | "scrollTop"
                                | "scrollLeft"
                                | "clientTop"
                                | "clientLeft"
                        ) {
                            return JsValue::Number(0.0);
                        }
                        // DOM tree traversal — safe null/empty stubs.
                        if matches!(
                            property.as_str(),
                            "parentNode" | "parentElement" | "offsetParent"
                        ) {
                            return JsValue::Null;
                        }
                        if matches!(
                            property.as_str(),
                            "children" | "childNodes" | "childElementCount"
                        ) {
                            return JsValue::Array(vec![]);
                        }
                        if matches!(
                            property.as_str(),
                            "firstChild"
                                | "lastChild"
                                | "firstElementChild"
                                | "lastElementChild"
                                | "nextSibling"
                                | "previousSibling"
                                | "nextElementSibling"
                                | "previousElementSibling"
                        ) {
                            return JsValue::Null;
                        }
                        if property == "nodeType" {
                            return JsValue::Number(1.0);
                        }
                        if dom_property_is_text_content(property) {
                            JsValue::String(
                                self.get_element_text_content(&element_ref)
                                    .unwrap_or_default(),
                            )
                        } else if dom_property_is_inner_html(property) {
                            JsValue::String(
                                self.get_element_inner_html(&element_ref)
                                    .unwrap_or_default(),
                            )
                        } else {
                            JsValue::String(
                                self.get_element_attribute(
                                    &element_ref,
                                    dom_property_to_attribute_name(property),
                                )
                                .unwrap_or_default(),
                            )
                        }
                    }
                    JsValue::DocumentRef => {
                        if property == "body" {
                            JsValue::ElementRef(existing_element_ref("body"))
                        } else if property == "head" {
                            JsValue::ElementRef(existing_element_ref("head"))
                        } else if property == "documentElement" {
                            JsValue::ElementRef(existing_element_ref("html"))
                        } else if property == "readyState" {
                            JsValue::String("complete".to_owned())
                        } else if property == "cookie" {
                            JsValue::String(String::new())
                        } else if property == "title" {
                            JsValue::String(String::new())
                        } else if property == "URL" || property == "referrer" || property == "domain" {
                            JsValue::String(String::new())
                        } else {
                            JsValue::Undefined
                        }
                    }
                    JsValue::NavigatorRef => self
                        .globals
                        .get("__navigatorData")
                        .and_then(|value| {
                            if let JsValue::Object(map) = value {
                                map.get(property).cloned()
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| Self::navigator_soft_failure_property(property)),
                    JsValue::WindowRef => {
                        self.globals.get(property).cloned().unwrap_or_else(|| {
                            match property.as_str() {
                                "external" => {
                                    let mut map = HashMap::new();
                                    map.insert(
                                        "msActiveXFilteringEnabled".to_owned(),
                                        JsValue::HostFunction("msActiveXFilteringEnabled".into()),
                                    );
                                    JsValue::Object(map)
                                }
                                "msActiveXFilteringEnabled" => {
                                    JsValue::HostFunction("msActiveXFilteringEnabled".into())
                                }
                                "AudioContext"
                                | "webkitAudioContext"
                                | "OfflineAudioContext"
                                | "webkitOfflineAudioContext" => {
                                    JsValue::HostFunction(property.clone())
                                }
                                "Symbol" => JsValue::HostFunction("Symbol".into()),
                                "performance" => {
                                    let mut map = HashMap::new();
                                    map.insert(
                                        "now".to_owned(),
                                        JsValue::HostFunction("performance.now".into()),
                                    );
                                    JsValue::Object(map)
                                }
                                "globalThis" => JsValue::WindowRef,
                                _ => JsValue::Undefined,
                            }
                        })
                    }
                    JsValue::StyleRef(element_id) => {
                        let element_ref = existing_element_ref(&element_id);
                        let inline = self
                            .get_element_attribute(&element_ref, "style")
                            .unwrap_or_default();
                        let css_prop = js_style_prop_to_css(property);
                        JsValue::String(
                            parse_inline_style_map(&inline)
                                .into_iter()
                                .find(|(k, _)| *k == css_prop)
                                .map(|(_, v)| v)
                                .unwrap_or_default(),
                        )
                    }
                    JsValue::StorageRef(kind) => {
                        if property == "length" {
                            JsValue::Number(self.storage_map(&kind).len() as f64)
                        } else {
                            self.storage_map(&kind)
                                .get(property.as_str())
                                .cloned()
                                .map(JsValue::String)
                                .unwrap_or(JsValue::Undefined)
                        }
                    }
                    JsValue::CanvasContextRef(_) => match property.as_str() {
                        "VENDOR" | "RENDERER" | "VERSION" | "SHADING_LANGUAGE_VERSION" => {
                            JsValue::String(property.clone())
                        }
                        _ => self
                            .native_prototype_property("Object", property)
                            .unwrap_or(JsValue::Undefined),
                    },
                    JsValue::HostFunction(name) => match property.as_str() {
                        "call" | "apply" | "bind" => JsValue::HostFunction(name.clone()),
                        "prototype" => Self::constructor_prototype_object(&name)
                            .unwrap_or_else(|| Self::host_function_prototype(&name)),
                        _ => self
                            .native_prototype_property("Function", property)
                            .unwrap_or(JsValue::Undefined),
                    },
                    JsValue::BoundHostFunction { .. } => match property.as_str() {
                        "call" | "apply" | "bind" => {
                            JsValue::HostFunction(format!("Function.prototype.{property}"))
                        }
                        _ => self
                            .native_prototype_property("Function", property)
                            .unwrap_or(JsValue::Undefined),
                    },
                    JsValue::HostObject(name) => {
                        let value = Self::host_object_property(&name, property);
                        if matches!(value, JsValue::Undefined) {
                            self.native_prototype_property("Object", property)
                                .unwrap_or(JsValue::Undefined)
                        } else {
                            value
                        }
                    }
                    JsValue::Object(map) => self
                        .object_property_or_native_fallback(&map, property)
                        .unwrap_or(JsValue::Undefined),
                    JsValue::Function(func) => {
                        if property == "prototype" {
                            func.properties
                                .get(property.as_str())
                                .cloned()
                                .unwrap_or_else(|| JsValue::Object(HashMap::new()))
                        } else if matches!(property.as_str(), "call" | "apply" | "bind") {
                            JsValue::HostFunction(format!("Function.prototype.{property}"))
                        } else {
                            func.properties
                                .get(property.as_str())
                                .cloned()
                                .or_else(|| self.native_prototype_property("Function", property))
                                .unwrap_or(JsValue::Undefined)
                        }
                    }
                    JsValue::NodeList(items) if property == "length" => {
                        JsValue::Number(items.len() as f64)
                    }
                    JsValue::Array(items) if property == "length" => {
                        JsValue::Number(items.len() as f64)
                    }
                    JsValue::Array(_) => self
                        .native_prototype_property("Array", property)
                        .unwrap_or(JsValue::Undefined),
                    _ => JsValue::Undefined,
                };
                let prototype_attempted =
                    Self::member_prototype_fallback_owner(&receiver).is_some();
                self.trace_member_read(object, property, &receiver, &result, prototype_attempted);
                result
            }
        }
    }

    fn proxy_get_property(
        &mut self,
        target: JsValue,
        get: Option<JsFunction>,
        property: &str,
    ) -> JsValue {
        if let Some(getter) = get {
            return self.call_function(getter, vec![target, JsValue::String(property.to_owned())]);
        }
        match target {
            JsValue::Object(map) => map.get(property).cloned().unwrap_or(JsValue::Undefined),
            JsValue::Array(items) if property == "length" => JsValue::Number(items.len() as f64),
            _ => JsValue::Undefined,
        }
    }

    fn create_element(&mut self, tag_name: String) -> JsValue {
        self.dom.next_created_id += 1;
        let element_ref = format!("created:{}", self.dom.next_created_id);
        self.dom.created_elements.insert(
            element_ref.clone(),
            DomElementSnapshot {
                tag_name,
                ..Default::default()
            },
        );
        JsValue::ElementRef(element_ref)
    }

    fn create_text_node(&mut self, text_content: String) -> JsValue {
        self.dom.next_created_id += 1;
        let element_ref = format!("created:{}", self.dom.next_created_id);
        self.dom.created_elements.insert(
            element_ref.clone(),
            DomElementSnapshot {
                tag_name: "#text".to_owned(),
                text_content,
                ..Default::default()
            },
        );
        JsValue::ElementRef(element_ref)
    }

    fn create_comment_node(&mut self, text_content: String) -> JsValue {
        self.dom.next_created_id += 1;
        let element_ref = format!("created:{}", self.dom.next_created_id);
        self.dom.created_elements.insert(
            element_ref.clone(),
            DomElementSnapshot {
                tag_name: "#comment".to_owned(),
                text_content,
                ..Default::default()
            },
        );
        JsValue::ElementRef(element_ref)
    }

    fn append_child(&mut self, parent_ref: &str, child_ref: &str) {
        let Some(child) = self.dom.created_elements.get(child_ref).cloned() else {
            return;
        };
        if let Some(parent_id) = existing_id_from_ref(parent_ref) {
            self.effects
                .push(BrowserEffect::AppendChild { parent_id, child });
        }
    }

    fn insert_before(&mut self, parent_ref: &str, child_ref: &str) {
        self.append_child(parent_ref, child_ref);
    }

    fn remove_child(&mut self, _parent_ref: &str, _child_ref: &str) {
        // No explicit remove effect exists yet; consuming the call lets renderers
        // continue through Vue/React mount sequences that manage anchor nodes.
    }

    fn set_element_text_content(&mut self, element_ref: &str, value: String) {
        if let Some(element) = self.dom.created_elements.get_mut(element_ref) {
            element.text_content = value;
        } else if let Some(element_id) = existing_id_from_ref(element_ref) {
            self.dom
                .text_content_by_id
                .insert(element_id.clone(), value.clone());
            self.effects
                .push(BrowserEffect::SetTextContent { element_id, value });
        }
    }

    fn set_element_inner_html(&mut self, element_ref: &str, value: String) {
        if let Some(element) = self.dom.created_elements.get_mut(element_ref) {
            element.inner_html = value;
            element.text_content.clear();
            element.children.clear();
        } else if let Some(element_id) = existing_id_from_ref(element_ref) {
            self.dom
                .inner_html_by_id
                .insert(element_id.clone(), value.clone());
            self.effects
                .push(BrowserEffect::SetInnerHtml { element_id, value });
        }
    }

    fn get_element_inner_html(&self, element_ref: &str) -> Option<String> {
        if let Some(element) = self.dom.created_elements.get(element_ref) {
            Some(element.inner_html.clone())
        } else {
            existing_id_from_ref(element_ref)
                .and_then(|id| self.dom.inner_html_by_id.get(&id).cloned())
        }
    }

    fn get_element_text_content(&self, element_ref: &str) -> Option<String> {
        if let Some(element) = self.dom.created_elements.get(element_ref) {
            Some(element.text_content.clone())
        } else {
            existing_id_from_ref(element_ref)
                .and_then(|id| self.dom.text_content_by_id.get(&id).cloned())
        }
    }

    fn set_element_attribute(&mut self, element_ref: &str, name: &str, value: String) {
        if let Some(element) = self.dom.created_elements.get_mut(element_ref) {
            element.attributes.insert(name.to_owned(), value);
        } else if let Some(element_id) = existing_id_from_ref(element_ref) {
            self.dom
                .attributes_by_id
                .entry(element_id.clone())
                .or_default()
                .insert(name.to_owned(), value.clone());
            self.effects.push(BrowserEffect::SetAttribute {
                element_id,
                name: name.to_owned(),
                value,
            });
        }
    }

    fn get_element_attribute(&self, element_ref: &str, name: &str) -> Option<String> {
        if let Some(element) = self.dom.created_elements.get(element_ref) {
            element.attributes.get(name).cloned()
        } else {
            existing_id_from_ref(element_ref)
                .and_then(|id| self.dom.attributes_by_id.get(&id)?.get(name).cloned())
        }
    }

    fn element_tag_name(&self, element_ref: &str) -> Option<&str> {
        if let Some(element) = self.dom.created_elements.get(element_ref) {
            Some(element.tag_name.as_str())
        } else {
            None
        }
    }

    fn get_binding(&self, name: &str) -> Option<JsValue> {
        for frame in self.stack.iter().rev() {
            if let Some(value) = frame.locals.borrow().get(name).cloned() {
                return Some(value);
            }
        }
        self.globals.get(name).cloned()
    }

    fn get_identifier_value(&self, name: &str) -> JsValue {
        self.get_binding(name).unwrap_or_else(|| match name {
            "document" => JsValue::DocumentRef,
            "window" => JsValue::WindowRef,
            "navigator" => JsValue::NavigatorRef,
            "globalThis" => JsValue::WindowRef,
            "ActiveXObject" => JsValue::HostFunction("ActiveXObject".into()),
            "Function" => JsValue::HostFunction("Function".into()),
            "Symbol" => JsValue::HostFunction("Symbol".into()),
            "AudioContext"
            | "webkitAudioContext"
            | "OfflineAudioContext"
            | "webkitOfflineAudioContext" => JsValue::HostFunction(name.to_owned()),
            _ => JsValue::Undefined,
        })
    }

    fn set_binding(&mut self, name: &str, value: JsValue) {
        for frame in self.stack.iter().rev() {
            if frame.locals.borrow().contains_key(name) {
                frame.locals.borrow_mut().insert(name.to_owned(), value);
                return;
            }
        }
        self.set_local(name, value);
    }

    fn set_local(&mut self, name: &str, value: JsValue) {
        self.ensure_global_frame();
        if let Some(frame) = self.stack.last() {
            frame.locals.borrow_mut().insert(name.to_owned(), value);
        }
    }

    // `var` is function-scoped: skip block frames and land in the nearest function frame.
    fn set_var(&mut self, name: &str, value: JsValue) {
        for frame in self.stack.iter().rev() {
            if frame.is_function_scope {
                frame.locals.borrow_mut().insert(name.to_owned(), value);
                return;
            }
        }
        self.set_local(name, value);
    }

    fn ensure_global_frame(&mut self) {
        if self.stack.is_empty() {
            self.stack.push(StackFrame::function_scope());
        }
    }

    fn eval_args(&mut self, arguments: &[Expression]) -> Vec<JsValue> {
        let mut out = Vec::new();
        for arg in arguments {
            if let Expression::Spread(inner) = arg {
                let val = self.execute_expression(inner);
                if let JsValue::Array(items) = val {
                    out.extend(items);
                } else {
                    out.push(val);
                }
            } else {
                out.push(self.execute_expression(arg));
            }
        }
        out
    }

    fn bind_params(&mut self, params: &[Param], args: Vec<JsValue>) {
        let mut arg_idx = 0;
        for param in params {
            if param.rest {
                let rest: Vec<JsValue> = args.into_iter().skip(arg_idx).collect();
                let binding = param.binding.clone();
                self.execute_binding(&binding, JsValue::Array(rest));
                return;
            }
            let raw = args.get(arg_idx).cloned().unwrap_or(JsValue::Undefined);
            let val = if matches!(raw, JsValue::Undefined) {
                if let Some(expr) = &param.default {
                    let expr = expr.clone();
                    self.execute_expression(&expr)
                } else {
                    JsValue::Undefined
                }
            } else {
                raw
            };
            let binding = param.binding.clone();
            self.execute_binding(&binding, val);
            arg_idx += 1;
        }
    }

    fn call_function(&mut self, func: JsFunction, args: Vec<JsValue>) -> JsValue {
        self.call_function_with_this(func, args, JsValue::Undefined)
            .0
    }

    fn call_function_with_this(
        &mut self,
        func: JsFunction,
        args: Vec<JsValue>,
        this_value: JsValue,
    ) -> (JsValue, JsValue) {
        // Move captured frames directly — no clone needed since func is owned.
        let saved_stack = std::mem::replace(&mut self.stack, func.captured);
        self.ensure_global_frame();
        self.stack.push(StackFrame::function_scope());
        self.set_local("this", this_value);
        self.bind_params(&func.params, args);
        let result = match func.body {
            FunctionBody::Block(block) => {
                self.hoist_function_declarations(&block.body);
                for stmt in &block.body {
                    self.execute_statement(stmt);
                    if self.early_exit.is_some() {
                        break;
                    }
                }
                match self.early_exit.take() {
                    Some(EarlyExit::Return(v)) => v,
                    Some(throw @ EarlyExit::Throw(_)) => {
                        self.early_exit = Some(throw);
                        JsValue::Undefined
                    }
                    _ => JsValue::Undefined,
                }
            }
            FunctionBody::Expr(expr) => self.execute_expression(&expr),
        };
        let this_after = self.get_identifier_value("this");
        self.stack.pop();
        self.ensure_global_frame();
        let _ = std::mem::replace(&mut self.stack, saved_stack);
        (result, this_after)
    }

    /// Like `call_function` but writes back mutated Object/Array params to the
    /// original argument expressions after the call. This gives reference-like
    /// semantics for object arguments passed via `.call()` / `.apply()`, which
    /// is required for Webpack's module factory pattern:
    ///   `factory.call(exports, module, exports, require)`
    ///   Inside: `exports.foo = 1` — must propagate back to the caller's `exports`.
    fn call_function_with_writeback(
        &mut self,
        func: JsFunction,
        args: Vec<JsValue>,
        arg_exprs: &[Expression],
        this_value: JsValue,
    ) -> JsValue {
        // Collect simple param names in order (destructuring params are skipped).
        let param_names: Vec<Option<String>> = func
            .params
            .iter()
            .map(|p| {
                if let Binding::Name(n) = &p.binding {
                    Some(n.clone())
                } else {
                    None
                }
            })
            .collect();

        // Snapshot the initial arg values so we can detect which params changed.
        let initial_values: Vec<JsValue> = args.clone();

        let saved_stack = std::mem::replace(&mut self.stack, func.captured);
        self.ensure_global_frame();
        self.stack.push(StackFrame::function_scope());
        self.set_local("this", this_value);
        self.bind_params(&func.params, args);
        let result = match func.body {
            FunctionBody::Block(block) => {
                self.hoist_function_declarations(&block.body);
                for stmt in &block.body {
                    self.execute_statement(stmt);
                    if self.early_exit.is_some() {
                        break;
                    }
                }
                match self.early_exit.take() {
                    Some(EarlyExit::Return(v)) => v,
                    Some(throw @ EarlyExit::Throw(_)) => {
                        self.early_exit = Some(throw);
                        JsValue::Undefined
                    }
                    _ => JsValue::Undefined,
                }
            }
            FunctionBody::Expr(expr) => self.execute_expression(&expr),
        };

        // Snapshot final param values before frame is destroyed.
        let final_values: Vec<Option<JsValue>> = param_names
            .iter()
            .map(|name_opt| name_opt.as_deref().and_then(|n| self.get_binding(n)))
            .collect();

        self.stack.pop();
        self.ensure_global_frame();
        let _ = std::mem::replace(&mut self.stack, saved_stack);

        // Write back only params that changed.  Skipping unchanged params avoids overwriting
        // sibling writebacks: e.g. if `module.exports = fn` was written through the `module`
        // param, the unchanged `exports` clone must not clobber `module.exports`.
        // Function values are never written back: comparing them recurses through captured
        // stack frames and can cause a stack overflow when a closure captures the global scope.
        // In practice, Webpack function exports always reach the caller through a mutated
        // `module.exports` (Object), which IS written back correctly.
        for ((final_val_opt, initial_val), arg_expr) in final_values
            .iter()
            .zip(initial_values.iter())
            .zip(arg_exprs.iter())
        {
            if let Some(final_val) = final_val_opt {
                if Self::writeback_value_changed(initial_val, final_val) {
                    self.assign_target(arg_expr, final_val.clone());
                }
            }
        }

        result
    }

    fn execute_binary(
        &mut self,
        op: &BinaryOperator,
        left: &Expression,
        right: &Expression,
    ) -> JsValue {
        // Short-circuit operators — evaluate right side only when needed.
        match op {
            BinaryOperator::LogicalAnd => {
                let left = self.execute_expression(left);
                if !Self::is_truthy(&left) {
                    return left;
                }
                return self.execute_expression(right);
            }
            BinaryOperator::LogicalOr => {
                let left = self.execute_expression(left);
                if Self::is_truthy(&left) {
                    return left;
                }
                return self.execute_expression(right);
            }
            BinaryOperator::NullishCoalescing => {
                let left = self.execute_expression(left);
                return if matches!(left, JsValue::Null | JsValue::Undefined) {
                    self.execute_expression(right)
                } else {
                    left
                };
            }
            _ => {}
        }

        let lv = self.execute_expression(left);
        let rv = self.execute_expression(right);

        match op {
            BinaryOperator::Add => match (&lv, &rv) {
                (JsValue::Number(a), JsValue::Number(b)) => JsValue::Number(a + b),
                _ => JsValue::String(format!(
                    "{}{}",
                    Self::value_to_string(&lv),
                    Self::value_to_string(&rv)
                )),
            },
            BinaryOperator::Subtract => {
                JsValue::Number(Self::value_to_number(&lv) - Self::value_to_number(&rv))
            }
            BinaryOperator::Multiply => {
                JsValue::Number(Self::value_to_number(&lv) * Self::value_to_number(&rv))
            }
            BinaryOperator::Divide => {
                JsValue::Number(Self::value_to_number(&lv) / Self::value_to_number(&rv))
            }
            BinaryOperator::Remainder => {
                JsValue::Number(Self::value_to_number(&lv) % Self::value_to_number(&rv))
            }
            BinaryOperator::Less => {
                JsValue::Boolean(Self::value_to_number(&lv) < Self::value_to_number(&rv))
            }
            BinaryOperator::LessEqual => {
                JsValue::Boolean(Self::value_to_number(&lv) <= Self::value_to_number(&rv))
            }
            BinaryOperator::Greater => {
                JsValue::Boolean(Self::value_to_number(&lv) > Self::value_to_number(&rv))
            }
            BinaryOperator::GreaterEqual => {
                JsValue::Boolean(Self::value_to_number(&lv) >= Self::value_to_number(&rv))
            }
            BinaryOperator::Equal | BinaryOperator::StrictEqual => {
                JsValue::Boolean(Self::values_equal(&lv, &rv))
            }
            BinaryOperator::NotEqual | BinaryOperator::StrictNotEqual => {
                JsValue::Boolean(!Self::values_equal(&lv, &rv))
            }
            BinaryOperator::BitXor => {
                let l = Self::value_to_number(&lv) as i64;
                let r = Self::value_to_number(&rv) as i64;
                JsValue::Number((l ^ r) as f64)
            }
            BinaryOperator::BitAnd => JsValue::Number(
                ((Self::value_to_number(&lv) as i64) & (Self::value_to_number(&rv) as i64)) as f64,
            ),
            BinaryOperator::BitOr => JsValue::Number(
                ((Self::value_to_number(&lv) as i64) | (Self::value_to_number(&rv) as i64)) as f64,
            ),
            BinaryOperator::ShiftLeft => JsValue::Number(
                (((Self::value_to_number(&lv) as i32) << (Self::value_to_number(&rv) as u32 & 31))
                    as i32) as f64,
            ),
            BinaryOperator::ShiftRight => JsValue::Number(
                (((Self::value_to_number(&lv) as i32) >> (Self::value_to_number(&rv) as u32 & 31))
                    as i32) as f64,
            ),
            BinaryOperator::UnsignedShiftRight => JsValue::Number(
                ((Self::value_to_number(&lv) as u32) >> (Self::value_to_number(&rv) as u32 & 31))
                    as f64,
            ),
            BinaryOperator::Exponent => {
                JsValue::Number(Self::value_to_number(&lv).powf(Self::value_to_number(&rv)))
            }
            BinaryOperator::Instanceof => JsValue::Boolean(false),
            BinaryOperator::In => match &rv {
                JsValue::Object(map) => {
                    JsValue::Boolean(map.contains_key(&Self::value_to_string(&lv)))
                }
                _ => JsValue::Boolean(false),
            },
            BinaryOperator::LogicalAnd
            | BinaryOperator::LogicalOr
            | BinaryOperator::NullishCoalescing => {
                unreachable!("handled above")
            }
        }
    }

    fn is_truthy(value: &JsValue) -> bool {
        match value {
            JsValue::Undefined | JsValue::Null => false,
            JsValue::Boolean(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            JsValue::Object(_)
            | JsValue::Array(_)
            | JsValue::Function(_)
            | JsValue::ElementRef(_)
            | JsValue::NodeList(_)
            | JsValue::StyleRef(_)
            | JsValue::StorageRef(_)
            | JsValue::DocumentRef
            | JsValue::WindowRef
            | JsValue::NavigatorRef
            | JsValue::HostFunction(_)
            | JsValue::BoundHostFunction { .. }
            | JsValue::HostObject(_)
            | JsValue::RegExp { .. }
            | JsValue::CanvasContextRef(_)
            | JsValue::DateInstance
            | JsValue::ResolvedPromise
            | JsValue::XhrInstance { .. }
            | JsValue::Proxy { .. }
            | JsValue::WeakMap(_) => true,
        }
    }

    fn value_to_number(value: &JsValue) -> f64 {
        match value {
            JsValue::Number(n) => *n,
            JsValue::Boolean(true) => 1.0,
            JsValue::Boolean(false) => 0.0,
            JsValue::String(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
            JsValue::Null => 0.0,
            JsValue::Array(items) if items.is_empty() => 0.0,
            JsValue::Undefined
            | JsValue::Object(_)
            | JsValue::Array(_)
            | JsValue::Function(_)
            | JsValue::ElementRef(_)
            | JsValue::NodeList(_)
            | JsValue::StyleRef(_)
            | JsValue::StorageRef(_)
            | JsValue::DocumentRef
            | JsValue::WindowRef
            | JsValue::NavigatorRef
            | JsValue::HostFunction(_)
            | JsValue::BoundHostFunction { .. }
            | JsValue::HostObject(_)
            | JsValue::RegExp { .. }
            | JsValue::CanvasContextRef(_)
            | JsValue::DateInstance
            | JsValue::ResolvedPromise
            | JsValue::XhrInstance { .. }
            | JsValue::Proxy { .. }
            | JsValue::WeakMap(_) => f64::NAN,
        }
    }

    fn weak_map_key(value: &JsValue) -> String {
        match value {
            JsValue::Object(map) => {
                let mut pairs: Vec<_> = map.iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(b.0));
                let body = pairs
                    .into_iter()
                    .map(|(key, value)| format!("{key}:{}", Self::value_to_string(value)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("object:{{{body}}}")
            }
            _ => Self::value_to_string(value),
        }
    }

    fn values_equal(a: &JsValue, b: &JsValue) -> bool {
        match (a, b) {
            (JsValue::Undefined, JsValue::Undefined)
            | (JsValue::Null, JsValue::Null)
            | (JsValue::Undefined, JsValue::Null)
            | (JsValue::Null, JsValue::Undefined) => true,
            (JsValue::Boolean(a), JsValue::Boolean(b)) => a == b,
            (JsValue::Number(a), JsValue::Number(b)) => a == b,
            (JsValue::String(a), JsValue::String(b)) => a == b,
            _ => false,
        }
    }

    fn value_type_str(value: &JsValue) -> String {
        match value {
            JsValue::Undefined => "undefined",
            JsValue::Null => "object",
            JsValue::Boolean(_) => "boolean",
            JsValue::Number(_) => "number",
            JsValue::String(_) => "string",
            JsValue::Function(_) => "function",
            JsValue::HostFunction(_) => "function",
            JsValue::BoundHostFunction { .. } => "function",
            _ => "object",
        }
        .to_owned()
    }

    fn value_to_string(value: &JsValue) -> String {
        match value {
            JsValue::Undefined => "undefined".to_owned(),
            JsValue::Null => "null".to_owned(),
            JsValue::Boolean(value) => value.to_string(),
            JsValue::Number(value) => {
                if value.fract() == 0.0 && value.is_finite() {
                    (*value as i64).to_string()
                } else {
                    value.to_string()
                }
            }
            JsValue::String(value) => value.clone(),
            JsValue::Array(items) => items
                .iter()
                .map(|v| Self::value_to_string(v))
                .collect::<Vec<_>>()
                .join(","),
            JsValue::Object(_) => "[object Object]".to_owned(),
            JsValue::Function(_) => "[object Function]".to_owned(),
            JsValue::ElementRef(_) => "[object Element]".to_owned(),
            JsValue::NodeList(_) => "[object NodeList]".to_owned(),
            JsValue::StyleRef(_) => "[object CSSStyleDeclaration]".to_owned(),
            JsValue::StorageRef(_) => "[object Storage]".to_owned(),
            JsValue::DocumentRef => "[object Document]".to_owned(),
            JsValue::WindowRef => "[object Window]".to_owned(),
            JsValue::NavigatorRef => "[object Navigator]".to_owned(),
            JsValue::HostFunction(_) => "[object Function]".to_owned(),
            JsValue::BoundHostFunction { .. } => "[object Function]".to_owned(),
            JsValue::HostObject(name) => format!("[object {name}]"),
            JsValue::CanvasContextRef(_) => "[object CanvasRenderingContext]".to_owned(),
            JsValue::DateInstance => "[object Date]".to_owned(),
            JsValue::RegExp { pattern, flags } => format!("/{pattern}/{flags}"),
            JsValue::ResolvedPromise => "[object Promise]".to_owned(),
            JsValue::XhrInstance { .. } => "[object XMLHttpRequest]".to_owned(),
            JsValue::Proxy { .. } => "[object Object]".to_owned(),
            JsValue::WeakMap(_) => "[object WeakMap]".to_owned(),
        }
    }

    /// Returns true if a param value changed in a way that warrants writing back to the caller.
    /// Function values are never written back: their PartialEq descends into captured stack
    /// frames that may form reference cycles, causing a stack overflow.  Webpack function
    /// exports always travel through a mutated Object/Array (e.g. `module.exports = fn`),
    /// which is detected by the Object writeback path.
    fn writeback_value_changed(initial: &JsValue, final_val: &JsValue) -> bool {
        // Same variant heuristic — skip expensive deep comparison for functions.
        match (initial, final_val) {
            (JsValue::Function(_), JsValue::Function(_)) => false,
            (a, b) => a != b,
        }
    }

    fn js_equal(a: &JsValue, b: &JsValue) -> bool {
        match (a, b) {
            (JsValue::Undefined, JsValue::Undefined) | (JsValue::Null, JsValue::Null) => true,
            (JsValue::Boolean(x), JsValue::Boolean(y)) => x == y,
            (JsValue::Number(x), JsValue::Number(y)) => x == y,
            (JsValue::String(x), JsValue::String(y)) => x == y,
            _ => false,
        }
    }

    fn host_function_default_return(name: &str) -> JsValue {
        if matches!(
            name,
            "permissions.query"
                | "mediaDevices.enumerateDevices"
                | "mediaDevices.getUserMedia"
                | "navigator.getBattery"
                | "AudioContext.close"
                | "AudioContext.resume"
                | "AudioContext.suspend"
                | "OfflineAudioContext.startRendering"
        ) {
            JsValue::ResolvedPromise
        } else if name.ends_with("Enabled") || name.starts_with("ms") {
            JsValue::Boolean(false)
        } else if name.starts_with("Function.prototype.") {
            JsValue::HostFunction(name.to_owned())
        } else {
            JsValue::Undefined
        }
    }

    fn call_host_function(&mut self, name: &str, this_arg: JsValue, args: Vec<JsValue>) -> JsValue {
        match name {
            "Object.prototype.toString" => {
                JsValue::String(format!("[object {}]", Self::object_tag(&this_arg)))
            }
            "Object.prototype.valueOf" => this_arg,
            "Object.prototype.hasOwnProperty" => {
                let key = args.first().map(Self::value_to_string).unwrap_or_default();
                match this_arg {
                    JsValue::Object(map) => JsValue::Boolean(map.contains_key(&key)),
                    _ => JsValue::Boolean(false),
                }
            }
            "Array.prototype.push" => {
                if let JsValue::Array(mut items) = this_arg {
                    items.extend(args);
                    JsValue::Number(items.len() as f64)
                } else {
                    JsValue::Number(0.0)
                }
            }
            "Array.prototype.pop" => {
                if let JsValue::Array(mut items) = this_arg {
                    items.pop().unwrap_or(JsValue::Undefined)
                } else {
                    JsValue::Undefined
                }
            }
            "Array.prototype.shift" => {
                if let JsValue::Array(mut items) = this_arg {
                    if items.is_empty() {
                        JsValue::Undefined
                    } else {
                        items.remove(0)
                    }
                } else {
                    JsValue::Undefined
                }
            }
            "Array.prototype.unshift" => {
                if let JsValue::Array(mut items) = this_arg {
                    let added = args.len();
                    for value in args.into_iter().rev() {
                        items.insert(0, value);
                    }
                    JsValue::Number((items.len().max(added)) as f64)
                } else {
                    JsValue::Number(0.0)
                }
            }
            "Array.prototype.join" => {
                let sep = args
                    .first()
                    .map(Self::value_to_string)
                    .unwrap_or_else(|| ",".to_owned());
                if let JsValue::Array(items) = this_arg {
                    JsValue::String(
                        items
                            .iter()
                            .map(Self::value_to_string)
                            .collect::<Vec<_>>()
                            .join(&sep),
                    )
                } else {
                    JsValue::String(String::new())
                }
            }
            "Array.prototype.slice" => {
                if let JsValue::Array(items) = this_arg {
                    JsValue::Array(items)
                } else {
                    JsValue::Array(vec![])
                }
            }
            "Array.prototype.concat" => {
                let mut result = match this_arg {
                    JsValue::Array(items) => items,
                    other => vec![other],
                };
                for arg in args {
                    match arg {
                        JsValue::Array(items) => result.extend(items),
                        other => result.push(other),
                    }
                }
                JsValue::Array(result)
            }
            "Array.prototype.indexOf" => {
                let needle = args.first().cloned().unwrap_or(JsValue::Undefined);
                if let JsValue::Array(items) = this_arg {
                    JsValue::Number(
                        items
                            .iter()
                            .position(|value| Self::js_equal(value, &needle))
                            .map(|index| index as f64)
                            .unwrap_or(-1.0),
                    )
                } else {
                    JsValue::Number(-1.0)
                }
            }
            "Array.prototype.includes" => {
                let needle = args.first().cloned().unwrap_or(JsValue::Undefined);
                if let JsValue::Array(items) = this_arg {
                    JsValue::Boolean(items.iter().any(|value| Self::js_equal(value, &needle)))
                } else {
                    JsValue::Boolean(false)
                }
            }
            "Array.prototype.lastIndexOf" => {
                let needle = args.first().cloned().unwrap_or(JsValue::Undefined);
                if let JsValue::Array(items) = this_arg {
                    JsValue::Number(
                        items
                            .iter()
                            .rposition(|value| Self::js_equal(value, &needle))
                            .map(|index| index as f64)
                            .unwrap_or(-1.0),
                    )
                } else {
                    JsValue::Number(-1.0)
                }
            }
            "Array.prototype.toString" => {
                if let JsValue::Array(items) = this_arg {
                    JsValue::String(
                        items
                            .iter()
                            .map(Self::value_to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    )
                } else {
                    JsValue::String(String::new())
                }
            }
            "String.prototype.toString" => JsValue::String(Self::value_to_string(&this_arg)),
            "String.prototype.replace" => {
                let source = Self::value_to_string(&this_arg);
                let needle = args.first().map(Self::value_to_string).unwrap_or_default();
                let replacement = args.get(1).map(Self::value_to_string).unwrap_or_default();
                JsValue::String(source.replacen(&needle, &replacement, 1))
            }
            "String.prototype.indexOf" => {
                let source = Self::value_to_string(&this_arg);
                let needle = args.first().map(Self::value_to_string).unwrap_or_default();
                JsValue::Number(
                    source
                        .find(&needle)
                        .map(|index| index as f64)
                        .unwrap_or(-1.0),
                )
            }
            "String.prototype.includes" => {
                let source = Self::value_to_string(&this_arg);
                let needle = args.first().map(Self::value_to_string).unwrap_or_default();
                JsValue::Boolean(source.contains(&needle))
            }
            "String.prototype.slice" => {
                let source = Self::value_to_string(&this_arg);
                let len = source.chars().count() as i64;
                let start = args
                    .first()
                    .map(Self::value_to_number)
                    .map(|n| if n < 0.0 { len + n as i64 } else { n as i64 })
                    .unwrap_or(0)
                    .clamp(0, len) as usize;
                let end = args
                    .get(1)
                    .map(Self::value_to_number)
                    .map(|n| if n < 0.0 { len + n as i64 } else { n as i64 })
                    .unwrap_or(len)
                    .clamp(start as i64, len) as usize;
                JsValue::String(source.chars().skip(start).take(end - start).collect())
            }
            "String.prototype.trim" => {
                JsValue::String(Self::value_to_string(&this_arg).trim().to_owned())
            }
            "String.prototype.charAt" => {
                let source = Self::value_to_string(&this_arg);
                let index = args.first().map(Self::value_to_number).unwrap_or(0.0) as usize;
                JsValue::String(
                    source
                        .chars()
                        .nth(index)
                        .map(|ch| ch.to_string())
                        .unwrap_or_default(),
                )
            }
            "String.prototype.split" => {
                let source = Self::value_to_string(&this_arg);
                let sep = args.first().map(Self::value_to_string).unwrap_or_default();
                if sep.is_empty() {
                    JsValue::Array(
                        source
                            .chars()
                            .map(|ch| JsValue::String(ch.to_string()))
                            .collect(),
                    )
                } else {
                    JsValue::Array(
                        source
                            .split(&sep)
                            .map(|part| JsValue::String(part.to_owned()))
                            .collect(),
                    )
                }
            }
            "Function.prototype.call" => {
                let mut real_args = args;
                let call_this = if real_args.is_empty() {
                    JsValue::Undefined
                } else {
                    real_args.remove(0)
                };
                match this_arg {
                    JsValue::HostFunction(host_name) => {
                        self.call_host_function(&host_name, call_this, real_args)
                    }
                    JsValue::BoundHostFunction {
                        name,
                        this_arg,
                        mut bound_args,
                    } => {
                        bound_args.extend(real_args);
                        self.call_host_function(&name, *this_arg, bound_args)
                    }
                    _ => JsValue::Undefined,
                }
            }
            "Function.prototype.apply" => {
                let call_this = args.first().cloned().unwrap_or(JsValue::Undefined);
                let real_args = match args.get(1) {
                    Some(JsValue::Array(items)) => items.clone(),
                    _ => vec![],
                };
                match this_arg {
                    JsValue::HostFunction(host_name) => {
                        self.call_host_function(&host_name, call_this, real_args)
                    }
                    JsValue::BoundHostFunction {
                        name,
                        this_arg,
                        mut bound_args,
                    } => {
                        bound_args.extend(real_args);
                        self.call_host_function(&name, *this_arg, bound_args)
                    }
                    _ => JsValue::Undefined,
                }
            }
            "Function.prototype.bind" => {
                let mut real_args = args;
                let bound_this = if real_args.is_empty() {
                    JsValue::Undefined
                } else {
                    real_args.remove(0)
                };
                match this_arg {
                    JsValue::HostFunction(host_name) => JsValue::BoundHostFunction {
                        name: host_name,
                        this_arg: Box::new(bound_this),
                        bound_args: real_args,
                    },
                    JsValue::BoundHostFunction {
                        name,
                        this_arg,
                        mut bound_args,
                    } => {
                        bound_args.extend(real_args);
                        JsValue::BoundHostFunction {
                            name,
                            this_arg,
                            bound_args,
                        }
                    }
                    _ => JsValue::BoundHostFunction {
                        name: "Function.prototype.bind".to_owned(),
                        this_arg: Box::new(bound_this),
                        bound_args: real_args,
                    },
                }
            }
            "Function.prototype.toString" => {
                JsValue::String("function () { [native code] }".into())
            }
            "Symbol" => {
                self.symbol_counter += 1;
                let desc = args
                    .first()
                    .map(Self::value_to_string)
                    .unwrap_or_default();
                JsValue::String(format!("__sym_{}_{}", self.symbol_counter, desc))
            }
            "Symbol.for" => {
                let key = args
                    .first()
                    .map(Self::value_to_string)
                    .unwrap_or_default();
                JsValue::String(format!("__sym_for_{key}"))
            }
            "Symbol.keyFor" => JsValue::Undefined,
            "performance.now" => JsValue::Number(self.current_time_ms as f64),
            _ => Self::host_function_default_return(name),
        }
    }

    fn native_prototype_object(owner: &str) -> JsValue {
        let mut map = HashMap::new();
        for method in Self::native_prototype_methods(owner) {
            let value = if *method == "constructor" {
                JsValue::HostFunction(owner.to_owned())
            } else {
                JsValue::HostFunction(format!("{owner}.prototype.{method}"))
            };
            map.insert((*method).to_owned(), value);
        }
        JsValue::Object(map)
    }

    fn native_prototype_property(&mut self, owner: &str, property: &str) -> Option<JsValue> {
        if Self::native_prototype_methods(owner).contains(&property) {
            self.trace_runtime("prototype.lookup", format!("{owner}.prototype.{property}"));
            if property == "constructor" {
                Some(JsValue::HostFunction(owner.to_owned()))
            } else {
                Some(JsValue::HostFunction(format!(
                    "{owner}.prototype.{property}"
                )))
            }
        } else {
            None
        }
    }

    fn native_prototype_methods(owner: &str) -> &'static [&'static str] {
        match owner {
            "Object" => &["constructor", "toString", "valueOf", "hasOwnProperty"],
            "Array" => &[
                "constructor",
                "push",
                "pop",
                "shift",
                "unshift",
                "join",
                "slice",
                "concat",
                "indexOf",
                "lastIndexOf",
                "includes",
                "forEach",
                "map",
                "filter",
                "reduce",
                "some",
                "every",
                "find",
                "flat",
                "at",
                "toString",
            ],
            "String" => &[
                "constructor",
                "toString",
                "replace",
                "split",
                "includes",
                "indexOf",
                "slice",
                "trim",
                "charAt",
            ],
            "Function" => &["constructor", "call", "apply", "bind", "toString"],
            _ => &[],
        }
    }

    fn object_property_or_native_fallback(
        &mut self,
        map: &HashMap<String, JsValue>,
        property: &str,
    ) -> Option<JsValue> {
        match map.get(property) {
            Some(JsValue::Undefined) if Self::soft_native_shadow_property(property) => self
                .native_prototype_property("Object", property)
                .inspect(|_| {
                    self.trace_runtime(
                        "prototype.shadowed_undefined",
                        format!("Object.prototype.{property}"),
                    );
                }),
            Some(value) => Some(value.clone()),
            None => self.native_prototype_property("Object", property),
        }
    }

    fn soft_native_shadow_property(property: &str) -> bool {
        matches!(
            property,
            "toString" | "valueOf" | "hasOwnProperty" | "constructor"
        )
    }

    fn member_prototype_fallback_owner(value: &JsValue) -> Option<&'static str> {
        match value {
            JsValue::String(_) => Some("String"),
            JsValue::Array(_) => Some("Array"),
            JsValue::Function(_) | JsValue::HostFunction(_) | JsValue::BoundHostFunction { .. } => {
                Some("Function")
            }
            JsValue::Object(_)
            | JsValue::ElementRef(_)
            | JsValue::NodeList(_)
            | JsValue::StyleRef(_)
            | JsValue::StorageRef(_)
            | JsValue::DocumentRef
            | JsValue::WindowRef
            | JsValue::NavigatorRef
            | JsValue::HostObject(_)
            | JsValue::RegExp { .. }
            | JsValue::CanvasContextRef(_)
            | JsValue::DateInstance
            | JsValue::ResolvedPromise
            | JsValue::XhrInstance { .. }
            | JsValue::Proxy { .. }
            | JsValue::WeakMap(_) => Some("Object"),
            JsValue::Undefined | JsValue::Null | JsValue::Boolean(_) | JsValue::Number(_) => None,
        }
    }

    fn diagnostic_member_property(property: &str) -> bool {
        matches!(
            property,
            "toString"
                | "valueOf"
                | "hasOwnProperty"
                | "constructor"
                | "call"
                | "apply"
                | "bind"
                | "push"
                | "replace"
        )
    }

    fn object_tag(value: &JsValue) -> &'static str {
        match value {
            JsValue::Undefined => "Undefined",
            JsValue::Null => "Null",
            JsValue::Boolean(_) => "Boolean",
            JsValue::Number(_) => "Number",
            JsValue::String(_) => "String",
            JsValue::Object(_) | JsValue::Proxy { .. } => "Object",
            JsValue::Array(_) => "Array",
            JsValue::Function(_) | JsValue::HostFunction(_) | JsValue::BoundHostFunction { .. } => {
                "Function"
            }
            JsValue::ElementRef(_) => "Element",
            JsValue::NodeList(_) => "NodeList",
            JsValue::StyleRef(_) => "CSSStyleDeclaration",
            JsValue::StorageRef(_) => "Storage",
            JsValue::DocumentRef => "Document",
            JsValue::WindowRef => "Window",
            JsValue::NavigatorRef => "Navigator",
            JsValue::HostObject(_) => "Object",
            JsValue::RegExp { .. } => "RegExp",
            JsValue::CanvasContextRef(_) => "CanvasRenderingContext",
            JsValue::DateInstance => "Date",
            JsValue::ResolvedPromise => "Promise",
            JsValue::XhrInstance { .. } => "XMLHttpRequest",
            JsValue::WeakMap(_) => "WeakMap",
        }
    }

    fn host_function_prototype(name: &str) -> JsValue {
        let mut map = HashMap::new();
        for method in ["call", "apply", "bind"] {
            map.insert(
                method.to_owned(),
                JsValue::HostFunction(format!("{name}.prototype.{method}")),
            );
        }
        JsValue::Object(map)
    }

    fn constructor_prototype_object(name: &str) -> Option<JsValue> {
        match name {
            "Object" | "Array" | "String" | "Function" => Some(Self::native_prototype_object(name)),
            _ => None,
        }
    }

    fn navigator_soft_failure_property(property: &str) -> JsValue {
        match property {
            "permissions" => {
                let mut map = HashMap::new();
                map.insert(
                    "query".to_owned(),
                    JsValue::HostFunction("permissions.query".to_owned()),
                );
                JsValue::Object(map)
            }
            "mediaDevices" => {
                let mut map = HashMap::new();
                map.insert(
                    "enumerateDevices".to_owned(),
                    JsValue::HostFunction("mediaDevices.enumerateDevices".to_owned()),
                );
                map.insert(
                    "getUserMedia".to_owned(),
                    JsValue::HostFunction("mediaDevices.getUserMedia".to_owned()),
                );
                JsValue::Object(map)
            }
            "getBattery" => JsValue::HostFunction("navigator.getBattery".to_owned()),
            _ => JsValue::Undefined,
        }
    }

    fn host_object_property(name: &str, property: &str) -> JsValue {
        if name.contains("AudioContext") {
            return match property {
                "state" => JsValue::String("suspended".to_owned()),
                "sampleRate" => JsValue::Number(44100.0),
                "currentTime" => JsValue::Number(0.0),
                "destination" | "listener" => JsValue::HostObject("AudioNode".to_owned()),
                _ => JsValue::Undefined,
            };
        }
        JsValue::Undefined
    }

    fn host_object_method_return(name: &str, method_name: &str) -> JsValue {
        if name.contains("AudioContext") {
            return match method_name {
                "close" | "resume" | "suspend" => JsValue::ResolvedPromise,
                "startRendering" => JsValue::ResolvedPromise,
                "createAnalyser"
                | "createOscillator"
                | "createDynamicsCompressor"
                | "createGain"
                | "createScriptProcessor"
                | "createBiquadFilter"
                | "createConvolver"
                | "createDelay"
                | "createBufferSource" => JsValue::HostObject("AudioNode".to_owned()),
                "createBuffer" => JsValue::HostObject("AudioBuffer".to_owned()),
                _ => JsValue::Undefined,
            };
        }
        if name == "AudioNode" {
            return match method_name {
                "connect" => JsValue::HostObject("AudioNode".to_owned()),
                "disconnect" | "start" | "stop" => JsValue::Undefined,
                "getFloatFrequencyData" | "getByteFrequencyData" => JsValue::Undefined,
                _ => JsValue::Undefined,
            };
        }
        if name == "AudioBuffer" {
            return match method_name {
                "getChannelData" => JsValue::Array(vec![]),
                _ => JsValue::Undefined,
            };
        }
        JsValue::Undefined
    }

    fn soft_failure_constructor_name(name: &str) -> bool {
        matches!(
            name,
            "AudioContext"
                | "webkitAudioContext"
                | "OfflineAudioContext"
                | "webkitOfflineAudioContext"
        )
    }

    fn soft_failure_host_name(expr: &Expression) -> String {
        match expr {
            Expression::Identifier(name) => name.clone(),
            Expression::Member {
                property: MemberProperty::Named(name),
                ..
            } => name.clone(),
            _ => "HostObject".to_owned(),
        }
    }

    fn soft_failure_constructor_name_from_expr(expr: &Expression) -> Option<String> {
        match expr {
            Expression::Identifier(name) if Self::soft_failure_constructor_name(name) => {
                Some(name.clone())
            }
            Expression::Member {
                property: MemberProperty::Named(name),
                ..
            } if Self::soft_failure_constructor_name(name) => Some(name.clone()),
            _ => None,
        }
    }

    fn simple_regex_test(pattern: &str, flags: &str, haystack: &str) -> bool {
        let mut needle = pattern
            .trim_start_matches('^')
            .trim_end_matches('$')
            .replace("\\.", ".")
            .replace("\\/", "/")
            .replace("\\-", "-")
            .replace("\\_", "_")
            .replace("\\s", " ");
        needle = needle.replace(".*", "");
        let haystack = if flags.contains('i') {
            haystack.to_ascii_lowercase()
        } else {
            haystack.to_owned()
        };
        let needle = if flags.contains('i') {
            needle.to_ascii_lowercase()
        } else {
            needle
        };
        needle
            .split('|')
            .map(|part| part.trim_matches(|c| matches!(c, '(' | ')' | '[' | ']')))
            .filter(|part| !part.is_empty())
            .any(|part| haystack.contains(part))
    }

    /// Fire all registered handlers for `(element_id, event_type)`, returning any DOM effects.
    /// `key` is supplied for keyboard events.
    pub fn fire_event(
        &mut self,
        element_id: &str,
        event_type: &str,
        key: Option<&str>,
    ) -> Vec<BrowserEffect> {
        let matching_indices: Vec<usize> = self
            .event_handlers
            .iter()
            .enumerate()
            .filter(|(_, h)| h.element_id == element_id && h.event_type == event_type)
            .map(|(i, _)| i)
            .collect();

        for idx in matching_indices {
            let handler = self.event_handlers[idx].clone();

            // Swap in the handler's captured environment as the active stack.
            let saved_stack = std::mem::replace(&mut self.stack, handler.captured.clone());
            self.ensure_global_frame();

            // Push an invocation frame for parameters.
            self.stack.push(StackFrame::function_scope());
            if let Some(param_name) = handler.params.first() {
                let mut event_obj = HashMap::new();
                event_obj.insert("type".to_owned(), JsValue::String(event_type.to_owned()));
                event_obj.insert(
                    "target".to_owned(),
                    JsValue::ElementRef(existing_element_ref(element_id)),
                );
                if let Some(k) = key {
                    event_obj.insert("key".to_owned(), JsValue::String(k.to_owned()));
                }
                if let Some(frame) = self.stack.last() {
                    frame
                        .locals
                        .borrow_mut()
                        .insert(param_name.clone(), JsValue::Object(event_obj));
                }
            }

            // execute_block pushes/pops its own frame.
            self.execute_block(&handler.body);

            // Pop our invocation frame (execute_block already popped its own).
            self.stack.pop();
            self.ensure_global_frame();

            // Save the (possibly mutated) captured environment back so closures persist state.
            let updated_captured = std::mem::replace(&mut self.stack, saved_stack);
            self.event_handlers[idx].captured = updated_captured;
        }

        self.drain_effects()
    }

    pub fn has_listener(&self, element_id: &str, event_type: &str) -> bool {
        self.event_handlers
            .iter()
            .any(|h| h.element_id == element_id && h.event_type == event_type)
    }

    pub fn all_element_ids_with_listener(&self, event_type: &str) -> Vec<String> {
        self.event_handlers
            .iter()
            .filter(|h| h.event_type == event_type)
            .map(|h| h.element_id.clone())
            .collect()
    }

    pub fn has_pending_timers(&self) -> bool {
        !self.pending_timers.is_empty()
    }

    /// Fire all timers due at or before `elapsed_ms` milliseconds since page load.
    pub fn poll_timers(&mut self, elapsed_ms: u64) -> Vec<BrowserEffect> {
        self.current_time_ms = elapsed_ms;

        let mut due = Vec::new();
        let mut i = 0;
        while i < self.pending_timers.len() {
            if self.pending_timers[i].fires_at_ms <= elapsed_ms {
                due.push(self.pending_timers.remove(i));
            } else {
                i += 1;
            }
        }

        for timer in due {
            // Run against the live stack so callbacks see variables updated by microtasks.
            self.stack.push(StackFrame::function_scope());
            self.execute_block(&timer.body);
            self.stack.pop();
            self.ensure_global_frame();
        }

        self.drain_effects()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct MethodCall {
    receiver: MethodReceiver,
    object: Expression,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MethodReceiver {
    Document,
    Object,
}

fn method_call(expression: &Expression) -> Option<MethodCall> {
    let Expression::Member {
        object,
        property,
        optional: _,
    } = expression
    else {
        return None;
    };
    let MemberProperty::Named(name) = property else {
        return None;
    };
    let receiver = if matches!(object.as_ref(), Expression::Identifier(identifier) if identifier == "document")
    {
        MethodReceiver::Document
    } else {
        MethodReceiver::Object
    };
    Some(MethodCall {
        receiver,
        object: object.as_ref().clone(),
        name: name.clone(),
    })
}

fn document_get_element_member(expression: &Expression) -> Option<(String, String)> {
    let Expression::Member {
        object,
        property,
        optional: _,
    } = expression
    else {
        return None;
    };
    let MemberProperty::Named(property) = property else {
        return None;
    };
    let Expression::Call { callee, arguments } = object.as_ref() else {
        return None;
    };
    let Expression::Member {
        object: callee_object,
        property: callee_property,
        optional: _,
    } = callee.as_ref()
    else {
        return None;
    };
    if !matches!(callee_object.as_ref(), Expression::Identifier(name) if name == "document") {
        return None;
    }
    if !matches!(callee_property, MemberProperty::Named(name) if name == "getElementById") {
        return None;
    }
    let [Expression::String(element_id)] = arguments.as_slice() else {
        return None;
    };
    Some((element_id.clone(), property.clone()))
}

fn member_assignment_target(expression: &Expression) -> Option<(&Expression, String)> {
    let Expression::Member {
        object,
        property,
        optional: _,
    } = expression
    else {
        return None;
    };
    let MemberProperty::Named(property) = property else {
        return None;
    };
    Some((object.as_ref(), property.clone()))
}

/// For `window.X`, `window["X"]`, and `(window.X = ...)` / `(window["X"] = ...)` expressions,
/// return the global name `X`. Used to detect calls like `(window.webpackJsonp = ...).push(data)`.
fn extract_window_global_name(expr: &Expression) -> Option<String> {
    // window.X  (named dot access)
    if let Expression::Member {
        object,
        property: MemberProperty::Named(name),
        ..
    } = expr
    {
        if matches!(object.as_ref(), Expression::Identifier(n) if n == "window") {
            return Some(name.clone());
        }
    }
    // window["X"]  (computed bracket access)
    if let Expression::Member {
        object,
        property: MemberProperty::Computed(key_expr),
        ..
    } = expr
    {
        if matches!(object.as_ref(), Expression::Identifier(n) if n == "window") {
            match key_expr.as_ref() {
                Expression::String(name) | Expression::Identifier(name) => {
                    return Some(name.clone());
                }
                _ => {}
            }
        }
    }
    // (window.X = ...) or (window["X"] = ...)  (assignment expression — recurse into target)
    if let Expression::Assignment { target, .. } = expr {
        return extract_window_global_name(target);
    }
    None
}

fn constructor_like_member_name(expr: &Expression) -> Option<String> {
    let Expression::Member {
        property: MemberProperty::Named(name),
        ..
    } = expr
    else {
        return None;
    };
    if name
        .chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
    {
        Some(name.clone())
    } else {
        None
    }
}

fn existing_element_ref(id: &str) -> String {
    format!("existing:{id}")
}

fn existing_id_from_ref(element_ref: &str) -> Option<String> {
    element_ref.strip_prefix("existing:").map(str::to_owned)
}

fn parse_regex_literal(src: &str) -> (String, String) {
    if !src.starts_with('/') {
        return (src.to_owned(), String::new());
    }
    let mut escaped = false;
    let mut in_class = false;
    for (idx, ch) in src.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => {
                return (
                    src[1..idx].to_owned(),
                    src[idx + ch.len_utf8()..].to_owned(),
                );
            }
            _ => {}
        }
    }
    (src.trim_matches('/').to_owned(), String::new())
}

fn dom_property_is_text_content(property: &str) -> bool {
    matches!(property, "textContent" | "innerText" | "nodeValue")
}

fn dom_property_is_inner_html(property: &str) -> bool {
    property == "innerHTML"
}

fn dom_property_to_attribute_name(property: &str) -> &str {
    match property {
        "className" => "class",
        "htmlFor" => "for",
        other => other,
    }
}

fn json_parse_str(s: &str) -> JsValue {
    let bytes = s.trim().as_bytes();
    let mut pos = 0;
    json_parse_value(bytes, &mut pos)
}

fn json_skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

fn json_parse_value(bytes: &[u8], pos: &mut usize) -> JsValue {
    json_skip_ws(bytes, pos);
    match bytes.get(*pos) {
        Some(b'"') => json_parse_string(bytes, pos),
        Some(b'{') => json_parse_object(bytes, pos),
        Some(b'[') => json_parse_array(bytes, pos),
        Some(b't') => {
            *pos += 4;
            JsValue::Boolean(true)
        }
        Some(b'f') => {
            *pos += 5;
            JsValue::Boolean(false)
        }
        Some(b'n') => {
            *pos += 4;
            JsValue::Null
        }
        _ => json_parse_number(bytes, pos),
    }
}

fn json_parse_string(bytes: &[u8], pos: &mut usize) -> JsValue {
    *pos += 1; // skip opening "
    let mut s = String::new();
    while *pos < bytes.len() {
        match bytes[*pos] {
            b'"' => {
                *pos += 1;
                break;
            }
            b'\\' if *pos + 1 < bytes.len() => {
                *pos += 1;
                match bytes[*pos] {
                    b'"' => s.push('"'),
                    b'\\' => s.push('\\'),
                    b'/' => s.push('/'),
                    b'n' => s.push('\n'),
                    b'r' => s.push('\r'),
                    b't' => s.push('\t'),
                    ch => s.push(ch as char),
                }
                *pos += 1;
            }
            ch => {
                s.push(ch as char);
                *pos += 1;
            }
        }
    }
    JsValue::String(s)
}

fn json_parse_object(bytes: &[u8], pos: &mut usize) -> JsValue {
    *pos += 1; // skip {
    let mut map = HashMap::new();
    json_skip_ws(bytes, pos);
    if bytes.get(*pos) == Some(&b'}') {
        *pos += 1;
        return JsValue::Object(map);
    }
    loop {
        json_skip_ws(bytes, pos);
        let key = match json_parse_string(bytes, pos) {
            JsValue::String(k) => k,
            _ => break,
        };
        json_skip_ws(bytes, pos);
        if bytes.get(*pos) == Some(&b':') {
            *pos += 1;
        }
        let value = json_parse_value(bytes, pos);
        map.insert(key, value);
        json_skip_ws(bytes, pos);
        match bytes.get(*pos) {
            Some(b',') => {
                *pos += 1;
            }
            Some(b'}') => {
                *pos += 1;
                break;
            }
            _ => break,
        }
    }
    JsValue::Object(map)
}

fn json_parse_array(bytes: &[u8], pos: &mut usize) -> JsValue {
    *pos += 1; // skip [
    let mut items = Vec::new();
    json_skip_ws(bytes, pos);
    if bytes.get(*pos) == Some(&b']') {
        *pos += 1;
        return JsValue::Array(items);
    }
    loop {
        items.push(json_parse_value(bytes, pos));
        json_skip_ws(bytes, pos);
        match bytes.get(*pos) {
            Some(b',') => {
                *pos += 1;
            }
            Some(b']') => {
                *pos += 1;
                break;
            }
            _ => break,
        }
    }
    JsValue::Array(items)
}

fn json_parse_number(bytes: &[u8], pos: &mut usize) -> JsValue {
    let start = *pos;
    while *pos < bytes.len()
        && (bytes[*pos].is_ascii_digit() || matches!(bytes[*pos], b'-' | b'+' | b'.' | b'e' | b'E'))
    {
        *pos += 1;
    }
    let s = std::str::from_utf8(&bytes[start..*pos]).unwrap_or("0");
    JsValue::Number(s.parse::<f64>().unwrap_or(0.0))
}

fn json_stringify(value: &JsValue) -> String {
    match value {
        JsValue::Null | JsValue::Undefined => "null".to_owned(),
        JsValue::Boolean(b) => b.to_string(),
        JsValue::Number(n) => {
            if n.fract() == 0.0 && n.is_finite() {
                (*n as i64).to_string()
            } else {
                n.to_string()
            }
        }
        JsValue::String(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{escaped}\"")
        }
        JsValue::Array(items) => {
            let parts: Vec<String> = items.iter().map(json_stringify).collect();
            format!("[{}]", parts.join(","))
        }
        JsValue::Object(map) => {
            let mut pairs: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    let key = json_stringify(&JsValue::String(k.clone()));
                    format!("{key}:{}", json_stringify(v))
                })
                .collect();
            pairs.sort(); // stable key order for deterministic output
            format!("{{{}}}", pairs.join(","))
        }
        JsValue::Function(_)
        | JsValue::ElementRef(_)
        | JsValue::NodeList(_)
        | JsValue::StyleRef(_)
        | JsValue::StorageRef(_)
        | JsValue::DocumentRef
        | JsValue::WindowRef
        | JsValue::NavigatorRef
        | JsValue::HostFunction(_)
        | JsValue::BoundHostFunction { .. }
        | JsValue::HostObject(_)
        | JsValue::RegExp { .. }
        | JsValue::CanvasContextRef(_)
        | JsValue::DateInstance
        | JsValue::ResolvedPromise
        | JsValue::XhrInstance { .. }
        | JsValue::Proxy { .. }
        | JsValue::WeakMap(_) => "null".to_owned(),
    }
}

fn js_style_prop_to_css(prop: &str) -> String {
    let mut result = String::new();
    for ch in prop.chars() {
        if ch.is_uppercase() {
            result.push('-');
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

fn parse_inline_style_map(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|decl| {
            let decl = decl.trim();
            let colon = decl.find(':')?;
            let name = decl[..colon].trim().to_lowercase();
            let val = decl[colon + 1..].trim().to_owned();
            if name.is_empty() {
                None
            } else {
                Some((name, val))
            }
        })
        .collect()
}

fn merge_inline_style(existing: &str, prop: &str, value: &str) -> String {
    let mut props = parse_inline_style_map(existing);
    if let Some((_, v)) = props.iter_mut().find(|(k, _)| k == prop) {
        *v = value.to_owned();
    } else {
        props.push((prop.to_owned(), value.to_owned()));
    }
    props
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_text_content_assignment_effect() {
        let program =
            crate::parse_script(r#"document.getElementById("result").textContent = "After";"#)
                .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "After".to_owned(),
            }]
        );
    }

    #[test]
    fn preserves_window_state_across_programs() {
        let first = crate::parse_script(r#"window.value = "A";"#).expect("script should parse");
        let second = crate::parse_script(r#"window.value = window.value + "B";"#)
            .expect("script should parse");
        let third =
            crate::parse_script(r#"document.getElementById("result").textContent = window.value;"#)
                .expect("script should parse");
        let mut state = BrowserExecutionState::default();

        state.execute_program(&first);
        assert!(state.drain_effects().is_empty());
        state.execute_program(&second);
        assert!(state.drain_effects().is_empty());
        state.execute_program(&third);

        assert_eq!(
            state.drain_effects(),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "AB".to_owned(),
            }]
        );
        assert_eq!(
            state.dom.text_content_by_id.get("result"),
            Some(&"AB".to_owned())
        );
    }

    #[test]
    fn creates_element_and_appends_it_to_existing_parent() {
        let program = crate::parse_script(
            r#"
            let p = document.createElement("p");
            p.textContent = "Created by script";
            document.getElementById("root").appendChild(p);
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::AppendChild {
                parent_id: "root".to_owned(),
                child: DomElementSnapshot {
                    tag_name: "p".to_owned(),
                    text_content: "Created by script".to_owned(),
                    ..Default::default()
                },
            }]
        );
    }

    #[test]
    fn treats_element_property_assignment_as_attribute_mutation() {
        let program =
            crate::parse_script(r#"document.getElementById("box").className = "active";"#)
                .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetAttribute {
                element_id: "box".to_owned(),
                name: "class".to_owned(),
                value: "active".to_owned(),
            }]
        );
    }

    #[test]
    fn reflects_general_element_properties_to_attributes() {
        let program =
            crate::parse_script(r#"document.getElementById("box").ariaLabel = "Greeting";"#)
                .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetAttribute {
                element_id: "box".to_owned(),
                name: "ariaLabel".to_owned(),
                value: "Greeting".to_owned(),
            }]
        );
    }

    #[test]
    fn existing_element_set_attribute_can_be_read_back() {
        let program = crate::parse_script(
            r#"
            let box = document.getElementById("box");
            box.setAttribute("data-state", "ready");
            document.getElementById("result").textContent = box.getAttribute("data-state");
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![
                BrowserEffect::SetAttribute {
                    element_id: "box".to_owned(),
                    name: "data-state".to_owned(),
                    value: "ready".to_owned(),
                },
                BrowserEffect::SetTextContent {
                    element_id: "result".to_owned(),
                    value: "ready".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn inner_html_assignment_is_dom_effect() {
        let program = crate::parse_script(
            r#"document.getElementById("root").innerHTML = "<span>Hello</span>";"#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetInnerHtml {
                element_id: "root".to_owned(),
                value: "<span>Hello</span>".to_owned(),
            }]
        );
    }

    #[test]
    fn query_selector_reads_seeded_text_content() {
        let mut state = BrowserExecutionState::default();
        state.seed_existing_element("message", "Hello".to_owned(), HashMap::new());
        let program = crate::parse_script(
            r##"
            let el = document.querySelector("#message");
            document.getElementById("result").textContent = el.textContent;
            "##,
        )
        .expect("script should parse");

        state.execute_program(&program);

        assert_eq!(
            state.drain_effects(),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "Hello".to_owned(),
            }]
        );
    }

    #[test]
    fn for_loop_appends_three_items_with_correct_text() {
        let program = crate::parse_script(
            r#"
            let list = document.getElementById("list");
            for (let i = 0; i < 3; i = i + 1) {
                let li = document.createElement("li");
                li.textContent = "Item " + i;
                list.appendChild(li);
            }
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![
                BrowserEffect::AppendChild {
                    parent_id: "list".to_owned(),
                    child: DomElementSnapshot {
                        tag_name: "li".to_owned(),
                        text_content: "Item 0".to_owned(),
                        ..Default::default()
                    },
                },
                BrowserEffect::AppendChild {
                    parent_id: "list".to_owned(),
                    child: DomElementSnapshot {
                        tag_name: "li".to_owned(),
                        text_content: "Item 1".to_owned(),
                        ..Default::default()
                    },
                },
                BrowserEffect::AppendChild {
                    parent_id: "list".to_owned(),
                    child: DomElementSnapshot {
                        tag_name: "li".to_owned(),
                        text_content: "Item 2".to_owned(),
                        ..Default::default()
                    },
                },
            ]
        );
    }

    #[test]
    fn click_event_listener_fires_and_updates_text_content() {
        let program = crate::parse_script(
            r#"
            let button = document.getElementById("button");
            button.addEventListener("click", function () {
                document.getElementById("result").textContent = "Clicked";
            });
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        state.execute_program(&program);
        assert!(
            state.drain_effects().is_empty(),
            "no DOM effects at load time"
        );
        assert!(state.has_listener("button", "click"));

        let effects = state.fire_event("button", "click", None);
        assert_eq!(
            effects,
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "Clicked".to_owned(),
            }]
        );
    }

    #[test]
    fn legacy_attach_event_registers_event_handler() {
        let program = crate::parse_script(
            r#"
            let button = document.getElementById("button");
            button.attachEvent("onclick", function () {
                document.getElementById("result").textContent = "legacy";
            });
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        state.execute_program(&program);
        assert!(state.has_listener("button", "click"));

        let effects = state.fire_event("button", "click", None);
        assert_eq!(
            effects,
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "legacy".to_owned(),
            }]
        );
    }

    #[test]
    fn tag_name_collection_supports_indexing_and_item() {
        let effects = run(r#"
            let bodies = document.getElementsByTagName("body");
            document.getElementById("result").textContent =
                String(bodies.length) + ":" + String(bodies[0]) + ":" + String(bodies.item(0));
            "#);

        assert_eq!(
            effects,
            vec![text("result", "1:[object Element]:[object Element]")]
        );
    }

    #[test]
    fn legacy_host_constructor_is_callable_stub() {
        let effects = run(r#"
            let ax = new window.ActiveXObject("ShockwaveFlash.ShockwaveFlash");
            document.getElementById("result").textContent =
                typeof window.ActiveXObject + ":" + String(ax);
            "#);

        assert_eq!(
            effects,
            vec![text("result", "function:[object ActiveXObject]")]
        );
    }

    #[test]
    fn constructor_prototype_methods_are_available_on_instances() {
        let effects = run(r#"
            function Detector(name) {
                this.name = name;
            }
            Detector.prototype.test = function () {
                return this.name + ":ok";
            };
            let detector = new Detector("plugin");
            document.getElementById("result").textContent = detector.test();
            "#);

        assert_eq!(effects, vec![text("result", "plugin:ok")]);
    }

    #[test]
    fn object_method_call_binds_and_writes_back_this() {
        let effects = run(r#"
            let counter = {
                value: 1,
                inc: function () {
                    this.value = this.value + 1;
                    return this.value;
                }
            };
            document.getElementById("a").textContent = String(counter.inc());
            document.getElementById("b").textContent = String(counter.value);
            "#);

        assert_eq!(effects, vec![text("a", "2"), text("b", "2")]);
    }

    #[test]
    fn regexp_constructor_supports_test_exec_and_to_string() {
        let effects = run(r#"
            let re = new RegExp("hello|world", "i");
            document.getElementById("a").textContent = String(re.test("Well Hello"));
            document.getElementById("b").textContent = String(re.exec("world")[0]);
            document.getElementById("c").textContent = String(re);
            "#);

        assert_eq!(
            effects,
            vec![
                text("a", "true"),
                text("b", "hello|world"),
                text("c", "/hello|world/i"),
            ]
        );
    }

    #[test]
    fn regexp_literal_supports_test() {
        let effects = run(r#"
            document.getElementById("result").textContent = String(/hello/i.test("HELLO"));
            "#);

        assert_eq!(effects, vec![text("result", "true")]);
    }

    #[test]
    fn function_constructor_returns_callable_host_function() {
        let effects = run(r#"
            let f = new Function("return true");
            document.getElementById("result").textContent = String(f.call(null));
            "#);

        assert_eq!(effects, vec![text("result", "undefined")]);
    }

    #[test]
    fn window_external_feature_detection_methods_are_safe_false() {
        let effects = run(r#"
            document.getElementById("result").textContent =
                String(window.external.msActiveXFilteringEnabled());
            "#);

        assert_eq!(effects, vec![text("result", "false")]);
    }

    #[test]
    fn plugin_element_methods_soft_fail_as_empty_values() {
        let effects = run(r#"
            let plugin = document.createElement("object");
            document.getElementById("result").textContent =
                "[" + plugin.getComponentVersion("Flash") + "]";
            "#);

        assert_eq!(effects, vec![text("result", "[]")]);
    }

    #[test]
    fn plugin_element_soft_fail_methods_do_not_emit_unsupported_traces() {
        let effects = run(r#"
            let plugin = document.createElement("embed");
            document.getElementById("result").textContent =
                [
                    plugin.getComponentVersion("Flash"),
                    plugin.GetVariable("$version"),
                    plugin.IsVersionSupported("1")
                ].join("|");
            "#);

        assert_eq!(effects, vec![text("result", "||")]);
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn canvas_context_soft_failure_returns_null_for_unknown_contexts() {
        let effects = run(r#"
            let canvas = document.createElement("canvas");
            document.getElementById("result").textContent =
                String(canvas.getContext("2d")) + ":" + String(canvas.getContext("bitmaprenderer"));
            "#);

        assert_eq!(
            effects,
            vec![text("result", "[object CanvasRenderingContext]:null")]
        );
    }

    #[test]
    fn permissions_media_and_battery_soft_fail_as_promises() {
        let effects = run(r#"
            navigator.permissions.query({ name: "camera" }).then(function () {
                document.getElementById("permissions").textContent = "ok";
            });
            navigator.mediaDevices.getUserMedia({ audio: true }).then(function () {
                document.getElementById("gum").textContent = "ok";
            });
            navigator.getBattery().then(function () {
                document.getElementById("battery").textContent = "ok";
            });
            "#);

        assert_eq!(
            effects,
            vec![
                text("permissions", "ok"),
                text("gum", "ok"),
                text("battery", "ok"),
            ]
        );
    }

    #[test]
    fn audio_context_soft_failure_exposes_inert_nodes_and_promises() {
        let effects = run(r#"
            let audio = new window.webkitAudioContext();
            let node = audio.createOscillator();
            let buffer = audio.createBuffer(1, 16, 44100);
            audio.resume().then(function () {
                document.getElementById("resume").textContent = "ok";
            });
            document.getElementById("result").textContent =
                audio.state + ":" + String(audio.destination) + ":" +
                String(node.connect(audio.destination)) + ":" +
                String(buffer.getChannelData(0).length);
            "#);

        assert_eq!(
            effects,
            vec![
                text(
                    "result",
                    "suspended:[object AudioNode]:[object AudioNode]:0",
                ),
                text("resume", "ok"),
            ]
        );
    }

    #[test]
    fn offline_audio_context_start_rendering_soft_fails_as_promise() {
        let effects = run(r#"
            let ctx = new window.OfflineAudioContext(1, 16, 44100);
            ctx.startRendering().then(function () {
                document.getElementById("result").textContent = ctx.state;
            });
            "#);

        assert_eq!(effects, vec![text("result", "suspended")]);
    }

    #[test]
    fn unsupported_method_traces_include_callee_context() {
        let effects = run(r#"
            let missing;
            missing.replace("a", "b");
            "#);

        assert!(effects.iter().any(|effect| matches!(
            effect,
            BrowserEffect::RuntimeTrace { kind, detail }
                if kind == "unsupported.method"
                    && detail.contains("replace on undefined via")
                    && detail.contains("replace")
        )));
    }

    #[test]
    fn undefined_member_receiver_emits_warning_trace() {
        let effects = run(r#"
            let j;
            let value = j.toString;
            "#);

        assert!(effects.iter().any(|effect| matches!(
            effect,
            BrowserEffect::RuntimeTrace { kind, detail }
                if kind == "member.receiver.warning"
                    && detail.contains("property=toString")
                    && detail.contains("receiver_tag=Undefined")
                    && detail.contains("prototype_attempted=false")
                    && detail.contains("Identifier(\"j\")")
        )));
    }

    #[test]
    fn diagnostic_member_read_emits_undefined_result_trace() {
        let effects = run(r#"
            let n = 5;
            let value = n.toString;
            "#);

        assert!(effects.iter().any(|effect| matches!(
            effect,
            BrowserEffect::RuntimeTrace { kind, detail }
                if kind == "member.read.undefined"
                    && detail.contains("property=toString")
                    && detail.contains("receiver_tag=Number")
                    && detail.contains("result_tag=Undefined")
        )));
    }

    #[test]
    fn object_prototype_to_string_call_works_for_browser_values() {
        let effects = run(r#"
            let j = {};
            document.getElementById("result").textContent =
                j.toString.call([]) + ":" +
                Object.prototype.toString.call(function () {}) + ":" +
                Object.prototype.toString.call(window);
            "#);

        assert!(effects.contains(&text(
            "result",
            "[object Array]:[object Function]:[object Window]",
        )));
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn native_constructor_prototype_chain_exposes_object_methods() {
        let effects = run(r#"
            let objectProto = ({}).constructor.prototype;
            document.getElementById("result").textContent =
                objectProto.toString.call([]) + ":" +
                objectProto.hasOwnProperty.call({ a: 1 }, "a") + ":" +
                objectProto.valueOf.call({ marker: "ok" }).marker;
            "#);

        assert!(effects.contains(&text("result", "[object Array]:true:ok")));
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn undefined_native_method_slots_soft_fallback_to_object_prototype() {
        let effects = run(r#"
            let j = { toString: undefined };
            document.getElementById("result").textContent = j.toString.call([]);
            "#);

        assert!(effects.contains(&text("result", "[object Array]")));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            BrowserEffect::RuntimeTrace { kind, detail }
                if kind == "prototype.shadowed_undefined"
                    && detail == "Object.prototype.toString"
        )));
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn constructor_prototype_lookup_works_for_array_string_and_function() {
        let effects = run(r#"
            let arrayProto = [].constructor.prototype;
            let stringProto = "abc".constructor.prototype;
            let functionProto = (function () {}).constructor.prototype;
            document.getElementById("result").textContent =
                arrayProto.join.call(["a", "b"], "-") + ":" +
                stringProto.replace.call("abc", "b", "B") + ":" +
                functionProto.toString.call(function () {});
            "#);

        assert!(effects.contains(&text("result", "a-b:aBc:function () { [native code] }",)));
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn array_method_references_bind_as_callable_native_functions() {
        let effects = run(r#"
            let d = [];
            let bound = d.push.bind(d);
            document.getElementById("result").textContent =
                typeof d.push + ":" + String(bound("x")) + ":" + String(bound);
            "#);

        assert!(effects.contains(&text("result", "function:1:[object Function]")));
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn native_function_call_apply_dispatch_with_this_values() {
        let effects = run(r#"
            let arr = ["a", "b"];
            let join = arr.join;
            let replace = "abc".replace;
            document.getElementById("result").textContent =
                join.call(arr, "-") + ":" +
                replace.call("abc", "b", "B") + ":" +
                Array.prototype.indexOf.apply(arr, ["b"]);
            "#);

        assert!(effects.contains(&text("result", "a-b:aBc:1")));
        assert!(!has_runtime_trace(&effects, "unsupported.method"));
    }

    #[test]
    fn prototype_fallbacks_emit_lookup_telemetry() {
        let effects = run(r#"
            let arr = [];
            let push = arr.push;
            document.getElementById("result").textContent = typeof push;
            "#);

        assert!(effects.contains(&text("result", "function")));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            BrowserEffect::RuntimeTrace { kind, detail }
                if kind == "prototype.lookup" && detail == "Array.prototype.push"
        )));
    }

    #[test]
    fn browser_sensitive_apis_have_soft_failure_surfaces() {
        let effects = run(r#"
            let canvas = document.createElement("canvas");
            let unsupported = canvas.getContext("bitmaprenderer");
            let ctx = new AudioContext();
            navigator.permissions.query({ name: "camera" }).then(function () {
                document.getElementById("permissions").textContent = "resolved";
            });
            navigator.mediaDevices.enumerateDevices().then(function () {
                document.getElementById("media").textContent = "resolved";
            });
            document.getElementById("result").textContent =
                String(unsupported) + ":" + ctx.state + ":" + String(ctx.createAnalyser());
            "#);

        assert_eq!(
            effects,
            vec![
                text("result", "null:suspended:[object AudioNode]"),
                text("permissions", "resolved"),
                text("media", "resolved"),
            ]
        );
    }

    #[test]
    fn function_prototype_host_fallbacks_are_callable() {
        let effects = run(r#"
            let bind = Function.prototype.bind;
            document.getElementById("result").textContent = String(bind.call(null));
            "#);

        assert_eq!(effects, vec![text("result", "[object Function]")]);
    }

    #[test]
    fn execution_budget_stops_long_running_loop() {
        let program = crate::parse_script("while (true) { var x = 1; }").expect("valid program");
        let mut state = BrowserExecutionState::default();
        state.set_execution_budget(25);

        state.execute_program(&program);

        assert!(state.execution_budget_exhausted());
    }

    #[test]
    fn click_event_closure_mutates_counter_across_firings() {
        let program = crate::parse_script(
            r#"
            let count = 0;
            let btn = document.getElementById("btn");
            btn.addEventListener("click", function () {
                count = count + 1;
                document.getElementById("out").textContent = String(count);
            });
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        state.execute_program(&program);
        state.drain_effects();

        let first = state.fire_event("btn", "click", None);
        assert_eq!(
            first,
            vec![BrowserEffect::SetTextContent {
                element_id: "out".to_owned(),
                value: "1".to_owned(),
            }]
        );
        let second = state.fire_event("btn", "click", None);
        assert_eq!(
            second,
            vec![BrowserEffect::SetTextContent {
                element_id: "out".to_owned(),
                value: "2".to_owned(),
            }]
        );
    }

    #[test]
    fn keydown_event_passes_key_to_handler() {
        let program = crate::parse_script(
            r#"
            let input = document.getElementById("input");
            input.addEventListener("keydown", function (e) {
                document.getElementById("out").textContent = e.key;
            });
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        state.execute_program(&program);
        state.drain_effects();

        let effects = state.fire_event("input", "keydown", Some("Enter"));
        assert_eq!(
            effects,
            vec![BrowserEffect::SetTextContent {
                element_id: "out".to_owned(),
                value: "Enter".to_owned(),
            }]
        );
    }

    #[test]
    fn dom_content_loaded_fires_as_microtask_after_script() {
        let program = crate::parse_script(
            r#"
            document.addEventListener("DOMContentLoaded", function () {
                document.getElementById("result").textContent = "Ready";
            });
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "Ready".to_owned(),
            }]
        );
    }

    #[test]
    fn json_parse_and_stringify_round_trips_object() {
        let program = crate::parse_script(
            r#"
            let obj = JSON.parse('{"name":"AlmosThere"}');
            document.getElementById("result").textContent = JSON.stringify(obj);
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "{\"name\":\"AlmosThere\"}".to_owned(),
            }]
        );
    }

    #[test]
    fn array_push_and_index_and_length() {
        let program = crate::parse_script(
            r#"
            let items = [];
            items.push("A");
            items.push("B");
            document.getElementById("result").textContent =
                items[0] + items[1] + String(items.length);
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "AB2".to_owned(),
            }]
        );
    }

    #[test]
    fn promise_microtask_runs_after_sync_code_and_before_timer() {
        let program = crate::parse_script(
            r#"
            let output = "";
            Promise.resolve().then(function () {
                output = output + "B";
            });
            output = output + "A";
            setTimeout(function () {
                document.getElementById("result").textContent = output;
            }, 0);
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        state.execute_program(&program);
        // After execute_program: output="A", then microtask ran → output="AB"
        state.drain_effects();

        let effects = state.poll_timers(0);
        assert_eq!(
            effects,
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "AB".to_owned(),
            }]
        );
    }

    #[test]
    fn style_property_assignment_emits_set_attribute_with_inline_style() {
        let program = crate::parse_script(
            r#"
            let box = document.getElementById("box");
            box.style.display = "none";
            "#,
        )
        .expect("script should parse");

        assert_eq!(
            collect_browser_effects(&program),
            vec![BrowserEffect::SetAttribute {
                element_id: "box".to_owned(),
                name: "style".to_owned(),
                value: "display: none".to_owned(),
            }]
        );
    }

    #[test]
    fn style_property_assignment_merges_with_existing_inline_style() {
        let program = crate::parse_script(
            r#"
            let box = document.getElementById("box");
            box.style.color = "red";
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        let mut attrs = HashMap::new();
        attrs.insert("style".to_owned(), "display: block".to_owned());
        state.seed_existing_element("box", String::new(), attrs);
        state.execute_program(&program);

        assert_eq!(
            state.drain_effects(),
            vec![BrowserEffect::SetAttribute {
                element_id: "box".to_owned(),
                name: "style".to_owned(),
                value: "display: block; color: red".to_owned(),
            }]
        );
    }

    #[test]
    fn get_computed_style_returns_seeded_display_value() {
        let program = crate::parse_script(
            r#"
            let style = getComputedStyle(document.getElementById("box"));
            document.getElementById("result").textContent = style.display;
            "#,
        )
        .expect("script should parse");

        let mut state = BrowserExecutionState::default();
        state.seed_existing_element("box", String::new(), HashMap::new());
        let mut computed = HashMap::new();
        computed.insert("display".to_owned(), "block".to_owned());
        state.seed_computed_style("box", computed);
        state.execute_program(&program);

        assert_eq!(
            state.drain_effects(),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "block".to_owned(),
            }]
        );
    }

    #[test]
    fn query_selector_all_length_uses_seeded_class_index() {
        let mut state = BrowserExecutionState::default();
        let mut attrs = HashMap::new();
        attrs.insert("class".to_owned(), "item".to_owned());
        state.seed_existing_element("a", "A".to_owned(), attrs.clone());
        state.seed_existing_element("b", "B".to_owned(), attrs.clone());
        state.seed_existing_element("c", "C".to_owned(), attrs);
        let program = crate::parse_script(
            r##"
            let items = document.querySelectorAll(".item");
            document.getElementById("result").textContent = String(items.length);
            "##,
        )
        .expect("script should parse");

        state.execute_program(&program);

        assert_eq!(
            state.drain_effects(),
            vec![BrowserEffect::SetTextContent {
                element_id: "result".to_owned(),
                value: "3".to_owned(),
            }]
        );
    }

    // ── Feature tests 031–037 ───────────────────────────────────────────────

    fn run(src: &str) -> Vec<BrowserEffect> {
        let program = crate::parse_script(src).expect("parse error");
        collect_browser_effects(&program)
    }

    fn text(id: &str, value: &str) -> BrowserEffect {
        BrowserEffect::SetTextContent {
            element_id: id.to_owned(),
            value: value.to_owned(),
        }
    }

    fn has_runtime_trace(effects: &[BrowserEffect], expected_kind: &str) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                BrowserEffect::RuntimeTrace { kind, .. } if kind == expected_kind
            )
        })
    }

    // 031 – default parameters
    #[test]
    fn t031_default_parameters() {
        let effects = run(r#"
            function greet(name, greeting) {
                if (greeting === undefined) { greeting = "Hello"; }
                if (name === undefined) { name = "World"; }
                document.getElementById("result").textContent = greeting + " " + name;
            }
            greet();
        "#);
        assert_eq!(effects, vec![text("result", "Hello World")]);
    }

    #[test]
    fn t031_default_parameters_native() {
        let effects = run(r#"
            function greet(name = "World", greeting = "Hello") {
                document.getElementById("result").textContent = greeting + " " + name;
            }
            greet();
        "#);
        assert_eq!(effects, vec![text("result", "Hello World")]);
    }

    // 032 – arrow functions
    #[test]
    fn t032_arrow_concise() {
        let effects = run(r#"
            var add = (a, b) => a + b;
            document.getElementById("result").textContent = add(2, 3);
        "#);
        assert_eq!(effects, vec![text("result", "5")]);
    }

    #[test]
    fn t032_arrow_block_body() {
        let effects = run(r#"
            var shout = function(s) { return s + "!"; };
            document.getElementById("result").textContent = shout("hello");
        "#);
        assert_eq!(effects, vec![text("result", "hello!")]);
    }

    // 033 – spread
    #[test]
    fn t033_spread_array() {
        let effects = run(r#"
            var a = [1, 2];
            var b = [3, 4];
            var c = [...a, ...b];
            document.getElementById("result").textContent = c.length;
        "#);
        assert_eq!(effects, vec![text("result", "4")]);
    }

    // 034 – optional chaining
    #[test]
    fn t034_optional_chaining_null() {
        let effects = run(r#"
            var obj = null;
            var result = obj?.name;
            if (result === undefined) { result = "none"; }
            document.getElementById("result").textContent = result;
        "#);
        assert_eq!(effects, vec![text("result", "none")]);
    }

    // 035 – template literals
    #[test]
    fn t035_template_literal() {
        let effects = run(r#"
            var name = "Alice";
            var age = 30;
            document.getElementById("result").textContent = `Hello ${name}, you are ${age}.`;
        "#);
        assert_eq!(effects, vec![text("result", "Hello Alice, you are 30.")]);
    }

    // 036 – try / catch / finally
    #[test]
    fn t036_try_catch() {
        let effects = run(r#"
            var log = "";
            try { throw "oops"; } catch (e) { log = log + "caught"; }
            log = log + "/ok";
            document.getElementById("result").textContent = log;
        "#);
        assert_eq!(effects, vec![text("result", "caught/ok")]);
    }

    #[test]
    fn t036_try_finally() {
        let effects = run(r#"
            var ran = false;
            try { var x = 1; } finally { ran = true; }
            document.getElementById("result").textContent = ran;
        "#);
        assert_eq!(effects, vec![text("result", "true")]);
    }

    // 037 – for…of
    #[test]
    fn t037_for_of_array() {
        let effects = run(r#"
            var sum = 0;
            for (var n of [1, 2, 3]) { sum = sum + n; }
            document.getElementById("result").textContent = sum;
        "#);
        assert_eq!(effects, vec![text("result", "6")]);
    }

    #[test]
    fn t037_for_of_string() {
        let effects = run(r#"
            var chars = "";
            for (var ch of "hello") { chars = chars + ch; }
            document.getElementById("result").textContent = chars;
        "#);
        assert_eq!(effects, vec![text("result", "hello")]);
    }

    #[test]
    fn t042_object_destructuring_simple() {
        let effects = run(r#"
            var obj = { a: 1, b: 2 };
            var { a, b } = obj;
            document.getElementById("result").textContent = a + "/" + b;
        "#);
        assert_eq!(effects, vec![text("result", "1/2")]);
    }

    #[test]
    fn t042_object_destructuring_renamed() {
        let effects = run(r#"
            var point = { x: 10, y: 20 };
            var { x: px, y: py } = point;
            document.getElementById("result").textContent = px + "/" + py;
        "#);
        assert_eq!(effects, vec![text("result", "10/20")]);
    }

    #[test]
    fn t042_object_destructuring_default() {
        let effects = run(r#"
            var opts = { color: "red" };
            var { color, size = "large" } = opts;
            document.getElementById("result").textContent = color + "/" + size;
        "#);
        assert_eq!(effects, vec![text("result", "red/large")]);
    }

    #[test]
    fn t042_array_destructuring_simple() {
        let effects = run(r#"
            var arr = [10, 20, 30];
            var [x, y, z] = arr;
            document.getElementById("result").textContent = x + "/" + y + "/" + z;
        "#);
        assert_eq!(effects, vec![text("result", "10/20/30")]);
    }

    #[test]
    fn t042_array_destructuring_skip() {
        let effects = run(r#"
            var arr = [1, 2, 3];
            var [first, , third] = arr;
            document.getElementById("result").textContent = first + "/" + third;
        "#);
        assert_eq!(effects, vec![text("result", "1/3")]);
    }

    // ── built-in methods ──────────────────────────────────────────────────

    #[test]
    fn t043_array_join() {
        let effects = run(r#"
            var arr = [1, 2, 3];
            document.getElementById("result").textContent = arr.join("-");
        "#);
        assert_eq!(effects, vec![text("result", "1-2-3")]);
    }

    #[test]
    fn t043_array_map() {
        let effects = run(r#"
            var arr = [1, 2, 3];
            var doubled = arr.map(function(x) { return x * 2; });
            document.getElementById("result").textContent = doubled.join(",");
        "#);
        assert_eq!(effects, vec![text("result", "2,4,6")]);
    }

    #[test]
    fn t043_array_filter() {
        let effects = run(r#"
            var arr = [1, 2, 3, 4, 5];
            var evens = arr.filter(function(x) { return x % 2 === 0; });
            document.getElementById("result").textContent = evens.join(",");
        "#);
        assert_eq!(effects, vec![text("result", "2,4")]);
    }

    #[test]
    fn t043_array_reduce() {
        let effects = run(r#"
            var arr = [1, 2, 3, 4];
            var sum = arr.reduce(function(acc, x) { return acc + x; }, 0);
            document.getElementById("result").textContent = sum;
        "#);
        assert_eq!(effects, vec![text("result", "10")]);
    }

    #[test]
    fn t043_array_find_some_every() {
        let effects = run(r#"
            var arr = [1, 3, 5, 7];
            var found = arr.find(function(x) { return x > 4; });
            var any = arr.some(function(x) { return x > 6; });
            var all = arr.every(function(x) { return x > 0; });
            document.getElementById("result").textContent = found + "/" + any + "/" + all;
        "#);
        assert_eq!(effects, vec![text("result", "5/true/true")]);
    }

    #[test]
    fn t043_array_includes_indexof() {
        let effects = run(r#"
            var arr = ["a", "b", "c"];
            document.getElementById("result").textContent =
                arr.includes("b") + "/" + arr.indexOf("c") + "/" + arr.indexOf("z");
        "#);
        assert_eq!(effects, vec![text("result", "true/2/-1")]);
    }

    #[test]
    fn t043_array_slice() {
        let effects = run(r#"
            var arr = [10, 20, 30, 40, 50];
            document.getElementById("result").textContent = arr.slice(1, 3).join(",");
        "#);
        assert_eq!(effects, vec![text("result", "20,30")]);
    }

    #[test]
    fn t043_array_flat() {
        let effects = run(r#"
            var arr = [[1, 2], [3, 4]];
            document.getElementById("result").textContent = arr.flat().join(",");
        "#);
        assert_eq!(effects, vec![text("result", "1,2,3,4")]);
    }

    #[test]
    fn t043_string_methods() {
        let effects = run(r#"
            var s = "  Hello World  ";
            var r = s.trim().toLowerCase().replace("hello", "hi");
            document.getElementById("result").textContent = r;
        "#);
        assert_eq!(effects, vec![text("result", "hi world")]);
    }

    #[test]
    fn t043_string_split_includes() {
        let effects = run(r#"
            var parts = "a,b,c".split(",");
            var ok = "hello".includes("ell");
            document.getElementById("result").textContent = parts.length + "/" + ok;
        "#);
        assert_eq!(effects, vec![text("result", "3/true")]);
    }

    #[test]
    fn t043_string_slice_padstart() {
        let effects = run(r#"
            var s = "hello";
            document.getElementById("result").textContent =
                s.slice(1, 4) + "/" + "7".padStart(3, "0");
        "#);
        assert_eq!(effects, vec![text("result", "ell/007")]);
    }

    #[test]
    fn t043_string_length() {
        let effects = run(r#"
            var s = "hello";
            document.getElementById("result").textContent = s.length;
        "#);
        assert_eq!(effects, vec![text("result", "5")]);
    }

    #[test]
    fn t043_object_keys_values_entries() {
        let effects = run(r#"
            var obj = { b: 2, a: 1 };
            var k = Object.keys(obj).join(",");
            var v = Object.values(obj).join(",");
            document.getElementById("result").textContent = k + "/" + v;
        "#);
        assert_eq!(effects, vec![text("result", "a,b/1,2")]);
    }

    #[test]
    fn t043_object_assign() {
        let effects = run(r#"
            var base = { a: 1, b: 2 };
            var ext  = { b: 99, c: 3 };
            var merged = Object.assign({}, base, ext);
            document.getElementById("result").textContent =
                merged.a + "/" + merged.b + "/" + merged.c;
        "#);
        assert_eq!(effects, vec![text("result", "1/99/3")]);
    }

    #[test]
    fn t043_math_methods() {
        let effects = run(r#"
            var r = Math.floor(3.9) + "/" + Math.ceil(3.1) + "/" + Math.abs(-5)
                  + "/" + Math.max(1, 2, 3) + "/" + Math.min(1, 2, 3)
                  + "/" + Math.PI.toFixed(0);
            document.getElementById("result").textContent = r;
        "#);
        assert_eq!(effects, vec![text("result", "3/4/5/3/1/3")]);
    }

    #[test]
    fn t043_parseint_parsefloat() {
        let effects = run(r#"
            var a = parseInt("42px");
            var b = parseFloat("3.14abc");
            document.getElementById("result").textContent = a + "/" + b;
        "#);
        assert_eq!(effects, vec![text("result", "42/3.14")]);
    }

    #[test]
    fn t043_array_isarray() {
        let effects = run(r#"
            document.getElementById("result").textContent =
                Array.isArray([1,2]) + "/" + Array.isArray("nope");
        "#);
        assert_eq!(effects, vec![text("result", "true/false")]);
    }

    #[test]
    fn t044_switch_basic() {
        let effects = run(r#"
            var x = 2;
            var result = "none";
            switch (x) {
                case 1: result = "one"; break;
                case 2: result = "two"; break;
                case 3: result = "three"; break;
            }
            document.getElementById("result").textContent = result;
        "#);
        assert_eq!(effects, vec![text("result", "two")]);
    }

    #[test]
    fn t044_switch_default() {
        let effects = run(r#"
            var x = 99;
            var result = "none";
            switch (x) {
                case 1: result = "one"; break;
                default: result = "default"; break;
                case 3: result = "three"; break;
            }
            document.getElementById("result").textContent = result;
        "#);
        assert_eq!(effects, vec![text("result", "default")]);
    }

    #[test]
    fn t044_switch_fallthrough() {
        let effects = run(r#"
            var x = 1;
            var log = "";
            switch (x) {
                case 1: log += "a";
                case 2: log += "b"; break;
                case 3: log += "c"; break;
            }
            document.getElementById("result").textContent = log;
        "#);
        assert_eq!(effects, vec![text("result", "ab")]);
    }

    #[test]
    fn t044_switch_string() {
        let effects = run(r#"
            var s = "hello";
            var result = "miss";
            switch (s) {
                case "world": result = "world"; break;
                case "hello": result = "hi"; break;
            }
            document.getElementById("result").textContent = result;
        "#);
        assert_eq!(effects, vec![text("result", "hi")]);
    }

    #[test]
    fn t044_bare_for_in() {
        let effects = run(r#"
            var obj = {a: 1, b: 2};
            var keys = "";
            var r;
            for (r in obj) { keys += r; }
            document.getElementById("result").textContent = keys;
        "#);
        // key order may vary; just check length 2 and both chars present
        if let Some(crate::effects::BrowserEffect::SetTextContent { value, .. }) = effects.first() {
            assert_eq!(value.len(), 2);
            assert!(value.contains('a') && value.contains('b'));
        } else {
            panic!("expected SetTextContent effect");
        }
    }

    #[test]
    fn t044_dot_number_literal() {
        let effects = run(r#"
            var x = .5;
            var y = .25;
            document.getElementById("result").textContent = String(x + y);
        "#);
        assert_eq!(effects, vec![text("result", "0.75")]);
    }

    // Rc<RefCell> shared-frame closure tests
    // These verify that closures sharing a captured environment see each other's mutations.

    #[test]
    fn t050_closure_shared_mutable_state() {
        // inc and get share the same 'n' frame — mutations from inc must be visible via get
        let effects = run(r#"
            function makeCounter() {
                var n = 0;
                function inc() { n = n + 1; }
                function get() { return n; }
                return { inc: inc, get: get };
            }
            var c = makeCounter();
            c.inc();
            c.inc();
            c.inc();
            document.getElementById("result").textContent = String(c.get());
        "#);
        assert_eq!(effects, vec![text("result", "3")]);
    }

    #[test]
    fn t051_closure_adder_independent_captures() {
        // Two adder instances must not share state with each other
        let effects = run(r#"
            function makeAdder(x) {
                return function(y) { return x + y; };
            }
            var add5 = makeAdder(5);
            var add10 = makeAdder(10);
            document.getElementById("a").textContent = String(add5(3));
            document.getElementById("b").textContent = String(add10(3));
        "#);
        assert_eq!(effects, vec![text("a", "8"), text("b", "13")]);
    }

    #[test]
    fn t052_closure_mutates_outer_scope_variable() {
        // A closure that assigns to an outer-scope variable; the caller must see the new value
        let effects = run(r#"
            var x = 10;
            function double() { x = x * 2; }
            double();
            double();
            document.getElementById("result").textContent = String(x);
        "#);
        assert_eq!(effects, vec![text("result", "40")]);
    }

    #[test]
    fn t053_webpack_jsonp_push_override() {
        // window.webpackJsonp.push = callback — direct assignment on global member
        let effects = run(r#"
            window.webpackJsonp = [];
            window.webpackJsonp.push = function(data) {
                document.getElementById("app").textContent = "loaded:" + data[0];
            };
            (window.webpackJsonp = window.webpackJsonp || []).push([42]);
        "#);
        assert_eq!(effects, vec![text("app", "loaded:42")]);
    }

    #[test]
    fn t054_webpack_jsonp_local_alias_push_override() {
        // var d = window.X = []; d.push = r  — local-alias pattern from Webpack runtime
        // var d = window.X = []; d.push = r  — local-alias pattern from Webpack runtime
        let effects = run(r#"
            var d = window.webpackJsonp = window.webpackJsonp || [];
            d.push = function(data) {
                document.getElementById("app").textContent = "via-alias:" + data[0];
            };
            (window.webpackJsonp = window.webpackJsonp || []).push([99]);
        "#);
        assert_eq!(effects, vec![text("app", "via-alias:99")]);
    }

    #[test]
    fn t055_function_call_apply_bind() {
        let effects = run(r#"
            function greet(a, b) {
                document.getElementById("a").textContent = a;
                document.getElementById("b").textContent = b;
            }
            greet.call(null, "hello", "world");
            greet.apply(null, ["foo", "bar"]);
            var bound = greet.bind(null);
            bound("x", "y");
        "#);
        assert_eq!(
            effects,
            vec![
                text("a", "hello"),
                text("b", "world"),
                text("a", "foo"),
                text("b", "bar"),
                text("a", "x"),
                text("b", "y"),
            ]
        );
    }

    #[test]
    fn t056_computed_member_assignment() {
        let effects = run(r#"
            var obj = {};
            var key = "result";
            obj[key] = "computed";
            document.getElementById("out").textContent = obj.result;
            var arr = [0, 0, 0];
            arr[1] = "mid";
            document.getElementById("arr").textContent = arr[1];
        "#);
        assert_eq!(effects, vec![text("out", "computed"), text("arr", "mid")]);
    }

    #[test]
    fn t057_named_object_property_assignment() {
        let effects = run(r#"
            var obj = {};
            obj.name = "alice";
            obj.age = 30;
            document.getElementById("name").textContent = obj.name;
            document.getElementById("age").textContent = String(obj.age);
        "#);
        assert_eq!(effects, vec![text("name", "alice"), text("age", "30")]);
    }

    #[test]
    fn t058_computed_object_read() {
        let effects = run(r#"
            var map = {};
            map["hello"] = "world";
            var key = "hello";
            document.getElementById("a").textContent = map["hello"];
            document.getElementById("b").textContent = map[key];
        "#);
        assert_eq!(effects, vec![text("a", "world"), text("b", "world")]);
    }

    #[test]
    fn t059_call_writeback_exports() {
        // Simulates Webpack module factory: factory.call(exports, module, exports, require)
        // exports.result inside factory must propagate back to module.exports outside.
        let effects = run(r#"
            function makeModule(module, exports, require) {
                exports.result = "from factory";
            }
            var mod = { exports: {} };
            makeModule.call(mod.exports, mod, mod.exports, function(){});
            document.getElementById("out").textContent = mod.exports.result;
        "#);
        assert_eq!(effects, vec![text("out", "from factory")]);
    }

    #[test]
    fn t060_call_writeback_nested_object() {
        // Object passed to .call() as arg — mutations visible after return.
        let effects = run(r#"
            function populate(obj) {
                obj.x = "hello";
                obj.y = "world";
            }
            var data = {};
            populate.call(null, data);
            document.getElementById("a").textContent = data.x;
            document.getElementById("b").textContent = data.y;
        "#);
        assert_eq!(effects, vec![text("a", "hello"), text("b", "world")]);
    }

    // AMIUnique / Nuxt bootstrap hypotheses.

    #[test]
    fn t061_webpack_jsonp_factory_pipeline_reaches_module_effect() {
        // Composes the current Webpack pieces: JSONP push override, module table
        // lookup, factory.call(...), exports write-back, and a final DOM effect.
        // If this regresses, AMIUnique can stop before Vue/Nuxt bootstrap begins.
        let effects = run(r#"
            window.webpackJsonp = [];
            window.webpackJsonp.push = function(chunk) {
                var factories = chunk[1];
                var module = { exports: {} };
                factories.entry.call(module.exports, module, module.exports, function(id) {
                    return factories[id];
                });
                document.getElementById("app").textContent = module.exports.value;
            };
            (window.webpackJsonp = window.webpackJsonp || []).push([["app"], {
                entry: function(module, exports, require) {
                    exports.value = "mounted";
                }
            }]);
        "#);
        assert_eq!(effects, vec![text("app", "mounted")]);
    }

    #[test]
    fn t062_vue_mount_dom_primitives_create_and_insert_nodes() {
        // Vue renderers commonly build text/comment nodes and insert/remove them
        // around a mount anchor. This should eventually emit an append/insert-like
        // DOM effect or equivalent durable DOM mutation.
        let effects = run(r#"
            var root = document.getElementById("app");
            var textNode = document.createTextNode("hello");
            var marker = document.createComment("anchor");
            root.appendChild(marker);
            root.insertBefore(textNode, marker);
            root.removeChild(marker);
        "#);
        assert!(
            !effects.is_empty(),
            "Vue-style DOM node creation/insertion produced no effects"
        );
    }

    #[test]
    fn t063_object_define_property_persists_to_original_target() {
        // Webpack and Vue define exports, getters, and flags via descriptors.
        // Returning a modified clone is not enough; later reads of the original
        // object must see the descriptor value.
        let effects = run(r#"
            var exports = {};
            Object.defineProperty(exports, "answer", { value: "ok" });
            document.getElementById("result").textContent = exports.answer;
        "#);
        assert_eq!(effects, vec![text("result", "ok")]);
    }

    #[test]
    fn t064_fetch_then_callback_can_update_dom() {
        // AMIUnique's bundles contain fetch/axios/XHR paths. A minimal host bridge
        // should return a thenable/resolved promise so collector chains can progress
        // instead of disappearing as undefined/no-op calls.
        let effects = run(r#"
            fetch("/fingerprint").then(function(response) {
                document.getElementById("result").textContent = "fetched";
            });
        "#);
        assert!(
            effects.iter().any(|effect| matches!(
                effect,
                BrowserEffect::NetworkRequest { method, url, .. }
                    if method == "GET" && url == "/fingerprint"
            )),
            "fetch should emit a network request effect: {effects:?}"
        );
        assert!(
            effects.contains(&text("result", "fetched")),
            "fetch callback should still update the DOM: {effects:?}"
        );
    }

    #[test]
    fn t064b_xml_http_request_send_emits_network_trace() {
        let effects = run(r#"
            var xhr = new XMLHttpRequest();
            xhr.open("POST", "/collect");
            xhr.setRequestHeader("content-type", "application/json");
            xhr.send("{\"ok\":true}");
        "#);
        assert!(
            effects.iter().any(|effect| matches!(
                effect,
                BrowserEffect::RuntimeTrace { kind, detail }
                    if kind == "xhr.open" && detail == "POST /collect"
            )),
            "XHR open should be traced: {effects:?}"
        );
        assert!(
            effects.iter().any(|effect| matches!(
                effect,
                BrowserEffect::NetworkRequest { method, url, body }
                    if method == "POST" && url == "/collect" && body == "{\"ok\":true}"
            )),
            "XHR send should emit a network request effect: {effects:?}"
        );
    }

    #[test]
    fn t065b_call_writeback_function_export() {
        // Webpack factories that do `module.exports = function() {...}` must propagate
        // the function value back through call_function_with_writeback (not just Objects).
        let effects = run(r#"
            function factory(module, exports, require) {
                module.exports = function greet() { return "hello"; };
            }
            var mod = { exports: {} };
            factory.call(mod.exports, mod, mod.exports, function(){});
            var fn = mod.exports;
            document.getElementById("result").textContent = fn();
        "#);
        assert_eq!(effects, vec![text("result", "hello")]);
    }

    #[test]
    fn t065c_symbol_produces_unique_strings() {
        // Symbol() must return a value usable as an object key without collision.
        let effects = run(r#"
            var s1 = Symbol("iter");
            var s2 = Symbol("iter");
            var obj = {};
            obj[s1] = "first";
            obj[s2] = "second";
            var sameKey = s1 === s2;
            document.getElementById("same").textContent = String(sameKey);
            document.getElementById("v1").textContent = obj[s1];
            document.getElementById("v2").textContent = obj[s2];
        "#);
        // s1 and s2 must be different strings, so they store different values.
        assert!(
            effects.contains(&text("same", "false")),
            "Symbol() values should be unique: {effects:?}"
        );
        assert!(
            effects.contains(&text("v1", "first")),
            "first symbol slot should hold 'first': {effects:?}"
        );
        assert!(
            effects.contains(&text("v2", "second")),
            "second symbol slot should hold 'second': {effects:?}"
        );
    }

    #[test]
    fn t065d_document_ready_state_is_complete() {
        let effects = run(r#"
            var ready = document.readyState === "complete";
            document.getElementById("result").textContent = String(ready);
        "#);
        assert_eq!(effects, vec![text("result", "true")]);
    }

    #[test]
    fn t065e_window_load_event_fires_as_microtask() {
        let effects = run(r#"
            window.addEventListener("load", function() {
                document.getElementById("result").textContent = "loaded";
            });
        "#);
        assert_eq!(effects, vec![text("result", "loaded")]);
    }

    #[test]
    fn t065f_promise_all_returns_resolved_promise() {
        // Promise.all must not crash and should allow .then chains to proceed.
        let effects = run(r#"
            Promise.all([Promise.resolve(1), Promise.resolve(2)]).then(function() {
                document.getElementById("result").textContent = "done";
            });
        "#);
        assert_eq!(effects, vec![text("result", "done")]);
    }

    #[test]
    fn t065_proxy_and_weakmap_basic_semantics() {
        // The AMIUnique Nuxt chunks reference Proxy and WeakMap. This smoke test
        // captures the minimum behavior needed before those constructs can be
        // trusted in real-world bundles.
        let effects = run(r#"
            var target = { name: "Alice" };
            var proxy = new Proxy(target, {
                get: function(obj, prop) {
                    if (obj[prop] === undefined) { return "missing"; }
                    return obj[prop];
                }
            });
            var map = new WeakMap();
            map.set(target, "stored");
            document.getElementById("result").textContent =
                proxy.name + "/" + proxy.unknown + "/" + map.get(target);
        "#);
        assert_eq!(effects, vec![text("result", "Alice/missing/stored")]);
    }

    // Webpack computed-access diagnostics — these pin-point the bootstrap failure.

    #[test]
    fn t066a_window_computed_write_then_named_read() {
        // window["x"] = 5 must set globals["x"] so that a later `x` identifier read
        // and a `window.x` named read both return 5.
        let effects = run(r#"
            window["myGlobal"] = "hello";
            document.getElementById("a").textContent = myGlobal;
            document.getElementById("b").textContent = window.myGlobal;
        "#);
        assert_eq!(
            effects,
            vec![text("a", "hello"), text("b", "hello")],
            "window[\"x\"] = val must set the global so identifier and window.x reads agree: {effects:?}"
        );
    }

    #[test]
    fn t066b_window_computed_read_returns_named_global() {
        // window.x = 5 set via named write; window["x"] computed read must return 5.
        let effects = run(r#"
            window.myGlobal = "world";
            var v = window["myGlobal"];
            document.getElementById("out").textContent = v;
        "#);
        assert_eq!(
            effects,
            vec![text("out", "world")],
            "window[\"x\"] computed read must return the previously-set global: {effects:?}"
        );
    }

    #[test]
    fn t066c_webpack_computed_jsonp_init_pattern() {
        // Actual Webpack runtime pattern:
        //   var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
        // This requires both computed read (to get existing value or undefined) and
        // computed write (to persist the new array as globals["webpackJsonp"]).
        // Note: `===` on two Array values is reference equality in JS and always false
        // in JBS (value semantics), so we only check that both sides are arrays.
        let effects = run(r#"
            var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
            document.getElementById("type").textContent = Array.isArray(jsonpArray) ? "array" : "not-array";
            document.getElementById("glob").textContent = Array.isArray(window["webpackJsonp"]) ? "array" : "not-array";
        "#);
        assert!(
            effects.contains(&text("type", "array")),
            "jsonpArray must be an array after computed init: {effects:?}"
        );
        assert!(
            effects.contains(&text("glob", "array")),
            "window[\"webpackJsonp\"] must also be an array after computed init: {effects:?}"
        );
    }

    #[test]
    fn t066d_webpack_push_override_propagates_after_computed_write() {
        // Full real Webpack runtime pattern (using computed access throughout):
        //   var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
        //   var oldPush = jsonpArray.push.bind(jsonpArray);
        //   jsonpArray.push = webpackJsonpCallback;
        // Then a chunk does:
        //   (window["webpackJsonp"] = window["webpackJsonp"] || []).push([chunkData]);
        // The overridden push must fire, not native Array.prototype.push.
        let effects = run(r#"
            var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
            jsonpArray.push = function(chunk) {
                document.getElementById("result").textContent = "callback:" + chunk[0];
            };
            (window["webpackJsonp"] = window["webpackJsonp"] || []).push(["chunk1"]);
        "#);
        assert_eq!(
            effects,
            vec![text("result", "callback:chunk1")],
            "push override must fire when chunk calls window[\"webpackJsonp\"].push: {effects:?}"
        );
    }

    #[test]
    fn t066e_webpack_require_module_cache_semantics() {
        // __webpack_require__ caches modules in installedModules[moduleId].
        // After first call, second call must return the cached module.exports, not {}.
        // The tricky part: `installedModules[id] = module = {exports:{}}` uses
        // value semantics in JBS — the module local and installedModules[id] diverge
        // if we don't write back properly.
        let effects = run(r#"
            var installedModules = {};
            function __webpack_require__(moduleId) {
                if (installedModules[moduleId]) {
                    return installedModules[moduleId].exports;
                }
                var module = installedModules[moduleId] = { id: moduleId, exports: {} };
                var factory = modules[moduleId];
                factory.call(module.exports, module, module.exports, __webpack_require__);
                return module.exports;
            }
            var modules = {
                42: function(module, exports, require) {
                    exports.answer = "forty-two";
                }
            };
            var result1 = __webpack_require__(42);
            var result2 = __webpack_require__(42);
            document.getElementById("r1").textContent = result1.answer;
            document.getElementById("r2").textContent = result2.answer;
            document.getElementById("cached").textContent = String(installedModules[42] !== undefined);
        "#);
        assert!(
            effects.contains(&text("r1", "forty-two")),
            "first __webpack_require__ call must return module.exports with factory output: {effects:?}"
        );
        assert!(
            effects.contains(&text("r2", "forty-two")),
            "second __webpack_require__ call must return cached exports, not {{}}: {effects:?}"
        );
        assert!(
            effects.contains(&text("cached", "true")),
            "installedModules[42] must be set after first require: {effects:?}"
        );
    }

    #[test]
    fn t066f_webpack_full_bootstrap_computed_access() {
        // Full end-to-end Webpack bootstrap using computed member access throughout,
        // matching what the real amiunique _nuxt/13ede67.js Webpack runtime does.
        // The runtime sets up webpackJsonp via computed access, registers the push
        // override, then a chunk script adds its factories. The factory runs,
        // sets exports.value, and the entry module produces a DOM effect.
        let effects = run(r#"
            var installedModules = {};
            var modules = {};
            function __webpack_require__(moduleId) {
                if (installedModules[moduleId]) {
                    return installedModules[moduleId].exports;
                }
                var module = installedModules[moduleId] = { id: moduleId, exports: {} };
                modules[moduleId].call(module.exports, module, module.exports, __webpack_require__);
                return module.exports;
            }
            function webpackJsonpCallback(data) {
                var chunkIds = data[0];
                var moreModules = data[1];
                var executeModules = data[2];
                for (var moduleId in moreModules) {
                    modules[moduleId] = moreModules[moduleId];
                }
                if (executeModules) {
                    for (var i = 0; i < executeModules.length; i++) {
                        var result = __webpack_require__(executeModules[i]);
                        if (result && result.default) {
                            result.default();
                        }
                    }
                }
            }
            var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
            jsonpArray.push = webpackJsonpCallback;

            // Simulate a chunk script call (what _nuxt/66e1ca0.js etc. do):
            (window["webpackJsonp"] = window["webpackJsonp"] || []).push([
                ["app"],
                {
                    "entry": function(module, exports, require) {
                        exports.default = function() {
                            document.getElementById("app").textContent = "vue-mounted";
                        };
                    }
                },
                ["entry"]
            ]);
        "#);
        assert_eq!(
            effects,
            vec![text("app", "vue-mounted")],
            "full Webpack bootstrap with computed access must reach the entry module effect: {effects:?}"
        );
    }

    // ── Multi-script execution tests ──────────────────────────────────────────
    // The browser runs each <script> tag as a separate execute_program call on
    // the SAME BrowserExecutionState. The Webpack runtime (script 5) sets up
    // the push override; the chunk scripts (6-14) call push on a separate program.
    // These tests pin down whether that inter-script state transfer works.

    #[test]
    fn t067e_function_declaration_hoisting_within_block() {
        // In JS, function declarations are hoisted to the top of their containing
        // function scope, so they can be referenced before the declaration site.
        // JBS currently executes statements in order — if a FunctionDeclaration
        // appears after a reference to that name, the name reads as Undefined.
        // This test confirms the bug: the Webpack IIFE assigns `arr.push = fn`
        // before the `function fn(data){...}` declaration appears in source order.
        let effects = run(r#"
            (function() {
                var arr = [];
                arr[0] = earlyRef;          // read name before declaration
                var called = (typeof earlyRef === "function") ? "hoisted" : "not-hoisted";
                document.getElementById("hoist").textContent = called;
                function earlyRef(x) { return x * 2; }
                // also verify the declaration itself works after its position
                document.getElementById("late").textContent = String(earlyRef(21));
            })();
        "#);
        assert!(
            effects.contains(&text("hoist", "hoisted")),
            "function declaration must be hoisted so it is visible before its source position: {effects:?}"
        );
        assert!(
            effects.contains(&text("late", "42")),
            "function declaration must also be callable after its source position: {effects:?}"
        );
    }

    #[test]
    fn t067f_webpack_iife_push_override_with_hoisted_callback() {
        // The real Webpack 4 runtime assigns `jsonpArray.push = webpackJsonpCallback`
        // BEFORE the `function webpackJsonpCallback(data){...}` declaration appears
        // in source order. Without hoisting, webpackJsonpCallback is Undefined at the
        // assignment site and the override is silently not stored.
        let effects = run_two_scripts(
            // Script A — Webpack runtime with function declaration AFTER assignment
            r#"
                (function(modules) {
                    var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
                    var oldPush = jsonpArray.push.bind(jsonpArray);
                    jsonpArray.push = webpackJsonpCallback;   // ← uses fn before declaration
                    jsonpArray = jsonpArray.slice();

                    function webpackJsonpCallback(data) {     // ← declared after usage
                        document.getElementById("app").textContent = "callback:" + data[0];
                    }
                })({});
            "#,
            // Script B — chunk calls push
            r#"
                (window["webpackJsonp"] = window["webpackJsonp"] || []).push(["chunk6"]);
            "#,
        );
        assert_eq!(
            effects,
            vec![text("app", "callback:chunk6")],
            "webpackJsonpCallback must be hoisted so push override is set correctly: {effects:?}"
        );
    }

    #[test]
    fn t067g_webpack_hoisted_callback_uses_iife_closure() {
        // t067f proved the override fires. This test adds closure variables (installedModules,
        // __webpack_require__) to verify the re-registered function has the full closure,
        // not the early hoisted snapshot.
        let effects = run_two_scripts(
            r#"
                (function(modules) {
                    var installedModules = {};
                    var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
                    jsonpArray.push = webpackJsonpCallback;

                    function __webpack_require__(id) {
                        if (installedModules[id]) { return installedModules[id].exports; }
                        var mod = installedModules[id] = { exports: {} };
                        modules[id].call(mod.exports, mod, mod.exports, __webpack_require__);
                        return mod.exports;
                    }

                    function webpackJsonpCallback(data) {
                        var moreModules = data[1];
                        for (var id in moreModules) { modules[id] = moreModules[id]; }
                        __webpack_require__(data[2][0]);
                    }
                })({
                    "entry": function(module, exports) {
                        document.getElementById("app").textContent = "webpack-loaded";
                    }
                });
            "#,
            r#"
                (window["webpackJsonp"] = window["webpackJsonp"] || []).push([
                    "chunk1",
                    {},
                    ["entry"]
                ]);
            "#,
        );
        assert!(
            effects.contains(&text("app", "webpack-loaded")),
            "callback must use IIFE-captured installedModules and __webpack_require__: {effects:?}"
        );
    }

    fn run_two_scripts(script_a: &str, script_b: &str) -> Vec<BrowserEffect> {
        let prog_a = crate::parse_script(script_a).expect("parse script_a");
        let prog_b = crate::parse_script(script_b).expect("parse script_b");
        let mut state = BrowserExecutionState::default();
        state.execute_program(&prog_a);
        let _ = state.drain_effects(); // discard script_a effects (as the browser does)
        state.execute_program(&prog_b);
        state.drain_effects()
    }

    #[test]
    fn t067a_webpack_push_override_survives_script_boundary() {
        // Script A (Webpack runtime, single-script): sets up the webpackJsonp global
        // and overrides push. Script B (chunk): calls push. The override must fire.
        let effects = run_two_scripts(
            // Script A — Webpack runtime IIFE
            r#"
                (function() {
                    var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
                    jsonpArray.push = function(data) {
                        document.getElementById("app").textContent = "chunk:" + data[0];
                    };
                    jsonpArray = jsonpArray.slice();
                })();
            "#,
            // Script B — chunk push call (separate script)
            r#"
                (window["webpackJsonp"] = window["webpackJsonp"] || []).push(["chunk6"]);
            "#,
        );
        assert_eq!(
            effects,
            vec![text("app", "chunk:chunk6")],
            "push override from script A must fire when script B calls push: {effects:?}"
        );
    }

    #[test]
    fn t067b_window_named_push_after_computed_init_cross_script() {
        // Script A uses computed init; Script B calls push via named window.webpackJsonp.
        let effects = run_two_scripts(
            r#"
                (function() {
                    var arr = window["webpackJsonp"] = window["webpackJsonp"] || [];
                    arr.push = function(data) {
                        document.getElementById("result").textContent = "named:" + data[0];
                    };
                })();
            "#,
            r#"
                window.webpackJsonp.push(["via-named"]);
            "#,
        );
        assert_eq!(
            effects,
            vec![text("result", "named:via-named")],
            "push override must be reachable via named window.webpackJsonp after computed init: {effects:?}"
        );
    }

    #[test]
    fn t067c_globals_webpackjsonp_set_by_computed_write_in_iife() {
        // Verify that window["webpackJsonp"] = [] inside an IIFE actually persists
        // to globals, so a subsequent script can read it via window.webpackJsonp.
        let effects = run_two_scripts(
            r#"
                (function() {
                    window["webpackJsonp"] = window["webpackJsonp"] || [];
                })();
            "#,
            r#"
                var arr = window.webpackJsonp;
                document.getElementById("out").textContent = Array.isArray(arr) ? "array" : "not-array";
            "#,
        );
        assert_eq!(
            effects,
            vec![text("out", "array")],
            "window[\"webpackJsonp\"] set inside IIFE must be readable as window.webpackJsonp in next script: {effects:?}"
        );
    }

    #[test]
    fn t067d_push_override_set_in_iife_fires_from_chunk_script() {
        // Full Webpack-style two-script test: IIFE sets override, chunk calls push,
        // callback invokes __webpack_require__, factory produces a DOM effect.
        let effects = run_two_scripts(
            // Webpack runtime IIFE (script 5 analogue)
            r#"
                (function(modules) {
                    var installedModules = {};
                    function __webpack_require__(moduleId) {
                        if (installedModules[moduleId]) {
                            return installedModules[moduleId].exports;
                        }
                        var module = installedModules[moduleId] = { id: moduleId, exports: {} };
                        modules[moduleId].call(module.exports, module, module.exports, __webpack_require__);
                        return module.exports;
                    }
                    function webpackJsonpCallback(data) {
                        var moreModules = data[1];
                        var executeModules = data[2];
                        for (var id in moreModules) {
                            modules[id] = moreModules[id];
                        }
                        if (executeModules) {
                            for (var i = 0; i < executeModules.length; i++) {
                                __webpack_require__(executeModules[i]);
                            }
                        }
                    }
                    var jsonpArray = window["webpackJsonp"] = window["webpackJsonp"] || [];
                    var oldPush = jsonpArray.push.bind(jsonpArray);
                    jsonpArray.push = webpackJsonpCallback;
                    jsonpArray = jsonpArray.slice();
                })({});
            "#,
            // Chunk script (script 6 analogue) — registers module and triggers entry
            r#"
                (window["webpackJsonp"] = window["webpackJsonp"] || []).push([
                    ["app"],
                    {
                        "entry": function(module, exports, require) {
                            document.getElementById("app").textContent = "booted";
                        }
                    },
                    ["entry"]
                ]);
            "#,
        );
        assert_eq!(
            effects,
            vec![text("app", "booted")],
            "webpackJsonpCallback must run from chunk script and execute the entry module: {effects:?}"
        );
    }

    #[test]
    fn t068a_array_push_apply_writes_back() {
        // f.push.apply(f, [1,2,3]) must actually mutate f.
        // Accessing f.push emits a prototype.lookup trace — filter it out and check only DOM effects.
        let effects = run(r#"
            var f = [];
            f.push.apply(f, [1, 2, 3]);
            document.getElementById("out").textContent = String(f.length);
        "#);
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("out", "3")], "push.apply must write items back to f: {effects:?}");
    }

    #[test]
    fn t068b_webpack_entry_chain_via_push_apply() {
        // Minimal Webpack 4 pattern: runtime's r(data) uses f.push.apply(f, executeModules)
        // to collect entry module IDs, then drains f with __webpack_require__.
        // Entry module triggers a DOM effect — confirms the full chain works.
        let effects = run(r#"
            var installedModules = {};
            var modules = {};
            var f = [];
            function __webpack_require__(id) {
                if (installedModules[id]) return installedModules[id].exports;
                var mod = installedModules[id] = { id: id, exports: {} };
                modules[id].call(mod.exports, mod, mod.exports, __webpack_require__);
                return mod.exports;
            }
            function r(data) {
                var moreModules = data[1] || {};
                var executeModules = data[2];
                for (var id in moreModules) {
                    modules[id] = moreModules[id];
                }
                f.push.apply(f, executeModules || []);
                while (f.length) {
                    __webpack_require__(f.shift());
                }
            }
            r([
                ["chunk0"],
                {
                    374: function(mod, exports, require) {
                        document.getElementById("root").textContent = "webpack-loaded";
                    }
                },
                [374]
            ]);
        "#);
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("root", "webpack-loaded")], "entry module must execute via push.apply: {effects:?}");
    }

    // ── AMIUnique exact-pattern tests ──────────────────────────────────────────
    // These reproduce the literal code from https://www.amiunique.org/_nuxt/13ede67.js
    // and its chunk scripts, using named `window.webpackJsonp` (not computed bracket).

    #[test]
    fn t069a_named_window_push_override_stored_and_called() {
        // Simplest possible named-member override: runtime stores override via
        // `d.push = r` where `d = window.webpackJsonp = ...`, chunk calls
        // `(window.webpackJsonp = window.webpackJsonp || []).push(...)`.
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        document.getElementById("out").textContent = "r-called";
                    }
                    var d = window.webpackJsonp = window.webpackJsonp || [];
                    d.push = r;
                    d = d.slice();
                }([]);
            "#,
            r#"
                (window.webpackJsonp = window.webpackJsonp || []).push([["c0"], {}, []]);
            "#,
        );
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("out", "r-called")],
            "named window.webpackJsonp override must fire from chunk: {effects:?}");
    }

    #[test]
    fn t069b_named_window_push_override_with_bind_line() {
        // Adds `l = d.push.bind(d)` between the init and override — mirrors the
        // exact runtime sequence and confirms the bind call doesn't clobber the override.
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        document.getElementById("out").textContent = "r-called";
                    }
                    var d = window.webpackJsonp = window.webpackJsonp || [], l = d.push.bind(d);
                    d.push = r;
                    d = d.slice();
                    var v = l;
                }([]);
            "#,
            r#"
                (window.webpackJsonp = window.webpackJsonp || []).push([["c0"], {}, []]);
            "#,
        );
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("out", "r-called")],
            "override must survive bind() call and slice() reassignment: {effects:?}");
    }

    #[test]
    fn t069b2_r_registers_module_and_calls_t() {
        // Tests r() registering a module into e[] and t() requiring it via c().
        // Uses simplified (but structurally correct) versions of r, t, c.
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        var mods = data[1];
                        var entries = data[2];
                        for (var k in mods) { e[k] = mods[k]; }
                        f.push.apply(f, entries || []);
                        t();
                    }
                    function t() {
                        while (f.length) {
                            var spec = f.shift();
                            c(spec[0]);
                        }
                    }
                    var n = {}, f = [];
                    function c(id) {
                        if (n[id]) return n[id].exports;
                        var mod = n[id] = { exports: {} };
                        e[id].call(mod.exports, mod, mod.exports, c);
                        return mod.exports;
                    }
                    var d = window.webpackJsonp = window.webpackJsonp || [], l = d.push.bind(d);
                    d.push = r;
                    d = d.slice();
                }([]);
            "#,
            r#"
                (window.webpackJsonp = window.webpackJsonp || []).push([
                    [65],
                    { 374: function(mod, exports, req) {
                        document.getElementById("root").textContent = "loaded";
                    }},
                    [[374]]
                ]);
            "#,
        );
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("root", "loaded")],
            "r() must register module and t() must require it: {effects:?}");
    }

    #[test]
    fn t069b2b_for_loop_multi_var_init_with_data_index() {
        // Tests that `for(var r,n,c=data[0],d=data[1],i=0; i<c.length; i++)` parses and
        // executes correctly — this is the exact pattern in the real Webpack runtime.
        let effects = run(r#"
            function test(data) {
                for(var r,n,c=data[0],d=data[1],i=0; i<c.length; i++) {
                    n = c[i];
                }
                document.getElementById("out").textContent = n;
            }
            test([[42], {}, []]);
        "#);
        assert!(
            effects.iter().any(|e| matches!(e, BrowserEffect::SetTextContent { element_id, value }
                if element_id == "out" && value == "42")),
            "for loop with multi-var init must execute: {effects:?}"
        );
    }

    #[test]
    fn t069b3_r_with_shadowed_var_names_registers_module() {
        // r() declares var c,d,l inside its body (shadowing IIFE's c function and d/l).
        // Verifies that module registration (e[key]=mods[key]) still works through
        // the captured closure despite the shadowing.
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        for(var r,n,c=data[0],d=data[1],l=data[2],i=0;i<c.length;i++) {
                            o[c[i]] = 0;
                        }
                        for(r in d) { e[r] = d[r]; }
                        f.push.apply(f, l || []);
                        t();
                    }
                    function t() {
                        while (f.length) {
                            var spec = f.shift();
                            mod_c(spec[0]);
                        }
                    }
                    var n = {}, o = {77:0}, f = [];
                    function mod_c(id) {
                        if (n[id]) return n[id].exports;
                        var mod = n[id] = { exports: {} };
                        e[id].call(mod.exports, mod, mod.exports, mod_c);
                        return mod.exports;
                    }
                    var d = window.webpackJsonp = window.webpackJsonp || [], l = d.push.bind(d);
                    d.push = r;
                    d = d.slice();
                }([]);
            "#,
            r#"
                (window.webpackJsonp = window.webpackJsonp || []).push([
                    [65],
                    { 374: function(mod, exports, req) {
                        document.getElementById("root").textContent = "shadowed-ok";
                    }},
                    [[374]]
                ]);
            "#,
        );
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("root", "shadowed-ok")],
            "shadowed var names inside r must not break module registration: {effects:?}");
    }

    #[test]
    fn t069c_amiunique_full_webpack_bootstrap_named() {
        // Full AMIUnique-style bootstrap: the IIFE mirrors the actual 13ede67.js
        // structure (shadowed var names inside r, module registration into e, t()
        // dispatch). The chunk registers a module that produces a DOM effect.
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        for(var r,n,c=data[0],d=data[1],l=data[2],i=0,h=[];i<c.length;i++)
                            n=c[i],
                            o[n]=0;
                        for(r in d)
                            e[r]=d[r];
                        return f.push.apply(f,l||[]),t();
                    }
                    function t() {
                        for(var e,i=0;i<f.length;i++){
                            var r=f[i];
                            var en=r[0];
                            f.splice(i--,1);
                            e=c(en);
                        }
                        return e;
                    }
                    var n={},o={77:0},f=[];
                    function c(r){
                        if(n[r])return n[r].exports;
                        var t=n[r]={i:r,l:false,exports:{}};
                        e[r].call(t.exports,t,t.exports,c);
                        t.l=true;
                        return t.exports;
                    }
                    var d=window.webpackJsonp=window.webpackJsonp||[],l=d.push.bind(d);
                    d.push=r;
                    d=d.slice();
                    for(var i=0;i<d.length;i++)r(d[i]);
                    var v=l;
                    t();
                }([]);
            "#,
            r#"
                (window.webpackJsonp=window.webpackJsonp||[]).push([
                    [65],
                    {
                        374: function(mod, exports, require) {
                            document.getElementById("root").textContent = "amiunique-loaded";
                        }
                    },
                    [[374]]
                ]);
            "#,
        );
        let dom: Vec<_> = effects.iter()
            .filter(|e| !matches!(e, BrowserEffect::RuntimeTrace { .. }))
            .cloned()
            .collect();
        assert_eq!(dom, vec![text("root", "amiunique-loaded")],
            "full named-window AMIUnique bootstrap must reach module 374: {effects:?}");
    }

    #[test]
    fn t069d_debug_t069c_push_fires() {
        // Step 1: does the push override even fire?
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        document.getElementById("step1").textContent = "r-called";
                    }
                    var d=window.webpackJsonp=window.webpackJsonp||[];
                    d.push=r;
                    d=d.slice();
                }([]);
            "#,
            r#"
                (window.webpackJsonp=window.webpackJsonp||[]).push([[65],{374:function(m,x,q){ document.getElementById("step2").textContent = "module-ran"; }},[[374]]]);
            "#,
        );
        let dom: Vec<_> = effects.iter().filter(|e| !matches!(e, BrowserEffect::RuntimeTrace{..})).cloned().collect();
        assert!(dom.iter().any(|e| matches!(e, BrowserEffect::SetTextContent{element_id,..} if element_id=="step1")),
            "push override r must fire: {dom:?}");
    }

    #[test]
    fn t069f_debug_t069c_t_and_c_chain() {
        // Step 3: does t()->c()->e[374].call() work with the exact t/c bodies from t069c?
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        var c2=data[0],d2=data[1],l2=data[2];
                        for(var k in d2) e[k]=d2[k];
                        f.push.apply(f,l2||[]);
                        t();
                    }
                    function t() {
                        for(var e,i=0;i<f.length;i++){
                            var r=f[i];
                            var en=r[0];
                            f.splice(i--,1);
                            e=c(en);
                        }
                        return e;
                    }
                    var n={},o={},f=[];
                    function c(r){
                        if(n[r])return n[r].exports;
                        var t=n[r]={i:r,l:false,exports:{}};
                        e[r].call(t.exports,t,t.exports,c);
                        t.l=true;
                        return t.exports;
                    }
                    var d=window.webpackJsonp=window.webpackJsonp||[];
                    d.push=r;
                    d=d.slice();
                }([]);
            "#,
            r#"
                (window.webpackJsonp=window.webpackJsonp||[]).push([[65],{374:function(m,x,q){ document.getElementById("root").textContent = "amiunique-loaded"; }},[[374]]]);
            "#,
        );
        let dom: Vec<_> = effects.iter().filter(|e| !matches!(e, BrowserEffect::RuntimeTrace{..})).cloned().collect();
        assert_eq!(dom, vec![text("root", "amiunique-loaded")],
            "t->c chain must reach module 374: {dom:?}");
    }

    #[test]
    fn t069e_debug_t069c_module_e_populated() {
        // Step 2: does e[374] get correctly populated with the module factory?
        let effects = run_two_scripts(
            r#"
                !function(e) {
                    function r(data) {
                        var c=data[0],d=data[1],l=data[2];
                        for(var k in d) e[k]=d[k];
                        if(typeof e[374] === 'function') {
                            document.getElementById("e374").textContent = "e374-set";
                        }
                    }
                    var d=window.webpackJsonp=window.webpackJsonp||[];
                    d.push=r;
                    d=d.slice();
                }([]);
            "#,
            r#"
                (window.webpackJsonp=window.webpackJsonp||[]).push([[65],{374:function(m,x,q){ document.getElementById("inner").textContent = "inner-ran"; }},[[374]]]);
            "#,
        );
        let dom: Vec<_> = effects.iter().filter(|e| !matches!(e, BrowserEffect::RuntimeTrace{..})).cloned().collect();
        assert!(dom.iter().any(|e| matches!(e, BrowserEffect::SetTextContent{element_id,..} if element_id=="e374")),
            "e[374] must be set to the module factory: {dom:?}");
    }
}
