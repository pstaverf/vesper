use vesper::lexer::lex;
use vesper::lexer::token::{Kw, StrPart, Token, TokenKind};

fn lex_ok(name: &str, src: &str) -> Vec<Token> {
    match lex(src, name) {
        Ok(t) => t,
        Err(d) => panic!("{name}: {}\n{}", d.text, d.render(src)),
    }
}

fn strings(toks: &[Token]) -> Vec<String> {
    toks.iter()
        .filter_map(|t| match &t.kind {
            TokenKind::Str(parts) => Some(
                parts
                    .iter()
                    .map(|p| match p {
                        StrPart::Text(s) => s.clone(),
                        StrPart::Expr { .. } => "<expr>".to_string(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

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

#[test]
fn все_примеры_лексируются_сбалансированно() {
    for (name, src) in EXAMPLES {
        let toks = lex_ok(name, src);
        assert!(toks.len() > 10, "{name}: слишком мало токенов");
        assert_eq!(toks.last().map(|t| t.kind.clone()), Some(TokenKind::Eof));
        assert_eq!(
            toks.iter().filter(|t| t.kind == TokenKind::Eof).count(),
            1,
            "{name}: Eof должен быть один"
        );
        let indents = toks.iter().filter(|t| t.kind == TokenKind::Indent).count();
        let dedents = toks.iter().filter(|t| t.kind == TokenKind::Dedent).count();
        assert_eq!(indents, dedents, "{name}: Indent/Dedent не сбалансированы");
        assert!(indents > 0, "{name}: блоков быть не должно ноль");
    }
}

#[test]
fn пример_кнопок_без_подстановок() {
    let toks = lex_ok("examples/02_buttons.vx", EXAMPLES[1].1);
    let found = toks.iter().any(|t| match &t.kind {
        TokenKind::Str(parts) => parts.iter().any(|p| matches!(p, StrPart::Expr { .. })),
        _ => false,
    });
    assert!(!found, "в 02_buttons.vx подстановок быть не должно");
    assert!(strings(&toks).contains(&"buy:coffee".to_string()));
}

#[test]
fn пример_старта_подставляет_имя() {
    let toks = lex_ok("examples/01_start.vx", EXAMPLES[0].1);
    let parts: Vec<_> = toks
        .iter()
        .filter_map(|t| match &t.kind {
            TokenKind::Str(p) => Some(p.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(parts.iter().any(|p| matches!(p, StrPart::Expr { .. })));
    assert!(strings(&toks).iter().any(|s| s.starts_with("Привет, ")));
}

#[test]
fn пример_фото_видит_поля_объекта() {
    let toks = lex_ok("examples/05_photo.vx", EXAMPLES[4].1);
    assert!(strings(&toks).contains(&"Твоё фото, <expr>".to_string()));
}

#[test]
fn пример_состояний_видит_ключевое_слово_state() {
    let toks = lex_ok("examples/03_states.vx", EXAMPLES[2].1);
    assert_eq!(
        toks.iter()
            .filter(|t| t.kind == TokenKind::Kw(Kw::State))
            .count(),
        2
    );
}

#[test]
fn пример_callback_видит_данные_кнопки() {
    let toks = lex_ok("examples/04_callback_edit.vx", EXAMPLES[3].1);
    assert!(strings(&toks).contains(&"inc".to_string()));
}

#[test]
fn примеры_дают_одинаковый_результат_для_crlf() {
    for (name, src) in EXAMPLES {
        let a = lex_ok(name, src);
        let crlf = src.replace('\n', "\r\n");
        let b = lex_ok(name, &crlf);
        assert_eq!(a.len(), b.len(), "{name}: разное число токенов в CRLF");
        let ka: Vec<_> = a.iter().map(|t| t.kind.clone()).collect();
        let kb: Vec<_> = b.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(ka, kb, "{name}: CRLF изменил токены");
    }
}

#[test]
fn битый_пример_даёт_ошибку_с_позицией() {
    let src = "on command \"start\"\n    \"abc\n";
    let d = lex(src, "bad.vx").expect_err("ожидалась ошибка строки");
    let out = d.render(src);
    assert!(
        out.contains("bad.vx:2:5: ошибка: незакрытая строка"),
        "{out}"
    );
    assert!(out.contains("    |     ^"), "{out}");
}
