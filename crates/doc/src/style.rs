//! Inline character styles (bold, italic, underline) as byte-range spans over paragraph text.

use serde::{Deserialize, Serialize};

/// A character style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
}

/// Which style flag a command changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Bold,
    Italic,
    Underline,
}

impl Style {
    pub const PLAIN: Style = Style { bold: false, italic: false, underline: false };
    pub fn is_plain(&self) -> bool {
        *self == Style::PLAIN
    }
    pub fn get(&self, f: Flag) -> bool {
        match f {
            Flag::Bold => self.bold,
            Flag::Italic => self.italic,
            Flag::Underline => self.underline,
        }
    }
    pub fn with(mut self, f: Flag, on: bool) -> Style {
        match f {
            Flag::Bold => self.bold = on,
            Flag::Italic => self.italic = on,
            Flag::Underline => self.underline = on,
        }
        self
    }
}

/// A styled byte range `[start, end)` of a paragraph's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub style: Style,
}

/// Per-byte styles from spans (`len` bytes, plain by default).
pub fn expand(spans: &[Span], len: usize) -> Vec<Style> {
    let mut v = vec![Style::PLAIN; len];
    for s in spans {
        let (a, b) = (s.start.min(len), s.end.min(len));
        if let Some(slice) = v.get_mut(a..b.max(a)) {
            for x in slice {
                *x = s.style;
            }
        }
    }
    v
}

/// Spans from per-byte styles over `text` (boundaries snapped to char starts; each char takes the
/// style of its first byte). Only non-plain runs are kept, adjacent equal runs merged.
pub fn compress(text: &str, styles: &[Style]) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (i, c) in text.char_indices() {
        let st = styles.get(i).copied().unwrap_or_default();
        let end = i + c.len_utf8();
        match out.last_mut() {
            Some(last) if last.end == i && last.style == st => last.end = end,
            _ if st.is_plain() => {}
            _ => out.push(Span { start: i, end, style: st }),
        }
    }
    out
}

/// Uppercase `text`, carrying each char's style onto the characters it becomes.
pub fn upper_with(text: &str, styles: &[Style]) -> (String, Vec<Style>) {
    let mut s = String::with_capacity(text.len());
    let mut v = Vec::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        let st = styles.get(i).copied().unwrap_or_default();
        let before = s.len();
        s.extend(c.to_uppercase());
        v.extend(std::iter::repeat_n(st, s.len() - before));
    }
    (s, v)
}

/// Clamp spans to `text`: inside its length, on char boundaries, non-empty, sorted, merged.
pub fn fix(text: &str, spans: &[Span]) -> Vec<Span> {
    compress(text, &expand(spans, text.len()))
}
