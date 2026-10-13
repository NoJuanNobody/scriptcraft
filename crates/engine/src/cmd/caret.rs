//! Caret movement and selection (none of these change the script).

use scriptcraft_doc::{Pos, next_boundary, prev_boundary, word_left, word_right};
use serde_json::Value;

use super::{parse_pos, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    const EXT: &str = r#"{"extend": bool (extend the selection)}"#;
    vec![
        CommandSpec::new("caret.left", "Move Left", "Navigate", left).key("ArrowLeft").params(EXT).pure(),
        CommandSpec::new("caret.right", "Move Right", "Navigate", right).key("ArrowRight").params(EXT).pure(),
        CommandSpec::new("caret.up", "Move Up a Line", "Navigate", up).key("ArrowUp").params(EXT).pure(),
        CommandSpec::new("caret.down", "Move Down a Line", "Navigate", down).key("ArrowDown").params(EXT).pure(),
        CommandSpec::new("caret.wordLeft", "Move a Word Left", "Navigate", wleft).key("Alt+ArrowLeft").params(EXT).pure(),
        CommandSpec::new("caret.wordRight", "Move a Word Right", "Navigate", wright).key("Alt+ArrowRight").params(EXT).pure(),
        CommandSpec::new("caret.home", "Start of Line", "Navigate", home).key("Home").params(EXT).pure(),
        CommandSpec::new("caret.end", "End of Line", "Navigate", end).key("End").params(EXT).pure(),
        CommandSpec::new("caret.docStart", "Start of Script", "Navigate", doc_start).key("Mod+ArrowUp").params(EXT).pure(),
        CommandSpec::new("caret.docEnd", "End of Script", "Navigate", doc_end).key("Mod+ArrowDown").params(EXT).pure(),
        CommandSpec::new("caret.set", "Place Caret", "Navigate", set).params(r#"{"para": n, "off": byteOffset, "extend": bool}"#).pure(),
        CommandSpec::new("caret.gotoPage", "Go to Page", "Navigate", goto_page)
            .params(r#"{"page": n (page count from 1) or "12A" (printed label)}"#)
            .pure(),
        CommandSpec::new("select.all", "Select All", "Edit", all).key("Mod+A").pure(),
        CommandSpec::new("select.range", "Select Range", "Edit", range).params(r#"{"anchor": {"para", "off"}, "focus": {"para", "off"}}"#).pure(),
        CommandSpec::new("select.para", "Select Paragraph", "Edit", para).params(r#"{"para": n}"#).pure(),
        CommandSpec::new("select.text", "Select Text", "Edit", text).params(r#"{"text": string, "matchCase": bool}"#).pure(),
    ]
}

fn extend(v: &Value) -> bool {
    p::bool(v, "extend").unwrap_or(false)
}

/// Move the focus to `to`; without `extend`, collapse the selection there.
fn go(s: &mut Session, v: &Value, to: Pos) -> CmdResult {
    let to = s.script.clamp(to);
    if extend(v) {
        s.sel.focus = to;
    } else {
        s.sel = Selection::caret(to);
    }
    sel_result(s)
}

fn step(s: &Session, at: Pos, back: bool) -> Pos {
    let text = s.script.para(at.para).map_or("", |p| p.text.as_str());
    if back {
        if at.off > 0 {
            Pos::new(at.para, prev_boundary(text, at.off))
        } else if at.para > 0 {
            Pos::new(at.para - 1, s.script.para(at.para - 1).map_or(0, |p| p.len()))
        } else {
            at
        }
    } else if at.off < text.len() {
        Pos::new(at.para, next_boundary(text, at.off))
    } else if at.para + 1 < s.script.paras.len() {
        Pos::new(at.para + 1, 0)
    } else {
        at
    }
}

fn left(s: &mut Session, v: &Value) -> CmdResult {
    if !s.sel.is_caret() && !extend(v) {
        let a = s.sel.ordered().0;
        return go(s, v, a);
    }
    let to = step(s, s.sel.focus, true);
    go(s, v, to)
}

fn right(s: &mut Session, v: &Value) -> CmdResult {
    if !s.sel.is_caret() && !extend(v) {
        let b = s.sel.ordered().1;
        return go(s, v, b);
    }
    let to = step(s, s.sel.focus, false);
    go(s, v, to)
}

fn vertical(s: &mut Session, v: &Value, dir: i32) -> CmdResult {
    let l = s.view_layout();
    let at = s.sel.focus;
    match l.vertical(at, dir, s.goal_x) {
        Some((to, gx)) => {
            let r = go(s, v, to);
            s.goal_x = Some(gx);
            r
        }
        None => {
            let to = if dir < 0 { Pos::new(0, 0) } else { s.script.end() };
            go(s, v, to)
        }
    }
}

fn up(s: &mut Session, v: &Value) -> CmdResult {
    vertical(s, v, -1)
}
fn down(s: &mut Session, v: &Value) -> CmdResult {
    vertical(s, v, 1)
}

fn wleft(s: &mut Session, v: &Value) -> CmdResult {
    let at = s.sel.focus;
    let text = s.script.para(at.para).map_or("", |p| p.text.as_str());
    let to = if at.off == 0 { step(s, at, true) } else { Pos::new(at.para, word_left(text, at.off)) };
    go(s, v, to)
}

fn wright(s: &mut Session, v: &Value) -> CmdResult {
    let at = s.sel.focus;
    let text = s.script.para(at.para).map_or("", |p| p.text.as_str());
    let to = if at.off >= text.len() { step(s, at, false) } else { Pos::new(at.para, word_right(text, at.off)) };
    go(s, v, to)
}

fn home(s: &mut Session, v: &Value) -> CmdResult {
    let l = s.view_layout();
    let to = l.line_edge(s.sel.focus, false).unwrap_or(Pos::new(s.sel.focus.para, 0));
    go(s, v, to)
}

fn end(s: &mut Session, v: &Value) -> CmdResult {
    let l = s.view_layout();
    let len = s.script.para(s.sel.focus.para).map_or(0, |p| p.len());
    let to = l.line_edge(s.sel.focus, true).unwrap_or(Pos::new(s.sel.focus.para, len));
    go(s, v, to)
}

fn doc_start(s: &mut Session, v: &Value) -> CmdResult {
    go(s, v, Pos::new(0, 0))
}

fn doc_end(s: &mut Session, v: &Value) -> CmdResult {
    let e = s.script.end();
    go(s, v, e)
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let para = p::req_usize(v, "para")?;
    let off = p::usize(v, "off").unwrap_or(0);
    go(s, v, Pos::new(para, off))
}

fn goto_page(s: &mut Session, v: &Value) -> CmdResult {
    let l = s.layout();
    let idx = if let Some(label) = p::str(v, "page") {
        let label = label.trim().trim_end_matches('.');
        l.pages
            .iter()
            .position(|pg| !pg.title_page && pg.label.eq_ignore_ascii_case(label))
            .ok_or_else(|| CmdError::Params(format!("no page {label}")))?
    } else {
        let page = p::req_usize(v, "page")?;
        l.pages.iter().position(|pg| pg.number == Some(page)).ok_or_else(|| CmdError::Params(format!("no page {page}")))?
    };
    let para = l.para_page.iter().position(|&pi| pi >= idx).unwrap_or(0);
    go(s, &Value::Null, Pos::new(para, 0))
}

fn all(s: &mut Session, _: &Value) -> CmdResult {
    s.sel = Selection { anchor: Pos::new(0, 0), focus: s.script.end() };
    sel_result(s)
}

fn range(s: &mut Session, v: &Value) -> CmdResult {
    let a = parse_pos(v.get("anchor")).ok_or_else(|| CmdError::Params("`anchor` {para, off} is required".into()))?;
    let f = parse_pos(v.get("focus")).unwrap_or(a);
    s.sel = Selection { anchor: s.script.clamp(a), focus: s.script.clamp(f) };
    sel_result(s)
}

fn para(s: &mut Session, v: &Value) -> CmdResult {
    let i = p::req_usize(v, "para")?;
    let pp = s.script.clamp(Pos::new(i, 0)).para;
    let len = s.script.para(pp).map_or(0, |q| q.len());
    s.sel = Selection { anchor: Pos::new(pp, 0), focus: Pos::new(pp, len) };
    sel_result(s)
}

fn text(s: &mut Session, v: &Value) -> CmdResult {
    let needle = p::req_str(v, "text")?;
    let mc = p::bool(v, "matchCase").unwrap_or(false);
    let (a, b) = super::edit::find_from(s, needle, Pos::new(0, 0), mc).ok_or_else(|| CmdError::Failed(format!("`{needle}` not found")))?;
    s.sel = Selection { anchor: a, focus: b };
    sel_result(s)
}
