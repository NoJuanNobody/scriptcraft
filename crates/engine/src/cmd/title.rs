//! Title page and script settings.

use scriptcraft_doc::{Align, Element, ElementFormats, MAX_TITLE_LINES, Style, TitleLine, TitlePage};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("title.get", "Title Page", "Document › Title Page", get).pure(),
        CommandSpec::new("title.set", "Edit Title Page", "Document › Title Page", set)
            .params(r#"{"field": "title"|"credit"|"author"|"source"|"draftDate"|"contact"|"copyright"|"notes", "value": string} or {"fields": {field: value}} (fields only; a free-form title page is edited with title.setLines)"#),
        CommandSpec::new("title.setLines", "Edit Free-Form Title Page", "Document › Title Page", set_lines).params(
            r#"{"lines": [{"align": "left"|"center"|"right", "text": string, "bold": bool, "italic": bool, "underline": bool}]} (empty: back to fields)"#,
        ),
        CommandSpec::new("title.makeFreeForm", "Free-Form Title Page", "Document › Title Page", make_free_form),
        CommandSpec::new("title.useFields", "Title Page from Fields", "Document › Title Page", use_fields),
        CommandSpec::new("settings.get", "Script Settings", "Document", get_settings).pure(),
        CommandSpec::new("settings.set", "Change Script Settings", "Document", set_settings)
            .params(r#"{"sceneNumbers": bool, "moreContd": bool, "autoContd": bool, "titlePage": bool}"#),
        CommandSpec::new("view.toggleSceneNumbers", "Show Scene Numbers", "View", toggle_numbers),
        CommandSpec::new("view.speed", "Speed View", "View", speed).params(r#"{"on": bool (default: toggle)}"#).pure(),
        CommandSpec::new("settings.elements", "Element Settings", "Format › Elements", elements).pure(),
        CommandSpec::new("settings.element", "Change Element Settings", "Format › Elements", set_element).params(
            r#"{"element": string, "left": inches, "right": inches, "before": lines, "align": "left"|"center"|"right", "uppercase": bool, "enter": element, "tab": element, "tabEmpty": element, "spacing": 1-3, "newPage": bool}"#,
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
            // `title.get`'s output can be passed back: its free-form lines are not a field.
            if k == "lines" {
                continue;
            }
            set_one(&mut t, k, val)?;
        }
    } else {
        let k = p::req_str(v, "field")?;
        set_one(&mut t, k, v.get("value").unwrap_or(&Value::Null))?;
    }
    if t != s.script.title && s.script.title.is_free_form() {
        return Err(CmdError::Failed(
            "the title page is free-form: edit its lines with title.setLines, or run title.useFields to edit fields".into(),
        ));
    }
    s.script.title = t;
    get(s, &Value::Null)
}

/// `{"align", "text", "bold", "italic", "underline"}` → a line.
fn line(v: &Value) -> Result<TitleLine, CmdError> {
    let align = match p::str(v, "align") {
        None => Align::Center,
        Some(a) => Align::parse(a).ok_or_else(|| CmdError::Params(format!("unknown alignment `{a}`")))?,
    };
    let text = match v.get("text") {
        None | Some(Value::Null) => "",
        Some(Value::String(t)) => t.as_str(),
        Some(_) => return Err(CmdError::Params("`text` must be a string".into())),
    };
    let flag = |k: &str| p::bool(v, k).unwrap_or(false);
    Ok(TitleLine { style: Style { bold: flag("bold"), italic: flag("italic"), underline: flag("underline") }, ..TitleLine::new(align, text) })
}

fn set_lines(s: &mut Session, v: &Value) -> CmdResult {
    let arr = v.get("lines").and_then(Value::as_array).ok_or_else(|| CmdError::Params("`lines` must be an array".into()))?;
    if arr.len() > MAX_TITLE_LINES {
        return Err(CmdError::Params(format!("at most {MAX_TITLE_LINES} lines")));
    }
    let lines = arr.iter().map(line).collect::<Result<Vec<_>, _>>()?;
    s.script.title.set_lines(lines);
    get(s, &Value::Null)
}

fn make_free_form(s: &mut Session, _: &Value) -> CmdResult {
    s.script.title.make_free_form();
    get(s, &Value::Null)
}

fn use_fields(s: &mut Session, _: &Value) -> CmdResult {
    s.script.title.use_fields();
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
                       "uppercase": x.uppercase, "enter": x.enter.id(), "tab": x.tab.id(), "tabEmpty": x.tab_empty.id(), "spacing": x.spacing, "newPage": x.new_page})
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
    if let Some(x) = p::usize(v, "spacing") {
        f.spacing = x.min(usize::from(u8::MAX)) as u8;
    }
    if let Some(b) = p::bool(v, "newPage") {
        f.new_page = b;
    }
    s.script.settings.elements.set(e, f);
    elements(s, v)
}

fn reset_elements(s: &mut Session, v: &Value) -> CmdResult {
    s.script.settings.elements = ElementFormats::default();
    elements(s, v)
}

fn speed(s: &mut Session, v: &Value) -> CmdResult {
    s.speed = p::bool(v, "on").unwrap_or(!s.speed);
    Ok(json!({"speed": s.speed}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn free_form_title_pages_through_commands() {
        let mut s = Session::new(scriptcraft_doc::Script::new());
        let ok = |s: &mut Session, id: &str, v: serde_json::Value| s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
        ok(&mut s, "title.set", json!({"fields": {"title": "NIGHT", "copyright": "(c) 2026", "notes": "Draft for Jo"}}));
        // title.get's output can go back into title.set.
        let got = ok(&mut s, "title.get", json!({}));
        ok(&mut s, "title.set", json!({"fields": got}));
        assert_eq!(s.script.title.copyright, "(c) 2026");
        // Free-form lines.
        let t = ok(
            &mut s,
            "title.setLines",
            json!({"lines": [{"align": "left", "text": "Agent: Bo"}, {"text": ""}, {"text": "NIGHT", "underline": true}, {"align": "right", "text": "May"}]}),
        );
        assert_eq!(t["lines"][2], json!({"align": "center", "text": "NIGHT", "underline": true}));
        assert_eq!(t["title"], "NIGHT");
        assert_eq!(t["draftDate"], "May");
        let err = s.run("title.set", &json!({"field": "title", "value": "DAY"})).unwrap_err();
        assert!(err.to_string().contains("free-form"), "{err}");
        // Passing title.get back unchanged is fine even when free-form.
        let got = ok(&mut s, "title.get", json!({}));
        ok(&mut s, "title.set", json!({"fields": got}));
        // Hostile lines are refused or cleaned, never stored raw.
        assert!(s.run("title.setLines", &json!({"lines": [{"align": "up"}]})).is_err());
        assert!(s.run("title.setLines", &json!({"lines": "x"})).is_err());
        assert!(s.run("title.setLines", &json!({"lines": vec![json!({"text": "x"}); 201]})).is_err());
        // Back to fields, then free-form from them.
        ok(&mut s, "title.useFields", json!({}));
        assert!(!s.script.title.is_free_form());
        ok(&mut s, "title.set", json!({"field": "title", "value": "DAY"}));
        let t = ok(&mut s, "title.makeFreeForm", json!({}));
        assert!(t["lines"].as_array().is_some_and(|l| l.iter().any(|l| l["text"] == "DAY")));
        // Undo restores the field title page.
        ok(&mut s, "edit.undo", json!({}));
        assert!(!s.script.title.is_free_form());
    }
}
