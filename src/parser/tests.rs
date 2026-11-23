use super::ast::*;
use super::parse;
use crate::diag::Diag;

fn ok(src: &str) -> Program {
    match parse(src, "t.vx") {
        Ok(p) => p,
        Err(d) => panic!("должно разбираться: {}\n{}", d.text, d.render(src)),
    }
}

fn bad(src: &str) -> Diag {
    match parse(src, "t.vx") {
        Ok(_) => panic!("ожидалась синтаксическая ошибка:\n{src}"),
        Err(d) => d,
    }
}

fn first_expr(src: &str) -> Expr {
    let p = ok(src);
    match p.stmts.into_iter().next() {
        Some(Stmt::Expr { expr, .. }) => expr,
        other => panic!("ожидалось выражение, получено {other:?}"),
    }
}

fn shape(e: &Expr) -> String {
    match e {
        Expr::Null(_) => "null".into(),
        Expr::Bool(v, _) => format!("{v}"),
        Expr::Int(v, _) => format!("{v}"),
        Expr::Float(v, _) => format!("{v}"),
        Expr::Ident(n, _) => n.clone(),
        Expr::Str { .. } => "str".into(),
        Expr::List(items, _) => {
            let inner: Vec<String> = items.iter().map(shape).collect();
            format!("[{}]", inner.join(", "))
        }
        Expr::Dict(pairs, _) => {
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{k}: {}", shape(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Expr::InlineKb(rows, _) => format!("inline({} строк)", rows.len()),
        Expr::MenuKb(rows, _) => format!("menu({} строк)", rows.len()),
        Expr::Unary { op, expr, .. } => {
            let o = match op {
                UnOp::Neg => "-",
                UnOp::Not => "not ",
            };
            format!("({o}{})", shape(expr))
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::FloorDiv => "//",
                BinOp::Mod => "%",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
                BinOp::In => "in",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Coalesce => "??",
                BinOp::And => "and",
                BinOp::Or => "or",
            };
            format!("({} {o} {})", shape(lhs), shape(rhs))
        }
        Expr::Field { obj, name, .. } => format!("{}.{name}", shape(obj)),
        Expr::Index { obj, index, .. } => format!("{}[{}]", shape(obj), shape(index)),
        Expr::Call { callee, args, .. } => {
            let inner: Vec<String> = args
                .iter()
                .map(|a| match &a.name {
                    Some(n) => format!("{n}= {}", shape(&a.value)),
                    None => shape(&a.value),
                })
                .collect();
            format!("{}({})", shape(callee), inner.join(", "))
        }
    }
}

fn expr_shape(src: &str) -> String {
    shape(&first_expr(src))
}

#[test]
fn пустая_программа() {
    assert!(ok("").stmts.is_empty());
    assert!(ok("\n\n# только комментарий\n").stmts.is_empty());
}

#[test]
fn объявление_бота() {
    let p = ok("bot \"Магазин\"\n");
    assert_eq!(
        p.stmts,
        vec![Stmt::Bot {
            name: "Магазин".into(),
            span: crate::diag::Span::new(1, 1, 13)
        }]
    );
}

#[test]
fn приоритет_арифметики() {
    assert_eq!(expr_shape("1 + 2 * 3"), "(1 + (2 * 3))");
    assert_eq!(expr_shape("(1 + 2) * 3"), "((1 + 2) * 3)");
    assert_eq!(expr_shape("1 - 2 - 3"), "((1 - 2) - 3)");
    assert_eq!(expr_shape("8 // 2 % 3"), "((8 // 2) % 3)");
    assert_eq!(expr_shape("-a * 2"), "((-a) * 2)");
}

#[test]
fn приоритет_coalesce_между_equality_и_and() {
    assert_eq!(expr_shape("a ?? b == c"), "(a ?? (b == c))");
    assert_eq!(expr_shape("a == b ?? c"), "((a == b) ?? c)");
    assert_eq!(expr_shape("a and b ?? c"), "(a and (b ?? c))");
    assert_eq!(expr_shape("a ?? b and c"), "((a ?? b) and c)");
    assert_eq!(expr_shape("a or b and c"), "(a or (b and c))");
    assert_eq!(expr_shape("a ?? b ?? c"), "((a ?? b) ?? c)");
}

#[test]
fn приоритет_сравнений() {
    assert_eq!(expr_shape("a + 1 < b * 2"), "((a + 1) < (b * 2))");
    assert_eq!(expr_shape("a < b == c"), "((a < b) == c)");
    assert_eq!(expr_shape("x in ys and not z"), "((x in ys) and (not z))");
    assert_eq!(expr_shape("a != b != c"), "((a != b) != c)");
}

#[test]
fn цепочка_полей_и_индексов() {
    assert_eq!(expr_shape("msg.from.first_name"), "msg.from.first_name");
    assert_eq!(
        expr_shape("msg.photo[len(msg.photo) - 1].file_id"),
        "msg.photo[(len(msg.photo) - 1)].file_id"
    );
    assert_eq!(expr_shape("m[\"a\"][0]"), "m[str][0]");
}

#[test]
fn вызовы_с_именованными_аргументами_и_ключевыми_словами() {
    assert_eq!(expr_shape("send(\"x\")"), "send(str)");
    assert_eq!(
        expr_shape("send(\"x\", inline = kb)"),
        "send(str, inline= kb)"
    );
    assert_eq!(
        expr_shape("send(\"x\", menu = menu[[\"Да\", \"Нет\"]])"),
        "send(str, menu= menu(1 строк))"
    );
    assert_eq!(
        expr_shape("inline[[\"Сайт\" -> link \"https://a.kz\"]]"),
        "inline(1 строк)"
    );
    assert_eq!(expr_shape("send(\"x\",)"), "send(str)");
}

#[test]
fn списки_словари_и_вложенность() {
    assert_eq!(expr_shape("[]"), "[]");
    assert_eq!(expr_shape("[1, 2, 3,]"), "[1, 2, 3]");
    assert_eq!(expr_shape("{}"), "{}");
    assert_eq!(
        expr_shape("{\"chat_id\": chat, n: 1}"),
        "{chat_id: chat, n: 1}"
    );
    assert_eq!(expr_shape("[[1], [2, [3]]]"), "[[1], [2, [3]]]");
}

#[test]
fn подстановка_в_строке_становится_выражением() {
    let e = first_expr("send(\"Привет, {msg.from.first_name}!\")\n");
    match e {
        Expr::Call { args, .. } => match &args[0].value {
            Expr::Str { parts, .. } => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], StrPart::Text("Привет, ".into()));
                match &parts[1] {
                    StrPart::Expr(x) => assert_eq!(shape(x), "msg.from.first_name"),
                    other => panic!("ожидалось выражение, получено {other:?}"),
                }
                assert_eq!(parts[2], StrPart::Text("!".into()));
            }
            other => panic!("ожидалась строка, получено {other:?}"),
        },
        other => panic!("ожидался вызов, получено {other:?}"),
    }
}

#[test]
fn сложное_выражение_в_подстановке() {
    let e = first_expr("\"{a + b * 2}\"");
    match e {
        Expr::Str { parts, .. } => match &parts[0] {
            StrPart::Expr(x) => assert_eq!(shape(x), "(a + (b * 2))"),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn присваивание_и_цепочки() {
    let p = ok("n = 1\nx = n + 1\n");
    assert_eq!(p.stmts.len(), 2);
    match &p.stmts[0] {
        Stmt::Assign { target, value, .. } => {
            assert_eq!(shape(target), "n");
            assert_eq!(shape(value), "1");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn присваивание_полю_и_индексу() {
    let p = ok("msg.text = \"x\"\nmsg.photo[0].file_id = \"y\"\n");
    assert_eq!(p.stmts.len(), 2);
    assert!(matches!(p.stmts[0], Stmt::Assign { .. }));
    assert!(matches!(p.stmts[1], Stmt::Assign { .. }));
}

#[test]
fn присваивание_в_выражение_запрещено() {
    let d = bad("1 = 2\n");
    assert!(d.text.contains("слева от '='"), "{}", d.text);
}

#[test]
fn if_elif_else() {
    let p = ok("if a:\n    x = 1\nelif b:\n    x = 2\nelse:\n    x = 3\n");
    match &p.stmts[0] {
        Stmt::If { arms, orelse, .. } => {
            assert_eq!(arms.len(), 2);
            assert!(orelse.is_some());
            assert_eq!(arms[0].1.stmts.len(), 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn while_for_break_continue() {
    let p = ok("while n > 0:\n    n = n - 1\n    if n == 1:\n        break\n    continue\n");
    match &p.stmts[0] {
        Stmt::While { body, .. } => assert_eq!(body.stmts.len(), 3),
        other => panic!("{other:?}"),
    }
    let p = ok("for user in users:\n    send(\"ok\")\n");
    match &p.stmts[0] {
        Stmt::For { var, iter, .. } => {
            assert_eq!(var, "user");
            assert_eq!(shape(iter), "users");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn функции_с_параметрами_по_умолчанию() {
    let p = ok("func greet(name, punct = \"!\"):\n    return \"Привет, \" + name + punct\n");
    match &p.stmts[0] {
        Stmt::Func {
            name, params, body, ..
        } => {
            assert_eq!(name, "greet");
            assert_eq!(params.len(), 2);
            assert!(params[1].default.is_some());
            assert!(matches!(body.stmts[0], Stmt::Return { value: Some(_), .. }));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn функция_без_параметров_и_с_пустыми_скобками() {
    assert!(matches!(
        ok("func ping:\n    return 1\n").stmts[0],
        Stmt::Func { .. }
    ));
    assert!(matches!(
        ok("func ping():\n    return 1\n").stmts[0],
        Stmt::Func { .. }
    ));
}

#[test]
fn return_без_значения() {
    let p = ok("func f():\n    return\n");
    match &p.stmts[0] {
        Stmt::Func { body, .. } => assert_eq!(
            body.stmts[0],
            Stmt::Return {
                value: None,
                span: crate::diag::Span::new(2, 5, 6)
            }
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn обработчики_всех_видов() {
    let src = concat!(
        "on command \"start\":\n    send(\"a\")\n",
        "on text:\n    send(\"b\")\n",
        "on photo:\n    send(\"c\")\n",
        "on document:\n    send(\"d\")\n",
        "on video:\n    send(\"e\")\n",
        "on audio:\n    send(\"f\")\n",
        "on voice:\n    send(\"g\")\n",
        "on sticker:\n    send(\"h\")\n",
        "on animation:\n    send(\"i\")\n",
        "on location:\n    send(\"j\")\n",
        "on contact:\n    send(\"k\")\n",
        "on poll:\n    send(\"l\")\n",
        "on inline:\n    send(\"m\")\n",
        "on pre_checkout:\n    send(\"n\")\n",
        "on payment:\n    send(\"o\")\n",
        "on any:\n    send(\"p\")\n",
        "on callback:\n    send(\"q\")\n",
        "on callback \"buy\":\n    send(\"r\")\n",
    );
    let p = ok(src);
    let triggers: Vec<Trigger> = p
        .stmts
        .iter()
        .map(|s| match s {
            Stmt::Handler(h) => h.trigger.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        triggers,
        vec![
            Trigger::Command("start".into()),
            Trigger::Text,
            Trigger::Photo,
            Trigger::Document,
            Trigger::Video,
            Trigger::Audio,
            Trigger::Voice,
            Trigger::Sticker,
            Trigger::Animation,
            Trigger::Location,
            Trigger::Contact,
            Trigger::Poll,
            Trigger::Inline,
            Trigger::PreCheckout,
            Trigger::Payment,
            Trigger::Any,
            Trigger::CallbackAny,
            Trigger::Callback("buy".into()),
        ]
    );
}

#[test]
fn состояния_с_обработчиками() {
    let src = "state \"name\":\n    on text:\n        save(\"n\", text)\n        set_state(null)\n";
    let p = ok(src);
    match &p.stmts[0] {
        Stmt::State { name, handlers, .. } => {
            assert_eq!(name, "name");
            assert_eq!(handlers.len(), 1);
            assert_eq!(handlers[0].body.stmts.len(), 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn get_state_это_обычный_вызов() {
    let src = "on text:\n    if get_state() == \"name\":\n        send(\"ok\")\n";
    let p = ok(src);
    match &p.stmts[0] {
        Stmt::Handler(h) => match &h.body.stmts[0] {
            Stmt::If { arms, .. } => assert_eq!(shape(&arms[0].0), "(get_state() == str)"),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn вложенные_обработчики_внутри_функции_запрещены() {
    let d = bad("func f():\n    on text:\n        send(\"x\")\n");
    assert!(d.text.contains("только на верхнем уровне"), "{}", d.text);
}

#[test]
fn ошибка_нет_двоеточия() {
    let d = bad("on text\n    send(\"x\")\n");
    assert!(d.text.contains("ожидалось ':'"), "{}", d.text);
    assert_eq!(d.span.line, 1);
}

#[test]
fn ошибка_нет_двоеточия_в_if() {
    let d = bad("if a\n    x = 1\n");
    assert!(d.text.contains("ожидалось ':'"), "{}", d.text);
}

#[test]
fn ошибка_пустой_блок() {
    let d = bad("on text:\n");
    assert!(d.text.contains("тело блока"), "{}", d.text);
}

#[test]
fn ошибка_пустой_блок_if() {
    let d = bad("if a:\nelse:\n    x = 1\n");
    assert!(d.text.contains("тело блока"), "{}", d.text);
}

#[test]
fn ошибка_лишний_отступ() {
    let d = bad("on text:\n    send(\"x\")\n        send(\"y\")\n");
    assert!(!d.text.is_empty());
    let d2 = bad("    send(\"x\")\n");
    assert!(d2.text.contains("неожиданный отступ"), "{}", d2.text);
}

#[test]
fn ошибка_on_без_типа() {
    let d = bad("on:\n    send(\"x\")\n");
    assert!(
        d.text.contains("после 'on' ожидался тип обработчика"),
        "{}",
        d.text
    );
}

#[test]
fn ошибка_неизвестный_тип_обработчика() {
    let d = bad("on nonsense:\n    send(\"x\")\n");
    assert!(
        d.text.contains("неизвестный тип обработчика 'nonsense'"),
        "{}",
        d.text
    );
}

#[test]
fn ошибка_state_внутри_обработчика() {
    let d = bad("on text:\n    state \"x\":\n        on text:\n            send(\"y\")\n");
    assert!(d.text.contains("только на верхнем уровне"), "{}", d.text);
}

#[test]
fn ошибка_state_без_обработчиков() {
    let d = bad("state \"x\":\n    send(\"y\")\n");
    assert!(d.text.contains("только обработчики on"), "{}", d.text);
}

#[test]
fn ошибка_state_внутри_if() {
    let d = bad("if a:\n    state \"x\":\n        on text:\n            send(\"y\")\n");
    assert!(d.text.contains("только на верхнем уровне"), "{}", d.text);
}

#[test]
fn ошибка_незакрытый_inline() {
    let d = bad("on text:\n    send(\"x\", inline = inline[[\"A\" -> \"a\"])\n");
    assert!(!d.text.is_empty());
    assert!(d.span.line == 2, "{:?}", d.span);
}

#[test]
fn ошибка_inline_без_стрелки() {
    let d = bad("x = inline[[\"A\"]]\n");
    assert!(d.text.contains("'->'"), "{}", d.text);
}

#[test]
fn ошибка_inline_кнопка_без_действия() {
    let d = bad("x = inline[[\"A\" -> 5]]\n");
    assert!(d.text.contains("после '->'"), "{}", d.text);
}

#[test]
fn ошибка_menu_без_скобок() {
    let d = bad("x = menu[\"Да\"]\n");
    assert!(d.text.contains("в начале строки menu"), "{}", d.text);
}

#[test]
fn ошибка_незакрытая_скобка_вызова() {
    let d = bad("send(\"x\"\n");
    assert!(!d.text.is_empty());
}

#[test]
fn ошибка_выражение_из_ключевого_слова() {
    let d = bad("x = if\n");
    assert!(
        d.text.contains("выражение не может начинаться"),
        "{}",
        d.text
    );
}

#[test]
fn ошибка_else_без_if() {
    let d = bad("else:\n    x = 1\n");
    assert!(
        d.text.contains("'else' без предшествующего 'if'"),
        "{}",
        d.text
    );
}

#[test]
fn ошибка_пустая_подстановка_в_позиции_выражения() {
    let d = bad("x = \"{ }\"\n");
    assert!(d.text.contains("подстановка"), "{}", d.text);
}

#[test]
fn ошибка_подстановка_с_мусором() {
    let d = bad("x = \"{a b}\"\n");
    assert!(d.text.contains("подстановке"), "{}", d.text);
}

#[test]
fn ошибка_имя_бота_без_кавычек() {
    let d = bad("bot shop\n");
    assert!(d.text.contains("имя бота"), "{}", d.text);
}

#[test]
fn ошибка_подстановка_в_имени_бота() {
    let d = bad("bot \"{x}\"\n");
    assert!(d.text.contains("подстановка {…} недопустима"), "{}", d.text);
}

#[test]
fn ошибка_for_без_in() {
    let d = bad("for x:\n    send(\"a\")\n");
    assert!(d.text.contains("'in'"), "{}", d.text);
}

#[test]
fn ошибка_два_выражения_в_строке() {
    let d = bad("x = 1 2\n");
    assert!(d.text.contains("ожидался конец строки"), "{}", d.text);
}

#[test]
fn ошибка_вложенная_функция_без_двоеточия() {
    let d = bad("func f()\n    return 1\n");
    assert!(d.text.contains("ожидалось ':'"), "{}", d.text);
}

#[test]
fn ошибка_незакрытая_клавиатура_в_примере() {
    let d = bad("on command \"start\":\n    kb = inline[\n        [\"A\" -> \"a\"]\n    ]\n    send(\"x\", inline = kb\n");
    assert!(!d.text.is_empty());
}
