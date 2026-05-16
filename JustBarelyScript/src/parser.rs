use crate::{
    Program,
    ast::{
        BinaryOperator, Binding, BlockStatement, ClassDeclaration, ClassMethod, Expression,
        ForInStatement, ForOfStatement, ForStatement, FunctionBody, FunctionDeclaration,
        FunctionExpression, IfStatement, MemberProperty, ObjectBindingProp, ObjectProperty, Param,
        ReturnStatement, Statement, SwitchCase, SwitchStatement, TemplateElement, ThrowStatement,
        TryCatchStatement, UnaryOperator, VarKind, VariableDeclaration, VariableDeclarator,
        WhileStatement,
    },
    error::JsError,
    lexer::{TemplatePart, Token, TokenKind, lex},
};

pub fn parse_script(source: &str) -> Result<Program, JsError> {
    Parser::new(source).parse_program()
}

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        Self {
            tokens: lex(source),
            position: 0,
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, JsError> {
        let mut body = Vec::new();
        while !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        Ok(Program { body })
    }

    fn parse_statement(&mut self) -> Result<Statement, JsError> {
        match self.current_kind() {
            TokenKind::Let | TokenKind::Const | TokenKind::Var => {
                self.parse_variable_statement(true)
            }
            TokenKind::Function => self
                .parse_function_declaration(false)
                .map(Statement::FunctionDeclaration),
            TokenKind::Async => {
                self.advance();
                if self.at(TokenKind::Function) {
                    self.parse_function_declaration(true)
                        .map(Statement::FunctionDeclaration)
                } else {
                    // async arrow as expression statement
                    let expr = self.parse_async_arrow()?;
                    self.consume_semicolon();
                    Ok(Statement::Expression(expr))
                }
            }
            TokenKind::Return => self.parse_return_statement().map(Statement::Return),
            TokenKind::Throw => {
                let span = self.advance().span;
                let mut argument = self.parse_expression(0)?;
                while self.eat(TokenKind::Comma) {
                    argument = self.parse_expression(0)?;
                }
                self.consume_semicolon();
                Ok(Statement::Throw(ThrowStatement { argument, span }))
            }
            TokenKind::If => self.parse_if_statement().map(Statement::If),
            TokenKind::While => self.parse_while_statement().map(Statement::While),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Try => self.parse_try_statement().map(Statement::TryCatch),
            TokenKind::Class => self
                .parse_class_declaration()
                .map(Statement::ClassDeclaration),
            TokenKind::Switch => self.parse_switch_statement().map(Statement::Switch),
            TokenKind::Break => {
                let span = self.advance().span;
                // optional label — skip identifier if present
                if matches!(self.current_kind(), TokenKind::Identifier(_)) {
                    self.advance();
                }
                self.consume_semicolon();
                Ok(Statement::Break(span))
            }
            TokenKind::Continue => {
                let span = self.advance().span;
                // optional label — skip identifier if present
                if matches!(self.current_kind(), TokenKind::Identifier(_)) {
                    self.advance();
                }
                self.consume_semicolon();
                Ok(Statement::Continue(span))
            }
            TokenKind::LeftBrace => self.parse_block().map(Statement::Block),
            TokenKind::Semicolon => {
                self.advance();
                Ok(Statement::Empty)
            }
            // Skip export/import declarations gracefully.
            TokenKind::Export => {
                self.advance();
                // export default expr
                if matches!(self.current_kind(), TokenKind::Identifier(n) if n == "default") {
                    self.advance();
                    let expr = self.parse_expression(0)?;
                    self.consume_semicolon();
                    return Ok(Statement::Expression(expr));
                }
                self.parse_statement()
            }
            TokenKind::Import => {
                // Skip the entire import statement.
                while !matches!(self.current_kind(), TokenKind::Semicolon | TokenKind::Eof) {
                    self.advance();
                }
                self.consume_semicolon();
                Ok(Statement::Empty)
            }
            _ => {
                let mut expr = self.parse_expression(0)?;
                // Comma (sequence) operator at statement level: a=1, b=2;
                while self.eat(TokenKind::Comma) {
                    expr = self.parse_expression(0)?;
                }
                self.consume_semicolon();
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_variable_statement(&mut self, consume_semicolon: bool) -> Result<Statement, JsError> {
        let start = self.current().span;
        let kind = match self.current_kind() {
            TokenKind::Let => VarKind::Let,
            TokenKind::Const => VarKind::Const,
            TokenKind::Var => VarKind::Var,
            _ => return self.error("expected variable declaration"),
        };
        self.advance();

        let mut declarations = Vec::new();
        loop {
            let id_span = self.current().span;
            let id = self.parse_binding()?;
            let init = if self.eat(TokenKind::Equals) {
                Some(self.parse_expression(0)?)
            } else {
                None
            };
            declarations.push(VariableDeclarator {
                id,
                init,
                span: id_span,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        if consume_semicolon {
            self.consume_semicolon();
        }

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations,
            span: start,
        }))
    }

    fn parse_function_declaration(
        &mut self,
        is_async: bool,
    ) -> Result<FunctionDeclaration, JsError> {
        let span = self.expect(TokenKind::Function)?.span;
        let name = self.expect_identifier()?;
        let params = self.parse_parameter_list()?;
        let body = self.parse_block()?;
        Ok(FunctionDeclaration {
            name,
            params,
            body,
            span,
            is_async,
        })
    }

    fn parse_function_expression_inner(&mut self) -> Result<FunctionExpression, JsError> {
        self.expect(TokenKind::Function)?;
        // Optional function name (named function expression)
        if matches!(self.current_kind(), TokenKind::Identifier(_)) {
            self.advance();
        }
        let params = self.parse_parameter_list()?;
        let body = self.parse_block()?;
        Ok(FunctionExpression {
            params,
            body,
            is_async: false,
        })
    }

    fn parse_async_arrow(&mut self) -> Result<Expression, JsError> {
        // `async` has already been consumed by caller.
        if self.at(TokenKind::LeftParen) {
            match self.try_arrow_from_paren(true) {
                Some(arrow) => Ok(arrow),
                None => self.error("expected async arrow function"),
            }
        } else {
            let name = self.expect_identifier()?;
            self.expect(TokenKind::Arrow)?;
            let body = self.parse_arrow_body()?;
            Ok(Expression::ArrowFunction {
                params: vec![Param::simple(name)],
                body: Box::new(body),
                is_async: true,
            })
        }
    }

    fn parse_return_statement(&mut self) -> Result<ReturnStatement, JsError> {
        let span = self.expect(TokenKind::Return)?.span;
        // No return value if next is ; or } or newline-implied ASI.
        let argument =
            if self.at(TokenKind::Semicolon) || self.at(TokenKind::RightBrace) || self.at_eof() {
                None
            } else {
                let mut expr = self.parse_expression(0)?;
                // Comma (sequence) operator in return: return a, b  →  return b
                while self.eat(TokenKind::Comma) {
                    expr = self.parse_expression(0)?;
                }
                Some(expr)
            };
        self.consume_semicolon();
        Ok(ReturnStatement { argument, span })
    }

    fn parse_sequence_expr(&mut self) -> Result<Expression, JsError> {
        let mut expr = self.parse_expression(0)?;
        while self.eat(TokenKind::Comma) {
            expr = self.parse_expression(0)?;
        }
        Ok(expr)
    }

    fn parse_if_statement(&mut self) -> Result<IfStatement, JsError> {
        let span = self.expect(TokenKind::If)?.span;
        self.expect(TokenKind::LeftParen)?;
        let test = self.parse_sequence_expr()?;
        self.expect(TokenKind::RightParen)?;
        let consequent = Box::new(self.parse_statement()?);
        let alternate = if self.eat(TokenKind::Else) {
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        Ok(IfStatement {
            test,
            consequent,
            alternate,
            span,
        })
    }

    fn parse_while_statement(&mut self) -> Result<WhileStatement, JsError> {
        let span = self.expect(TokenKind::While)?.span;
        self.expect(TokenKind::LeftParen)?;
        let test = self.parse_sequence_expr()?;
        self.expect(TokenKind::RightParen)?;
        let body = Box::new(self.parse_statement()?);
        Ok(WhileStatement { test, body, span })
    }

    fn parse_for_statement(&mut self) -> Result<Statement, JsError> {
        let span = self.expect(TokenKind::For)?.span;
        self.expect(TokenKind::LeftParen)?;

        // Detect for-of and for-in: "for (var/let/const x of/in ...)".
        if matches!(
            self.current_kind(),
            TokenKind::Let | TokenKind::Const | TokenKind::Var
        ) {
            let binding_kind = match self.current_kind() {
                TokenKind::Let => VarKind::Let,
                TokenKind::Const => VarKind::Const,
                _ => VarKind::Var,
            };
            let saved = self.position;
            self.advance();
            if let TokenKind::Identifier(name) = self.current_kind() {
                let name = name.clone();
                self.advance();
                if self.eat(TokenKind::Of) {
                    let iterable = self.parse_expression(0)?;
                    self.expect(TokenKind::RightParen)?;
                    let body = Box::new(self.parse_statement()?);
                    return Ok(Statement::ForOf(ForOfStatement {
                        binding: name,
                        binding_kind,
                        iterable,
                        body,
                        span,
                    }));
                } else if self.eat(TokenKind::In) {
                    let object = self.parse_expression(0)?;
                    self.expect(TokenKind::RightParen)?;
                    let body = Box::new(self.parse_statement()?);
                    return Ok(Statement::ForIn(ForInStatement {
                        binding: name,
                        binding_kind,
                        object,
                        body,
                        span,
                    }));
                }
                // Not for-of or for-in — reset and parse as C-style for.
                self.position = saved;
            } else {
                self.position = saved;
            }
        }

        // Detect bare for-in: "for (x in obj)" — no var/let/const.
        if let TokenKind::Identifier(name) = self.current_kind() {
            let saved = self.position;
            let name = name.clone();
            self.advance();
            if self.eat(TokenKind::In) {
                let object = self.parse_expression(0)?;
                self.expect(TokenKind::RightParen)?;
                let body = Box::new(self.parse_statement()?);
                return Ok(Statement::ForIn(ForInStatement {
                    binding: name,
                    binding_kind: VarKind::Var,
                    object,
                    body,
                    span,
                }));
            }
            self.position = saved;
        }

        // C-style for loop: for (init; test; update)
        let init = if self.eat(TokenKind::Semicolon) {
            None
        } else if matches!(
            self.current_kind(),
            TokenKind::Let | TokenKind::Const | TokenKind::Var
        ) {
            let stmt = self.parse_variable_statement(false)?;
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(stmt))
        } else {
            let mut expr = self.parse_expression(0)?;
            while self.eat(TokenKind::Comma) {
                expr = self.parse_expression(0)?;
            }
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(Statement::Expression(expr)))
        };

        let test = if self.at(TokenKind::Semicolon) {
            self.advance();
            None
        } else {
            let t = self.parse_sequence_expr()?;
            self.expect(TokenKind::Semicolon)?;
            Some(t)
        };

        let update = if self.at(TokenKind::RightParen) {
            None
        } else {
            // for (;;  i++, j++) — comma sequence in update
            let mut expr = self.parse_expression(0)?;
            while self.eat(TokenKind::Comma) {
                expr = self.parse_expression(0)?;
            }
            Some(expr)
        };
        self.expect(TokenKind::RightParen)?;
        let body = Box::new(self.parse_statement()?);
        Ok(Statement::For(ForStatement {
            init,
            test,
            update,
            body,
            span,
        }))
    }

    fn parse_switch_statement(&mut self) -> Result<SwitchStatement, JsError> {
        let span = self.expect(TokenKind::Switch)?.span;
        self.expect(TokenKind::LeftParen)?;
        let discriminant = self.parse_expression(0)?;
        self.expect(TokenKind::RightParen)?;
        self.expect(TokenKind::LeftBrace)?;

        let mut cases = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() {
            let test = if self.eat(TokenKind::Case) {
                let expr = self.parse_expression(0)?;
                self.expect(TokenKind::Colon)?;
                Some(expr)
            } else if self.eat(TokenKind::Default) {
                self.expect(TokenKind::Colon)?;
                None
            } else {
                break;
            };

            let mut body = Vec::new();
            while !matches!(
                self.current_kind(),
                TokenKind::Case | TokenKind::Default | TokenKind::RightBrace | TokenKind::Eof
            ) {
                body.push(self.parse_statement()?);
            }
            cases.push(SwitchCase { test, body });
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(SwitchStatement {
            discriminant,
            cases,
            span,
        })
    }

    fn parse_try_statement(&mut self) -> Result<TryCatchStatement, JsError> {
        let span = self.expect(TokenKind::Try)?.span;
        let body = self.parse_block()?;

        let catch = if self.eat(TokenKind::Catch) {
            let catch_param = if self.eat(TokenKind::LeftParen) {
                let name = self.expect_identifier()?;
                self.expect(TokenKind::RightParen)?;
                Some(name)
            } else {
                None
            };
            let catch_body = self.parse_block()?;
            Some((catch_param, catch_body))
        } else {
            None
        };

        let finally_body = if self.eat(TokenKind::Finally) {
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(TryCatchStatement {
            body,
            catch_param: catch.as_ref().and_then(|(p, _)| p.clone()),
            catch_body: catch.map(|(_, b)| b),
            finally_body,
            span,
        })
    }

    fn parse_class_declaration(&mut self) -> Result<ClassDeclaration, JsError> {
        let span = self.expect(TokenKind::Class)?.span;
        let name = self.expect_identifier()?;
        let superclass = if self.eat(TokenKind::Extends) {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect(TokenKind::LeftBrace)?;
        let mut methods = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() {
            // Skip semicolons between class members.
            if self.eat(TokenKind::Semicolon) {
                continue;
            }
            let is_static = self.eat(TokenKind::Static);
            // Parse method name: identifier, keyword-as-identifier, private, or string.
            let method_name = match self.current_kind() {
                TokenKind::PrivateIdentifier(n) => {
                    let n = format!("#{}", n.clone());
                    self.advance();
                    n
                }
                TokenKind::String(s) => {
                    let s = s.clone();
                    self.advance();
                    s
                }
                _ => {
                    // Identifier or keyword used as method name.
                    let n = self.expect_identifier_or_keyword()?;
                    // 'get'/'set' accessor — we flatten these as normal methods.
                    if (n == "get" || n == "set") && !self.at(TokenKind::LeftParen) {
                        // Skip get/set keyword, parse the actual name.
                        let actual = self.expect_identifier_or_keyword()?;
                        let params = self.parse_parameter_list()?;
                        let body = self.parse_block()?;
                        let is_constructor = actual == "constructor";
                        methods.push(ClassMethod {
                            name: actual,
                            is_static,
                            is_constructor,
                            params,
                            body,
                        });
                        continue;
                    }
                    n
                }
            };
            let is_constructor = method_name == "constructor";
            // Method shorthand or computed — parse parameters and body.
            let params = self.parse_parameter_list()?;
            let body = self.parse_block()?;
            methods.push(ClassMethod {
                name: method_name,
                is_static,
                is_constructor,
                params,
                body,
            });
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(ClassDeclaration {
            name,
            superclass,
            methods,
            span,
        })
    }

    fn parse_block(&mut self) -> Result<BlockStatement, JsError> {
        let span = self.expect(TokenKind::LeftBrace)?.span;
        let mut body = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(BlockStatement { body, span })
    }

    fn parse_expression(&mut self, min_bp: u8) -> Result<Expression, JsError> {
        let mut left = self.parse_prefix()?;

        loop {
            // Optional chaining: obj?.prop or obj?.[expr]
            if self.at(TokenKind::QuestionDot) {
                self.advance();
                if self.eat(TokenKind::LeftBracket) {
                    let property = self.parse_expression(0)?;
                    self.expect(TokenKind::RightBracket)?;
                    left = Expression::Member {
                        object: Box::new(left),
                        property: MemberProperty::Computed(Box::new(property)),
                        optional: true,
                    };
                } else if self.eat(TokenKind::LeftParen) {
                    // Optional call: obj?.()
                    let mut arguments = Vec::new();
                    if !self.at(TokenKind::RightParen) {
                        loop {
                            arguments.push(self.parse_argument()?);
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RightParen)?;
                    left = Expression::Call {
                        callee: Box::new(left),
                        arguments,
                    };
                } else {
                    let property = self.expect_identifier_or_keyword()?;
                    left = Expression::Member {
                        object: Box::new(left),
                        property: MemberProperty::Named(property),
                        optional: true,
                    };
                }
                continue;
            }

            // Regular call
            if self.eat(TokenKind::LeftParen) {
                let mut arguments = Vec::new();
                if !self.at(TokenKind::RightParen) {
                    loop {
                        arguments.push(self.parse_argument()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightParen)?;
                left = Expression::Call {
                    callee: Box::new(left),
                    arguments,
                };
                continue;
            }

            // Member access: obj.prop
            if self.eat(TokenKind::Dot) {
                let property = self.expect_identifier_or_keyword()?;
                left = Expression::Member {
                    object: Box::new(left),
                    property: MemberProperty::Named(property),
                    optional: false,
                };
                continue;
            }

            // Computed member: obj[expr]
            if self.eat(TokenKind::LeftBracket) {
                let property = self.parse_expression(0)?;
                self.expect(TokenKind::RightBracket)?;
                left = Expression::Member {
                    object: Box::new(left),
                    property: MemberProperty::Computed(Box::new(property)),
                    optional: false,
                };
                continue;
            }

            // Postfix ++ / --  (desugar to n = n ± 1)
            if self.at(TokenKind::PlusPlus) || self.at(TokenKind::MinusMinus) {
                let op = if self.at(TokenKind::PlusPlus) {
                    BinaryOperator::Add
                } else {
                    BinaryOperator::Subtract
                };
                self.advance();
                let target = left.clone();
                left = Expression::Assignment {
                    target: Box::new(target.clone()),
                    value: Box::new(Expression::Binary {
                        op,
                        left: Box::new(target),
                        right: Box::new(Expression::Number(1.0)),
                    }),
                };
                continue;
            }

            // Single-parameter arrow: ident =>
            if self.at(TokenKind::Arrow) {
                if let Expression::Identifier(name) = &left {
                    let name = name.clone();
                    if 1u8 < min_bp {
                        break;
                    }
                    self.advance(); // =>
                    let body = self.parse_arrow_body()?;
                    left = Expression::ArrowFunction {
                        params: vec![Param::simple(name)],
                        body: Box::new(body),
                        is_async: false,
                    };
                    continue;
                }
                break;
            }

            // Simple assignment: target = value
            if self.at(TokenKind::Equals) {
                if 1u8 < min_bp {
                    break;
                }
                self.advance();
                let value = self.parse_expression(0)?;
                left = Expression::Assignment {
                    target: Box::new(left),
                    value: Box::new(value),
                };
                continue;
            }

            // Compound assignments: +=, -=, *=, /=, %=, **=, &=, |=, ^=
            if let Some(bin_op) = self.compound_assignment_op() {
                if 1u8 < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expression(0)?;
                let target = left.clone();
                left = Expression::Assignment {
                    target: Box::new(target.clone()),
                    value: Box::new(Expression::Binary {
                        op: bin_op,
                        left: Box::new(target),
                        right: Box::new(rhs),
                    }),
                };
                continue;
            }

            // Ternary: test ? consequent : alternate
            if self.at(TokenKind::Question) {
                if 1u8 < min_bp {
                    break;
                }
                self.advance();
                let consequent = self.parse_expression(0)?;
                self.expect(TokenKind::Colon)?;
                let alternate = self.parse_expression(0)?;
                left = Expression::Ternary {
                    test: Box::new(left),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                };
                continue;
            }

            let Some((operator, left_bp, right_bp)) = self.binary_operator() else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.advance();
            let right = self.parse_expression(right_bp)?;
            left = Expression::Binary {
                op: operator,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse one function call argument (may be a spread expression).
    fn parse_argument(&mut self) -> Result<Expression, JsError> {
        if self.eat(TokenKind::DotDotDot) {
            let expr = self.parse_expression(0)?;
            Ok(Expression::Spread(Box::new(expr)))
        } else {
            self.parse_expression(0)
        }
    }

    fn parse_prefix(&mut self) -> Result<Expression, JsError> {
        match self.current_kind() {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expression::Identifier(name))
            }
            TokenKind::Number(value) => {
                let value = value.clone();
                let number = if value.starts_with("0x") || value.starts_with("0X") {
                    i64::from_str_radix(&value[2..], 16).unwrap_or(0) as f64
                } else if value.starts_with("0b") || value.starts_with("0B") {
                    i64::from_str_radix(&value[2..], 2).unwrap_or(0) as f64
                } else {
                    value.replace('_', "").parse().unwrap_or(0.0)
                };
                self.advance();
                Ok(Expression::Number(number))
            }
            TokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression::String(value))
            }
            TokenKind::Regex(_) => {
                // Regex literals are not executed — treat as a null-ish opaque object.
                self.advance();
                Ok(Expression::Null)
            }
            TokenKind::TemplateLiteral(parts) => {
                let parts = parts.clone();
                self.advance();
                self.parse_template_literal(parts)
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression::Boolean(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::Boolean(false))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expression::Null)
            }
            TokenKind::Undefined => {
                self.advance();
                Ok(Expression::Undefined)
            }
            TokenKind::This => {
                self.advance();
                Ok(Expression::This)
            }
            TokenKind::Super => {
                self.advance();
                Ok(Expression::Super)
            }

            // Prefix ++ / -- (desugar to n = n ± 1)
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                let op = if matches!(self.current_kind(), TokenKind::PlusPlus) {
                    BinaryOperator::Add
                } else {
                    BinaryOperator::Subtract
                };
                self.advance();
                let inner = self.parse_expression(13)?;
                let target = inner.clone();
                Ok(Expression::Assignment {
                    target: Box::new(target.clone()),
                    value: Box::new(Expression::Binary {
                        op,
                        left: Box::new(target),
                        right: Box::new(Expression::Number(1.0)),
                    }),
                })
            }

            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Unary {
                    op: UnaryOperator::Not,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Unary {
                    op: UnaryOperator::Negate,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Plus => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Unary {
                    op: UnaryOperator::Plus,
                    expr: Box::new(expr),
                })
            }
            TokenKind::BitNot => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Unary {
                    op: UnaryOperator::BitNot,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Typeof => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Typeof(Box::new(expr)))
            }
            TokenKind::Void => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Void(Box::new(expr)))
            }
            TokenKind::Delete => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Delete(Box::new(expr)))
            }
            TokenKind::Await => {
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Await(Box::new(expr)))
            }

            TokenKind::DotDotDot => {
                self.advance();
                let expr = self.parse_expression(2)?;
                Ok(Expression::Spread(Box::new(expr)))
            }

            TokenKind::LeftParen => {
                // Try to parse as arrow function; fall back to grouped/sequence expression.
                match self.try_arrow_from_paren(false) {
                    Some(arrow) => Ok(arrow),
                    None => {
                        self.advance(); // (
                        let mut expr = self.parse_expression(0)?;
                        // Comma (sequence) operator: (a, b, c) → value of last expr.
                        while self.eat(TokenKind::Comma) {
                            expr = self.parse_expression(0)?;
                        }
                        self.expect(TokenKind::RightParen)?;
                        Ok(expr)
                    }
                }
            }

            TokenKind::LeftBracket => self.parse_array_literal(),
            TokenKind::LeftBrace => self.parse_object_literal(),
            TokenKind::Function => {
                let func = self.parse_function_expression_inner()?;
                Ok(Expression::Function(func))
            }
            TokenKind::Async => {
                self.advance();
                if self.at(TokenKind::Function) {
                    let mut func = self.parse_function_expression_inner()?;
                    func.is_async = true;
                    Ok(Expression::Function(func))
                } else {
                    self.parse_async_arrow()
                }
            }
            TokenKind::New => {
                self.advance();
                let callee = self.parse_new_target()?;
                let arguments = if self.eat(TokenKind::LeftParen) {
                    let mut args = Vec::new();
                    if !self.at(TokenKind::RightParen) {
                        loop {
                            args.push(self.parse_argument()?);
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RightParen)?;
                    args
                } else {
                    Vec::new()
                };
                Ok(Expression::New {
                    callee: Box::new(callee),
                    arguments,
                })
            }
            _ => self.error("expected expression"),
        }
    }

    /// Parse the callee for `new` (identifiers and member access, no calls yet).
    fn parse_new_target(&mut self) -> Result<Expression, JsError> {
        let mut expr = match self.current_kind() {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Expression::Identifier(name)
            }
            _ => return self.error("expected class name after new"),
        };
        while self.eat(TokenKind::Dot) {
            let prop = self.expect_identifier_or_keyword()?;
            expr = Expression::Member {
                object: Box::new(expr),
                property: MemberProperty::Named(prop),
                optional: false,
            };
        }
        Ok(expr)
    }

    /// Try to parse `(params) => body`. Saves and restores position on failure.
    fn try_arrow_from_paren(&mut self, is_async: bool) -> Option<Expression> {
        let saved = self.position;
        self.advance(); // consume (
        let mut params = Vec::new();
        if !self.at(TokenKind::RightParen) {
            loop {
                let rest = self.eat(TokenKind::DotDotDot);
                let binding = match self.parse_binding() {
                    Ok(b) => b,
                    Err(_) => {
                        self.position = saved;
                        return None;
                    }
                };
                let default = if !rest && self.eat(TokenKind::Equals) {
                    match self.parse_expression(2) {
                        Ok(e) => Some(e),
                        Err(_) => {
                            self.position = saved;
                            return None;
                        }
                    }
                } else {
                    None
                };
                params.push(Param {
                    binding,
                    default,
                    rest,
                });
                if rest {
                    break;
                }
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        if !self.eat(TokenKind::RightParen) {
            self.position = saved;
            return None;
        }
        if !self.eat(TokenKind::Arrow) {
            self.position = saved;
            return None;
        }
        match self.parse_arrow_body() {
            Ok(body) => Some(Expression::ArrowFunction {
                params,
                body: Box::new(body),
                is_async,
            }),
            Err(_) => {
                self.position = saved;
                None
            }
        }
    }

    fn parse_arrow_body(&mut self) -> Result<FunctionBody, JsError> {
        if self.at(TokenKind::LeftBrace) {
            self.parse_block().map(FunctionBody::Block)
        } else {
            self.parse_expression(0)
                .map(|e| FunctionBody::Expr(Box::new(e)))
        }
    }

    fn parse_template_literal(&mut self, parts: Vec<TemplatePart>) -> Result<Expression, JsError> {
        let mut elements = Vec::new();
        for part in parts {
            match part {
                TemplatePart::Str(s) => elements.push(TemplateElement::Str(s)),
                TemplatePart::Expr(src) => {
                    let inner = parse_script(&src)?;
                    // Take the last expression statement as the interpolated value.
                    let expr = inner
                        .body
                        .into_iter()
                        .last()
                        .and_then(|s| {
                            if let Statement::Expression(e) = s {
                                Some(e)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(Expression::Undefined);
                    elements.push(TemplateElement::Expr(Box::new(expr)));
                }
            }
        }
        Ok(Expression::TemplateLiteral(elements))
    }

    fn parse_array_literal(&mut self) -> Result<Expression, JsError> {
        self.expect(TokenKind::LeftBracket)?;
        let mut items = Vec::new();
        while !self.at(TokenKind::RightBracket) && !self.at_eof() {
            // Elision: allow trailing comma → undefined element.
            if self.at(TokenKind::Comma) {
                items.push(Expression::Undefined);
                self.advance();
                continue;
            }
            items.push(self.parse_argument()?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightBracket)?;
        Ok(Expression::Array(items))
    }

    fn parse_object_literal(&mut self) -> Result<Expression, JsError> {
        self.expect(TokenKind::LeftBrace)?;
        let mut properties = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() {
            // Spread: { ...expr }
            if self.eat(TokenKind::DotDotDot) {
                let value = self.parse_expression(2)?;
                properties.push(ObjectProperty {
                    key: String::new(),
                    value: Expression::Spread(Box::new(value)),
                    shorthand: false,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                continue;
            }

            // Computed key: [expr]: value
            if self.eat(TokenKind::LeftBracket) {
                let key_expr = self.parse_expression(0)?;
                self.expect(TokenKind::RightBracket)?;
                self.expect(TokenKind::Colon)?;
                let value = self.parse_expression(2)?;
                // Represent computed key as __computed__; executor ignores for now.
                let key = match &key_expr {
                    Expression::String(s) => s.clone(),
                    Expression::Identifier(n) => n.clone(),
                    _ => "__computed__".to_owned(),
                };
                properties.push(ObjectProperty {
                    key,
                    value,
                    shorthand: false,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                continue;
            }

            // Allow string or number literals as object keys.
            let key = match self.current_kind() {
                TokenKind::String(s) => {
                    let s = s.clone();
                    self.advance();
                    s
                }
                TokenKind::Number(n) => {
                    let s = n.clone();
                    self.advance();
                    s
                }
                _ => self.expect_identifier_or_keyword()?,
            };

            // get/set accessor — treat as method shorthand.
            if (key == "get" || key == "set")
                && !self.at(TokenKind::LeftParen)
                && !self.at(TokenKind::Colon)
                && !self.at(TokenKind::Comma)
                && !self.at(TokenKind::RightBrace)
            {
                let actual_key = self.expect_identifier_or_keyword()?;
                let params = self.parse_parameter_list()?;
                let body = self.parse_block()?;
                properties.push(ObjectProperty {
                    key: actual_key,
                    value: Expression::Function(FunctionExpression {
                        params,
                        body,
                        is_async: false,
                    }),
                    shorthand: false,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                continue;
            }

            if self.at(TokenKind::LeftParen) {
                // Method shorthand: { method(params) { body } }
                let params = self.parse_parameter_list()?;
                let body = self.parse_block()?;
                properties.push(ObjectProperty {
                    key: key.clone(),
                    value: Expression::Function(FunctionExpression {
                        params,
                        body,
                        is_async: false,
                    }),
                    shorthand: false,
                });
            } else if self.eat(TokenKind::Colon) {
                let value = self.parse_expression(2)?;
                properties.push(ObjectProperty {
                    key,
                    value,
                    shorthand: false,
                });
            } else {
                // Shorthand property: { key } → { key: key }
                properties.push(ObjectProperty {
                    value: Expression::Identifier(key.clone()),
                    key,
                    shorthand: true,
                });
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(Expression::Object(properties))
    }

    fn parse_parameter_list(&mut self) -> Result<Vec<Param>, JsError> {
        self.expect(TokenKind::LeftParen)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at_eof() {
            let rest = self.eat(TokenKind::DotDotDot);
            let binding = self.parse_binding()?;
            let default = if !rest && self.eat(TokenKind::Equals) {
                Some(self.parse_expression(2)?)
            } else {
                None
            };
            params.push(Param {
                binding,
                default,
                rest,
            });
            if rest {
                break;
            } // rest must be last
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightParen)?;
        Ok(params)
    }

    fn compound_assignment_op(&self) -> Option<BinaryOperator> {
        match self.current_kind() {
            TokenKind::PlusEquals => Some(BinaryOperator::Add),
            TokenKind::MinusEquals => Some(BinaryOperator::Subtract),
            TokenKind::StarEquals => Some(BinaryOperator::Multiply),
            TokenKind::SlashEquals => Some(BinaryOperator::Divide),
            TokenKind::PercentEquals => Some(BinaryOperator::Remainder),
            TokenKind::StarStarEquals => Some(BinaryOperator::Exponent),
            TokenKind::AmpEquals => Some(BinaryOperator::BitAnd),
            TokenKind::PipeEquals => Some(BinaryOperator::BitOr),
            TokenKind::CaretEquals => Some(BinaryOperator::BitXor),
            _ => None,
        }
    }

    fn binary_operator(&self) -> Option<(BinaryOperator, u8, u8)> {
        let op = match self.current_kind() {
            TokenKind::QuestionQuestion => (BinaryOperator::NullishCoalescing, 2, 3),
            TokenKind::PipePipe => (BinaryOperator::LogicalOr, 2, 3),
            TokenKind::BitOr => (BinaryOperator::BitOr, 3, 4),
            TokenKind::AmpAmp => (BinaryOperator::LogicalAnd, 4, 5),
            TokenKind::Caret => (BinaryOperator::BitXor, 5, 6),
            TokenKind::BitAnd => (BinaryOperator::BitAnd, 6, 7),
            TokenKind::EqualEqual => (BinaryOperator::Equal, 7, 8),
            TokenKind::EqualEqualEqual => (BinaryOperator::StrictEqual, 7, 8),
            TokenKind::BangEqual => (BinaryOperator::NotEqual, 7, 8),
            TokenKind::BangEqualEqual => (BinaryOperator::StrictNotEqual, 7, 8),
            TokenKind::Less => (BinaryOperator::Less, 8, 9),
            TokenKind::LessEqual => (BinaryOperator::LessEqual, 8, 9),
            TokenKind::Greater => (BinaryOperator::Greater, 8, 9),
            TokenKind::GreaterEqual => (BinaryOperator::GreaterEqual, 8, 9),
            TokenKind::Instanceof => (BinaryOperator::Instanceof, 8, 9),
            TokenKind::In => (BinaryOperator::In, 8, 9),
            TokenKind::ShiftLeft => (BinaryOperator::ShiftLeft, 9, 10),
            TokenKind::ShiftRight => (BinaryOperator::ShiftRight, 9, 10),
            TokenKind::Plus => (BinaryOperator::Add, 10, 11),
            TokenKind::Minus => (BinaryOperator::Subtract, 10, 11),
            TokenKind::Star => (BinaryOperator::Multiply, 12, 13),
            TokenKind::Slash => (BinaryOperator::Divide, 12, 13),
            TokenKind::Percent => (BinaryOperator::Remainder, 12, 13),
            TokenKind::StarStar => (BinaryOperator::Exponent, 14, 13), // right-associative
            _ => return None,
        };
        Some(op)
    }

    fn parse_binding(&mut self) -> Result<Binding, JsError> {
        match self.current_kind() {
            TokenKind::LeftBrace => {
                self.advance();
                let mut props = Vec::new();
                while !matches!(self.current_kind(), TokenKind::RightBrace | TokenKind::Eof) {
                    let key = self.expect_identifier_or_keyword()?;
                    let (binding, default) = if self.eat(TokenKind::Colon) {
                        let b = self.parse_binding()?;
                        let d = if self.eat(TokenKind::Equals) {
                            Some(self.parse_expression(0)?)
                        } else {
                            None
                        };
                        (b, d)
                    } else {
                        let d = if self.eat(TokenKind::Equals) {
                            Some(self.parse_expression(0)?)
                        } else {
                            None
                        };
                        (Binding::Name(key.clone()), d)
                    };
                    props.push(ObjectBindingProp {
                        key,
                        binding,
                        default,
                    });
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RightBrace)?;
                Ok(Binding::Object(props))
            }
            TokenKind::LeftBracket => {
                self.advance();
                let mut items = Vec::new();
                while !matches!(
                    self.current_kind(),
                    TokenKind::RightBracket | TokenKind::Eof
                ) {
                    if matches!(self.current_kind(), TokenKind::Comma) {
                        items.push(None);
                        self.advance();
                    } else {
                        let b = self.parse_binding()?;
                        items.push(Some(b));
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightBracket)?;
                Ok(Binding::Array(items))
            }
            _ => Ok(Binding::Name(self.expect_identifier()?)),
        }
    }

    fn expect_identifier(&mut self) -> Result<String, JsError> {
        match self.current_kind() {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => self.error("expected identifier"),
        }
    }

    /// Accept identifiers AND keywords that commonly appear as property names.
    fn expect_identifier_or_keyword(&mut self) -> Result<String, JsError> {
        match self.current_kind() {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            kind => {
                if let Some(text) = keyword_as_identifier(kind) {
                    self.advance();
                    Ok(text.to_owned())
                } else {
                    self.error("expected identifier or keyword")
                }
            }
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, JsError> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            self.error("unexpected token")
        }
    }

    fn consume_semicolon(&mut self) {
        self.eat(TokenKind::Semicolon);
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at(&self, kind: TokenKind) -> bool {
        std::mem::discriminant(self.current_kind()) == std::mem::discriminant(&kind)
    }

    fn at_eof(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn current_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if self.position + 1 < self.tokens.len() {
            self.position += 1;
        }
        token
    }

    fn error<T>(&self, message: &str) -> Result<T, JsError> {
        Err(JsError::parse(message, self.current().span))
    }
}

/// Map keyword tokens to their text for use as property / method names.
fn keyword_as_identifier(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::Static => Some("static"),
        TokenKind::Async => Some("async"),
        TokenKind::Of => Some("of"),
        TokenKind::Let => Some("let"),
        TokenKind::Const => Some("const"),
        TokenKind::Var => Some("var"),
        TokenKind::Return => Some("return"),
        TokenKind::New => Some("new"),
        TokenKind::Delete => Some("delete"),
        TokenKind::Typeof => Some("typeof"),
        TokenKind::Void => Some("void"),
        TokenKind::In => Some("in"),
        TokenKind::Instanceof => Some("instanceof"),
        TokenKind::If => Some("if"),
        TokenKind::Else => Some("else"),
        TokenKind::While => Some("while"),
        TokenKind::For => Some("for"),
        TokenKind::Break => Some("break"),
        TokenKind::Continue => Some("continue"),
        TokenKind::Class => Some("class"),
        TokenKind::Extends => Some("extends"),
        TokenKind::Super => Some("super"),
        TokenKind::Import => Some("import"),
        TokenKind::Export => Some("export"),
        TokenKind::This => Some("this"),
        TokenKind::Null => Some("null"),
        TokenKind::True => Some("true"),
        TokenKind::False => Some("false"),
        TokenKind::Await => Some("await"),
        TokenKind::Throw => Some("throw"),
        TokenKind::Try => Some("try"),
        TokenKind::Catch => Some("catch"),
        TokenKind::Finally => Some("finally"),
        TokenKind::Function => Some("function"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::parse_inline_scripts_from_html;

    #[test]
    fn parses_basic_precedence() {
        let program = parse_script("let x = 1 + 2 * 3;").unwrap();
        let Statement::VariableDeclaration(decl) = &program.body[0] else {
            panic!("expected declaration");
        };
        let Expression::Binary { op, right, .. } = decl.declarations[0].init.as_ref().unwrap()
        else {
            panic!("expected binary expression");
        };
        assert_eq!(*op, BinaryOperator::Add);
        assert!(matches!(
            right.as_ref(),
            Expression::Binary {
                op: BinaryOperator::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn parses_browser_batch_a_shapes() {
        let html = include_str!("../UnitTest/004-element-creation/index.html");
        let report = parse_inline_scripts_from_html(html);
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.scripts.len(), 1);
    }

    #[test]
    fn parses_for_loop_and_member_calls() {
        let html = include_str!("../UnitTest/011-for-loop-dom-update/index.html");
        let report = parse_inline_scripts_from_html(html);
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn parses_current_browser_smoke_test_scripts() {
        let pages = [
            include_str!("../UnitTest/001-basic-script-execution/index.html"),
            include_str!("../UnitTest/002-multiple-script-tags-execute-in-order/index.html"),
            include_str!("../UnitTest/003-console-logging/index.html"),
            include_str!("../UnitTest/004-element-creation/index.html"),
            include_str!("../UnitTest/005-css-class-assignment/index.html"),
            include_str!("../UnitTest/006-setattribute-and-getattribute/index.html"),
            include_str!("../UnitTest/007-innerhtml-basic-replacement/index.html"),
            include_str!("../UnitTest/008-query-selector-by-id/index.html"),
            include_str!("../UnitTest/009-query-selector-by-class/index.html"),
            include_str!("../UnitTest/010-queryselectorall-and-length/index.html"),
            include_str!("../UnitTest/011-for-loop-dom-update/index.html"),
            include_str!("../UnitTest/012-event-listener-click/index.html"),
            include_str!("../UnitTest/013-event-object-target/index.html"),
            include_str!("../UnitTest/014-input-value-reading/index.html"),
            include_str!("../UnitTest/015-input-event/index.html"),
            include_str!("../UnitTest/016-style-property-mutation/index.html"),
            include_str!("../UnitTest/017-computed-style-smoke-test/index.html"),
            include_str!("../UnitTest/018-settimeout/index.html"),
            include_str!("../UnitTest/019-promise-microtask/index.html"),
            include_str!("../UnitTest/020-json-parse-and-stringify/index.html"),
            include_str!("../UnitTest/021-array-operations/index.html"),
            include_str!("../UnitTest/022-object-literals-and-properties/index.html"),
            include_str!("../UnitTest/023-closures-in-event-handlers/index.html"),
            include_str!("../UnitTest/024-domcontentloaded/index.html"),
            include_str!("../UnitTest/025-minimal-todo-app/index.html"),
            include_str!("../UnitTest/026-decorator-skip/index.html"),
            include_str!("../UnitTest/027-xor-operator/index.html"),
            include_str!("../UnitTest/028-increment-decrement/index.html"),
            include_str!("../UnitTest/029-compound-assignment/index.html"),
            include_str!("../UnitTest/030-nullish-coalescing/index.html"),
        ];

        for (index, page) in pages.iter().enumerate() {
            let report = parse_inline_scripts_from_html(page);
            assert_eq!(
                report.error_count(),
                0,
                "script smoke test {} had a parser error: {:?}",
                index + 1,
                report
                    .scripts
                    .iter()
                    .filter_map(|s| s.program.as_ref().err())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn parses_extended_feature_scripts() {
        let pages = [
            include_str!("../UnitTest/031-default-parameters/index.html"),
            include_str!("../UnitTest/032-arrow-functions/index.html"),
            include_str!("../UnitTest/033-spread-operator/index.html"),
            include_str!("../UnitTest/034-optional-chaining/index.html"),
            include_str!("../UnitTest/035-template-literals/index.html"),
            include_str!("../UnitTest/036-try-catch-finally/index.html"),
            include_str!("../UnitTest/037-for-of/index.html"),
        ];
        for (index, page) in pages.iter().enumerate() {
            let report = parse_inline_scripts_from_html(page);
            assert_eq!(
                report.error_count(),
                0,
                "extended test {} (031+{}) had a parser error: {:?}",
                index + 31,
                index,
                report
                    .scripts
                    .iter()
                    .filter_map(|s| s.program.as_ref().err())
                    .collect::<Vec<_>>()
            );
        }
    }
}
