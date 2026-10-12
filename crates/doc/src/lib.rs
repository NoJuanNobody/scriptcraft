//! ScriptCraft L1: the screenplay model.
//!
//! A [`Script`] is an ordered list of [`Para`]graphs, each tagged with an [`Element`] type (Scene
//! Heading, Action, Character, Parenthetical, Dialogue, Transition, Shot, General), plus the
//! [`TitlePage`] fields and per-script [`Settings`]. Positions are [`Pos`] = paragraph index +
//! byte offset (always on a char boundary). Edit primitives never panic: positions are clamped.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod element;
mod text;

pub use element::Element;
pub use text::{next_boundary, prev_boundary, word_left, word_right};

use serde::{Deserialize, Serialize};

/// Most paragraphs a script may hold (hostile files are refused beyond this).
pub const MAX_PARAS: usize = 200_000;
/// Longest paragraph, in bytes.
pub const MAX_PARA_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum DocError {
    #[error("the script is too large ({0} paragraphs; the limit is {MAX_PARAS})")]
    TooLarge(usize),
    #[error("no paragraph {0}")]
    NoPara(usize),
    #[error("no scene {0}")]
    NoScene(usize),
}

/// One paragraph of the script.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Para {
    pub element: Element,
    pub text: String,
    /// Locked scene number (scene headings only), e.g. `"12A"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_number: Option<String>,
    /// This character cue starts the right-hand side of a dual dialogue (it pairs with the
    /// dialogue block just before it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dual: bool,
    /// Index-card summary (scene headings only).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
}

impl Para {
    pub fn new(element: Element, text: impl Into<String>) -> Self {
        let mut text: String = text.into();
        if element.uppercase() {
            text = text.to_uppercase();
        }
        text = sanitize(&text);
        Para { element, text, scene_number: None, dual: false, summary: String::new() }
    }
    pub fn len(&self) -> usize {
        self.text.len()
    }
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Strip control characters (line breaks become spaces) and cap the length.
pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_PARA_BYTES));
    for c in s.chars() {
        let c = match c {
            '\t' | '\n' | '\r' | '\u{2028}' | '\u{2029}' => ' ',
            c if c.is_control() => continue,
            c => c,
        };
        if out.len() + c.len_utf8() > MAX_PARA_BYTES {
            break;
        }
        out.push(c);
    }
    out
}

/// Title page fields (Fountain's keys; Final Draft's title page in its usual arrangement).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TitlePage {
    pub title: String,
    pub credit: String,
    pub author: String,
    pub source: String,
    pub draft_date: String,
    pub contact: String,
    pub copyright: String,
    pub notes: String,
}

impl TitlePage {
    /// Field ids, in title-page order.
    pub const FIELDS: [&'static str; 8] = ["title", "credit", "author", "source", "draftDate", "contact", "copyright", "notes"];

    pub fn get(&self, field: &str) -> Option<&str> {
        Some(match field {
            "title" => &self.title,
            "credit" => &self.credit,
            "author" | "authors" => &self.author,
            "source" => &self.source,
            "draftDate" | "draft date" | "date" => &self.draft_date,
            "contact" => &self.contact,
            "copyright" => &self.copyright,
            "notes" => &self.notes,
            _ => return None,
        })
    }

    /// Set a field (multi-line values keep their line breaks); `false` for an unknown field.
    pub fn set(&mut self, field: &str, value: &str) -> bool {
        let v: String = value.chars().filter(|c| *c == '\n' || !c.is_control()).take(MAX_PARA_BYTES).collect();
        let slot = match field {
            "title" => &mut self.title,
            "credit" => &mut self.credit,
            "author" | "authors" => &mut self.author,
            "source" => &mut self.source,
            "draftDate" | "draft date" | "date" => &mut self.draft_date,
            "contact" => &mut self.contact,
            "copyright" => &mut self.copyright,
            "notes" => &mut self.notes,
            _ => return false,
        };
        *slot = v;
        true
    }

    pub fn is_empty(&self) -> bool {
        Self::FIELDS.iter().all(|f| self.get(f).is_none_or(|v| v.trim().is_empty()))
    }
}

/// Per-script options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Print scene numbers in both margins.
    pub show_scene_numbers: bool,
    /// (MORE) / (CONT'D) when dialogue breaks across pages.
    pub more_contd: bool,
    /// Automatic (CONT'D) when a character speaks again after action in the same scene.
    pub auto_contd: bool,
    /// Print the title page (when it has any content).
    pub title_page: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { show_scene_numbers: false, more_contd: true, auto_contd: true, title_page: true }
    }
}

/// A position: paragraph index and byte offset into its text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub para: usize,
    pub off: usize,
}

impl Pos {
    pub const fn new(para: usize, off: usize) -> Self {
        Pos { para, off }
    }
}

/// A scene: its heading paragraph and the paragraphs up to the next heading.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub index: usize,
    /// Paragraph index of the heading.
    pub para: usize,
    /// One past the scene's last paragraph.
    pub end: usize,
    pub heading: String,
    pub number: Option<String>,
    pub summary: String,
}

/// The screenplay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    #[serde(default)]
    pub title: TitlePage,
    pub paras: Vec<Para>,
    #[serde(default)]
    pub settings: Settings,
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

impl Script {
    /// A blank script: one empty scene heading.
    pub fn new() -> Self {
        Script { title: TitlePage::default(), paras: vec![Para::new(Element::SceneHeading, "")], settings: Settings::default() }
    }

    /// A script from paragraphs (an empty list gets one empty Action).
    pub fn from_paras(paras: Vec<Para>) -> Result<Self, DocError> {
        let mut s = Script { title: TitlePage::default(), paras, settings: Settings::default() };
        s.normalize()?;
        Ok(s)
    }

    /// Enforce the invariants: 1..=MAX_PARAS paragraphs, clean text, uppercase where the element
    /// demands it, summaries and scene numbers only on headings.
    pub fn normalize(&mut self) -> Result<(), DocError> {
        if self.paras.len() > MAX_PARAS {
            return Err(DocError::TooLarge(self.paras.len()));
        }
        if self.paras.is_empty() {
            self.paras.push(Para::new(Element::Action, ""));
        }
        for p in &mut self.paras {
            p.text = sanitize(&p.text);
            if p.element.uppercase() {
                p.text = p.text.to_uppercase();
            }
            if p.element != Element::SceneHeading {
                p.scene_number = None;
            }
            if p.element != Element::Character {
                p.dual = false;
            }
        }
        Ok(())
    }

    pub fn para(&self, i: usize) -> Option<&Para> {
        self.paras.get(i)
    }

    pub fn para_mut(&mut self, i: usize) -> Result<&mut Para, DocError> {
        self.paras.get_mut(i).ok_or(DocError::NoPara(i))
    }

    /// The position at the end of the script.
    pub fn end(&self) -> Pos {
        let last = self.paras.len().saturating_sub(1);
        Pos::new(last, self.paras.get(last).map_or(0, Para::len))
    }

    /// `pos` moved onto an existing paragraph and a char boundary at or before its offset.
    pub fn clamp(&self, pos: Pos) -> Pos {
        let para = pos.para.min(self.paras.len().saturating_sub(1));
        let text = self.paras.get(para).map_or("", |p| p.text.as_str());
        let mut off = pos.off.min(text.len());
        while off > 0 && !text.is_char_boundary(off) {
            off -= 1;
        }
        Pos { para, off }
    }

    /// Insert `text` (control characters dropped; uppercased for uppercase elements) at `pos`;
    /// returns the position after it.
    pub fn insert_text(&mut self, pos: Pos, text: &str) -> Result<Pos, DocError> {
        let pos = self.clamp(pos);
        let p = self.para_mut(pos.para)?;
        let mut ins = sanitize(text);
        if p.element.uppercase() {
            ins = ins.to_uppercase();
        }
        let room = MAX_PARA_BYTES.saturating_sub(p.text.len());
        if ins.len() > room {
            let mut cut = room;
            while cut > 0 && !ins.is_char_boundary(cut) {
                cut -= 1;
            }
            ins.truncate(cut);
        }
        p.text.insert_str(pos.off, &ins);
        Ok(Pos::new(pos.para, pos.off + ins.len()))
    }

    /// Delete between two positions (in either order). Paragraphs in between are removed and
    /// the last one's tail joins the first, which keeps its element. Returns the start.
    pub fn delete_range(&mut self, a: Pos, b: Pos) -> Pos {
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        if a == b {
            return a;
        }
        if a.para == b.para {
            if let Some(p) = self.paras.get_mut(a.para) {
                p.text.replace_range(a.off..b.off, "");
            }
            return a;
        }
        let tail = self.paras.get(b.para).and_then(|p| p.text.get(b.off..)).unwrap_or("").to_string();
        if let Some(p) = self.paras.get_mut(a.para) {
            p.text.truncate(a.off);
            p.text.push_str(&tail);
            if p.element.uppercase() {
                p.text = p.text.to_uppercase();
            }
        }
        let end = b.para.min(self.paras.len().saturating_sub(1));
        if a.para < end {
            self.paras.drain(a.para + 1..=end);
        }
        a
    }

    /// Split the paragraph at `pos`; the tail becomes a new paragraph of type `element`. Returns
    /// the start of the new paragraph.
    pub fn split(&mut self, pos: Pos, element: Element) -> Result<Pos, DocError> {
        if self.paras.len() >= MAX_PARAS {
            return Err(DocError::TooLarge(self.paras.len() + 1));
        }
        let pos = self.clamp(pos);
        let p = self.para_mut(pos.para)?;
        let tail = p.text.split_off(pos.off);
        let next = Para::new(element, tail);
        self.paras.insert(pos.para + 1, next);
        Ok(Pos::new(pos.para + 1, 0))
    }

    /// Change a paragraph's element (uppercasing the text when the element asks for it).
    pub fn set_element(&mut self, para: usize, element: Element) -> Result<(), DocError> {
        let p = self.para_mut(para)?;
        p.element = element;
        if element.uppercase() {
            p.text = p.text.to_uppercase();
        }
        if element != Element::SceneHeading {
            p.scene_number = None;
        }
        if element != Element::Character {
            p.dual = false;
        }
        Ok(())
    }

    /// Every scene, in order. Paragraphs before the first heading belong to no scene.
    pub fn scenes(&self) -> Vec<Scene> {
        let heads: Vec<usize> = self.paras.iter().enumerate().filter(|(_, p)| p.element == Element::SceneHeading).map(|(i, _)| i).collect();
        heads
            .iter()
            .enumerate()
            .filter_map(|(index, &para)| {
                let p = self.paras.get(para)?;
                Some(Scene {
                    index,
                    para,
                    end: heads.get(index + 1).copied().unwrap_or(self.paras.len()),
                    heading: p.text.clone(),
                    number: p.scene_number.clone(),
                    summary: p.summary.clone(),
                })
            })
            .collect()
    }

    /// The scene containing paragraph `para`, if any.
    pub fn scene_of(&self, para: usize) -> Option<usize> {
        self.paras.iter().take(para.saturating_add(1)).filter(|p| p.element == Element::SceneHeading).count().checked_sub(1)
    }

    /// Move scene `from` (heading through the paragraph before the next heading) so it becomes
    /// scene `to`. Returns the new paragraph index of the moved heading.
    pub fn move_scene(&mut self, from: usize, to: usize) -> Result<usize, DocError> {
        let scenes = self.scenes();
        let src = scenes.get(from).ok_or(DocError::NoScene(from))?;
        if to >= scenes.len() {
            return Err(DocError::NoScene(to));
        }
        if from == to {
            return Ok(src.para);
        }
        let block: Vec<Para> = self.paras.drain(src.para..src.end).collect();
        let n = block.len();
        // Where the target scene starts once the block is gone.
        let at = if to < from { scenes.get(to).map_or(0, |s| s.para) } else { scenes.get(to).map_or(self.paras.len(), |s| s.end.saturating_sub(n)) };
        let at = at.min(self.paras.len());
        for (i, p) in block.into_iter().enumerate() {
            self.paras.insert(at + i, p);
        }
        Ok(at)
    }

    /// Character names in order of first appearance (cue extensions removed).
    pub fn characters(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in &self.paras {
            if p.element == Element::Character {
                let name = character_name(&p.text);
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    }

    /// Locations named in scene headings, in order of first appearance.
    pub fn locations(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in &self.paras {
            if p.element == Element::SceneHeading {
                let loc = heading_parts(&p.text).1;
                if !loc.is_empty() && !out.contains(&loc) {
                    out.push(loc);
                }
            }
        }
        out
    }

    /// Words in the script body.
    pub fn word_count(&self) -> usize {
        self.paras.iter().map(|p| p.text.split_whitespace().count()).sum()
    }

    /// Renumber every scene 1, 2, 3…
    pub fn number_scenes(&mut self) {
        let mut n = 0usize;
        for p in &mut self.paras {
            if p.element == Element::SceneHeading {
                n += 1;
                p.scene_number = Some(n.to_string());
            }
        }
    }

    /// Plain text of a range (paragraphs joined by newlines).
    pub fn text_between(&self, a: Pos, b: Pos) -> String {
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let mut out = String::new();
        for i in a.para..=b.para {
            let Some(p) = self.paras.get(i) else { break };
            let from = if i == a.para { a.off } else { 0 };
            let to = if i == b.para { b.off } else { p.text.len() };
            out.push_str(p.text.get(from..to).unwrap_or(""));
            if i != b.para {
                out.push('\n');
            }
        }
        out
    }
}

/// A character cue without its extensions: `"MARGO (V.O.)"` → `"MARGO"`.
pub fn character_name(cue: &str) -> String {
    let base = cue.split('(').next().unwrap_or("");
    base.trim().trim_end_matches('^').trim().to_string()
}

/// Scene-heading prefixes, longest first.
pub const HEADING_PREFIXES: [&str; 8] = ["INT./EXT.", "EXT./INT.", "INT/EXT.", "I/E.", "INT.", "EXT.", "EST.", "INT "];

/// A heading split into (prefix, location, time): `"INT. DINER - NIGHT"` →
/// `("INT.", "DINER", "NIGHT")`. Missing parts are empty.
pub fn heading_parts(h: &str) -> (String, String, String) {
    let t = h.trim();
    let up = t.to_uppercase();
    let mut prefix = "";
    for p in HEADING_PREFIXES {
        if up.starts_with(p) {
            prefix = p.trim();
            break;
        }
    }
    let rest = t.get(prefix.len()..).unwrap_or("").trim();
    let (loc, time) = match rest.rfind(" - ") {
        Some(i) => (rest.get(..i).unwrap_or(""), rest.get(i + 3..).unwrap_or("")),
        None => (rest, ""),
    };
    (prefix.to_string(), loc.trim().to_string(), time.trim().to_string())
}

#[cfg(test)]
mod tests;
