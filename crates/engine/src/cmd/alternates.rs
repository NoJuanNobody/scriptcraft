//! Alternate lines: other versions of a dialogue paragraph. The paragraph's text is the active
//! version (the one laid out, printed and exported as script text); the others wait in its
//! `alternates`. All commands act on the caret's paragraph.

use scriptcraft_doc::{Alternate, Element, MAX_ALTERNATES};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn on_dialogue(s: &Session) -> Option<&'static str> {
    (s.element() != Element::Dialogue).then_some("put the caret in a line of dialogue")
}

fn has_alternates(s: &Session) -> Option<&'static str> {
    s.script.para(s.sel.focus.para).is_none_or(|p| p.alternates.is_empty()).then_some("this line has no alternates")
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("alt.add", "Add Alternate Line", "Tools › Alternates", add)
            .params(r#"{"text": string (default: a copy of the line), "use": bool (make it the active line)}"#)
            .when(on_dialogue),
        CommandSpec::new("alt.list", "Alternate Lines", "Tools › Alternates", list)
            .params(r#"{"all": bool (every line that has alternates)}"#)
            .pure(),
        CommandSpec::new("alt.choose", "Use Alternate Line", "Tools › Alternates", choose)
            .params(r#"{"index": n (into alternates)}"#)
            .when(has_alternates),
        CommandSpec::new("alt.next", "Next Alternate Line", "Tools › Alternates", next).when(has_alternates),
        CommandSpec::new("alt.previous", "Previous Alternate Line", "Tools › Alternates", previous).when(has_alternates),
        CommandSpec::new("alt.edit", "Edit Alternate Line", "Tools › Alternates", edit)
            .params(r#"{"index": n, "text": string}"#)
            .when(has_alternates),
        CommandSpec::new("alt.delete", "Delete Alternate Line", "Tools › Alternates", delete).params(r#"{"index": n}"#).when(has_alternates),
    ]
}

fn para_json(s: &Session, i: usize) -> Value {
    let Some(p) = s.script.para(i) else { return json!(null) };
    json!({
        "para": i, "element": p.element.id(), "active": p.text,
        "alternates": p.alternates.iter().map(|a| a.text.as_str()).collect::<Vec<_>>(),
        "count": p.alternates.len() + 1,
    })
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    let i = s.sel.focus.para;
    let make_active = p::bool(v, "use").unwrap_or(false);
    let para = s.script.para_mut(i)?;
    if para.element != Element::Dialogue {
        return Err(CmdError::Disabled("alternates are for lines of dialogue".into()));
    }
    if para.alternates.len() >= MAX_ALTERNATES {
        return Err(CmdError::Failed(format!("a line can have at most {MAX_ALTERNATES} alternates")));
    }
    let alt = match v.get("text") {
        None | Some(Value::Null) => para.as_alternate(),
        Some(Value::String(t)) => Alternate::plain(t),
        Some(_) => return Err(CmdError::Params("`text` must be a string".into())),
    };
    if alt.text.trim().is_empty() {
        return Err(CmdError::Params("the alternate is empty".into()));
    }
    para.alternates.push(alt);
    if make_active {
        let last = para.alternates.len() - 1;
        para.use_alternate(last);
    }
    Ok(para_json(s, i))
}

fn list(s: &mut Session, v: &Value) -> CmdResult {
    if p::bool(v, "all").unwrap_or(false) {
        let l = s.layout();
        let lines: Vec<Value> = s
            .script
            .paras
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.alternates.is_empty())
            .map(|(i, _)| {
                let mut o = para_json(s, i);
                if let Some(m) = o.as_object_mut() {
                    m.insert("page".into(), json!(l.page_number_of(i)));
                }
                o
            })
            .collect();
        return Ok(json!({"lines": lines}));
    }
    Ok(para_json(s, s.sel.focus.para))
}

fn index(s: &Session, v: &Value) -> Result<usize, CmdError> {
    let k = p::req_usize(v, "index")?;
    let n = s.script.para(s.sel.focus.para).map_or(0, |p| p.alternates.len());
    if k >= n {
        return Err(CmdError::Params(format!("no alternate {k} (this line has {n})")));
    }
    Ok(k)
}

fn choose(s: &mut Session, v: &Value) -> CmdResult {
    let k = index(s, v)?;
    let i = s.sel.focus.para;
    s.script.para_mut(i)?.use_alternate(k);
    Ok(para_json(s, i))
}

fn cycle(s: &mut Session, forward: bool) -> CmdResult {
    let i = s.sel.focus.para;
    s.script.para_mut(i)?.cycle_alternates(forward);
    Ok(para_json(s, i))
}

fn next(s: &mut Session, _: &Value) -> CmdResult {
    cycle(s, true)
}

fn previous(s: &mut Session, _: &Value) -> CmdResult {
    cycle(s, false)
}

fn edit(s: &mut Session, v: &Value) -> CmdResult {
    let k = index(s, v)?;
    let text = p::req_str(v, "text")?;
    let alt = Alternate::plain(text);
    if alt.text.trim().is_empty() {
        return Err(CmdError::Params("the alternate is empty (use alt.delete)".into()));
    }
    let i = s.sel.focus.para;
    if let Some(a) = s.script.para_mut(i)?.alternates.get_mut(k) {
        *a = alt;
    }
    Ok(para_json(s, i))
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let k = index(s, v)?;
    let i = s.sel.focus.para;
    let p = s.script.para_mut(i)?;
    if k < p.alternates.len() {
        p.alternates.remove(k);
    }
    Ok(para_json(s, i))
}
