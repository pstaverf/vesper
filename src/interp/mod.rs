pub mod builtins;
pub mod env;
pub mod value;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::diag::{Diag, Span};
use crate::parser::ast::{BinOp, Block, ButtonAction, Expr, Handler, Program, Stmt, StrPart, UnOp};
use crate::parser::parse;

pub use builtins::{Builtin, BuiltinFunc};
pub use env::{EnvCtx, FixedEnv, RealEnv};
pub use value::Value;

pub const MAX_DEPTH: usize = 100;
pub const STACK_SIZE: usize = 32 * 1024 * 1024;

pub struct Ctx {
    pub globals: Rc<RefCell<BTreeMap<String, Value>>>,
    pub file: String,
    pub builtins: BTreeMap<String, Builtin>,
    pub stack: Vec<String>,
    pub logs: Vec<String>,
    pub exit: Option<i64>,
}

impl Ctx {
    pub fn new(file: &str) -> Ctx {
        Ctx {
            globals: Rc::new(RefCell::new(BTreeMap::new())),
            file: file.to_string(),
            builtins: builtins::core_table(),
            stack: Vec::new(),
            logs: Vec::new(),
            exit: None,
        }
    }

    pub fn register(&mut self, name: &str, b: Builtin) {
        self.builtins.insert(name.to_string(), b);
    }

    pub fn set_global(&mut self, name: &str, v: Value) {
        self.globals.borrow_mut().insert(name.to_string(), v);
    }

    pub fn get_global(&self, name: &str) -> Option<Value> {
        self.globals.borrow().get(name).cloned()
    }

    fn err(&self, span: Span, text: impl Into<String>) -> Diag {
        let mut d = Diag::new(&self.file, span, text);
        if !self.stack.is_empty() {
            d.text
                .push_str(&format!("\n  стек вызовов: {}", self.stack.join(" → ")));
        }
        d
    }
}

pub enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
    Exit,
}

pub struct Run<'a> {
    pub ctx: &'a mut Ctx,
    pub env: &'a mut dyn EnvCtx,
    pub functions: BTreeMap<String, &'a Stmt>,
    pub locals: Vec<BTreeMap<String, Value>>,
    pub depth: usize,
    pub global_scope: bool,
}

pub fn run_program(program: &Program, file: &str, env: &mut dyn EnvCtx) -> Result<Ctx, Diag> {
    let mut ctx = Ctx::new(file);
    run_top(&mut ctx, program, env)?;
    Ok(ctx)
}

pub fn run_top(ctx: &mut Ctx, program: &Program, env: &mut dyn EnvCtx) -> Result<(), Diag> {
    let mut run = Run::new(ctx, env, program);
    run.global_scope = true;
    for stmt in &program.stmts {
        if is_top_runtime(stmt) {
            run.exec(stmt)?;
            if run.ctx.exit.is_some() {
                break;
            }
        }
    }
    Ok(())
}

pub fn run_handler(
    ctx: &mut Ctx,
    program: &Program,
    handler: &Handler,
    ctx_vars: BTreeMap<String, Value>,
    env: &mut dyn EnvCtx,
) -> Result<Value, Diag> {
    let mut run = Run::new(ctx, env, program);
    for (k, v) in ctx_vars {
        run.set_local(&k, v);
    }
    match run.exec_block(&handler.body)? {
        Flow::Return(v) => Ok(v),
        _ => Ok(Value::Null),
    }
}

pub fn run_with_stack<T, F>(f: F) -> std::io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(f)
        .map_err(|e| std::io::Error::new(e.kind(), format!("не удалось создать поток: {e}")))?
        .join()
        .map_err(|_| std::io::Error::other("поток интерпретатора упал"))
}

pub fn handler_table(program: &Program) -> Vec<(Option<String>, Handler)> {
    let mut out = Vec::new();
    for stmt in &program.stmts {
        match stmt {
            Stmt::Handler(h) => out.push((None, h.clone())),
            Stmt::State { name, handlers, .. } => {
                for h in handlers {
                    out.push((Some(name.clone()), h.clone()));
                }
            }
            _ => {}
        }
    }
    out
}

fn is_top_runtime(stmt: &Stmt) -> bool {
    !matches!(
        stmt,
        Stmt::Bot { .. } | Stmt::Handler(_) | Stmt::State { .. } | Stmt::Func { .. }
    )
}

impl<'a> Run<'a> {
    pub fn new(ctx: &'a mut Ctx, env: &'a mut dyn EnvCtx, program: &'a Program) -> Run<'a> {
        let mut functions = BTreeMap::new();
        for stmt in &program.stmts {
            if let Stmt::Func { name, .. } = stmt {
                functions.insert(name.clone(), stmt);
            }
        }
        Run {
            ctx,
            env,
            functions,
            locals: vec![BTreeMap::new()],
            depth: 0,
            global_scope: false,
        }
    }

    pub fn set_local(&mut self, name: &str, v: Value) {
        if let Some(frame) = self.locals.last_mut() {
            frame.insert(name.to_string(), v);
        }
    }

    pub fn diag(&self, span: Span, text: impl Into<String>) -> Diag {
        self.ctx.err(span, text)
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        if let Some(frame) = self.locals.last() {
            if let Some(v) = frame.get(name) {
                return Some(v.clone());
            }
        }
        self.ctx.globals.borrow().get(name).cloned()
    }

    fn assign(&mut self, name: &str, v: Value) {
        if self.global_scope && self.locals.len() == 1 {
            self.ctx.globals.borrow_mut().insert(name.to_string(), v);
            return;
        }
        if let Some(frame) = self.locals.last_mut() {
            frame.insert(name.to_string(), v);
        }
    }

    pub fn exec_block(&mut self, block: &Block) -> Result<Flow, Diag> {
        for stmt in &block.stmts {
            match self.exec(stmt)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    pub fn exec(&mut self, stmt: &Stmt) -> Result<Flow, Diag> {
        if self.ctx.exit.is_some() {
            return Ok(Flow::Exit);
        }
        match stmt {
            Stmt::Bot { .. } | Stmt::Handler(_) | Stmt::State { .. } | Stmt::Func { .. } => {
                Ok(Flow::Normal)
            }
            Stmt::Assign { target, value, .. } => {
                let v = self.eval(value)?;
                self.store(target, v)?;
                Ok(Flow::Normal)
            }
            Stmt::If { arms, orelse, .. } => {
                for (cond, body) in arms {
                    if self.eval(cond)?.truthy() {
                        return self.exec_block(body);
                    }
                }
                match orelse {
                    Some(b) => self.exec_block(b),
                    None => Ok(Flow::Normal),
                }
            }
            Stmt::While { cond, body, .. } => {
                loop {
                    if !self.eval(cond)?.truthy() {
                        break;
                    }
                    match self.exec_block(body)? {
                        Flow::Break => break,
                        Flow::Exit => return Ok(Flow::Exit),
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                let items = self.iter_values(iter)?;
                for item in items {
                    self.assign(var, item);
                    match self.exec_block(body)? {
                        Flow::Break => break,
                        Flow::Exit => return Ok(Flow::Exit),
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Return { value, .. } => match value {
                Some(e) => Ok(Flow::Return(self.eval(e)?)),
                None => Ok(Flow::Return(Value::Null)),
            },
            Stmt::Break { .. } => Ok(Flow::Break),
            Stmt::Continue { .. } => Ok(Flow::Continue),
            Stmt::Expr { expr, .. } => {
                self.eval(expr)?;
                Ok(Flow::Normal)
            }
        }
    }

    fn store(&mut self, target: &Expr, v: Value) -> Result<(), Diag> {
        if self.stopping() {
            return Ok(());
        }
        match target {
            Expr::Ident(name, _) => {
                self.assign(name, v);
                Ok(())
            }
            Expr::Field { obj, name, span } => {
                let base = self.eval(obj)?;
                match base {
                    Value::Map(m) => {
                        m.borrow_mut().insert(name.clone(), v);
                        Ok(())
                    }
                    other => Err(self.diag(
                        *span,
                        format!(
                            "нельзя присвоить поле у значения типа {}",
                            other.type_name()
                        ),
                    )),
                }
            }
            Expr::Index { obj, index, span } => {
                let base = self.eval(obj)?;
                let key = self.eval(index)?;
                match base {
                    Value::List(l) => {
                        let idx = self.index_of(&key, *span)?;
                        let mut items = l.borrow_mut();
                        let len = items.len() as i64;
                        let real = if idx < 0 { len + idx } else { idx };
                        if real < 0 || real >= len {
                            return Err(self.diag(
                                *span,
                                format!("выход за границы списка: индекс {idx}, длина {len}"),
                            ));
                        }
                        items[real as usize] = v;
                        Ok(())
                    }
                    Value::Map(m) => {
                        let k = self.key_of(&key, *span)?;
                        m.borrow_mut().insert(k, v);
                        Ok(())
                    }
                    other => Err(self.diag(
                        *span,
                        format!(
                            "нельзя присвоить элемент у значения типа {}",
                            other.type_name()
                        ),
                    )),
                }
            }
            other => Err(self.diag(other.span(), "нельзя присвоить значение этому выражению")),
        }
    }

    fn index_of(&self, key: &Value, span: Span) -> Result<i64, Diag> {
        match key {
            Value::Int(i) => Ok(*i),
            other => Err(self.diag(
                span,
                format!("индекс должен быть int, а не {}", other.type_name()),
            )),
        }
    }

    fn key_of(&self, key: &Value, span: Span) -> Result<String, Diag> {
        match key {
            Value::Str(s) => Ok(s.to_string()),
            Value::Int(_) | Value::Float(_) | Value::Bool(_) => Ok(key.display()),
            other => Err(self.diag(
                span,
                format!(
                    "ключ словаря должен быть строкой, а не {}",
                    other.type_name()
                ),
            )),
        }
    }

    fn iter_values(&mut self, iter: &Expr) -> Result<Vec<Value>, Diag> {
        match self.eval(iter)? {
            Value::List(l) => Ok(l.borrow().clone()),
            Value::Map(m) => Ok(m.borrow().keys().map(|k| Value::str(k.clone())).collect()),
            Value::Str(s) => Ok(s.chars().map(|c| Value::str(c.to_string())).collect()),
            other => Err(self.diag(
                iter.span(),
                format!("в for нельзя перебрать значение типа {}", other.type_name()),
            )),
        }
    }

    fn stopping(&self) -> bool {
        self.ctx.exit.is_some()
    }

    pub fn eval(&mut self, expr: &Expr) -> Result<Value, Diag> {
        if self.stopping() {
            return Ok(Value::Null);
        }
        match expr {
            Expr::Null(_) => Ok(Value::Null),
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::Int(i, _) => Ok(Value::Int(*i)),
            Expr::Float(f, _) => Ok(Value::Float(*f)),
            Expr::Ident(name, span) => match self.lookup(name) {
                Some(v) => Ok(v),
                None => Err(self.diag(*span, format!("неизвестное имя '{name}'"))),
            },
            Expr::Str { parts, .. } => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        StrPart::Text(s) => out.push_str(s),
                        StrPart::Expr(e) => {
                            let v = self.eval(e)?;
                            out.push_str(&v.display());
                        }
                    }
                }
                Ok(Value::str(out))
            }
            Expr::List(items, _) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.eval(item)?);
                }
                Ok(Value::list(out))
            }
            Expr::Dict(pairs, _) => {
                let mut m = BTreeMap::new();
                for (k, e) in pairs {
                    let v = self.eval(e)?;
                    m.insert(k.clone(), v);
                }
                Ok(Value::map(m))
            }
            Expr::InlineKb(rows, _) => {
                let mut lines = Vec::new();
                for row in rows {
                    let mut line = Vec::new();
                    for btn in row {
                        let mut m = BTreeMap::new();
                        m.insert("text".to_string(), Value::str(btn.text.clone()));
                        match &btn.action {
                            ButtonAction::Callback(data) => {
                                m.insert("callback_data".to_string(), Value::str(data.clone()));
                            }
                            ButtonAction::Link(e) => {
                                let v = self.eval(e)?;
                                m.insert("url".to_string(), Value::str(v.display()));
                            }
                            ButtonAction::Switch(e) => {
                                let v = self.eval(e)?;
                                m.insert(
                                    "switch_inline_query".to_string(),
                                    Value::str(v.display()),
                                );
                            }
                        }
                        line.push(Value::map(m));
                    }
                    lines.push(Value::list(line));
                }
                Ok(Value::from_pairs(vec![(
                    "inline_keyboard",
                    Value::list(lines),
                )]))
            }
            Expr::MenuKb(rows, _) => {
                let mut lines = Vec::new();
                for row in rows {
                    let mut line = Vec::new();
                    for text in row {
                        line.push(Value::from_pairs(vec![("text", Value::str(text.clone()))]));
                    }
                    lines.push(Value::list(line));
                }
                Ok(Value::from_pairs(vec![
                    ("keyboard", Value::list(lines)),
                    ("resize_keyboard", Value::Bool(true)),
                ]))
            }
            Expr::Unary { op, expr, span } => {
                let v = self.eval(expr)?;
                if self.stopping() {
                    return Ok(Value::Null);
                }
                match op {
                    UnOp::Neg => match v {
                        Value::Int(i) => i
                            .checked_neg()
                            .map(Value::Int)
                            .ok_or_else(|| self.diag(*span, "переполнение при отрицании числа")),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        other => Err(self.diag(
                            *span,
                            format!("нельзя применить '-' к значению типа {}", other.type_name()),
                        )),
                    },
                    UnOp::Not => Ok(Value::Bool(!v.truthy())),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => self.binary(*op, lhs, rhs, *span),
            Expr::Field { obj, name, .. } => {
                let base = self.eval(obj)?;
                Ok(field(&base, name).unwrap_or(Value::Null))
            }
            Expr::Index { obj, index, span } => {
                let base = self.eval(obj)?;
                let key = self.eval(index)?;
                self.index_get(&base, &key, *span)
            }
            Expr::Call { callee, args, span } => self.call(callee, args, *span),
        }
    }

    fn index_get(&mut self, base: &Value, key: &Value, span: Span) -> Result<Value, Diag> {
        if self.stopping() {
            return Ok(Value::Null);
        }
        match base {
            Value::List(l) => {
                let idx = self.index_of(key, span)?;
                let items = l.borrow();
                let len = items.len() as i64;
                let real = if idx < 0 { len + idx } else { idx };
                if real < 0 || real >= len {
                    return Ok(Value::Null);
                }
                Ok(items[real as usize].clone())
            }
            Value::Map(m) => {
                let k = self.key_of(key, span)?;
                Ok(m.borrow().get(&k).cloned().unwrap_or(Value::Null))
            }
            Value::Str(s) => {
                let idx = self.index_of(key, span)?;
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let real = if idx < 0 { len + idx } else { idx };
                if real < 0 || real >= len {
                    return Ok(Value::Null);
                }
                Ok(Value::str(chars[real as usize].to_string()))
            }
            Value::Null => Ok(Value::Null),
            other => Err(self.diag(
                span,
                format!("нельзя взять элемент у значения типа {}", other.type_name()),
            )),
        }
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, span: Span) -> Result<Value, Diag> {
        match op {
            BinOp::And => {
                let l = self.eval(lhs)?;
                if !l.truthy() {
                    return Ok(Value::Bool(false));
                }
                return Ok(Value::Bool(self.eval(rhs)?.truthy()));
            }
            BinOp::Or => {
                let l = self.eval(lhs)?;
                if l.truthy() {
                    return Ok(Value::Bool(true));
                }
                return Ok(Value::Bool(self.eval(rhs)?.truthy()));
            }
            BinOp::Coalesce => {
                let l = self.eval(lhs)?;
                if !matches!(l, Value::Null) {
                    return Ok(l);
                }
                return self.eval(rhs);
            }
            _ => {}
        }
        let l = self.eval(lhs)?;
        let r = self.eval(rhs)?;
        if self.stopping() {
            return Ok(Value::Null);
        }
        match op {
            BinOp::Add => self.add(&l, &r, span),
            BinOp::Sub => self.arith(&l, &r, span, "вычитания", arith_sub),
            BinOp::Mul => self.arith(&l, &r, span, "умножения", arith_mul),
            BinOp::Div => self.arith(&l, &r, span, "деления", arith_div),
            BinOp::FloorDiv => self.arith(&l, &r, span, "целочисленного деления", arith_floor_div),
            BinOp::Mod => self.arith(&l, &r, span, "остатка", arith_mod),
            BinOp::Eq => Ok(Value::Bool(l.equals(&r))),
            BinOp::Ne => Ok(Value::Bool(!l.equals(&r))),
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => self.compare(op, &l, &r, span),
            BinOp::In => self.contains(&l, &r, span),
            BinOp::And | BinOp::Or | BinOp::Coalesce => Ok(Value::Null),
        }
    }

    fn add(&mut self, l: &Value, r: &Value, span: Span) -> Result<Value, Diag> {
        match (l, r) {
            (Value::Str(a), b) => {
                let mut s = a.to_string();
                s.push_str(&b.display());
                Ok(Value::str(s))
            }
            (a, Value::Str(_)) => {
                let mut s = a.display();
                s.push_str(&r.display());
                Ok(Value::str(s))
            }
            (Value::List(a), Value::List(b)) => {
                let mut items = a.borrow().clone();
                items.extend(b.borrow().iter().cloned());
                Ok(Value::list(items))
            }
            _ => self.arith(l, r, span, "сложения", arith_add),
        }
    }

    fn arith(
        &mut self,
        l: &Value,
        r: &Value,
        span: Span,
        what: &str,
        f: fn(&Value, &Value) -> Result<Value, String>,
    ) -> Result<Value, Diag> {
        f(l, r).map_err(|msg| self.diag(span, format!("ошибка {what}: {msg}")))
    }

    fn compare(&mut self, op: BinOp, l: &Value, r: &Value, span: Span) -> Result<Value, Diag> {
        let ord = match (l, r) {
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Int(a), Value::Float(b)) => value::cmp_int_float(*a, *b),
            (Value::Float(a), Value::Int(b)) => value::cmp_float_int(*a, *b),
            (Value::Str(a), Value::Str(b)) => Some(a.as_ref().cmp(b.as_ref())),
            (Value::List(a), Value::List(b)) => {
                let a = a.borrow();
                let b = b.borrow();
                let mut out = std::cmp::Ordering::Equal;
                for (x, y) in a.iter().zip(b.iter()) {
                    if !x.equals(y) {
                        out = match (x, y) {
                            (Value::Int(x), Value::Int(y)) => x.cmp(y),
                            (Value::Str(x), Value::Str(y)) => x.as_ref().cmp(y.as_ref()),
                            _ => {
                                return Err(self.diag(
                                    span,
                                    "списки сравниваются только из чисел или только из строк",
                                ))
                            }
                        };
                        break;
                    }
                }
                if out == std::cmp::Ordering::Equal {
                    out = a.len().cmp(&b.len());
                }
                Some(out)
            }
            (Value::Map(_), Value::Map(_)) => {
                return Err(self.diag(span, "словари нельзя сравнивать через '<' или '>'"))
            }
            (Value::Bool(_), Value::Bool(_)) => {
                return Err(self.diag(span, "нельзя сравнить bool и bool через '<' или '>'"))
            }
            (a, b) if a.type_name() != b.type_name() => {
                return Err(self.diag(
                    span,
                    format!(
                        "нельзя сравнить {} и {}: разные типы",
                        a.type_name(),
                        b.type_name()
                    ),
                ))
            }
            _ => None,
        };
        let res = match ord {
            None => false,
            Some(o) => match op {
                BinOp::Lt => o == std::cmp::Ordering::Less,
                BinOp::Le => o != std::cmp::Ordering::Greater,
                BinOp::Gt => o == std::cmp::Ordering::Greater,
                BinOp::Ge => o != std::cmp::Ordering::Less,
                _ => false,
            },
        };
        Ok(Value::Bool(res))
    }

    fn contains(&mut self, needle: &Value, hay: &Value, span: Span) -> Result<Value, Diag> {
        match hay {
            Value::Str(h) => match needle {
                Value::Str(n) => Ok(Value::Bool(h.contains(n.as_ref()))),
                other => Err(self.diag(
                    span,
                    format!(
                        "в строке можно искать только строку, а не {}",
                        other.type_name()
                    ),
                )),
            },
            Value::List(l) => Ok(Value::Bool(l.borrow().iter().any(|x| x.equals(needle)))),
            Value::Map(m) => {
                let k = self.key_of(needle, span)?;
                Ok(Value::Bool(m.borrow().contains_key(&k)))
            }
            other => Err(self.diag(
                span,
                format!(
                    "'in' работает для строк, списков и словарей, а не для {}",
                    other.type_name()
                ),
            )),
        }
    }

    fn call(
        &mut self,
        callee: &Expr,
        args: &[crate::parser::ast::Arg],
        span: Span,
    ) -> Result<Value, Diag> {
        if self.stopping() {
            return Ok(Value::Null);
        }
        let name = match callee {
            Expr::Ident(n, _) => n.clone(),
            other => {
                return Err(self.diag(other.span(), "можно вызывать только именованные функции"))
            }
        };
        let mut values = Vec::with_capacity(args.len());
        let mut names: Vec<Option<String>> = Vec::with_capacity(args.len());
        for a in args {
            values.push(self.eval(&a.value)?);
            names.push(a.name.clone());
        }
        let builtin = match self.ctx.builtins.get(&name) {
            Some(b) => b.clone(),
            None => {
                let def = match self.functions.get(&name) {
                    Some(d) => *d,
                    None => return Err(self.diag(span, format!("неизвестная функция '{name}'"))),
                };
                return self.call_user(&name, def, values, names, span);
            }
        };
        let n_pos = builtin.positional.len();
        let mut slots: Vec<Option<Value>> = vec![None; n_pos];
        let mut next = 0usize;
        for (i, v) in values.into_iter().enumerate() {
            match &names[i] {
                None => {
                    if next >= n_pos {
                        return Err(
                            self.diag(span, format!("у функции '{name}' слишком много аргументов"))
                        );
                    }
                    slots[next] = Some(v);
                    next += 1;
                }
                Some(n) => {
                    let pos = builtin
                        .positional
                        .iter()
                        .position(|p| p == n)
                        .ok_or_else(|| {
                            self.diag(span, format!("у функции '{name}' нет аргумента '{n}'"))
                        })?;
                    if slots[pos].is_some() {
                        return Err(self.diag(
                            span,
                            format!("аргумент '{n}' функции '{name}' задан дважды"),
                        ));
                    }
                    slots[pos] = Some(v);
                }
            }
        }
        let last = slots.iter().rposition(|s| s.is_some());
        let mut passed: Vec<Value> = Vec::new();
        if let Some(last) = last {
            for slot in &slots[..=last] {
                match slot {
                    Some(v) => passed.push(v.clone()),
                    None => passed.push(Value::Null),
                }
            }
        }
        let f = builtin.func.clone();
        let out = f(self, &passed, span);
        if self.stopping() {
            return Ok(Value::Null);
        }
        out
    }

    fn call_user(
        &mut self,
        name: &str,
        def: &'a Stmt,
        values: Vec<Value>,
        names: Vec<Option<String>>,
        span: Span,
    ) -> Result<Value, Diag> {
        let (params, body) = match def {
            Stmt::Func { params, body, .. } => (params, body),
            _ => return Err(self.diag(span, "вызов не функции")),
        };
        if self.depth >= MAX_DEPTH {
            return Err(self.diag(
                span,
                format!("превышена глубина рекурсии ({MAX_DEPTH} вызовов)"),
            ));
        }
        let mut frame: BTreeMap<String, Value> = BTreeMap::new();
        let mut next = 0usize;
        for (i, v) in values.into_iter().enumerate() {
            match &names[i] {
                None => {
                    if next >= params.len() {
                        return Err(
                            self.diag(span, format!("у функции '{name}' слишком много аргументов"))
                        );
                    }
                    frame.insert(params[next].name.clone(), v);
                    next += 1;
                }
                Some(n) => {
                    if !params.iter().any(|p| &p.name == n) {
                        return Err(
                            self.diag(span, format!("у функции '{name}' нет аргумента '{n}'"))
                        );
                    }
                    frame.insert(n.clone(), v);
                }
            }
        }
        for p in params.iter().skip(next) {
            if frame.contains_key(&p.name) {
                continue;
            }
            match &p.default {
                Some(d) => {
                    let v = self.eval(d)?;
                    frame.insert(p.name.clone(), v);
                }
                None => {
                    return Err(self.diag(
                        span,
                        format!("у функции '{name}' не задан аргумент '{}'", p.name),
                    ))
                }
            }
        }
        self.locals.push(frame);
        self.ctx.stack.push(name.to_string());
        self.depth += 1;
        let res = self.exec_block(body);
        self.depth -= 1;
        self.ctx.stack.pop();
        self.locals.pop();
        match res? {
            Flow::Exit => Ok(Value::Null),
            Flow::Return(v) => Ok(v),
            _ => Ok(Value::Null),
        }
    }
}

fn float_of(v: f64) -> Result<Value, String> {
    if v.is_finite() {
        return Ok(Value::Float(v));
    }
    Err("переполнение float: результат не число".to_string())
}

enum Nums {
    Ints(i64, i64),
    Floats(f64, f64),
}

fn nums(l: &Value, r: &Value) -> Result<Nums, String> {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => Ok(Nums::Ints(*a, *b)),
        (Value::Int(a), Value::Float(b)) => Ok(Nums::Floats(*a as f64, *b)),
        (Value::Float(a), Value::Int(b)) => Ok(Nums::Floats(*a, *b as f64)),
        (Value::Float(a), Value::Float(b)) => Ok(Nums::Floats(*a, *b)),
        (a, b) => Err(format!(
            "нужны числа, а не {} и {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn arith_add(l: &Value, r: &Value) -> Result<Value, String> {
    match nums(l, r)? {
        Nums::Ints(a, b) => a
            .checked_add(b)
            .map(Value::Int)
            .ok_or_else(|| "переполнение int".to_string()),
        Nums::Floats(a, b) => float_of(a + b),
    }
}

fn arith_sub(l: &Value, r: &Value) -> Result<Value, String> {
    match nums(l, r)? {
        Nums::Ints(a, b) => a
            .checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| "переполнение int".to_string()),
        Nums::Floats(a, b) => float_of(a - b),
    }
}

fn arith_mul(l: &Value, r: &Value) -> Result<Value, String> {
    match nums(l, r)? {
        Nums::Ints(a, b) => a
            .checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| "переполнение int".to_string()),
        Nums::Floats(a, b) => float_of(a * b),
    }
}

fn arith_div(l: &Value, r: &Value) -> Result<Value, String> {
    let (a, b) = match nums(l, r)? {
        Nums::Ints(a, b) => (a as f64, b as f64),
        Nums::Floats(a, b) => (a, b),
    };
    if b == 0.0 {
        return Err("деление на ноль".to_string());
    }
    float_of(a / b)
}

fn arith_floor_div(l: &Value, r: &Value) -> Result<Value, String> {
    match nums(l, r)? {
        Nums::Ints(a, b) => {
            if b == 0 {
                return Err("деление на ноль".to_string());
            }
            if a == i64::MIN && b == -1 {
                return Err("переполнение int".to_string());
            }
            let q = a / b;
            let rem = a % b;
            let v = if rem != 0 && ((rem < 0) != (b < 0)) {
                q - 1
            } else {
                q
            };
            Ok(Value::Int(v))
        }
        Nums::Floats(a, b) => {
            if b == 0.0 {
                return Err("деление на ноль".to_string());
            }
            let v = (a / b).floor();
            if v < i64::MIN as f64 || v > i64::MAX as f64 {
                return Err("переполнение int".to_string());
            }
            Ok(Value::Int(v as i64))
        }
    }
}

fn arith_mod(l: &Value, r: &Value) -> Result<Value, String> {
    match nums(l, r)? {
        Nums::Ints(a, b) => {
            if b == 0 {
                return Err("остаток от деления на ноль".to_string());
            }
            if a == i64::MIN && b == -1 {
                return Err("переполнение int".to_string());
            }
            let rem = a % b;
            let v = if rem != 0 && ((rem < 0) != (b < 0)) {
                rem + b
            } else {
                rem
            };
            Ok(Value::Int(v))
        }
        Nums::Floats(a, b) => {
            if b == 0.0 {
                return Err("остаток от деления на ноль".to_string());
            }
            float_of(a % b)
        }
    }
}

pub fn handler_vars(upd: &Value, args: Vec<String>) -> BTreeMap<String, Value> {
    let msg = msg_of(upd);
    let mut m = BTreeMap::new();
    m.insert("upd".to_string(), upd.clone());
    m.insert("msg".to_string(), msg.clone());
    m.insert(
        "cb".to_string(),
        field(upd, "callback_query").unwrap_or(Value::Null),
    );
    m.insert("chat".to_string(), chat_of(upd));
    m.insert("user".to_string(), user_of(upd));
    m.insert(
        "args".to_string(),
        Value::list(args.into_iter().map(Value::str).collect()),
    );
    let text = field(&msg, "text").unwrap_or(Value::Null);
    m.insert(
        "text".to_string(),
        match text {
            Value::Str(_) => text,
            _ => Value::str(""),
        },
    );
    m
}

pub fn msg_of(upd: &Value) -> Value {
    if let Some(m) = field(upd, "message") {
        if !matches!(m, Value::Null) {
            return m;
        }
    }
    if let Some(cb) = field(upd, "callback_query") {
        if let Some(m) = field(&cb, "message") {
            if !matches!(m, Value::Null) {
                return m;
            }
        }
    }
    Value::Null
}

pub fn chat_of(upd: &Value) -> Value {
    let msg = msg_of(upd);
    match field(&msg, "chat") {
        Some(c) => field(&c, "id").unwrap_or(Value::Null),
        None => Value::Null,
    }
}

pub fn user_of(upd: &Value) -> Value {
    let msg = msg_of(upd);
    if let Some(u) = field(&msg, "from") {
        if !matches!(u, Value::Null) {
            return u;
        }
    }
    field(&upd.clone(), "callback_query")
        .and_then(|cb| field(&cb, "from"))
        .unwrap_or(Value::Null)
}

pub fn field(base: &Value, name: &str) -> Option<Value> {
    match base {
        Value::Map(m) => m.borrow().get(name).cloned(),
        Value::List(l) => match name {
            "length" => Some(Value::Int(l.borrow().len() as i64)),
            _ => None,
        },
        Value::Str(s) => match name {
            "length" => Some(Value::Int(s.chars().count() as i64)),
            _ => None,
        },
        _ => None,
    }
}

pub fn parse_and_run(src: &str, file: &str, env: &mut dyn EnvCtx) -> Result<Ctx, Diag> {
    let program = parse(src, file)?;
    run_program(&program, file, env)
}

pub fn wall_clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
