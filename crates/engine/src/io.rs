//! Reading and writing script files by extension: `.fountain` (also `.spmd`, `.txt`), `.fdx`;
//! `.pdf` is export only.

use std::path::Path;

use scriptcraft_doc::Script;

/// Largest file we open.
pub const MAX_FILE: u64 = 128 << 20;

/// A file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Fountain,
    Fdx,
    Pdf,
}

impl Format {
    /// The format for a path's extension.
    pub fn of(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "fountain" | "spmd" | "txt" => Format::Fountain,
            "fdx" => Format::Fdx,
            "pdf" => Format::Pdf,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Format::Fountain => "Fountain",
            Format::Fdx => "Final Draft (FDX)",
            Format::Pdf => "PDF",
        }
    }
}

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
    match format {
        Format::Fountain => Ok(scriptcraft_fountain::parse(&decode(bytes))),
        Format::Fdx => scriptcraft_fdx::parse(&decode(bytes)).map_err(|e| e.to_string()),
        Format::Pdf => Err("PDF files can be exported but not opened".into()),
    }
}

/// Encode a script in a format.
pub fn encode(script: &Script, format: Format) -> Result<Vec<u8>, String> {
    match format {
        Format::Fountain => Ok(scriptcraft_fountain::write(script).into_bytes()),
        Format::Fdx => Ok(scriptcraft_fdx::write(script).into_bytes()),
        Format::Pdf => scriptcraft_pdf::export(script).map_err(|e| e.to_string()),
    }
}

/// Open a script file.
pub fn read(path: &Path) -> Result<Script, String> {
    let format = Format::of(path).ok_or_else(|| format!("unknown file type: {}", path.display()))?;
    let meta = std::fs::metadata(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    if meta.len() > MAX_FILE {
        return Err(format!("{} is too large to open", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse(&bytes, format)
}

/// Write a script file (format by extension), through a temporary file so a failed write never
/// leaves half a script behind.
pub fn write(script: &Script, path: &Path) -> Result<Format, String> {
    let format = Format::of(path).ok_or_else(|| format!("unknown file type (use .fountain, .fdx or .pdf): {}", path.display()))?;
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
