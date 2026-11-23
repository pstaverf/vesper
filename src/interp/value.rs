use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub const MAX_VALUE_DEPTH: usize = 100;

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    List(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<BTreeMap<String, Value>>>),
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(Rc::from(s.into()))
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn map(pairs: BTreeMap<String, Value>) -> Value {
        Value::Map(Rc::new(RefCell::new(pairs)))
    }

    pub fn from_pairs(pairs: Vec<(&str, Value)>) -> Value {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert(k.to_string(), v);
        }
        Value::map(m)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Map(_) => "map",
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            Value::Map(m) => !m.borrow().is_empty(),
        }
    }

    pub fn equals(&self, other: &Value) -> bool {
        equal_at(self, other, 0)
    }

    pub fn display(&self) -> String {
        display_at(self, 0)
    }

    pub fn to_json(&self) -> Result<String, String> {
        let mut out = String::new();
        json_write(self, &mut out, 0)?;
        Ok(out)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(v) => f.write_str(&fmt_float(*v)),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::List(_) | Value::Map(_) => match self.to_json() {
                Ok(t) => f.write_str(&t),
                Err(e) => write!(f, "<{e}>"),
            },
        }
    }
}

pub fn too_deep() -> String {
    "цикл или слишком глубокая вложенность".to_string()
}

pub fn cmp_int_float(a: i64, b: f64) -> Option<Ordering> {
    if b.is_nan() {
        return None;
    }
    let hi = i64::MAX as f64;
    if b == b.trunc() && b >= i64::MIN as f64 && b < hi {
        return Some(a.cmp(&(b as i64)));
    }
    (a as f64).partial_cmp(&b)
}

pub fn cmp_float_int(a: f64, b: i64) -> Option<Ordering> {
    cmp_int_float(b, a).map(|o| o.reverse())
}

fn equal_at(a: &Value, b: &Value, depth: usize) -> bool {
    if depth > MAX_VALUE_DEPTH {
        return false;
    }
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => cmp_int_float(*x, *y) == Some(Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => cmp_float_int(*x, *y) == Some(Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            let x = x.borrow();
            let y = y.borrow();
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(p, q)| equal_at(p, q, depth + 1))
        }
        (Value::Map(x), Value::Map(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            let x = x.borrow();
            let y = y.borrow();
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).map(|o| equal_at(v, o, depth + 1)).unwrap_or(false))
        }
        _ => false,
    }
}

fn display_at(v: &Value, depth: usize) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => fmt_float(*f),
        Value::Str(s) => s.to_string(),
        Value::List(_) | Value::Map(_) => {
            if depth > MAX_VALUE_DEPTH {
                return format!("<{}>", too_deep());
            }
            let mut out = String::new();
            match json_write(v, &mut out, depth) {
                Ok(()) => out,
                Err(e) => format!("<{e}>"),
            }
        }
    }
}

pub fn fmt_float(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if v == v.trunc() && v.abs() < 1e16 {
        return format!("{v:.1}");
    }
    format!("{v}")
}

fn json_write(v: &Value, out: &mut String, depth: usize) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(too_deep());
    }
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => {
            if !f.is_finite() {
                return Err("нельзя превратить в JSON: NaN или бесконечность".to_string());
            }
            out.push_str(&fmt_float(*f));
        }
        Value::Str(s) => json_str(s, out),
        Value::List(l) => {
            out.push('[');
            for (i, item) in l.borrow().iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                json_write(item, out, depth + 1)?;
            }
            out.push(']');
        }
        Value::Map(m) => {
            out.push('{');
            for (i, (k, item)) in m.borrow().iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                json_str(k, out);
                out.push(':');
                json_write(item, out, depth + 1)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn parse_json(s: &str) -> Result<Value, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let v = json_value(&chars, &mut i, 0)?;
    json_ws(&chars, &mut i);
    if i != chars.len() {
        return Err(format!("лишние символы после JSON, позиция {}", i + 1));
    }
    Ok(v)
}

fn json_ws(c: &[char], i: &mut usize) {
    while *i < c.len() && matches!(c[*i], ' ' | '\t' | '\n' | '\r') {
        *i += 1;
    }
}

fn json_value(c: &[char], i: &mut usize, depth: usize) -> Result<Value, String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(too_deep());
    }
    json_ws(c, i);
    let ch = match c.get(*i) {
        Some(ch) => *ch,
        None => return Err("неожиданный конец JSON".to_string()),
    };
    match ch {
        '{' => {
            *i += 1;
            let mut m = BTreeMap::new();
            json_ws(c, i);
            if c.get(*i) == Some(&'}') {
                *i += 1;
                return Ok(Value::map(m));
            }
            loop {
                json_ws(c, i);
                let key = match c.get(*i) {
                    Some('"') => json_string(c, i)?,
                    _ => return Err(format!("ожидался ключ в кавычках, позиция {}", *i + 1)),
                };
                json_ws(c, i);
                if c.get(*i) != Some(&':') {
                    return Err(format!("ожидалось ':' после ключа, позиция {}", *i + 1));
                }
                *i += 1;
                let v = json_value(c, i, depth + 1)?;
                m.insert(key, v);
                json_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some('}') => {
                        *i += 1;
                        break;
                    }
                    _ => return Err(format!("ожидалось ',' или '}}', позиция {}", *i + 1)),
                }
            }
            Ok(Value::map(m))
        }
        '[' => {
            *i += 1;
            let mut items = Vec::new();
            json_ws(c, i);
            if c.get(*i) == Some(&']') {
                *i += 1;
                return Ok(Value::list(items));
            }
            loop {
                items.push(json_value(c, i, depth + 1)?);
                json_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some(']') => {
                        *i += 1;
                        break;
                    }
                    _ => return Err(format!("ожидалось ',' или ']', позиция {}", *i + 1)),
                }
            }
            Ok(Value::list(items))
        }
        '"' => Ok(Value::str(json_string(c, i)?)),
        't' => json_word(c, i, "true", Value::Bool(true)),
        'f' => json_word(c, i, "false", Value::Bool(false)),
        'n' => json_word(c, i, "null", Value::Null),
        _ if ch == '-' || ch.is_ascii_digit() => json_number(c, i),
        _ => Err(format!(
            "недопустимый символ '{ch}' в JSON, позиция {}",
            *i + 1
        )),
    }
}

fn json_word(c: &[char], i: &mut usize, word: &str, v: Value) -> Result<Value, String> {
    for w in word.chars() {
        if c.get(*i) != Some(&w) {
            return Err(format!("ожидалось '{word}', позиция {}", *i + 1));
        }
        *i += 1;
    }
    Ok(v)
}

fn json_number(c: &[char], i: &mut usize) -> Result<Value, String> {
    let start = *i;
    let mut text = String::new();
    if c.get(*i) == Some(&'-') {
        text.push('-');
        *i += 1;
    }
    let mut digits = 0;
    while *i < c.len() && c[*i].is_ascii_digit() {
        text.push(c[*i]);
        *i += 1;
        digits += 1;
    }
    if digits == 0 {
        return Err(format!("ожидалась цифра, позиция {}", start + 1));
    }
    let mut is_float = false;
    if c.get(*i) == Some(&'.') {
        is_float = true;
        text.push('.');
        *i += 1;
        let mut frac = 0;
        while *i < c.len() && c[*i].is_ascii_digit() {
            text.push(c[*i]);
            *i += 1;
            frac += 1;
        }
        if frac == 0 {
            return Err(format!("ожидалась цифра после точки, позиция {}", *i + 1));
        }
    }
    if matches!(c.get(*i), Some('e') | Some('E')) {
        is_float = true;
        text.push('e');
        *i += 1;
        if matches!(c.get(*i), Some('+') | Some('-')) {
            text.push(c[*i]);
            *i += 1;
        }
        let mut exp = 0;
        while *i < c.len() && c[*i].is_ascii_digit() {
            text.push(c[*i]);
            *i += 1;
            exp += 1;
        }
        if exp == 0 {
            return Err(format!("ожидалась степень, позиция {}", *i + 1));
        }
    }
    if !is_float {
        if let Ok(v) = text.parse::<i64>() {
            return Ok(Value::Int(v));
        }
    }
    match text.parse::<f64>() {
        Ok(v) => Ok(Value::Float(v)),
        Err(_) => Err(format!("неверное число '{text}'")),
    }
}

fn json_string(c: &[char], i: &mut usize) -> Result<String, String> {
    *i += 1;
    let mut out = String::new();
    loop {
        let ch = match c.get(*i) {
            Some(ch) => *ch,
            None => return Err("незакрытая строка в JSON".to_string()),
        };
        *i += 1;
        match ch {
            '"' => return Ok(out),
            '\\' => {
                let esc = match c.get(*i) {
                    Some(e) => *e,
                    None => return Err("обрыв escape-последовательности в JSON".to_string()),
                };
                *i += 1;
                match esc {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'u' => {
                        let hi = json_hex4(c, i)?;
                        if (0xD800..0xDC00).contains(&hi) {
                            if c.get(*i) == Some(&'\\') && c.get(*i + 1) == Some(&'u') {
                                *i += 2;
                                let lo = json_hex4(c, i)?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    let code = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                                } else {
                                    out.push('\u{FFFD}');
                                }
                            } else {
                                out.push('\u{FFFD}');
                            }
                        } else {
                            out.push(char::from_u32(hi).unwrap_or('\u{FFFD}'));
                        }
                    }
                    other => {
                        return Err(format!(
                            "неизвестная escape-последовательность '\\{other}' в JSON"
                        ))
                    }
                }
            }
            c if (c as u32) < 0x20 => {
                return Err(format!("недопустимый символ в строке JSON, позиция {}", *i))
            }
            c => out.push(c),
        }
    }
}

fn json_hex4(c: &[char], i: &mut usize) -> Result<u32, String> {
    let mut v = 0u32;
    for _ in 0..4 {
        let ch = match c.get(*i) {
            Some(ch) => *ch,
            None => return Err("обрыв \\u в JSON".to_string()),
        };
        let d = match ch.to_digit(16) {
            Some(d) => d,
            None => return Err(format!("неверная hex-цифра '{ch}' в JSON")),
        };
        v = v * 16 + d;
        *i += 1;
    }
    Ok(v)
}
