#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub len: u32,
}

impl Span {
    pub fn new(line: u32, col: u32, len: u32) -> Span {
        Span { line, col, len }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub file: String,
    pub span: Span,
    pub text: String,
}

const TAB_WIDTH: usize = 4;

impl Diag {
    pub fn new(file: &str, span: Span, text: impl Into<String>) -> Diag {
        Diag {
            file: file.to_string(),
            span,
            text: text.into(),
        }
    }

    pub fn render(&self, src: &str) -> String {
        let num = self.span.line.to_string();
        let pad = " ".repeat(num.len());
        let head = format!(
            "{}:{}:{}: ошибка: {}",
            self.file, self.span.line, self.span.col, self.text
        );
        let index = (self.span.line as usize).saturating_sub(1);
        let raw = src
            .split('\n')
            .nth(index)
            .unwrap_or("")
            .trim_end_matches('\r');
        let shown = expand(raw);
        let before = self.span.col.saturating_sub(1) as usize;
        let indent = " ".repeat(display_width(&raw.chars().take(before).collect::<String>()));
        let left = (display_width(&shown) + 1).saturating_sub(indent.len());
        let carets = "^".repeat((self.span.len as usize).max(1).min(left).max(1));
        format!("{head}\n  {num} | {shown}\n  {pad} | {indent}{carets}")
    }
}

fn expand(raw: &str) -> String {
    raw.replace('\t', &" ".repeat(TAB_WIDTH))
}

fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| if c == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}
