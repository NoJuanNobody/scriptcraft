//! Files. Programmatic calls never open dialogs: the UI asks for a path first.
//!
//! `file.new`, `file.open` and `file.sample` replace the script and clear the undo history, so
//! they are not undoable steps (`mutates: false`).

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::io::{self, Format};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("file.new", "New Script", "File", new).key("Mod+N").pure(),
        CommandSpec::new("file.sample", "Open Sample Script", "Help", sample).pure(),
        CommandSpec::new("file.open", "Open", "File", open).params(r#"{"path": ".fountain|.fdx file, or .docx|.celtx|.pdf (imported)"}"#).pure(),
        CommandSpec::new("file.import", "Import", "File", import)
            .params(r#"{"path": ".docx|.celtx|.pdf"} or {"name": "draft.docx", "data": base64} → {"paragraphs", "format", "notes"}"#)
            .pure(),
        CommandSpec::new("file.save", "Save", "File", save).key("Mod+S").params(r#"{"path": string (optional once saved)}"#).pure(),
        CommandSpec::new("file.saveAs", "Save As", "File", save_as).key("Mod+Shift+S").params(r#"{"path": ".fountain|.fdx"}"#).pure(),
        CommandSpec::new("file.exportPdf", "Export PDF", "File › Export", export_pdf).params(r#"{"path": ".pdf"}"#).pure(),
        CommandSpec::new("file.exportFdx", "Export Final Draft (FDX)", "File › Export", export_fdx).params(r#"{"path": ".fdx"}"#).pure(),
        CommandSpec::new("file.exportFountain", "Export Fountain", "File › Export", export_fountain).params(r#"{"path": ".fountain"}"#).pure(),
        CommandSpec::new("file.load", "Open from Data", "File", load)
            .params(r#"{"name": "draft.fdx|draft.fountain|draft.docx|draft.celtx|draft.pdf", "text": string} or {"name", "data": base64} (no file system: the browser, agents)"#)
            .pure(),
        CommandSpec::new("document.export", "Export as Data", "File › Export", export_data)
            .params(r#"{"format": "pdf"|"fdx"|"fountain"|"csv" (the breakdown)} → {"name", "data": base64}"#)
            .pure(),
    ]
}

fn new(s: &mut Session, _: &Value) -> CmdResult {
    s.replace_script(scriptcraft_doc::Script::new());
    s.path = None;
    Ok(json!({}))
}

fn sample(s: &mut Session, _: &Value) -> CmdResult {
    s.replace_script(crate::sample::script());
    s.path = None;
    Ok(json!({"paragraphs": s.script.paras.len()}))
}

fn open(s: &mut Session, v: &Value) -> CmdResult {
    let path = PathBuf::from(p::req_str(v, "path")?);
    let (script, format, notes) = io::read_noted_with(&path, &s.sluglines).map_err(CmdError::Failed)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("the file").to_string();
    if format.imported() {
        return Ok(imported(s, script, format, &name, notes));
    }
    s.replace_script(script);
    s.status = format!("Opened {}", path.display());
    s.path = Some(path);
    Ok(json!({"paragraphs": s.script.paras.len()}))
}

/// Take an imported script: not tied to its file (it can't be saved back), so it starts unsaved.
fn imported(s: &mut Session, script: scriptcraft_doc::Script, format: Format, name: &str, notes: Vec<String>) -> Value {
    s.replace_script(script);
    s.path = None;
    s.dirty = true;
    s.status = format!("Imported {name}; save it as .fdx or .fountain to keep it");
    json!({"paragraphs": s.script.paras.len(), "format": format.name(), "notes": notes, "name": name})
}

fn import(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(path) = p::str(v, "path") {
        let path = PathBuf::from(path);
        let format =
            Format::of(&path).filter(|f| f.imported()).ok_or_else(|| CmdError::Params("import takes a .docx, .celtx or .pdf file".into()))?;
        let (script, _, notes) = io::read_noted_with(&path, &s.sluglines).map_err(CmdError::Failed)?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("the file").to_string();
        return Ok(imported(s, script, format, &name, notes));
    }
    let name = p::req_str(v, "name")?.to_string();
    let format = Format::of(std::path::Path::new(&name))
        .filter(|f| f.imported())
        .ok_or_else(|| CmdError::Params("import takes a .docx, .celtx or .pdf file".into()))?;
    let bytes = crate::b64::decode(p::req_str(v, "data")?).map_err(CmdError::Params)?;
    if bytes.len() as u64 > io::MAX_FILE {
        return Err(CmdError::Failed(format!("{name} is too large to open")));
    }
    let (script, notes) = io::parse_noted_with(&bytes, format, &s.sluglines).map_err(CmdError::Failed)?;
    Ok(imported(s, script, format, &name, notes))
}

fn write_to(s: &mut Session, path: PathBuf, keep: bool) -> CmdResult {
    let format = io::write(&s.script, &path).map_err(CmdError::Failed)?;
    s.status = format!("Saved {} ({})", path.display(), format.name());
    if keep && format != Format::Pdf {
        s.path = Some(path.clone());
        s.dirty = false;
    }
    Ok(json!({"path": path.display().to_string(), "format": format.name()}))
}

fn save(s: &mut Session, v: &Value) -> CmdResult {
    let path = match p::str(v, "path") {
        Some(p) => PathBuf::from(p),
        None => s.path.clone().ok_or_else(|| CmdError::Params("`path` is required (the script has not been saved yet)".into()))?,
    };
    write_to(s, path, true)
}

fn save_as(s: &mut Session, v: &Value) -> CmdResult {
    let path = PathBuf::from(p::req_str(v, "path")?);
    write_to(s, path, true)
}

fn export(s: &mut Session, v: &Value, want: Format) -> CmdResult {
    let path = PathBuf::from(p::req_str(v, "path")?);
    if Format::of(&path) != Some(want) {
        return Err(CmdError::Params(format!("the path must end in .{}", want.ext())));
    }
    write_to(s, path, false)
}

fn export_pdf(s: &mut Session, v: &Value) -> CmdResult {
    export(s, v, Format::Pdf)
}
fn export_fdx(s: &mut Session, v: &Value) -> CmdResult {
    export(s, v, Format::Fdx)
}
fn export_fountain(s: &mut Session, v: &Value) -> CmdResult {
    export(s, v, Format::Fountain)
}

fn load(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?;
    let format = Format::of(std::path::Path::new(name)).ok_or_else(|| CmdError::Params(format!("unknown file type: {name}")))?;
    let bytes = match (p::str(v, "text"), p::str(v, "data")) {
        (Some(t), _) => t.as_bytes().to_vec(),
        (None, Some(d)) => crate::b64::decode(d).map_err(CmdError::Params)?,
        _ => return Err(CmdError::Params("`text` or `data` (base64) is required".into())),
    };
    if bytes.len() as u64 > io::MAX_FILE {
        return Err(CmdError::Failed(format!("{name} is too large to open")));
    }
    let (script, notes) = io::parse_noted_with(&bytes, format, &s.sluglines).map_err(CmdError::Failed)?;
    if format.imported() {
        return Ok(imported(s, script, format, name, notes));
    }
    s.replace_script(script);
    s.path = None;
    s.status = format!("Opened {name}");
    Ok(json!({"paragraphs": s.script.paras.len(), "name": name}))
}

fn export_data(s: &mut Session, v: &Value) -> CmdResult {
    let f = p::req_str(v, "format")?.to_ascii_lowercase();
    if f == "csv" {
        let l = s.layout();
        let bytes = crate::breakdown::csv(&s.script, &l, &s.sluglines).into_bytes();
        let stem = export_stem(s);
        return Ok(json!({"name": format!("{stem} Breakdown.csv"), "data": crate::b64::encode(&bytes), "bytes": bytes.len()}));
    }
    let (format, ext) = match f.as_str() {
        "pdf" => (Format::Pdf, "pdf"),
        "fdx" => (Format::Fdx, "fdx"),
        "fountain" => (Format::Fountain, "fountain"),
        other => return Err(CmdError::Params(format!("unknown format `{other}` (pdf, fdx, fountain or csv)"))),
    };
    let bytes = io::encode(&s.script, format).map_err(CmdError::Failed)?;
    let stem = export_stem(s);
    Ok(json!({"name": format!("{stem}.{ext}"), "data": crate::b64::encode(&bytes), "bytes": bytes.len()}))
}

/// The file name stem for exports: the script's file, else its title, else Untitled.
fn export_stem(s: &Session) -> String {
    s.path.as_ref().and_then(|p| p.file_stem()).and_then(|x| x.to_str()).map(str::to_string).unwrap_or_else(|| {
        let t = s.script.title.title.trim();
        if t.is_empty() { "Untitled".to_string() } else { t.to_string() }
    })
}
