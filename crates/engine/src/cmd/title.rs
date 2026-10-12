//! Title page and script settings.

use scriptcraft_doc::TitlePage;
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
