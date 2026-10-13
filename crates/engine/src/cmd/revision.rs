//! Revision mode: revision sets, the active set, and the marks on changed text.

use scriptcraft_doc::revision::{MAX_SETS, RevisionSet, clear_marks, parse_color};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("revision.get", "Revisions", "Production › Revisions", get).pure(),
        CommandSpec::new("revision.mode", "Revision Mode", "Production › Revisions", mode)
            .params(r#"{"on": bool (default: toggle), "set": index|name (optional: make it active)}"#),
        CommandSpec::new("revision.set", "Change Revision Settings", "Production › Revisions", set)
            .params(r#"{"active": index|name, "tracking": bool, "showMarks": bool, "colorPages": bool}"#),
        CommandSpec::new("revision.editSet", "Edit Revision Set", "Production › Revisions", edit_set)
            .params(r##"{"set": index|name (default: active), "name": string, "color": "#rrggbb", "date": string, "mark": string}"##),
        CommandSpec::new("revision.addSet", "Add Revision Set", "Production › Revisions", add_set)
            .params(r##"{"name": string, "color": "#rrggbb", "date": string}"##),
        CommandSpec::new("revision.clearMarks", "Clear Revision Marks", "Production › Revisions", clear)
            .params(r#"{"set": index|name (default: every set), "selection": bool (default: whole script)}"#),
        CommandSpec::new("revision.pages", "Revised Pages", "Production › Revisions", pages).params(r#"{} → [{"page", "set"}]"#).pure(),
    ]
}

/// A set named or numbered by `v[k]`.
fn which(s: &Session, v: &Value, k: &str) -> Result<Option<usize>, CmdError> {
    let r = &s.script.settings.revision;
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            n.as_u64().map(|i| i as usize).filter(|i| *i < r.sets.len()).map(Some).ok_or_else(|| CmdError::Params(format!("no revision set {n}")))
        }
        Some(Value::String(name)) => r.find(name).map(Some).ok_or_else(|| CmdError::Params(format!("no revision set `{name}`"))),
        Some(_) => Err(CmdError::Params(format!("`{k}` must be a set index or name"))),
    }
}

fn set_json(i: usize, st: &RevisionSet) -> Value {
    json!({"index": i, "name": st.name, "color": st.hex(), "date": st.date, "mark": st.mark, "label": st.label()})
}

fn revised_pages(s: &mut Session) -> Vec<Value> {
    let l = s.layout();
    let r = &s.script.settings.revision;
    l.pages
        .iter()
        .zip(&l.page_rev)
        .filter_map(|(pg, lvl)| {
            let lvl = (*lvl)?;
            let label = if pg.title_page { "title".to_string() } else { pg.number.unwrap_or(0).to_string() };
            Some(json!({"page": label, "set": r.set(lvl).map(|x| x.name.clone()), "level": lvl}))
        })
        .collect()
}

fn get(s: &mut Session, _: &Value) -> CmdResult {
    let pages = revised_pages(s);
    let r = &s.script.settings.revision;
    let marks: usize = s.script.paras.iter().map(|p| p.revs.len()).sum();
    let paras = s.script.paras.iter().filter(|p| !p.revs.is_empty()).count();
    Ok(json!({
        "tracking": r.tracking,
        "active": r.active,
        "activeName": r.sets.get(r.active).map(|x| x.name.clone()),
        "showMarks": r.show_marks,
        "colorPages": r.color_pages,
        "sets": r.sets.iter().enumerate().map(|(i, st)| set_json(i, st)).collect::<Vec<_>>(),
        "marks": marks,
        "markedParagraphs": paras,
        "revisedPages": pages,
    }))
}

fn mode(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(i) = which(s, v, "set")? {
        s.script.settings.revision.active = i;
    }
    let r = &mut s.script.settings.revision;
    r.tracking = p::bool(v, "on").unwrap_or(!r.tracking);
    s.status = if r.tracking {
        format!("Revision mode on: changes are marked {}", r.sets.get(r.active).map_or("", |x| x.name.as_str()))
    } else {
        "Revision mode off".into()
    };
    get(s, v)
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let active = which(s, v, "active")?;
    let r = &mut s.script.settings.revision;
    if let Some(i) = active {
        r.active = i;
    }
    if let Some(b) = p::bool(v, "tracking") {
        r.tracking = b;
    }
    if let Some(b) = p::bool(v, "showMarks") {
        r.show_marks = b;
    }
    if let Some(b) = p::bool(v, "colorPages") {
        r.color_pages = b;
    }
    s.invalidate();
    get(s, v)
}

fn apply(st: &mut RevisionSet, v: &Value) -> Result<(), CmdError> {
    if let Some(n) = p::str(v, "name") {
        st.name = n.to_string();
    }
    if let Some(c) = p::str(v, "color") {
        st.color = parse_color(c).ok_or_else(|| CmdError::Params(format!("bad colour `{c}` (use #rrggbb)")))?;
    }
    if let Some(d) = p::str(v, "date") {
        st.date = d.to_string();
    }
    if let Some(m) = p::str(v, "mark") {
        st.mark = m.to_string();
    }
    Ok(())
}

fn edit_set(s: &mut Session, v: &Value) -> CmdResult {
    let i = which(s, v, "set")?.unwrap_or(s.script.settings.revision.active);
    let r = &mut s.script.settings.revision;
    let st = r.sets.get_mut(i).ok_or_else(|| CmdError::Params(format!("no revision set {i}")))?;
    apply(st, v)?;
    r.normalize();
    s.invalidate();
    get(s, v)
}

fn add_set(s: &mut Session, v: &Value) -> CmdResult {
    let r = &mut s.script.settings.revision;
    if r.sets.len() >= MAX_SETS {
        return Err(CmdError::Failed(format!("a script keeps at most {MAX_SETS} revision sets")));
    }
    let mut st = RevisionSet::new(p::req_str(v, "name")?, [0xff, 0xff, 0xff]);
    apply(&mut st, v)?;
    r.sets.push(st);
    r.normalize();
    get(s, v)
}

fn clear(s: &mut Session, v: &Value) -> CmdResult {
    let level = which(s, v, "set")?.map(|i| i.min(usize::from(u8::MAX)) as u8);
    let range = p::bool(v, "selection").unwrap_or(false).then(|| {
        let (a, b) = s.sel.ordered();
        a.para..=b.para
    });
    let n = clear_marks(&mut s.script, level, range);
    s.status = format!("Cleared {n} revision mark{}", if n == 1 { "" } else { "s" });
    let mut out = get(s, v)?;
    if let Some(o) = out.as_object_mut() {
        o.insert("cleared".into(), json!(n));
    }
    Ok(out)
}

fn pages(s: &mut Session, _: &Value) -> CmdResult {
    Ok(Value::Array(revised_pages(s)))
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::Pos;
    use serde_json::json;

    use crate::Session;

    #[test]
    fn revision_mode_marks_edits() {
        let mut s = Session::sample();
        s.run("revision.mode", &json!({"on": true, "set": "Pink"})).unwrap();
        s.run("revision.editSet", &json!({"date": "10/12/26"})).unwrap();
        // Typing in the first action paragraph is marked Pink.
        let para = s.script.paras.iter().position(|p| p.element == scriptcraft_doc::Element::Action).unwrap();
        s.sel = crate::Selection::caret(Pos::new(para, 0));
        s.run("text.insert", &json!({"text": "Suddenly, "})).unwrap();
        let p = &s.script.paras[para];
        assert_eq!(p.revs.len(), 1);
        assert_eq!(&p.text[p.revs[0].start..p.revs[0].end], "Suddenly, ");
        assert_eq!(p.revs[0].level, 2);
        let v = s.run("revision.get", &json!({})).unwrap();
        assert_eq!(v["activeName"], "Pink");
        assert_eq!(v["marks"], 1);
        let pages = v["revisedPages"].as_array().unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0]["set"], "Pink");
        // The page shows the mark and the header names the revision.
        let l = s.layout();
        let pi = l.para_page[para];
        assert!(l.pages[pi].lines.iter().any(|x| x.kind == scriptcraft_layout::LineKind::RevisionMark && x.text == "*"));
        if l.pages[pi].number.unwrap_or(1) > 1 {
            assert!(l.pages[pi].lines.iter().any(|x| x.kind == scriptcraft_layout::LineKind::Header && x.text.starts_with("Pink Rev. 10/12/26")));
        }
        // Undo removes the text and its mark together.
        s.run("edit.undo", &json!({})).unwrap();
        assert!(s.script.paras[para].revs.is_empty());
        s.run("edit.redo", &json!({})).unwrap();
        // Out of revision mode, edits are not marked but marks follow the text.
        s.run("revision.mode", &json!({"on": false})).unwrap();
        s.sel = crate::Selection::caret(Pos::new(para, 0));
        s.run("text.insert", &json!({"text": "And "})).unwrap();
        let p = &s.script.paras[para];
        assert_eq!(&p.text[p.revs[0].start..p.revs[0].end], "Suddenly, ");
        // Printing only revised pages.
        let v = s.run("print.pages", &json!({"revisedOnly": true, "titlePage": false})).unwrap();
        assert_eq!(v["count"], 1);
        let v = s.run("revision.clearMarks", &json!({})).unwrap();
        assert_eq!(v["cleared"], 1);
        assert!(s.run("print.pages", &json!({"revisedOnly": true, "titlePage": false})).is_err());
    }

    #[test]
    fn revision_sets() {
        let mut s = Session::new(scriptcraft_doc::Script::new());
        assert!(s.run("revision.set", &json!({"active": "Mauve"})).is_err());
        assert!(s.run("revision.set", &json!({"active": 99})).is_err());
        s.run("revision.set", &json!({"active": 3, "colorPages": true})).unwrap();
        assert_eq!(s.script.settings.revision.active, 3);
        assert!(s.run("revision.editSet", &json!({"color": "blue"})).is_err());
        let v = s.run("revision.editSet", &json!({"set": "yellow", "name": "Yellow 2", "color": "#ffee00", "mark": "**x"})).unwrap();
        assert_eq!(v["sets"][3]["name"], "Yellow 2");
        assert_eq!(v["sets"][3]["color"], "#ffee00");
        assert_eq!(v["sets"][3]["mark"], "**");
        let v = s.run("revision.addSet", &json!({"name": "2nd Blue", "color": "#aabbff"})).unwrap();
        assert_eq!(v["sets"].as_array().unwrap().len(), 10);
        for i in 0..20 {
            let _ = s.run("revision.addSet", &json!({"name": format!("x{i}")}));
        }
        assert_eq!(s.script.settings.revision.sets.len(), 20);
    }
}
