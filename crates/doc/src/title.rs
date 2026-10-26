//! Free-form title pages.
//!
//! A title page is normally its fields (title, credit, author…) laid out in the usual
//! arrangement. Final Draft stores a title page as free-form paragraphs, and some don't fit the
//! fields: several contact blocks, text in unusual places, underlined titles. Those keep their
//! paragraphs as [`TitleLine`]s ([`TitlePage::lines`]): they are printed and saved as they are,
//! and the fields are derived from them ([`TitlePage::fields_from_lines`]) for headers,
//! Fountain and reports.

use serde::{Deserialize, Serialize};

use crate::{Align, MAX_PARA_BYTES, Style, TitlePage};

/// Most lines a free-form title page keeps.
pub const MAX_TITLE_LINES: usize = 200;

/// One paragraph of a free-form title page (an empty text is a blank line).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleLine {
    pub align: Align,
    pub text: String,
    /// The whole line's style (serialized as `bold`, `italic`, `underline`).
    #[serde(flatten)]
    pub style: Style,
}

impl Default for TitleLine {
    fn default() -> Self {
        TitleLine { align: Align::Center, text: String::new(), style: Style::PLAIN }
    }
}

impl TitleLine {
    pub fn new(align: Align, text: &str) -> Self {
        TitleLine { align, text: clean_line(text), style: Style::PLAIN }
    }
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// One line of text: no control characters or line breaks, capped.
fn clean_line(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = if c == '\t' { ' ' } else { c };
        if c.is_control() {
            continue;
        }
        if out.len() + c.len_utf8() > MAX_PARA_BYTES {
            break;
        }
        out.push(c);
    }
    out
}

fn is_credit(l: &str) -> bool {
    let l = l.trim().to_lowercase();
    l == "by"
        || l.ends_with(" by")
        || l.starts_with("written by")
        || l.starts_with("screenplay by")
        || l.starts_with("teleplay by")
        || l.starts_with("story by")
}

impl TitlePage {
    /// A free-form title page: its lines are what prints.
    pub fn is_free_form(&self) -> bool {
        self.lines.iter().any(|l| !l.is_blank())
    }

    /// Map free-form lines onto fields: the first centred line is the title, a "written by"
    /// line the credit, the next centred lines the author, later centred lines the source;
    /// left-aligned lines are the contact block, right-aligned ones the draft date.
    pub fn fields_from_lines(lines: &[TitleLine]) -> TitlePage {
        let mut t = TitlePage::default();
        let add = |slot: &mut String, v: &str| {
            if !slot.is_empty() {
                slot.push('\n');
            }
            slot.push_str(v);
        };
        let mut stage = 0; // 0 title, 1 credit, 2 author, 3 source
        for l in lines {
            let v = l.text.trim();
            if v.is_empty() {
                continue;
            }
            match l.align {
                Align::Right => add(&mut t.draft_date, v),
                Align::Left => add(&mut t.contact, v),
                Align::Center => match stage {
                    0 => {
                        if is_credit(v) {
                            t.credit = v.to_string();
                            stage = 2;
                        } else {
                            add(&mut t.title, v);
                            stage = 1;
                        }
                    }
                    1 if is_credit(v) => {
                        t.credit = v.to_string();
                        stage = 2;
                    }
                    1 | 2 => {
                        add(&mut t.author, v);
                        stage = 3;
                    }
                    _ => add(&mut t.source, v),
                },
            }
        }
        t
    }

    /// The fields in the usual arrangement, as lines with the field each came from (`None`
    /// for spacing): 12 blank lines, title, credit and author, source, then the contact block,
    /// copyright and notes on the left and the draft date on the right.
    pub fn field_lines(&self) -> Vec<(TitleLine, Option<&'static str>)> {
        let mut out = Vec::new();
        let blank = |out: &mut Vec<(TitleLine, Option<&'static str>)>, n: usize| {
            for _ in 0..n {
                out.push((TitleLine::new(Align::Center, ""), None));
            }
        };
        let field = |out: &mut Vec<(TitleLine, Option<&'static str>)>, align: Align, id: &'static str, v: &str| {
            for l in v.lines().filter(|l| !l.trim().is_empty()) {
                out.push((TitleLine::new(align, l.trim()), Some(id)));
            }
        };
        blank(&mut out, 12);
        field(&mut out, Align::Center, "title", &self.title);
        blank(&mut out, 1);
        field(&mut out, Align::Center, "credit", &self.credit);
        field(&mut out, Align::Center, "author", &self.author);
        blank(&mut out, 1);
        field(&mut out, Align::Center, "source", &self.source);
        blank(&mut out, 16);
        field(&mut out, Align::Left, "contact", &self.contact);
        field(&mut out, Align::Left, "copyright", &self.copyright);
        field(&mut out, Align::Left, "notes", &self.notes);
        field(&mut out, Align::Right, "draftDate", &self.draft_date);
        out
    }

    /// A title page read from free-form lines: the fields when they hold everything the lines
    /// say (laid out the usual way they give the same lines, ignoring spacing), otherwise the
    /// lines kept as they are, with fields derived from them.
    pub fn from_lines(lines: Vec<TitleLine>) -> TitlePage {
        let t = Self::fields_from_lines(&lines);
        let shown = |v: &mut dyn Iterator<Item = &TitleLine>| -> Vec<(Align, String, Style)> {
            v.filter(|l| !l.is_blank()).map(|l| (l.align, l.text.trim().to_string(), l.style)).collect()
        };
        let regenerated = t.field_lines();
        if shown(&mut lines.iter()) == shown(&mut regenerated.iter().map(|(l, _)| l)) {
            return t;
        }
        let mut t = TitlePage { lines, ..t }.with_lines_cleaned();
        while t.lines.last().is_some_and(TitleLine::is_blank) {
            t.lines.pop();
        }
        t
    }

    /// Set free-form lines (and the fields derived from them). No text on any line: back to
    /// fields (blank lines alone are no title page).
    pub fn set_lines(&mut self, lines: Vec<TitleLine>) {
        let fields = Self::fields_from_lines(&lines);
        *self = TitlePage { lines, ..fields }.with_lines_cleaned();
        if !self.is_free_form() {
            self.lines.clear();
        }
    }

    /// Turn a field title page into free-form lines laid out the usual way (to edit freely).
    pub fn make_free_form(&mut self) {
        if !self.is_free_form() {
            self.lines = self.field_lines().into_iter().map(|(l, _)| l).collect();
            self.clean_lines();
            if !self.is_free_form() {
                self.lines.clear();
            }
        }
    }

    /// Drop the free-form lines; the (derived) fields remain.
    pub fn use_fields(&mut self) {
        self.lines.clear();
    }

    fn with_lines_cleaned(mut self) -> Self {
        self.clean_lines();
        self
    }

    /// Clean each line and cap the count.
    pub fn clean_lines(&mut self) {
        self.lines.truncate(MAX_TITLE_LINES);
        for l in &mut self.lines {
            l.text = clean_line(&l.text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(a: Align, t: &str) -> TitleLine {
        TitleLine::new(a, t)
    }

    #[test]
    fn usual_title_pages_become_fields() {
        let mut t = TitlePage::default();
        t.title = "NIGHT SIGNAL".into();
        t.credit = "Written by".into();
        t.author = "Ada Lane".into();
        t.contact = "ada@example.com".into();
        t.draft_date = "May 2026".into();
        let lines: Vec<TitleLine> = t.field_lines().into_iter().map(|(l, _)| l).collect();
        let back = TitlePage::from_lines(lines);
        assert!(!back.is_free_form());
        assert_eq!(back, t);
    }

    #[test]
    fn unusual_title_pages_keep_their_lines() {
        let lines = vec![
            l(Align::Left, "Agent: Bo Reyes"),
            l(Align::Center, ""),
            TitleLine { style: Style { underline: true, ..Style::PLAIN }, ..l(Align::Center, "NIGHT SIGNAL") },
            l(Align::Center, "by"),
            l(Align::Center, "Ada Lane"),
            l(Align::Right, "Draft 3"),
            l(Align::Left, "WGA Reg. 123"),
            l(Align::Center, ""),
        ];
        let t = TitlePage::from_lines(lines.clone());
        assert!(t.is_free_form());
        assert_eq!(t.lines, lines[..7].to_vec(), "trailing blank dropped, the rest kept");
        assert_eq!(t.title, "NIGHT SIGNAL");
        assert_eq!(t.author, "Ada Lane");
        assert_eq!(t.contact, "Agent: Bo Reyes\nWGA Reg. 123");
        assert!(!t.is_empty());
    }

    #[test]
    fn free_form_editing() {
        let mut t = TitlePage { title: "A".into(), ..TitlePage::default() };
        t.make_free_form();
        assert!(t.is_free_form());
        assert!(t.lines.iter().any(|x| x.text == "A"));
        t.set_lines(vec![l(Align::Center, "B\tC\u{7}"), l(Align::Center, "")]);
        assert_eq!(t.lines, vec![l(Align::Center, "B C"), l(Align::Center, "")], "a blank line being added stays");
        assert_eq!(t.title, "B C");
        t.use_fields();
        assert!(!t.is_free_form());
        assert_eq!(t.title, "B C");
        t.set_lines(vec![l(Align::Center, " ")]);
        assert!(t.lines.is_empty() && t.is_empty());
    }
}
