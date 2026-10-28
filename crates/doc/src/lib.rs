//! ScriptCraft L1: the screenplay model.
//!
//! A [`Script`] is an ordered list of [`Para`]graphs, each tagged with an [`Element`] type (Scene
//! Heading, Action, Character, Parenthetical, Dialogue, Transition, Shot, General), plus the
//! [`TitlePage`] fields and per-script [`Settings`]. Positions are [`Pos`] = paragraph index +
//! byte offset (always on a char boundary). Edit primitives never panic: positions are clamped.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod annot;
mod beats;
mod element;
mod format;
pub mod page_setup;
mod production;
pub mod revision;
pub mod sluglines;
mod structure;
mod style;
pub mod template;
mod text;
mod title;
pub mod track;
pub mod voices;

pub use annot::{
    Alternate, MAX_ALTERNATES, MAX_NOTES, MAX_TAGGED, Note, NoteRef, TAG_CATEGORIES, Tagged, alternate_from_runs, category_rgb, clean_author,
    clean_category, clean_note,
};
pub use beats::{
    BEAT_H, BEAT_MAX, BEAT_MIN, BEAT_W, BOARD_EXTENT, Beat, BeatBoard, MAX_BEAT_TEXT, MAX_BEAT_TITLE, MAX_BEATS, MAX_BOOKMARK_CHARS, beat_key,
    clean_beat_text, clean_beat_title, clean_bookmark,
};
pub use element::Element;
pub use format::{Align, ElementFormat, ElementFormats};
pub use page_setup::{HeaderFooter, PageSetup, Paper};
pub use production::{expand_page_label, next_label};
pub use revision::{RevMark, RevisionSet, Revisions};
pub use sluglines::{Setting as HeadingSetting, Sluglines};
pub use structure::{MAX_STRUCTURE, OutlineItem, PointSlot, StructureKind, StructurePoint};
pub use style::{Flag, Span, Style};
pub use template::Template;
pub use text::{next_boundary, prev_boundary, word_left, word_right};
pub use title::{MAX_TITLE_LINES, TitleLine};
pub use track::{Change, ChangeKind, TrackSettings};
pub use voices::Voices;

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
    /// Bold / italic / underline runs (byte ranges of `text`; plain text has none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<Span>,
    /// Index-card colour (scene headings only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<CardColor>,
    /// Index-card tags (scene headings only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The scene is omitted: it prints as its heading's number and `OMITTED`, its text kept
    /// (scene headings only).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub omitted: bool,
    /// Locked pages: the labels of the locked pages that start with this paragraph (the first
    /// is the page it starts; later ones are pages its content ran onto when locked).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_lock: Vec<String>,
    /// Script notes anchored to ranges of `text`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    /// Production tags on ranges of `text` (breakdown items).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tagged: Vec<Tagged>,
    /// Other versions of this paragraph; `text` is the active one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternates: Vec<Alternate>,
    /// Revised text (byte ranges of `text` and their revision set).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revs: Vec<RevMark>,
    /// Structure points (acts, sequences, sections) that start just before this paragraph
    /// (scene headings only); see [`structure`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structure: Vec<StructurePoint>,
    /// A named bookmark on this paragraph (FDX `Bookmark`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,
    /// Beat-board beats linked to this scene (scene headings only): beat ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beats: Vec<u32>,
    /// Tracked changes awaiting accept or reject (see [`track`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<Change>,
}

/// Index-card colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

impl CardColor {
    pub const ALL: [CardColor; 8] = [
        CardColor::Red,
        CardColor::Orange,
        CardColor::Yellow,
        CardColor::Green,
        CardColor::Teal,
        CardColor::Blue,
        CardColor::Purple,
        CardColor::Pink,
    ];
    pub fn id(self) -> &'static str {
        match self {
            CardColor::Red => "red",
            CardColor::Orange => "orange",
            CardColor::Yellow => "yellow",
            CardColor::Green => "green",
            CardColor::Teal => "teal",
            CardColor::Blue => "blue",
            CardColor::Purple => "purple",
            CardColor::Pink => "pink",
        }
    }
    pub fn parse(s: &str) -> Option<CardColor> {
        CardColor::ALL.into_iter().find(|c| c.id().eq_ignore_ascii_case(s.trim()))
    }
    /// sRGB of the colour.
    pub const fn rgb(self) -> [u8; 3] {
        match self {
            CardColor::Red => [0xe0, 0x4f, 0x4f],
            CardColor::Orange => [0xe8, 0x8a, 0x2c],
            CardColor::Yellow => [0xe6, 0xc2, 0x2e],
            CardColor::Green => [0x4c, 0xae, 0x4f],
            CardColor::Teal => [0x2f, 0x9e, 0x96],
            CardColor::Blue => [0x4c, 0x6e, 0xf5],
            CardColor::Purple => [0x8e, 0x5c, 0xd6],
            CardColor::Pink => [0xd6, 0x5c, 0xa8],
        }
    }
    /// The nearest colour to an sRGB value.
    pub fn nearest(rgb: [u8; 3]) -> CardColor {
        let d = |c: CardColor| c.rgb().iter().zip(rgb).map(|(a, b)| (i32::from(*a) - i32::from(b)).pow(2)).sum::<i32>();
        CardColor::ALL.into_iter().min_by_key(|c| d(*c)).unwrap_or(CardColor::Blue)
    }
}

/// Clean index-card tags: trimmed, no commas or control characters, unique, at most 20 of 40 chars.
pub fn clean_tags<'a>(tags: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t: String = sanitize(t).replace([',', '[', ']'], " ").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
        if !t.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&t)) && out.len() < 20 {
            out.push(t);
        }
    }
    out
}

impl Para {
    pub fn new(element: Element, text: impl Into<String>) -> Self {
        let mut text: String = text.into();
        if element.uppercase() {
            text = text.to_uppercase();
        }
        text = sanitize(&text);
        Para {
            element,
            text,
            scene_number: None,
            dual: false,
            summary: String::new(),
            spans: Vec::new(),
            color: None,
            tags: Vec::new(),
            omitted: false,
            page_lock: Vec::new(),
            notes: Vec::new(),
            tagged: Vec::new(),
            alternates: Vec::new(),
            revs: Vec::new(),
            structure: Vec::new(),
            bookmark: None,
            beats: Vec::new(),
            changes: Vec::new(),
        }
    }

    /// A paragraph from styled text runs (FDX `<Text Style>` runs, Fountain emphasis).
    pub fn styled(element: Element, runs: &[(String, Style)]) -> Self {
        let mut text = String::new();
        let mut styles = Vec::new();
        for (t, st) in runs {
            let t = sanitize(t);
            if text.len() + t.len() > MAX_PARA_BYTES {
                break;
            }
            styles.extend(std::iter::repeat_n(*st, t.len()));
            text.push_str(&t);
        }
        let mut p = Para::new(element, "");
        p.set_text_styled(&text, &styles, element.uppercase());
        p
    }

    /// Per-byte styles of the text.
    pub fn styles(&self) -> Vec<Style> {
        style::expand(&self.spans, self.text.len())
    }

    /// Replace text and styles together (`upper` capitalises, keeping each char's style).
    pub fn set_text_styled(&mut self, text: &str, styles: &[Style], upper: bool) {
        if upper {
            let (t, v) = style::upper_with(text, styles);
            self.spans = style::compress(&t, &v);
            self.text = t;
        } else {
            self.spans = style::compress(text, styles);
            self.text = text.to_string();
        }
    }

    /// The style of the character just before `off` (what typing at `off` continues).
    pub fn style_before(&self, off: usize) -> Style {
        let i = prev_boundary(&self.text, off.min(self.text.len()));
        if off == 0 {
            return self.spans.first().filter(|s| s.start == 0).map_or(Style::PLAIN, |s| s.style);
        }
        self.spans.iter().find(|s| s.start <= i && i < s.end).map_or(Style::PLAIN, |s| s.style)
    }

    /// Text runs with their styles, in order (plain runs included).
    pub fn runs(&self) -> Vec<(&str, Style)> {
        let mut out = Vec::new();
        let mut at = 0;
        for s in &self.spans {
            if s.start > at {
                out.push((self.text.get(at..s.start).unwrap_or(""), Style::PLAIN));
            }
            out.push((self.text.get(s.start..s.end).unwrap_or(""), s.style));
            at = s.end;
        }
        if at < self.text.len() || out.is_empty() {
            out.push((self.text.get(at..).unwrap_or(""), Style::PLAIN));
        }
        out
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
    /// A free-form title page's paragraphs (see the `title` module); empty: the fields are laid
    /// out in the usual arrangement.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<TitleLine>,
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
        Self::FIELDS.iter().all(|f| self.get(f).is_none_or(|v| v.trim().is_empty())) && !self.is_free_form()
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
    /// Element indents, spacing, capitals and Enter/Tab flow.
    #[serde(skip_serializing_if = "ElementFormats::is_default")]
    pub elements: ElementFormats,
    /// Locked pages: page breaks stay where they were locked; new text makes A/B pages.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pages_locked: bool,
    /// Locked scene numbers: numbers stay; new scenes are numbered after the one before (12A).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub scene_numbers_locked: bool,
    /// Paper, header and footer.
    #[serde(skip_serializing_if = "PageSetup::is_default")]
    pub page: PageSetup,
    /// Revision sets, the active one and revision mode.
    #[serde(skip_serializing_if = "Revisions::is_default")]
    pub revision: Revisions,
    /// Read-aloud voices: the narrator's and each character's.
    #[serde(skip_serializing_if = "Voices::is_default")]
    pub voices: Voices,
    /// Number Scenes uses letters (A, B … Z, AA, BB …), as multi-camera scripts do.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub scene_letters: bool,
    /// Track changes.
    #[serde(skip_serializing_if = "TrackSettings::is_default")]
    pub track: TrackSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_scene_numbers: false,
            more_contd: true,
            auto_contd: true,
            title_page: true,
            elements: ElementFormats::default(),
            pages_locked: false,
            scene_numbers_locked: false,
            page: PageSetup::default(),
            revision: Revisions::default(),
            voices: Voices::default(),
            scene_letters: false,
            track: TrackSettings::default(),
        }
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
    pub color: Option<CardColor>,
    pub tags: Vec<String>,
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
    /// Structure points after the last scene (e.g. an act with no scenes yet).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structure_tail: Vec<StructurePoint>,
    /// The beat board.
    #[serde(default, skip_serializing_if = "BeatBoard::is_empty")]
    pub board: BeatBoard,
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

impl Script {
    /// A blank script: one empty scene heading.
    pub fn new() -> Self {
        Script {
            title: TitlePage::default(),
            paras: vec![Para::new(Element::SceneHeading, "")],
            settings: Settings::default(),
            structure_tail: Vec::new(),
            board: BeatBoard::default(),
        }
    }

    /// A script from paragraphs (an empty list gets one empty Action).
    pub fn from_paras(paras: Vec<Para>) -> Result<Self, DocError> {
        let mut s =
            Script { title: TitlePage::default(), paras, settings: Settings::default(), structure_tail: Vec::new(), board: BeatBoard::default() };
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
        for e in Element::ALL {
            let f = self.settings.elements.get(e);
            self.settings.elements.set(e, f);
        }
        self.settings.page.header.clean();
        self.settings.page.footer.clean();
        self.settings.revision.normalize();
        self.settings.voices.normalize();
        let top_level = (self.settings.revision.sets.len().saturating_sub(1)).min(usize::from(u8::MAX)) as u8;
        let caps = Element::ALL.map(|e| self.upper(e));
        for p in &mut self.paras {
            let clean = sanitize(&p.text);
            if clean != p.text {
                p.text = clean;
                p.spans.clear();
            }
            p.spans = style::fix(&p.text, &p.spans);
            if caps.get(p.element.index()).copied().unwrap_or(false) && p.text.chars().any(char::is_lowercase) {
                let st = p.styles();
                let t = p.text.clone();
                p.set_text_styled(&t, &st, true);
            }
            if p.element != Element::SceneHeading {
                p.scene_number = None;
                p.color = None;
                p.tags.clear();
                p.omitted = false;
            } else {
                let t = std::mem::take(&mut p.tags);
                p.tags = clean_tags(t.iter().map(String::as_str));
            }
            if p.element != Element::Character {
                p.dual = false;
            }
            p.fix_marks();
            if !p.revs.is_empty() {
                for m in &mut p.revs {
                    m.level = m.level.min(top_level);
                }
                p.revs = revision::fix_marks(&p.text, &p.revs);
            }
        }
        self.normalize_production();
        self.fix_structure();
        self.normalize_boards();
        if self.paras.iter().any(|p| !p.changes.is_empty()) {
            track::fix_all(self);
        }
        Ok(())
    }

    pub fn para(&self, i: usize) -> Option<&Para> {
        self.paras.get(i)
    }

    pub fn para_mut(&mut self, i: usize) -> Result<&mut Para, DocError> {
        self.paras.get_mut(i).ok_or(DocError::NoPara(i))
    }

    /// Whether `e` is typed in capitals in this script.
    pub fn upper(&self, e: Element) -> bool {
        self.settings.elements.get(e).uppercase
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

    /// Insert `text` (control characters dropped; uppercased for uppercase elements) at `pos`,
    /// continuing the style of the character before it; returns the position after it.
    pub fn insert_text(&mut self, pos: Pos, text: &str) -> Result<Pos, DocError> {
        let pos = self.clamp(pos);
        let st = self.para(pos.para).map_or(Style::PLAIN, |p| p.style_before(pos.off));
        self.insert_styled(pos, text, st)
    }

    /// Insert `text` in style `st` at `pos`; returns the position after it.
    pub fn insert_styled(&mut self, pos: Pos, text: &str, st: Style) -> Result<Pos, DocError> {
        let pos = self.clamp(pos);
        let caps = self.para(pos.para).is_some_and(|p| self.upper(p.element));
        let p = self.para_mut(pos.para)?;
        let mut ins = sanitize(text);
        if caps {
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
        let mut styles = p.styles();
        let at = pos.off.min(styles.len());
        styles.splice(at..at, std::iter::repeat_n(st, ins.len()));
        p.text.insert_str(pos.off, &ins);
        p.spans = style::compress(&p.text, &styles);
        p.marks_insert(pos.off, ins.len());
        Ok(Pos::new(pos.para, pos.off + ins.len()))
    }

    /// Set or clear a style flag over a range (in either order).
    pub fn set_flag(&mut self, a: Pos, b: Pos, flag: Flag, on: bool) {
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        for i in a.para..=b.para {
            let Some(p) = self.paras.get_mut(i) else { break };
            let from = if i == a.para { a.off } else { 0 };
            let to = if i == b.para { b.off } else { p.text.len() };
            let mut st = p.styles();
            for x in st.get_mut(from..to.max(from)).into_iter().flatten() {
                *x = x.with(flag, on);
            }
            p.spans = style::compress(&p.text, &st);
        }
    }

    /// Whether every character in the range has `flag` (an empty range: the style before `a`).
    pub fn all_have(&self, a: Pos, b: Pos, flag: Flag) -> bool {
        let (a, b) = (self.clamp(a), self.clamp(b));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        if a == b {
            return self.para(a.para).is_some_and(|p| p.style_before(a.off).get(flag));
        }
        let mut any = false;
        for i in a.para..=b.para {
            let Some(p) = self.paras.get(i) else { break };
            let from = if i == a.para { a.off } else { 0 };
            let to = if i == b.para { b.off } else { p.text.len() };
            let st = p.styles();
            for (k, _) in p.text.char_indices().filter(|(k, _)| *k >= from && *k < to) {
                any = true;
                if !st.get(k).is_some_and(|s| s.get(flag)) {
                    return false;
                }
            }
        }
        any
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
                let mut st = p.styles();
                st.drain(a.off..b.off.min(st.len()).max(a.off));
                p.text.replace_range(a.off..b.off, "");
                p.spans = style::compress(&p.text, &st);
                p.marks_delete(a.off, b.off);
            }
            return a;
        }
        let (tail, tail_st) = match self.paras.get(b.para) {
            Some(q) => (q.text.get(b.off..).unwrap_or("").to_string(), q.styles().get(b.off..).map(<[Style]>::to_vec).unwrap_or_default()),
            None => (String::new(), Vec::new()),
        };
        let upper = self.para(a.para).is_some_and(|p| self.upper(p.element));
        let moved = self.paras.get(b.para).map(|q| q.marks_of_tail(b.off, a.off)).unwrap_or_default();
        if let Some(p) = self.paras.get_mut(a.para) {
            p.marks_delete(a.off, p.text.len());
            p.marks_append(moved);
            let mut st = p.styles();
            st.truncate(a.off);
            st.extend(tail_st);
            let mut t = p.text.get(..a.off).unwrap_or("").to_string();
            t.push_str(&tail);
            p.set_text_styled(&t, &st, upper);
        }
        let end = b.para.min(self.paras.len().saturating_sub(1));
        if a.para < end {
            // Structure points of removed paragraphs stay: they move to the next heading.
            let gone: Vec<Para> = self.paras.drain(a.para + 1..=end).collect();
            let kept: Vec<StructurePoint> = gone.iter().flat_map(|p| p.structure.iter().cloned()).collect();
            if let Some(p) = self.paras.get_mut(a.para) {
                p.structure.extend(kept);
            }
            self.inherit_anchors(a.para, &gone);
            self.fix_structure();
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
        let caps = self.upper(element);
        let p = self.para_mut(pos.para)?;
        let mut st = p.styles();
        let tail_st = st.split_off(pos.off.min(st.len()));
        let tail = p.text.split_off(pos.off);
        p.spans = style::compress(&p.text, &st);
        let moved = p.marks_split(pos.off);
        // Splitting at the very start moves the whole text down: its structure points go too.
        // So do its bookmark and beat links, which belong to the text.
        let whole = pos.off == 0 && !tail.is_empty();
        let structure = if whole { std::mem::take(&mut p.structure) } else { Vec::new() };
        let (bookmark, beats) = if whole { (p.bookmark.take(), std::mem::take(&mut p.beats)) } else { (None, Vec::new()) };
        let mut next = Para::new(element, "");
        next.set_text_styled(&tail, &tail_st, caps);
        next.marks_append(moved);
        next.structure = structure;
        next.bookmark = bookmark;
        next.beats = beats;
        self.paras.insert(pos.para + 1, next);
        Ok(Pos::new(pos.para + 1, 0))
    }

    /// Change a paragraph's element (uppercasing the text when the element asks for it).
    pub fn set_element(&mut self, para: usize, element: Element) -> Result<(), DocError> {
        let caps = self.upper(element);
        let p = self.para_mut(para)?;
        p.element = element;
        if caps {
            let st = p.styles();
            let t = p.text.clone();
            p.set_text_styled(&t, &st, true);
        }
        if element != Element::SceneHeading {
            p.scene_number = None;
        }
        if element != Element::Character {
            p.dual = false;
        }
        if element != Element::SceneHeading && !p.structure.is_empty() {
            self.fix_structure();
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
                    color: p.color,
                    tags: p.tags.clone(),
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
        let n = self.scenes().len();
        if from >= n {
            return Err(DocError::NoScene(from));
        }
        if to >= n {
            return Err(DocError::NoScene(to));
        }
        if from != to {
            // Structure points stay where they are: the scene joins the group of the scene it
            // lands next to.
            let order = self.outline();
            let src = order.iter().position(|i| *i == OutlineItem::Scene(from)).ok_or(DocError::NoScene(from))?;
            let mut rest = order.clone();
            rest.remove(src);
            let at = rest.iter().position(|i| *i == OutlineItem::Scene(to)).ok_or(DocError::NoScene(to))?;
            let dest = if to < from { at } else { at + 1 };
            self.move_outline_item(src, dest.min(order.len().saturating_sub(1)));
        }
        self.scenes().get(to).map(|s| s.para).ok_or(DocError::NoScene(to))
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
        let letters = self.settings.scene_letters;
        let mut n = 0usize;
        for p in &mut self.paras {
            if p.element == Element::SceneHeading {
                n += 1;
                p.scene_number = Some(if letters { template::scene_letter(n) } else { n.to_string() });
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

/// A heading split into (prefix, location, time) with the default words (English and Russian):
/// `"INT. DINER - NIGHT"` → `("INT.", "DINER", "NIGHT")`. Missing parts are empty. Code that
/// knows the user's words uses [`Sluglines::parts`].
pub fn heading_parts(h: &str) -> (String, String, String) {
    static DEFAULT: std::sync::OnceLock<Sluglines> = std::sync::OnceLock::new();
    DEFAULT.get_or_init(Sluglines::default).parts(h)
}

#[cfg(test)]
mod structure_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "v04_tests.rs"]
mod v04_tests;
