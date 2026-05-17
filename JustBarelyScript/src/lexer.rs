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
pub enum TemplatePart {
    Str(String),
    Expr(String), // raw source inside ${}
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(String),
    String(String),
    Regex(String), // /pattern/flags — treated as opaque string
    TemplateLiteral(Vec<TemplatePart>),
    PrivateIdentifier(String),
    // keywords
    Let,
    Const,
    Var,
    Function,
    Return,
    If,
    Else,
    Do,
    While,
    For,
    True,
    False,
    Null,
    Undefined,
    New,
    This,
    Throw,
    Try,
    Catch,
    Finally,
    Class,
    Extends,
    Super,
    Static,
    Async,
    Await,
    Import,
    Export,
    Typeof,
    Void,
    Delete,
    In,
    Instanceof,
    Of,
    Break,
    Continue,
    Switch,
    Case,
    Default,
    // arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    StarStar,
    // assignment
    Equals,
    PlusEquals,
    MinusEquals,
    StarEquals,
    SlashEquals,
    PercentEquals,
    StarStarEquals,
    AmpEquals,
    PipeEquals,
    CaretEquals,
    // comparison
    EqualEqual,
    EqualEqualEqual,
    Bang,
    BangEqual,
    BangEqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    // logical
    AmpAmp,
    PipePipe,
    QuestionQuestion,
    // bitwise
    BitAnd,
    BitOr,
    BitNot,
    Caret,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    ShiftLeftEquals,
    ShiftRightEquals,
    UnsignedShiftRightEquals,
    // increment / decrement
    PlusPlus,
    MinusMinus,
    // special syntax
    Arrow,
    DotDotDot,
    QuestionDot,
    // punctuation
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
    prev_kind: Option<TokenKind>,
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
            prev_kind: None,
        }
    }

    /// Returns true when the previous token means `/` starts a regex literal
    /// rather than a division operator.
    fn slash_is_regex(&self) -> bool {
        match &self.prev_kind {
            None => true, // beginning of file
            Some(k) => !matches!(
                k,
                TokenKind::Identifier(_)
                    | TokenKind::Number(_)
                    | TokenKind::String(_)
                    | TokenKind::Regex(_)
                    | TokenKind::This
                    | TokenKind::Null
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::RightParen
                    | TokenKind::RightBracket
                    | TokenKind::PlusPlus
                    | TokenKind::MinusMinus
            ),
        }
    }

    fn lex_regex(&mut self, span: Span) -> Token {
        // Current position is just after the opening `/`.
        let mut body = String::new();
        let mut in_class = false;
        loop {
            match self.current() {
                None | Some('\n') | Some('\r') => break, // unterminated — stop
                Some('\\') => {
                    body.push('\\');
                    self.bump();
                    if let Some(c) = self.current() {
                        body.push(c);
                        self.bump();
                    }
                }
                Some('[') => {
                    in_class = true;
                    body.push('[');
                    self.bump();
                }
                Some(']') => {
                    in_class = false;
                    body.push(']');
                    self.bump();
                }
                Some('/') if !in_class => {
                    self.bump();
                    break;
                } // closing /
                Some(c) => {
                    body.push(c);
                    self.bump();
                }
            }
        }
        // Consume flags: g i m s u y
        let mut flags = String::new();
        while matches!(
            self.current(),
            Some('g' | 'i' | 'm' | 's' | 'u' | 'y' | 'd' | 'v')
        ) {
            flags.push(self.current().unwrap());
            self.bump();
        }
        let src = format!("/{body}/{flags}");
        Token {
            kind: TokenKind::Regex(src),
            span: self.finish_span(span),
        }
    }

    fn current(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position + 1).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.position + 2).copied()
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

            // Skip decorator lines: @annotation before function declarations.
            if self.current() == Some('@') {
                while !matches!(self.current(), None | Some('\n')) {
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
            "do" => TokenKind::Do,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            "new" => TokenKind::New,
            "this" => TokenKind::This,
            "throw" => TokenKind::Throw,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "class" => TokenKind::Class,
            "extends" => TokenKind::Extends,
            "super" => TokenKind::Super,
            "static" => TokenKind::Static,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "typeof" => TokenKind::Typeof,
            "void" => TokenKind::Void,
            "delete" => TokenKind::Delete,
            "in" => TokenKind::In,
            "instanceof" => TokenKind::Instanceof,
            "of" => TokenKind::Of,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        Token {
            kind,
            span: self.finish_span(span),
        }
    }

    fn number(&mut self, span: Span) -> Token {
        // Hex literal: 0x or 0X
        if self.current() == Some('0') && matches!(self.peek(), Some('x') | Some('X')) {
            self.bump(); // 0
            self.bump(); // x/X
            while matches!(self.current(), Some(ch) if ch.is_ascii_hexdigit()) {
                self.bump();
            }
            return Token {
                kind: TokenKind::Number(self.source[span.start..self.byte_position].to_owned()),
                span: self.finish_span(span),
            };
        }
        // Binary literal: 0b or 0B
        if self.current() == Some('0') && matches!(self.peek(), Some('b') | Some('B')) {
            self.bump();
            self.bump();
            while matches!(self.current(), Some('0') | Some('1')) {
                self.bump();
            }
            return Token {
                kind: TokenKind::Number(self.source[span.start..self.byte_position].to_owned()),
                span: self.finish_span(span),
            };
        }
        // Decimal (possibly with fractional part and exponent)
        while matches!(self.current(), Some(ch) if ch.is_ascii_digit() || ch == '_') {
            self.bump();
        }
        // Consume `.` as part of the number if followed by a digit OR another `.`
        // The second case handles `1..toString()` — `1.` is the float, `.toString` is member access.
        if self.current() == Some('.') && matches!(self.peek(), Some(ch) if ch.is_ascii_digit() || ch == '.') {
            self.bump();
            while matches!(self.current(), Some(ch) if ch.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.current(), Some('e') | Some('E')) {
            self.bump();
            if matches!(self.current(), Some('+') | Some('-')) {
                self.bump();
            }
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
        self.bump(); // consume opening quote
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
                    Some('0') => '\0',
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

    fn template_literal(&mut self, span: Span) -> Token {
        self.bump(); // consume opening `
        let mut parts: Vec<TemplatePart> = Vec::new();
        let mut current_str = String::new();
        loop {
            match self.current() {
                None => break,
                Some('`') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    self.bump();
                    let ch = match self.current() {
                        Some('n') => '\n',
                        Some('r') => '\r',
                        Some('t') => '\t',
                        Some('`') => '`',
                        Some('\\') => '\\',
                        Some('$') => '$',
                        Some('0') => '\0',
                        Some(other) => other,
                        None => break,
                    };
                    current_str.push(ch);
                    self.bump();
                }
                Some('$') if self.peek() == Some('{') => {
                    parts.push(TemplatePart::Str(std::mem::take(&mut current_str)));
                    self.bump(); // $
                    self.bump(); // {
                    let mut depth = 1usize;
                    let mut expr_src = String::new();
                    while let Some(ch) = self.current() {
                        match ch {
                            '{' => {
                                depth += 1;
                                expr_src.push(ch);
                                self.bump();
                            }
                            '}' if depth == 1 => {
                                self.bump();
                                break;
                            }
                            '}' => {
                                depth -= 1;
                                expr_src.push(ch);
                                self.bump();
                            }
                            _ => {
                                expr_src.push(ch);
                                self.bump();
                            }
                        }
                    }
                    parts.push(TemplatePart::Expr(expr_src));
                }
                Some(ch) => {
                    current_str.push(ch);
                    self.bump();
                }
            }
        }
        // Final string segment (may be empty for `${x}` with nothing after)
        parts.push(TemplatePart::Str(current_str));
        Token {
            kind: TokenKind::TemplateLiteral(parts),
            span: self.finish_span(span),
        }
    }

    fn private_identifier(&mut self, span: Span) -> Token {
        self.bump(); // consume #
        while matches!(self.current(), Some(ch) if is_identifier_part(ch)) {
            self.bump();
        }
        let text = &self.source[span.start + 1..self.byte_position]; // skip the #
        Token {
            kind: TokenKind::PrivateIdentifier(text.to_owned()),
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
            Some('`') => self.template_literal(span),
            Some('#') => self.private_identifier(span),
            Some('+') => {
                self.bump();
                match self.current() {
                    Some('+') => {
                        self.bump();
                        Token {
                            kind: TokenKind::PlusPlus,
                            span: self.finish_span(span),
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::PlusEquals,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::Plus,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('-') => {
                self.bump();
                match self.current() {
                    Some('-') => {
                        self.bump();
                        Token {
                            kind: TokenKind::MinusMinus,
                            span: self.finish_span(span),
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::MinusEquals,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::Minus,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('*') => {
                self.bump();
                match self.current() {
                    Some('*') => {
                        self.bump();
                        if self.current() == Some('=') {
                            self.bump();
                            Token {
                                kind: TokenKind::StarStarEquals,
                                span: self.finish_span(span),
                            }
                        } else {
                            Token {
                                kind: TokenKind::StarStar,
                                span: self.finish_span(span),
                            }
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::StarEquals,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::Star,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('/') => {
                self.bump();
                // Check regex context FIRST: `/=` after `=` is a regex `=/g`, not `/=` operator.
                if self.slash_is_regex() {
                    self.lex_regex(span)
                } else if self.current() == Some('=') {
                    self.bump();
                    Token {
                        kind: TokenKind::SlashEquals,
                        span: self.finish_span(span),
                    }
                } else {
                    Token {
                        kind: TokenKind::Slash,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('%') => {
                self.bump();
                if self.current() == Some('=') {
                    self.bump();
                    Token {
                        kind: TokenKind::PercentEquals,
                        span: self.finish_span(span),
                    }
                } else {
                    Token {
                        kind: TokenKind::Percent,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('(') => self.simple(span, TokenKind::LeftParen),
            Some(')') => self.simple(span, TokenKind::RightParen),
            Some('{') => self.simple(span, TokenKind::LeftBrace),
            Some('}') => self.simple(span, TokenKind::RightBrace),
            Some('[') => self.simple(span, TokenKind::LeftBracket),
            Some(']') => self.simple(span, TokenKind::RightBracket),
            Some(';') => self.simple(span, TokenKind::Semicolon),
            Some(',') => self.simple(span, TokenKind::Comma),
            Some(':') => self.simple(span, TokenKind::Colon),
            Some('~') => self.simple(span, TokenKind::BitNot),
            Some('^') => {
                self.bump();
                if self.current() == Some('=') {
                    self.bump();
                    Token {
                        kind: TokenKind::CaretEquals,
                        span: self.finish_span(span),
                    }
                } else {
                    Token {
                        kind: TokenKind::Caret,
                        span: self.finish_span(span),
                    }
                }
            }
            Some('.') => {
                if self.peek() == Some('.') && self.peek2() == Some('.') {
                    self.bump();
                    self.bump();
                    self.bump();
                    Token {
                        kind: TokenKind::DotDotDot,
                        span: self.finish_span(span),
                    }
                } else if matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    // Decimal number with no leading digit: .123
                    self.bump(); // consume '.'
                    let _num_span = self.start_span();
                    // Rewind to include the dot in the number string
                    let start_pos = span.start;
                    while matches!(self.current(), Some(d) if d.is_ascii_digit()) {
                        self.bump();
                    }
                    if self.current() == Some('e') || self.current() == Some('E') {
                        self.bump();
                        if matches!(self.current(), Some('+') | Some('-')) {
                            self.bump();
                        }
                        while matches!(self.current(), Some(d) if d.is_ascii_digit()) {
                            self.bump();
                        }
                    }
                    let text = self.source[start_pos..self.byte_position].to_owned();
                    Token {
                        kind: TokenKind::Number(text),
                        span: self.finish_span(span),
                    }
                } else {
                    self.simple(span, TokenKind::Dot)
                }
            }
            Some('?') => {
                self.bump();
                match self.current() {
                    Some('?') => {
                        self.bump();
                        Token {
                            kind: TokenKind::QuestionQuestion,
                            span: self.finish_span(span),
                        }
                    }
                    Some('.') => {
                        // `?.digit` is ternary `?` + float `.5`, not optional chaining.
                        if matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                            Token {
                                kind: TokenKind::Question,
                                span: self.finish_span(span),
                            }
                        } else {
                            self.bump();
                            Token {
                                kind: TokenKind::QuestionDot,
                                span: self.finish_span(span),
                            }
                        }
                    }
                    _ => Token {
                        kind: TokenKind::Question,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('=') => {
                self.bump();
                match self.current() {
                    Some('>') => {
                        self.bump();
                        Token {
                            kind: TokenKind::Arrow,
                            span: self.finish_span(span),
                        }
                    }
                    Some('=') => {
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
                    }
                    _ => Token {
                        kind: TokenKind::Equals,
                        span: self.finish_span(span),
                    },
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
                match self.current() {
                    Some('<') => {
                        self.bump();
                        if self.current() == Some('=') {
                            self.bump();
                            Token {
                                kind: TokenKind::ShiftLeftEquals,
                                span: self.finish_span(span),
                            }
                        } else {
                            Token {
                                kind: TokenKind::ShiftLeft,
                                span: self.finish_span(span),
                            }
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::LessEqual,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::Less,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('>') => {
                self.bump();
                match self.current() {
                    Some('>') => {
                        self.bump();
                        if self.current() == Some('>') {
                            self.bump();
                            if self.current() == Some('=') {
                                self.bump();
                                Token {
                                    kind: TokenKind::UnsignedShiftRightEquals,
                                    span: self.finish_span(span),
                                }
                            } else {
                                Token {
                                    kind: TokenKind::UnsignedShiftRight,
                                    span: self.finish_span(span),
                                }
                            }
                        } else if self.current() == Some('=') {
                            self.bump();
                            Token {
                                kind: TokenKind::ShiftRightEquals,
                                span: self.finish_span(span),
                            }
                        } else {
                            Token {
                                kind: TokenKind::ShiftRight,
                                span: self.finish_span(span),
                            }
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::GreaterEqual,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::Greater,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('&') => {
                self.bump();
                match self.current() {
                    Some('&') => {
                        self.bump();
                        Token {
                            kind: TokenKind::AmpAmp,
                            span: self.finish_span(span),
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::AmpEquals,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::BitAnd,
                        span: self.finish_span(span),
                    },
                }
            }
            Some('|') => {
                self.bump();
                match self.current() {
                    Some('|') => {
                        self.bump();
                        Token {
                            kind: TokenKind::PipePipe,
                            span: self.finish_span(span),
                        }
                    }
                    Some('=') => {
                        self.bump();
                        Token {
                            kind: TokenKind::PipeEquals,
                            span: self.finish_span(span),
                        }
                    }
                    _ => Token {
                        kind: TokenKind::BitOr,
                        span: self.finish_span(span),
                    },
                }
            }
            // Silently skip unknown characters rather than emitting Eof.
            Some(_) => {
                self.bump();
                return self.next();
            }
        };
        self.prev_kind = Some(token.kind.clone());
        Some(token)
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}
