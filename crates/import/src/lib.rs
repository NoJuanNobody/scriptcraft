//! ScriptCraft L3: import screenplays written in other apps.
//!
//! - **Word (`.docx`)**: paragraph styles named like screenplay elements (Scene Heading, Action,
//!   Character, Dialogue, Parenthetical, Transition, Shot, General and common variants), else the
//!   paragraph's indent and alignment, else (no styles, no indents) the text itself, read the way
//!   Fountain reads plain text. Bold, italic and underline are kept; a first page of centred text
//!   becomes the title page.
//! - **Celtx**: legacy `.celtx` project archives (a ZIP holding the script as HTML whose
//!   paragraphs carry element classes such as `sceneheading`, `action`, `character`, `dialog`,
//!   `parenthetical`, `transition`, `shot`) and that HTML on its own. Current Celtx exports to
//!   Final Draft or PDF, which ScriptCraft opens too.
//! - **PDF**: text with positions; elements told by indent (see [`pdf`]).
//!
//! Every importer is panic-free on any input and caps what it reads.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod celtx;
mod docx;
mod html;
mod pdf;
mod shape;
mod zip;

use scriptcraft_doc::Script;

/// Largest file imported.
pub const MAX_INPUT: usize = 128 << 20;

/// A format that can be imported (opened, not saved).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Docx,
    Celtx,
    Pdf,
}

impl Kind {
    /// The kind for a file extension.
    pub fn of_ext(ext: &str) -> Option<Kind> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "docx" => Kind::Docx,
            "celtx" => Kind::Celtx,
            "pdf" => Kind::Pdf,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Docx => "Word (.docx)",
            Kind::Celtx => "Celtx",
            Kind::Pdf => "PDF",
        }
    }
    /// Every importable extension.
    pub const EXTENSIONS: [&'static str; 3] = ["docx", "celtx", "pdf"];
}

/// An imported script and notes on how it was read.
#[derive(Debug)]
pub struct Imported {
    pub script: Script,
    pub notes: Vec<String>,
}

/// Import a file's bytes.
pub fn import(bytes: &[u8], kind: Kind) -> Result<Imported, String> {
    if bytes.len() > MAX_INPUT {
        return Err("the file is too large to import".into());
    }
    let mut r = match kind {
        Kind::Docx => docx::import(bytes)?,
        Kind::Celtx => celtx::import(bytes)?,
        Kind::Pdf => pdf::import(bytes)?,
    };
    for p in &mut r.script.paras {
        if p.text.len() > scriptcraft_doc::MAX_PARA_BYTES {
            let mut cut = scriptcraft_doc::MAX_PARA_BYTES;
            while cut > 0 && !p.text.is_char_boundary(cut) {
                cut -= 1;
            }
            p.text.truncate(cut);
            p.spans.retain(|s| s.start < cut);
            for s in &mut p.spans {
                s.end = s.end.min(cut);
            }
        }
    }
    r.script.normalize().map_err(|e| e.to_string())?;
    Ok(r)
}

#[cfg(test)]
mod tests;
