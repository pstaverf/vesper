use super::lex;
use super::token::{Kw, StrPart, Token, TokenKind};
use crate::diag::{Diag, Span};

fn kinds(src: &str) -> Vec<TokenKind> {
    let toks = lex(src, "<тест>").expect("должно лексироваться");
    toks.into_iter().map(|t| t.kind).collect()
}

fn fail(src: &str) -> Diag {
    lex(src, "<тест>").expect_err("должна быть ошибка")
}

fn text_of(t: &Token) -> String {
    match &t.kind {
        TokenKind::Str(parts) => parts
            .iter()
            .map(|p| match p {
                StrPart::Text(s) => s.clone(),
                StrPart::Expr { .. } => "<expr>".to_string(),
            })
            .collect(),
        k => panic!("не строка: {k:?}"),
    }
}

fn str_parts(src: &str) -> Vec<StrPart> {
    match kinds(src).into_iter().next().expect("один токен") {
        TokenKind::Str(p) => p,
        k => panic!("не строка: {k:?}"),
    }
}

fn expr_of(src: &str, index: usize) -> Vec<TokenKind> {
    match &str_parts(src)[index] {
        StrPart::Expr { tokens, .. } => tokens.iter().map(|t| t.kind.clone()).collect(),
        p => panic!("ожидалось выражение, получено {p:?}"),
    }
}

#[test]
fn пустой_файл_даёт_только_eof() {
    assert_eq!(kinds(""), vec![TokenKind::Eof]);
}

#[test]
fn пробелы_и_переводы_строк_игнорируются() {
    assert_eq!(kinds("\n\n   \n\t\n"), vec![TokenKind::Eof]);
}

#[test]
fn простой_оператор() {
    assert_eq!(
        kinds("x = 1\n"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Assign,
            TokenKind::Int(1),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn без_перевода_строки_в_конце() {
    assert_eq!(
        kinds("x = 1"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Assign,
            TokenKind::Int(1),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn crlf_не_ломает_разбор() {
    assert_eq!(
        kinds("x = 1\r\ny = 2\r\n"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Assign,
            TokenKind::Int(1),
            TokenKind::Newline,
            TokenKind::Ident("y".into()),
            TokenKind::Assign,
            TokenKind::Int(2),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn одиночный_cr_в_середине_строки() {
    let d = fail("x = 1\ry = 2\n");
    assert!(d.text.contains("возврата каретки"), "{}", d.text);
    assert_eq!(d.span.line, 1);
    assert_eq!(d.span.col, 6);
}

#[test]
fn ключевые_слова_и_идентификаторы() {
    assert_eq!(
        kinds("on state if elif else while for in break continue func return and or not bot inline menu link switch true false null"),
        vec![
            TokenKind::Kw(Kw::On),
            TokenKind::Kw(Kw::State),
            TokenKind::Kw(Kw::If),
            TokenKind::Kw(Kw::Elif),
            TokenKind::Kw(Kw::Else),
            TokenKind::Kw(Kw::While),
            TokenKind::Kw(Kw::For),
            TokenKind::Kw(Kw::In),
            TokenKind::Kw(Kw::Break),
            TokenKind::Kw(Kw::Continue),
            TokenKind::Kw(Kw::Func),
            TokenKind::Kw(Kw::Return),
            TokenKind::Kw(Kw::And),
            TokenKind::Kw(Kw::Or),
            TokenKind::Kw(Kw::Not),
            TokenKind::Kw(Kw::Bot),
            TokenKind::Kw(Kw::Inline),
            TokenKind::Kw(Kw::Menu),
            TokenKind::Kw(Kw::Link),
            TokenKind::Kw(Kw::Switch),
            TokenKind::Bool(true),
            TokenKind::Bool(false),
            TokenKind::Null,
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn идентификатор_с_подчёркиванием() {
    assert_eq!(
        kinds("_x1 ab_c"),
        vec![
            TokenKind::Ident("_x1".into()),
            TokenKind::Ident("ab_c".into()),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn числа() {
    assert_eq!(
        kinds("1_000 19.99 1e5 1.5e-3 0.0"),
        vec![
            TokenKind::Int(1000),
            TokenKind::Float(19.99),
            TokenKind::Float(100000.0),
            TokenKind::Float(0.0015),
            TokenKind::Float(0.0),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn двойное_подчёркивание_в_числе() {
    let d = fail("x = 1__0\n");
    assert!(d.text.contains("'_' не может идти подряд"), "{}", d.text);
    assert_eq!(d.span.line, 1);
    assert_eq!(d.span.col, 7);
}

#[test]
fn подчёркивание_в_конце_числа() {
    let d = fail("x = 1_\n");
    assert!(d.text.contains("'_' не может стоять в конце"), "{}", d.text);
    assert_eq!(d.span.col, 6);
}

#[test]
fn точка_без_цифры() {
    let d = fail("x = 1.\n");
    assert!(d.text.contains("после точки нужна цифра"), "{}", d.text);
    assert_eq!(d.span.col, 6);
}

#[test]
fn буква_после_цифры() {
    let d = fail("x = 123abc\n");
    assert!(d.text.contains("лишний символ 'a'"), "{}", d.text);
    assert_eq!(d.span.col, 8);
}

#[test]
fn экспонента_без_степени() {
    let d = fail("x = 1e\n");
    assert!(d.text.contains("после 'e' нужна степень"), "{}", d.text);
    assert_eq!(d.span.col, 6);
}

#[test]
fn слишком_большое_число() {
    let d = fail("x = 99999999999999999999999\n");
    assert!(d.text.contains("число слишком велико"), "{}", d.text);
    assert_eq!(d.span.col, 5);
    assert_eq!(d.span.len, 23);
}

#[test]
fn операторы() {
    assert_eq!(
        kinds("+ - * / // % == != < <= > >= ?? -> . , : ( ) [ ] { }"),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::SlashSlash,
            TokenKind::Percent,
            TokenKind::Eq,
            TokenKind::Ne,
            TokenKind::Lt,
            TokenKind::Le,
            TokenKind::Gt,
            TokenKind::Ge,
            TokenKind::Coalesce,
            TokenKind::Arrow,
            TokenKind::Dot,
            TokenKind::Comma,
            TokenKind::Colon,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn одинокие_знаки_дают_понятную_ошибку() {
    assert!(fail("a = b ! c\n").text.contains("используй '!='"));
    assert!(fail("a = b ? c\n").text.contains("используй '??'"));
    assert!(fail("a = b @ c\n").text.contains("недопустимый символ '@'"));
}

#[test]
fn незакрытая_скобка() {
    let d = fail("f(a\n");
    assert!(d.text.contains("незакрытая скобка '('"), "{}", d.text);
    assert_eq!(d.span.col, 2);
}

#[test]
fn несогласованная_скобка() {
    let d = fail("f(a]\n");
    assert!(d.text.contains("не закрывает"), "{}", d.text);
    assert_eq!(d.span.col, 4);
}

#[test]
fn лишняя_закрывающая_скобка() {
    let d = fail("f(a))\n");
    assert!(d.text.contains("лишняя закрывающая"), "{}", d.text);
    assert_eq!(d.span.col, 5);
}

#[test]
fn отступы_и_блоки() {
    assert_eq!(
        kinds("if x:\n    y = 1\nz = 2\n"),
        vec![
            TokenKind::Kw(Kw::If),
            TokenKind::Ident("x".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("y".into()),
            TokenKind::Assign,
            TokenKind::Int(1),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Ident("z".into()),
            TokenKind::Assign,
            TokenKind::Int(2),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn вложенные_отступы_и_несколько_dedent() {
    assert_eq!(
        kinds("a:\n    b:\n        c\n    d\ne\n"),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("b".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("c".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Ident("d".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Ident("e".into()),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn пустые_строки_не_закрывают_блок() {
    assert_eq!(
        kinds("a:\n    b\n\n    c\nd\n"),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("b".into()),
            TokenKind::Newline,
            TokenKind::Ident("c".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Ident("d".into()),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn комментарии_не_влияют_на_блоки() {
    assert_eq!(
        kinds("a:\n    # комментарий\n    b\n# хвост\nd\n"),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("b".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Ident("d".into()),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn все_dedent_в_конце_файла() {
    assert_eq!(
        kinds("a:\n    b:\n        c\n"),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("b".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Ident("c".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Dedent,
            TokenKind::Eof
        ]
    );
}

#[test]
fn неверный_отступ() {
    let d = fail("a:\n    b\n  c\n");
    assert!(d.text.contains("неверный отступ"), "{}", d.text);
    assert_eq!(d.span.line, 3);
    assert_eq!(d.span.col, 3);
}

#[test]
fn табуляция_в_отступе() {
    let d = fail("a:\n\tb\n");
    assert!(
        d.text.contains("табуляция недопустима в отступе"),
        "{}",
        d.text
    );
    assert_eq!(d.span.line, 2);
    assert_eq!(d.span.col, 1);
}

#[test]
fn табуляция_между_токенами() {
    let d = fail("a\t= 1\n");
    assert!(
        d.text.contains("табуляция недопустима вне строки"),
        "{}",
        d.text
    );
    assert_eq!(d.span.col, 2);
}

#[test]
fn скобки_переносят_строку() {
    assert_eq!(
        kinds("x = [\n    1,\n    2\n]\ny = 3\n"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Assign,
            TokenKind::LBracket,
            TokenKind::Int(1),
            TokenKind::Comma,
            TokenKind::Int(2),
            TokenKind::RBracket,
            TokenKind::Newline,
            TokenKind::Ident("y".into()),
            TokenKind::Assign,
            TokenKind::Int(3),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn экранирование_в_строке() {
    let toks = lex(r#"s = "a\nb\t\\\"\{\}""#, "<тест>").expect("строка");
    assert_eq!(text_of(&toks[2]), "a\nb\t\\\"{}");
}

#[test]
fn подстановка_разбивается_на_части() {
    let parts = str_parts(r#""Привет, {msg.from.first_name}!""#);
    assert_eq!(parts.len(), 3);
    match &parts[0] {
        StrPart::Text(s) => assert_eq!(s, "Привет, "),
        _ => panic!("ожидался текст"),
    }
    match &parts[1] {
        StrPart::Expr { tokens, span } => {
            assert_eq!(tokens.len(), 5);
            assert_eq!(tokens[0].kind, TokenKind::Ident("msg".into()));
            assert_eq!(tokens[0].span, Span::new(1, 11, 3));
            assert_eq!(tokens[1].kind, TokenKind::Dot);
            assert_eq!(tokens[1].span, Span::new(1, 14, 1));
            assert_eq!(tokens[2].kind, TokenKind::Ident("from".into()));
            assert_eq!(*span, Span::new(1, 10, 1));
        }
        _ => panic!("ожидалось выражение"),
    }
    match &parts[2] {
        StrPart::Text(s) => assert_eq!(s, "!"),
        _ => panic!("ожидался текст"),
    }
}

#[test]
fn подстановка_внутри_вызова() {
    let toks = lex("send(\"a {x} b\")\n", "<тест>").expect("вызов с подстановкой");
    assert_eq!(toks[0].kind, TokenKind::Ident("send".into()));
    assert_eq!(toks[1].kind, TokenKind::LParen);
    assert_eq!(toks[3].kind, TokenKind::RParen);
    assert_eq!(toks[4].kind, TokenKind::Newline);
    assert_eq!(text_of(&toks[2]), "a <expr> b");
    assert_eq!(expr_of("\"a {x} b\"", 1)[0], TokenKind::Ident("x".into()));
}

#[test]
fn подстановка_внутри_списка_и_словаря() {
    let src = "v = [\"a {x}\", 2]\nw = {\"k\": \"b {y}\"}\n";
    let toks = lex(src, "<тест>").expect("список и словарь с подстановками");
    let strings: Vec<String> = toks
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::Str(_)))
        .map(text_of)
        .collect();
    assert_eq!(
        strings,
        vec![
            "a <expr>".to_string(),
            "k".to_string(),
            "b <expr>".to_string()
        ]
    );
    assert_eq!(expr_of("\"a {x}\"", 1)[0], TokenKind::Ident("x".into()));
    assert_eq!(expr_of("\"b {y}\"", 1)[0], TokenKind::Ident("y".into()));
    assert_eq!(
        toks.iter().filter(|t| t.kind == TokenKind::Newline).count(),
        2
    );
}

#[test]
fn скобки_подстановки_не_попадают_во_внешнюю_вложенность() {
    let toks = lex("a = \"{f(1)}\"\nb = 2\n", "<тест>").expect("подстановка с вызовом");
    assert_eq!(
        toks.iter().filter(|t| t.kind == TokenKind::Newline).count(),
        2
    );
    assert_eq!(toks.last().map(|t| t.kind.clone()), Some(TokenKind::Eof));
    assert_eq!(
        expr_of("\"{f(1)}\"", 0),
        vec![
            TokenKind::Ident("f".into()),
            TokenKind::LParen,
            TokenKind::Int(1),
            TokenKind::RParen
        ]
    );
}

#[test]
fn вложенные_скобки_в_подстановке() {
    assert_eq!(
        expr_of(r#""{f({1})}""#, 0),
        vec![
            TokenKind::Ident("f".into()),
            TokenKind::LParen,
            TokenKind::LBrace,
            TokenKind::Int(1),
            TokenKind::RBrace,
            TokenKind::RParen
        ]
    );
}

#[test]
fn словарь_внутри_подстановки() {
    assert_eq!(
        expr_of(r#""{f({"a": 1})}!""#, 0),
        vec![
            TokenKind::Ident("f".into()),
            TokenKind::LParen,
            TokenKind::LBrace,
            TokenKind::Str(vec![StrPart::Text("a".into())]),
            TokenKind::Colon,
            TokenKind::Int(1),
            TokenKind::RBrace,
            TokenKind::RParen
        ]
    );
}

#[test]
fn число_с_плавающей_точкой_в_подстановке() {
    assert_eq!(
        expr_of(r#""{p * 1.5}""#, 0),
        vec![
            TokenKind::Ident("p".into()),
            TokenKind::Star,
            TokenKind::Float(1.5)
        ]
    );
}

#[test]
fn индекс_внутри_подстановки() {
    assert_eq!(
        expr_of(r#""{m["a"]}""#, 0),
        vec![
            TokenKind::Ident("m".into()),
            TokenKind::LBracket,
            TokenKind::Str(vec![StrPart::Text("a".into())]),
            TokenKind::RBracket
        ]
    );
}

#[test]
fn несогласованная_скобка_в_подстановке() {
    let d = fail(r#"s = "{f(a]}"#);
    assert!(d.text.contains("в подстановке"), "{}", d.text);
    assert_eq!(d.span.col, 10);
}

#[test]
fn лишняя_скобка_в_подстановке() {
    let d = fail(r#"s = "{f(a))}""#);
    assert!(d.text.contains("лишняя закрывающая"), "{}", d.text);
}

#[test]
fn экранированная_фигурная_скобка() {
    let parts = str_parts(r#""\{x\}""#);
    assert_eq!(parts, vec![StrPart::Text("{x}".into())]);
}

#[test]
fn пустая_подстановка() {
    let d = fail(r#"s = "{}""#);
    assert!(d.text.contains("пустая подстановка"), "{}", d.text);
    assert_eq!(d.span.col, 6);
}

#[test]
fn незакрытая_подстановка() {
    let d = fail(r#"s = "a{b"#);
    assert!(d.text.contains("незакрытая подстановка"), "{}", d.text);
    assert_eq!(d.span.col, 7);
}

#[test]
fn незакрытая_строка() {
    let d = fail("s = \"abc\n");
    assert!(d.text.contains("незакрытая строка"), "{}", d.text);
    assert_eq!(d.span.col, 5);
}

#[test]
fn незакрытая_строка_в_подстановке() {
    let d = fail(r#"s = "{f("abc"#);
    assert!(
        d.text.contains("незакрытая строка в подстановке"),
        "{}",
        d.text
    );
}

#[test]
fn неизвестное_экранирование() {
    let d = fail(r#"s = "a\qb""#);
    assert!(
        d.text.contains("неизвестная escape-последовательность"),
        "{}",
        d.text
    );
    assert_eq!(d.span.col, 7);
}

#[test]
fn кириллица_в_строке() {
    let toks = lex("s = \"Привет, Женя\"\n", "<тест>").expect("строка");
    assert_eq!(text_of(&toks[2]), "Привет, Женя");
    assert_eq!(toks[2].span, Span::new(1, 5, 14));
}

#[test]
fn кириллица_в_идентификаторе() {
    let d = fail("имя = 1\n");
    assert!(d.text.contains("недопустимый символ 'и'"), "{}", d.text);
    assert_eq!(d.span.col, 1);
}

#[test]
fn колонки_считаются_по_символам() {
    let toks = lex("s = \"я\" + x\n", "<тест>").expect("строка");
    assert_eq!(toks[3].kind, TokenKind::Plus);
    assert_eq!(toks[3].span, Span::new(1, 9, 1));
    assert_eq!(toks[4].kind, TokenKind::Ident("x".into()));
    assert_eq!(toks[4].span, Span::new(1, 11, 1));
}

#[test]
fn позиции_токенов() {
    let toks = lex("ab = 1000\n", "<тест>").expect("строка");
    assert_eq!(toks[0].span, Span::new(1, 1, 2));
    assert_eq!(toks[1].span, Span::new(1, 4, 1));
    assert_eq!(toks[2].span, Span::new(1, 6, 4));
}

#[test]
fn комментарий_после_кода() {
    assert_eq!(
        kinds("x = 1 # пояснение\n"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Assign,
            TokenKind::Int(1),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
}

#[test]
fn ошибка_рисуется_с_подчёркиванием() {
    let src = "on start:\n    x = 1__0\n";
    let d = lex(src, "bot.vx").expect_err("ошибка");
    let want = concat!(
        "bot.vx:2:11: ошибка: неверная запись числа: '_' не может идти подряд\n",
        "  2 |     x = 1__0\n",
        "    |           ^"
    );
    assert_eq!(d.render(src), want);
}

#[test]
fn ошибка_в_первой_колонке() {
    let src = "\tb\n";
    let d = lex(src, "b.vx").expect_err("ошибка");
    let want = concat!(
        "b.vx:1:1: ошибка: табуляция недопустима в отступе: используй пробелы\n",
        "  1 |     b\n",
        "    | ^"
    );
    assert_eq!(d.render(src), want);
}

#[test]
fn ошибка_в_последней_колонке() {
    let src = "x = @\n";
    let d = lex(src, "b.vx").expect_err("ошибка");
    let want = concat!(
        "b.vx:1:5: ошибка: недопустимый символ '@'\n",
        "  1 | x = @\n",
        "    |     ^"
    );
    assert_eq!(d.render(src), want);
}

#[test]
fn ошибка_на_пустой_строке() {
    let src = "a\n\n";
    let d = Diag::new("b.vx", Span::new(2, 1, 1), "пустая строка");
    let want = concat!("b.vx:2:1: ошибка: пустая строка\n", "  2 | \n", "    | ^");
    assert_eq!(d.render(src), want);
}

#[test]
fn ошибка_с_нулевой_колонкой_не_падает() {
    let src = "a\n";
    let d = Diag::new("b.vx", Span::new(1, 0, 1), "нулевая колонка");
    let out = d.render(src);
    assert!(
        out.starts_with("b.vx:1:0: ошибка: нулевая колонка\n"),
        "{out}"
    );
    assert!(out.ends_with("^"), "{out}");
}

#[test]
fn ошибка_за_концом_строки_не_падает() {
    let src = "a\n";
    let d = Diag::new("b.vx", Span::new(1, 40, 3), "далеко");
    let out = d.render(src);
    assert_eq!(out.lines().count(), 3);
    assert!(out.ends_with("^"), "{out}");
}
