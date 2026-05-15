use crate::lexer::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    VariableDeclaration(VariableDeclaration),
    FunctionDeclaration(FunctionDeclaration),
    Return(ReturnStatement),
    If(IfStatement),
    While(WhileStatement),
    For(ForStatement),
    Block(BlockStatement),
    Expression(Expression),
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableDeclaration {
    pub kind: VarKind,
    pub declarations: Vec<VariableDeclarator>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Let,
    Const,
    Var,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableDeclarator {
    pub id: String,
    pub init: Option<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReturnStatement {
    pub argument: Option<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfStatement {
    pub test: Expression,
    pub consequent: Box<Statement>,
    pub alternate: Option<Box<Statement>>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WhileStatement {
    pub test: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForStatement {
    pub init: Option<Box<Statement>>,
    pub test: Option<Expression>,
    pub update: Option<Expression>,
    pub body: Box<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockStatement {
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Identifier(String),
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    This,
    Array(Vec<Expression>),
    Object(Vec<ObjectProperty>),
    Function(FunctionExpression),
    Binary {
        op: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Unary {
        op: UnaryOperator,
        expr: Box<Expression>,
    },
    Assignment {
        target: Box<Expression>,
        value: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Member {
        object: Box<Expression>,
        property: MemberProperty,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionExpression {
    pub params: Vec<String>,
    pub body: BlockStatement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectProperty {
    pub key: String,
    pub value: Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MemberProperty {
    Named(String),
    Computed(Box<Expression>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    StrictEqual,
    NotEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negate,
    Plus,
}
