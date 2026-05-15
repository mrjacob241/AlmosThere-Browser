use crate::{
    Program,
    ast::{
        BinaryOperator, BlockStatement, Expression, ForStatement, FunctionDeclaration,
        FunctionExpression, IfStatement, MemberProperty, ObjectProperty, ReturnStatement,
        Statement, UnaryOperator, VarKind, VariableDeclaration, VariableDeclarator, WhileStatement,
    },
    error::JsError,
    lexer::{Token, TokenKind, lex},
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
        while !self.at(TokenKind::Eof) {
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
                .parse_function_declaration()
                .map(Statement::FunctionDeclaration),
            TokenKind::Return => self.parse_return_statement().map(Statement::Return),
            TokenKind::If => self.parse_if_statement().map(Statement::If),
            TokenKind::While => self.parse_while_statement().map(Statement::While),
            TokenKind::For => self.parse_for_statement().map(Statement::For),
            TokenKind::LeftBrace => self.parse_block().map(Statement::Block),
            TokenKind::Semicolon => {
                self.advance();
                Ok(Statement::Empty)
            }
            _ => {
                let expr = self.parse_expression(0)?;
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
            let id = self.expect_identifier()?;
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

    fn parse_function_declaration(&mut self) -> Result<FunctionDeclaration, JsError> {
        let span = self.expect(TokenKind::Function)?.span;
        let name = self.expect_identifier()?;
        let params = self.parse_parameter_list()?;
        let body = self.parse_block()?;
        Ok(FunctionDeclaration {
            name,
            params,
            body,
            span,
        })
    }

    fn parse_return_statement(&mut self) -> Result<ReturnStatement, JsError> {
        let span = self.expect(TokenKind::Return)?.span;
        let argument = if self.at(TokenKind::Semicolon) || self.at(TokenKind::RightBrace) {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        self.consume_semicolon();
        Ok(ReturnStatement { argument, span })
    }

    fn parse_if_statement(&mut self) -> Result<IfStatement, JsError> {
        let span = self.expect(TokenKind::If)?.span;
        self.expect(TokenKind::LeftParen)?;
        let test = self.parse_expression(0)?;
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
        let test = self.parse_expression(0)?;
        self.expect(TokenKind::RightParen)?;
        let body = Box::new(self.parse_statement()?);
        Ok(WhileStatement { test, body, span })
    }

    fn parse_for_statement(&mut self) -> Result<ForStatement, JsError> {
        let span = self.expect(TokenKind::For)?.span;
        self.expect(TokenKind::LeftParen)?;
        let init = if self.eat(TokenKind::Semicolon) {
            None
        } else if matches!(
            self.current_kind(),
            TokenKind::Let | TokenKind::Const | TokenKind::Var
        ) {
            let statement = self.parse_variable_statement(false)?;
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(statement))
        } else {
            let expr = self.parse_expression(0)?;
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(Statement::Expression(expr)))
        };

        let test = if self.eat(TokenKind::Semicolon) {
            None
        } else {
            let test = self.parse_expression(0)?;
            self.expect(TokenKind::Semicolon)?;
            Some(test)
        };

        let update = if self.at(TokenKind::RightParen) {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        self.expect(TokenKind::RightParen)?;
        let body = Box::new(self.parse_statement()?);
        Ok(ForStatement {
            init,
            test,
            update,
            body,
            span,
        })
    }

    fn parse_block(&mut self) -> Result<BlockStatement, JsError> {
        let span = self.expect(TokenKind::LeftBrace)?.span;
        let mut body = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            body.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(BlockStatement { body, span })
    }

    fn parse_expression(&mut self, min_bp: u8) -> Result<Expression, JsError> {
        let mut left = self.parse_prefix()?;

        loop {
            if self.eat(TokenKind::LeftParen) {
                let mut arguments = Vec::new();
                if !self.at(TokenKind::RightParen) {
                    loop {
                        arguments.push(self.parse_expression(0)?);
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

            if self.eat(TokenKind::Dot) {
                let property = self.expect_identifier()?;
                left = Expression::Member {
                    object: Box::new(left),
                    property: MemberProperty::Named(property),
                };
                continue;
            }

            if self.eat(TokenKind::LeftBracket) {
                let property = self.parse_expression(0)?;
                self.expect(TokenKind::RightBracket)?;
                left = Expression::Member {
                    object: Box::new(left),
                    property: MemberProperty::Computed(Box::new(property)),
                };
                continue;
            }

            if self.at(TokenKind::Equals) {
                let (left_bp, right_bp) = (1, 0);
                if left_bp < min_bp {
                    break;
                }
                self.advance();
                let value = self.parse_expression(right_bp)?;
                left = Expression::Assignment {
                    target: Box::new(left),
                    value: Box::new(value),
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

    fn parse_prefix(&mut self) -> Result<Expression, JsError> {
        match self.current_kind() {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expression::Identifier(name))
            }
            TokenKind::Number(value) => {
                let number = value.parse().unwrap_or(0.0);
                self.advance();
                Ok(Expression::Number(number))
            }
            TokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression::String(value))
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
            TokenKind::Bang | TokenKind::Minus | TokenKind::Plus => {
                let op = match self.current_kind() {
                    TokenKind::Bang => UnaryOperator::Not,
                    TokenKind::Minus => UnaryOperator::Negate,
                    TokenKind::Plus => UnaryOperator::Plus,
                    _ => unreachable!(),
                };
                self.advance();
                let expr = self.parse_expression(12)?;
                Ok(Expression::Unary {
                    op,
                    expr: Box::new(expr),
                })
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expression(0)?;
                self.expect(TokenKind::RightParen)?;
                Ok(expr)
            }
            TokenKind::LeftBracket => self.parse_array_literal(),
            TokenKind::LeftBrace => self.parse_object_literal(),
            TokenKind::Function => self.parse_function_expression(),
            _ => self.error("expected expression"),
        }
    }

    fn parse_array_literal(&mut self) -> Result<Expression, JsError> {
        self.expect(TokenKind::LeftBracket)?;
        let mut items = Vec::new();
        if !self.at(TokenKind::RightBracket) {
            loop {
                items.push(self.parse_expression(0)?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightBracket)?;
        Ok(Expression::Array(items))
    }

    fn parse_object_literal(&mut self) -> Result<Expression, JsError> {
        self.expect(TokenKind::LeftBrace)?;
        let mut properties = Vec::new();
        if !self.at(TokenKind::RightBrace) {
            loop {
                let key = match self.current_kind() {
                    TokenKind::Identifier(name) => {
                        let key = name.clone();
                        self.advance();
                        key
                    }
                    TokenKind::String(value) => {
                        let key = value.clone();
                        self.advance();
                        key
                    }
                    _ => return self.error("expected object property name"),
                };
                self.expect(TokenKind::Colon)?;
                let value = self.parse_expression(0)?;
                properties.push(ObjectProperty { key, value });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(Expression::Object(properties))
    }

    fn parse_function_expression(&mut self) -> Result<Expression, JsError> {
        self.expect(TokenKind::Function)?;
        if matches!(self.current_kind(), TokenKind::Identifier(_)) {
            self.advance();
        }
        let params = self.parse_parameter_list()?;
        let body = self.parse_block()?;
        Ok(Expression::Function(FunctionExpression { params, body }))
    }

    fn parse_parameter_list(&mut self) -> Result<Vec<String>, JsError> {
        self.expect(TokenKind::LeftParen)?;
        let mut params = Vec::new();
        if !self.at(TokenKind::RightParen) {
            loop {
                params.push(self.expect_identifier()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen)?;
        Ok(params)
    }

    fn binary_operator(&self) -> Option<(BinaryOperator, u8, u8)> {
        let operator = match self.current_kind() {
            TokenKind::PipePipe => (BinaryOperator::LogicalOr, 2, 3),
            TokenKind::AmpAmp => (BinaryOperator::LogicalAnd, 4, 5),
            TokenKind::EqualEqual => (BinaryOperator::Equal, 6, 7),
            TokenKind::EqualEqualEqual => (BinaryOperator::StrictEqual, 6, 7),
            TokenKind::BangEqual => (BinaryOperator::NotEqual, 6, 7),
            TokenKind::BangEqualEqual => (BinaryOperator::StrictNotEqual, 6, 7),
            TokenKind::Less => (BinaryOperator::Less, 8, 9),
            TokenKind::LessEqual => (BinaryOperator::LessEqual, 8, 9),
            TokenKind::Greater => (BinaryOperator::Greater, 8, 9),
            TokenKind::GreaterEqual => (BinaryOperator::GreaterEqual, 8, 9),
            TokenKind::Plus => (BinaryOperator::Add, 10, 11),
            TokenKind::Minus => (BinaryOperator::Subtract, 10, 11),
            TokenKind::Star => (BinaryOperator::Multiply, 12, 13),
            TokenKind::Slash => (BinaryOperator::Divide, 12, 13),
            TokenKind::Percent => (BinaryOperator::Remainder, 12, 13),
            _ => return None,
        };
        Some(operator)
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
                    .filter_map(|script| script.program.as_ref().err())
                    .collect::<Vec<_>>()
            );
        }
    }
}
