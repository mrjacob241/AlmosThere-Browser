use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    Program,
    ast::{
        BinaryOperator, Binding, BlockStatement, Expression, FunctionBody, MemberProperty,
        ObjectProperty, Param, Statement, SwitchStatement, UnaryOperator, VariableDeclaration,
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
    pub params: Vec<Param>,
    pub body: FunctionBody,
    pub captured: Vec<StackFrame>,
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
}

#[derive(Clone, Debug)]
struct StackFrame {
    locals: Rc<RefCell<HashMap<String, JsValue>>>,
}

impl Default for StackFrame {
    fn default() -> Self {
        StackFrame {
            locals: Rc::new(RefCell::new(HashMap::new())),
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
    CanvasContextRef(String),
    DateInstance,
    ResolvedPromise,
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
            .insert("navigator".into(), JsValue::Object(obj));
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
            obj.insert(
                "hostname".into(),
                JsValue::String(host.split(':').next().unwrap_or("").to_owned()),
            );
            obj.insert(
                "pathname".into(),
                JsValue::String(
                    rest[host_end..]
                        .split(['?', '#'])
                        .next()
                        .filter(|path| !path.is_empty())
                        .unwrap_or("/")
                        .to_owned(),
                ),
            );
        } else {
            obj.insert("protocol".into(), JsValue::String(String::new()));
            obj.insert("host".into(), JsValue::String(String::new()));
            obj.insert("hostname".into(), JsValue::String(String::new()));
            obj.insert("pathname".into(), JsValue::String(href.to_owned()));
        }

        self.globals.insert("location".into(), JsValue::Object(obj));
    }

    pub fn execute_program(&mut self, program: &Program) {
        self.ensure_global_frame();
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
            self.stack.push(StackFrame::default());
            self.execute_block(&task.body);
            self.stack.pop();
            self.ensure_global_frame();
        }
    }

    pub fn drain_effects(&mut self) -> Vec<BrowserEffect> {
        self.effects.drain(..).collect()
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
                    params: decl.params.clone(),
                    body: FunctionBody::Block(decl.body.clone()),
                    captured: self.stack.clone(),
                };
                let name = decl.name.clone();
                self.set_local(&name, JsValue::Function(func));
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
                for item in items {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    self.stack.push(StackFrame::default());
                    self.execute_binding(&stmt.binding, item);
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
                for key in keys {
                    if self.execution_budget_exhausted {
                        break;
                    }
                    self.stack.push(StackFrame::default());
                    self.execute_binding(&stmt.binding, JsValue::String(key));
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

    fn execute_block(&mut self, block: &BlockStatement) {
        self.stack.push(StackFrame::default());
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
            self.execute_binding(&binding, value);
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
                params: fe.params.clone(),
                body: FunctionBody::Block(fe.body.clone()),
                captured: self.stack.clone(),
            }),
            Expression::ArrowFunction { params, body, .. } => JsValue::Function(JsFunction {
                params: params.clone(),
                body: *body.clone(),
                captured: self.stack.clone(),
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
                    self.get_binding(name).unwrap_or(JsValue::Undefined)
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
            Expression::New { callee, .. } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Date") {
                    JsValue::DateInstance
                } else {
                    JsValue::Undefined
                }
            }
            Expression::Spread(_) | Expression::Super => JsValue::Undefined,
            Expression::Identifier(name) => self.get_binding(name).unwrap_or(JsValue::Undefined),
            Expression::Number(value) => JsValue::Number(*value),
            Expression::String(value) => JsValue::String(value.clone()),
            Expression::Boolean(value) => JsValue::Boolean(*value),
            Expression::Null => JsValue::Null,
            Expression::Undefined | Expression::This => JsValue::Undefined,
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
                "addEventListener" => {
                    let receiver = self.execute_expression(&method.object);
                    let event_type = arguments
                        .first()
                        .map(|a| self.execute_expression(a))
                        .map(|v| Self::value_to_string(&v))
                        .unwrap_or_default();
                    // document.addEventListener("DOMContentLoaded", fn) — DOM is already
                    // parsed by the time scripts run, so fire as a microtask immediately.
                    if matches!(&method.object, Expression::Identifier(n) if n == "document")
                        && event_type == "DOMContentLoaded"
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
                    return JsValue::CanvasContextRef(context_name);
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

            // Function.prototype.call / apply / bind
            if let JsValue::Function(func) = receiver.clone() {
                match method_name.as_str() {
                    "call" => {
                        // func.call(thisArg, arg1, arg2, ...) — skip thisArg
                        let args = self.eval_args(arguments);
                        let real_args =
                            if args.is_empty() { vec![] } else { args[1..].to_vec() };
                        let real_exprs: Vec<Expression> = arguments
                            .iter()
                            .skip(1)
                            .cloned()
                            .collect();
                        return self.call_function_with_writeback(func, real_args, &real_exprs);
                    }
                    "apply" => {
                        // func.apply(thisArg, [arg1, arg2, ...]) — spread second arg
                        let mut iter = arguments.iter();
                        let _this_arg = iter.next().map(|a| self.execute_expression(a));
                        let args_val = iter
                            .next()
                            .map(|a| self.execute_expression(a))
                            .unwrap_or(JsValue::Undefined);
                        let real_args = match args_val {
                            JsValue::Array(arr) => arr,
                            _ => vec![],
                        };
                        return self.call_function(func, real_args);
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
                    return self.call_function(func, args);
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
            // Evaluated receiver but method not found — still evaluate args for side effects
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
            "defineProperties" | "getOwnPropertyDescriptor" | "getOwnPropertyNames"
            | "getOwnPropertySymbols" | "getPrototypeOf" | "setPrototypeOf" => {
                args.into_iter().next().unwrap_or(JsValue::Undefined)
            }
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
            "push" => {
                let new_len = arr.len()
                    + arguments
                        .iter()
                        .filter(|a| !matches!(a, Expression::Spread(_)))
                        .count();
                for arg in arguments {
                    self.execute_expression(arg);
                }
                Some(JsValue::Number(new_len as f64))
            }
            "unshift" => {
                let added = arguments.len();
                for arg in arguments {
                    self.execute_expression(arg);
                }
                Some(JsValue::Number((arr.len() + added) as f64))
            }
            "splice" => Some(JsValue::Array(Vec::new())),
            "pop" => {
                // read-only approximation: return last element
                Some(arr.into_iter().last().unwrap_or(JsValue::Undefined))
            }
            "shift" => Some(arr.into_iter().next().unwrap_or(JsValue::Undefined)),
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
                _ => {}
            }
        }

        if let Some(global_name) = window_member_name(target) {
            self.globals.insert(global_name, value);
            return;
        }

        // anyExpr.method = fn  →  store as array method override
        // Handles two cases:
        //   window.X.method = fn  → keyed "X:method"
        //   localVar.method = fn  → keyed "localVar:method", and also propagated to any
        //                           global that currently holds the same array value
        //                           (bridges the var d = window.X = []; d.push = r pattern)
        if let Expression::Member {
            object,
            property: MemberProperty::Named(method_name),
            ..
        } = target
        {
            if let Some(global_name) = window_member_name(object) {
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
            self.set_binding(name, value);
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
        if let Some(global_name) = window_member_name(expression) {
            return self
                .globals
                .get(&global_name)
                .cloned()
                .unwrap_or(JsValue::Undefined);
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
                match receiver {
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
                match receiver {
                    JsValue::String(ref s) if property == "length" => {
                        JsValue::Number(s.chars().count() as f64)
                    }
                    JsValue::ElementRef(element_ref) => {
                        if property == "style" {
                            return if let Some(id) = existing_id_from_ref(&element_ref) {
                                JsValue::StyleRef(id)
                            } else {
                                JsValue::Undefined
                            };
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
                        _ => JsValue::Undefined,
                    },
                    JsValue::Object(map) => map
                        .get(property.as_str())
                        .cloned()
                        .unwrap_or(JsValue::Undefined),
                    JsValue::NodeList(items) if property == "length" => {
                        JsValue::Number(items.len() as f64)
                    }
                    JsValue::Array(items) if property == "length" => {
                        JsValue::Number(items.len() as f64)
                    }
                    _ => JsValue::Undefined,
                }
            }
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

    fn append_child(&mut self, parent_ref: &str, child_ref: &str) {
        let Some(child) = self.dom.created_elements.get(child_ref).cloned() else {
            return;
        };
        if let Some(parent_id) = existing_id_from_ref(parent_ref) {
            self.effects
                .push(BrowserEffect::AppendChild { parent_id, child });
        }
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

    fn ensure_global_frame(&mut self) {
        if self.stack.is_empty() {
            self.stack.push(StackFrame::default());
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
        // Move captured frames directly — no clone needed since func is owned.
        let saved_stack = std::mem::replace(&mut self.stack, func.captured);
        self.ensure_global_frame();
        self.stack.push(StackFrame::default());
        self.bind_params(&func.params, args);
        let result = match func.body {
            FunctionBody::Block(block) => {
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
        self.stack.pop();
        self.ensure_global_frame();
        let _ = std::mem::replace(&mut self.stack, saved_stack);
        result
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

        let saved_stack = std::mem::replace(&mut self.stack, func.captured);
        self.ensure_global_frame();
        self.stack.push(StackFrame::default());
        self.bind_params(&func.params, args);
        let result = match func.body {
            FunctionBody::Block(block) => {
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

        // Write mutated Objects/Arrays back to the caller's argument expressions.
        for (final_val_opt, arg_expr) in final_values.iter().zip(arg_exprs.iter()) {
            if let Some(final_val) = final_val_opt {
                if matches!(final_val, JsValue::Object(_) | JsValue::Array(_)) {
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
                (((Self::value_to_number(&lv) as u32) >> (Self::value_to_number(&rv) as u32 & 31))
                    ) as f64,
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
            | JsValue::CanvasContextRef(_)
            | JsValue::DateInstance
            | JsValue::ResolvedPromise => true,
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
            | JsValue::CanvasContextRef(_)
            | JsValue::DateInstance
            | JsValue::ResolvedPromise => f64::NAN,
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
            JsValue::CanvasContextRef(_) => "[object CanvasRenderingContext]".to_owned(),
            JsValue::DateInstance => "[object Date]".to_owned(),
            JsValue::ResolvedPromise => "[object Promise]".to_owned(),
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
            self.stack.push(StackFrame::default());
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
            self.stack.push(StackFrame::default());
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

fn window_member_name(expression: &Expression) -> Option<String> {
    let Expression::Member {
        object,
        property,
        optional: _,
    } = expression
    else {
        return None;
    };
    if !matches!(object.as_ref(), Expression::Identifier(name) if name == "window") {
        return None;
    }
    let MemberProperty::Named(name) = property else {
        return None;
    };
    Some(name.clone())
}

/// For `window.X` and `(window.X = ...)` expressions, return the global name `X`.
/// Used to detect calls like `(window.webpackJsonp = ...).push(data)`.
fn extract_window_global_name(expr: &Expression) -> Option<String> {
    if let Some(name) = window_member_name(expr) {
        return Some(name);
    }
    if let Expression::Assignment { target, .. } = expr {
        return extract_window_global_name(target);
    }
    None
}

fn existing_element_ref(id: &str) -> String {
    format!("existing:{id}")
}

fn existing_id_from_ref(element_ref: &str) -> Option<String> {
    element_ref.strip_prefix("existing:").map(str::to_owned)
}

fn dom_property_is_text_content(property: &str) -> bool {
    matches!(property, "textContent" | "innerText")
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
        | JsValue::CanvasContextRef(_)
        | JsValue::DateInstance
        | JsValue::ResolvedPromise => "null".to_owned(),
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
        assert_eq!(effects, vec![
            text("a", "hello"), text("b", "world"),
            text("a", "foo"), text("b", "bar"),
            text("a", "x"), text("b", "y"),
        ]);
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
}
