//! Read aloud and dictation commands.
//!
//! The engine plans what to read and with which voice (`speech.plan`, `speech.cast`,
//! `speech.setVoice`) and turns dictated phrases into script text (`dictation.insert`). Playing
//! audio needs a speech engine, which only the app has: `speech.play`, `pause`, `stop`, `status`
//! and `voices` are answered by the window (and over the control channel); headless, they say so.

use scriptcraft_doc::{Element, voices::character_key, voices::clean_voice_name};
use serde_json::{Value, json};

use super::sel_result;
use crate::dictation::{Step, interpret};
use crate::speech::{Options, cast_map, plan};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

const L: &str = "Tools › Read Aloud";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("speech.plan", "Read Aloud Plan", L, plan_cmd)
            .params(r#"{"from": para, "to": para, "scene": index (instead of from/to), "headings": bool (default true), "action": bool (default true), "parentheticals": bool, "transitions": bool, "characterNames": bool, "voices": [available voice names], "narrator": default voice} → {"utterances": [{"para", "element", "character", "text", "voice"}], "narrator", "cast"}"#)
            .pure(),
        CommandSpec::new("speech.cast", "Cast Voices", L, cast)
            .params(r#"{"voices": [available voice names]} → [{"character", "voice", "chosen": bool (set for this script)}]"#)
            .pure(),
        CommandSpec::new("speech.setVoice", "Set Voice", L, set_voice)
            .params(r#"{"character": name, "voice": name or null (automatic)} or {"narrator": true, "voice": name or null}"#),
        CommandSpec::new("speech.play", "Read Aloud", L, needs_app)
            .params(r#"{"from": para, "scene": index, plus speech.plan's options} (default: from the caret)"#)
            .pure(),
        CommandSpec::new("speech.pause", "Pause Reading", L, needs_app).pure(),
        CommandSpec::new("speech.stop", "Stop Reading", L, needs_app).pure(),
        CommandSpec::new("speech.status", "Reading Status", L, status).pure(),
        CommandSpec::new("speech.voices", "Voices", L, needs_app).pure(),
        CommandSpec::new("dictation.transcribe", "Transcribe Audio File", "Tools › Dictation", needs_app)
            .params(r#"{"path": audio file} → transcribed on this computer with whisper.cpp when installed, then inserted like dictation.insert"#)
            .pure(),
        CommandSpec::new("dictation.insert", "Dictate", "Tools › Dictation", dictate)
            .params(r#"{"text": spoken phrase ("character Nell. Don't you dare period", "new scene interior kitchen night", "undo that")} → one undo step"#),
    ]
}

fn needs_app(_: &mut Session, _: &Value) -> CmdResult {
    Err(CmdError::Disabled("reading aloud needs the ScriptCraft window and a speech engine; use speech.plan for the text and voices".into()))
}

fn status(_: &mut Session, _: &Value) -> CmdResult {
    Ok(json!({"available": false, "state": "idle", "reason": "no speech engine here (the ScriptCraft window has one)"}))
}

/// Voice names from `v.voices` (at most 2000, cleaned).
fn voice_list(v: &Value) -> Vec<String> {
    v.get("voices")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).filter_map(clean_voice_name).take(2000).collect())
        .unwrap_or_default()
}

/// Plan options from params; `from`/`to` default to the whole script.
pub fn options(s: &Session, v: &Value) -> Result<Options, CmdError> {
    let mut o = Options::default();
    if let Some(i) = p::usize(v, "scene") {
        let scenes = s.script.scenes();
        let sc = scenes.get(i).ok_or_else(|| CmdError::Params(format!("no scene {i} (the script has {})", scenes.len())))?;
        o.from = sc.para;
        o.to = scenes.get(i + 1).map_or(usize::MAX, |n| n.para.saturating_sub(1));
    }
    if let Some(f) = p::usize(v, "from") {
        o.from = f;
    }
    if let Some(t) = p::usize(v, "to") {
        o.to = t;
    }
    if o.from > o.to {
        return Err(CmdError::Params(format!("range {}–{} is backwards", o.from, o.to)));
    }
    let b = |k: &str, d: bool| p::bool(v, k).unwrap_or(d);
    o.headings = b("headings", o.headings);
    o.action = b("action", o.action);
    o.parentheticals = b("parentheticals", o.parentheticals);
    o.transitions = b("transitions", o.transitions);
    o.character_names = b("characterNames", o.character_names);
    Ok(o)
}

fn plan_cmd(s: &mut Session, v: &Value) -> CmdResult {
    let o = options(s, v)?;
    let pl = plan(&s.script, &o, &voice_list(v), p::str(v, "narrator"));
    Ok(json!({"utterances": pl.utterances, "narrator": pl.narrator, "cast": cast_map(&pl.cast)}))
}

fn cast(s: &mut Session, v: &Value) -> CmdResult {
    let (narrator, cast) = crate::speech::assign(&s.script, &voice_list(v), None);
    let stored = &s.script.settings.voices;
    let mut rows = vec![json!({"character": null, "narrator": true, "voice": narrator, "chosen": stored.narrator.is_some()})];
    rows.extend(cast.into_iter().map(|(c, voice)| {
        let chosen = stored.cast.contains_key(&c);
        json!({"character": c, "voice": voice, "chosen": chosen})
    }));
    Ok(Value::Array(rows))
}

fn set_voice(s: &mut Session, v: &Value) -> CmdResult {
    let voice = match v.get("voice") {
        None | Some(Value::Null) => None,
        Some(Value::String(x)) => clean_voice_name(x),
        Some(_) => return Err(CmdError::Params("`voice` must be a voice name or null".into())),
    };
    if p::bool(v, "narrator") == Some(true) {
        s.script.settings.voices.narrator = voice.clone();
        return Ok(json!({"narrator": true, "voice": voice}));
    }
    let name = character_key(p::req_str(v, "character")?);
    if name.is_empty() {
        return Err(CmdError::Params("`character` must name a character".into()));
    }
    let cast = &mut s.script.settings.voices.cast;
    match &voice {
        Some(x) => {
            if !cast.contains_key(&name) && cast.len() >= scriptcraft_doc::voices::MAX_CAST_VOICES {
                return Err(CmdError::Failed("too many cast voices".into()));
            }
            cast.insert(name.clone(), x.clone());
        }
        None => {
            cast.remove(&name);
        }
    }
    Ok(json!({"character": name, "voice": voice}))
}

/// Dictation: run the steps of a spoken phrase as one undo step.
fn dictate(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    let steps = interpret(text);
    let mut start = s.history_len();
    let r = apply(s, &steps, &mut start);
    match r {
        Ok(()) => {
            s.squash_history(start, "Dictation");
            let mut out = sel_result(s)?;
            if let Some(o) = out.as_object_mut() {
                o.insert("steps".into(), json!(steps.len()));
            }
            Ok(out)
        }
        Err(e) => {
            s.rollback_history(start);
            Err(e)
        }
    }
}

fn apply(s: &mut Session, steps: &[Step], start: &mut usize) -> Result<(), CmdError> {
    let mut pending: Option<Element> = None;
    let mut glue = false;
    // A phrase that starts with plain words after a finished heading, cue, parenthetical or
    // transition continues the element flow (heading → action, cue → dialogue), as if Enter was
    // pressed between phrases.
    if matches!(steps.first(), Some(Step::Text(_))) && at_end_of_short_element(s) {
        s.run("text.enter", &json!({}))?;
    }
    for step in steps {
        match step {
            Step::Start(e) => {
                pending = None;
                open(s, *e)?;
                glue = false;
            }
            Step::Then(e) => pending = Some(*e),
            Step::NewParagraph => {
                pending = None;
                s.run("text.enter", &json!({}))?;
            }
            Step::Undo => {
                pending = None;
                if s.can_undo() {
                    s.run("edit.undo", &json!({}))?;
                    *start = (*start).min(s.history_len());
                }
            }
            Step::Text(t) => {
                if let Some(e) = pending.take() {
                    open(s, e)?;
                }
                let text = words(s, t, glue);
                s.run("text.insert", &json!({"text": text}))?;
                glue = false;
            }
            Step::Punct { text, glue_before, glue_after } => {
                if let Some(e) = pending.take() {
                    open(s, e)?;
                }
                let lead = if !glue_before && !glue && needs_space(s) { " " } else { "" };
                s.run("text.insert", &json!({"text": format!("{lead}{text}")}))?;
                glue = *glue_after;
            }
        }
    }
    Ok(())
}

/// Start a paragraph of element `e`: the caret's paragraph if it is empty, else a new one.
fn open(s: &mut Session, e: Element) -> Result<(), CmdError> {
    let blank = s.script.para(s.sel.focus.para).is_none_or(|p| {
        let t = p.text.trim();
        t.is_empty() || (p.element == Element::Parenthetical && t == "()")
    });
    if !blank {
        s.run("text.enter", &json!({}))?;
    }
    if s.element() != e {
        s.run("element.set", &json!({"element": e.id()}))?;
    }
    Ok(())
}

/// The caret is at the end of a non-empty heading, cue, parenthetical or transition.
fn at_end_of_short_element(s: &Session) -> bool {
    if !s.sel.is_caret() {
        return false;
    }
    let at = s.sel.focus;
    let Some(p) = s.script.para(at.para) else { return false };
    let t = p.text.trim();
    match p.element {
        Element::SceneHeading | Element::Character | Element::Transition => !t.is_empty() && at.off >= p.text.trim_end().len(),
        Element::Parenthetical => t != "()" && !t.is_empty() && at.off + 1 >= p.text.trim_end().len(),
        _ => false,
    }
}

/// The character before the caret in its paragraph.
fn before_caret(s: &Session) -> Option<char> {
    let at = s.sel.ordered().0;
    s.script.para(at.para).and_then(|p| p.text.get(..at.off.min(p.text.len()))).and_then(|t| t.chars().next_back())
}

fn needs_space(s: &Session) -> bool {
    before_caret(s).is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | '"' | '“' | '\'' | '-' | '/'))
}

/// Words to type: a space before them when needed, and a capital at the start of a sentence.
fn words(s: &Session, t: &str, glue: bool) -> String {
    let lead = if !glue && needs_space(s) { " " } else { "" };
    let at = s.sel.ordered().0;
    let before: String = s.script.para(at.para).and_then(|p| p.text.get(..at.off.min(p.text.len()))).unwrap_or("").to_string();
    let last = before.trim_end().chars().next_back();
    let sentence_start = match s.element() {
        Element::Action | Element::Dialogue | Element::General | Element::Shot => last.is_none_or(|c| matches!(c, '.' | '?' | '!')),
        _ => false,
    };
    let mut out = String::from(lead);
    let mut chars = t.chars();
    if sentence_start && let Some(c) = chars.next() {
        out.extend(c.to_uppercase());
    }
    out.extend(chars);
    out
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Para, Pos, Script};
    use serde_json::json;

    use crate::Session;

    fn texts(s: &Session) -> Vec<(String, String)> {
        s.script.paras.iter().map(|p| (p.element.id().to_string(), p.text.clone())).collect()
    }

    fn empty() -> Session {
        Session::new(Script::from_paras(vec![Para::new(scriptcraft_doc::Element::Action, "")]).unwrap_or_default())
    }

    #[test]
    fn dictating_a_scene() {
        let mut s = empty();
        for phrase in [
            "new scene interior kitchen night",
            "rain on the window period the kettle ticks",
            "character Nell.",
            "parenthetical quietly.",
            "don't you dare period",
            "character Teo says is it over question mark",
            "cut to",
        ] {
            s.run("dictation.insert", &json!({"text": phrase})).unwrap();
        }
        let t = texts(&s);
        let want: Vec<(&str, &str)> = vec![
            ("sceneHeading", "INT. KITCHEN - NIGHT"),
            ("action", "Rain on the window. The kettle ticks"),
            ("character", "NELL"),
            ("parenthetical", "(quietly)"),
            ("dialogue", "Don't you dare."),
            ("character", "TEO"),
            ("dialogue", "Is it over?"),
            ("transition", "CUT TO:"),
        ];
        assert_eq!(t.iter().map(|(e, x)| (e.as_str(), x.as_str())).collect::<Vec<_>>(), want);
        // Each phrase is one undo step.
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.script.paras.len(), 7);
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.script.paras.len(), 5);
        assert_eq!(s.script.paras.last().map(|p| p.text.as_str()), Some("Don't you dare."));
    }

    #[test]
    fn undo_that() {
        let mut s = empty();
        s.run("dictation.insert", &json!({"text": "She runs period"})).unwrap();
        s.run("dictation.insert", &json!({"text": "he walks"})).unwrap();
        assert_eq!(s.script.paras[0].text, "She runs. He walks");
        // "scratch that" undoes the previous phrase, and is itself one step.
        s.run("dictation.insert", &json!({"text": "scratch that"})).unwrap();
        assert_eq!(s.script.paras[0].text, "She runs.");
        // Within a phrase it undoes the last part.
        s.run("dictation.insert", &json!({"text": "She stops. undo that she falls"})).unwrap();
        assert!(s.script.paras[0].text.starts_with("She runs."), "{}", s.script.paras[0].text);
        assert!(s.script.paras[0].text.ends_with("She falls"), "{}", s.script.paras[0].text);
        assert!(!s.script.paras[0].text.contains("stops"));
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.script.paras[0].text, "She runs.");
    }

    #[test]
    fn dictation_respects_the_caret() {
        let mut s = Session::sample();
        let n = s.script.paras.len();
        s.sel = crate::Selection::caret(Pos::new(1, 0));
        s.run("dictation.insert", &json!({"text": "fade in"})).unwrap();
        // "FADE IN:" goes in its own paragraph before the heading's text (split at the caret).
        assert_eq!(s.script.paras.len(), n + 1);
        assert!(s.script.paras.iter().any(|p| p.text == "FADE IN:"));
        assert!(s.run("dictation.insert", &json!({})).is_err());
        assert!(s.run("dictation.insert", &json!({"text": 5})).is_err());
        let r = s.run("dictation.insert", &json!({"text": ""})).unwrap();
        assert_eq!(r["steps"], 0);
    }

    #[test]
    fn voices_commands() {
        let mut s = Session::sample();
        let v = s.run("speech.setVoice", &json!({"character": "nell (v.o.)", "voice": "Daniel"})).unwrap();
        assert_eq!(v["character"], "NELL");
        s.run("speech.setVoice", &json!({"narrator": true, "voice": "Samantha"})).unwrap();
        let cast = s.run("speech.cast", &json!({"voices": ["Samantha", "Daniel", "Karen", "Moira", "Fred"]})).unwrap();
        assert_eq!(cast[0]["voice"], "Samantha");
        assert!(cast.as_array().unwrap().iter().any(|r| r["character"] == "NELL" && r["voice"] == "Daniel" && r["chosen"] == true));
        let skipper = cast.as_array().unwrap().iter().find(|r| r["character"] == "SKIPPER").cloned().unwrap();
        assert_eq!(skipper["chosen"], false);
        assert!(skipper["voice"].is_string());
        // Plan over one scene.
        let p = s.run("speech.plan", &json!({"scene": 2, "voices": ["Samantha", "Daniel", "Karen"]})).unwrap();
        let u = p["utterances"].as_array().unwrap();
        assert!(!u.is_empty());
        assert!(u.iter().all(|x| x["voice"].is_string()));
        assert!(u.first().unwrap()["text"].as_str().unwrap().starts_with("Interior."));
        // Undoable; clearing; bad params.
        s.run("edit.undo", &json!({})).unwrap();
        assert!(s.script.settings.voices.narrator.is_none());
        s.run("speech.setVoice", &json!({"character": "NELL", "voice": null})).unwrap();
        assert!(s.script.settings.voices.cast.is_empty());
        assert!(s.run("speech.setVoice", &json!({"character": "()", "voice": "x"})).is_err());
        assert!(s.run("speech.setVoice", &json!({"character": "A", "voice": 3})).is_err());
        assert!(s.run("speech.plan", &json!({"from": 9, "to": 2})).is_err());
        assert!(s.run("speech.plan", &json!({"scene": 99})).is_err());
        assert!(s.run("speech.play", &json!({})).is_err());
        assert_eq!(s.run("speech.status", &json!({})).unwrap()["available"], false);
    }

    #[test]
    fn a_dictated_phrase_is_one_tracked_insertion() {
        let mut s = empty();
        s.author = "Ana".into();
        s.run("track.toggle", &json!({"on": true})).unwrap();
        s.run("dictation.insert", &json!({"text": "rain on the window period the kettle ticks"})).unwrap();
        let l = s.run("track.list", &json!({})).unwrap();
        let ch = l["changes"].as_array().unwrap();
        assert_eq!(ch.len(), 1, "{l}");
        assert_eq!(ch[0]["kind"], "insert");
        assert_eq!(ch[0]["author"], "Ana");
        assert_eq!(ch[0]["text"], "Rain on the window. The kettle ticks");
        // "Undo that" takes the phrase and its change back.
        s.run("dictation.insert", &json!({"text": "undo that"})).unwrap();
        assert!(s.script.paras.iter().all(|p| p.changes.is_empty() && p.text.is_empty()));
    }
}
