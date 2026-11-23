pub mod token;

#[cfg(test)]
mod tests;

use crate::diag::{Diag, Span};
use token::{Kw, StrPart, Token, TokenKind};

struct Scanner<'a> {
    file: &'a str,
    out: Vec<Token>,
    indents: Vec<u32>,
    depth: Vec<(char, u32, u32)>,
}

pub fn lex(src: &str, file: &str) -> Result<Vec<Token>, Diag> {
    let mut sc = Scanner {
        file,
        out: Vec::new(),
        indents: vec![0],
        depth: Vec::new(),
    };
    for (n, raw) in src.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        sc.line(n as u32 + 1, line)?;
    }
    if let Some((ch, ln, col)) = sc.depth.first() {
        return Err(sc.err(*ln, *col, 1, format!("незакрытая скобка '{ch}'")));
    }
    let last = src.split('\n').count().max(1) as u32;
    while sc.indents.len() > 1 {
        sc.indents.pop();
        sc.out
            .push(Token::new(TokenKind::Dedent, Span::new(last, 1, 1)));
    }
    sc.out
        .push(Token::new(TokenKind::Eof, Span::new(last, 1, 1)));
    Ok(sc.out)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Scanner<'_> {
    fn err(&self, line: u32, col: u32, len: u32, text: impl Into<String>) -> Diag {
        Diag::new(self.file, Span::new(line, col, len), text)
    }

    fn push(&mut self, kind: TokenKind, line: u32, col: u32, len: u32) {
        self.out.push(Token::new(kind, Span::new(line, col, len)));
    }

    fn line(&mut self, ln: u32, line: &str) -> Result<(), Diag> {
        let ch: Vec<char> = line.chars().collect();
        if let Some(pos) = ch.iter().position(|c| *c == '\r') {
            return Err(self.err(
                ln,
                pos as u32 + 1,
                1,
                "недопустимый символ возврата каретки",
            ));
        }
        let mut i = 0usize;
        let mut indent = 0u32;
        while i < ch.len() && ch[i] == ' ' {
            i += 1;
            indent += 1;
        }
        let blank = ch[i..].iter().all(|c| *c == ' ' || *c == '\t') || ch.get(i) == Some(&'#');
        if blank {
            return Ok(());
        }
        if ch[i] == '\t' {
            return Err(self.err(
                ln,
                i as u32 + 1,
                1,
                "табуляция недопустима в отступе: используй пробелы",
            ));
        }
        if self.depth.is_empty() {
            let cur = *self.indents.last().unwrap_or(&0);
            if indent > cur {
                self.indents.push(indent);
                self.push(TokenKind::Indent, ln, 1, indent);
            } else if indent < cur {
                while *self.indents.last().unwrap_or(&0) > indent {
                    self.indents.pop();
                    self.push(TokenKind::Dedent, ln, 1, 1);
                }
                let top = *self.indents.last().unwrap_or(&0);
                if top != indent {
                    return Err(self.err(
                        ln,
                        i as u32 + 1,
                        1,
                        format!(
                            "неверный отступ: {indent} пробелов, блок открыт с {top} пробелами"
                        ),
                    ));
                }
            }
        }
        while i < ch.len() {
            let c = ch[i];
            if c == ' ' {
                i += 1;
                continue;
            }
            if c == '#' {
                break;
            }
            if c == '\t' {
                return Err(self.err(
                    ln,
                    i as u32 + 1,
                    1,
                    "табуляция недопустима вне строки: используй пробелы",
                ));
            }
            let start = i;
            let kind = self.token(&ch, &mut i, ln)?;
            self.push(kind, ln, start as u32 + 1, (i - start) as u32);
        }
        if self.depth.is_empty() {
            let col = ch.len() as u32 + 1;
            self.push(TokenKind::Newline, ln, col, 1);
        }
        Ok(())
    }

    fn token(&mut self, ch: &[char], i: &mut usize, ln: u32) -> Result<TokenKind, Diag> {
        let col = *i as u32 + 1;
        let c = ch[*i];
        if c == '"' {
            return self.string(ch, i, ln);
        }
        if c.is_ascii_digit() {
            return self.number(ch, i, ln);
        }
        if is_ident_start(c) {
            return Ok(self.ident(ch, i));
        }
        let next = ch.get(*i + 1).copied();
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => {
                if next == Some('>') {
                    *i += 1;
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => TokenKind::Star,
            '/' => {
                if next == Some('/') {
                    *i += 1;
                    TokenKind::SlashSlash
                } else {
                    TokenKind::Slash
                }
            }
            '%' => TokenKind::Percent,
            '=' => {
                if next == Some('=') {
                    *i += 1;
                    TokenKind::Eq
                } else {
                    TokenKind::Assign
                }
            }
            '!' => {
                if next == Some('=') {
                    *i += 1;
                    TokenKind::Ne
                } else {
                    return Err(self.err(
                        ln,
                        col,
                        1,
                        "неизвестный символ '!': для сравнения используй '!='",
                    ));
                }
            }
            '<' => {
                if next == Some('=') {
                    *i += 1;
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if next == Some('=') {
                    *i += 1;
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            '?' => {
                if next == Some('?') {
                    *i += 1;
                    TokenKind::Coalesce
                } else {
                    return Err(self.err(
                        ln,
                        col,
                        1,
                        "неизвестный символ '?': для значения по умолчанию используй '??'",
                    ));
                }
            }
            '.' => TokenKind::Dot,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '(' => {
                self.depth.push(('(', ln, col));
                TokenKind::LParen
            }
            '[' => {
                self.depth.push(('[', ln, col));
                TokenKind::LBracket
            }
            '{' => {
                self.depth.push(('{', ln, col));
                TokenKind::LBrace
            }
            ')' => {
                self.close(')', ln, col)?;
                TokenKind::RParen
            }
            ']' => {
                self.close(']', ln, col)?;
                TokenKind::RBracket
            }
            '}' => {
                self.close('}', ln, col)?;
                TokenKind::RBrace
            }
            _ => {
                return Err(self.err(ln, col, 1, format!("недопустимый символ '{c}'")));
            }
        };
        *i += 1;
        Ok(kind)
    }

    fn close(&mut self, c: char, ln: u32, col: u32) -> Result<(), Diag> {
        let want = match c {
            ')' => '(',
            ']' => '[',
            _ => '{',
        };
        match self.depth.pop() {
            None => Err(self.err(ln, col, 1, format!("лишняя закрывающая скобка '{c}'"))),
            Some((open, oln, _)) if open != want => Err(self.err(
                ln,
                col,
                1,
                format!("закрывающая скобка '{c}' не закрывает открытую '{open}' со строки {oln}"),
            )),
            Some(_) => Ok(()),
        }
    }

    fn ident(&self, ch: &[char], i: &mut usize) -> TokenKind {
        let start = *i;
        while *i < ch.len() && is_ident_part(ch[*i]) {
            *i += 1;
        }
        let s: String = ch[start..*i].iter().collect();
        match s.as_str() {
            "on" => TokenKind::Kw(Kw::On),
            "state" => TokenKind::Kw(Kw::State),
            "if" => TokenKind::Kw(Kw::If),
            "elif" => TokenKind::Kw(Kw::Elif),
            "else" => TokenKind::Kw(Kw::Else),
            "while" => TokenKind::Kw(Kw::While),
            "for" => TokenKind::Kw(Kw::For),
            "in" => TokenKind::Kw(Kw::In),
            "break" => TokenKind::Kw(Kw::Break),
            "continue" => TokenKind::Kw(Kw::Continue),
            "func" => TokenKind::Kw(Kw::Func),
            "return" => TokenKind::Kw(Kw::Return),
            "and" => TokenKind::Kw(Kw::And),
            "or" => TokenKind::Kw(Kw::Or),
            "not" => TokenKind::Kw(Kw::Not),
            "bot" => TokenKind::Kw(Kw::Bot),
            "inline" => TokenKind::Kw(Kw::Inline),
            "menu" => TokenKind::Kw(Kw::Menu),
            "link" => TokenKind::Kw(Kw::Link),
            "switch" => TokenKind::Kw(Kw::Switch),
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            "null" => TokenKind::Null,
            _ => TokenKind::Ident(s),
        }
    }

    fn digits(&self, ch: &[char], i: &mut usize, text: &mut String, ln: u32) -> Result<(), Diag> {
        let mut under = false;
        while *i < ch.len() && (ch[*i].is_ascii_digit() || ch[*i] == '_') {
            if ch[*i] == '_' {
                if under {
                    return Err(self.err(
                        ln,
                        *i as u32 + 1,
                        1,
                        "неверная запись числа: '_' не может идти подряд",
                    ));
                }
                under = true;
            } else {
                under = false;
                text.push(ch[*i]);
            }
            *i += 1;
        }
        if under {
            return Err(self.err(
                ln,
                *i as u32,
                1,
                "неверная запись числа: '_' не может стоять в конце",
            ));
        }
        Ok(())
    }

    fn number(&mut self, ch: &[char], i: &mut usize, ln: u32) -> Result<TokenKind, Diag> {
        let start = *i;
        let mut text = String::new();
        self.digits(ch, i, &mut text, ln)?;
        let mut is_float = false;
        if *i < ch.len() && ch[*i] == '.' {
            if !ch.get(*i + 1).is_some_and(|c| c.is_ascii_digit()) {
                return Err(self.err(
                    ln,
                    *i as u32 + 1,
                    1,
                    "неверная запись числа: после точки нужна цифра",
                ));
            }
            is_float = true;
            text.push('.');
            *i += 1;
            self.digits(ch, i, &mut text, ln)?;
        }
        if *i < ch.len() && (ch[*i] == 'e' || ch[*i] == 'E') {
            let at = *i;
            let mut j = *i + 1;
            let mut exp = String::from("e");
            if j < ch.len() && (ch[j] == '+' || ch[j] == '-') {
                exp.push(ch[j]);
                j += 1;
            }
            let first = j;
            while j < ch.len() && ch[j].is_ascii_digit() {
                exp.push(ch[j]);
                j += 1;
            }
            if j == first {
                return Err(self.err(
                    ln,
                    at as u32 + 1,
                    1,
                    "неверная запись числа: после 'e' нужна степень",
                ));
            }
            text.push_str(&exp);
            is_float = true;
            *i = j;
        }
        if *i < ch.len() && (is_ident_part(ch[*i]) || ch[*i] == '.') {
            return Err(self.err(
                ln,
                *i as u32 + 1,
                1,
                format!("неверная запись числа: лишний символ '{}'", ch[*i]),
            ));
        }
        if is_float {
            return match text.parse::<f64>() {
                Ok(v) => Ok(TokenKind::Float(v)),
                Err(_) => Err(self.err(ln, start as u32 + 1, 1, "неверная запись числа")),
            };
        }
        match text.parse::<i64>() {
            Ok(v) => Ok(TokenKind::Int(v)),
            Err(_) => Err(self.err(
                ln,
                start as u32 + 1,
                (*i - start) as u32,
                "число слишком велико",
            )),
        }
    }

    fn string(&mut self, ch: &[char], i: &mut usize, ln: u32) -> Result<TokenKind, Diag> {
        let open = *i as u32 + 1;
        *i += 1;
        let mut parts: Vec<StrPart> = Vec::new();
        let mut buf = String::new();
        loop {
            if *i >= ch.len() {
                return Err(self.err(ln, open, 1, "незакрытая строка"));
            }
            let c = ch[*i];
            match c {
                '"' => {
                    *i += 1;
                    break;
                }
                '\\' => {
                    buf.push(self.escape(ch, i, ln)?);
                    *i += 2;
                }
                '{' => {
                    if !buf.is_empty() {
                        parts.push(StrPart::Text(std::mem::take(&mut buf)));
                    }
                    parts.push(self.substitution(ch, i, ln)?);
                }
                _ => {
                    buf.push(c);
                    *i += 1;
                }
            }
        }
        if !buf.is_empty() {
            parts.push(StrPart::Text(buf));
        }
        Ok(TokenKind::Str(parts))
    }

    fn escape(&self, ch: &[char], i: &mut usize, ln: u32) -> Result<char, Diag> {
        let col = *i as u32 + 1;
        match ch.get(*i + 1).copied() {
            Some('n') => Ok('\n'),
            Some('t') => Ok('\t'),
            Some('\\') => Ok('\\'),
            Some('"') => Ok('"'),
            Some('{') => Ok('{'),
            Some('}') => Ok('}'),
            Some(other) => Err(self.err(
                ln,
                col,
                2,
                format!("неизвестная escape-последовательность '\\{other}'"),
            )),
            None => Err(self.err(ln, col, 1, "escape-последовательность оборвана")),
        }
    }

    fn substitution(&mut self, ch: &[char], i: &mut usize, ln: u32) -> Result<StrPart, Diag> {
        let brace = *i as u32 + 1;
        *i += 1;
        let mut nest: Vec<char> = Vec::new();
        let mut toks = Vec::new();
        loop {
            if *i >= ch.len() {
                return Err(self.err(ln, brace, 1, "незакрытая подстановка: нет '}'"));
            }
            let c = ch[*i];
            if c == ' ' {
                *i += 1;
                continue;
            }
            if c == '\t' {
                return Err(self.err(ln, *i as u32 + 1, 1, "табуляция недопустима в подстановке"));
            }
            if c == '}' && nest.is_empty() {
                *i += 1;
                break;
            }
            let start = *i;
            let kind = self.sub_token(&mut nest, ch, i, ln)?;
            toks.push(Token::new(
                kind,
                Span::new(ln, start as u32 + 1, (*i - start) as u32),
            ));
        }
        if toks.is_empty() {
            return Err(self.err(ln, brace, 1, "пустая подстановка '{}'"));
        }
        Ok(StrPart::Expr {
            tokens: toks,
            span: Span::new(ln, brace, 1),
        })
    }

    fn sub_token(
        &mut self,
        nest: &mut Vec<char>,
        ch: &[char],
        i: &mut usize,
        ln: u32,
    ) -> Result<TokenKind, Diag> {
        let col = *i as u32 + 1;
        let c = ch[*i];
        let pair = ch.get(*i + 1).copied();
        let kind = match c {
            '"' => return self.sub_string(ch, i, ln),
            '(' | '[' | '{' => {
                nest.push(c);
                match c {
                    '(' => TokenKind::LParen,
                    '[' => TokenKind::LBracket,
                    _ => TokenKind::LBrace,
                }
            }
            ')' | ']' | '}' => {
                let want = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                match nest.pop() {
                    Some(open) if open == want => match c {
                        ')' => TokenKind::RParen,
                        ']' => TokenKind::RBracket,
                        _ => TokenKind::RBrace,
                    },
                    Some(open) => {
                        return Err(self.err(
                            ln,
                            col,
                            1,
                            format!("в подстановке '{c}' не закрывает открытую '{open}'"),
                        ))
                    }
                    None => {
                        return Err(self.err(
                            ln,
                            col,
                            1,
                            format!("в подстановке лишняя закрывающая скобка '{c}'"),
                        ))
                    }
                }
            }
            '+' => TokenKind::Plus,
            '-' => {
                if pair == Some('>') {
                    *i += 1;
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => TokenKind::Star,
            '/' => {
                if pair == Some('/') {
                    *i += 1;
                    TokenKind::SlashSlash
                } else {
                    TokenKind::Slash
                }
            }
            '%' => TokenKind::Percent,
            '=' => {
                if pair == Some('=') {
                    *i += 1;
                    TokenKind::Eq
                } else {
                    TokenKind::Assign
                }
            }
            '!' if pair == Some('=') => {
                *i += 1;
                TokenKind::Ne
            }
            '?' if pair == Some('?') => {
                *i += 1;
                TokenKind::Coalesce
            }
            '<' => {
                if pair == Some('=') {
                    *i += 1;
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if pair == Some('=') {
                    *i += 1;
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            '.' => TokenKind::Dot,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            _ if c.is_ascii_digit() => return self.number(ch, i, ln),
            _ if is_ident_start(c) => {
                let start = *i;
                while *i < ch.len() && is_ident_part(ch[*i]) {
                    *i += 1;
                }
                let word: String = ch[start..*i].iter().collect();
                return Ok(match word.as_str() {
                    "true" => TokenKind::Bool(true),
                    "false" => TokenKind::Bool(false),
                    "null" => TokenKind::Null,
                    _ => TokenKind::Ident(word),
                });
            }
            _ => {
                return Err(self.err(ln, col, 1, format!("недопустимый символ '{c}'")));
            }
        };
        *i += 1;
        Ok(kind)
    }

    fn sub_string(&mut self, ch: &[char], i: &mut usize, ln: u32) -> Result<TokenKind, Diag> {
        let open = *i as u32 + 1;
        *i += 1;
        let mut buf = String::new();
        loop {
            if *i >= ch.len() {
                return Err(self.err(ln, open, 1, "незакрытая строка в подстановке"));
            }
            match ch[*i] {
                '"' => {
                    *i += 1;
                    break;
                }
                '\\' => {
                    buf.push(self.escape(ch, i, ln)?);
                    *i += 2;
                }
                c => {
                    buf.push(c);
                    *i += 1;
                }
            }
        }
        Ok(TokenKind::Str(vec![StrPart::Text(buf)]))
    }
}
