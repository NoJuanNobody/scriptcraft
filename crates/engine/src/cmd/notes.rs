//! Script notes: comments anchored to ranges of the script's text, with a colour and an author.
//! Notes are numbered in script order (`index`).

use scriptcraft_doc::{CardColor, NoteRef, Pos};
use serde_json::{Value, json};

use super::parse_pos;
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

fn has_notes(s: &Session) -> Option<&'static str> {
    (!s.script.paras.iter().any(|p| !p.notes.is_empty())).then_some("the script has no notes")
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("note.add", "Add Script Note", "Tools › Script Notes", add).params(
            r#"{"text": string, "color": "yellow"|"red"|"orange"|"green"|"teal"|"blue"|"purple"|"pink", "author": string, "anchor": {"para", "off"}, "focus": {"para", "off"} (default: the selection)}"#,
        ),
        CommandSpec::new("note.list", "Script Notes", "Tools › Script Notes", list).pure(),
        CommandSpec::new("note.edit", "Edit Script Note", "Tools › Script Notes", edit)
            .params(r#"{"index": n, "text": string, "color": colour name, "author": string} (each optional but index)"#),
        CommandSpec::new("note.delete", "Delete Script Note", "Tools › Script Notes", delete).params(r#"{"index": n}"#),
        CommandSpec::new("note.deleteAll", "Delete All Script Notes", "Tools › Script Notes", delete_all).when(has_notes),
        CommandSpec::new("note.goto", "Go to Script Note", "Tools › Script Notes", goto).params(r#"{"index": n}"#).pure(),
        CommandSpec::new("note.next", "Next Script Note", "Tools › Script Notes", next).pure().when(has_notes),
        CommandSpec::new("note.previous", "Previous Script Note", "Tools › Script Notes", previous).pure().when(has_notes),
    ]
}

/// A range from `anchor`/`focus` params, else the selection.
pub(crate) fn range_param(s: &Session, v: &Value) -> (Pos, Pos) {
    match (parse_pos(v.get("anchor")), parse_pos(v.get("focus"))) {
        (Some(a), Some(b)) => (s.script.clamp(a), s.script.clamp(b)),
        (Some(a), None) => (s.script.clamp(a), s.script.clamp(a)),
        _ => s.sel.ordered(),
    }
}

fn color_param(v: &Value) -> Result<Option<CardColor>, CmdError> {
    match v.get("color") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(c)) => CardColor::parse(c).map(Some).ok_or_else(|| CmdError::Params(format!("unknown colour `{c}`"))),
        Some(_) => Err(CmdError::Params("`color` must be a colour name".into())),
    }
}

fn note_ref(s: &Session, v: &Value) -> Result<NoteRef, CmdError> {
    let i = p::req_usize(v, "index")?;
    s.script.note_refs().get(i).copied().ok_or_else(|| CmdError::Params(format!("no note {i}")))
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    if scriptcraft_doc::clean_note(text).is_empty() {
        return Err(CmdError::Params("the note is empty".into()));
    }
    let color = color_param(v)?;
    let author = p::str(v, "author").unwrap_or("");
    let (a, b) = range_param(s, v);
    let index =
        s.script.add_note(a, b, text, color, author).ok_or_else(|| CmdError::Failed("this paragraph has as many notes as it can hold".into()))?;
    Ok(json!({"index": index}))
}

/// Every note with where it is.
pub fn notes_json(s: &mut Session) -> Vec<Value> {
    let l = s.layout();
    s.script
        .note_refs()
        .iter()
        .enumerate()
        .filter_map(|(index, r)| {
            let para = s.script.para(r.para)?;
            let n = para.notes.get(r.k)?;
            Some(json!({
                "index": index, "para": r.para, "start": n.start, "end": n.end, "text": n.text,
                "color": n.color.id(), "author": n.author, "quote": para.slice(n.start, n.end),
                "element": para.element.id(), "page": l.page_number_of(r.para),
                "scene": s.script.scene_of(r.para).map(|x| x + 1),
            }))
        })
        .collect()
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    let notes = notes_json(s);
    Ok(json!({"notes": notes, "count": notes.len()}))
}

fn edit(s: &mut Session, v: &Value) -> CmdResult {
    let r = note_ref(s, v)?;
    let color = color_param(v)?;
    let text = p::str(v, "text").map(scriptcraft_doc::clean_note);
    if text.as_deref().is_some_and(str::is_empty) {
        return Err(CmdError::Params("the note is empty (use note.delete)".into()));
    }
    let author = p::str(v, "author").map(scriptcraft_doc::clean_author);
    let n = s.script.note_mut(r).ok_or_else(|| CmdError::Failed("the note is gone".into()))?;
    if let Some(t) = text {
        n.text = t;
    }
    if let Some(c) = color {
        n.color = c;
    }
    if let Some(a) = author {
        n.author = a;
    }
    Ok(json!({"index": p::req_usize(v, "index")?}))
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let r = note_ref(s, v)?;
    let para = s.script.para_mut(r.para)?;
    if r.k < para.notes.len() {
        para.notes.remove(r.k);
    }
    Ok(json!({"remaining": s.script.note_refs().len()}))
}

fn delete_all(s: &mut Session, _: &Value) -> CmdResult {
    let n = s.script.note_refs().len();
    for p in &mut s.script.paras {
        p.notes.clear();
    }
    Ok(json!({"deleted": n}))
}

/// Select a note's range.
fn select(s: &mut Session, r: NoteRef) -> CmdResult {
    let n = s.script.note(r).cloned().ok_or_else(|| CmdError::Failed("the note is gone".into()))?;
    s.sel = Selection { anchor: s.script.clamp(Pos::new(r.para, n.start)), focus: s.script.clamp(Pos::new(r.para, n.end)) };
    let index = s.script.note_refs().iter().position(|x| *x == r);
    Ok(json!({"index": index, "para": r.para, "text": n.text, "author": n.author, "color": n.color.id()}))
}

fn goto(s: &mut Session, v: &Value) -> CmdResult {
    let r = note_ref(s, v)?;
    select(s, r)
}

fn next(s: &mut Session, _: &Value) -> CmdResult {
    let (a, _) = s.sel.ordered();
    let refs = s.script.note_refs();
    let key = |r: &NoteRef| (r.para, s.script.note(*r).map_or(0, |n| n.start));
    let r = refs
        .iter()
        .find(|r| key(r) > (a.para, a.off))
        .or(refs.first())
        .copied()
        .ok_or_else(|| CmdError::Disabled("the script has no notes".into()))?;
    select(s, r)
}

fn previous(s: &mut Session, _: &Value) -> CmdResult {
    let (a, _) = s.sel.ordered();
    let refs = s.script.note_refs();
    let key = |r: &NoteRef| (r.para, s.script.note(*r).map_or(0, |n| n.start));
    let r = refs
        .iter()
        .rev()
        .find(|r| key(r) < (a.para, a.off))
        .or(refs.last())
        .copied()
        .ok_or_else(|| CmdError::Disabled("the script has no notes".into()))?;
    select(s, r)
}
