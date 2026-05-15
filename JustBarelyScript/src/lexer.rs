#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(String),
    String(String),
    Let,
    Const,
    Var,
    Function,
    Return,
    If,
    Else,
    While,
    For,
    True,
    False,
    Null,
    Undefined,
    New,
    This,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equals,
    EqualEqual,
    EqualEqualEqual,
    Bang,
    BangEqual,
    BangEqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AmpAmp,
    PipePipe,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Semicolon,
    Comma,
    Dot,
    Colon,
    Question,
    Eof,
}

pub fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source).collect()
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,
    position: usize,
    byte_position: usize,
    line: usize,
    column: usize,
    emitted_eof: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            position: 0,
            byte_position: 0,
            line: 1,
            column: 1,
            emitted_eof: false,
        }
    }

    fn current(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.current()?;
        self.position += 1;
        self.byte_position += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn start_span(&self) -> Span {
        Span {
            start: self.byte_position,
            end: self.byte_position,
            line: self.line,
            column: self.column,
        }
    }

    fn finish_span(&self, mut span: Span) -> Span {
        span.end = self.byte_position;
        span
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while matches!(self.current(), Some(ch) if ch.is_whitespace()) {
                self.bump();
            }

            if self.current() == Some('/') && self.peek() == Some('/') {
                while !matches!(self.current(), None | Some('\n')) {
                    self.bump();
                }
                continue;
            }

            if self.current() == Some('/') && self.peek() == Some('*') {
                self.bump();
                self.bump();
                while self.current().is_some() {
                    if self.current() == Some('*') && self.peek() == Some('/') {
                        self.bump();
                        self.bump();
                        break;
                    }
                    self.bump();
                }
                continue;
            }

            break;
        }
    }

    fn identifier_or_keyword(&mut self, span: Span) -> Token {
        while matches!(self.current(), Some(ch) if is_identifier_part(ch)) {
            self.bump();
        }
        let text = &self.source[span.start..self.byte_position];
        let kind = match text {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "function" => TokenKind::Function,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            "new" => TokenKind::New,
            "this" => TokenKind::This,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        Token {
            kind,
            span: self.finish_span(span),
        }
    }

    fn number(&mut self, span: Span) -> Token {
        while matches!(self.current(), Some(ch) if ch.is_ascii_digit()) {
            self.bump();
        }
        if self.current() == Some('.') && matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            self.bump();
            while matches!(self.current(), Some(ch) if ch.is_ascii_digit()) {
                self.bump();
            }
        }
        Token {
            kind: TokenKind::Number(self.source[span.start..self.byte_position].to_owned()),
            span: self.finish_span(span),
        }
    }

    fn string(&mut self, span: Span, quote: char) -> Token {
        let mut value = String::new();
        self.bump();
        while let Some(ch) = self.current() {
            if ch == quote {
                self.bump();
                break;
            }
            if ch == '\\' {
                self.bump();
                let escaped = match self.current() {
                    Some('n') => '\n',
                    Some('r') => '\r',
                    Some('t') => '\t',
                    Some('"') => '"',
                    Some('\'') => '\'',
                    Some('\\') => '\\',
                    Some(other) => other,
                    None => break,
                };
                value.push(escaped);
                self.bump();
            } else {
                value.push(ch);
                self.bump();
            }
        }
        Token {
            kind: TokenKind::String(value),
            span: self.finish_span(span),
        }
    }

    fn simple(&mut self, span: Span, kind: TokenKind) -> Token {
        self.bump();
        Token {
            kind,
            span: self.finish_span(span),
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_ws_and_comments();
        let span = self.start_span();
        let ch = self.current();
        let token = match ch {
            None if self.emitted_eof => return None,
            None => {
                self.emitted_eof = true;
                Token {
                    kind: TokenKind::Eof,
                    span,
                }
            }
            Some(ch) if is_identifier_start(ch) => self.identifier_or_keyword(span),
            Some(ch) if ch.is_ascii_digit() => self.number(span),
            Some('"') | Some('\'') => self.string(span, ch.unwrap()),
            Some('+') => self.simple(span, TokenKind::Plus),
            Some('-') => self.simple(span, TokenKind::Minus),
            Some('*') => self.simple(span, TokenKind::Star),
            Some('/') => self.simple(span, TokenKind::Slash),
            Some('%') => self.simple(span, TokenKind::Percent),
            Some('(') => self.simple(span, TokenKind::LeftParen),
            Some(')') => self.simple(span, TokenKind::RightParen),
            Some('{') => self.simple(span, TokenKind::LeftBrace),
            Some('}') => self.simple(span, TokenKind::RightBrace),
            Some('[') => self.simple(span, TokenKind::LeftBracket),
            Some(']') => self.simple(span, TokenKind::RightBracket),
            Some(';') => self.simple(span, TokenKind::Semicolon),
            Some(',') => self.simple(span, TokenKind::Comma),
            Some('.') => self.simple(span, TokenKind::Dot),
            Some(':') => self.simple(span, TokenKind::Colon),
            Some('?') => self.simple(span, TokenKind::Question),
            Some('=') => {
                self.bump();
                if self.current() == Some('=') {
                    self.bump();
                    if self.current() == Some('=') {
                        self.bump();
                        Token {
                            kind: TokenKind::EqualEqualEqual,
                            span: self.finish_span(span),
                        }
                    } else {
                        Token {
                            kind: TokenKind::EqualEqual,
                            span: self.finish_span(span),
                        }
                    }
                } else {
                    Token {
                        kind: TokenKind::Equals,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('!') => {
                self.bump();
                if self.current() == Some('=') {
                    self.bump();
                    if self.current() == Some('=') {
                        self.bump();
                        Token {
                            kind: TokenKind::BangEqualEqual,
                            span: self.finish_span(span),
                        }
                    } else {
                        Token {
                            kind: TokenKind::BangEqual,
                            span: self.finish_span(span),
                        }
                    }
                } else {
                    Token {
                        kind: TokenKind::Bang,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('<') => {
                self.bump();
                if self.current() == Some('=') {
                    self.bump();
                    Token {
                        kind: TokenKind::LessEqual,
                        span: self.finish_span(span),
                    }
                } else {
                    Token {
                        kind: TokenKind::Less,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('>') => {
                self.bump();
                if self.current() == Some('=') {
                    self.bump();
                    Token {
                        kind: TokenKind::GreaterEqual,
                        span: self.finish_span(span),
                    }
                } else {
                    Token {
                        kind: TokenKind::Greater,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('&') if self.peek() == Some('&') => {
                self.bump();
                self.bump();
                Token {
                    kind: TokenKind::AmpAmp,
                    span: self.finish_span(span),
                }
            }
            Some('|') if self.peek() == Some('|') => {
                self.bump();
                self.bump();
                Token {
                    kind: TokenKind::PipePipe,
                    span: self.finish_span(span),
                }
            }
            Some(_) => self.simple(span, TokenKind::Eof),
        };
        Some(token)
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}
