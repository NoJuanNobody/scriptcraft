//! Queries for agents and the UI: inspect, stats, layout, reports, Fountain/FDX text.

use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p, reports};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("document.inspect", "Inspect Script", "Tools", inspect).params(r#"{"from": n, "count": n (paragraph window)}"#).pure(),
        CommandSpec::new("document.stats", "Script Statistics", "Tools", stats).pure(),
        CommandSpec::new("document.layout", "Page Layout", "Tools", layout)
            .params(r#"{"page": n (1-based index into pages, title page included)}"#)
            .pure(),
        CommandSpec::new("document.fountain", "Script as Fountain", "Tools", fountain).pure(),
        CommandSpec::new("document.setFountain", "Replace Script from Fountain", "Tools", set_fountain).params(r#"{"text": string}"#),
        CommandSpec::new("document.fdx", "Script as FDX", "Tools", fdx).pure(),
        CommandSpec::new("document.setFdx", "Replace Script from FDX", "Tools", set_fdx).params(r#"{"xml": string}"#),
        CommandSpec::new("report.characters", "Character Report", "Reports", characters).pure(),
        CommandSpec::new("report.scenes", "Scene Report", "Reports", scenes).pure(),
        CommandSpec::new("report.locations", "Location Report", "Reports", locations).pure(),
    ]
}

fn to_json<T: serde::Serialize>(v: &T) -> CmdResult {
    serde_json::to_value(v).map_err(|e| CmdError::Failed(e.to_string()))
}

fn inspect(s: &mut Session, v: &Value) -> CmdResult {
    let from = p::usize(v, "from").unwrap_or(0);
    let count = p::usize(v, "count").unwrap_or(500).min(5000);
    let l = s.layout();
    let paras: Vec<Value> = s
        .script
        .paras
        .iter()
        .enumerate()
        .skip(from)
        .take(count)
        .map(|(i, p)| {
            let mut o = json!({"para": i, "element": p.element.id(), "text": p.text, "page": l.page_number_of(i)});
            if let Some(m) = o.as_object_mut() {
                if let Some(n) = &p.scene_number {
                    m.insert("sceneNumber".into(), json!(n));
                }
                if p.dual {
                    m.insert("dual".into(), json!(true));
                }
                if !p.summary.is_empty() {
                    m.insert("summary".into(), json!(p.summary));
                }
                if !p.notes.is_empty() {
                    m.insert("notes".into(), json!(p.notes.len()));
                }
                if !p.tagged.is_empty() {
                    m.insert(
                        "tags".into(),
                        json!(p.tagged.iter().map(|t| json!({"category": t.category, "item": p.slice(t.start, t.end)})).collect::<Vec<_>>()),
                    );
                }
                if !p.alternates.is_empty() {
                    m.insert("alternates".into(), json!(p.alternates.len()));
                }
            }
            o
        })
        .collect();
    Ok(json!({
        "paragraphs": paras,
        "total": s.script.paras.len(),
        "selection": {"anchor": super::pos_json(s.sel.anchor), "focus": super::pos_json(s.sel.focus)},
        "element": s.element().id(),
        "pages": l.script_pages(),
        "title": s.script.title,
        "path": s.path.as_ref().map(|p| p.display().to_string()),
        "dirty": s.dirty,
    }))
}

fn stats(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    Ok(json!({
        "pages": l.script_pages(),
        "scenes": s.script.scenes().len(),
        "paragraphs": s.script.paras.len(),
        "words": s.script.word_count(),
        "characters": s.script.characters().len(),
        "locations": s.script.locations().len(),
        "currentPage": l.caret(s.sel.focus).and_then(|c| l.pages.get(c.page)).and_then(|p| p.number),
    }))
}

fn layout(s: &mut Session, v: &Value) -> CmdResult {
    let l = s.layout();
    let pages: Vec<Value> = l
        .pages
        .iter()
        .enumerate()
        .filter(|(i, _)| p::usize(v, "page").is_none_or(|n| n == i + 1))
        .map(|(_, pg)| {
            json!({
                "number": pg.number, "titlePage": pg.title_page,
                "lines": pg.lines.iter().map(|ln| json!({
                    "kind": ln.kind, "element": ln.element.map(|e| e.id()), "x": ln.x, "y": ln.y,
                    "text": ln.display(), "para": ln.para,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({"pages": pages, "count": l.pages.len()}))
}

fn fountain(s: &mut Session, _: &Value) -> CmdResult {
    Ok(json!({"text": scriptcraft_fountain::write(&s.script)}))
}

fn fdx(s: &mut Session, _: &Value) -> CmdResult {
    Ok(json!({"xml": scriptcraft_fdx::write(&s.script)}))
}

fn set_fountain(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    s.script = scriptcraft_fountain::parse(text);
    s.sel = crate::Selection::default();
    Ok(json!({"paragraphs": s.script.paras.len()}))
}

fn set_fdx(s: &mut Session, v: &Value) -> CmdResult {
    let xml = p::req_str(v, "xml")?;
    s.script = scriptcraft_fdx::parse(xml).map_err(|e| CmdError::Failed(e.to_string()))?;
    s.sel = crate::Selection::default();
    Ok(json!({"paragraphs": s.script.paras.len()}))
}

fn characters(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    to_json(&reports::characters(&s.script, &l))
}

fn scenes(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    to_json(&reports::scenes(&s.script, &l))
}

fn locations(s: &mut Session, _: &Value) -> CmdResult {
    to_json(&reports::locations(&s.script))
}
