//! Import a screenplay from a PDF by where its text sits on the page.
//!
//! Text is read with positions ([`text`]), grouped into lines, and each line's element is told by
//! its indent from the action margin (Courier 12 conventions, see [`crate::shape::by_indent`]).
//! Page numbers and headers in the top margin, footers, `(MORE)` / `CONTINUED` markers and margin
//! scene numbers are dropped (scene numbers are kept on their headings); a cue repeated as
//! `NAME (CONT'D)` at the top of a page continues the speech; wrapped lines join into paragraphs;
//! side-by-side cues become dual dialogue; a first page of centred text becomes the title page.

mod file;
mod font;
mod obj;
mod text;

use scriptcraft_doc::{Element, Script, Style};

use crate::Imported;
use crate::shape::{self, Builder, Geometry, Styled, TitleBlocks};

/// A run of text on a line (inches from the page's top-left).
#[derive(Clone, Debug)]
struct Seg {
    x: f32,
    x_end: f32,
    text: Styled,
}

#[derive(Clone, Debug)]
struct Line {
    page: usize,
    y: f32,
    segs: Vec<Seg>,
    scene_number: Option<String>,
}

impl Line {
    fn x(&self) -> f32 {
        self.segs.first().map_or(0.0, |s| s.x)
    }
    fn x_end(&self) -> f32 {
        self.segs.last().map_or(0.0, |s| s.x_end)
    }
    fn styled(&self) -> Styled {
        let mut t = Styled::default();
        for s in &self.segs {
            t.join(&s.text);
        }
        t
    }
    fn text(&self) -> String {
        self.styled().text()
    }
}

struct Page {
    width: f32,
    height: f32,
    lines: Vec<Line>,
    /// The page ends with `(MORE)`: a speech continues on the next page.
    more: bool,
}

/// Import a PDF.
pub fn import(bytes: &[u8]) -> Result<Imported, String> {
    let f = file::File::parse(bytes)?;
    let refs = f.pages();
    if refs.is_empty() {
        return Err("the PDF has no pages".into());
    }
    let mut pages = Vec::new();
    let mut sizes = Vec::new();
    for (i, p) in refs.iter().enumerate() {
        let t = text::page_text(&f, p);
        sizes.extend(t.glyphs.iter().filter(|g| !g.text.trim().is_empty()).map(|g| g.size as f32));
        pages.push(lines_of(i, t));
    }
    if pages.iter().all(|p| p.lines.is_empty()) {
        return Err("no text found in the PDF (a scan or an image-only PDF can't be imported)".into());
    }
    sizes.sort_by(f32::total_cmp);
    let size_pt = sizes.get(sizes.len() / 2).copied().unwrap_or(12.0).clamp(4.0, 72.0);
    Ok(assemble(pages, size_pt / 72.0))
}

/// Group a page's glyphs into lines of segments.
fn lines_of(page: usize, t: text::PageText) -> Page {
    let mut glyphs: Vec<text::Glyph> = t.glyphs.into_iter().filter(|g| !g.text.is_empty() && g.size > 0.5).collect();
    glyphs.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut rows: Vec<Vec<text::Glyph>> = Vec::new();
    for g in glyphs {
        match rows.last_mut() {
            Some(r) if r.first().is_some_and(|f| (g.y - f.y).abs() <= (0.3 * f.size).max(1.0)) => r.push(g),
            _ => rows.push(vec![g]),
        }
    }
    let mut lines = Vec::new();
    for mut r in rows {
        r.sort_by(|a, b| a.x.total_cmp(&b.x));
        let y = r.first().map_or(0.0, |g| g.y);
        let mut segs: Vec<Seg> = Vec::new();
        let mut end = f64::MIN;
        let mut space = false;
        for g in &r {
            let cw = if g.w > 0.1 { g.w } else { g.size * 0.6 };
            let gap = g.x - end;
            let new_seg = segs.is_empty() || gap > 2.5 * cw;
            if g.text.trim().is_empty() {
                space |= !new_seg;
                end = end.max(g.x + cw);
                continue;
            }
            let st = Style {
                bold: g.bold,
                italic: g.italic,
                underline: t.rules.iter().any(|&(x0, x1, ry)| {
                    let cx = g.x + cw / 2.0;
                    cx >= x0 && cx <= x1 && ry >= g.y - 0.15 * g.size && ry <= g.y + 0.45 * g.size
                }),
            };
            if new_seg {
                segs.push(Seg { x: (g.x / 72.0) as f32, x_end: 0.0, text: Styled::default() });
            }
            if let Some(s) = segs.last_mut() {
                if !new_seg && (space || gap > 0.3 * cw) && s.text.0.last().is_some_and(|(c, _)| *c != ' ') {
                    s.text.push(' ', Style::PLAIN);
                }
                s.text.push_str(&g.text, st);
                s.x_end = ((g.x + cw) / 72.0) as f32;
            }
            space = false;
            end = g.x + cw;
        }
        let segs: Vec<Seg> = segs.into_iter().map(|s| Seg { text: s.text.clean(), ..s }).filter(|s| !s.text.is_blank()).collect();
        if !segs.is_empty() {
            lines.push(Line { page, y: (y / 72.0) as f32, segs, scene_number: None });
        }
    }
    Page { width: (t.width / 72.0) as f32, height: (t.height / 72.0) as f32, lines, more: false }
}

fn is_page_number(t: &str) -> bool {
    let t = t.trim().trim_end_matches('.');
    !t.is_empty() && t.len() <= 8 && t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Whether a page holds a title page: centred text and corner blocks, no script.
fn is_title_page(p: &Page) -> bool {
    if p.lines.is_empty() || p.lines.iter().any(|l| shape::is_heading(&l.text())) {
        return false;
    }
    let mid = p.width / 2.0;
    let centred = p.lines.iter().filter(|l| ((l.x() + l.x_end()) / 2.0 - mid).abs() < 0.35 && l.x() > 1.9).count();
    let corner = p.lines.iter().filter(|l| l.y > p.height * 0.55).count();
    centred >= 1 && centred + corner >= p.lines.len() * 7 / 10
}

fn title_blocks(p: &Page, lh: f32) -> TitleBlocks {
    let mid = p.width / 2.0;
    let mut b = TitleBlocks::default();
    let mut last_y = f32::MIN;
    for l in &p.lines {
        for s in &l.segs {
            let centred = ((s.x + s.x_end) / 2.0 - mid).abs() < 0.35 && s.x > 1.9;
            let t = s.text.text();
            if centred {
                if l.y - last_y > 1.5 * lh || b.centered.is_empty() {
                    b.centered.push(Vec::new());
                }
                if let Some(g) = b.centered.last_mut() {
                    g.push(t);
                }
                last_y = l.y;
            } else if s.x < mid {
                b.left.push(t);
            } else {
                b.right.push(t);
            }
        }
    }
    b
}

/// One side of a dual-dialogue block.
#[derive(Default, Clone)]
struct Column {
    cue: Styled,
    items: Vec<(Element, Styled)>,
}

impl Column {
    fn add(&mut self, t: Styled, cont: bool) {
        let s = t.text();
        let open_paren = self.items.last().is_some_and(|(e, x)| *e == Element::Parenthetical && !x.text().trim_end().ends_with(')'));
        let e = if s.trim_start().starts_with('(') || open_paren { Element::Parenthetical } else { Element::Dialogue };
        match self.items.last_mut() {
            Some((last, x)) if *last == e && (cont || e == Element::Dialogue || open_paren) => x.join(&t),
            _ => self.items.push((e, t)),
        }
    }
}

struct Dual {
    left: Column,
    right: Column,
}

/// Emit a dual-dialogue block; returns the right-hand cue (the last speaker).
fn flush_dual(b: &mut Builder, d: Dual) -> String {
    let last = d.right.cue.text();
    b.start(Element::Character, d.left.cue);
    for (e, t) in d.left.items {
        b.start(e, t);
    }
    b.start(Element::Character, d.right.cue);
    if let Some(last) = b.paras.last_mut() {
        last.2 = true;
    }
    for (e, t) in d.right.items {
        b.start(e, t);
    }
    last
}

fn assemble(mut pages: Vec<Page>, lh: f32) -> Imported {
    let mut notes = Vec::new();
    let mut script = Script::new();
    // The title page.
    let mut first_body = 0;
    if pages.len() > 1 && pages.first().is_some_and(is_title_page) {
        if let Some(p) = pages.first() {
            script.title = shape::title_from(&title_blocks(p, lh));
            script.settings.title_page = true;
        }
        first_body = 1;
    }
    // Page furniture.
    for p in pages.iter_mut().skip(first_body) {
        let (h, w) = (p.height, p.width);
        p.more = p.lines.iter().rev().take(3).any(|l| l.text().trim().eq_ignore_ascii_case("(MORE)"));
        p.lines.retain(|l| {
            let t = l.text();
            let top = l.y < 0.85 || (l.y < 1.05 && l.segs.len() == 1 && is_page_number(&t) && l.x() > w * 0.6);
            let bottom = l.y > h - 0.55 && (is_page_number(&t) || l.y > h - 0.45 || shape::is_break_marker(&t));
            !(top || bottom || shape::is_break_marker(&t))
        });
    }
    // The action margin, ignoring margin scene numbers.
    let xs: Vec<f32> = pages
        .iter()
        .skip(first_body)
        .flat_map(|p| &p.lines)
        .filter_map(|l| l.segs.iter().find(|s| !(shape::is_scene_number(&s.text.text()) && l.segs.len() > 1)).map(|s| s.x))
        .collect();
    // Scene headings sit at the action margin; without any, the commonest small indent.
    let mut heads: Vec<f32> = pages
        .iter()
        .skip(first_body)
        .flat_map(|p| &p.lines)
        .filter_map(|l| l.segs.iter().find(|s| shape::is_heading(&s.text.text())).map(|s| s.x))
        .collect();
    heads.sort_by(f32::total_cmp);
    // Centred headings (a stage play) don't share a left edge: use the commonest small indent.
    let aligned = heads.first().zip(heads.last()).is_some_and(|(a, z)| z - a < 0.3);
    let base = heads.get(heads.len() / 2).copied().filter(|_| aligned).unwrap_or_else(|| shape::base_margin(&xs));
    // Scene numbers in the margins.
    for p in pages.iter_mut().skip(first_body) {
        for l in &mut p.lines {
            if l.segs.len() < 2 {
                continue;
            }
            if l.segs.first().is_some_and(|s| shape::is_scene_number(&s.text.text()) && s.x_end <= base + 0.05) {
                let s = l.segs.remove(0);
                l.scene_number = Some(s.text.text().trim().trim_end_matches('.').to_string());
            }
            let n = l.segs.len();
            if n >= 2
                && let (Some(prev), Some(last)) = (l.segs.get(n - 2), l.segs.last())
                && shape::is_scene_number(&last.text.text())
                && last.x >= 6.4
                && last.x - prev.x_end > 0.25
            {
                let s = l.segs.pop();
                if l.scene_number.is_none() {
                    l.scene_number = s.map(|s| s.text.text().trim().trim_end_matches('.').to_string());
                }
            }
        }
    }
    let right = {
        let mut ends: Vec<f32> = pages.iter().skip(first_body).flat_map(|p| &p.lines).filter(|l| l.x() > base + 3.0).map(Line::x_end).collect();
        ends.sort_by(f32::total_cmp);
        ends.last().copied().unwrap_or(base + 6.0).max(base + 5.0)
    };
    let g = Geometry { base, right };
    let mid = (base + right) / 2.0 - 0.1;

    let uses_more = pages.iter().any(|p| p.more);
    // Dialogue line spacing: double-spaced (multi-camera) when consecutive dialogue lines sit
    // mostly two lines apart.
    let (mut single, mut double) = (0usize, 0usize);
    for p in pages.iter().skip(first_body) {
        for w in p.lines.windows(2) {
            let (Some(a), Some(c)) = (w.first(), w.get(1)) else { continue };
            let is_dlg = |l: &Line| l.segs.len() == 1 && shape::by_indent(l.x(), l.x_end(), g, &l.text()) == Element::Dialogue;
            if is_dlg(a) && is_dlg(c) {
                let gap = (c.y - a.y) / lh;
                if (0.75..1.25).contains(&gap) {
                    single += 1;
                } else if (1.75..2.25).contains(&gap) {
                    double += 1;
                }
            }
        }
    }
    let dialogue_gap = if double >= 3 && double > 2 * single { 2.25 } else { 1.25 };
    let context = context_elements(&pages, first_body, base, right, lh, dialogue_gap);
    if context.is_some() {
        notes.push("not laid out like a screenplay: elements read from context (cues, speeches, stage directions)".into());
    }
    let mut b = Builder::default();
    let mut last: Option<(usize, f32)> = None;
    let mut last_cue = String::new();
    let mut continue_speech = false;
    let mut dual: Option<Dual> = None;
    let mut dual_y = 0.0f32;
    let mut skipped = 0usize;

    let mut prev_more = false;
    for p in pages.iter().skip(first_body) {
        let mut first_on_page = true;
        let speech_continues = prev_more || !uses_more;
        prev_more = p.more;
        for (li, l) in p.lines.iter().enumerate() {
            let new_page = last.is_some_and(|(pg, _)| pg != l.page);
            let gap = match last {
                Some((pg, y)) if pg == l.page => (l.y - y) / lh,
                _ => f32::INFINITY,
            };
            // Dual dialogue: two cues side by side start a block.
            let two_cues = l.segs.len() >= 2
                && l.segs.first().is_some_and(|s| shape::is_caps(&s.text.text()) && s.x - base >= 0.5 && s.x < mid)
                && l.segs.last().is_some_and(|s| shape::is_caps(&s.text.text()) && s.x >= mid);
            if two_cues {
                let mut lc = l.segs.first().map(|s| s.text.clone()).unwrap_or_default();
                let mut rc = l.segs.last().map(|s| s.text.clone()).unwrap_or_default();
                shape::strip_contd(&mut lc);
                shape::strip_contd(&mut rc);
                let same = dual.as_ref().is_some_and(|d| first_on_page && d.left.cue.text() == lc.text() && d.right.cue.text() == rc.text());
                if !same {
                    if let Some(d) = dual.take() {
                        last_cue = flush_dual(&mut b, d);
                    }
                    dual = Some(Dual { left: Column { cue: lc, items: Vec::new() }, right: Column { cue: rc, items: Vec::new() } });
                    continue_speech = false;
                } else {
                    continue_speech = true;
                }
                dual_y = l.y;
                last = Some((l.page, l.y));
                first_on_page = false;
                continue;
            }
            if let Some(d) = dual.as_mut() {
                let in_block = !new_page && l.y - dual_y <= 1.3 * lh && (l.segs.len() >= 2 || l.x() >= base + 0.5 || l.x_end() < mid + 0.2);
                if in_block {
                    for s in &l.segs {
                        let col = if s.x >= mid { &mut d.right } else { &mut d.left };
                        col.add(s.text.clone(), continue_speech);
                    }
                    continue_speech = false;
                    dual_y = l.y;
                    last = Some((l.page, l.y));
                    first_on_page = false;
                    continue;
                }
                // A dual block may continue on the next page (its cues come back); otherwise it ends.
                if let Some(d) = dual.take() {
                    last_cue = flush_dual(&mut b, d);
                }
            }

            let mut t = l.styled();
            let mut e = match &context {
                Some(c) => c.get(&(l.page, li)).copied().unwrap_or(Element::Action),
                None => shape::by_indent(l.x(), l.x_end(), g, &t.text()),
            };
            if e == Element::Character {
                shape::strip_contd(&mut t);
            }
            let prev = b.last_element();
            // A cue repeated at the top of a page continues the speech.
            if new_page
                && first_on_page
                && speech_continues
                && e == Element::Character
                && matches!(prev, Some(Element::Dialogue | Element::Parenthetical))
                && t.text() == last_cue
            {
                continue_speech = true;
                first_on_page = false;
                last = Some((l.page, l.y));
                skipped += 1;
                continue;
            }
            // A long heading wraps onto action-margin lines.
            if e == Element::Action && prev == Some(Element::SceneHeading) && gap <= 1.25 {
                e = Element::SceneHeading;
            }
            let joinable = match e {
                Element::Action | Element::Dialogue | Element::General | Element::SceneHeading => true,
                Element::Parenthetical => b.paras.last().is_some_and(|(_, x, _, _)| !x.text().trim_end().ends_with(')')),
                _ => false,
            };
            let same = prev == Some(e);
            let join = same
                && joinable
                && (gap <= 1.25
                    || (matches!(e, Element::Dialogue | Element::Parenthetical) && gap <= dialogue_gap)
                    || (continue_speech && e == Element::Dialogue)
                    || (new_page
                        && first_on_page
                        && e == Element::Action
                        && b.paras.last().is_some_and(|(_, x, _, _)| !x.text().trim_end().ends_with(['.', '!', '?', ':', '"', '\'', '”', ')']))));
            if join && !(e == Element::SceneHeading && l.scene_number.is_some()) {
                b.cont(&t);
            } else {
                if e == Element::Character {
                    last_cue = t.text();
                }
                b.start(e, t);
                if let Some(last) = b.paras.last_mut() {
                    last.3 = l.scene_number.clone();
                }
            }
            continue_speech = false;
            first_on_page = false;
            last = Some((l.page, l.y));
        }
    }
    if let Some(d) = dual.take() {
        flush_dual(&mut b, d);
    }
    let paras = b.finish();
    if paras.is_empty() {
        notes.push("no script text was recognised".to_string());
    } else {
        script.paras = paras;
    }
    if script.paras.iter().any(|p| p.scene_number.is_some()) {
        script.settings.show_scene_numbers = true;
    }
    if skipped > 0 {
        notes.push(format!("{skipped} speech{} continued across a page break joined", if skipped == 1 { "" } else { "es" }));
    }
    notes.push(format!("action margin at {base:.2}\" (elements told by indent)"));
    Imported { script, notes }
}

/// A line that could be a character cue: short, in capitals, not a heading or transition.
fn cue_like(t: &str) -> bool {
    let t = t.trim();
    shape::is_caps(t)
        && t.chars().count() <= 45
        && !t.starts_with('(')
        && !t.ends_with(':')
        && !shape::is_heading(t)
        && !shape::is_transition_text(t)
        && !shape::is_break_marker(t)
}

/// Elements by context, for PDFs not laid out like a screenplay (a stage play, odd margins):
/// `None` when the dialogue sits where a screenplay's does (about 1" in from the action margin).
fn context_elements(
    pages: &[Page],
    first_body: usize,
    base: f32,
    right: f32,
    lh: f32,
    dialogue_gap: f32,
) -> Option<std::collections::HashMap<(usize, usize), Element>> {
    // Cues and the line after each.
    let mut after: Vec<f32> = Vec::new();
    let mut cues = std::collections::HashSet::new();
    for p in pages.iter().skip(first_body) {
        for (i, w) in p.lines.windows(2).enumerate() {
            let (Some(a), Some(n)) = (w.first(), w.get(1)) else { continue };
            let gap = (n.y - a.y) / lh;
            let mid = p.width / 2.0;
            let centred = ((a.x() + a.x_end()) / 2.0 - mid).abs() < 0.3;
            if a.segs.len() == 1 && cue_like(&a.text()) && (a.x() >= base + 0.5 || centred) && gap <= dialogue_gap && !cue_like(&n.text()) {
                cues.insert((a.page, i));
                if !n.text().trim_start().starts_with('(') {
                    after.push(n.x() - base);
                }
            }
        }
    }
    if after.is_empty() {
        return None;
    }
    after.sort_by(f32::total_cmp);
    let typical = after.get(after.len() / 2).copied().unwrap_or(1.0);
    if (typical - 1.0).abs() < 0.35 {
        return None;
    }
    let mut out = std::collections::HashMap::new();
    let mut in_speech = false;
    let mut open_paren = false;
    let mut last: Option<(usize, f32)> = None;
    for p in pages.iter().skip(first_body) {
        let mid = p.width / 2.0;
        for (i, l) in p.lines.iter().enumerate() {
            let gap = match last {
                Some((pg, y)) if pg == l.page => (l.y - y) / lh,
                _ => f32::INFINITY,
            };
            if gap > dialogue_gap + 0.01 {
                in_speech = false;
                open_paren = false;
            }
            let t = l.text();
            let t = t.trim();
            let centred = ((l.x() + l.x_end()) / 2.0 - mid).abs() < 0.3;
            let e = if cues.contains(&(l.page, i)) {
                in_speech = true;
                Element::Character
            } else if in_speech {
                if t.starts_with('(') || open_paren {
                    open_paren = !t.ends_with(')');
                    Element::Parenthetical
                } else {
                    Element::Dialogue
                }
            } else if shape::is_heading(t) || (centred && shape::is_caps(t) && t.to_uppercase().starts_with("SCENE")) {
                Element::SceneHeading
            } else if ((l.x() + l.x_end()) / 2.0 - mid).abs() < 0.12 && shape::is_caps(t) && !t.ends_with(':') {
                Element::General
            } else if shape::is_shot(t) {
                Element::Shot
            } else if shape::is_transition_text(t) || (l.x_end() >= right - 0.25 && l.x() > mid && shape::is_caps(t)) {
                Element::Transition
            } else {
                Element::Action
            };
            out.insert((l.page, i), e);
            last = Some((l.page, l.y));
        }
    }
    Some(out)
}

#[cfg(test)]
pub(crate) mod tests;
