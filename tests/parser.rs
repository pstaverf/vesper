use vesper::parser::ast::{Expr, Stmt, StrPart, Trigger};
use vesper::parser::parse;

const EXAMPLES: [(&str, &str); 5] = [
    (
        "examples/01_start.vx",
        include_str!("../examples/01_start.vx"),
    ),
    (
        "examples/02_buttons.vx",
        include_str!("../examples/02_buttons.vx"),
    ),
    (
        "examples/03_states.vx",
        include_str!("../examples/03_states.vx"),
    ),
    (
        "examples/04_callback_edit.vx",
        include_str!("../examples/04_callback_edit.vx"),
    ),
    (
        "examples/05_photo.vx",
        include_str!("../examples/05_photo.vx"),
    ),
];

fn parse_ok(name: &str, src: &str) -> vesper::parser::ast::Program {
    match parse(src, name) {
        Ok(p) => p,
        Err(d) => panic!("{name}: {}\n{}", d.text, d.render(src)),
    }
}

fn parse_err(name: &str, src: &str) -> String {
    match parse(src, name) {
        Ok(_) => panic!("{name}: ожидалась ошибка"),
        Err(d) => d.render(src),
    }
}

fn handlers(p: &vesper::parser::ast::Program) -> Vec<&Trigger> {
    p.stmts
        .iter()
        .filter_map(|s| match s {
            Stmt::Handler(h) => Some(&h.trigger),
            _ => None,
        })
        .collect()
}

fn string_literals(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Str { parts, .. } => {
            for p in parts {
                match p {
                    StrPart::Text(s) => out.push(s.clone()),
                    StrPart::Expr(x) => string_literals(x, out),
                }
            }
        }
        Expr::List(items, _) => items.iter().for_each(|i| string_literals(i, out)),
        Expr::Dict(pairs, _) => pairs.iter().for_each(|(_, v)| string_literals(v, out)),
        Expr::InlineKb(rows, _) => rows.iter().flatten().for_each(|b| {
            out.push(b.text.clone());
            if let vesper::parser::ast::ButtonAction::Link(x) = &b.action {
                string_literals(x, out);
            }
            if let vesper::parser::ast::ButtonAction::Switch(x) = &b.action {
                string_literals(x, out);
            }
        }),
        Expr::MenuKb(rows, _) => out.extend(rows.iter().flatten().cloned()),
        Expr::Unary { expr, .. } => string_literals(expr, out),
        Expr::Binary { lhs, rhs, .. } => {
            string_literals(lhs, out);
            string_literals(rhs, out);
        }
        Expr::Field { obj, .. } => string_literals(obj, out),
        Expr::Index { obj, index, .. } => {
            string_literals(obj, out);
            string_literals(index, out);
        }
        Expr::Call { callee, args, .. } => {
            string_literals(callee, out);
            args.iter().for_each(|a| string_literals(&a.value, out));
        }
        _ => {}
    }
}

#[test]
fn все_примеры_разбираются() {
    for (name, src) in EXAMPLES {
        let p = parse_ok(name, src);
        assert!(!p.stmts.is_empty(), "{name}: пустая программа");
        let mut literals = Vec::new();
        for s in &p.stmts {
            if let Stmt::Handler(h) = s {
                for st in &h.body.stmts {
                    if let Stmt::Expr { expr, .. } = st {
                        string_literals(expr, &mut literals);
                    }
                    if let Stmt::Assign { value, .. } = st {
                        string_literals(value, &mut literals);
                    }
                }
            }
        }
        assert!(!literals.is_empty(), "{name}: не нашлось ни одной строки");
    }
}

#[test]
fn пример_старта_видит_команду_и_текст() {
    let p = parse_ok(EXAMPLES[0].0, EXAMPLES[0].1);
    assert_eq!(
        handlers(&p),
        vec![&Trigger::Command("start".into()), &Trigger::Text]
    );
    assert!(matches!(p.stmts[0], Stmt::Bot { .. }));
}

#[test]
fn пример_кнопок_видит_inline_и_callback() {
    let p = parse_ok(EXAMPLES[1].0, EXAMPLES[1].1);
    let cb: Vec<&Trigger> = handlers(&p)
        .into_iter()
        .filter(|t| matches!(t, Trigger::Callback(_)))
        .collect();
    assert_eq!(cb.len(), 2);
    assert_eq!(cb[0], &Trigger::Callback("buy:coffee".into()));
}

#[test]
fn пример_состояний_видит_два_блока_state() {
    let p = parse_ok(EXAMPLES[2].0, EXAMPLES[2].1);
    let states: Vec<&str> = p
        .stmts
        .iter()
        .filter_map(|s| match s {
            Stmt::State { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(states, vec!["name", "city"]);
}

#[test]
fn пример_callback_редактирует_сообщение() {
    let p = parse_ok(EXAMPLES[3].0, EXAMPLES[3].1);
    assert_eq!(handlers(&p)[1], &Trigger::Callback("inc".into()));
}

#[test]
fn пример_фото_видит_обработчики_вложений() {
    let p = parse_ok(EXAMPLES[4].0, EXAMPLES[4].1);
    let triggers = handlers(&p);
    assert!(triggers.contains(&&Trigger::Photo));
    assert!(triggers.contains(&&Trigger::Document));
}

#[test]
fn битый_пример_нет_двоеточия() {
    let out = parse_err("bad.vx", "on command \"start\"\n    send(\"hi\")\n");
    assert!(
        out.contains("bad.vx:1:19: ошибка: ожидалось ':' после заголовка обработчик on"),
        "{out}"
    );
}

#[test]
fn битый_пример_пустой_блок() {
    let out = parse_err("bad.vx", "on text:\n\non any:\n    send(\"x\")\n");
    assert!(out.contains("тело блока"), "{out}");
}

#[test]
fn битый_пример_незакрытый_inline() {
    let out = parse_err(
        "bad.vx",
        "on text:\n    send(\"x\", inline = inline[[\"A\" -> \"a\"]]\n",
    );
    assert!(
        out.contains("незакрытая скобка") || out.contains("не закрывает"),
        "{out}"
    );
}

#[test]
fn битый_пример_state_внутри_обработчика() {
    let out = parse_err(
        "bad.vx",
        "on text:\n    state \"x\":\n        on text:\n            send(\"y\")\n",
    );
    assert!(out.contains("только на верхнем уровне"), "{out}");
}

#[test]
fn crlf_не_ломает_разбор() {
    for (name, src) in EXAMPLES {
        let a = parse_ok(name, src);
        let b = parse_ok(name, &src.replace('\n', "\r\n"));
        assert_eq!(a.stmts.len(), b.stmts.len(), "{name}");
    }
}

#[test]
fn парсер_не_паникует_на_мусоре() {
    let inputs = [
        "",
        "\n\n",
        ":",
        "on",
        "on\n",
        "bot",
        "if",
        "x =",
        "x = (",
        "x = [1,",
        "x = {\"a\"",
        "x = inline[",
        "x = menu[]",
        "state \"a\":",
        "func",
        "func f(",
        "for in",
        "while",
        "return",
        "    ",
        "on text:\n    x = 1\n  y = 2\n",
        "x = \"{a\"\n",
        "x = - - -\n",
        "x = a ?? ?? b\n",
    ];
    for src in inputs {
        let _ = parse(src, "fuzz.vx");
    }
}
