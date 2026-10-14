//! Undo, redo, clipboard, find and replace.

use scriptcraft_doc::{Para, Pos, Script};
use serde_json::{Value, json};

use super::{delete_selection, sel_result, set_caret};
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
        CommandSpec::new("edit.paste", "Paste", "Edit", paste)
            .key("Mod+V")
            .params(r#"{"text": string, "fountain": bool (default: detect blank-line separated blocks)}"#),
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

/// The selected paragraphs, trimmed to the selection (elements, styles and flags kept).
pub fn fragment(s: &Session, a: Pos, b: Pos) -> Vec<Para> {
    let paras: Vec<Para> = s.script.paras.get(a.para..=b.para.min(s.script.paras.len().saturating_sub(1))).map(<[Para]>::to_vec).unwrap_or_default();
    let n = paras.len();
    let Ok(mut t) = Script::from_paras(paras) else { return Vec::new() };
    t.settings = s.script.settings.clone();
    let end = t.end();
    t.delete_range(Pos::new(n.saturating_sub(1), b.off), end);
    t.delete_range(Pos::new(0, 0), Pos::new(0, a.off));
    t.paras
}

/// What Copy puts on the clipboard: plain text within one paragraph, Fountain across several.
fn clip(s: &mut Session) -> Value {
    let (a, b) = s.sel.ordered();
    if a.para == b.para {
        s.clip = None;
        return json!({"text": s.script.text_between(a, b), "paragraphs": 1});
    }
    let paras = fragment(s, a, b);
    let mut frag = Script::from_paras(paras.clone()).unwrap_or_default();
    frag.settings = s.script.settings.clone();
    // Bookmarks and beat links stay in this script; the clipboard text carries none.
    for p in &mut frag.paras {
        p.bookmark = None;
        p.beats.clear();
    }
    let text = scriptcraft_fountain::write(&frag);
    let n = paras.len();
    s.clip = Some((text.clone(), paras));
    json!({"text": text, "paragraphs": n, "format": "fountain"})
}

fn copy(s: &mut Session, _: &Value) -> CmdResult {
    Ok(clip(s))
}

fn cut(s: &mut Session, _: &Value) -> CmdResult {
    let v = clip(s);
    delete_selection(s);
    Ok(v)
}

/// Insert whole paragraphs at the caret: the paragraph is split there, the pasted paragraphs go
/// between the halves, and an empty half is dropped. One paragraph pastes inline, styles kept.
pub fn paste_paras(s: &mut Session, paras: &[Para]) -> CmdResult {
    let at = delete_selection(s);
    if let [one] = paras {
        let mut pos = at;
        for (t, st) in one.runs() {
            pos = s.script.insert_styled(pos, t, st)?;
        }
        set_caret(s, pos);
        return sel_result(s);
    }
    if paras.is_empty() {
        return sel_result(s);
    }
    if s.script.paras.len() + paras.len() > scriptcraft_doc::MAX_PARAS {
        return Err(CmdError::Failed("the script would be too large".into()));
    }
    let el = s.element();
    let tail = s.script.split(at, el)?;
    for (i, p) in paras.iter().enumerate() {
        let mut p = p.clone();
        // A pasted copy doesn't take a bookmark or beat link the script already has (a cut and
        // paste, which moves them, keeps them).
        if p.bookmark.as_deref().is_some_and(|b| s.script.bookmark_para(b).is_some()) {
            p.bookmark = None;
        }
        p.beats.retain(|id| s.script.beat_para(*id).is_none());
        s.script.paras.insert(tail.para + i, p);
    }
    let mut last = tail.para + paras.len() - 1;
    if s.script.para(last + 1).is_some_and(|p| p.text.is_empty()) {
        s.script.paras.remove(last + 1);
    }
    if s.script.para(at.para).is_some_and(|p| p.text.is_empty()) {
        s.script.paras.remove(at.para);
        last -= 1;
    }
    let len = s.script.para(last).map_or(0, |p| p.len());
    set_caret(s, Pos::new(last, len));
    sel_result(s)
}

/// Paste: our own copy comes back exactly; Fountain (blank-line separated blocks) is parsed
/// into elements; other text is typed, each line a new paragraph.
fn paste(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    if let Some((t, paras)) = s.clip.clone()
        && t == text
    {
        return paste_paras(s, &paras);
    }
    let fountain = p::bool(v, "fountain").unwrap_or_else(|| text.replace("\r\n", "\n").contains("\n\n"));
    if fountain {
        let parsed = scriptcraft_fountain::parse(text);
        return paste_paras(s, &parsed.paras);
    }
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
