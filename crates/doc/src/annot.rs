//! Things attached to a paragraph's text: script notes, production tags (for breakdowns) and
//! alternate versions of a line.
//!
//! Notes and tags are byte ranges `[start, end)` of [`Para::text`], like style spans. The edit
//! primitives in `lib.rs` move them with the text (insert, delete, split, join); anything that
//! changes text another way is caught by [`Para::fix_marks`] (run by `Script::normalize`), which
//! clamps every range onto the text, so a stale offset can never point outside it.
//!
//! Alternates are other versions of the paragraph: the active version is the paragraph's own
//! text, so only it is laid out, printed and exported as script text.

use serde::{Deserialize, Serialize};

use crate::{CardColor, Para, Pos, Script, Span, Style, sanitize, style};

/// Most notes on one paragraph.
pub const MAX_NOTES: usize = 64;
/// Most tagged ranges on one paragraph.
pub const MAX_TAGGED: usize = 256;
/// Most alternates of one paragraph (besides the active text).
pub const MAX_ALTERNATES: usize = 16;
/// Longest note, bytes.
pub const MAX_NOTE_BYTES: usize = 8 * 1024;
/// Longest author name or tag category, chars.
pub const MAX_NAME_CHARS: usize = 40;

/// A script note: a comment anchored to a range of a paragraph (an empty range is a point).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub start: usize,
    pub end: usize,
    pub text: String,
    #[serde(default = "default_note_color")]
    pub color: CardColor,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
}

fn default_note_color() -> CardColor {
    CardColor::Yellow
}

impl Note {
    /// A note (text cleaned: control characters other than line breaks dropped, length capped).
    pub fn new(start: usize, end: usize, text: &str, color: Option<CardColor>, author: &str) -> Self {
        Note { start, end, text: clean_note(text), color: color.unwrap_or(CardColor::Yellow), author: clean_author(author) }
    }
}

/// A production tag on a range of text: the tagged words are an item of `category`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tagged {
    pub start: usize,
    pub end: usize,
    pub category: String,
}

/// Another version of a paragraph's text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alternate {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<Span>,
}

impl Alternate {
    /// Plain text as an alternate.
    pub fn plain(text: &str) -> Self {
        Alternate { text: sanitize(text), spans: Vec::new() }
    }
}

/// The standard breakdown categories, with their highlight colours (sRGB).
pub const TAG_CATEGORIES: &[(&str, [u8; 3])] = &[
    ("Cast", [0xe5, 0x3e, 0x3e]),
    ("Background Actors", [0x3f, 0xa8, 0x5a]),
    ("Stunts", [0xf0, 0x8a, 0x24]),
    ("Vehicles", [0xc0, 0x5c, 0xd6]),
    ("Props", [0x8a, 0x5a, 0x2b]),
    ("Special Effects", [0x3b, 0x82, 0xf6]),
    ("Wardrobe", [0xe4, 0x5f, 0xb4]),
    ("Makeup and Hair", [0xd9, 0x7f, 0x8a]),
    ("Animals", [0x8f, 0xb3, 0x2e]),
    ("Music", [0x6a, 0x5a, 0xe0]),
    ("Sound", [0x2a, 0xa1, 0xb3]),
    ("Set Dressing", [0xb8, 0x8a, 0x3a]),
    ("Greenery", [0x2f, 0x8f, 0x3a]),
    ("Special Equipment", [0x70, 0x78, 0x88]),
    ("Visual Effects", [0x18, 0xa0, 0xd8]),
    ("Mechanical Effects", [0x9a, 0x6a, 0x48]),
    ("Security", [0x55, 0x55, 0x55]),
    ("Additional Labor", [0xa8, 0x8f, 0x14]),
    ("Miscellaneous", [0x9a, 0x9a, 0x9a]),
];

/// A clean tag category: trimmed, single-spaced, no `,;[]` or control characters, at most 40
/// chars; a standard category is matched case-insensitively and returned in its own spelling.
/// `None` when nothing is left.
pub fn clean_category(s: &str) -> Option<String> {
    let t: String =
        sanitize(s).replace([',', ';', '[', ']'], " ").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME_CHARS).collect();
    if t.is_empty() {
        return None;
    }
    Some(TAG_CATEGORIES.iter().find(|(n, _)| n.eq_ignore_ascii_case(&t)).map_or(t, |(n, _)| (*n).to_string()))
}

/// A category's colour: its standard colour, or a stable one picked from the name.
pub fn category_rgb(name: &str) -> [u8; 3] {
    if let Some((_, c)) = TAG_CATEGORIES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        return *c;
    }
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    CardColor::ALL.get(h as usize % CardColor::ALL.len()).map_or([0x88, 0x88, 0x88], |c| c.rgb())
}

/// An author name: single-spaced, no control characters, at most 40 chars.
pub fn clean_author(s: &str) -> String {
    sanitize(s).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME_CHARS).collect()
}

/// Note text: line breaks kept, other control characters dropped, at most [`MAX_NOTE_BYTES`].
pub fn clean_note(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_NOTE_BYTES));
    for c in s.replace("\r\n", "\n").chars() {
        let c = match c {
            '\r' | '\u{2028}' | '\u{2029}' => '\n',
            '\t' => ' ',
            c if c.is_control() && c != '\n' => continue,
            c => c,
        };
        if out.len() + c.len_utf8() > MAX_NOTE_BYTES {
            break;
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// Moved notes and tags (a paragraph's tail when it splits, or a joined paragraph's).
#[derive(Default)]
pub(crate) struct Marks {
    notes: Vec<Note>,
    tagged: Vec<Tagged>,
}

/// `x` after inserting `len` bytes at `at`: positions at or after it move, unless `stay` (a range
/// end exactly at `at` does not grow).
fn ins(x: usize, at: usize, len: usize, stay: bool) -> usize {
    if x > at || (x == at && !stay) { x.saturating_add(len) } else { x }
}

/// `x` after deleting `[from, to)`.
fn del(x: usize, from: usize, to: usize) -> usize {
    if x <= from {
        x
    } else if x >= to {
        x - (to - from)
    } else {
        from
    }
}

fn floor_boundary(text: &str, off: usize) -> usize {
    let mut o = off.min(text.len());
    while o > 0 && !text.is_char_boundary(o) {
        o -= 1;
    }
    o
}

impl Para {
    /// Whether any note, tag or alternate is attached.
    pub fn has_marks(&self) -> bool {
        !self.notes.is_empty() || !self.tagged.is_empty() || !self.alternates.is_empty()
    }

    /// `len` bytes were inserted at `at`.
    pub(crate) fn marks_insert(&mut self, at: usize, len: usize) {
        if len == 0 {
            return;
        }
        for n in &mut self.notes {
            n.start = ins(n.start, at, len, false);
            n.end = ins(n.end, at, len, true).max(n.start);
        }
        for t in &mut self.tagged {
            t.start = ins(t.start, at, len, false);
            t.end = ins(t.end, at, len, true).max(t.start);
        }
    }

    /// `[from, to)` was deleted. Notes shrink (to a point if all their text went); tags with
    /// nothing left are removed.
    pub(crate) fn marks_delete(&mut self, from: usize, to: usize) {
        if to <= from {
            return;
        }
        for n in &mut self.notes {
            n.start = del(n.start, from, to);
            n.end = del(n.end, from, to).max(n.start);
        }
        for t in &mut self.tagged {
            t.start = del(t.start, from, to);
            t.end = del(t.end, from, to);
        }
        self.tagged.retain(|t| t.end > t.start);
    }

    /// The paragraph splits at `at`: marks after it leave (shifted to the new paragraph's
    /// start); ranges across `at` are cut there. A point note at `at` stays.
    pub(crate) fn marks_split(&mut self, at: usize) -> Marks {
        let mut out = Marks::default();
        let moves = |start: usize, end: usize| start > at || (start == at && end > at);
        let (go, stay): (Vec<Note>, Vec<Note>) = std::mem::take(&mut self.notes).into_iter().partition(|n| moves(n.start, n.end));
        self.notes = stay;
        for n in &mut self.notes {
            n.end = n.end.min(at);
        }
        out.notes = go.into_iter().map(|n| Note { start: n.start - at, end: n.end.saturating_sub(at), ..n }).collect();
        let (go, stay): (Vec<Tagged>, Vec<Tagged>) = std::mem::take(&mut self.tagged).into_iter().partition(|t| moves(t.start, t.end));
        self.tagged = stay;
        for t in &mut self.tagged {
            t.end = t.end.min(at);
        }
        self.tagged.retain(|t| t.end > t.start);
        out.tagged = go.into_iter().map(|t| Tagged { start: t.start - at, end: t.end.saturating_sub(at), ..t }).collect();
        out
    }

    /// The marks of a paragraph whose text from `from` on was joined onto this one at `at`.
    pub(crate) fn marks_of_tail(&self, from: usize, at: usize) -> Marks {
        let shift = |x: usize| x.max(from) - from + at;
        Marks {
            notes: self
                .notes
                .iter()
                .filter(|n| n.end > from || n.start >= from)
                .map(|n| Note { start: shift(n.start), end: shift(n.end), ..n.clone() })
                .collect(),
            tagged: self.tagged.iter().filter(|t| t.end > from).map(|t| Tagged { start: shift(t.start), end: shift(t.end), ..t.clone() }).collect(),
        }
    }

    /// Take moved marks.
    pub(crate) fn marks_append(&mut self, m: Marks) {
        self.notes.extend(m.notes);
        self.tagged.extend(m.tagged);
    }

    /// Keep marks valid for the current text: ranges clamped onto char boundaries, empty tags
    /// and duplicates removed, sorted by position, counts and sizes capped.
    pub fn fix_marks(&mut self) {
        if !self.has_marks() {
            return;
        }
        let text = self.text.as_str();
        for n in &mut self.notes {
            n.start = floor_boundary(text, n.start);
            n.end = floor_boundary(text, n.end).max(n.start);
            if n.text.len() > MAX_NOTE_BYTES || n.text.chars().any(|c| c.is_control() && c != '\n') {
                n.text = clean_note(&n.text);
            }
            if n.author.chars().count() > MAX_NAME_CHARS || n.author.chars().any(char::is_control) {
                n.author = clean_author(&n.author);
            }
        }
        self.notes.sort_by_key(|n| (n.start, n.end));
        self.notes.truncate(MAX_NOTES);
        for t in &mut self.tagged {
            t.start = floor_boundary(text, t.start);
            t.end = floor_boundary(text, t.end);
            match clean_category(&t.category) {
                Some(c) => t.category = c,
                None => t.end = t.start,
            }
        }
        self.tagged.retain(|t| t.end > t.start);
        self.tagged.sort_by(|a, b| (a.start, a.end, &a.category).cmp(&(b.start, b.end, &b.category)));
        self.tagged.dedup();
        self.tagged.truncate(MAX_TAGGED);
        self.alternates.truncate(MAX_ALTERNATES);
        for a in &mut self.alternates {
            let clean = sanitize(&a.text);
            if clean != a.text {
                a.text = clean;
                a.spans.clear();
            }
            a.spans = style::fix(&a.text, &a.spans);
        }
    }

    /// Make alternate `i` the active text (the current text takes its place in the list).
    pub fn use_alternate(&mut self, i: usize) -> bool {
        let Some(alt) = self.alternates.get_mut(i) else { return false };
        std::mem::swap(&mut alt.text, &mut self.text);
        std::mem::swap(&mut alt.spans, &mut self.spans);
        self.fix_marks();
        true
    }

    /// Cycle through the versions: forward makes the first alternate active and puts the current
    /// text last; backward the reverse.
    pub fn cycle_alternates(&mut self, forward: bool) -> bool {
        if self.alternates.is_empty() {
            return false;
        }
        let cur = Alternate { text: std::mem::take(&mut self.text), spans: std::mem::take(&mut self.spans) };
        let next = if forward {
            self.alternates.push(cur);
            self.alternates.remove(0)
        } else {
            self.alternates.insert(0, cur);
            self.alternates.pop().unwrap_or_default()
        };
        self.text = next.text;
        self.spans = next.spans;
        self.fix_marks();
        true
    }

    /// Plain text of a byte range (clamped).
    pub fn slice(&self, start: usize, end: usize) -> &str {
        let a = floor_boundary(&self.text, start);
        let b = floor_boundary(&self.text, end).max(a);
        self.text.get(a..b).unwrap_or("")
    }

    /// A paragraph's text as an alternate (keeps its styles).
    pub fn as_alternate(&self) -> Alternate {
        Alternate { text: self.text.clone(), spans: self.spans.clone() }
    }
}

/// Where a note lives: paragraph and index in its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteRef {
    pub para: usize,
    pub k: usize,
}

impl Script {
    /// Every note in script order (its position in this list is the note's index).
    pub fn note_refs(&self) -> Vec<NoteRef> {
        self.paras.iter().enumerate().flat_map(|(para, p)| (0..p.notes.len()).map(move |k| NoteRef { para, k })).collect()
    }

    pub fn note(&self, r: NoteRef) -> Option<&Note> {
        self.paras.get(r.para)?.notes.get(r.k)
    }

    pub fn note_mut(&mut self, r: NoteRef) -> Option<&mut Note> {
        self.paras.get_mut(r.para)?.notes.get_mut(r.k)
    }

    /// Anchor a note to `a..b` (a range across paragraphs is cut at the end of the first).
    /// Returns the note's index, or `None` when the paragraph already has [`MAX_NOTES`].
    pub fn add_note(&mut self, a: Pos, b: Pos, text: &str, color: Option<CardColor>, author: &str) -> Option<usize> {
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let p = self.paras.get_mut(a.para)?;
        if p.notes.len() >= MAX_NOTES {
            return None;
        }
        let end = if b.para == a.para { b.off } else { p.text.len() };
        let n = Note::new(a.off, end, text, color, author);
        let key = (n.start, n.end);
        p.notes.push(n);
        p.fix_marks();
        let k = p.notes.iter().rposition(|n| (n.start, n.end) == key)?;
        self.note_refs().iter().position(|r| *r == NoteRef { para: a.para, k })
    }

    /// Tag `a..b` as an item of `category`, one range per paragraph (surrounding spaces left
    /// out). Returns how many ranges were tagged.
    pub fn add_tag(&mut self, a: Pos, b: Pos, category: &str) -> usize {
        let Some(cat) = clean_category(category) else { return 0 };
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let mut n = 0;
        for i in a.para..=b.para {
            let Some(p) = self.paras.get_mut(i) else { break };
            let from = if i == a.para { a.off } else { 0 };
            let to = if i == b.para { b.off } else { p.text.len() };
            let piece = p.text.get(from..to).unwrap_or("");
            let lead = piece.len() - piece.trim_start().len();
            let trail = piece.len() - piece.trim_end().len();
            let (s, e) = (from + lead, to.saturating_sub(trail));
            if e <= s || p.tagged.len() >= MAX_TAGGED {
                continue;
            }
            let t = Tagged { start: s, end: e, category: cat.clone() };
            if !p.tagged.contains(&t) {
                p.tagged.push(t);
                p.fix_marks();
                n += 1;
            }
        }
        n
    }

    /// Remove tags overlapping `a..b` (touching the point when `a == b`), of one category or
    /// any. Returns how many were removed.
    pub fn remove_tags(&mut self, a: Pos, b: Pos, category: Option<&str>) -> usize {
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let cat = category.and_then(clean_category);
        let mut n = 0;
        for i in a.para..=b.para {
            let Some(p) = self.paras.get_mut(i) else { break };
            let from = if i == a.para { a.off } else { 0 };
            let to = if i == b.para { b.off } else { p.text.len() };
            let before = p.tagged.len();
            p.tagged.retain(|t| {
                let hit = if from == to { t.start <= from && from <= t.end } else { t.start < to && from < t.end };
                !(hit && cat.as_deref().is_none_or(|c| c == t.category))
            });
            n += before - p.tagged.len();
        }
        n
    }

    /// Every tag category in use, standard ones first (in their standard order).
    pub fn tag_categories_used(&self) -> Vec<String> {
        let mut used: Vec<String> = Vec::new();
        for p in &self.paras {
            for t in &p.tagged {
                if !used.contains(&t.category) {
                    used.push(t.category.clone());
                }
            }
        }
        let rank = |c: &String| TAG_CATEGORIES.iter().position(|(n, _)| n == c).unwrap_or(usize::MAX);
        used.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.cmp(b)));
        used
    }
}

/// An alternate from styled text runs (importers).
pub fn alternate_from_runs(runs: &[(String, Style)]) -> Alternate {
    let p = Para::styled(crate::Element::Dialogue, runs);
    Alternate { text: p.text, spans: p.spans }
}

#[cfg(test)]
#[path = "annot_tests.rs"]
mod tests;
