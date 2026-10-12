//! PDF export, written with krilla: each laid-out page becomes a US Letter PDF page with the
//! text set in embedded (subsetted) Courier Prime, so it is selectable and searchable.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use krilla::Document;
use krilla::geom::Point;
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::text::{Font, TextDirection};
use scriptcraft_doc::Script;
use scriptcraft_geom::{FONT_SIZE, PAGE_H, PAGE_W};
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
    let font = Font::new(scriptcraft_fonts::REGULAR.to_vec().into(), 0).ok_or(PdfError::Font)?;
    let mut doc = Document::new();
    let settings = || PageSettings::from_wh(PAGE_W, PAGE_H);
    for page in &l.pages {
        let Some(ps) = settings() else { return Err(PdfError::Write("bad page size".into())) };
        let mut p = doc.start_page_with(ps);
        let mut surface = p.surface();
        for line in &page.lines {
            let text = line.display();
            if text.trim().is_empty() {
                continue;
            }
            surface.draw_text(Point::from_xy(line.x, line.baseline()), font.clone(), FONT_SIZE, &text, false, TextDirection::Auto);
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
    }
}
