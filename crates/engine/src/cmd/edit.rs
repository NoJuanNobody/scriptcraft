//! Undo, redo, clipboard, find and replace.

use scriptcraft_doc::Pos;
use serde_json::{Value, json};

use super::{delete_selection, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

fn can_undo(s: &Session) -> Option<&'static str> {
    (!s.can_undo()).then_some("nothing to undo")
}
fn can_redo(s: &Session) -> Option<&'static str> {
    (!s.can_redo()).then_some("nothing to redo")
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.undo", "Undo", "Edit", undo).key("Mod+Z").when(can_undo),
        CommandSpec::new("edit.redo", "Redo", "Edit", redo).key("Mod+Shift+Z / Mod+Y").when(can_redo),
        CommandSpec::new("edit.copy", "Copy", "Edit", copy).key("Mod+C").pure(),
        CommandSpec::new("edit.cut", "Cut", "Edit", cut).key("Mod+X"),
        CommandSpec::new("edit.paste", "Paste", "Edit", paste).key("Mod+V").params(r#"{"text": string}"#),
        CommandSpec::new("edit.find", "Find Next", "Edit › Find", find)
            .key("Mod+G")
            .params(r#"{"text": string, "matchCase": bool, "backwards": bool}"#)
            .pure(),
        CommandSpec::new("edit.replace", "Replace", "Edit › Find", replace).params(r#"{"find": string, "replace": string, "matchCase": bool}"#),
        CommandSpec::new("edit.replaceAll", "Replace All", "Edit › Find", replace_all)
            .params(r#"{"find": string, "replace": string, "matchCase": bool}"#),
    ]
}

fn undo(s: &mut Session, _: &Value) -> CmdResult {
    s.undo();
    sel_result(s)
}

fn redo(s: &mut Session, _: &Value) -> CmdResult {
    s.redo();
    sel_result(s)
}

fn copy(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    Ok(json!({"text": s.script.text_between(a, b)}))
}

fn cut(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    let text = s.script.text_between(a, b);
    delete_selection(s);
    Ok(json!({"text": text}))
}

fn paste(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    super::text::insert(s, &json!({"text": text}))
}

/// The match of `needle` in `hay` starting at or after byte `from`: (start, end).
pub fn find_in(hay: &str, needle: &str, from: usize, match_case: bool) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    if match_case {
        let from = (from..=hay.len()).find(|&i| hay.is_char_boundary(i))?;
        return hay.get(from..)?.find(needle).map(|i| (from + i, from + i + needle.len()));
    }
    let n: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    for (i, _) in hay.char_indices().filter(|(i, _)| *i >= from) {
        let mut k = 0;
        for (j, c) in hay.get(i..)?.char_indices() {
            let mut ok = true;
            for lc in c.to_lowercase() {
                if n.get(k) != Some(&lc) {
                    ok = false;
                    break;
                }
                k += 1;
            }
            if !ok {
                break;
            }
            if k == n.len() {
                return Some((i, i + j + c.len_utf8()));
            }
        }
    }
    None
}

/// The next match at or after `from`, wrapping around to the start.
pub fn find_from(s: &Session, needle: &str, from: Pos, mc: bool) -> Option<(Pos, Pos)> {
    let n = s.script.paras.len();
    for k in 0..=n {
        let i = (from.para + k) % n.max(1);
        let start = if k == 0 { from.off } else { 0 };
        if k == n && from.off == 0 {
            break;
        }
        let text = s.script.para(i).map_or("", |p| p.text.as_str());
        let limit = if k == n { from.off } else { usize::MAX };
        if let Some((a, b)) = find_in(text, needle, start, mc).filter(|&(_, b)| b <= limit) {
            return Some((Pos::new(i, a), Pos::new(i, b)));
        }
    }
    None
}

/// The last match ending at or before `before`, wrapping around to the end.
fn find_back(s: &Session, needle: &str, before: Pos, mc: bool) -> Option<(Pos, Pos)> {
    let mut all = Vec::new();
    for (i, p) in s.script.paras.iter().enumerate() {
        let mut at = 0;
        while let Some((a, b)) = find_in(&p.text, needle, at, mc) {
            all.push((Pos::new(i, a), Pos::new(i, b)));
            at = b.max(a + 1);
        }
    }
    all.iter().rev().find(|(_, b)| *b <= before).or_else(|| all.last()).copied()
}

fn find(s: &mut Session, v: &Value) -> CmdResult {
    let needle = p::req_str(v, "text")?;
    let mc = p::bool(v, "matchCase").unwrap_or(false);
    let (a, b) = s.sel.ordered();
    let hit = if p::bool(v, "backwards").unwrap_or(false) { find_back(s, needle, a, mc) } else { find_from(s, needle, b, mc) };
    let (a, b) = hit.ok_or_else(|| CmdError::Failed(format!("`{needle}` not found")))?;
    s.sel = Selection { anchor: a, focus: b };
    sel_result(s)
}

fn replace(s: &mut Session, v: &Value) -> CmdResult {
    let needle = p::req_str(v, "find")?;
    let with = p::str(v, "replace").unwrap_or("");
    let mc = p::bool(v, "matchCase").unwrap_or(false);
    let (a, b) = s.sel.ordered();
    let selected = s.script.text_between(a, b);
    let matches = if mc { selected == needle } else { selected.to_lowercase() == needle.to_lowercase() };
    if matches && a.para == b.para {
        let at = s.script.delete_range(a, b);
        let end = s.script.insert_text(at, with)?;
        s.sel = Selection::caret(end);
    }
    let from = s.sel.ordered().1;
    if let Some((a, b)) = find_from(s, needle, from, mc) {
        s.sel = Selection { anchor: a, focus: b };
    }
    sel_result(s)
}

fn replace_all(s: &mut Session, v: &Value) -> CmdResult {
    let needle = p::req_str(v, "find")?;
    let with = p::str(v, "replace").unwrap_or("");
    let mc = p::bool(v, "matchCase").unwrap_or(false);
    let mut count = 0usize;
    for i in 0..s.script.paras.len() {
        let mut at = 0;
        loop {
            let text = s.script.para(i).map_or("", |p| p.text.as_str());
            let Some((a, b)) = find_in(text, needle, at, mc) else { break };
            s.script.delete_range(Pos::new(i, a), Pos::new(i, b));
            let end = s.script.insert_text(Pos::new(i, a), with)?;
            at = end.off.max(a + 1);
            count += 1;
            if count > 1_000_000 {
                break;
            }
        }
    }
    s.clamp_selection();
    Ok(json!({"count": count}))
}
