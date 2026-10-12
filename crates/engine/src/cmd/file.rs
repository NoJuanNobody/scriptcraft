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
        CommandSpec::new("file.open", "Open", "File", open).params(r#"{"path": ".fountain|.fdx file"}"#).pure(),
        CommandSpec::new("file.save", "Save", "File", save).key("Mod+S").params(r#"{"path": string (optional once saved)}"#).pure(),
        CommandSpec::new("file.saveAs", "Save As", "File", save_as).key("Mod+Shift+S").params(r#"{"path": ".fountain|.fdx"}"#).pure(),
        CommandSpec::new("file.exportPdf", "Export PDF", "File › Export", export_pdf).key("Mod+P").params(r#"{"path": ".pdf"}"#).pure(),
        CommandSpec::new("file.exportFdx", "Export Final Draft (FDX)", "File › Export", export_fdx).params(r#"{"path": ".fdx"}"#).pure(),
        CommandSpec::new("file.exportFountain", "Export Fountain", "File › Export", export_fountain).params(r#"{"path": ".fountain"}"#).pure(),
        CommandSpec::new("file.load", "Open from Data", "File", load)
            .params(r#"{"name": "draft.fdx|draft.fountain", "text": string} or {"name", "data": base64} (no file system: the browser, agents)"#)
            .pure(),
        CommandSpec::new("document.export", "Export as Data", "File › Export", export_data)
            .params(r#"{"format": "pdf"|"fdx"|"fountain"} → {"name", "data": base64}"#)
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
    let script = io::read(&path).map_err(CmdError::Failed)?;
    s.replace_script(script);
    s.status = format!("Opened {}", path.display());
    s.path = Some(path);
    Ok(json!({"paragraphs": s.script.paras.len()}))
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
        return Err(CmdError::Params(format!(
            "the path must end in .{}",
            match want {
                Format::Pdf => "pdf",
                Format::Fdx => "fdx",
                Format::Fountain => "fountain",
            }
        )));
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
    let script = io::parse(&bytes, format).map_err(CmdError::Failed)?;
    s.replace_script(script);
    s.path = None;
    s.status = format!("Opened {name}");
    Ok(json!({"paragraphs": s.script.paras.len(), "name": name}))
}

fn export_data(s: &mut Session, v: &Value) -> CmdResult {
    let f = p::req_str(v, "format")?.to_ascii_lowercase();
    let (format, ext) = match f.as_str() {
        "pdf" => (Format::Pdf, "pdf"),
        "fdx" => (Format::Fdx, "fdx"),
        "fountain" => (Format::Fountain, "fountain"),
        other => return Err(CmdError::Params(format!("unknown format `{other}` (pdf, fdx or fountain)"))),
    };
    let bytes = io::encode(&s.script, format).map_err(CmdError::Failed)?;
    let stem = s.path.as_ref().and_then(|p| p.file_stem()).and_then(|x| x.to_str()).map(str::to_string).unwrap_or_else(|| {
        let t = s.script.title.title.trim();
        if t.is_empty() { "Untitled".to_string() } else { t.to_string() }
    });
    Ok(json!({"name": format!("{stem}.{ext}"), "data": crate::b64::encode(&bytes), "bytes": bytes.len()}))
}
