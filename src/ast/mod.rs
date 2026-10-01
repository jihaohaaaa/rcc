use crate::span::Span;
use crate::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LogicalAnd,
    LogicalOr,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    ShlAssign,
    ShrAssign,
    Comma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Pos,         // +x
    Neg,         // -x
    BitNot,      // ~x
    LogNot,      // !x
    Deref,       // *x
    AddrOf,      // &x
    PreInc,      // ++x
    PreDec,      // --x
    PostInc,     // x++
    PostDec,     // x--
    Sizeof,      // sizeof x
    Alignof,     // _Alignof x
    AddrOfLabel, // &&label (GNU C computed goto)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Designator {
    Field(String),
    Index(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitItem {
    pub designators: Vec<Designator>,
    pub init: Initializer,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Initializer {
    Single(Box<Expr>),
    List(Vec<InitItem>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64, Span),
    Float(f64, Span),
    Char(char, Span),
    String(String, Span),
    Var(String, Span),
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Ternary {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Member {
        expr: Box<Expr>,
        member: String,
        is_arrow: bool,
        span: Span,
    },
    Index {
        expr: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Cast {
        target_type: Type,
        expr: Box<Expr>,
        span: Span,
    },
    CompoundLiteral {
        target_type: Type,
        init: Initializer,
        span: Span,
    },
    SizeofType {
        target_type: Type,
        span: Span,
    },
    StmtExpr {
        body: Vec<Stmt>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int(_, span)
            | Expr::Float(_, span)
            | Expr::Char(_, span)
            | Expr::String(_, span)
            | Expr::Var(_, span)
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Ternary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Member { span, .. }
            | Expr::Index { span, .. }
            | Expr::Cast { span, .. }
            | Expr::CompoundLiteral { span, .. }
            | Expr::SizeofType { span, .. }
            | Expr::StmtExpr { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub name: String,
    pub ty: Type,
    pub init: Option<Initializer>,
    pub is_static: bool,
    pub is_extern: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForInit {
    Expr(Expr),
    Decl(Vec<VarDecl>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expr(Expr),
    Return(Option<Expr>, Span),
    Block(Vec<Stmt>, Span),
    If {
        cond: Expr,
        then_stmt: Box<Stmt>,
        else_stmt: Option<Box<Stmt>>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
        span: Span,
    },
    For {
        init: Option<ForInit>,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    Switch {
        expr: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    Case {
        val: i64,
        body: Box<Stmt>,
        span: Span,
    },
    Default {
        body: Box<Stmt>,
        span: Span,
    },
    Goto(String, Span),
    GotoExpr(Expr, Span),
    Label(String, Box<Stmt>, Span),
    Decl(Vec<VarDecl>),
    Empty(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub ret_type: Type,
    pub params: Vec<(Type, String)>,
    pub is_variadic: bool,
    pub body: Option<Vec<Stmt>>,
    pub is_static: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalVar {
    pub name: String,
    pub ty: Type,
    pub init: Option<Initializer>,
    pub is_static: bool,
    pub is_extern: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    pub functions: Vec<Function>,
    pub globals: Vec<GlobalVar>,
}
