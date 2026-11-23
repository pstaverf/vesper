use std::collections::BTreeMap;
use std::rc::Rc;

use crate::diag::{Diag, Span};

use super::value::{self, Value};
use super::Run;

pub type BuiltinFunc = dyn Fn(&mut Run<'_>, &[Value], Span) -> Result<Value, Diag>;

#[derive(Clone)]
pub struct Builtin {
    pub positional: Vec<String>,
    pub func: Rc<BuiltinFunc>,
}

impl Builtin {
    pub fn new(
        positional: &[&str],
        func: impl Fn(&mut Run<'_>, &[Value], Span) -> Result<Value, Diag> + 'static,
    ) -> Builtin {
        Builtin {
            positional: positional.iter().map(|s| s.to_string()).collect(),
            func: Rc::new(func),
        }
    }
}

pub fn core_table() -> BTreeMap<String, Builtin> {
    let mut t = BTreeMap::new();
    let mut add = |name: &str, b: Builtin| {
        t.insert(name.to_string(), b);
    };
    add(
        "len",
        Builtin::new(&["value"], |r, a, s| match arg(r, a, 0, "len", s)? {
            Value::Str(x) => Ok(Value::Int(x.chars().count() as i64)),
            Value::List(l) => Ok(Value::Int(l.borrow().len() as i64)),
            Value::Map(m) => Ok(Value::Int(m.borrow().len() as i64)),
            other => Err(r.diag(s, format!("len не работает для типа {}", other.type_name()))),
        }),
    );
    add(
        "str",
        Builtin::new(&["value"], |r, a, s| {
            Ok(Value::str(arg(r, a, 0, "str", s)?.display()))
        }),
    );
    add(
        "int",
        Builtin::new(&["value"], |r, a, s| match arg(r, a, 0, "int", s)? {
            Value::Int(i) => Ok(Value::Int(i)),
            Value::Float(f) => {
                if !f.is_finite() {
                    return Err(r.diag(s, "нельзя превратить NaN или бесконечность в int"));
                }
                Ok(Value::Int(f.trunc() as i64))
            }
            Value::Bool(b) => Ok(Value::Int(if b { 1 } else { 0 })),
            Value::Str(x) => {
                let t = x.trim();
                match t.parse::<i64>() {
                    Ok(v) => Ok(Value::Int(v)),
                    Err(_) => Err(r.diag(s, format!("'{t}' не является целым числом"))),
                }
            }
            Value::Null => Ok(Value::Int(0)),
            other => Err(r.diag(s, format!("int не работает для типа {}", other.type_name()))),
        }),
    );
    add(
        "float",
        Builtin::new(&["value"], |r, a, s| match arg(r, a, 0, "float", s)? {
            Value::Float(f) => Ok(Value::Float(f)),
            Value::Int(i) => Ok(Value::Float(i as f64)),
            Value::Bool(b) => Ok(Value::Float(if b { 1.0 } else { 0.0 })),
            Value::Str(x) => {
                let t = x.trim();
                match t.parse::<f64>() {
                    Ok(v) if v.is_finite() => Ok(Value::Float(v)),
                    _ => Err(r.diag(s, format!("'{t}' не является числом"))),
                }
            }
            Value::Null => Ok(Value::Float(0.0)),
            other => Err(r.diag(
                s,
                format!("float не работает для типа {}", other.type_name()),
            )),
        }),
    );
    add(
        "bool",
        Builtin::new(&["value"], |r, a, s| {
            Ok(Value::Bool(arg(r, a, 0, "bool", s)?.truthy()))
        }),
    );
    add(
        "type",
        Builtin::new(&["value"], |r, a, s| {
            Ok(Value::str(arg(r, a, 0, "type", s)?.type_name()))
        }),
    );
    add(
        "keys",
        Builtin::new(&["map"], |r, a, s| match arg(r, a, 0, "keys", s)? {
            Value::Map(m) => Ok(Value::list(
                m.borrow().keys().map(|k| Value::str(k.clone())).collect(),
            )),
            other => Err(r.diag(
                s,
                format!(
                    "keys работает только для словаря, а не для {}",
                    other.type_name()
                ),
            )),
        }),
    );
    add(
        "values",
        Builtin::new(&["map"], |r, a, s| match arg(r, a, 0, "values", s)? {
            Value::Map(m) => Ok(Value::list(m.borrow().values().cloned().collect())),
            other => Err(r.diag(
                s,
                format!(
                    "values работает только для словаря, а не для {}",
                    other.type_name()
                ),
            )),
        }),
    );
    add(
        "has",
        Builtin::new(&["value", "key"], |r, a, s| {
            let v = arg(r, a, 0, "has", s)?;
            let k = arg(r, a, 1, "has", s)?;
            match v {
                Value::Map(m) => match &k {
                    Value::Str(x) => Ok(Value::Bool(m.borrow().contains_key(x.as_ref()))),
                    other => Err(r.diag(
                        s,
                        format!(
                            "ключ словаря должен быть строкой, а не {}",
                            other.type_name()
                        ),
                    )),
                },
                Value::List(l) => Ok(Value::Bool(l.borrow().iter().any(|x| x.equals(&k)))),
                other => Err(r.diag(
                    s,
                    format!(
                        "has работает для словаря и списка, а не для {}",
                        other.type_name()
                    ),
                )),
            }
        }),
    );
    add(
        "push",
        Builtin::new(&["list", "value"], |r, a, s| {
            let v = arg(r, a, 0, "push", s)?;
            let item = arg(r, a, 1, "push", s)?;
            match &v {
                Value::List(l) => {
                    l.borrow_mut().push(item);
                    Ok(v.clone())
                }
                other => Err(r.diag(
                    s,
                    format!(
                        "push работает только для списка, а не для {}",
                        other.type_name()
                    ),
                )),
            }
        }),
    );
    add(
        "pop",
        Builtin::new(&["list", "index"], |r, a, s| {
            let v = arg(r, a, 0, "pop", s)?;
            let idx = a.get(1).cloned().unwrap_or(Value::Null);
            match v {
                Value::List(l) => {
                    let mut items = l.borrow_mut();
                    if items.is_empty() {
                        return Ok(Value::Null);
                    }
                    let pos = match idx {
                        Value::Null => items.len() - 1,
                        Value::Int(i) => {
                            let real = if i < 0 { items.len() as i64 + i } else { i };
                            if real < 0 || real as usize >= items.len() {
                                return Ok(Value::Null);
                            }
                            real as usize
                        }
                        other => {
                            return Err(r.diag(
                                s,
                                format!("индекс должен быть int, а не {}", other.type_name()),
                            ))
                        }
                    };
                    Ok(items.remove(pos))
                }
                other => Err(r.diag(
                    s,
                    format!(
                        "pop работает только для списка, а не для {}",
                        other.type_name()
                    ),
                )),
            }
        }),
    );
    add(
        "join",
        Builtin::new(&["list", "sep"], |r, a, s| {
            let v = arg(r, a, 0, "join", s)?;
            let sep = a.get(1).cloned().unwrap_or_else(|| Value::str(""));
            let parts = match v {
                Value::List(l) => l.borrow().clone(),
                Value::Str(x) => vec![Value::Str(x)],
                other => {
                    return Err(r.diag(
                        s,
                        format!(
                            "join работает только для списка, а не для {}",
                            other.type_name()
                        ),
                    ))
                }
            };
            let mut out = String::new();
            for (i, p) in parts.iter().enumerate() {
                if i > 0 {
                    out.push_str(&sep.display());
                }
                out.push_str(&p.display());
            }
            Ok(Value::str(out))
        }),
    );
    add(
        "split",
        Builtin::new(&["text", "sep"], |r, a, s| {
            let v = arg(r, a, 0, "split", s)?;
            let sep = a
                .get(1)
                .cloned()
                .unwrap_or_else(|| Value::str(" "))
                .display();
            let text = match v {
                Value::Str(x) => x.to_string(),
                other => {
                    return Err(r.diag(
                        s,
                        format!(
                            "split работает только для строки, а не для {}",
                            other.type_name()
                        ),
                    ))
                }
            };
            if sep.is_empty() {
                return Ok(Value::list(
                    text.chars().map(|c| Value::str(c.to_string())).collect(),
                ));
            }
            Ok(Value::list(text.split(&sep).map(Value::str).collect()))
        }),
    );
    add(
        "trim",
        Builtin::new(&["text"], |r, a, s| match arg(r, a, 0, "trim", s)? {
            Value::Str(x) => Ok(Value::str(x.trim().to_string())),
            other => Err(r.diag(
                s,
                format!(
                    "trim работает только для строки, а не для {}",
                    other.type_name()
                ),
            )),
        }),
    );
    add(
        "lower",
        Builtin::new(&["text"], |r, a, s| match arg(r, a, 0, "lower", s)? {
            Value::Str(x) => Ok(Value::str(x.to_lowercase())),
            other => Err(r.diag(
                s,
                format!(
                    "lower работает только для строки, а не для {}",
                    other.type_name()
                ),
            )),
        }),
    );
    add(
        "upper",
        Builtin::new(&["text"], |r, a, s| match arg(r, a, 0, "upper", s)? {
            Value::Str(x) => Ok(Value::str(x.to_uppercase())),
            other => Err(r.diag(
                s,
                format!(
                    "upper работает только для строки, а не для {}",
                    other.type_name()
                ),
            )),
        }),
    );
    add(
        "replace",
        Builtin::new(&["text", "from", "to"], |r, a, s| {
            let v = arg(r, a, 0, "replace", s)?;
            let from = arg(r, a, 1, "replace", s)?.display();
            let to = arg(r, a, 2, "replace", s)?.display();
            match v {
                Value::Str(x) => {
                    if from.is_empty() {
                        return Err(r.diag(s, "replace: нельзя заменить пустую строку"));
                    }
                    Ok(Value::str(x.replace(&from, &to)))
                }
                other => Err(r.diag(
                    s,
                    format!(
                        "replace работает только для строки, а не для {}",
                        other.type_name()
                    ),
                )),
            }
        }),
    );
    add(
        "find",
        Builtin::new(&["text", "needle"], |r, a, s| {
            let v = arg(r, a, 0, "find", s)?;
            let needle = arg(r, a, 1, "find", s)?;
            match v {
                Value::Str(x) => {
                    let n = needle.display();
                    match x.find(&n) {
                        Some(pos) => Ok(Value::Int(x[..pos].chars().count() as i64)),
                        None => Ok(Value::Int(-1)),
                    }
                }
                Value::List(l) => {
                    let items = l.borrow();
                    Ok(match items.iter().position(|x| x.equals(&needle)) {
                        Some(p) => Value::Int(p as i64),
                        None => Value::Int(-1),
                    })
                }
                other => Err(r.diag(
                    s,
                    format!(
                        "find работает для строки и списка, а не для {}",
                        other.type_name()
                    ),
                )),
            }
        }),
    );
    add(
        "slice",
        Builtin::new(&["value", "start", "end"], |r, a, s| {
            let v = arg(r, a, 0, "slice", s)?;
            let start = a.get(1).cloned().unwrap_or(Value::Int(0));
            let end = a.get(2).cloned().unwrap_or(Value::Null);
            let start = as_int(&start, r, s, "start")?;
            match v {
                Value::Str(x) => {
                    let chars: Vec<char> = x.chars().collect();
                    let (from, to) = clamp(start, &end, chars.len(), r, s)?;
                    Ok(Value::str(chars[from..to].iter().collect::<String>()))
                }
                Value::List(l) => {
                    let items = l.borrow();
                    let (from, to) = clamp(start, &end, items.len(), r, s)?;
                    Ok(Value::list(items[from..to].to_vec()))
                }
                other => Err(r.diag(
                    s,
                    format!(
                        "slice работает для строки и списка, а не для {}",
                        other.type_name()
                    ),
                )),
            }
        }),
    );
    add(
        "range",
        Builtin::new(&["from", "to", "step"], |r, a, s| {
            let first = arg(r, a, 0, "range", s)?;
            let second = a.get(1).cloned().unwrap_or(Value::Null);
            let step = as_int(&a.get(2).cloned().unwrap_or(Value::Int(1)), r, s, "step")?;
            if step == 0 {
                return Err(r.diag(s, "range: шаг не может быть 0"));
            }
            let (from, to) = match second {
                Value::Null => (0, as_int(&first, r, s, "to")?),
                other => (as_int(&first, r, s, "from")?, as_int(&other, r, s, "to")?),
            };
            let mut out = Vec::new();
            let mut cur = from;
            while (step > 0 && cur < to) || (step < 0 && cur > to) {
                out.push(Value::Int(cur));
                match cur.checked_add(step) {
                    Some(v) => cur = v,
                    None => break,
                }
                if out.len() > 1_000_000 {
                    return Err(r.diag(s, "range: больше 1 000 000 элементов"));
                }
            }
            Ok(Value::list(out))
        }),
    );
    add(
        "random",
        Builtin::new(&["value"], |r, a, _s| {
            let v = a.first().cloned().unwrap_or(Value::Null);
            match v {
                Value::Null => Ok(Value::Float(r.env.rand())),
                Value::Int(n) => {
                    if n <= 0 {
                        return Ok(Value::Int(0));
                    }
                    let f = r.env.rand();
                    Ok(Value::Int(((f * n as f64) as i64).clamp(0, n - 1)))
                }
                Value::List(l) => {
                    let items = l.borrow();
                    if items.is_empty() {
                        return Ok(Value::Null);
                    }
                    let f = r.env.rand();
                    let idx = ((f * items.len() as f64) as usize).min(items.len() - 1);
                    Ok(items[idx].clone())
                }
                Value::Str(x) => {
                    let chars: Vec<char> = x.chars().collect();
                    if chars.is_empty() {
                        return Ok(Value::str(""));
                    }
                    let f = r.env.rand();
                    let idx = ((f * chars.len() as f64) as usize).min(chars.len() - 1);
                    Ok(Value::str(chars[idx].to_string()))
                }
                other => Err(r.diag(
                    _s,
                    format!("random не работает для типа {}", other.type_name()),
                )),
            }
        }),
    );
    add(
        "now",
        Builtin::new(&[], |r, _a, _s| Ok(Value::Int(r.env.now()))),
    );
    add(
        "json",
        Builtin::new(&["value"], |r, a, s| {
            match arg(r, a, 0, "json", s)?.to_json() {
                Ok(t) => Ok(Value::str(t)),
                Err(e) => Err(r.diag(s, e)),
            }
        }),
    );
    add(
        "parse_json",
        Builtin::new(&["text"], |r, a, s| match arg(r, a, 0, "parse_json", s)? {
            Value::Str(x) => match value::parse_json(x.as_ref()) {
                Ok(v) => Ok(v),
                Err(e) => Err(r.diag(s, format!("неверный JSON: {e}"))),
            },
            other => Err(r.diag(
                s,
                format!(
                    "parse_json работает только для строки, а не для {}",
                    other.type_name()
                ),
            )),
        }),
    );
    add(
        "log",
        Builtin::new(&["value"], |r, a, s| {
            let line = arg(r, a, 0, "log", s)?.display();
            r.ctx.logs.push(line.clone());
            Ok(Value::str(line))
        }),
    );
    add(
        "exit",
        Builtin::new(&["code"], |r, a, s| {
            let code = match a.first().cloned().unwrap_or(Value::Null) {
                Value::Null => 0,
                Value::Int(i) => i,
                Value::Float(f) => f as i64,
                Value::Str(x) => x.trim().parse::<i64>().unwrap_or(1),
                other => return Err(r.diag(s, format!("exit: неверный код {}", other.display()))),
            };
            r.ctx.exit = Some(code);
            Ok(Value::Null)
        }),
    );
    t
}

pub fn arg(r: &Run<'_>, a: &[Value], idx: usize, fname: &str, span: Span) -> Result<Value, Diag> {
    match a.get(idx) {
        Some(v) => Ok(v.clone()),
        None => Err(r.diag(span, format!("функции {fname} нужен аргумент #{}", idx + 1))),
    }
}

fn as_int(v: &Value, r: &Run<'_>, s: Span, what: &str) -> Result<i64, Diag> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Float(f) if f.is_finite() => Ok(*f as i64),
        other => Err(r.diag(
            s,
            format!("{what} должен быть числом, а не {}", other.type_name()),
        )),
    }
}

fn clamp(
    start: i64,
    end: &Value,
    len: usize,
    r: &Run<'_>,
    s: Span,
) -> Result<(usize, usize), Diag> {
    let fix = |i: i64| -> usize {
        let v = if i < 0 { len as i64 + i } else { i };
        v.clamp(0, len as i64) as usize
    };
    let from = fix(start);
    let to = match end {
        Value::Null => len,
        other => fix(as_int(other, r, s, "end")?),
    };
    if from > to {
        return Ok((from, from));
    }
    Ok((from, to))
}
