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
        let raw = src
            .split('\n')
            .nth(self.span.line as usize - 1)
            .unwrap_or("")
            .trim_end_matches('\r');
        let shown = raw.replace('\t', "    ");
        let before = self.span.col as usize - 1;
        let indent: String = raw
            .chars()
            .take(before)
            .map(|c| if c == '\t' { "    " } else { " " })
            .collect();
        let width = shown.chars().count().max(before + 1);
        let len = (self.span.len as usize).max(1);
        let carets = "^".repeat(len.min(width.saturating_sub(before)));
        format!("{head}\n  {num} | {shown}\n  {pad} | {indent}{carets}")
    }
}
