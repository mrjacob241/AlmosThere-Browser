use crate::lexer::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsErrorKind {
    Lex,
    Parse,
    Runtime,
    Type,
    Reference,
    Security,
    Host,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JsError {
    pub kind: JsErrorKind,
    pub message: String,
    pub span: Option<Span>,
}

impl JsError {
    pub fn new(kind: JsErrorKind, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            kind,
            message: message.into(),
            span,
        }
    }

    pub fn parse(message: impl Into<String>, span: Span) -> Self {
        Self::new(JsErrorKind::Parse, message, Some(span))
    }

    pub fn security(message: impl Into<String>) -> Self {
        Self::new(JsErrorKind::Security, message, None)
    }

    pub fn diagnostic_message(&self) -> String {
        match self.span {
            Some(span) => format!(
                "{:?} error at line {}, column {}: {}",
                self.kind, span.line, span.column, self.message
            ),
            None => format!("{:?} error: {}", self.kind, self.message),
        }
    }
}

impl std::fmt::Display for JsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic_message())
    }
}

impl std::error::Error for JsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_span_diagnostics() {
        let error = JsError::parse(
            "expected expression",
            Span {
                start: 5,
                end: 6,
                line: 2,
                column: 3,
            },
        );

        assert_eq!(
            error.diagnostic_message(),
            "Parse error at line 2, column 3: expected expression"
        );
    }
}
