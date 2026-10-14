//! Shared helpers that turn positioned or styled text into screenplay paragraphs: styled text,
//! element-by-indent rules (Courier 12 conventions), scene-heading and shot detection, and the
//! title-page mapping.

use scriptcraft_doc::{Element, Para, Span, Style, TitlePage};

/// Text with a style per character.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Styled(pub Vec<(char, Style)>);

impl Styled {
    #[cfg(test)]
    pub fn plain(s: &str) -> Styled {
        Styled(s.chars().map(|c| (c, Style::PLAIN)).collect())
    }
    pub fn text(&self) -> String {
        self.0.iter().map(|(c, _)| *c).collect()
    }
    pub fn is_blank(&self) -> bool {
        self.0.iter().all(|(c, _)| c.is_whitespace())
    }
    pub fn push(&mut self, c: char, st: Style) {
        self.0.push((c, st));
    }
    pub fn push_str(&mut self, s: &str, st: Style) {
        self.0.extend(s.chars().map(|c| (c, st)));
    }
    /// Collapse runs of whitespace (tabs, newlines, no-break spaces) to one space; trim the ends.
    pub fn clean(&self) -> Styled {
        let mut out: Vec<(char, Style)> = Vec::with_capacity(self.0.len());
        for &(c, st) in &self.0 {
            if c.is_whitespace() || c == '\u{a0}' {
                if out.last().is_some_and(|(p, _)| *p != ' ') {
                    out.push((' ', Style::PLAIN));
                }
            } else if !c.is_control() {
                out.push((c, st));
            }
        }
        while out.last().is_some_and(|(c, _)| *c == ' ') {
            out.pop();
        }
        Styled(out)
    }
    /// Append a continuation line: joined with a space, or directly after a word-hyphen.
    pub fn join(&mut self, more: &Styled) {
        let last = self.0.last().map(|(c, _)| *c);
        let before = self.0.len().checked_sub(2).and_then(|i| self.0.get(i)).map(|(c, _)| *c);
        let hyphen = last == Some('-') && before.is_some_and(char::is_alphabetic) && more.0.first().is_some_and(|(c, _)| c.is_alphabetic());
        if !self.0.is_empty() && !hyphen && last != Some(' ') {
            self.0.push((' ', Style::PLAIN));
        }
        self.0.extend(more.0.iter().copied());
    }
    /// Remove a trailing suffix (case-insensitive, ignoring trailing spaces).
    pub fn strip_suffix_ci(&mut self, suffix: &str) -> bool {
        let t = self.text();
        let trimmed = t.trim_end();
        let n = suffix.chars().count();
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() < n {
            return false;
        }
        let tail: String = chars.iter().skip(chars.len() - n).collect();
        if !tail.eq_ignore_ascii_case(suffix) {
            return false;
        }
        self.0.truncate(chars.len() - n);
        while self.0.last().is_some_and(|(c, _)| *c == ' ') {
            self.0.pop();
        }
        true
    }
    /// A paragraph of this text.
    pub fn para(&self, element: Element) -> Para {
        let mut p = Para::new(element, String::new());
        let mut text = String::new();
        let mut spans: Vec<Span> = Vec::new();
        for &(c, st) in &self.0 {
            let start = text.len();
            text.push(c);
            if st.is_plain() {
                continue;
            }
            match spans.last_mut() {
                Some(s) if s.end == start && s.style == st => s.end = text.len(),
                _ => spans.push(Span { start, end: text.len(), style: st }),
            }
        }
        p.text = text;
        p.spans = spans;
        p
    }
}

/// Character cues repeated on a continued page carry one of these; they are dropped (ScriptCraft
/// adds its own).
pub const CONTD: [&str; 6] = ["(CONT'D)", "(CONT’D)", "(CONTD)", "(CONT.)", "(CONTINUING)", "(CONT)"];

/// Remove a continued-cue suffix.
pub fn strip_contd(t: &mut Styled) {
    for s in CONTD {
        if t.strip_suffix_ci(s) {
            return;
        }
    }
}

/// Lines that only mark a page break in a printed script.
pub fn is_break_marker(t: &str) -> bool {
    let u = t.trim().to_uppercase();
    matches!(u.as_str(), "(MORE)" | "(CONTINUED)" | "CONTINUED:" | "CONTINUED" | "(CONTINUED:)" | "- MORE -")
        || (u.starts_with("CONTINUED:") && u.len() <= 18 && u.contains('('))
}

/// A scene heading (INT./EXT. and friends).
pub fn is_heading(t: &str) -> bool {
    let u = t.trim_start().to_uppercase();
    const P: [&str; 14] = ["INT.", "EXT.", "INT ", "EXT ", "INT/", "EXT/", "I/E", "E/I", "EST.", "EST ", "INTERIOR ", "EXTERIOR ", "INT.-", "EXT.-"];
    P.iter().any(|p| u.starts_with(p))
}

/// An uppercase camera direction at the action margin.
pub fn is_shot(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() || t.chars().any(char::is_lowercase) || t.len() > 60 || t.ends_with('.') {
        return false;
    }
    const P: [&str; 12] =
        ["CLOSE ON", "CLOSE UP", "CLOSE-UP", "ANGLE ON", "ANGLE -", "WIDE ON", "WIDE SHOT", "POV", "INSERT", "BACK TO", "EXTREME CLOSE", "TRACKING"];
    P.iter().any(|p| t.starts_with(p))
}

/// Mostly uppercase (letters only), with at least one letter.
pub fn is_caps(t: &str) -> bool {
    let letters: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    !letters.is_empty() && letters.iter().all(|c| !c.is_lowercase())
}

/// A transition written at any margin.
pub fn is_transition_text(t: &str) -> bool {
    let t = t.trim();
    is_caps(t) && (t.ends_with("TO:") || t == "FADE OUT." || t == "FADE TO BLACK." || t == "FADE OUT" || t == "THE END")
}

/// A scene number as printed in the margins: `12`, `12A`, `A12`, `12.`.
pub fn is_scene_number(t: &str) -> bool {
    let t = t.trim().trim_end_matches('.');
    let digits = t.chars().filter(char::is_ascii_digit).count();
    !t.is_empty()
        && t.len() <= 6
        && digits >= 1
        && t.chars().all(|c| c.is_ascii_alphanumeric())
        && t.chars().filter(|c| c.is_ascii_alphabetic()).count() <= 2
}

/// Page geometry for indent rules, in inches from the page's left edge.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    /// The action / scene-heading margin.
    pub base: f32,
    /// The right edge of the text block (where transitions end).
    pub right: f32,
}

/// The element for a line by its indent (Courier 12: dialogue +1.0", parenthetical +1.6",
/// character +2.2", transitions right-aligned), refined by its text.
pub fn by_indent(x: f32, x_end: f32, g: Geometry, text: &str) -> Element {
    let d = x - g.base;
    let t = text.trim();
    if d < 0.5 {
        return if is_heading(t) {
            Element::SceneHeading
        } else if is_transition_text(t) && t.ends_with("TO:") {
            Element::Transition
        } else if is_shot(t) {
            Element::Shot
        } else {
            Element::Action
        };
    }
    let right_aligned = x_end >= g.right - 0.25 && d > 2.5;
    if d >= 3.6 || (right_aligned && (is_caps(t) || t.ends_with(':'))) {
        return Element::Transition;
    }
    if d >= 1.95 {
        return if is_caps(t) { Element::Character } else { Element::Dialogue };
    }
    if d >= 1.35 || (d >= 1.2 && t.starts_with('(')) {
        return Element::Parenthetical;
    }
    Element::Dialogue
}

/// The most common left edge at or near the smallest commonly used one: the action margin.
pub fn base_margin(xs: &[f32]) -> f32 {
    if xs.is_empty() {
        return 1.5;
    }
    let mut buckets: Vec<(i32, usize)> = Vec::new();
    for &x in xs {
        let k = (x * 20.0).round() as i32;
        match buckets.iter_mut().find(|(b, _)| *b == k) {
            Some((_, n)) => *n += 1,
            None => buckets.push((k, 1)),
        }
    }
    buckets.sort_unstable();
    let need = (xs.len() / 20).max(2).min(xs.len());
    buckets.iter().find(|(_, n)| *n >= need).or(buckets.first()).map_or(1.5, |(k, _)| *k as f32 / 20.0)
}

/// Title-page text in reading order: centred blocks (split at blank lines), the bottom-left block
/// and the bottom-right block.
#[derive(Clone, Debug, Default)]
pub struct TitleBlocks {
    pub centered: Vec<Vec<String>>,
    pub left: Vec<String>,
    pub right: Vec<String>,
}

fn is_credit(s: &str) -> bool {
    let l = s.trim().to_lowercase();
    l == "by" || l.ends_with(" by") || l.starts_with("written by") || l.starts_with("screenplay by") || l.starts_with("teleplay by")
}

/// Map title-page blocks onto the title-page fields.
pub fn title_from(b: &TitleBlocks) -> TitlePage {
    let mut t = TitlePage::default();
    let mut groups = b.centered.iter().filter(|g| !g.is_empty());
    if let Some(g) = groups.next() {
        t.title = g.join("\n");
    }
    let rest: Vec<&Vec<String>> = groups.collect();
    let credit_at = rest.iter().position(|g| g.first().is_some_and(|l| is_credit(l)));
    let mut source: Vec<String> = Vec::new();
    for (i, g) in rest.iter().enumerate() {
        match credit_at {
            Some(c) if i == c => {
                // "Written by" may share its block with the names below it.
                t.credit = g.first().cloned().unwrap_or_default();
                if g.len() > 1 {
                    t.author = g.get(1..).unwrap_or(&[]).join("\n");
                }
            }
            Some(c) if i == c + 1 && t.author.is_empty() => t.author = g.join("\n"),
            None if i == 0 => t.author = g.join("\n"),
            _ => source.push(g.join("\n")),
        }
    }
    t.source = source.join("\n");
    let mut contact = Vec::new();
    for l in &b.left {
        let lower = l.to_lowercase();
        if t.copyright.is_empty() && (l.contains('©') || lower.starts_with("copyright") || lower.starts_with("(c)")) {
            t.copyright = l.clone();
        } else {
            contact.push(l.clone());
        }
    }
    t.contact = contact.join("\n");
    t.draft_date = b.right.join("\n");
    t
}

/// Paragraph assembly: one paragraph per call to [`Builder::start`], continued with
/// [`Builder::cont`].
#[derive(Default)]
pub struct Builder {
    pub paras: Vec<(Element, Styled, bool, Option<String>)>,
}

impl Builder {
    pub fn start(&mut self, e: Element, t: Styled) {
        self.paras.push((e, t, false, None));
    }
    pub fn cont(&mut self, t: &Styled) {
        if let Some(last) = self.paras.last_mut() {
            last.1.join(t);
        }
    }
    pub fn last_element(&self) -> Option<Element> {
        self.paras.last().map(|p| p.0)
    }
    /// The finished paragraphs (blank ones dropped).
    pub fn finish(self) -> Vec<Para> {
        self.paras
            .into_iter()
            .filter_map(|(e, t, dual, num)| {
                let t = t.clean();
                if t.is_blank() {
                    return None;
                }
                let mut p = t.para(e);
                p.dual = dual && e == Element::Character;
                if e == Element::SceneHeading {
                    p.scene_number = num;
                }
                Some(p)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indents_map_to_elements() {
        let g = Geometry { base: 1.5, right: 7.5 };
        assert_eq!(by_indent(1.5, 4.0, g, "INT. KITCHEN - DAY"), Element::SceneHeading);
        assert_eq!(by_indent(1.5, 7.4, g, "She waits."), Element::Action);
        assert_eq!(by_indent(3.7, 4.2, g, "NELL"), Element::Character);
        assert_eq!(by_indent(2.5, 6.0, g, "Hello there."), Element::Dialogue);
        assert_eq!(by_indent(3.1, 4.0, g, "(quietly)"), Element::Parenthetical);
        assert_eq!(by_indent(6.3, 7.5, g, "CUT TO:"), Element::Transition);
        assert_eq!(by_indent(1.5, 4.0, g, "CLOSE ON THE BAROMETER"), Element::Shot);
    }

    #[test]
    fn title_blocks_map_to_fields() {
        let b = TitleBlocks {
            centered: vec![vec!["NIGHT SIGNAL".into()], vec!["Written by".into()], vec!["Ann Lee".into()], vec!["Based on a story".into()]],
            left: vec!["Copyright 2026".into(), "ann@example.com".into()],
            right: vec!["October 2026".into()],
        };
        let t = title_from(&b);
        assert_eq!(
            (t.title.as_str(), t.credit.as_str(), t.author.as_str(), t.source.as_str()),
            ("NIGHT SIGNAL", "Written by", "Ann Lee", "Based on a story")
        );
        assert_eq!((t.copyright.as_str(), t.contact.as_str(), t.draft_date.as_str()), ("Copyright 2026", "ann@example.com", "October 2026"));
    }

    #[test]
    fn styled_text_keeps_spans() {
        let mut s = Styled::plain("Hello ");
        s.push_str("world", Style { bold: true, ..Style::PLAIN });
        let p = s.para(Element::Action);
        assert_eq!(p.text, "Hello world");
        assert_eq!((p.spans.len(), p.spans[0].start, p.spans[0].end), (1, 6, 11));
        let mut c = Styled::plain("NELL (CONT'D)");
        strip_contd(&mut c);
        assert_eq!(c.text(), "NELL");
        let mut j = Styled::plain("well-");
        j.join(&Styled::plain("known"));
        assert_eq!(j.text(), "well-known");
    }
}
