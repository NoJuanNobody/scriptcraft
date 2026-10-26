//! Print: the pages to print, rendered as PDF. The window sends the PDF to a printer (`print.send`,
//! `print.printers`: CUPS, the Windows print action, or the browser's print window), or opens it in
//! the system's viewer; agents get the bytes from `print.render`.

use scriptcraft_pdf::Selection;
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("print.pages", "Pages to Print", "File › Print", pages)
            .params(r#"{"from": page or label ("12A"), "to": page or label, "titlePage": bool (default true), "revisedOnly": bool} → {"pages": ["title", "1", "2", …]}"#)
            .pure(),
        CommandSpec::new("print.render", "Print", "File › Print", render)
            .params(r#"{"from": page or label ("12A"), "to": page or label, "titlePage": bool (default true), "revisedOnly": bool} → {"name", "data": base64 PDF, "pages", "bytes"}"#)
            .pure(),
        CommandSpec::new("print.printers", "Printers", "File › Print", needs_app)
            .params(r#"{"refresh": bool (ask the system again)} → {"printers": [{"name", "default"}], "default", "engine", "choosesInDialog": bool (the browser picks the printer in its own print window)}"#)
            .pure(),
        CommandSpec::new("print.send", "Print to Printer", "File › Print", needs_app)
            .params(r#"{"printer": name from print.printers (default: the system's default), "copies": 1–99, "from", "to", "titlePage", "revisedOnly" (as print.render)} → {"name", "pages", "printer", "copies", "sent": true}; the outcome shows in the status bar"#)
            .pure(),
    ]
}

/// Printers belong to the computer running the window, not to a headless session.
fn needs_app(_: &mut Session, _: &Value) -> CmdResult {
    Err(CmdError::Disabled("printing to a printer needs the ScriptCraft window; print.render gives the PDF".into()))
}

/// The page selection in `v`.
pub(crate) fn selection(v: &Value) -> Result<Selection, CmdError> {
    let num = |k: &str| -> Result<Option<usize>, CmdError> {
        match v.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => p::usize(v, k).map(Some).ok_or_else(|| CmdError::Params(format!("`{k}` must be a page number"))),
        }
    };
    let (from, to) = (num("from")?, num("to")?);
    if let (Some(f), Some(t)) = (from, to)
        && f > t
    {
        return Err(CmdError::Params(format!("page range {f}–{t} is backwards")));
    }
    Ok(Selection { from, to, title_page: p::bool(v, "titlePage").unwrap_or(true), revised_only: p::bool(v, "revisedOnly").unwrap_or(false) })
}

/// Indices of the selected pages; an error when none.
fn chosen(s: &mut Session, v: &Value) -> Result<(std::sync::Arc<scriptcraft_layout::Layout>, Vec<usize>), CmdError> {
    let l = s.layout();
    let sel = selection(&with_labels_resolved(v, &l)?)?;
    let which = sel.pages(&l);
    if which.is_empty() {
        return Err(CmdError::Failed(format!("no pages to print (the script has {} pages)", l.script_pages())));
    }
    Ok((l, which))
}

/// `from`/`to` given as printed labels (`"12A"`, locked pages) become page numbers.
fn with_labels_resolved(v: &Value, l: &scriptcraft_layout::Layout) -> Result<Value, CmdError> {
    let mut v = v.clone();
    for k in ["from", "to"] {
        if let Some(label) = v.get(k).and_then(Value::as_str).map(|t| t.trim().trim_end_matches('.').to_string()) {
            let n = match label.parse::<usize>() {
                Ok(n) if !l.pages.iter().any(|pg| !pg.title_page && pg.label.eq_ignore_ascii_case(&label)) => Some(n),
                _ => l.pages.iter().find(|pg| !pg.title_page && pg.label.eq_ignore_ascii_case(&label)).and_then(|pg| pg.number),
            };
            let n = n.ok_or_else(|| CmdError::Params(format!("no page {label}")))?;
            if let Some(o) = v.as_object_mut() {
                o.insert(k.to_string(), json!(n));
            }
        }
    }
    Ok(v)
}

/// What each chosen page prints as: `title`, `12`, or `12A` when pages are locked.
fn labels(l: &scriptcraft_layout::Layout, which: &[usize]) -> Vec<String> {
    which
        .iter()
        .filter_map(|i| l.pages.get(*i))
        .map(|p| {
            if p.title_page {
                "title".to_string()
            } else if p.label.is_empty() {
                p.number.unwrap_or(0).to_string()
            } else {
                p.label.clone()
            }
        })
        .collect()
}

fn pages(s: &mut Session, v: &Value) -> CmdResult {
    let (l, which) = chosen(s, v)?;
    Ok(json!({"pages": labels(&l, &which), "count": which.len()}))
}

fn render(s: &mut Session, v: &Value) -> CmdResult {
    let (l, which) = chosen(s, v)?;
    let bytes = scriptcraft_pdf::export_pages(&s.script, &l, &which).map_err(|e| CmdError::Failed(e.to_string()))?;
    let stem = s.path.as_ref().and_then(|p| p.file_stem()).and_then(|x| x.to_str()).map(str::to_string).unwrap_or_else(|| {
        let t = s.script.title.title.lines().next().unwrap_or("").trim();
        if t.is_empty() { "Untitled".to_string() } else { t.to_string() }
    });
    Ok(json!({"name": format!("{stem}.pdf"), "data": crate::b64::encode(&bytes), "bytes": bytes.len(), "pages": labels(&l, &which)}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn print_ranges() {
        let mut s = Session::sample();
        let all = s.run("print.pages", &json!({})).unwrap();
        let n = all["count"].as_u64().unwrap();
        assert_eq!(all["pages"][0], "title");
        assert!(n >= 3);
        let v = s.run("print.pages", &json!({"from": 2, "to": 3, "titlePage": false})).unwrap();
        assert_eq!(v["pages"], json!(["2", "3"]));
        let v = s.run("print.pages", &json!({"to": 1})).unwrap();
        assert_eq!(v["pages"], json!(["title", "1"]));
        assert!(s.run("print.pages", &json!({"from": 3, "to": 2})).is_err());
        assert!(s.run("print.pages", &json!({"from": 999, "titlePage": false})).is_err());
        assert!(s.run("print.pages", &json!({"from": "x"})).is_err());
        let r = s.run("print.render", &json!({"from": 2, "to": 2, "titlePage": false})).unwrap();
        let pdf = crate::b64::decode(r["data"].as_str().unwrap()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let full = s.run("print.render", &json!({})).unwrap();
        assert!(full["bytes"].as_u64().unwrap() > r["bytes"].as_u64().unwrap());
        assert!(r["name"].as_str().unwrap().ends_with(".pdf"));
    }
}
