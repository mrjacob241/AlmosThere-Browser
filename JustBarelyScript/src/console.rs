#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleMessage {
    pub level: ConsoleLevel,
    pub text: String,
}

pub trait ConsoleSink {
    fn message(&mut self, level: ConsoleLevel, text: String);

    fn log(&mut self, text: impl Into<String>) {
        self.message(ConsoleLevel::Log, text.into());
    }

    fn info(&mut self, text: impl Into<String>) {
        self.message(ConsoleLevel::Info, text.into());
    }

    fn warn(&mut self, text: impl Into<String>) {
        self.message(ConsoleLevel::Warn, text.into());
    }

    fn error(&mut self, text: impl Into<String>) {
        self.message(ConsoleLevel::Error, text.into());
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VecConsole {
    pub messages: Vec<ConsoleMessage>,
}

impl ConsoleSink for VecConsole {
    fn message(&mut self, level: ConsoleLevel, text: String) {
        self.messages.push(ConsoleMessage { level, text });
    }
}

pub const INTERPRETER_CONSOLE_INSTRUCTIONS: &str = "\
Interpreter console policy:
1. Route console.log/info/warn/error through ConsoleSink.
2. Never expose filesystem, process, shell, deletion, creation, or network capabilities through console.
3. Convert JavaScript values to display strings before calling ConsoleSink.
4. Report parser/runtime failures with ConsoleLevel::Error so the browser Console tab can highlight them.
";

pub fn collect_static_console_messages(program: &Program) -> Vec<ConsoleMessage> {
    let mut out = Vec::new();
    for statement in &program.body {
        collect_statement_console_messages(statement, &mut out);
    }
    out
}

fn collect_statement_console_messages(statement: &Statement, out: &mut Vec<ConsoleMessage>) {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let Some(init) = &declarator.init {
                    collect_expression_console_messages(init, out);
                }
            }
        }
        Statement::FunctionDeclaration(declaration) => {
            collect_block_console_messages(&declaration.body, out);
        }
        Statement::ClassDeclaration(_)
        | Statement::Throw(_)
        | Statement::ForOf(_)
        | Statement::ForIn(_)
        | Statement::TryCatch(_)
        | Statement::Switch(_)
        | Statement::Break(_)
        | Statement::Continue(_) => {}
        Statement::Return(statement) => {
            if let Some(argument) = &statement.argument {
                collect_expression_console_messages(argument, out);
            }
        }
        Statement::If(statement) => {
            collect_expression_console_messages(&statement.test, out);
            collect_statement_console_messages(&statement.consequent, out);
            if let Some(alternate) = &statement.alternate {
                collect_statement_console_messages(alternate, out);
            }
        }
        Statement::While(statement) => {
            collect_expression_console_messages(&statement.test, out);
            collect_statement_console_messages(&statement.body, out);
        }
        Statement::For(statement) => {
            if let Some(init) = &statement.init {
                collect_statement_console_messages(init, out);
            }
            if let Some(test) = &statement.test {
                collect_expression_console_messages(test, out);
            }
            if let Some(update) = &statement.update {
                collect_expression_console_messages(update, out);
            }
            collect_statement_console_messages(&statement.body, out);
        }
        Statement::Block(block) => collect_block_console_messages(block, out),
        Statement::Expression(expression) => collect_expression_console_messages(expression, out),
        Statement::Empty => {}
    }
}

fn collect_block_console_messages(block: &BlockStatement, out: &mut Vec<ConsoleMessage>) {
    for statement in &block.body {
        collect_statement_console_messages(statement, out);
    }
}

fn collect_expression_console_messages(expression: &Expression, out: &mut Vec<ConsoleMessage>) {
    match expression {
        Expression::Call { callee, arguments } => {
            if let Some(level) = console_call_level(callee) {
                out.push(ConsoleMessage {
                    level,
                    text: arguments
                        .iter()
                        .map(display_static_console_argument)
                        .collect::<Vec<_>>()
                        .join(" "),
                });
            }
            collect_expression_console_messages(callee, out);
            for argument in arguments {
                collect_expression_console_messages(argument, out);
            }
        }
        Expression::Array(items) => {
            for item in items {
                collect_expression_console_messages(item, out);
            }
        }
        Expression::Object(properties) => {
            for property in properties {
                collect_expression_console_messages(&property.value, out);
            }
        }
        Expression::Function(_) => {} // deferred callbacks — don't collect as synchronous output
        Expression::Binary { left, right, .. } => {
            collect_expression_console_messages(left, out);
            collect_expression_console_messages(right, out);
        }
        Expression::Unary { expr, .. } => collect_expression_console_messages(expr, out),
        Expression::Assignment { target, value } => {
            collect_expression_console_messages(target, out);
            collect_expression_console_messages(value, out);
        }
        Expression::Ternary {
            test,
            consequent,
            alternate,
        } => {
            collect_expression_console_messages(test, out);
            collect_expression_console_messages(consequent, out);
            collect_expression_console_messages(alternate, out);
        }
        Expression::Member {
            object,
            property,
            optional: _,
        } => {
            collect_expression_console_messages(object, out);
            if let MemberProperty::Computed(expression) = property {
                collect_expression_console_messages(expression, out);
            }
        }
        Expression::ArrowFunction { .. }
        | Expression::New { .. }
        | Expression::Await(_)
        | Expression::Typeof(_)
        | Expression::Void(_)
        | Expression::Delete(_)
        | Expression::Spread(_)
        | Expression::TemplateLiteral(_)
        | Expression::Super
        | Expression::Identifier(_)
        | Expression::Number(_)
        | Expression::String(_)
        | Expression::Boolean(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::This => {}
    }
}

fn console_call_level(callee: &Expression) -> Option<ConsoleLevel> {
    let Expression::Member {
        object,
        property,
        optional: _,
    } = callee
    else {
        return None;
    };
    if !matches!(object.as_ref(), Expression::Identifier(name) if name == "console") {
        return None;
    }
    let MemberProperty::Named(name) = property else {
        return None;
    };
    match name.as_str() {
        "log" => Some(ConsoleLevel::Log),
        "info" => Some(ConsoleLevel::Info),
        "warn" => Some(ConsoleLevel::Warn),
        "error" => Some(ConsoleLevel::Error),
        _ => None,
    }
}

fn display_static_console_argument(expression: &Expression) -> String {
    match expression {
        Expression::String(value) => value.clone(),
        Expression::Number(value) => value.to_string(),
        Expression::Boolean(value) => value.to_string(),
        Expression::Null => "null".to_owned(),
        Expression::Undefined => "undefined".to_owned(),
        Expression::Identifier(name) => format!("<{name}>"),
        _ => "<expression>".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_console_records_standard_levels() {
        let mut console = VecConsole::default();

        console.log("hello");
        console.error("broken");

        assert_eq!(
            console.messages,
            vec![
                ConsoleMessage {
                    level: ConsoleLevel::Log,
                    text: "hello".to_owned(),
                },
                ConsoleMessage {
                    level: ConsoleLevel::Error,
                    text: "broken".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn collects_static_console_calls() {
        let program = crate::parse_script(r#"console.log("ready"); console.error("broken");"#)
            .expect("script should parse");

        assert_eq!(
            collect_static_console_messages(&program),
            vec![
                ConsoleMessage {
                    level: ConsoleLevel::Log,
                    text: "ready".to_owned(),
                },
                ConsoleMessage {
                    level: ConsoleLevel::Error,
                    text: "broken".to_owned(),
                },
            ]
        );
    }
}
use crate::{
    Program,
    ast::{BlockStatement, Expression, MemberProperty, Statement},
};
