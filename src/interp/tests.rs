use std::collections::BTreeMap;

use super::builtins::Builtin;
use super::value::{self, Value};
use super::{parse_and_run, run_handler, Ctx, FixedEnv};
use crate::diag::Span;
use crate::parser::ast::Handler;
use crate::parser::parse;

fn env() -> FixedEnv {
    FixedEnv::new(1_700_000_000, vec![0.0, 0.25, 0.5, 0.999])
}

fn with_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("поток")
        .join()
        .expect("без паники")
}

fn run(src: &str) -> Ctx {
    let mut e = env();
    match parse_and_run(src, "t.vx", &mut e) {
        Ok(c) => c,
        Err(d) => panic!("ошибка: {}\n{}", d.text, d.render(src)),
    }
}

fn eval_src(src: &str) -> Value {
    let ctx = run(src);
    ctx.get_global("result").unwrap_or(Value::Null)
}

fn eval_expr(expr: &str) -> Value {
    eval_src(&format!("result = {expr}\n"))
}

fn err(src: &str) -> String {
    let src = src.to_string();
    with_stack(move || {
        let mut e = env();
        match parse_and_run(&src, "t.vx", &mut e) {
            Ok(_) => panic!("ожидалась ошибка:\n{src}"),
            Err(d) => d.render(&src),
        }
    })
}

fn s(v: Value) -> String {
    v.display()
}

fn txt(x: &str) -> String {
    s(eval_expr(&format!("{x:?}")))
}

#[test]
fn литералы_и_типы() {
    assert_eq!(s(eval_expr("null")), "null");
    assert_eq!(s(eval_expr("true")), "true");
    assert_eq!(s(eval_expr("42")), "42");
    assert_eq!(s(eval_expr("1.0")), "1.0");
    assert_eq!(s(eval_expr("1.5")), "1.5");
    assert_eq!(s(eval_expr("\"текст\"")), "текст");
    assert_eq!(s(eval_expr("[1, 2]")), "[1,2]");
    assert_eq!(s(eval_expr("{\"a\": 1}")), "{\"a\":1}");
}

#[test]
fn арифметика_int() {
    assert_eq!(s(eval_expr("7 + 3")), "10");
    assert_eq!(s(eval_expr("7 - 3")), "4");
    assert_eq!(s(eval_expr("7 * 3")), "21");
    assert_eq!(s(eval_expr("7 // 3")), "2");
    assert_eq!(s(eval_expr("7 % 3")), "1");
    assert_eq!(s(eval_expr("2 + 3 * 4")), "14");
    assert_eq!(s(eval_expr("(2 + 3) * 4")), "20");
    assert_eq!(s(eval_expr("-7 // 3")), "-3");
    assert_eq!(s(eval_expr("-7 % 3")), "2");
}

#[test]
fn деление_всегда_float() {
    assert_eq!(s(eval_expr("6 / 3")), "2.0");
    assert_eq!(s(eval_expr("7 / 2")), "3.5");
    assert_eq!(s(eval_expr("1 / 2.0")), "0.5");
}

#[test]
fn смешение_int_и_float() {
    assert_eq!(s(eval_expr("1 + 1.5")), "2.5");
    assert_eq!(s(eval_expr("1.5 + 1")), "2.5");
    assert_eq!(s(eval_expr("2 * 2.5")), "5.0");
    assert_eq!(s(eval_expr("3 - 0.5")), "2.5");
}

#[test]
fn ошибка_деления_на_ноль() {
    assert!(err("x = 1 / 0\n").contains("деление на ноль"));
    assert!(err("x = 1 // 0\n").contains("деление на ноль"));
    assert!(err("x = 1 % 0\n").contains("остаток от деления на ноль"));
    assert!(err("x = 1.0 / 0.0\n").contains("деление на ноль"));
}

#[test]
fn переполнение_int() {
    let max = i64::MAX;
    assert!(err(&format!("x = {max} + 1\n")).contains("переполнение"));
    assert!(err("x = -9223372036854775807 - 2\n").contains("переполнение"));
    assert!(err(&format!("x = {max} * 2\n")).contains("переполнение"));
    assert!(err(&format!("x = -{max} - 2\n")).contains("переполнение"));
}

#[test]
fn ошибка_сложения_разных_типов() {
    assert!(err("x = 1 + true\n").contains("нужны числа"));
    assert!(err("x = [1] + 1\n").contains("нужны числа"));
    assert!(err("x = null + 1\n").contains("нужны числа"));
}

#[test]
fn строки_и_списки_склеиваются() {
    assert_eq!(s(eval_expr("\"a\" + \"b\"")), "ab");
    assert_eq!(s(eval_expr("\"n=\" + 5")), "n=5");
    assert_eq!(s(eval_expr("5 + \"!\"")), "5!");
    assert_eq!(s(eval_expr("[1] + [2, 3]")), "[1,2,3]");
}

#[test]
fn истинность() {
    assert_eq!(s(eval_expr("bool(0)")), "false");
    assert_eq!(s(eval_expr("bool(1)")), "true");
    assert_eq!(s(eval_expr("bool(\"\")")), "false");
    assert_eq!(s(eval_expr("bool(\"x\")")), "true");
    assert_eq!(s(eval_expr("bool([])")), "false");
    assert_eq!(s(eval_expr("bool({})")), "false");
    assert_eq!(s(eval_expr("bool(null)")), "false");
    assert_eq!(s(eval_expr("bool(0.0)")), "false");
}

#[test]
fn равенство_между_типами() {
    assert_eq!(s(eval_expr("1 == 1")), "true");
    assert_eq!(s(eval_expr("1 == 1.0")), "true");
    assert_eq!(s(eval_expr("1 == \"1\"")), "false");
    assert_eq!(s(eval_expr("null == null")), "true");
    assert_eq!(s(eval_expr("[1, 2] == [1, 2]")), "true");
    assert_eq!(s(eval_expr("{\"a\": 1} == {\"a\": 1}")), "true");
    assert_eq!(s(eval_expr("{\"a\": 1} == {\"b\": 1}")), "false");
    assert_eq!(s(eval_expr("1 != 2")), "true");
}

#[test]
fn сравнение_и_ошибка_разных_типов() {
    assert_eq!(s(eval_expr("1 < 2")), "true");
    assert_eq!(s(eval_expr("\"a\" < \"b\"")), "true");
    assert_eq!(s(eval_expr("[1, 2] < [1, 3]")), "true");
    assert_eq!(s(eval_expr("1 <= 1")), "true");
    assert!(err("x = 1 < \"2\"\n").contains("разные типы"));
    assert!(err("x = null < 1\n").contains("разные типы"));
    assert!(err("x = {} > {}\n").contains("словари нельзя сравнивать"));
}

#[test]
fn coalesce_только_для_null() {
    assert_eq!(s(eval_expr("null ?? 5")), "5");
    assert_eq!(s(eval_expr("0 ?? 5")), "0");
    assert_eq!(s(eval_expr("\"\" ?? \"x\"")), "");
    assert_eq!(s(eval_expr("false ?? true")), "false");
    assert_eq!(s(eval_expr("null ?? null ?? 7")), "7");
    assert_eq!(s(eval_expr("null ?? 1")), "1");
}

#[test]
fn and_or_короткая_цепочка() {
    assert_eq!(s(eval_expr("false and boom")), "false");
    assert_eq!(s(eval_expr("true or boom")), "true");
    assert_eq!(s(eval_expr("true and true")), "true");
    assert_eq!(s(eval_expr("0 or \"\"")), "false");
    assert_eq!(s(eval_expr("1 and 2")), "true");
}

#[test]
fn in_для_строк_списков_словарей() {
    assert_eq!(s(eval_expr("\"ел\" in \"зелень\"")), "true");
    assert_eq!(s(eval_expr("\"x\" in \"зелень\"")), "false");
    assert_eq!(s(eval_expr("2 in [1, 2, 3]")), "true");
    assert_eq!(s(eval_expr("\"a\" in {\"a\": 1}")), "true");
    assert_eq!(s(eval_expr("\"b\" in {\"a\": 1}")), "false");
    assert!(err("x = 1 in 2\n").contains("'in' работает"));
}

#[test]
fn поле_и_индекс_отсутствующие_дают_null() {
    assert_eq!(s(eval_src("m = {\"a\": 1}\nresult = m.missing\n")), "null");
    assert_eq!(s(eval_expr("[1, 2][10]")), "null");
    assert_eq!(s(eval_expr("{\"a\": 1}[\"b\"]")), "null");
    assert_eq!(s(eval_expr("null.missing")), "null");
    assert_eq!(s(eval_expr("\"abc\"[10]")), "null");
}

#[test]
fn отрицательные_индексы() {
    assert_eq!(s(eval_expr("[10, 20, 30][-1]")), "30");
    assert_eq!(s(eval_expr("[10, 20, 30][-3]")), "10");
    assert_eq!(s(eval_expr("[10, 20, 30][-4]")), "null");
    assert_eq!(s(eval_expr("\"абв\"[-1]")), "в");
}

#[test]
fn присваивание_полю_и_индексу() {
    let ctx = run("m = {\"a\": 1}\nm.a = 2\nm[\"b\"] = 3\nl = [1, 2]\nl[0] = 9\nl[-1] = 8\n");
    let m = ctx.get_global("m").expect("m");
    let l = ctx.get_global("l").expect("l");
    assert_eq!(s(m), "{\"a\":2,\"b\":3}");
    assert_eq!(s(l), "[9,8]");
}

#[test]
fn присваивание_за_границы_списка_ошибка() {
    assert!(err("l = [1]\nl[5] = 2\n").contains("выход за границы списка"));
}

#[test]
fn присваивание_полю_не_словаря() {
    assert!(err("x = 1\nx.y = 2\n").contains("нельзя присвоить поле"));
}

#[test]
fn если_и_циклы() {
    let ctx = run(concat!(
        "a = 0\n",
        "if false:\n    a = 1\nelif true:\n    a = 2\nelse:\n    a = 3\n",
        "n = 0\n",
        "while n < 3:\n    n = n + 1\n",
        "acc = 0\n",
        "for i in [1, 2, 3]:\n    acc = acc + i\n",
        "for k in {\"a\": 1, \"b\": 2}:\n    acc = acc + 1\n",
    ));
    assert_eq!(s(ctx.get_global("a").unwrap()), "2");
    assert_eq!(s(ctx.get_global("n").unwrap()), "3");
    assert_eq!(s(ctx.get_global("acc").unwrap()), "8");
}

#[test]
fn break_continue() {
    let ctx = run(concat!(
        "acc = []\n",
        "for i in [1, 2, 3, 4, 5]:\n",
        "    if i == 2:\n        continue\n",
        "    if i == 4:\n        break\n",
        "    push(acc, i)\n",
    ));
    assert_eq!(s(ctx.get_global("acc").unwrap()), "[1,3]");
}

#[test]
fn файл_из_пяти_строк_для_цикла_число() {
    let ctx = run("s = 0\nfor i in range(5):\n    s = s + i\n");
    assert_eq!(s(ctx.get_global("s").unwrap()), "10");
}

#[test]
fn функции_рекурсия_и_области_видимости() {
    let ctx = run(concat!(
        "g = 10\n",
        "func fact(n):\n",
        "    if n <= 1:\n        return 1\n",
        "    return n * fact(n - 1)\n",
        "func touch():\n",
        "    g = 99\n",
        "    return g\n",
        "f = fact(5)\n",
        "t = touch()\n",
    ));
    assert_eq!(s(ctx.get_global("f").unwrap()), "120");
    assert_eq!(s(ctx.get_global("t").unwrap()), "99");
    assert_eq!(s(ctx.get_global("g").unwrap()), "10");
}

#[test]
fn функция_видит_глобальную_переменную() {
    let ctx = run("base = 5\nfunc add(x):\n    return base + x\ny = add(2)\n");
    assert_eq!(s(ctx.get_global("y").unwrap()), "7");
}

#[test]
fn значения_по_умолчанию_и_именованные_аргументы() {
    let ctx = run(concat!(
        "func greet(name, punct = \"!\"):\n",
        "    return \"Привет, \" + name + punct\n",
        "a = greet(\"Аня\")\n",
        "b = greet(\"Боря\", \"?\")\n",
        "c = greet(punct = \".\", name = \"Вера\")\n",
    ));
    assert_eq!(s(ctx.get_global("a").unwrap()), "Привет, Аня!");
    assert_eq!(s(ctx.get_global("b").unwrap()), "Привет, Боря?");
    assert_eq!(s(ctx.get_global("c").unwrap()), "Привет, Вера.");
}

#[test]
fn лимит_глубины_рекурсии() {
    let out = err("func boom(n):\n    return boom(n + 1)\nx = boom(0)\n");
    assert!(out.contains("глубина рекурсии"), "{out}");
    assert!(out.contains("стек вызовов"), "{out}");
}

#[test]
fn ошибка_неизвестная_функция_и_имя() {
    assert!(err("x = nope()\n").contains("неизвестная функция 'nope'"));
    assert!(err("x = nope\n").contains("неизвестное имя 'nope'"));
}

#[test]
fn ошибка_аргументы_функции() {
    assert!(err("func f(a):\n    return a\nx = f()\n").contains("не задан аргумент 'a'"));
    assert!(err("func f(a):\n    return a\nx = f(1, 2)\n").contains("слишком много аргументов"));
    assert!(err("func f(a):\n    return a\nx = f(b = 1)\n").contains("нет аргумента 'b'"));
}

#[test]
fn подстановка_в_строке_форматирует_значения() {
    assert_eq!(txt("{1.0}"), "1.0");
    assert_eq!(txt("{2.5}"), "2.5");
    assert_eq!(txt("{null}"), "null");
    assert_eq!(txt("{true}"), "true");
    assert_eq!(
        s(eval_src("l = [1, \"a\"]\nresult = \"{l}\"\n")),
        "[1,\"a\"]"
    );
    assert_eq!(
        s(eval_src("d = {\"a\": 1}\nresult = \"{d}\"\n")),
        "{\"a\":1}"
    );
    assert_eq!(s(eval_expr("\"итог: {1 + 2}, {null ?? 0}\"")), "итог: 3, 0");
}

#[test]
fn builtin_len() {
    assert_eq!(s(eval_expr("len(\"абв\")")), "3");
    assert_eq!(s(eval_expr("len([1, 2])")), "2");
    assert_eq!(s(eval_expr("len({})")), "0");
    assert!(err("x = len(1)\n").contains("len не работает"));
}

#[test]
fn builtin_str_int_float_bool_type() {
    assert_eq!(s(eval_expr("str(1.0)")), "1.0");
    assert_eq!(s(eval_expr("str(null)")), "null");
    assert_eq!(s(eval_expr("int(\"42\")")), "42");
    assert_eq!(s(eval_expr("int(3.9)")), "3");
    assert_eq!(s(eval_expr("int(true)")), "1");
    assert_eq!(s(eval_expr("int(null)")), "0");
    assert_eq!(s(eval_expr("float(\"1.5\")")), "1.5");
    assert_eq!(s(eval_expr("float(2)")), "2.0");
    assert_eq!(s(eval_expr("bool(\"x\")")), "true");
    assert_eq!(s(eval_expr("type([])")), "list");
    assert_eq!(s(eval_expr("type({})")), "map");
    assert_eq!(s(eval_expr("type(1)")), "int");
    assert!(err("x = int(\"abc\")\n").contains("не является целым числом"));
    assert!(err("x = float(\"\")\n").contains("не является числом"));
    assert!(err("x = int([1])\n").contains("int не работает"));
    assert!(err("x = type()\n").contains("нужен аргумент #1"));
}

#[test]
fn builtin_keys_values_has() {
    assert_eq!(s(eval_expr("keys({\"b\": 1, \"a\": 2})")), "[\"a\",\"b\"]");
    assert_eq!(s(eval_expr("values({\"b\": 1, \"a\": 2})")), "[2,1]");
    assert_eq!(s(eval_expr("has({\"a\": 1}, \"a\")")), "true");
    assert_eq!(s(eval_expr("has({\"a\": 1}, \"b\")")), "false");
    assert_eq!(s(eval_expr("has([1, 2], 2)")), "true");
    assert!(err("x = keys(1)\n").contains("keys работает только для словаря"));
    assert_eq!(s(eval_expr("has([], 1)")), "false");
}

#[test]
fn builtin_push_pop() {
    let ctx = run("l = []\npush(l, 1)\npush(l, 2)\na = pop(l)\nb = pop(l)\nc = pop(l)\n");
    assert_eq!(s(ctx.get_global("l").unwrap()), "[]");
    assert_eq!(s(ctx.get_global("a").unwrap()), "2");
    assert_eq!(s(ctx.get_global("b").unwrap()), "1");
    assert_eq!(s(ctx.get_global("c").unwrap()), "null");
    let ctx = run("l = [1, 2, 3]\nx = pop(l, 0)\n");
    assert_eq!(s(ctx.get_global("x").unwrap()), "1");
    assert!(err("x = push(1, 2)\n").contains("push работает только для списка"));
}

#[test]
fn builtin_join_split_trim_case() {
    assert_eq!(s(eval_expr("join([1, 2, 3], \"-\")")), "1-2-3");
    assert_eq!(s(eval_expr("join([], \"-\")")), "");
    assert_eq!(s(eval_expr("split(\"а б в\")")), "[\"а\",\"б\",\"в\"]");
    assert_eq!(s(eval_expr("split(\"а,б\", \",\")")), "[\"а\",\"б\"]");
    assert_eq!(s(eval_expr("split(\"аб\", \"\")")), "[\"а\",\"б\"]");
    assert_eq!(s(eval_expr("trim(\"  x  \")")), "x");
    assert_eq!(s(eval_expr("lower(\"ПРИВЕТ\")")), "привет");
    assert_eq!(s(eval_expr("upper(\"привет\")")), "ПРИВЕТ");
    assert!(err("x = join(1)\n").contains("join работает только для списка"));
}

#[test]
fn builtin_replace_find_slice() {
    assert_eq!(s(eval_expr("replace(\"ааа\", \"а\", \"б\")")), "ббб");
    assert_eq!(s(eval_expr("replace(\"abc\", \"x\", \"y\")")), "abc");
    assert!(err("x = replace(\"a\", \"\", \"b\")\n").contains("пустую строку"));
    assert_eq!(s(eval_expr("find(\"абв\", \"в\")")), "2");
    assert_eq!(s(eval_expr("find(\"абв\", \"z\")")), "-1");
    assert_eq!(s(eval_expr("find([1, 2], 2)")), "1");
    assert_eq!(s(eval_expr("slice(\"абвгд\", 1, 3)")), "бв");
    assert_eq!(s(eval_expr("slice([1, 2, 3], -2)")), "[2,3]");
    assert_eq!(s(eval_expr("slice(\"абв\", 5)")), "");
    assert_eq!(s(eval_expr("slice([1, 2, 3], 2, 1)")), "[]");
}

#[test]
fn builtin_range() {
    assert_eq!(s(eval_expr("range(3)")), "[0,1,2]");
    assert_eq!(s(eval_expr("range(2, 5)")), "[2,3,4]");
    assert_eq!(s(eval_expr("range(0, 10, 3)")), "[0,3,6,9]");
    assert_eq!(s(eval_expr("range(3, 0, -1)")), "[3,2,1]");
    assert_eq!(s(eval_expr("range(0)")), "[]");
    assert!(err("x = range(0, 5, 0)\n").contains("шаг не может быть 0"));
    assert!(err("x = range(\"a\")\n").contains("должен быть числом"));
}

#[test]
fn builtin_random_детерминирован() {
    assert_eq!(s(eval_expr("random()")), "0.0");
    assert_eq!(s(eval_expr("random(10)")), "0");
    assert_eq!(s(eval_expr("random([])")), "null");
    assert_eq!(s(eval_expr("random(\"\")")), "");
    assert_eq!(s(eval_expr("random(-5)")), "0");
    let ctx = run("a = random([1, 2, 3])\nb = random(10)\nc = random()\n");
    assert_eq!(s(ctx.get_global("a").unwrap()), "1");
    assert_eq!(s(ctx.get_global("b").unwrap()), "2");
    assert_eq!(s(ctx.get_global("c").unwrap()), "0.5");
}

#[test]
fn builtin_now_из_окружения() {
    assert_eq!(s(eval_expr("now()")), "1700000000");
}

#[test]
fn builtin_json_и_parse_json() {
    assert_eq!(
        s(eval_expr("json({\"a\": [1, true, null]})")),
        "{\"a\":[1,true,null]}"
    );
    assert_eq!(s(eval_expr("json(\"привет\")")), "\"привет\"");
    assert_eq!(s(eval_expr("json(1.0)")), "1.0");
    assert_eq!(
        s(eval_src("d = parse_json(\"[1, 2]\")\nresult = d[1]\n")),
        "2"
    );
    assert_eq!(s(eval_expr("type(parse_json(\"[1,2]\"))")), "list");
    assert_eq!(s(eval_expr("parse_json(\"[1, 2]\")[1]")), "2");
    assert!(err("x = parse_json(\"[1,\")\n").contains("неверный JSON"));
    assert!(err("x = parse_json(1)\n").contains("parse_json работает только для строки"));
    assert!(err("x = json(1.0 / 0.0)\n").contains("деление на ноль"));
}

#[test]
fn builtin_log_и_exit() {
    let ctx = run("log(\"привет\")\n");
    assert_eq!(ctx.logs, vec!["привет".to_string()]);
    let mut e = env();
    let ctx = parse_and_run("x = 1\nexit(3)\nx = 2\n", "t.vx", &mut e).expect("exit не ошибка");
    assert_eq!(ctx.exit, Some(3));
    assert_eq!(s(ctx.get_global("x").unwrap()), "1");
}

#[test]
fn inline_и_menu_в_формате_telegram() {
    let kb = eval_expr(
        "inline[[\"Кофе\" -> \"buy:coffee\", \"Чай\" -> \"buy:tea\"], [\"Сайт\" -> link \"https://a.kz\"]]",
    );
    assert_eq!(
        s(kb),
        "{\"inline_keyboard\":[[{\"callback_data\":\"buy:coffee\",\"text\":\"Кофе\"},{\"callback_data\":\"buy:tea\",\"text\":\"Чай\"}],[{\"text\":\"Сайт\",\"url\":\"https://a.kz\"}]]}"
    );
    let menu = eval_expr("menu[[\"Да\", \"Нет\"], [\"Отмена\"]]");
    assert_eq!(
        s(menu),
        "{\"keyboard\":[[{\"text\":\"Да\"},{\"text\":\"Нет\"}],[{\"text\":\"Отмена\"}]],\"resize_keyboard\":true}"
    );
    let empty = eval_expr("menu[]");
    assert_eq!(s(empty), "{\"keyboard\":[],\"resize_keyboard\":true}");
    let sw = eval_expr("inline[[\"Позже\" -> switch \"напомни\"]]");
    assert_eq!(
        s(sw),
        "{\"inline_keyboard\":[[{\"switch_inline_query\":\"напомни\",\"text\":\"Позже\"}]]}"
    );
}

#[test]
fn ссылка_вычисляется_из_переменной() {
    let ctx = run("u = \"https://x.kz\"\nkb = inline[[\"Сайт\" -> link u]]\n");
    assert_eq!(
        s(ctx.get_global("kb").unwrap()),
        "{\"inline_keyboard\":[[{\"text\":\"Сайт\",\"url\":\"https://x.kz\"}]]}"
    );
}

fn example_env(ctx: &mut Ctx) {
    ctx.register(
        "send",
        Builtin::new(
            &["text", "inline", "menu", "parse_mode", "chat"],
            |r, a, s| {
                let text = a.first().cloned().unwrap_or(Value::Null).display();
                r.ctx.logs.push(format!("send:{text}"));
                let _ = s;
                Ok(Value::from_pairs(vec![("message_id", Value::Int(1))]))
            },
        ),
    );
    ctx.register(
        "edit",
        Builtin::new(&["text", "inline"], |r, a, _s| {
            let text = a.first().cloned().unwrap_or(Value::Null).display();
            r.ctx.logs.push(format!("edit:{text}"));
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "answer",
        Builtin::new(&["text"], |r, a, _s| {
            let text = a.first().cloned().unwrap_or(Value::Null).display();
            r.ctx.logs.push(format!("answer:{text}"));
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "send_photo",
        Builtin::new(&["photo", "caption"], |r, a, _s| {
            let p = a.first().cloned().unwrap_or(Value::Null).display();
            r.ctx.logs.push(format!("send_photo:{p}"));
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "send_document",
        Builtin::new(&["document", "caption"], |r, a, _s| {
            let p = a.first().cloned().unwrap_or(Value::Null).display();
            r.ctx.logs.push(format!("send_document:{p}"));
            Ok(Value::Null)
        }),
    );
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
            Ok(super::field(&store, &k).unwrap_or(Value::Null))
        }),
    );
    ctx.register(
        "set_state",
        Builtin::new(&["state"], |r, a, _s| {
            let v = a.first().cloned().unwrap_or(Value::Null);
            r.ctx.set_global("__state", v);
            Ok(Value::Null)
        }),
    );
    ctx.register(
        "get_state",
        Builtin::new(&[], |r, _a, _s| {
            Ok(r.ctx.get_global("__state").unwrap_or(Value::Null))
        }),
    );
}

fn run_example(name: &'static str, src: &'static str) {
    with_stack(move || run_example_inner(name, src));
}

fn run_example_inner(name: &str, src: &str) {
    let program = match parse(src, name) {
        Ok(p) => p,
        Err(d) => panic!("{name}: {}\n{}", d.text, d.render(src)),
    };
    let mut ctx = Ctx::new(name);
    example_env(&mut ctx);
    let mut e = env();
    super::run_top(&mut ctx, &program, &mut e).expect("инициализация");
    let table = super::handler_table(&program);
    assert!(!table.is_empty(), "{name}: нет обработчиков");
    for (state, handler) in table {
        let mut vars = BTreeMap::new();
        vars.insert(
            "u".to_string(),
            Value::from_pairs(vec![
                (
                    "message",
                    Value::from_pairs(vec![
                        ("text", Value::str("Аня")),
                        (
                            "from",
                            Value::from_pairs(vec![("first_name", Value::str("Аня"))]),
                        ),
                        ("chat", Value::from_pairs(vec![("id", Value::Int(7))])),
                        (
                            "photo",
                            Value::list(vec![Value::from_pairs(vec![(
                                "file_id",
                                Value::str("ph1"),
                            )])]),
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
                    Value::from_pairs(vec![("data", Value::str("inc"))]),
                ),
            ]),
        );
        let _ = state;
        let h: Handler = handler;
        let mut vars2 = vars.clone();
        for (k, v) in msg_vars(vars.get("u").expect("u")) {
            vars2.insert(k, v);
        }
        let res = run_handler(&mut ctx, &program, &h, vars2, &mut e);
        match res {
            Ok(_) => {}
            Err(d) => panic!(
                "{name}: ошибка в обработчике: {}\n{}",
                d.text,
                d.render(src)
            ),
        }
    }
    assert!(
        !ctx.logs.is_empty(),
        "{name}: обработчики ничего не сделали"
    );
}

fn msg_vars(upd: &Value) -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    m.insert("upd".to_string(), upd.clone());
    m.insert("msg".to_string(), super::msg_of(upd));
    m.insert(
        "cb".to_string(),
        super::field(upd, "callback_query").unwrap_or(Value::Null),
    );
    m.insert("chat".to_string(), super::chat_of(upd));
    m.insert("user".to_string(), super::user_of(upd));
    m.insert("args".to_string(), Value::list(Vec::new()));
    m.insert("text".to_string(), Value::str("Аня"));
    m
}

#[test]
fn пример_1_выполняется() {
    run_example(
        "examples/01_start.vx",
        include_str!("../../examples/01_start.vx"),
    );
}

#[test]
fn пример_2_выполняется() {
    run_example(
        "examples/02_buttons.vx",
        include_str!("../../examples/02_buttons.vx"),
    );
}

#[test]
fn пример_3_выполняется() {
    run_example(
        "examples/03_states.vx",
        include_str!("../../examples/03_states.vx"),
    );
}

#[test]
fn пример_4_выполняется() {
    run_example(
        "examples/04_callback_edit.vx",
        include_str!("../../examples/04_callback_edit.vx"),
    );
}

#[test]
fn пример_5_выполняется() {
    run_example(
        "examples/05_photo.vx",
        include_str!("../../examples/05_photo.vx"),
    );
}

#[test]
fn пример_4_считает_нажатия() {
    let src = include_str!("../../examples/04_callback_edit.vx");
    let program = parse(src, "examples/04_callback_edit.vx").expect("парсится");
    let mut ctx = Ctx::new("examples/04_callback_edit.vx");
    example_env(&mut ctx);
    let mut e = env();
    let table = super::handler_table(&program);
    let inc = table
        .iter()
        .find(|(_, h)| matches!(h.trigger, crate::parser::ast::Trigger::Callback(_)))
        .expect("callback inc");
    let upd = Value::from_pairs(vec![(
        "callback_query",
        Value::from_pairs(vec![
            ("data", Value::str("inc")),
            (
                "message",
                Value::from_pairs(vec![("message_id", Value::Int(3))]),
            ),
        ]),
    )]);
    for _ in 0..2 {
        let vars = msg_vars(&upd);
        run_handler(&mut ctx, &program, &inc.1, vars, &mut e).expect("обработчик");
    }
    let state = ctx.get_global("__store").expect("store");
    assert_eq!(s(super::field(&state, "count").unwrap_or(Value::Null)), "2");
    assert!(ctx.logs.iter().any(|l| l == "edit:Нажато раз: 2"));
}

#[test]
fn run_handler_возвращает_return() {
    let src = "on text:\n    return 42\n";
    let program = parse(src, "t.vx").expect("парсится");
    let handler = match &program.stmts[0] {
        crate::parser::ast::Stmt::Handler(h) => h.clone(),
        _ => panic!("обработчик"),
    };
    let mut ctx = Ctx::new("t.vx");
    let mut e = env();
    let vars = msg_vars(&Value::Null);
    let v = run_handler(&mut ctx, &program, &handler, vars, &mut e).expect("ок");
    assert_eq!(s(v), "42");
}

#[test]
fn таблица_обработчиков() {
    let program = parse(
        "on text:\n    send(\"a\")\nstate \"s\":\n    on callback \"x\":\n        send(\"b\")\n",
        "t.vx",
    )
    .expect("парсится");
    let table = super::handler_table(&program);
    assert_eq!(table.len(), 2);
    assert_eq!(table[0].0, None);
    assert_eq!(table[1].0.as_deref(), Some("s"));
}

#[test]
fn ctx_переменные_обработчика() {
    let src = "on text:\n    x = chat + \"|\" + user.first_name + \"|\" + text\n    return x\n";
    let program = parse(src, "t.vx").expect("парсится");
    let handler = match &program.stmts[0] {
        crate::parser::ast::Stmt::Handler(h) => h.clone(),
        _ => panic!("обработчик"),
    };
    let mut ctx = Ctx::new("t.vx");
    let mut e = env();
    let upd = Value::from_pairs(vec![(
        "message",
        Value::from_pairs(vec![
            ("text", Value::str("привет")),
            (
                "from",
                Value::from_pairs(vec![("first_name", Value::str("Аня"))]),
            ),
            ("chat", Value::from_pairs(vec![("id", Value::Int(7))])),
        ]),
    )]);
    let mut vars = msg_vars(&upd);
    vars.insert("text".to_string(), Value::str("привет"));
    let v = run_handler(&mut ctx, &program, &handler, vars, &mut e).expect("ок");
    assert_eq!(s(v), "7|Аня|привет");
}

#[test]
fn чужие_builtins_регистрируются() {
    let mut e = env();
    let program = parse("x = my(2)\n", "t.vx").expect("парсится");
    let mut ctx = Ctx::new("t.vx");
    ctx.register(
        "my",
        Builtin::new(&["n"], |_r, a, _s| {
            let n = match a.first() {
                Some(Value::Int(i)) => *i,
                _ => 0,
            };
            Ok(Value::Int(n * 10))
        }),
    );
    super::run_top(&mut ctx, &program, &mut e).expect("exec");
    assert_eq!(s(ctx.get_global("x").unwrap()), "20");
}

#[test]
fn json_парсер_краевые_случаи() {
    assert_eq!(
        value::parse_json("null").map(|v| v.display()),
        Ok("null".into())
    );
    assert_eq!(
        value::parse_json(" 1 ").map(|v| v.display()),
        Ok("1".into())
    );
    assert_eq!(
        value::parse_json("-0.5").map(|v| v.display()),
        Ok("-0.5".into())
    );
    assert_eq!(
        value::parse_json("1e3").map(|v| v.display()),
        Ok("1000.0".into())
    );
    assert!(value::parse_json("").is_err());
    assert!(value::parse_json("{").is_err());
    assert!(value::parse_json("[1,]").is_err());
    assert!(value::parse_json("1 2").is_err());
}

#[test]
fn ошибка_с_позицией_и_стеком() {
    let out = err("func a():\n    return b()\nfunc b():\n    return 1 / 0\nx = a()\n");
    assert!(out.contains("t.vx:4:12"), "{out}");
    assert!(out.contains("деление на ноль"), "{out}");
    assert!(out.contains("a → b"), "{out}");
}

#[test]
fn span_доступен_в_тестах() {
    let _ = Span::new(1, 1, 1);
}
