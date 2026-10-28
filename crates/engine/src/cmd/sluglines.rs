//! Scene-heading words: the user's abbreviations for interior, exterior and both (`INT.`,
//! `ИНТ.`, `INNEN.` …) and times of day. They belong to the user, not the script: the app keeps
//! them in its settings and sets [`Session::sluglines`].

use scriptcraft_doc::Sluglines;
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session};

const L: &str = "Tools › SmartType";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("sluglines.get", "Scene Heading Abbreviations", L, get).pure(),
        CommandSpec::new("sluglines.set", "Set Scene Heading Abbreviations", L, set)
            .params(r#"{"preset": "en"|"ru"|"uk"|"de"|"fr"|"es"|"it"|"pt"|"pl"|"default" (optional, applied first), "interior": [string], "exterior": [string], "both": [string], "times": [string] (each optional)}"#)
            .pure(),
    ]
}

fn state(s: &Session) -> Value {
    let presets: Vec<Value> = Sluglines::presets().map(|(code, name)| json!({"code": code, "name": name})).collect();
    json!({
        "interior": s.sluglines.interior, "exterior": s.sluglines.exterior,
        "both": s.sluglines.both, "times": s.sluglines.times, "presets": presets,
    })
}

fn get(s: &mut Session, _: &Value) -> CmdResult {
    Ok(state(s))
}

fn words(v: &Value, key: &str) -> Result<Option<Vec<String>>, CmdError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => a
            .iter()
            .map(|w| w.as_str().map(str::to_string).ok_or_else(|| CmdError::Params(format!("`{key}` takes strings"))))
            .collect::<Result<_, _>>()
            .map(Some),
        // A comma-separated list, as typed in a text field.
        Some(Value::String(t)) => Ok(Some(t.split(',').map(str::to_string).collect())),
        Some(_) => Err(CmdError::Params(format!("`{key}` takes a list of words"))),
    }
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let mut next = match v.get("preset").and_then(Value::as_str) {
        Some("default") => Sluglines::default(),
        Some(code) => Sluglines::preset(code).ok_or_else(|| CmdError::Params(format!("unknown preset `{code}`")))?,
        None => s.sluglines.clone(),
    };
    for (key, list) in [("interior", &mut next.interior), ("exterior", &mut next.exterior), ("both", &mut next.both), ("times", &mut next.times)] {
        if let Some(w) = words(v, key)? {
            *list = w;
        }
    }
    s.sluglines = next.sanitized();
    Ok(state(s))
}
