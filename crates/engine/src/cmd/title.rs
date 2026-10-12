//! Title page and script settings.

use scriptcraft_doc::{Align, Element, ElementFormats, TitlePage};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("title.get", "Title Page", "Document › Title Page", get).pure(),
        CommandSpec::new("title.set", "Edit Title Page", "Document › Title Page", set)
            .params(r#"{"field": "title"|"credit"|"author"|"source"|"draftDate"|"contact"|"copyright"|"notes", "value": string} or {"fields": {field: value}}"#),
        CommandSpec::new("settings.get", "Script Settings", "Document", get_settings).pure(),
        CommandSpec::new("settings.set", "Change Script Settings", "Document", set_settings)
            .params(r#"{"sceneNumbers": bool, "moreContd": bool, "autoContd": bool, "titlePage": bool}"#),
        CommandSpec::new("view.toggleSceneNumbers", "Show Scene Numbers", "View", toggle_numbers),
        CommandSpec::new("settings.elements", "Element Settings", "Format › Elements", elements).pure(),
        CommandSpec::new("settings.element", "Change Element Settings", "Format › Elements", set_element).params(
            r#"{"element": string, "left": inches, "right": inches, "before": lines, "align": "left"|"center"|"right", "uppercase": bool, "enter": element, "tab": element, "tabEmpty": element}"#,
        ),
        CommandSpec::new("settings.resetElements", "Reset Element Settings", "Format › Elements", reset_elements),
    ]
}

fn get(s: &mut Session, _: &Value) -> CmdResult {
    serde_json::to_value(&s.script.title).map_err(|e| CmdError::Failed(e.to_string()))
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let set_one = |t: &mut TitlePage, k: &str, val: &Value| -> Result<(), CmdError> {
        let val = val.as_str().ok_or_else(|| CmdError::Params(format!("`{k}` must be a string")))?;
        if t.set(k, val) { Ok(()) } else { Err(CmdError::Params(format!("unknown title field `{k}`"))) }
    };
    let mut t = s.script.title.clone();
    if let Some(m) = v.get("fields").and_then(Value::as_object) {
        for (k, val) in m {
            set_one(&mut t, k, val)?;
        }
    } else {
        let k = p::req_str(v, "field")?;
        set_one(&mut t, k, v.get("value").unwrap_or(&Value::Null))?;
    }
    s.script.title = t;
    get(s, &Value::Null)
}

fn get_settings(s: &mut Session, _: &Value) -> CmdResult {
    let st = &s.script.settings;
    Ok(json!({"sceneNumbers": st.show_scene_numbers, "moreContd": st.more_contd, "autoContd": st.auto_contd, "titlePage": st.title_page}))
}

fn set_settings(s: &mut Session, v: &Value) -> CmdResult {
    let st = &mut s.script.settings;
    if let Some(b) = p::bool(v, "sceneNumbers") {
        st.show_scene_numbers = b;
    }
    if let Some(b) = p::bool(v, "moreContd") {
        st.more_contd = b;
    }
    if let Some(b) = p::bool(v, "autoContd") {
        st.auto_contd = b;
    }
    if let Some(b) = p::bool(v, "titlePage") {
        st.title_page = b;
    }
    get_settings(s, v)
}

fn toggle_numbers(s: &mut Session, _: &Value) -> CmdResult {
    let st = &mut s.script.settings;
    st.show_scene_numbers = !st.show_scene_numbers;
    if st.show_scene_numbers && s.script.paras.iter().all(|p| p.scene_number.is_none()) {
        s.script.number_scenes();
    }
    get_settings(s, &Value::Null)
}

fn elements(s: &mut Session, _: &Value) -> CmdResult {
    let f = &s.script.settings.elements;
    Ok(Value::Array(
        Element::ALL
            .iter()
            .map(|e| {
                let x = f.get(*e);
                json!({"element": e.id(), "name": e.name(), "left": x.left, "right": x.right, "before": x.before, "align": x.align,
                       "uppercase": x.uppercase, "enter": x.enter.id(), "tab": x.tab.id(), "tabEmpty": x.tab_empty.id()})
            })
            .collect(),
    ))
}

fn set_element(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "element")?;
    let e = Element::parse(name).ok_or_else(|| CmdError::Params(format!("unknown element `{name}`")))?;
    let mut f = s.script.settings.elements.get(e);
    let num = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite()).map(|x| x as f32);
    let el = |k: &str| -> Result<Option<Element>, CmdError> {
        match p::str(v, k) {
            None => Ok(None),
            Some(n) => Element::parse(n).map(Some).ok_or_else(|| CmdError::Params(format!("`{k}`: unknown element `{n}`"))),
        }
    };
    if let Some(x) = num("left") {
        f.left = x;
    }
    if let Some(x) = num("right") {
        f.right = x;
    }
    if let Some(x) = p::usize(v, "before") {
        f.before = x.min(u8::MAX as usize) as u8;
    }
    if let Some(a) = p::str(v, "align") {
        f.align = Align::parse(a).ok_or_else(|| CmdError::Params(format!("unknown alignment `{a}`")))?;
    }
    if let Some(b) = p::bool(v, "uppercase") {
        f.uppercase = b;
    }
    if let Some(x) = el("enter")? {
        f.enter = x;
    }
    if let Some(x) = el("tab")? {
        f.tab = x;
    }
    if let Some(x) = el("tabEmpty")? {
        f.tab_empty = x;
    }
    s.script.settings.elements.set(e, f);
    elements(s, v)
}

fn reset_elements(s: &mut Session, v: &Value) -> CmdResult {
    s.script.settings.elements = ElementFormats::default();
    elements(s, v)
}
