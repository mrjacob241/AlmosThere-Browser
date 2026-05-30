use crate::lexer::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    ImportDeclaration(ImportDeclaration),
    ExportDeclaration(ExportDeclaration),
    VariableDeclaration(VariableDeclaration),
    FunctionDeclaration(FunctionDeclaration),
    ClassDeclaration(ClassDeclaration),
    Return(ReturnStatement),
    Throw(ThrowStatement),
    If(IfStatement),
    While(WhileStatement),
    DoWhile(DoWhileStatement),
    For(ForStatement),
    ForOf(ForOfStatement),
    ForIn(ForInStatement),
    TryCatch(TryCatchStatement),
    Switch(SwitchStatement),
    Labeled(LabeledStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    Block(BlockStatement),
    Expression(Expression),
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabeledStatement {
    pub label: String,
    pub body: Box<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BreakStatement {
    pub label: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContinueStatement {
    pub label: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportDeclaration {
    pub specifiers: Vec<ImportSpecifier>,
    pub source: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportSpecifier {
    Default { local: String },
    Namespace { local: String },
    Named { imported: String, local: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExportDeclaration {
    Named {
        specifiers: Vec<ExportSpecifier>,
        source: Option<String>,
        span: Span,
    },
    All {
        source: String,
        exported: Option<String>,
        span: Span,
    },
    Default {
        declaration: ExportDefaultDeclaration,
        span: Span,
    },
    Declaration {
        declaration: Box<Statement>,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportSpecifier {
    pub local: String,
    pub exported: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExportDefaultDeclaration {
    Expression(Expression),
    Function(FunctionDeclaration),
    Class(ClassDeclaration),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchStatement {
    pub discriminant: Expression,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expression>,
    pub body: Vec<Statement>,
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
pub enum Binding {
    Name(String),
    Object(Vec<ObjectBindingProp>),
    Array(Vec<Option<Binding>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectBindingProp {
    pub key: String,
    pub binding: Binding,
    pub default: Option<Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableDeclarator {
    pub id: Binding,
    pub init: Option<Expression>,
    pub span: Span,
}

/// A function parameter — supports plain names, destructuring, defaults, and rest.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub binding: Binding,
    pub default: Option<Expression>,
    pub rest: bool,
}

impl Param {
    pub fn simple(name: impl Into<String>) -> Self {
        Param {
            binding: Binding::Name(name.into()),
            default: None,
            rest: false,
        }
    }

    pub fn name(&self) -> &str {
        match &self.binding {
            Binding::Name(n) => n,
            _ => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub params: Vec<Param>,
    pub body: BlockStatement,
    pub span: Span,
    pub is_async: bool,
    pub is_generator: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassDeclaration {
    pub name: String,
    pub superclass: Option<String>,
    pub methods: Vec<ClassMethod>,
    pub fields: Vec<ClassField>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassMethod {
    pub name: String,
    pub is_static: bool,
    pub is_constructor: bool,
    pub kind: MethodKind,
    pub params: Vec<Param>,
    pub body: BlockStatement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodKind {
    Method,
    Get,
    Set,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassField {
    pub name: String,
    pub init: Option<Expression>,
    pub is_static: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReturnStatement {
    pub argument: Option<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThrowStatement {
    pub argument: Expression,
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
pub struct DoWhileStatement {
    pub body: Box<Statement>,
    pub test: Expression,
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
pub struct ForOfStatement {
    pub binding: Binding,
    pub binding_kind: VarKind,
    pub iterable: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForInStatement {
    pub binding: Binding,
    pub binding_kind: VarKind,
    pub object: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TryCatchStatement {
    pub body: BlockStatement,
    pub catch_param: Option<Binding>,
    pub catch_body: Option<BlockStatement>,
    pub finally_body: Option<BlockStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockStatement {
    pub body: Vec<Statement>,
    pub span: Span,
}

/// The body of an arrow function or function expression.
#[derive(Clone, Debug, PartialEq)]
pub enum FunctionBody {
    Block(BlockStatement),
    Expr(Box<Expression>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Identifier(String),
    Number(f64),
    BigInt(String), // integer literal with `n` suffix; string is base-10 digits
    String(String),
    Regex(String),
    TemplateLiteral(Vec<TemplateElement>),
    Boolean(bool),
    Null,
    Undefined,
    This,
    Super,
    Array(Vec<Expression>),
    Object(Vec<ObjectProperty>),
    Function(FunctionExpression),
    ArrowFunction {
        params: Vec<Param>,
        body: Box<FunctionBody>,
        is_async: bool,
    },
    New {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    /// `yield expr` / `yield` — only valid inside a generator body.
    Yield(Option<Box<Expression>>),
    /// `yield* expr` — delegate to another iterable.
    YieldStar(Box<Expression>),
    Await(Box<Expression>),
    Typeof(Box<Expression>),
    Void(Box<Expression>),
    Delete(Box<Expression>),
    Spread(Box<Expression>),
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
    Ternary {
        test: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Member {
        object: Box<Expression>,
        property: MemberProperty,
        optional: bool,
    },
    // Comma-sequence operator: evaluate all expressions, return the last value.
    Sequence(Vec<Expression>),
    // Class expression: `class Foo { ... }` or `class { ... }` used as a value.
    Class(Box<ClassDeclaration>),
}

/// One segment of a template literal: either literal text or an interpolated expression.
#[derive(Clone, Debug, PartialEq)]
pub enum TemplateElement {
    Str(String),
    Expr(Box<Expression>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionExpression {
    pub params: Vec<Param>,
    pub body: BlockStatement,
    pub is_async: bool,
    pub is_generator: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectProperty {
    pub key: String,
    pub computed_key: Option<Box<Expression>>,
    pub value: Expression,
    pub shorthand: bool,
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
    Exponent,
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
    BitXor,
    BitAnd,
    BitOr,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    NullishCoalescing,
    Instanceof,
    In,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negate,
    Plus,
    BitNot,
    Typeof,
    Void,
    Delete,
}
