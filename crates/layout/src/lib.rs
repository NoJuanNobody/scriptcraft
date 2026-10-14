//! ScriptCraft L2: screenplay pagination.
//!
//! The industry page: US Letter, 12-point Courier (10 characters per inch, 6 lines per inch),
//! 1" top and bottom margins (54 body lines), and fixed element indents measured from the left
//! edge of the paper ([`spec`]). Page numbers sit top-right half an inch down, from page 2 on.
//!
//! Page-break rules ([`layout`]):
//! - A scene heading (or shot) is never the last thing on a page: it moves to the next page
//!   unless at least the first lines of what follows fit with it.
//! - A character cue is never separated from its first line of dialogue.
//! - Dialogue that does not fit is split between lines (preferring the end of a sentence, never
//!   right after a parenthetical, at least two lines on each side when possible), with `(MORE)`
//!   under the first part and the cue repeated with `(CONT'D)` on the next page.
//! - Action splits only with two or more lines on each side (preferring a sentence end);
//!   otherwise it moves whole.
//! - Dual dialogue sits side by side; when it does not fit, both columns split at the same row
//!   with `(MORE)` under each column that continues and its cue with `(CONT'D)` on the next page
//!   ([`dual`]). Nothing is ever cut off.
//!
//! Production ([`locked`]): an omitted scene prints as its heading's number and `OMITTED`; with
//! locked pages, every locked page starts where it was locked and text that no longer fits flows
//! onto A/B pages (`12A.`, `12B.`), while a page that lost its text takes the numbers of the
//! pages it replaces (`12-13.`).
//!
//! Optionally, a cue gets `(CONT'D)` when the same character speaks again after action in the
//! same scene, and scene numbers print in both margins.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use scriptcraft_doc::{Element, Paper, Pos, Script, Span, character_name};
use scriptcraft_geom::{BASELINE, CHAR_W, LINE_H, TOP, chars_between, inch};

mod decorate;
pub use decorate::{body_rows, slots};
use serde::Serialize;

pub use scriptcraft_doc::Align;

/// Where an element sits on the page.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Spec {
    /// Left edge, inches from the paper's left edge.
    pub left: f32,
    /// Right edge, inches from the paper's left edge.
    pub right: f32,
    /// Blank lines before the element (dropped at the top of a page).
    pub before: usize,
    pub align: Align,
}

/// The standard indents (inches from the left edge of the paper).
pub const fn spec(e: Element) -> Spec {
    let f = scriptcraft_doc::ElementFormat::default_for(e);
    Spec { left: f.left, right: f.right, before: f.before as usize, align: f.align }
}

/// The indents of `e` in this script (its element settings).
pub fn spec_in(script: &Script, e: Element) -> Spec {
    let f = script.settings.elements.get(e);
    Spec { left: f.left, right: f.right, before: f.before as usize, align: f.align }
}

/// Characters per line for an element (standard indents).
pub fn width_chars(e: Element) -> usize {
    let s = spec(e);
    chars_between(inch(s.left), inch(s.right))
}

fn width_in(script: &Script, e: Element) -> usize {
    let s = spec_in(script, e);
    chars_between(inch(s.left), inch(s.right))
}

/// What a laid-out line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    /// A line of a script paragraph.
    Body,
    /// `(MORE)` under dialogue that continues on the next page.
    More,
    /// The repeated cue `NAME (CONT'D)` at the top of the next page.
    Contd,
    /// Header text (the page number by default).
    Header,
    /// Footer text.
    Footer,
    /// The revision mark (`*`) in the right margin of a line with revised text.
    RevisionMark,
    SceneNumber,
    /// A title-page line.
    Title,
}

/// One line on a page.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub kind: LineKind,
    pub element: Option<Element>,
    /// Left edge of the text, points from the paper's left edge.
    pub x: f32,
    /// Top of the line box, points from the paper's top edge.
    pub y: f32,
    /// The text from the paragraph (`para.text[start..end]`).
    pub text: String,
    /// Display-only text after it (an automatic ` (CONT'D)`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub suffix: String,
    /// Styled runs, as byte ranges of `text` (plain lines have none).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<Span>,
    /// The script paragraph this line shows (`None` for decorations).
    pub para: Option<usize>,
    pub start: usize,
    pub end: usize,
}

impl Line {
    /// Baseline, points from the top of the paper.
    pub fn baseline(&self) -> f32 {
        self.y + BASELINE
    }
    /// The whole visible text.
    pub fn display(&self) -> String {
        format!("{}{}", self.text, self.suffix)
    }
    /// The text in styled segments: (column, text, style), covering the whole line.
    pub fn segments(&self) -> Vec<(usize, &str, scriptcraft_doc::Style)> {
        let mut out = Vec::new();
        let mut at = 0;
        let mut col = 0;
        let push = |a: usize, b: usize, st, col: &mut usize, out: &mut Vec<_>| {
            if let Some(t) = self.text.get(a..b).filter(|t| !t.is_empty()) {
                out.push((*col, t, st));
                *col += t.chars().count();
            }
        };
        for sp in &self.spans {
            push(at, sp.start.max(at), scriptcraft_doc::Style::PLAIN, &mut col, &mut out);
            push(sp.start.max(at), sp.end, sp.style, &mut col, &mut out);
            at = sp.end.max(at);
        }
        push(at, self.text.len(), scriptcraft_doc::Style::PLAIN, &mut col, &mut out);
        out
    }
    fn chars(&self) -> usize {
        self.text.chars().count()
    }
}

/// A laid-out page.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// The page's position among script pages, from 1 (`None` on the title page).
    pub number: Option<usize>,
    /// The printed page number: `"3"`, or with locked pages `"12A"` or `"12-13"` (empty on the
    /// title page).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    pub title_page: bool,
    pub lines: Vec<Line>,
}

/// A caret location.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Caret {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub h: f32,
}

/// The paginated script.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub pages: Vec<Page>,
    /// For each paragraph, the index (into `pages`) of the page its first line is on.
    pub para_page: Vec<usize>,
    /// The paper the pages are laid out on.
    pub paper: Paper,
    /// For each page, the highest revision set with marks on it (`None`: unrevised).
    pub page_rev: Vec<Option<u8>>,
}

impl Layout {
    /// Height of page `i` in points: the paper's, or as tall as its text in speed view.
    pub fn page_height(&self, i: usize) -> f32 {
        let h = self.paper.height_pt();
        let Some(p) = self.pages.get(i) else { return h };
        let bottom = p.lines.iter().filter(|l| l.kind != LineKind::Footer).map(|l| l.y + LINE_H).fold(0.0f32, f32::max) + scriptcraft_geom::BOTTOM;
        bottom.max(h)
    }

    /// Width of every page in points.
    pub fn page_width(&self) -> f32 {
        self.paper.width_pt()
    }

    /// Pages of script (not counting the title page).
    pub fn script_pages(&self) -> usize {
        self.pages.iter().filter(|p| !p.title_page).count()
    }

    /// The page number paragraph `para` starts on, counting script pages from 1 (1 when unknown).
    pub fn page_number_of(&self, para: usize) -> usize {
        self.para_page.get(para).and_then(|i| self.pages.get(*i)).and_then(|p| p.number).unwrap_or(1)
    }

    /// The printed page label paragraph `para` starts on (`"12A"` with locked pages).
    pub fn page_label_of(&self, para: usize) -> String {
        self.para_page.get(para).and_then(|i| self.pages.get(*i)).map_or_else(|| "1".into(), |p| p.label.clone())
    }

    /// Body lines in reading order, with their page index.
    pub fn body(&self) -> impl Iterator<Item = (usize, &Line)> {
        self.pages.iter().enumerate().flat_map(|(i, p)| p.lines.iter().filter(|l| l.kind == LineKind::Body && l.para.is_some()).map(move |l| (i, l)))
    }

    /// The line showing `pos` (index into [`Layout::body`]).
    fn line_of(&self, pos: Pos) -> Option<(usize, usize, &Line)> {
        let mut best: Option<(usize, usize, &Line)> = None;
        for (k, (pi, l)) in self.body().enumerate() {
            if l.para == Some(pos.para) {
                if l.start <= pos.off || best.is_none() {
                    best = Some((k, pi, l));
                }
            } else if best.is_some() && l.para.is_some_and(|p| p > pos.para) {
                break;
            }
        }
        best
    }

    /// Where the caret for `pos` is drawn.
    pub fn caret(&self, pos: Pos) -> Option<Caret> {
        let (_, page, l) = self.line_of(pos)?;
        let n = col_of(l, pos.off);
        Some(Caret { page, x: l.x + n as f32 * CHAR_W, y: l.y, h: LINE_H })
    }

    /// The position under a point on page `page` (points from the paper's top-left).
    pub fn hit(&self, page: usize, x: f32, y: f32) -> Option<Pos> {
        let p = self.pages.get(page)?;
        let dist = |l: &Line| {
            let dy = if y < l.y {
                l.y - y
            } else if y >= l.y + LINE_H {
                y - (l.y + LINE_H) + 0.001
            } else {
                0.0
            };
            let w = (l.chars() as f32 + 1.0) * CHAR_W;
            let dx = if x < l.x {
                l.x - x
            } else if x > l.x + w {
                x - (l.x + w)
            } else {
                0.0
            };
            dy * 1000.0 + dx
        };
        let l = p.lines.iter().filter(|l| l.kind == LineKind::Body && l.para.is_some()).min_by(|a, b| dist(a).total_cmp(&dist(b)))?;
        Some(pos_at_x(l, x))
    }

    /// The position one line up (`dir < 0`) or down from `pos`, keeping the goal x (points).
    /// Returns the new position and the goal x to keep.
    pub fn vertical(&self, pos: Pos, dir: i32, goal_x: Option<f32>) -> Option<(Pos, f32)> {
        let (k, _, l) = self.line_of(pos)?;
        let gx = goal_x.unwrap_or(l.x + col_of(l, pos.off) as f32 * CHAR_W);
        let target = if dir < 0 { k.checked_sub(1)? } else { k + 1 };
        let (_, t) = self.body().nth(target)?;
        Some((pos_at_x(t, gx), gx))
    }

    /// The start or end of the visual line showing `pos`.
    pub fn line_edge(&self, pos: Pos, end: bool) -> Option<Pos> {
        let (_, _, l) = self.line_of(pos)?;
        Some(Pos::new(pos.para, if end { l.end } else { l.start }))
    }
}

/// Columns from the line's start to `off`.
fn col_of(l: &Line, off: usize) -> usize {
    if off <= l.start {
        return 0;
    }
    if off <= l.end {
        let k = (off - l.start).min(l.text.len());
        let mut k2 = k;
        while k2 > 0 && !l.text.is_char_boundary(k2) {
            k2 -= 1;
        }
        return l.text.get(..k2).map_or(0, |t| t.chars().count());
    }
    l.chars() + (off - l.end)
}

fn pos_at_x(l: &Line, x: f32) -> Pos {
    let para = l.para.unwrap_or(0);
    let col = ((x - l.x) / CHAR_W).round();
    let col = if col.is_finite() && col > 0.0 { col as usize } else { 0 };
    let off = l.text.char_indices().nth(col).map_or(l.text.len(), |(i, _)| i);
    Pos::new(para, l.start + off)
}

/// Greedy word wrap at `width` characters: byte ranges of each line (spaces at the breaks belong
/// to neither line). Always at least one line.
pub fn wrap(text: &str, width: usize) -> Vec<(usize, usize)> {
    let width = width.max(1);
    let mut out = Vec::new();
    let mut start = 0;
    let len = text.len();
    while start < len {
        // Byte offset after `width` chars, or the end.
        let limit = text.get(start..).and_then(|t| t.char_indices().nth(width)).map(|(i, _)| start + i);
        let Some(limit) = limit else {
            out.push((start, len));
            break;
        };
        let slice = text.get(start..limit).unwrap_or("");
        let next_is_space = text.get(limit..).is_some_and(|t| t.starts_with(' '));
        let (end, next) = if next_is_space {
            (limit, limit)
        } else if let Some(sp) = slice.rfind(' ').filter(|&i| i > 0) {
            (start + sp, start + sp)
        } else {
            (limit, limit)
        };
        let end_trim = start + text.get(start..end).unwrap_or("").trim_end_matches(' ').len();
        out.push((start, end_trim.max(start)));
        let mut n = next;
        while text.get(n..).is_some_and(|t| t.starts_with(' ')) {
            n += 1;
        }
        if n <= start {
            n = limit.max(start + 1);
        }
        start = n;
    }
    if out.is_empty() {
        out.push((0, 0));
    }
    out
}

/// A paragraph with its wrapped lines.
#[derive(Clone, Debug)]
struct Item {
    para: usize,
    el: Element,
    lines: Vec<(usize, usize)>,
    suffix: String,
    /// Geometry override (dual dialogue columns): (left pt, right pt).
    cols: Option<(f32, f32)>,
}

enum Unit {
    Single(Item),
    Dialogue(Vec<Item>),
    Dual(Vec<Item>, Vec<Item>),
}

impl Unit {
    /// The paragraph the unit starts with.
    fn first_para(&self) -> Option<usize> {
        match self {
            Unit::Single(i) => Some(i.para),
            Unit::Dialogue(v) | Unit::Dual(v, _) => v.first().map(|i| i.para),
        }
    }
    fn first_el(&self) -> Element {
        match self {
            Unit::Single(i) => i.el,
            Unit::Dialogue(v) | Unit::Dual(v, _) => v.first().map_or(Element::Character, |i| i.el),
        }
    }
    /// Lines that must come along with a preceding scene heading.
    fn min_lines(&self) -> usize {
        match self {
            Unit::Single(i) => i.lines.len().min(2),
            Unit::Dialogue(v) => v.iter().map(|i| i.lines.len()).sum::<usize>().min(2),
            Unit::Dual(a, b) => dual_height(a, b).min(3),
        }
    }
}

fn dual_height(a: &[Item], b: &[Item]) -> usize {
    let h = |v: &[Item]| v.iter().map(|i| i.lines.len()).sum::<usize>();
    h(a).max(h(b))
}

/// Dual-dialogue column bounds (points): left and right columns.
const DUAL_COLS: [(f32, f32); 2] = [(inch(1.5), inch(4.3)), (inch(4.6), inch(7.4))];

fn dual_item(script: &Script, para: usize, col: (f32, f32)) -> Option<Item> {
    let p = script.paras.get(para)?;
    let (l, r) = col;
    let (l, r) = match p.element {
        Element::Character => (l + inch(0.9), r),
        Element::Parenthetical => (l + inch(0.4), r - inch(0.3)),
        _ => (l, r),
    };
    Some(Item { para, el: p.element, lines: wrap(&p.text, chars_between(l, r)), suffix: String::new(), cols: Some((l, r)) })
}

fn item(script: &Script, para: usize) -> Option<Item> {
    let p = script.paras.get(para)?;
    let spacing = usize::from(script.settings.elements.get(p.element).spacing).max(1);
    Some(Item { para, el: p.element, lines: spaced(wrap(&p.text, width_in(script, p.element)), spacing), suffix: String::new(), cols: None })
}

/// A blank row between the lines of a double-spaced paragraph (no text, no caret).
const BLANK: (usize, usize) = (usize::MAX, usize::MAX);

/// Lines with `spacing - 1` blank rows between them.
fn spaced(lines: Vec<(usize, usize)>, spacing: usize) -> Vec<(usize, usize)> {
    if spacing <= 1 {
        return lines;
    }
    let n = lines.len();
    let mut out = Vec::with_capacity(n * spacing);
    for (k, l) in lines.into_iter().enumerate() {
        out.push(l);
        if k + 1 < n {
            out.extend(std::iter::repeat_n(BLANK, spacing - 1));
        }
    }
    out
}

fn block_end(script: &Script, i: usize) -> usize {
    let mut j = i + 1;
    while script.paras.get(j).is_some_and(|p| matches!(p.element, Element::Parenthetical | Element::Dialogue)) {
        j += 1;
    }
    j
}

fn units(script: &Script) -> Vec<Unit> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = script.paras.len();
    let mut last_speaker: Option<String> = None;
    while i < n {
        let Some(p) = script.paras.get(i) else { break };
        match p.element {
            Element::Character => {
                let end = block_end(script, i);
                let dual_next = script.paras.get(end).is_some_and(|q| q.element == Element::Character && q.dual);
                if dual_next {
                    let end2 = block_end(script, end);
                    let a = (i..end).filter_map(|k| dual_item(script, k, DUAL_COLS[0])).collect();
                    let b = (end..end2).filter_map(|k| dual_item(script, k, DUAL_COLS[1])).collect();
                    out.push(Unit::Dual(a, b));
                    last_speaker = None;
                    i = end2;
                    continue;
                }
                let mut v: Vec<Item> = (i..end).filter_map(|k| item(script, k)).collect();
                let name = character_name(&p.text);
                if script.settings.auto_contd
                    && !name.is_empty()
                    && last_speaker.as_deref() == Some(name.as_str())
                    && !p.text.contains("CONT'D")
                    && !p.text.contains("CONT’D")
                    && let Some(cue) = v.first_mut()
                {
                    cue.suffix = " (CONT'D)".into();
                }
                last_speaker = Some(name);
                out.push(Unit::Dialogue(v));
                i = end;
            }
            Element::Parenthetical | Element::Dialogue => {
                let end = block_end(script, i);
                out.push(Unit::Dialogue((i..end).filter_map(|k| item(script, k)).collect()));
                i = end;
            }
            Element::SceneHeading if p.omitted => {
                // Prints as its number and OMITTED; the scene's text is kept but not shown.
                last_speaker = None;
                out.push(Unit::Single(Item { para: i, el: Element::SceneHeading, lines: vec![(0, 0)], suffix: locked::OMITTED.into(), cols: None }));
                i += 1;
                while script.paras.get(i).is_some_and(|q| q.element != Element::SceneHeading) {
                    i += 1;
                }
            }
            e => {
                if e == Element::SceneHeading {
                    last_speaker = None;
                }
                if let Some(it) = item(script, i) {
                    out.push(Unit::Single(it));
                }
                i += 1;
            }
        }
    }
    out
}

fn sentence_end(text: &str) -> bool {
    let t = text.trim_end().trim_end_matches(['"', '\'', ')', '”', '’']);
    t.ends_with(['.', '?', '!', ':', '-', '—']) || t.ends_with("...")
}

struct Builder<'a> {
    script: &'a Script,
    pages: Vec<Page>,
    cur: Vec<Line>,
    row: usize,
    rows: usize,
}

impl Builder<'_> {
    fn y(&self) -> f32 {
        TOP + self.row as f32 * LINE_H
    }

    fn new_page(&mut self) {
        let lines = std::mem::take(&mut self.cur);
        self.pages.push(Page { number: None, label: String::new(), title_page: false, lines });
        self.row = 0;
    }

    fn left(&self) -> usize {
        self.rows.saturating_sub(self.row)
    }

    fn emit(&mut self, it: &Item, k: usize, suffix: bool) {
        let Some(&(start, end)) = it.lines.get(k) else { return };
        if (start, end) == BLANK {
            self.row += 1;
            return;
        }
        let para = self.script.paras.get(it.para);
        let text = para.and_then(|p| p.text.get(start..end)).unwrap_or("").to_string();
        let spans: Vec<Span> = para
            .map(|p| {
                p.spans
                    .iter()
                    .filter(|sp| sp.end > start && sp.start < end)
                    .map(|sp| Span { start: sp.start.max(start) - start, end: sp.end.min(end) - start, style: sp.style })
                    .collect()
            })
            .unwrap_or_default();
        let sfx = if suffix && k + 1 == it.lines.len() { it.suffix.clone() } else { String::new() };
        let s = spec_in(self.script, it.el);
        let (l, r) = it.cols.unwrap_or((inch(s.left), inch(s.right)));
        let n = text.chars().count() + sfx.chars().count();
        let x = match s.align {
            Align::Right if it.cols.is_none() => (r - n as f32 * CHAR_W).max(l),
            Align::Center if it.cols.is_none() => ((l + r - n as f32 * CHAR_W) / 2.0).max(l),
            _ => l,
        };
        let y = self.y();
        if k == 0
            && it.el == Element::SceneHeading
            && self.script.settings.show_scene_numbers
            && let Some(num) = self.script.paras.get(it.para).and_then(|p| p.scene_number.clone())
        {
            let w = num.chars().count() as f32 * CHAR_W;
            self.cur.push(deco(LineKind::SceneNumber, inch(1.0) - w, y, &num));
            self.cur.push(deco(LineKind::SceneNumber, self.script.settings.page.paper.width_pt() - inch(0.75), y, &num));
        }
        self.cur.push(Line { kind: LineKind::Body, element: Some(it.el), x, y, text, suffix: sfx, spans, para: Some(it.para), start, end });
        self.row += 1;
    }

    fn gap(&mut self, before: usize) {
        if self.row > 0 {
            self.row += before;
        }
    }

    /// Place a single paragraph.
    fn single(&mut self, it: &Item, keep: usize) {
        let before = spec_in(self.script, it.el).before;
        let need = it.lines.len();
        let fits = |b: &Self| b.row == 0 || b.row + before + need + keep <= b.rows;
        if !fits(self) {
            // Action may split across the break.
            let room = self.left().saturating_sub(before);
            let splittable = matches!(it.el, Element::Action | Element::General) && keep == 0;
            let mut k = 0;
            if splittable && need >= 4 && room >= 2 {
                let max = room.min(need - 2);
                let ok = |k: usize| k >= 2 && need - k >= 2;
                let text = self.script.paras.get(it.para).map_or("", |p| p.text.as_str());
                k = (2..=max)
                    .rev()
                    .find(|&k| ok(k) && it.lines.get(k - 1).is_some_and(|&(s, e)| sentence_end(text.get(s..e).unwrap_or(""))))
                    .unwrap_or(0);
                if k == 0 && max >= 2 {
                    k = max;
                }
            }
            if k > 0 {
                self.gap(before);
                for j in 0..k {
                    self.emit(it, j, true);
                }
                self.new_page();
                for j in k..need {
                    if self.row >= self.rows {
                        self.new_page();
                    }
                    self.emit(it, j, true);
                }
                return;
            }
            self.new_page();
        }
        self.gap(before);
        for j in 0..need {
            if self.row >= self.rows {
                self.new_page();
            }
            self.emit(it, j, true);
        }
    }

    fn more(&mut self) {
        let y = self.y();
        self.cur.push(deco(LineKind::More, inch(spec(Element::Character).left), y, "(MORE)"));
        self.row += 1;
    }

    fn contd(&mut self, cue: &Item) {
        let name = self.script.paras.get(cue.para).map_or(String::new(), |p| p.text.clone());
        let name = if name.contains("CONT'D") { name } else { format!("{name} (CONT'D)") };
        let y = self.y();
        self.cur.push(deco(LineKind::Contd, inch(spec(Element::Character).left), y, &name));
        self.row += 1;
    }

    /// Place a dialogue block, splitting it with (MORE)/(CONT'D) when needed.
    fn dialogue(&mut self, items: &[Item]) {
        let before = items.first().map_or(0, |i| spec_in(self.script, i.el).before);
        let rows: Vec<(usize, usize)> = items.iter().enumerate().flat_map(|(a, it)| (0..it.lines.len()).map(move |k| (a, k))).collect();
        let has_cue = items.first().is_some_and(|i| i.el == Element::Character);
        let total = rows.len();
        if self.row > 0 && self.row + before + total > self.rows {
            // Split here if a good break exists, else start a new page.
            let room = self.left().saturating_sub(before);
            if let Some(k) = self.split_point(items, &rows, room, has_cue) {
                self.gap(before);
                self.place_rows(items, rows.get(..k).unwrap_or(&[]));
                if self.script.settings.more_contd && has_cue {
                    self.more();
                }
                self.new_page();
                if self.script.settings.more_contd
                    && has_cue
                    && let Some(cue) = items.first()
                {
                    self.contd(cue);
                }
                self.rest(items, rows.get(k..).unwrap_or(&[]), has_cue);
                return;
            }
            self.new_page();
        }
        self.gap(before);
        self.rest(items, &rows, has_cue);
    }

    /// Place rows from the current position, breaking pages (with MORE/CONT'D) when a block is
    /// longer than a page.
    fn rest(&mut self, items: &[Item], rows: &[(usize, usize)], has_cue: bool) {
        let mut rows = rows;
        while !rows.is_empty() {
            let room = self.left();
            if rows.len() <= room {
                self.place_rows(items, rows);
                return;
            }
            let k = room.saturating_sub(1).max(1).min(rows.len());
            self.place_rows(items, rows.get(..k).unwrap_or(&[]));
            if self.script.settings.more_contd && has_cue && self.row < self.rows {
                self.more();
            }
            self.new_page();
            if self.script.settings.more_contd
                && has_cue
                && let Some(cue) = items.first()
            {
                self.contd(cue);
            }
            rows = rows.get(k..).unwrap_or(&[]);
        }
    }

    fn place_rows(&mut self, items: &[Item], rows: &[(usize, usize)]) {
        for &(a, k) in rows {
            if let Some(it) = items.get(a) {
                self.emit(it, k, true);
            }
        }
    }

    /// How many rows of a dialogue block go on this page (room includes the (MORE) line).
    fn split_point(&self, items: &[Item], rows: &[(usize, usize)], room: usize, has_cue: bool) -> Option<usize> {
        let total = rows.len();
        let more = usize::from(has_cue && self.script.settings.more_contd);
        let cue = usize::from(has_cue);
        let max = room.checked_sub(more)?.min(total.saturating_sub(1));
        let el = |i: usize| rows.get(i).and_then(|&(a, _)| items.get(a)).map(|it| it.el);
        let text = |i: usize| {
            rows.get(i)
                .and_then(|&(a, k)| items.get(a).and_then(|it| it.lines.get(k).map(|&(s, e)| (it.para, s, e))))
                .and_then(|(p, s, e)| self.script.paras.get(p).and_then(|p| p.text.get(s..e)))
                .unwrap_or("")
        };
        let last_of_para = |i: usize| rows.get(i).zip(rows.get(i + 1)).is_none_or(|(x, y)| x.0 != y.0);
        let valid = |k: usize, min_after: usize| k >= cue + 2 && total - k >= min_after && el(k - 1) != Some(Element::Parenthetical);
        let good = |k: usize| sentence_end(text(k - 1)) || last_of_para(k - 1);
        (1..=max)
            .rev()
            .find(|&k| valid(k, 2) && good(k))
            .or_else(|| (1..=max).rev().find(|&k| valid(k, 2)))
            .or_else(|| (1..=max).rev().find(|&k| valid(k, 1)))
    }
}

fn deco(kind: LineKind, x: f32, y: f32, text: &str) -> Line {
    Line { kind, element: None, x, y, text: text.to_string(), suffix: String::new(), spans: Vec::new(), para: None, start: 0, end: 0 }
}

fn title_page(script: &Script) -> Option<Page> {
    let t = &script.title;
    let paper = script.settings.page.paper;
    let page_w = paper.width_pt();
    if !script.settings.title_page || t.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    let center = |s: &str| ((page_w - s.chars().count() as f32 * CHAR_W) / 2.0).max(inch(1.0));
    let mut row = 18usize;
    let put_center = |lines: &mut Vec<Line>, v: &str, row: &mut usize| {
        for l in v.lines() {
            for (s, e) in wrap(l.trim(), 60) {
                let text = l.trim().get(s..e).unwrap_or("");
                lines.push(deco(LineKind::Title, center(text), TOP + *row as f32 * LINE_H, text));
                *row += 1;
            }
        }
    };
    put_center(&mut lines, &t.title.to_uppercase(), &mut row);
    row += 3;
    put_center(&mut lines, &t.credit, &mut row);
    row += 1;
    put_center(&mut lines, &t.author, &mut row);
    if !t.source.trim().is_empty() {
        row += 3;
        put_center(&mut lines, &t.source, &mut row);
    }
    // Contact (and copyright, notes) bottom-left; draft date bottom-right.
    let block: Vec<&str> = [&t.notes, &t.copyright, &t.contact].iter().flat_map(|v| v.lines()).map(str::trim).filter(|l| !l.is_empty()).collect();
    let rows = body_rows(paper);
    let first = rows.saturating_sub(block.len()).max(row + 2);
    for (i, l) in block.iter().enumerate() {
        lines.push(deco(LineKind::Title, inch(1.5), TOP + (first + i) as f32 * LINE_H, l));
    }
    let dates: Vec<&str> = t.draft_date.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let first = rows.saturating_sub(dates.len()).max(row + 2);
    for (i, l) in dates.iter().enumerate() {
        let x = (page_w - inch(1.0) - l.chars().count() as f32 * CHAR_W).max(inch(1.0));
        lines.push(deco(LineKind::Title, x, TOP + (first + i) as f32 * LINE_H, l));
    }
    Some(Page { number: None, label: String::new(), title_page: true, lines })
}

/// Paginate a script.
pub fn layout(script: &Script) -> Layout {
    layout_with(script, Mode::Pages)
}

/// How to lay out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Industry pages with breaks, numbers and the title page.
    #[default]
    Pages,
    /// Speed view: one continuous page, no breaks, no title page, no page numbers.
    Speed,
}

/// Lay out a script in a mode.
pub fn layout_with(script: &Script, mode: Mode) -> Layout {
    let us = units(script);
    let speed = mode == Mode::Speed;
    // Speed view: effectively unlimited rows, so nothing ever breaks.
    let paper = script.settings.page.paper;
    let rows = if speed { usize::MAX / 4 } else { body_rows(paper) };
    let mut b = Builder { script, pages: Vec::new(), cur: Vec::new(), row: 0, rows };
    // Locked pages: each anchored unit starts a page; (first page index, its locked labels).
    let locked = !speed && script.settings.pages_locked;
    let anchor = |u: &Unit| if locked { locked::anchor(script, u.first_para()) } else { None };
    let mut segments: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, u) in us.iter().enumerate() {
        if let Some(labels) = anchor(u) {
            if !b.cur.is_empty() {
                b.new_page();
            }
            segments.push((b.pages.len(), labels));
        } else if !speed && !b.cur.is_empty() && script.settings.elements.get(u.first_el()).new_page {
            // Elements set to start a new page (multi-camera scenes, act breaks).
            b.new_page();
        }
        match u {
            Unit::Single(it) => {
                let keep = if matches!(it.el, Element::SceneHeading | Element::Shot) {
                    // Nothing to keep with when the next unit starts a locked page anyway.
                    us.get(i + 1).filter(|n| anchor(n).is_none()).map_or(0, |n| spec_in(script, n.first_el()).before + n.min_lines())
                } else {
                    0
                };
                b.single(it, keep);
            }
            Unit::Dialogue(v) => b.dialogue(v),
            Unit::Dual(x, y) => b.dual(x, y),
        }
    }
    if !b.cur.is_empty() || b.pages.is_empty() {
        b.new_page();
    }
    let mut pages = b.pages;
    // Printed labels (12A, 12-13 when locked); the header puts them on the page.
    let labels = if locked { locked::assign(pages.len(), &segments) } else { (1..=pages.len()).map(|n| n.to_string()).collect() };
    for (i, (p, label)) in pages.iter_mut().zip(labels).enumerate() {
        p.number = Some(i + 1);
        p.label = label;
    }
    if !speed && let Some(tp) = title_page(script) {
        pages.insert(0, tp);
    }
    let page_rev = decorate::revisions(script, &mut pages, paper);
    if !speed {
        decorate::headers(script, &mut pages, &page_rev, paper);
    }
    let mut para_page = vec![usize::MAX; script.paras.len()];
    for (pi, p) in pages.iter().enumerate() {
        for l in &p.lines {
            if let Some(slot) = l.para.and_then(|k| para_page.get_mut(k))
                && *slot == usize::MAX
            {
                *slot = pi;
            }
        }
    }
    // Paragraphs with no line (never happens; be safe) take the previous paragraph's page.
    let mut last = 0;
    for s in &mut para_page {
        if *s == usize::MAX {
            *s = last;
        }
        last = *s;
    }
    Layout { pages, para_page, paper, page_rev }
}

/// Which paragraphs start a unit that can begin a locked page (the first paragraph of a scene
/// heading, action, dialogue block or dual dialogue), skipping text hidden in omitted scenes.
pub fn unit_starts(script: &Script) -> Vec<bool> {
    let mut v = vec![false; script.paras.len()];
    for u in units(script) {
        if let Some(slot) = u.first_para().and_then(|p| v.get_mut(p)) {
            *slot = true;
        }
    }
    v
}

mod dual;
pub mod locked;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_page;
