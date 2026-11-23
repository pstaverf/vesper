pub mod ast;

#[cfg(test)]
mod tests;

use crate::diag::{Diag, Span};
use crate::lexer::lex;
use crate::lexer::token::{Kw, StrPart as LexStrPart, Token, TokenKind};
use ast::{
    Arg, BinOp, Block, Button, ButtonAction, Expr, Handler, Param, Program, Stmt, StrPart, Trigger,
    UnOp,
};

pub fn parse(src: &str, file: &str) -> Result<Program, Diag> {
    let tokens = lex(src, file)?;
    let mut p = Parser {
        file,
        toks: tokens,
        pos: 0,
        last: Span::new(1, 1, 1),
    };
    p.program()
}

pub fn parse_expr_tokens(tokens: &[Token], file: &str, span: Span) -> Result<Expr, Diag> {
    let mut toks = tokens.to_vec();
    toks.push(Token::new(TokenKind::Eof, span));
    let mut p = Parser {
        file,
        toks,
        pos: 0,
        last: span,
    };
    let expr = p.expr()?;
    if !p.at(TokenKind::Eof) {
        let t = p.cur().clone();
        return Err(p.err(
            t.span,
            format!("лишний токен в подстановке: {}", describe(&t)),
        ));
    }
    Ok(expr)
}

struct Parser<'a> {
    file: &'a str,
    toks: Vec<Token>,
    pos: usize,
    last: Span,
}

fn describe(t: &Token) -> String {
    match &t.kind {
        TokenKind::Int(v) => format!("число {v}"),
        TokenKind::Float(v) => format!("число {v}"),
        TokenKind::Str(_) => "строка".to_string(),
        TokenKind::Ident(s) => format!("имя '{s}'"),
        kind => format!("'{}'", kind_text(kind)),
    }
}

fn kind_text(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Kw(k) => kw_text(*k),
        TokenKind::Plus => "+",
        TokenKind::Minus => "-",
        TokenKind::Star => "*",
        TokenKind::Slash => "/",
        TokenKind::SlashSlash => "//",
        TokenKind::Percent => "%",
        TokenKind::Assign => "=",
        TokenKind::Eq => "==",
        TokenKind::Ne => "!=",
        TokenKind::Lt => "<",
        TokenKind::Le => "<=",
        TokenKind::Gt => ">",
        TokenKind::Ge => ">=",
        TokenKind::Coalesce => "??",
        TokenKind::Arrow => "->",
        TokenKind::Dot => ".",
        TokenKind::Comma => ",",
        TokenKind::Colon => ":",
        TokenKind::LParen => "(",
        TokenKind::RParen => ")",
        TokenKind::LBracket => "[",
        TokenKind::RBracket => "]",
        TokenKind::LBrace => "{",
        TokenKind::RBrace => "}",
        TokenKind::Newline => "конец строки",
        TokenKind::Indent => "отступ",
        TokenKind::Dedent => "конец блока",
        TokenKind::Eof => "конец файла",
        _ => "значение",
    }
}

fn kw_text(k: Kw) -> &'static str {
    match k {
        Kw::On => "on",
        Kw::State => "state",
        Kw::If => "if",
        Kw::Elif => "elif",
        Kw::Else => "else",
        Kw::While => "while",
        Kw::For => "for",
        Kw::In => "in",
        Kw::Break => "break",
        Kw::Continue => "continue",
        Kw::Func => "func",
        Kw::Return => "return",
        Kw::And => "and",
        Kw::Or => "or",
        Kw::Not => "not",
        Kw::Bot => "bot",
        Kw::Inline => "inline",
        Kw::Menu => "menu",
        Kw::Link => "link",
        Kw::Switch => "switch",
    }
}

fn is_compound(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Handler(_)
            | Stmt::State { .. }
            | Stmt::Func { .. }
            | Stmt::If { .. }
            | Stmt::While { .. }
            | Stmt::For { .. }
    )
}

fn join(a: Span, b: Span) -> Span {
    if a.line != b.line {
        return Span::new(a.line, a.col, a.len);
    }
    let end_a = a.col + a.len;
    let end_b = b.col + b.len;
    Span::new(a.line, a.col, end_a.max(end_b) - a.col)
}

impl Parser<'_> {
    fn err(&self, span: Span, text: impl Into<String>) -> Diag {
        Diag::new(self.file, span, text)
    }

    fn cur(&self) -> &Token {
        let idx = self.pos.min(self.toks.len() - 1);
        &self.toks[idx]
    }

    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos + 1)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.cur().kind == kind
    }

    fn at_kw(&self, k: Kw) -> bool {
        self.cur().kind == TokenKind::Kw(k)
    }

    fn advance(&mut self) -> Token {
        let t = self.cur().clone();
        self.last = t.span;
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            return true;
        }
        false
    }

    fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) {
            self.advance();
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token, Diag> {
        if self.at(kind.clone()) {
            return Ok(self.advance());
        }
        let t = self.cur().clone();
        Err(self.err(
            t.span,
            format!("ожидалось {what}, а найдено {}", describe(&t)),
        ))
    }

    fn program(&mut self) -> Result<Program, Diag> {
        let start = self.cur().span;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(TokenKind::Eof) {
                break;
            }
            if self.at(TokenKind::Indent) {
                let t = self.cur().clone();
                return Err(self.err(
                    t.span,
                    "неожиданный отступ: перед блоком должна быть строка с ':'",
                ));
            }
            if self.at(TokenKind::Dedent) {
                let t = self.cur().clone();
                return Err(self.err(t.span, "лишний конец блока"));
            }
            let stmt = self.stmt(true)?;
            if !is_compound(&stmt) {
                self.finish_stmt()?;
            }
            stmts.push(stmt);
        }
        Ok(Program {
            stmts,
            span: join(start, self.cur().span),
        })
    }

    fn finish_stmt(&mut self) -> Result<(), Diag> {
        if self.at(TokenKind::Newline) {
            self.advance();
            self.skip_newlines();
            return Ok(());
        }
        if self.at(TokenKind::Dedent) || self.at(TokenKind::Eof) {
            return Ok(());
        }
        let t = self.cur().clone();
        if t.span.line > self.last.line {
            return Ok(());
        }
        Err(self.err(
            t.span,
            format!("ожидался конец строки, а найдено {}", describe(&t)),
        ))
    }

    fn stmt(&mut self, top: bool) -> Result<Stmt, Diag> {
        let start = self.cur().span;
        match self.cur().kind.clone() {
            TokenKind::Kw(Kw::Bot) => {
                if !top {
                    let t = self.cur().clone();
                    return Err(self.err(
                        t.span,
                        "объявление bot допустимо только на верхнем уровне файла",
                    ));
                }
                self.advance();
                let name = self.string_value("ожидалось имя бота в кавычках")?;
                Ok(Stmt::Bot {
                    name,
                    span: join(start, self.last),
                })
            }
            TokenKind::Kw(Kw::On) => {
                if !top {
                    let t = self.cur().clone();
                    return Err(self.err(
                        t.span,
                        "обработчик on допустим только на верхнем уровне файла",
                    ));
                }
                let h = self.handler()?;
                Ok(Stmt::Handler(h))
            }
            TokenKind::Kw(Kw::State) => {
                if !top {
                    let t = self.cur().clone();
                    return Err(
                        self.err(t.span, "блок state допустим только на верхнем уровне файла")
                    );
                }
                self.state_block()
            }
            TokenKind::Kw(Kw::Func) => self.func(),
            TokenKind::Kw(Kw::If) => self.if_stmt(),
            TokenKind::Kw(Kw::While) => {
                self.advance();
                let cond = self.expr()?;
                let body = self.block("цикл while")?;
                Ok(Stmt::While {
                    cond,
                    body,
                    span: join(start, self.last),
                })
            }
            TokenKind::Kw(Kw::For) => {
                self.advance();
                let var = self.ident("ожидалось имя переменной после 'for'")?;
                self.expect(TokenKind::Kw(Kw::In), "'in' после переменной цикла")?;
                let iter = self.expr()?;
                let body = self.block("цикл for")?;
                Ok(Stmt::For {
                    var,
                    iter,
                    body,
                    span: join(start, self.last),
                })
            }
            TokenKind::Kw(Kw::Return) => {
                self.advance();
                let value = if self.at(TokenKind::Newline)
                    || self.at(TokenKind::Dedent)
                    || self.at(TokenKind::Eof)
                {
                    None
                } else {
                    Some(self.expr()?)
                };
                let span = join(start, self.last);
                Ok(Stmt::Return { value, span })
            }
            TokenKind::Kw(Kw::Break) => {
                self.advance();
                Ok(Stmt::Break {
                    span: join(start, self.last),
                })
            }
            TokenKind::Kw(Kw::Continue) => {
                self.advance();
                Ok(Stmt::Continue {
                    span: join(start, self.last),
                })
            }
            TokenKind::Kw(Kw::Elif) | TokenKind::Kw(Kw::Else) => {
                let t = self.cur().clone();
                Err(self.err(
                    t.span,
                    format!("'{}' без предшествующего 'if'", kind_text(&t.kind)),
                ))
            }
            TokenKind::Kw(Kw::Not) | TokenKind::Kw(Kw::Inline) | TokenKind::Kw(Kw::Menu) => {
                let expr = self.expr()?;
                let span = join(start, self.last);
                Ok(Stmt::Expr { expr, span })
            }
            TokenKind::Kw(k) => {
                let t = self.cur().clone();
                Err(self.err(
                    t.span,
                    format!("выражение не может начинаться с '{}'", kw_text(k)),
                ))
            }
            _ => {
                let expr = self.expr()?;
                if self.at(TokenKind::Assign) {
                    self.advance();
                    let value = self.expr()?;
                    match &expr {
                        Expr::Ident(..) | Expr::Field { .. } | Expr::Index { .. } => {}
                        other => {
                            return Err(self.err(
                                other.span(),
                                "слева от '=' должно быть имя, поле или элемент списка",
                            ))
                        }
                    }
                    let span = join(start, self.last);
                    return Ok(Stmt::Assign {
                        target: expr,
                        value,
                        span,
                    });
                }
                let span = join(start, self.last);
                Ok(Stmt::Expr { expr, span })
            }
        }
    }

    fn ident(&mut self, what: &str) -> Result<String, Diag> {
        let t = self.cur().clone();
        if let TokenKind::Ident(name) = t.kind {
            self.advance();
            return Ok(name);
        }
        Err(self.err(t.span, format!("{what}, а найдено {}", describe(&t))))
    }

    fn string_value(&mut self, what: &str) -> Result<String, Diag> {
        let t = self.cur().clone();
        if let TokenKind::Str(parts) = t.kind {
            self.advance();
            let mut out = String::new();
            for p in parts {
                match p {
                    LexStrPart::Text(s) => out.push_str(&s),
                    LexStrPart::Expr { span, .. } => {
                        return Err(self.err(span, "в этом месте подстановка {…} недопустима"))
                    }
                }
            }
            return Ok(out);
        }
        Err(self.err(t.span, format!("{what}, а найдено {}", describe(&t))))
    }

    fn handler(&mut self) -> Result<Handler, Diag> {
        let start = self.cur().span;
        self.advance();
        let trigger = self.trigger()?;
        let body = self.block("обработчик on")?;
        Ok(Handler {
            trigger,
            body,
            span: join(start, self.last),
        })
    }

    fn trigger(&mut self) -> Result<Trigger, Diag> {
        let t = self.cur().clone();
        let name = match t.kind {
            TokenKind::Ident(s) => {
                self.advance();
                s
            }
            TokenKind::Kw(k) => {
                self.advance();
                kw_text(k).to_string()
            }
            _ => {
                return Err(self.err(
                    t.span,
                    format!(
                        "после 'on' ожидался тип обработчика, а найдено {}",
                        describe(&t)
                    ),
                ))
            }
        };
        let trigger = match name.as_str() {
            "command" => Trigger::Command(self.string_value("ожидалось имя команды в кавычках")?),
            "callback" => {
                if self.at(TokenKind::Newline) || self.at(TokenKind::Colon) {
                    Trigger::CallbackAny
                } else {
                    Trigger::Callback(self.string_value("ожидались данные кнопки в кавычках")?)
                }
            }
            "text" => Trigger::Text,
            "photo" => Trigger::Photo,
            "document" => Trigger::Document,
            "video" => Trigger::Video,
            "audio" => Trigger::Audio,
            "voice" => Trigger::Voice,
            "sticker" => Trigger::Sticker,
            "animation" => Trigger::Animation,
            "location" => Trigger::Location,
            "contact" => Trigger::Contact,
            "poll" => Trigger::Poll,
            "inline" => Trigger::Inline,
            "pre_checkout" => Trigger::PreCheckout,
            "payment" => Trigger::Payment,
            "any" => Trigger::Any,
            other => return Err(self.err(t.span, format!("неизвестный тип обработчика '{other}'"))),
        };
        Ok(trigger)
    }

    fn state_block(&mut self) -> Result<Stmt, Diag> {
        let start = self.cur().span;
        self.advance();
        let name = self.string_value("ожидалось имя состояния в кавычках")?;
        self.expect(TokenKind::Colon, "':' после имени состояния")?;
        self.expect(TokenKind::Newline, "конец строки после ':'")?;
        self.expect(
            TokenKind::Indent,
            "тело блока state с отступом (внутри допустимы только обработчики on)",
        )?;
        let mut handlers = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(TokenKind::Dedent) {
                self.advance();
                break;
            }
            if self.at(TokenKind::Eof) {
                return Err(self.err(self.cur().span, "блок state не закрыт"));
            }
            if !self.at_kw(Kw::On) {
                let t = self.cur().clone();
                return Err(self.err(
                    t.span,
                    format!(
                        "в блоке state допустимы только обработчики on, а найдено {}",
                        describe(&t)
                    ),
                ));
            }
            handlers.push(self.handler()?);
        }
        if handlers.is_empty() {
            return Err(self.err(start, "в блоке state нет ни одного обработчика on"));
        }
        Ok(Stmt::State {
            name,
            handlers,
            span: join(start, self.last),
        })
    }

    fn func(&mut self) -> Result<Stmt, Diag> {
        let start = self.cur().span;
        self.advance();
        let name = self.ident("ожидалось имя функции после 'func'")?;
        let mut params = Vec::new();
        if self.eat(TokenKind::LParen) {
            loop {
                if self.eat(TokenKind::RParen) {
                    break;
                }
                let pstart = self.cur().span;
                let pname = self.ident("ожидалось имя параметра")?;
                let default = if self.eat(TokenKind::Assign) {
                    Some(self.expr()?)
                } else {
                    None
                };
                params.push(Param {
                    name: pname,
                    default,
                    span: join(pstart, self.last),
                });
                if self.eat(TokenKind::Comma) {
                    continue;
                }
                self.expect(TokenKind::RParen, "')' в конце списка параметров")?;
                break;
            }
        }
        let body = self.block("функция func")?;
        Ok(Stmt::Func {
            name,
            params,
            body,
            span: join(start, self.last),
        })
    }

    fn if_stmt(&mut self) -> Result<Stmt, Diag> {
        let start = self.cur().span;
        self.advance();
        let mut arms = Vec::new();
        let cond = self.expr()?;
        let body = self.block("условие if")?;
        arms.push((cond, body));
        let mut orelse = None;
        loop {
            if self.at_kw(Kw::Elif) {
                self.advance();
                let c = self.expr()?;
                let b = self.block("условие elif")?;
                arms.push((c, b));
                continue;
            }
            if self.at_kw(Kw::Else) {
                self.advance();
                orelse = Some(self.block("блок else")?);
            }
            break;
        }
        Ok(Stmt::If {
            arms,
            orelse,
            span: join(start, self.last),
        })
    }

    fn block(&mut self, what: &str) -> Result<Block, Diag> {
        let start = self.cur().span;
        self.expect(TokenKind::Colon, &format!("':' после заголовка {what}"))?;
        self.expect(TokenKind::Newline, "конец строки после ':'")?;
        self.expect(
            TokenKind::Indent,
            "тело блока с отступом (после ':' должна быть строка с отступом)",
        )?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(TokenKind::Dedent) {
                self.advance();
                break;
            }
            if self.at(TokenKind::Eof) {
                return Err(self.err(self.cur().span, format!("блок {what} не закрыт")));
            }
            if self.at(TokenKind::Indent) {
                let t = self.cur().clone();
                return Err(self.err(t.span, "лишний отступ: строка не продолжает блок"));
            }
            let stmt = self.stmt(false)?;
            if !is_compound(&stmt) {
                self.finish_stmt()?;
            }
            stmts.push(stmt);
        }
        Ok(Block {
            stmts,
            span: join(start, self.last),
        })
    }

    fn expr(&mut self) -> Result<Expr, Diag> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.and_expr()?;
        while self.at_kw(Kw::Or) {
            self.advance();
            let rhs = self.and_expr()?;
            lhs = self.binary(BinOp::Or, lhs, rhs);
        }
        Ok(lhs)
    }

    fn and_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.coalesce_expr()?;
        while self.at_kw(Kw::And) {
            self.advance();
            let rhs = self.coalesce_expr()?;
            lhs = self.binary(BinOp::And, lhs, rhs);
        }
        Ok(lhs)
    }

    fn coalesce_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.equality_expr()?;
        while self.at(TokenKind::Coalesce) {
            self.advance();
            let rhs = self.equality_expr()?;
            lhs = self.binary(BinOp::Coalesce, lhs, rhs);
        }
        Ok(lhs)
    }

    fn equality_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.compare_expr()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Eq => BinOp::Eq,
                TokenKind::Ne => BinOp::Ne,
                _ => break,
            };
            self.advance();
            let rhs = self.compare_expr()?;
            lhs = self.binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn compare_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.add_expr()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Ge => BinOp::Ge,
                TokenKind::Kw(Kw::In) => BinOp::In,
                _ => break,
            };
            self.advance();
            let rhs = self.add_expr()?;
            lhs = self.binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn add_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.mul_expr()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.mul_expr()?;
            lhs = self.binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn mul_expr(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::SlashSlash => BinOp::FloorDiv,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.unary()?;
            lhs = self.binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn binary(&self, op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        let span = join(lhs.span(), rhs.span());
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        }
    }

    fn unary(&mut self) -> Result<Expr, Diag> {
        let start = self.cur().span;
        if self.at(TokenKind::Minus) {
            self.advance();
            let e = self.unary()?;
            let span = join(start, e.span());
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(e),
                span,
            });
        }
        if self.at_kw(Kw::Not) {
            self.advance();
            let e = self.unary()?;
            let span = join(start, e.span());
            return Ok(Expr::Unary {
                op: UnOp::Not,
                expr: Box::new(e),
                span,
            });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, Diag> {
        let mut base = self.primary()?;
        loop {
            if self.at(TokenKind::Dot) {
                self.advance();
                let name = self.name_token("ожидалось имя поля после '.'")?;
                let span = join(base.span(), self.last);
                base = Expr::Field {
                    obj: Box::new(base),
                    name,
                    span,
                };
                continue;
            }
            if self.at(TokenKind::LBracket) {
                self.advance();
                let index = self.expr()?;
                self.expect(TokenKind::RBracket, "']' после индекса")?;
                let span = join(base.span(), self.last);
                base = Expr::Index {
                    obj: Box::new(base),
                    index: Box::new(index),
                    span,
                };
                continue;
            }
            if self.at(TokenKind::LParen) {
                let args = self.call_args()?;
                let span = join(base.span(), self.last);
                base = Expr::Call {
                    callee: Box::new(base),
                    args,
                    span,
                };
                continue;
            }
            break;
        }
        Ok(base)
    }

    fn name_token(&mut self, what: &str) -> Result<String, Diag> {
        let t = self.cur().clone();
        match t.kind {
            TokenKind::Ident(s) => {
                self.advance();
                Ok(s)
            }
            TokenKind::Kw(k) => {
                self.advance();
                Ok(kw_text(k).to_string())
            }
            _ => Err(self.err(t.span, format!("{what}, а найдено {}", describe(&t)))),
        }
    }

    fn call_args(&mut self) -> Result<Vec<Arg>, Diag> {
        self.expect(TokenKind::LParen, "'('")?;
        let mut args = Vec::new();
        loop {
            if self.eat(TokenKind::RParen) {
                break;
            }
            let start = self.cur().span;
            let named = matches!(self.cur().kind, TokenKind::Ident(_) | TokenKind::Kw(_))
                && self.peek().map(|t| t.kind == TokenKind::Assign) == Some(true);
            let name = if named {
                let n = self.name_token("ожидалось имя аргумента")?;
                self.expect(TokenKind::Assign, "'=' после имени аргумента")?;
                Some(n)
            } else {
                None
            };
            let value = self.expr()?;
            args.push(Arg {
                name,
                value,
                span: join(start, self.last),
            });
            if self.eat(TokenKind::Comma) {
                continue;
            }
            self.expect(TokenKind::RParen, "')' в конце аргументов")?;
            break;
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr, Diag> {
        let t = self.cur().clone();
        let span = t.span;
        match t.kind {
            TokenKind::Int(v) => {
                self.advance();
                Ok(Expr::Int(v, span))
            }
            TokenKind::Float(v) => {
                self.advance();
                Ok(Expr::Float(v, span))
            }
            TokenKind::Bool(v) => {
                self.advance();
                Ok(Expr::Bool(v, span))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Null(span))
            }
            TokenKind::Str(parts) => {
                self.advance();
                let mut out = Vec::new();
                for p in parts {
                    match p {
                        LexStrPart::Text(s) => out.push(StrPart::Text(s)),
                        LexStrPart::Expr {
                            tokens,
                            span: sspan,
                        } => {
                            out.push(StrPart::Expr(Box::new(parse_expr_tokens(
                                &tokens, self.file, sspan,
                            )?)));
                        }
                    }
                }
                Ok(Expr::Str { parts: out, span })
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(name, span))
            }
            TokenKind::LParen => {
                self.advance();
                let e = self.expr()?;
                self.expect(TokenKind::RParen, "')' после выражения")?;
                Ok(e)
            }
            TokenKind::LBracket => self.list_expr(),
            TokenKind::LBrace => self.dict_expr(),
            TokenKind::Kw(Kw::Inline) => self.inline_kb(),
            TokenKind::Kw(Kw::Menu) => self.menu_kb(),
            TokenKind::Kw(Kw::State) => {
                Err(self.err(span, "'state' здесь не переменная, используй get_state()"))
            }
            TokenKind::Kw(k) => Err(self.err(
                span,
                format!("выражение не может начинаться с '{}'", kw_text(k)),
            )),
            _ => Err(self.err(
                span,
                format!("ожидалось выражение, а найдено {}", describe(&t)),
            )),
        }
    }

    fn list_expr(&mut self) -> Result<Expr, Diag> {
        let start = self.cur().span;
        self.advance();
        let mut items = Vec::new();
        loop {
            if self.eat(TokenKind::RBracket) {
                break;
            }
            items.push(self.expr()?);
            if self.eat(TokenKind::Comma) {
                continue;
            }
            self.expect(TokenKind::RBracket, "']' в конце списка")?;
            break;
        }
        Ok(Expr::List(items, join(start, self.last)))
    }

    fn dict_expr(&mut self) -> Result<Expr, Diag> {
        let start = self.cur().span;
        self.advance();
        let mut pairs = Vec::new();
        loop {
            if self.eat(TokenKind::RBrace) {
                break;
            }
            let key = match self.cur().kind.clone() {
                TokenKind::Str(_) => self.string_value("ожидался ключ в кавычках")?,
                TokenKind::Ident(s) => {
                    self.advance();
                    s
                }
                _ => {
                    let t = self.cur().clone();
                    return Err(self.err(
                        t.span,
                        format!("ожидался ключ словаря, а найдено {}", describe(&t)),
                    ));
                }
            };
            self.expect(TokenKind::Colon, "':' после ключа словаря")?;
            let value = self.expr()?;
            pairs.push((key, value));
            if self.eat(TokenKind::Comma) {
                continue;
            }
            self.expect(TokenKind::RBrace, "'}' в конце словаря")?;
            break;
        }
        Ok(Expr::Dict(pairs, join(start, self.last)))
    }

    fn inline_kb(&mut self) -> Result<Expr, Diag> {
        let start = self.cur().span;
        self.advance();
        self.expect(TokenKind::LBracket, "'[' после inline")?;
        let mut rows = Vec::new();
        loop {
            if self.eat(TokenKind::RBracket) {
                break;
            }
            rows.push(self.inline_row()?);
            if self.eat(TokenKind::Comma) {
                continue;
            }
            self.expect(
                TokenKind::RBracket,
                "']' в конце inline[...] (клавиатура не закрыта)",
            )?;
            break;
        }
        Ok(Expr::InlineKb(rows, join(start, self.last)))
    }

    fn inline_row(&mut self) -> Result<Vec<Button>, Diag> {
        self.expect(TokenKind::LBracket, "'[' в начале строки кнопок")?;
        let mut buttons = Vec::new();
        loop {
            if self.eat(TokenKind::RBracket) {
                break;
            }
            let start = self.cur().span;
            let text = self.string_value("ожидался текст кнопки в кавычках")?;
            self.expect(TokenKind::Arrow, "'->' после текста кнопки")?;
            let action = match self.cur().kind.clone() {
                TokenKind::Str(_) => ButtonAction::Callback(self.string_value("ожидались данные")?),
                TokenKind::Kw(Kw::Link) => {
                    self.advance();
                    ButtonAction::Link(self.expr()?)
                }
                TokenKind::Kw(Kw::Switch) => {
                    self.advance();
                    ButtonAction::Switch(self.expr()?)
                }
                _ => {
                    let t = self.cur().clone();
                    return Err(self.err(
                        t.span,
                        format!(
                            "после '->' ожидались данные в кавычках, link или switch, а найдено {}",
                            describe(&t)
                        ),
                    ));
                }
            };
            buttons.push(Button {
                text,
                action,
                span: join(start, self.last),
            });
            if self.eat(TokenKind::Comma) {
                continue;
            }
            self.expect(TokenKind::RBracket, "']' в конце строки кнопок")?;
            break;
        }
        Ok(buttons)
    }

    fn menu_kb(&mut self) -> Result<Expr, Diag> {
        let start = self.cur().span;
        self.advance();
        self.expect(TokenKind::LBracket, "'[' после menu")?;
        let mut rows = Vec::new();
        loop {
            if self.eat(TokenKind::RBracket) {
                break;
            }
            self.expect(TokenKind::LBracket, "'[' в начале строки menu")?;
            let mut row = Vec::new();
            loop {
                if self.eat(TokenKind::RBracket) {
                    break;
                }
                row.push(self.string_value("ожидался текст кнопки в кавычках")?);
                if self.eat(TokenKind::Comma) {
                    continue;
                }
                self.expect(TokenKind::RBracket, "']' в конце строки menu")?;
                break;
            }
            rows.push(row);
            if self.eat(TokenKind::Comma) {
                continue;
            }
            self.expect(TokenKind::RBracket, "']' в конце menu[...]")?;
            break;
        }
        Ok(Expr::MenuKb(rows, join(start, self.last)))
    }
}
