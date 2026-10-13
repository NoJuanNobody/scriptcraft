//! PDF export, written with krilla: each laid-out page becomes a US Letter PDF page with the
//! text set in embedded (subsetted) Courier Prime, so it is selectable and searchable. Bold,
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

/// Export an existing layout of `script`.
pub fn export_layout(script: &Script, l: &Layout) -> Result<Vec<u8>, PdfError> {
    let load = |b: &'static [u8]| Font::new(b.to_vec().into(), 0).ok_or(PdfError::Font);
    let faces =
        [load(scriptcraft_fonts::REGULAR)?, load(scriptcraft_fonts::BOLD)?, load(scriptcraft_fonts::ITALIC)?, load(scriptcraft_fonts::BOLD_ITALIC)?];
    let face = |st: Style| faces[usize::from(st.bold) + 2 * usize::from(st.italic)].clone();
    let mut doc = Document::new();
    let settings = || PageSettings::from_wh(l.page_width(), l.paper.height_pt());
    for page in &l.pages {
        let Some(ps) = settings() else { return Err(PdfError::Write("bad page size".into())) };
        let mut p = doc.start_page_with(ps);
        let mut surface = p.surface();
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
