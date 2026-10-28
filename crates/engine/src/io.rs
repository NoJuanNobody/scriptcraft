//! Reading and writing script files by extension: `.fountain` (also `.spmd`, `.txt`), `.fdx`;
//! `.pdf` is exported, and imported by where its text sits; `.docx` and `.celtx` are imported
//! only (see `scriptcraft-import`).

use std::path::Path;

use scriptcraft_doc::{Script, Sluglines};

/// Largest file we open.
pub const MAX_FILE: u64 = 128 << 20;

/// A file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Fountain,
    Fdx,
    Pdf,
    Docx,
    Celtx,
}

impl Format {
    /// The format for a path's extension.
    pub fn of(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "fountain" | "spmd" | "txt" => Format::Fountain,
            "fdx" => Format::Fdx,
            "pdf" => Format::Pdf,
            "docx" => Format::Docx,
            "celtx" => Format::Celtx,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Format::Fountain => "Fountain",
            Format::Fdx => "Final Draft (FDX)",
            Format::Pdf => "PDF",
            Format::Docx => "Word (.docx)",
            Format::Celtx => "Celtx",
        }
    }
    /// The usual file extension.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Fountain => "fountain",
            Format::Fdx => "fdx",
            Format::Pdf => "pdf",
            Format::Docx => "docx",
            Format::Celtx => "celtx",
        }
    }
    /// Opened by importing (ScriptCraft can't save the script back in this format).
    pub fn imported(self) -> bool {
        self.import_kind().is_some()
    }
    fn import_kind(self) -> Option<scriptcraft_import::Kind> {
        match self {
            Format::Pdf => Some(scriptcraft_import::Kind::Pdf),
            Format::Docx => Some(scriptcraft_import::Kind::Docx),
            Format::Celtx => Some(scriptcraft_import::Kind::Celtx),
            Format::Fountain | Format::Fdx => None,
        }
    }
}

/// Extensions that open (natively or by import).
pub const OPEN_EXTENSIONS: [&str; 7] = ["fountain", "fdx", "spmd", "txt", "docx", "celtx", "pdf"];
/// Extensions that are imported.
pub const IMPORT_EXTENSIONS: [&str; 3] = scriptcraft_import::Kind::EXTENSIONS;

/// Decode text: UTF-8 (BOM dropped), else Latin-1.
pub fn decode(bytes: &[u8]) -> String {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| c as char).collect(),
    }
}

/// Parse file contents in a format.
pub fn parse(bytes: &[u8], format: Format) -> Result<Script, String> {
    parse_noted(bytes, format).map(|(s, _)| s)
}

/// Parse file contents, with notes on how an imported file was read.
pub fn parse_noted(bytes: &[u8], format: Format) -> Result<(Script, Vec<String>), String> {
    parse_noted_with(bytes, format, &Sluglines::default())
}

/// [`parse_noted`] recognising Fountain scene headings by the user's `words`.
pub fn parse_noted_with(bytes: &[u8], format: Format, words: &Sluglines) -> Result<(Script, Vec<String>), String> {
    match format {
        Format::Fountain => Ok((scriptcraft_fountain::parse_with(&decode(bytes), words), Vec::new())),
        Format::Fdx => scriptcraft_fdx::parse(&decode(bytes)).map(|s| (s, Vec::new())).map_err(|e| e.to_string()),
        Format::Pdf | Format::Docx | Format::Celtx => {
            let kind = format.import_kind().ok_or("not an imported format")?;
            scriptcraft_import::import(bytes, kind).map(|r| (r.script, r.notes))
        }
    }
}

/// Encode a script in a format.
pub fn encode(script: &Script, format: Format) -> Result<Vec<u8>, String> {
    match format {
        Format::Fountain => Ok(scriptcraft_fountain::write(script).into_bytes()),
        Format::Fdx => Ok(scriptcraft_fdx::write(script).into_bytes()),
        Format::Pdf => scriptcraft_pdf::export(script).map_err(|e| e.to_string()),
        Format::Docx | Format::Celtx => Err(format!("ScriptCraft opens {} files but can't save them; save as .fdx or .fountain", format.name())),
    }
}

/// Open a script file.
pub fn read(path: &Path) -> Result<Script, String> {
    read_noted(path).map(|(s, _, _)| s)
}

/// Open a script file: the script, its format and notes on how an imported file was read.
pub fn read_noted(path: &Path) -> Result<(Script, Format, Vec<String>), String> {
    read_noted_with(path, &Sluglines::default())
}

/// [`read_noted`] recognising Fountain scene headings by the user's `words`.
pub fn read_noted_with(path: &Path, words: &Sluglines) -> Result<(Script, Format, Vec<String>), String> {
    let format = Format::of(path).ok_or_else(|| format!("unknown file type: {}", path.display()))?;
    let meta = std::fs::metadata(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    if meta.len() > MAX_FILE {
        return Err(format!("{} is too large to open", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let (script, notes) = parse_noted_with(&bytes, format, words)?;
    Ok((script, format, notes))
}

/// Write a script file (format by extension), through a temporary file so a failed write never
/// leaves half a script behind.
pub fn write(script: &Script, path: &Path) -> Result<Format, String> {
    let format = Format::of(path).ok_or_else(|| format!("unknown file type (use .fountain, .fdx or .pdf): {}", path.display()))?;
    if matches!(format, Format::Docx | Format::Celtx) {
        return Err(format!("ScriptCraft opens {} files but can't save them; save as .fdx or .fountain", format.name()));
    }
    let bytes = encode(script, format)?;
    write_bytes(path, &bytes)?;
    Ok(format)
}

/// Write any file through a temporary file, so a failed write never leaves half of it behind.
pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("x")));
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot write {}: {e}", path.display())
    })
}
