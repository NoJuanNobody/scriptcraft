//! Page setup: paper size, headers and footers.

use scriptcraft_doc::page_setup::{HeaderFooter, Paper, clean_slot};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("page.get", "Page Setup", "Document › Page Setup", get).pure(),
        CommandSpec::new("page.set", "Change Page Setup", "Document › Page Setup", set).params(
            r#"{"paper": "letter"|"a4", "header": {"left", "center", "right": text with {page} {pages} {title} {author} {date} {revision} {revname} {revdate}}, "footer": {…same}, "headerFirstPage": bool, "footerFirstPage": bool, "headerOnTitlePage": bool, "titlePage": bool}"#,
        ),
        CommandSpec::new("page.reset", "Reset Page Setup", "Document › Page Setup", reset),
    ]
}

fn hf_json(h: &HeaderFooter) -> Value {
    json!({"left": h.left, "center": h.center, "right": h.right})
}

pub(crate) fn get(s: &mut Session, _: &Value) -> CmdResult {
    let pages = s.layout().script_pages();
    let st = &s.script.settings;
    let pg = &st.page;
    Ok(json!({
        "paper": pg.paper.id(),
        "paperName": pg.paper.name(),
        "widthPt": pg.paper.width_pt(),
        "heightPt": pg.paper.height_pt(),
        "linesPerPage": scriptcraft_layout::body_rows(pg.paper),
        "header": hf_json(&pg.header),
        "footer": hf_json(&pg.footer),
        "headerFirstPage": pg.header_first_page,
        "footerFirstPage": pg.footer_first_page,
        "headerOnTitlePage": pg.header_on_title_page,
        "titlePage": st.title_page,
        "pages": pages,
        "fields": ["{page}", "{pages}", "{title}", "{author}", "{date}", "{revision}", "{revname}", "{revdate}"],
    }))
}

fn apply_hf(h: &mut HeaderFooter, v: Option<&Value>, what: &str) -> Result<(), CmdError> {
    let Some(v) = v else { return Ok(()) };
    let m = v.as_object().ok_or_else(|| CmdError::Params(format!("`{what}` must be an object with left/center/right")))?;
    for (k, val) in m {
        let text = val.as_str().ok_or_else(|| CmdError::Params(format!("`{what}.{k}` must be a string")))?;
        let slot = h.slot_mut(k).ok_or_else(|| CmdError::Params(format!("`{what}`: unknown slot `{k}` (left, center or right)")))?;
        *slot = clean_slot(text);
    }
    Ok(())
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let mut pg = s.script.settings.page.clone();
    if let Some(name) = p::str(v, "paper") {
        pg.paper = Paper::parse(name).ok_or_else(|| CmdError::Params(format!("unknown paper `{name}` (letter or a4)")))?;
    }
    apply_hf(&mut pg.header, v.get("header"), "header")?;
    apply_hf(&mut pg.footer, v.get("footer"), "footer")?;
    if let Some(b) = p::bool(v, "headerFirstPage") {
        pg.header_first_page = b;
    }
    if let Some(b) = p::bool(v, "footerFirstPage") {
        pg.footer_first_page = b;
    }
    if let Some(b) = p::bool(v, "headerOnTitlePage") {
        pg.header_on_title_page = b;
    }
    if let Some(b) = p::bool(v, "titlePage") {
        s.script.settings.title_page = b;
    }
    s.script.settings.page = pg;
    s.invalidate();
    get(s, v)
}

fn reset(s: &mut Session, v: &Value) -> CmdResult {
    let paper = s.script.settings.page.paper;
    s.script.settings.page = Default::default();
    s.script.settings.page.paper = paper;
    s.invalidate();
    get(s, v)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn page_setup_commands() {
        let mut s = Session::sample();
        let v = s.run("page.get", &json!({})).unwrap();
        assert_eq!(v["paper"], "letter");
        assert_eq!(v["linesPerPage"], 54);
        assert_eq!(v["header"]["right"], "{revision}  {page}.");
        let letter_pages = v["pages"].as_u64().unwrap();
        let v = s.run("page.set", &json!({"paper": "A4", "footer": {"center": "{title}\nx"}, "headerFirstPage": true})).unwrap();
        assert_eq!(v["paper"], "a4");
        assert_eq!(v["linesPerPage"], 58);
        assert_eq!(v["footer"]["center"], "{title} x");
        assert!(v["pages"].as_u64().unwrap() <= letter_pages);
        assert!(s.run("page.set", &json!({"paper": "legal"})).is_err());
        assert!(s.run("page.set", &json!({"header": {"middle": "x"}})).is_err());
        assert!(s.run("page.set", &json!({"header": "x"})).is_err());
        assert_eq!(s.script.settings.page.paper, scriptcraft_doc::Paper::A4, "failed commands change nothing");
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.script.settings.page.paper, scriptcraft_doc::Paper::Letter, "undoable");
        s.run("edit.redo", &json!({})).unwrap();
        let v = s.run("page.reset", &json!({})).unwrap();
        assert_eq!(v["paper"], "a4", "reset keeps the paper");
        assert_eq!(v["footer"]["center"], "");
    }
}
