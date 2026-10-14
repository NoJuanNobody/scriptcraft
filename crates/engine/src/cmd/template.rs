//! Templates: new scripts for features, TV (single and multi-camera) and the stage, or a
//! template's formatting applied to the open script.

use scriptcraft_doc::{Pos, Template};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("template.list", "Templates", "File › New from Template", list).pure(),
        CommandSpec::new("template.new", "New from Template", "File › New from Template", new)
            .params(r#"{"template": "feature"|"tv-single-camera"|"tv-multi-camera"|"stage-play", "acts": n (0-7), "teaser": bool (TV: teaser / cold open)}"#)
            .pure(),
        CommandSpec::new("template.apply", "Apply Template Formatting", "Format › Template", apply)
            .params(r#"{"template": id} (element settings, page setup and scene lettering; the text is kept)"#),
        CommandSpec::new("template.current", "Current Template", "Format › Template", current).pure(),
    ]
}

fn json_of(t: Template) -> Value {
    let (acts, teaser) = t.default_acts();
    json!({"id": t.id(), "name": t.name(), "description": t.description(), "acts": acts, "teaser": teaser})
}

fn list(_: &mut Session, _: &Value) -> CmdResult {
    Ok(Value::Array(Template::ALL.into_iter().map(json_of).collect()))
}

fn which(v: &Value) -> Result<Template, CmdError> {
    let name = p::req_str(v, "template")?;
    Template::parse(name).ok_or_else(|| {
        let ids: Vec<&str> = Template::ALL.iter().map(|t| t.id()).collect();
        CmdError::Params(format!("unknown template `{name}` ({})", ids.join(", ")))
    })
}

fn new(s: &mut Session, v: &Value) -> CmdResult {
    let t = which(v)?;
    let acts = match v.get("acts") {
        None | Some(Value::Null) => None,
        Some(_) => Some(p::usize(v, "acts").ok_or_else(|| CmdError::Params("`acts` must be a number from 0 to 7".into()))?),
    };
    let (script, caret) = t.script(acts, p::bool(v, "teaser"));
    s.replace_script(script);
    s.path = None;
    s.sel = Selection::caret(s.script.clamp(caret));
    s.status = format!("New {} script", t.name());
    Ok(json!({"template": t.id(), "paragraphs": s.script.paras.len(), "caret": {"para": caret.para, "off": caret.off}}))
}

fn apply(s: &mut Session, v: &Value) -> CmdResult {
    let t = which(v)?;
    t.apply(&mut s.script.settings);
    let numbered = s.script.paras.iter().any(|p| p.scene_number.is_some());
    if numbered && !s.script.settings.scene_numbers_locked {
        s.script.number_scenes();
    }
    let _ = s.script.normalize();
    s.sel = Selection::caret(s.script.clamp(Pos::new(s.sel.focus.para, s.sel.focus.off)));
    s.invalidate();
    Ok(json!({"template": t.id(), "pages": s.layout().script_pages()}))
}

fn current(s: &mut Session, _: &Value) -> CmdResult {
    Ok(match Template::detect(&s.script.settings) {
        Some(t) => json_of(t),
        None => json!({"id": null, "name": "Custom"}),
    })
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::Element;
    use scriptcraft_layout::LineKind;
    use serde_json::json;

    use crate::Session;

    fn body_rows_used(s: &mut Session, para: usize) -> Vec<f32> {
        let l = s.layout();
        l.pages.iter().flat_map(|p| p.lines.iter()).filter(|x| x.kind == LineKind::Body && x.para == Some(para)).map(|x| x.y).collect()
    }

    #[test]
    fn new_and_apply() {
        let mut s = Session::sample();
        let list = s.run("template.list", &json!({})).unwrap();
        assert_eq!(list.as_array().map(Vec::len), Some(4));
        let r = s.run("template.new", &json!({"template": "sitcom", "acts": 2})).unwrap();
        assert_eq!(r["template"], "tv-multi-camera");
        assert!(!s.can_undo());
        assert_eq!(s.run("template.current", &json!({})).unwrap()["id"], "tv-multi-camera");
        // Typing at the caret goes into the first scene heading (after COLD OPEN).
        assert_eq!(s.element(), Element::SceneHeading);
        assert!(s.run("template.new", &json!({"template": "opera"})).is_err());
        assert!(s.run("template.new", &json!({"template": "feature", "acts": "x"})).is_err());
        assert!(s.run("template.new", &json!({})).is_err());
    }

    /// Multi-camera: dialogue double-spaced, action in capitals, each scene on its own page,
    /// scenes lettered.
    #[test]
    fn multi_camera_layout() {
        let mut s = Session::sample();
        let one = body_rows_used(&mut s, 0).len();
        assert!(one >= 1);
        s.run("template.apply", &json!({"template": "tv-multi-camera"})).unwrap();
        // The long dialogue paragraph: every other row.
        let long = s.script.paras.iter().position(|p| p.element == Element::Dialogue && p.text.len() > 300).unwrap();
        let ys = body_rows_used(&mut s, long);
        assert!(ys.len() > 3);
        for w in ys.windows(2) {
            let d = w[1] - w[0];
            assert!(d > 20.0 || d < 0.0, "double-spaced rows {ys:?}");
        }
        // Action in capitals.
        assert!(s.script.paras.iter().filter(|p| p.element == Element::Action).all(|p| !p.text.chars().any(char::is_lowercase)));
        // Every scene heading starts a page.
        let l = s.layout();
        let heads: Vec<usize> = s.script.paras.iter().enumerate().filter(|(_, p)| p.element == Element::SceneHeading).map(|(i, _)| i).collect();
        for h in heads.iter().skip(1) {
            let page = l.pages.iter().find(|pg| pg.lines.iter().any(|x| x.para == Some(*h))).unwrap();
            let first = page.lines.iter().find(|x| x.kind == LineKind::Body).unwrap();
            assert_eq!(first.para, Some(*h), "scene heading {h} starts its page");
        }
        s.run("scene.numberAll", &json!({})).unwrap();
        let nums: Vec<String> = s.script.scenes().iter().filter_map(|x| x.number.clone()).collect();
        assert_eq!(nums, ["A", "B", "C", "D"]);
        // Undo restores the feature format.
        s.run("edit.undo", &json!({})).unwrap();
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.run("template.current", &json!({})).unwrap()["id"], "feature");
    }

    #[test]
    fn line_counts_per_page() {
        // Multi-camera dialogue takes about twice the rows: the same script needs more pages.
        let mut s = Session::sample();
        let feature = s.layout().script_pages();
        s.run("template.apply", &json!({"template": "tv-multi-camera"})).unwrap();
        let multi = s.layout().script_pages();
        assert!(multi > feature, "{multi} > {feature}");
        // Stage play: character names centred.
        s.run("template.apply", &json!({"template": "stage-play"})).unwrap();
        let l = s.layout();
        let cue = l.pages.iter().flat_map(|p| &p.lines).find(|x| x.element == Some(Element::Character)).unwrap();
        let w = (cue.text.chars().count() + cue.suffix.chars().count()) as f32 * scriptcraft_geom::CHAR_W;
        let mid = cue.x + w / 2.0;
        assert!((mid - 4.5 * 72.0).abs() < 8.0, "centred cue at {mid}");
    }
}
