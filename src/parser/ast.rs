use crate::diag::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    Command(String),
    Callback(String),
    CallbackAny,
    Text,
    Photo,
    Document,
    Video,
    Audio,
    Voice,
    Sticker,
    Animation,
    Location,
    Contact,
    Poll,
    Inline,
    PreCheckout,
    Payment,
    Any,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Handler {
    pub trigger: Trigger,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Bot {
        name: String,
        span: Span,
    },
    Handler(Handler),
    State {
        name: String,
        handlers: Vec<Handler>,
        span: Span,
    },
    Func {
        name: String,
        params: Vec<Param>,
        body: Block,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    If {
        arms: Vec<(Expr, Block)>,
        orelse: Option<Block>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    For {
        var: String,
        iter: Expr,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Bot { span, .. }
            | Stmt::State { span, .. }
            | Stmt::Func { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::Expr { span, .. } => *span,
            Stmt::Handler(h) => h.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Expr(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Eq,
    Ne,
    Coalesce,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ButtonAction {
    Callback(String),
    Link(Expr),
    Switch(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub text: String,
    pub action: ButtonAction,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Null(Span),
    Bool(bool, Span),
    Int(i64, Span),
    Float(f64, Span),
    Str {
        parts: Vec<StrPart>,
        span: Span,
    },
    Ident(String, Span),
    List(Vec<Expr>, Span),
    Dict(Vec<(String, Expr)>, Span),
    InlineKb(Vec<Vec<Button>>, Span),
    MenuKb(Vec<Vec<String>>, Span),
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Field {
        obj: Box<Expr>,
        name: String,
        span: Span,
    },
    Index {
        obj: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Null(s)
            | Expr::Bool(_, s)
            | Expr::Int(_, s)
            | Expr::Float(_, s)
            | Expr::Str { span: s, .. }
            | Expr::Ident(_, s)
            | Expr::List(_, s)
            | Expr::Dict(_, s)
            | Expr::InlineKb(_, s)
            | Expr::MenuKb(_, s)
            | Expr::Unary { span: s, .. }
            | Expr::Binary { span: s, .. }
            | Expr::Field { span: s, .. }
            | Expr::Index { span: s, .. }
            | Expr::Call { span: s, .. } => *s,
        }
    }
}
