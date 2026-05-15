use std::collections::HashMap;

use crate::{
    Program,
    ast::{
        BinaryOperator, BlockStatement, Expression, MemberProperty, ObjectProperty, Statement,
        UnaryOperator, VariableDeclaration,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BrowserExecutionState {
    pub dom: DomExecutionState,
    globals: HashMap<String, JsValue>,
    stack: Vec<StackFrame>,
    effects: Vec<BrowserEffect>,
    event_handlers: Vec<EventHandler>,
    pending_timers: Vec<PendingTimer>,
    pending_microtasks: Vec<PendingMicrotask>,
    pub current_time_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct StackFrame {
    locals: HashMap<String, JsValue>,
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
    ElementRef(String),
    NodeList(Vec<String>),
    StyleRef(String),
    ResolvedPromise,
}

pub fn collect_browser_effects(program: &Program) -> Vec<BrowserEffect> {
    let mut state = BrowserExecutionState::default();
    state.execute_program(program);
    state.drain_effects()
}

impl BrowserExecutionState {
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

    pub fn execute_program(&mut self, program: &Program) {
        self.ensure_global_frame();
        for statement in &program.body {
            self.execute_statement(statement);
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
            Statement::While(statement) => loop {
                let condition = self.execute_expression(&statement.test);
                if !Self::is_truthy(&condition) {
                    break;
                }
                self.execute_statement(&statement.body);
            },
            Statement::For(statement) => {
                self.stack.push(StackFrame::default());
                if let Some(init) = &statement.init {
                    self.execute_statement(init);
                }
                loop {
                    if let Some(test) = &statement.test {
                        let condition = self.execute_expression(test);
                        if !Self::is_truthy(&condition) {
                            break;
                        }
                    }
                    self.execute_statement(&statement.body);
                    if let Some(update) = &statement.update {
                        self.execute_expression(update);
                    }
                }
                self.stack.pop();
                self.ensure_global_frame();
            }
            Statement::FunctionDeclaration(_) | Statement::Return(_) | Statement::Empty => {}
        }
    }

    fn execute_block(&mut self, block: &BlockStatement) {
        self.stack.push(StackFrame::default());
        for statement in &block.body {
            self.execute_statement(statement);
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
            self.set_local(&declarator.id, value);
        }
    }

    fn execute_expression(&mut self, expression: &Expression) -> JsValue {
        match expression {
            Expression::Assignment { target, value } => {
                let value = self.execute_expression(value);
                self.assign_target(target, value.clone());
                value
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
                }
            }
            Expression::Array(items) => {
                let values: Vec<JsValue> = items
                    .iter()
                    .map(|item| self.execute_expression(item))
                    .collect();
                JsValue::Array(values)
            }
            Expression::Object(properties) => {
                JsValue::Object(self.object_from_properties(properties))
            }
            Expression::Function(_) => JsValue::Undefined,
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
                    params: func.params.clone(),
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
                                params: func.params.clone(),
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
                                params: func.params.clone(),
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
                                    params: func.params.clone(),
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

        self.execute_expression(callee);
        for argument in arguments {
            self.execute_expression(argument);
        }
        JsValue::Undefined
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
                _ => {}
            }
        }

        if let Some(global_name) = window_member_name(target) {
            self.globals.insert(global_name, value);
            return;
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
        let Expression::Member { object, property } = expression else {
            return JsValue::Undefined;
        };
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
                    _ => JsValue::Undefined,
                }
            }
            MemberProperty::Named(property) => {
                let receiver = self.execute_expression(object);
                match receiver {
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

    fn get_binding(&self, name: &str) -> Option<JsValue> {
        for frame in self.stack.iter().rev() {
            if let Some(value) = frame.locals.get(name) {
                return Some(value.clone());
            }
        }
        self.globals.get(name).cloned()
    }

    fn set_binding(&mut self, name: &str, value: JsValue) {
        for frame in self.stack.iter_mut().rev() {
            if frame.locals.contains_key(name) {
                frame.locals.insert(name.to_owned(), value);
                return;
            }
        }
        self.set_local(name, value);
    }

    fn set_local(&mut self, name: &str, value: JsValue) {
        self.ensure_global_frame();
        if let Some(frame) = self.stack.last_mut() {
            frame.locals.insert(name.to_owned(), value);
        }
    }

    fn ensure_global_frame(&mut self) {
        if self.stack.is_empty() {
            self.stack.push(StackFrame::default());
        }
    }

    fn execute_binary(
        &mut self,
        op: &BinaryOperator,
        left: &Expression,
        right: &Expression,
    ) -> JsValue {
        // Short-circuit logical operators before evaluating both sides.
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
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
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
            | JsValue::ElementRef(_)
            | JsValue::NodeList(_)
            | JsValue::StyleRef(_)
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
            | JsValue::ElementRef(_)
            | JsValue::NodeList(_)
            | JsValue::StyleRef(_)
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
            JsValue::ElementRef(_) => "[object Element]".to_owned(),
            JsValue::NodeList(_) => "[object NodeList]".to_owned(),
            JsValue::StyleRef(_) => "[object CSSStyleDeclaration]".to_owned(),
            JsValue::ResolvedPromise => "[object Promise]".to_owned(),
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
                if let Some(frame) = self.stack.last_mut() {
                    frame
                        .locals
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
    let Expression::Member { object, property } = expression else {
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
    let Expression::Member { object, property } = expression else {
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
    let Expression::Member { object, property } = expression else {
        return None;
    };
    let MemberProperty::Named(property) = property else {
        return None;
    };
    Some((object.as_ref(), property.clone()))
}

fn window_member_name(expression: &Expression) -> Option<String> {
    let Expression::Member { object, property } = expression else {
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
        JsValue::ElementRef(_)
        | JsValue::NodeList(_)
        | JsValue::StyleRef(_)
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
}
