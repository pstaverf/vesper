use crate::diag::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kw {
    On,
    State,
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Func,
    Return,
    And,
    Or,
    Not,
    Bot,
    Inline,
    Menu,
    Link,
    Switch,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Expr { tokens: Vec<Token>, span: Span },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Int(i64),
    Float(f64),
    Str(Vec<StrPart>),
    Ident(String),
    Kw(Kw),
    Bool(bool),
    Null,
    Plus,
    Minus,
    Star,
    Slash,
    SlashSlash,
    Percent,
    Assign,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Coalesce,
    Arrow,
    Dot,
    Comma,
    Colon,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Token {
        Token { kind, span }
    }
}
