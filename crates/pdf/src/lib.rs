//! PDF export, written with krilla: each laid-out page becomes a US Letter PDF page with the
//! text set in embedded (subsetted) Courier Badi, so it is selectable and searchable. Bold,
//! italic and bold italic use the matching faces; underlines are drawn as thin rules.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use krilla::Document;
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::Fill;
use krilla::text::{Font, TextDirection};
use scriptcraft_doc::Script;
use scriptcraft_doc::Style;
use scriptcraft_geom::{CHAR_W, FONT_SIZE};
use scriptcraft_layout::{Layout, layout};

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum PdfError {
    #[error("the embedded font could not be loaded")]
    Font,
    #[error("PDF writing failed: {0}")]
    Write(String),
}

/// Export a script as PDF bytes.
pub fn export(script: &Script) -> Result<Vec<u8>, PdfError> {
    export_layout(script, &layout(script))
}

/// Which pages to print or export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// First script page number (inclusive); `None` = from the start.
    pub from: Option<usize>,
    /// Last script page number (inclusive); `None` = to the end.
    pub to: Option<usize>,
    /// Include the title page (when the script has one).
    pub title_page: bool,
    /// Only pages with revision marks (the title page still follows `title_page`).
    pub revised_only: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Selection { from: None, to: None, title_page: true, revised_only: false }
    }
}

impl Selection {
    /// Whether this selection keeps a page.
    pub fn keeps(&self, page: &scriptcraft_layout::Page) -> bool {
        if page.title_page {
            return self.title_page;
        }
        let n = page.number.unwrap_or(0);
        self.from.is_none_or(|f| n >= f) && self.to.is_none_or(|t| n <= t)
    }

    /// Indices (into `l.pages`) of the pages kept.
    pub fn pages(&self, l: &Layout) -> Vec<usize> {
        let revised = |i: usize| l.page_rev.get(i).copied().flatten().is_some();
        l.pages.iter().enumerate().filter(|(i, p)| self.keeps(p) && (p.title_page || !self.revised_only || revised(*i))).map(|(i, _)| i).collect()
    }
}

/// Export an existing layout of `script`.
pub fn export_layout(script: &Script, l: &Layout) -> Result<Vec<u8>, PdfError> {
    export_pages(script, l, &(0..l.pages.len()).collect::<Vec<_>>())
}

/// Export some pages of a layout (indices into `l.pages`, in order; unknown ones are skipped).
pub fn export_pages(script: &Script, l: &Layout, which: &[usize]) -> Result<Vec<u8>, PdfError> {
    let load = |b: &'static [u8]| Font::new(b.to_vec().into(), 0).ok_or(PdfError::Font);
    let faces =
        [load(scriptcraft_fonts::REGULAR)?, load(scriptcraft_fonts::BOLD)?, load(scriptcraft_fonts::ITALIC)?, load(scriptcraft_fonts::BOLD_ITALIC)?];
    let face = |st: Style| faces[usize::from(st.bold) + 2 * usize::from(st.italic)].clone();
    let mut doc = Document::new();
    let settings = || PageSettings::from_wh(l.page_width(), l.paper.height_pt());
    let rev = &script.settings.revision;
    for (page, idx) in which.iter().filter_map(|i| l.pages.get(*i).map(|p| (p, *i))) {
        let Some(ps) = settings() else { return Err(PdfError::Write("bad page size".into())) };
        let mut p = doc.start_page_with(ps);
        let mut surface = p.surface();
        // Coloured revision pages: the page tinted with its revision set's colour.
        if rev.color_pages
            && let Some(set) = l.page_rev.get(idx).copied().flatten().and_then(|lvl| rev.set(lvl))
            && set.color != [0xff, 0xff, 0xff]
        {
            let [r, g, b] = set.color;
            let mut pb = PathBuilder::new();
            if let Some(rect) = Rect::from_xywh(0.0, 0.0, l.page_width(), l.paper.height_pt()) {
                pb.push_rect(rect);
            }
            if let Some(path) = pb.finish() {
                surface.set_fill(Some(Fill { paint: krilla::color::rgb::Color::new(r, g, b).into(), ..Fill::default() }));
                surface.draw_path(&path);
                surface.set_fill(Some(Fill::default()));
            }
        }
        for line in &page.lines {
            for (col, text, st) in line.segments() {
                let x = line.x + col as f32 * CHAR_W;
                if !text.trim().is_empty() {
                    surface.draw_text(Point::from_xy(x, line.baseline()), face(st), FONT_SIZE, text, false, TextDirection::Auto);
                }
                if st.underline {
                    let w = text.chars().count() as f32 * CHAR_W;
                    let mut pb = PathBuilder::new();
                    if let Some(r) = Rect::from_xywh(x, line.baseline() + 1.6, w, 0.6) {
                        pb.push_rect(r);
                    }
                    if let Some(path) = pb.finish() {
                        surface.set_fill(Some(Fill::default()));
                        surface.draw_path(&path);
                    }
                }
            }
            if !line.suffix.trim().is_empty() {
                let x = line.x + line.text.chars().count() as f32 * CHAR_W;
                surface.draw_text(Point::from_xy(x, line.baseline()), face(Style::PLAIN), FONT_SIZE, &line.suffix, false, TextDirection::Auto);
            }
        }
        surface.finish();
        p.finish();
    }
    let t = &script.title;
    let mut meta = Metadata::new().creator("ScriptCraft".to_string()).producer("ScriptCraft (krilla)".to_string());
    if !t.title.trim().is_empty() {
        meta = meta.title(t.title.trim().to_string());
    }
    if !t.author.trim().is_empty() {
        meta = meta.authors(vec![t.author.trim().to_string()]);
    }
    doc.set_metadata(meta);
    doc.finish().map_err(|e| PdfError::Write(format!("{e:?}")))
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Element, Para};

    use super::*;

    #[test]
    fn russian_script_exports_in_every_style() {
        let mut s = Script::from_paras(vec![Para::new(Element::Action, "Ёжик ждёт у окна. Алёна говорит: «Всё готово?»")]).unwrap();
        s.title.title = "Ночной разговор".into();
        s.title.author = "Алёна Ёлкина".into();
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let end = s.paras[0].text.len();
            s.set_flag(scriptcraft_doc::Pos::new(0, 0), scriptcraft_doc::Pos::new(0, end), scriptcraft_doc::Flag::Bold, bold);
            s.set_flag(scriptcraft_doc::Pos::new(0, 0), scriptcraft_doc::Pos::new(0, end), scriptcraft_doc::Flag::Italic, italic);
            assert!(export(&s).unwrap().starts_with(b"%PDF-"));
        }
    }

    #[test]
    fn writes_pages() {
        let mut s = Script::from_paras((0..80).map(|i| Para::new(Element::Action, format!("Line {i} & more."))).collect()).unwrap();
        s.title.title = "Test".into();
        let bytes = export(&s).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 1000);
        assert!(export(&Script::new()).unwrap().starts_with(b"%PDF-"));
        let mut st = Script::from_paras(vec![Para::new(Element::Action, "Plain bold italic under")]).unwrap();
        st.set_flag(scriptcraft_doc::Pos::new(0, 6), scriptcraft_doc::Pos::new(0, 10), scriptcraft_doc::Flag::Bold, true);
        st.set_flag(scriptcraft_doc::Pos::new(0, 11), scriptcraft_doc::Pos::new(0, 17), scriptcraft_doc::Flag::Italic, true);
        st.set_flag(scriptcraft_doc::Pos::new(0, 18), scriptcraft_doc::Pos::new(0, 23), scriptcraft_doc::Flag::Underline, true);
        let styled = export(&st).unwrap();
        assert!(styled.len() > export(&Script::from_paras(vec![Para::new(Element::Action, "Plain bold italic under")]).unwrap()).unwrap().len());
    }
}
