//! Print: the pages to print, rendered as PDF. The app hands the PDF to the system (the default
//! viewer's print dialog natively, a download in the browser); agents get the bytes.

use scriptcraft_pdf::Selection;
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("print.pages", "Pages to Print", "File › Print", pages)
            .params(r#"{"from": page, "to": page, "titlePage": bool (default true)} → {"pages": ["title", "1", "2", …]}"#)
            .pure(),
        CommandSpec::new("print.render", "Print", "File › Print", render)
            .params(r#"{"from": page, "to": page, "titlePage": bool (default true)} → {"name", "data": base64 PDF, "pages", "bytes"}"#)
            .pure(),
    ]
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
    Ok(Selection { from, to, title_page: p::bool(v, "titlePage").unwrap_or(true) })
}

/// Indices of the selected pages; an error when none.
fn chosen(s: &mut Session, v: &Value) -> Result<(std::sync::Arc<scriptcraft_layout::Layout>, Vec<usize>), CmdError> {
    let sel = selection(v)?;
    let l = s.layout();
    let which = sel.pages(&l);
    if which.is_empty() {
        return Err(CmdError::Failed(format!("no pages to print (the script has {} pages)", l.script_pages())));
    }
    Ok((l, which))
}

fn labels(l: &scriptcraft_layout::Layout, which: &[usize]) -> Vec<String> {
    which.iter().filter_map(|i| l.pages.get(*i)).map(|p| if p.title_page { "title".to_string() } else { p.number.unwrap_or(0).to_string() }).collect()
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
