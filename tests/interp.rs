use std::collections::BTreeMap;

use vesper::interp::builtins::Builtin;
use vesper::interp::value::Value;
use vesper::interp::{handler_table, run_handler, run_top, Ctx, FixedEnv};

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

fn stub_ctx(file: &str) -> Ctx {
    let mut ctx = Ctx::new(file);
    let log = [
        "send",
        "reply",
        "edit",
        "delete",
        "forward",
        "send_photo",
        "send_document",
        "answer",
        "typing",
    ];
    for name in log {
        ctx.register(
            name,
            Builtin::new(
                &[
                    "text",
                    "photo",
                    "document",
                    "caption",
                    "inline",
                    "menu",
                    "parse_mode",
                    "chat",
                    "id",
                    "file",
                ],
                |r, a, _s| {
                    let all: Vec<String> = a.iter().map(|v| v.display()).collect();
                    let text = all.join(" ");
                    r.ctx.logs.push(text.clone());
                    Ok(Value::from_pairs(vec![
                        ("message_id", Value::Int(1)),
                        ("text", Value::str(text)),
                    ]))
                },
            ),
        );
    }
    ctx.register(
        "save",
        Builtin::new(&["key", "value"], |r, a, _s| {
            let k = a.first().cloned().unwrap_or(Value::Null).display();
            let v = a.get(1).cloned().unwrap_or(Value::Null);
            let store = r
                .ctx
                .get_global("__store")
                .unwrap_or_else(|| Value::map(BTreeMap::new()));
            if let Value::Map(m) = &store {
                m.borrow_mut().insert(k, v);
            }
            r.ctx.set_global("__store", store);
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "load",
        Builtin::new(&["key"], |r, a, _s| {
            let k = a.first().cloned().unwrap_or(Value::Null).display();
            let store = r.ctx.get_global("__store").unwrap_or(Value::Null);
            match &store {
                Value::Map(m) => Ok(m.borrow().get(&k).cloned().unwrap_or(Value::Null)),
                _ => Ok(Value::Null),
            }
        }),
    );
    ctx.register(
        "forget",
        Builtin::new(&["key"], |r, a, _s| {
            let k = a.first().cloned().unwrap_or(Value::Null).display();
            let store = r.ctx.get_global("__store").unwrap_or(Value::Null);
            if let Value::Map(m) = &store {
                m.borrow_mut().remove(&k);
            }
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "set_state",
        Builtin::new(&["state"], |r, a, _s| {
            r.ctx
                .set_global("__state", a.first().cloned().unwrap_or(Value::Null));
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "get_state",
        Builtin::new(&[], |r, _a, _s| {
            Ok(r.ctx.get_global("__state").unwrap_or(Value::Null))
        }),
    );
    ctx
}

fn update(text: &str) -> Value {
    Value::from_pairs(vec![
        (
            "message",
            Value::from_pairs(vec![
                ("text", Value::str(text)),
                (
                    "from",
                    Value::from_pairs(vec![
                        ("id", Value::Int(42)),
                        ("first_name", Value::str("Аня")),
                    ]),
                ),
                ("chat", Value::from_pairs(vec![("id", Value::Int(7))])),
                (
                    "photo",
                    Value::list(vec![
                        Value::from_pairs(vec![("file_id", Value::str("small"))]),
                        Value::from_pairs(vec![("file_id", Value::str("big"))]),
                    ]),
                ),
                (
                    "document",
                    Value::from_pairs(vec![
                        ("file_id", Value::str("doc1")),
                        ("file_name", Value::str("a.pdf")),
                    ]),
                ),
            ]),
        ),
        (
            "callback_query",
            Value::from_pairs(vec![
                ("data", Value::str("inc")),
                (
                    "message",
                    Value::from_pairs(vec![("message_id", Value::Int(3))]),
                ),
            ]),
        ),
    ])
}

fn run_all_handlers(name: &str, src: &str) -> Ctx {
    let program = match vesper::parser::parse(src, name) {
        Ok(p) => p,
        Err(d) => panic!("{name}: {}\n{}", d.text, d.render(src)),
    };
    let mut ctx = stub_ctx(name);
    let mut e = FixedEnv::new(1_700_000_000, vec![0.5]);
    run_top(&mut ctx, &program, &mut e).expect("инициализация");
    for (_, handler) in handler_table(&program) {
        let upd = update("Аня");
        let vars = vesper::interp::handler_vars(&upd, Vec::new());
        if let Err(d) = run_handler(&mut ctx, &program, &handler, vars, &mut e) {
            panic!("{name}: {}\n{}", d.text, d.render(src));
        }
    }
    ctx
}

#[test]
fn пример_1_работает_на_заглушках() {
    let ctx = run_all_handlers(EXAMPLES[0].0, EXAMPLES[0].1);
    assert!(ctx
        .logs
        .contains(&"Привет, Аня! Я бот на Vesper.".to_string()));
    assert!(ctx.logs.iter().any(|l| l.starts_with("Ты написал: ")));
}

#[test]
fn пример_2_работает_на_заглушках() {
    let ctx = run_all_handlers(EXAMPLES[1].0, EXAMPLES[1].1);
    assert!(ctx.logs.iter().any(|l| l.contains("Что берём?")));
    assert!(ctx.logs.iter().any(|l| l.contains("Добавлено")));
    assert!(ctx.logs.iter().any(|l| l.contains("₸")));
}

#[test]
fn пример_3_работает_на_заглушках() {
    let ctx = run_all_handlers(EXAMPLES[2].0, EXAMPLES[2].1);
    assert!(ctx.logs.iter().any(|l| l.contains("Как тебя зовут?")));
    assert!(ctx
        .logs
        .iter()
        .any(|l| l.contains("Имя:") || l.contains("Профиля нет")));
    assert!(ctx
        .logs
        .iter()
        .any(|l| l.contains("Сейчас спрашиваю") || l.contains("Сейчас ничего не запрашиваю")));
}

#[test]
fn пример_3_запоминает_состояние_и_данные() {
    let src = EXAMPLES[2].1;
    let program = vesper::parser::parse(src, EXAMPLES[2].0).expect("парсится");
    let mut ctx = stub_ctx(EXAMPLES[2].0);
    let mut e = FixedEnv::new(1_700_000_000, vec![0.5]);
    let table = handler_table(&program);
    let start = table
        .iter()
        .find(|(_, h)| matches!(h.trigger, vesper::parser::ast::Trigger::Command(ref c) if c == "start"))
        .expect("start");
    let upd = update("/start");
    let vars = vesper::interp::handler_vars(&upd, vec!["start".to_string()]);
    run_handler(&mut ctx, &program, &start.1, vars, &mut e).expect("start");
    assert_eq!(
        ctx.get_global("__state").map(|v| v.display()),
        Some("name".to_string())
    );
    let name_state = table
        .iter()
        .find(|(s, h)| {
            s.as_deref() == Some("name") && matches!(h.trigger, vesper::parser::ast::Trigger::Text)
        })
        .expect("state name");
    let upd = update("Аня");
    let vars = vesper::interp::handler_vars(&upd, Vec::new());
    run_handler(&mut ctx, &program, &name_state.1, vars, &mut e).expect("name");
    let store = ctx.get_global("__store").expect("store");
    assert!(store.display().contains("\"name\":\"Аня\""));
}

#[test]
fn пример_4_работает_на_заглушках() {
    let ctx = run_all_handlers(EXAMPLES[3].0, EXAMPLES[3].1);
    assert!(ctx.logs.iter().any(|l| l.contains("Нажми кнопку")));
    assert!(ctx.logs.iter().any(|l| l.contains("Нажато раз:")));
}

#[test]
fn пример_5_работает_на_заглушках() {
    let ctx = run_all_handlers(EXAMPLES[4].0, EXAMPLES[4].1);
    assert!(ctx.logs.iter().any(|l| l.contains("big")));
    assert!(ctx.logs.iter().any(|l| l.contains("a.pdf")));
    assert!(ctx
        .logs
        .iter()
        .any(|l| l.contains("Логотип") || l.contains("Твоё фото")));
}

#[test]
fn handler_vars_содержит_нужные_имена() {
    let upd = update("привет");
    let vars = vesper::interp::handler_vars(&upd, vec!["a".to_string()]);
    let mut keys: Vec<&str> = vars.keys().map(|k| k.as_str()).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["args", "cb", "chat", "msg", "text", "upd", "user"]
    );
    assert_eq!(vars["chat"].display(), "7");
    assert_eq!(vars["text"].display(), "привет");
    assert_eq!(vars["args"].display(), "[\"a\"]");
}

#[test]
fn state_в_условии_через_get_state() {
    let src = concat!(
        "on text:\n",
        "    if get_state() == null:\n        send(\"нет состояния\")\n",
        "    else:\n        send(\"есть\")\n",
    );
    let program = vesper::parser::parse(src, "t.vx").expect("парсится");
    let mut ctx = stub_ctx("t.vx");
    let mut e = FixedEnv::new(0, vec![]);
    let table = handler_table(&program);
    let upd = update("x");
    let vars = vesper::interp::handler_vars(&upd, Vec::new());
    run_handler(&mut ctx, &program, &table[0].1, vars, &mut e).expect("ок");
    assert_eq!(ctx.logs, vec!["нет состояния".to_string()]);
}

#[test]
fn run_top_выполняет_верхний_уровень() {
    let src = "prices = {\"coffee\": 250}\non text:\n    send(\"x\")\n";
    let program = vesper::parser::parse(src, "t.vx").expect("парсится");
    let mut ctx = Ctx::new("t.vx");
    let mut e = FixedEnv::new(0, vec![]);
    run_top(&mut ctx, &program, &mut e).expect("ок");
    assert_eq!(
        ctx.get_global("prices").map(|v| v.display()),
        Some("{\"coffee\":250}".to_string())
    );
}

#[test]
fn exit_останавливает_верхний_уровень() {
    let src = "a = 1\nexit(2)\na = 3\n";
    let program = vesper::parser::parse(src, "t.vx").expect("парсится");
    let mut ctx = Ctx::new("t.vx");
    let mut e = FixedEnv::new(0, vec![]);
    run_top(&mut ctx, &program, &mut e).expect("ок");
    assert_eq!(ctx.exit, Some(2));
    assert_eq!(
        ctx.get_global("a").map(|v| v.display()),
        Some("1".to_string())
    );
}
