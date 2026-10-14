//! Track changes: record who inserted and deleted what; accept or reject each change.
//! The recording itself happens in `Session::run` (see `scriptcraft_doc::track`).

use scriptcraft_doc::Pos;
use scriptcraft_doc::track::{self, ChangeKind, ChangeRef};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("track.toggle", "Track Changes", "Tools › Track Changes", toggle).key("Mod+Shift+E").params(r#"{"on": bool (default: toggle)}"#),
        CommandSpec::new("track.list", "Tracked Changes", "Tools › Track Changes", list)
            .params(r#"{"author": string (optional filter)} → {"on", "author", "authors", "changes": [{"para", "index", "kind": "insert"|"delete"|"break"|"element", "author", "text", "start", "end"}]}"#)
            .pure(),
        CommandSpec::new("track.accept", "Accept Change", "Tools › Track Changes", accept)
            .params(r#"{"para": n, "index": n} (default: the change at the caret, else the next one)"#)
            .when(any),
        CommandSpec::new("track.reject", "Reject Change", "Tools › Track Changes", reject)
            .params(r#"{"para": n, "index": n} (default: the change at the caret, else the next one)"#)
            .when(any),
        CommandSpec::new("track.acceptAll", "Accept All Changes", "Tools › Track Changes", accept_all).params(r#"{"author": string (only this author's)}"#).when(any),
        CommandSpec::new("track.rejectAll", "Reject All Changes", "Tools › Track Changes", reject_all).params(r#"{"author": string (only this author's)}"#).when(any),
        CommandSpec::new("track.next", "Next Change", "Tools › Track Changes", next).pure().when(any),
        CommandSpec::new("track.previous", "Previous Change", "Tools › Track Changes", previous).pure().when(any),
        CommandSpec::new("track.setAuthor", "Set Author", "Tools › Track Changes", set_author).params(r#"{"name": string}"#).pure(),
    ]
}

fn any(s: &Session) -> Option<&'static str> {
    if track::has_changes(&s.script) { None } else { Some("there are no tracked changes") }
}

fn change_json(c: &ChangeRef) -> Value {
    json!({"para": c.para, "index": c.index, "kind": c.kind.id(), "author": c.author, "text": c.text, "start": c.start, "end": c.end})
}

fn author_name(s: &Session) -> String {
    track::clean_author(&s.author)
}

fn list(s: &mut Session, v: &Value) -> CmdResult {
    let only = p::str(v, "author");
    let all = track::list(&s.script);
    let changes: Vec<Value> = all.iter().filter(|c| only.is_none_or(|a| c.author == a)).map(change_json).collect();
    Ok(
        json!({"on": s.script.settings.track.on, "author": author_name(s), "authors": track::authors(&s.script), "count": changes.len(), "changes": changes}),
    )
}

fn toggle(s: &mut Session, v: &Value) -> CmdResult {
    let on = p::bool(v, "on").unwrap_or(!s.script.settings.track.on);
    s.script.settings.track.on = on;
    s.status = if on { format!("Track changes on: changes are marked as {}", author_name(s)) } else { "Track changes off".into() };
    Ok(json!({"on": on, "author": author_name(s)}))
}

/// The change named by `para`/`index`, else the one at the caret, else the next one after it.
fn target(s: &Session, v: &Value) -> Result<ChangeRef, CmdError> {
    let all = track::list(&s.script);
    if let Some(para) = p::usize(v, "para") {
        let index = p::usize(v, "index").unwrap_or(0);
        return all
            .into_iter()
            .find(|c| c.para == para && c.index == index)
            .ok_or_else(|| CmdError::Params(format!("no change {index} in paragraph {para}")));
    }
    let at = s.sel.focus;
    let (a, b) = s.sel.ordered();
    let covers = |c: &ChangeRef| c.para >= a.para && c.para <= b.para && (c.para != at.para || (c.start <= at.off && at.off <= c.end.max(c.start)));
    if let Some(c) = all.iter().find(|c| covers(c)) {
        return Ok(c.clone());
    }
    all.iter()
        .find(|c| (c.para, c.start) >= (at.para, at.off))
        .or(all.first())
        .cloned()
        .ok_or_else(|| CmdError::Failed("there are no tracked changes".into()))
}

fn accept(s: &mut Session, v: &Value) -> CmdResult {
    let c = target(s, v)?;
    track::accept(&mut s.script, c.para, c.index);
    s.sel = Selection::caret(s.script.clamp(Pos::new(c.para, c.end)));
    Ok(json!({"accepted": change_json(&c), "left": track::list(&s.script).len()}))
}

fn reject(s: &mut Session, v: &Value) -> CmdResult {
    let c = target(s, v)?;
    let at = track::reject(&mut s.script, c.para, c.index).ok_or_else(|| CmdError::Failed("that change can't be rejected".into()))?;
    s.sel = Selection::caret(s.script.clamp(at));
    Ok(json!({"rejected": change_json(&c), "left": track::list(&s.script).len()}))
}

fn resolve(s: &mut Session, v: &Value, accept_them: bool) -> CmdResult {
    let author = p::str(v, "author").map(str::to_string);
    let n = track::resolve_all(&mut s.script, accept_them, author.as_deref());
    s.clamp_selection();
    s.status = format!("{} {n} change{}", if accept_them { "Accepted" } else { "Rejected" }, if n == 1 { "" } else { "s" });
    Ok(json!({"count": n, "left": track::list(&s.script).len()}))
}

fn accept_all(s: &mut Session, v: &Value) -> CmdResult {
    resolve(s, v, true)
}
fn reject_all(s: &mut Session, v: &Value) -> CmdResult {
    resolve(s, v, false)
}

fn go(s: &mut Session, c: &ChangeRef) -> CmdResult {
    s.sel = if c.kind == ChangeKind::Insert {
        Selection { anchor: Pos::new(c.para, c.start), focus: Pos::new(c.para, c.end) }
    } else {
        Selection::caret(Pos::new(c.para, c.start))
    };
    s.clamp_selection();
    Ok(change_json(c))
}

fn next(s: &mut Session, _: &Value) -> CmdResult {
    let all = track::list(&s.script);
    let (_, b) = s.sel.ordered();
    let c = all
        .iter()
        .find(|c| (c.para, c.start) > (b.para, b.off) || ((c.para, c.start) == (b.para, b.off) && s.sel.is_caret() && c.kind == ChangeKind::Insert))
        .or(all.first())
        .cloned();
    let c = c.ok_or_else(|| CmdError::Failed("there are no tracked changes".into()))?;
    go(s, &c)
}

fn previous(s: &mut Session, _: &Value) -> CmdResult {
    let all = track::list(&s.script);
    let (a, _) = s.sel.ordered();
    let c = all.iter().rev().find(|c| (c.para, c.start) < (a.para, a.off)).or(all.last()).cloned();
    let c = c.ok_or_else(|| CmdError::Failed("there are no tracked changes".into()))?;
    go(s, &c)
}

fn set_author(s: &mut Session, v: &Value) -> CmdResult {
    s.author = track::clean_author(p::req_str(v, "name")?);
    Ok(json!({"author": s.author}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn tracked_typing_accept_reject_undo() {
        let mut s = Session::sample();
        s.author = "Ana".into();
        assert!(s.run("track.accept", &json!({})).is_err(), "disabled without changes");
        s.run("track.toggle", &json!({"on": true})).unwrap();
        let orig = s.script.paras[3].text.clone();
        s.run("caret.set", &json!({"para": 3, "off": 0})).unwrap();
        for ch in ["Wind ", "howls. "] {
            s.run("text.insert", &json!({"text": ch})).unwrap();
        }
        // Delete the next word, then a whole paragraph.
        s.run("select.range", &json!({"anchor": {"para": 3, "off": 12}, "focus": {"para": 3, "off": 18}})).unwrap();
        s.run("text.backspace", &json!({})).unwrap();
        let l = s.run("track.list", &json!({})).unwrap();
        let kinds: Vec<&str> = l["changes"].as_array().unwrap().iter().map(|c| c["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["insert", "delete"], "{l}");
        assert_eq!(l["changes"][0]["text"], "Wind howls. ");
        assert_eq!(l["changes"][0]["author"], "Ana");
        // Bo joins in.
        s.run("track.setAuthor", &json!({"name": "Bo"})).unwrap();
        s.run("caret.set", &json!({"para": 1, "off": 0})).unwrap();
        s.run("text.insert", &json!({"text": "NIGHT. "})).unwrap();
        let l = s.run("track.list", &json!({})).unwrap();
        assert_eq!(l["authors"], json!(["Bo", "Ana"]));
        // Reject Bo's, accept Ana's insertion, reject Ana's deletion.
        s.run("track.rejectAll", &json!({"author": "Bo"})).unwrap();
        assert!(!s.script.paras[1].text.contains("NIGHT. NIGHT"));
        s.run("track.next", &json!({})).unwrap();
        s.run("track.accept", &json!({})).unwrap();
        s.run("track.reject", &json!({})).unwrap();
        assert_eq!(s.script.paras[3].text, format!("Wind howls. {orig}"));
        assert!(s.run("track.list", &json!({})).unwrap()["count"] == 0);
        // Undo brings the deletion back as a pending change.
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.run("track.list", &json!({})).unwrap()["count"], 1);
        // Tracking off: edits are not recorded, existing changes stay put.
        s.run("track.toggle", &json!({"on": false})).unwrap();
        s.run("caret.set", &json!({"para": 3, "off": 0})).unwrap();
        s.run("text.insert", &json!({"text": "Untracked. "})).unwrap();
        let l = s.run("track.list", &json!({})).unwrap();
        assert_eq!(l["count"], 1);
        assert_eq!(l["changes"][0]["start"].as_u64(), Some("Untracked. Wind howls. ".len() as u64));
        // Hostile targets.
        assert!(s.run("track.accept", &json!({"para": 9999, "index": 0})).is_err());
    }

    #[test]
    fn printed_text_omits_deletions() {
        let mut s = Session::sample();
        s.run("track.toggle", &json!({"on": true})).unwrap();
        s.run("select.text", &json!({"text": "Don't you dare."})).unwrap();
        s.run("text.backspace", &json!({})).unwrap();
        let l = s.layout();
        assert!(!l.pages.iter().flat_map(|p| &p.lines).any(|x| x.text.contains("Don't you dare")));
        let fountain = scriptcraft_fountain::write(&s.script);
        assert!(!fountain.contains("Don't you dare"));
    }
}
