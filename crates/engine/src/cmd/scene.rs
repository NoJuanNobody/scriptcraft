//! Scenes: navigator list, go to, index-card summaries, reordering, scene numbers.

use scriptcraft_doc::{Element, Pos};
use serde_json::{Value, json};

use super::set_caret;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("scene.list", "Scenes", "View › Navigator", list).pure(),
        CommandSpec::new("scene.goto", "Go to Scene", "View › Navigator", goto).params(r#"{"index": n (0-based)}"#).pure(),
        CommandSpec::new("scene.setSummary", "Edit Index Card Summary", "View › Index Cards", set_summary)
            .params(r#"{"index": n, "summary": string}"#),
        CommandSpec::new("scene.setHeading", "Edit Scene Heading", "View › Index Cards", set_heading).params(r#"{"index": n, "heading": string}"#),
        CommandSpec::new("scene.move", "Move Scene", "View › Index Cards", move_scene).params(r#"{"from": n, "to": n}"#),
        CommandSpec::new("scene.setColor", "Card Colour", "View › Index Cards", set_color)
            .params(r#"{"index": n, "color": "red"|"orange"|"yellow"|"green"|"teal"|"blue"|"purple"|"pink"|null}"#),
        CommandSpec::new("scene.setTags", "Card Tags", "View › Index Cards", set_tags)
            .params(r#"{"index": n, "tags": [string] or "comma, separated"}"#),
        CommandSpec::new("scene.numberAll", "Number Scenes", "Production › Scene Numbers", number_all).when(unlocked),
        CommandSpec::new("scene.clearNumbers", "Remove Scene Numbers", "Production › Scene Numbers", clear_numbers).when(unlocked),
        CommandSpec::new("scene.setNumber", "Set Scene Number", "Production › Scene Numbers", set_number).params(r#"{"index": n, "number": string}"#),
    ]
}

/// Renumbering and removing numbers wait until scene numbers are unlocked.
fn unlocked(s: &Session) -> Option<&'static str> {
    s.script.settings.scene_numbers_locked.then_some("scene numbers are locked")
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let cur = s.script.scene_of(s.sel.focus.para);
    Ok(json!({
        "current": cur,
        "scenes": s.script.scenes().iter().map(|sc| json!({
            "index": sc.index, "para": sc.para, "heading": sc.heading, "number": sc.number,
            "summary": sc.summary, "page": l.page_number_of(sc.para), "pageLabel": l.page_label_of(sc.para),
            "color": sc.color.map(|c| c.id()), "tags": sc.tags,
        })).collect::<Vec<_>>(),
    }))
}

fn scene_para(s: &Session, v: &Value) -> Result<usize, CmdError> {
    let i = p::req_usize(v, "index")?;
    s.script.scenes().get(i).map(|sc| sc.para).ok_or_else(|| CmdError::Params(format!("no scene {i}")))
}

fn goto(s: &mut Session, v: &Value) -> CmdResult {
    let para = scene_para(s, v)?;
    set_caret(s, Pos::new(para, 0));
    super::sel_result(s)
}

fn set_summary(s: &mut Session, v: &Value) -> CmdResult {
    let para = scene_para(s, v)?;
    let text = p::req_str(v, "summary")?;
    s.script.para_mut(para)?.summary = scriptcraft_doc::sanitize(text);
    Ok(json!({"para": para}))
}

fn set_heading(s: &mut Session, v: &Value) -> CmdResult {
    let para = scene_para(s, v)?;
    let text = p::req_str(v, "heading")?;
    s.script.para_mut(para)?.text = scriptcraft_doc::sanitize(text).to_uppercase();
    Ok(json!({"para": para}))
}

fn move_scene(s: &mut Session, v: &Value) -> CmdResult {
    let from = p::req_usize(v, "from")?;
    let to = p::req_usize(v, "to")?;
    let para = s.script.move_scene(from, to)?;
    set_caret(s, Pos::new(para, 0));
    Ok(json!({"para": para}))
}

fn number_all(s: &mut Session, _: &Value) -> CmdResult {
    s.script.number_scenes();
    s.script.settings.show_scene_numbers = true;
    Ok(json!({"scenes": s.script.scenes().len()}))
}

fn clear_numbers(s: &mut Session, _: &Value) -> CmdResult {
    for p in &mut s.script.paras {
        p.scene_number = None;
    }
    Ok(json!({}))
}

fn set_number(s: &mut Session, v: &Value) -> CmdResult {
    let para = scene_para(s, v)?;
    let n = scriptcraft_doc::sanitize(p::req_str(v, "number")?).trim().chars().take(8).collect::<String>();
    let p = s.script.para_mut(para)?;
    if p.element == Element::SceneHeading {
        p.scene_number = (!n.is_empty()).then_some(n);
    }
    Ok(json!({"para": para}))
}

fn set_color(s: &mut Session, v: &Value) -> CmdResult {
    let para = scene_para(s, v)?;
    let color = match v.get("color") {
        None | Some(Value::Null) => None,
        Some(Value::String(c)) if c.is_empty() || c == "none" => None,
        Some(Value::String(c)) => Some(scriptcraft_doc::CardColor::parse(c).ok_or_else(|| CmdError::Params(format!("unknown colour `{c}`")))?),
        Some(_) => return Err(CmdError::Params("`color` must be a colour name or null".into())),
    };
    s.script.para_mut(para)?.color = color;
    Ok(json!({"para": para, "color": color.map(|c| c.id())}))
}

fn set_tags(s: &mut Session, v: &Value) -> CmdResult {
    let para = scene_para(s, v)?;
    let tags: Vec<String> = match v.get("tags") {
        Some(Value::String(t)) => scriptcraft_doc::clean_tags(t.split(',')),
        Some(Value::Array(a)) => scriptcraft_doc::clean_tags(a.iter().filter_map(Value::as_str)),
        None | Some(Value::Null) => Vec::new(),
        Some(_) => return Err(CmdError::Params("`tags` must be a list or a comma-separated string".into())),
    };
    s.script.para_mut(para)?.tags = tags.clone();
    Ok(json!({"para": para, "tags": tags}))
}
