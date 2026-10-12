//! Typing: insert, Enter, Tab, Backspace, Delete, with Final Draft-style element flow.

use scriptcraft_doc::{Element, Para, Pos, next_boundary, prev_boundary};
use serde_json::Value;

use super::{delete_selection, sel_result, set_caret};
use crate::typing::{enter_empty, enter_next, looks_like_heading, shift_tab, tab_empty, tab_next};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Longest text one `text.insert` accepts.
const MAX_INSERT: usize = 16 << 20;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("text.insert", "Type Text", "Edit", insert).params(r#"{"text": string (newlines start new paragraphs)}"#),
        CommandSpec::new("text.enter", "New Paragraph", "Edit", enter).key("Enter"),
        CommandSpec::new("text.tab", "Next Element", "Edit", tab).key("Tab"),
        CommandSpec::new("text.shiftTab", "Previous Element", "Edit", back_tab).key("Shift+Tab"),
        CommandSpec::new("text.backspace", "Delete Backward", "Edit", backspace).key("Backspace"),
        CommandSpec::new("text.delete", "Delete Forward", "Edit", delete).key("Delete"),
    ]
}

/// The paragraph counts as empty for element flow (blank, or just the `()` of a parenthetical).
fn is_blank(p: &Para) -> bool {
    let t = p.text.trim();
    t.is_empty() || (p.element == Element::Parenthetical && t == "()")
}

/// Change a paragraph's element; an empty Parenthetical gets `()` with the caret inside, and a
/// bare `()` is cleared when it stops being one.
pub fn convert(s: &mut Session, para: usize, e: Element) -> Result<(), CmdError> {
    let was = s.script.para(para).map(|p| (p.element, p.text.clone()));
    s.script.set_element(para, e)?;
    if let Some((old, text)) = was {
        if old == Element::Parenthetical && e != Element::Parenthetical && text.trim() == "()" {
            s.script.para_mut(para)?.text.clear();
        }
        if e == Element::Parenthetical && text.trim().is_empty() {
            s.script.para_mut(para)?.text = "()".into();
            set_caret(s, Pos::new(para, 1));
            return Ok(());
        }
    }
    let off = s.sel.focus.off;
    set_caret(s, Pos::new(para, off));
    Ok(())
}

/// Insert a new paragraph after `para` with element `e`, caret in it.
fn new_after(s: &mut Session, para: usize, e: Element) -> Result<(), CmdError> {
    let end = s.script.para(para).map_or(0, |p| p.len());
    let at = s.script.split(Pos::new(para, end), e)?;
    set_caret(s, at);
    convert(s, at.para, e)
}

fn type_str(s: &mut Session, at: Pos, text: &str) -> Result<Pos, CmdError> {
    let Some(p) = s.script.para(at.para) else { return Ok(at) };
    if p.element == Element::Parenthetical {
        // Typing over the automatic parentheses.
        if text == "(" && at.off > 0 && p.text.get(at.off - 1..at.off) == Some("(") {
            return Ok(at);
        }
        if text == ")" && p.text.get(at.off..).is_some_and(|t| t.starts_with(')')) {
            return Ok(Pos::new(at.para, at.off + 1));
        }
    }
    let element = p.element;
    let st = s.pending_style.unwrap_or_else(|| p.style_before(at.off));
    let after = s.script.insert_styled(at, text, st)?;
    // `int. ` typed at the start of Action makes it a Scene Heading.
    if element == Element::Action
        && let Some(p) = s.script.para(at.para)
        && p.text.len() <= 12
        && looks_like_heading(&p.text)
    {
        s.script.set_element(at.para, Element::SceneHeading)?;
    }
    Ok(after)
}

pub(crate) fn insert(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    if text.len() > MAX_INSERT {
        return Err(CmdError::Params("text is too long".into()));
    }
    let mut at = delete_selection(s);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            let e = s.script.para(at.para).map_or(Element::Action, |p| p.element);
            at = s.script.split(at, enter_next(e))?;
        }
        at = type_str(s, at, line.trim_end_matches('\r'))?;
    }
    s.pending_style = None;
    set_caret(s, at);
    sel_result(s)
}

fn enter(s: &mut Session, _: &Value) -> CmdResult {
    let mut at = delete_selection(s);
    let Some(p) = s.script.para(at.para).cloned() else { return sel_result(s) };
    if is_blank(&p) {
        convert(s, at.para, enter_empty(p.element))?;
        let end = s.script.para(at.para).map_or(0, |q| if q.element == Element::Parenthetical { 1 } else { q.len() });
        set_caret(s, Pos::new(at.para, end));
        return sel_result(s);
    }
    // Enter just before a parenthetical's closing `)` ends it.
    if p.element == Element::Parenthetical && p.text.ends_with(')') && at.off + 1 == p.text.len() {
        at.off = p.text.len();
    }
    if at.off == 0 {
        // At the start: a new empty paragraph of the same element above.
        s.script.paras.insert(at.para, Para::new(p.element, ""));
        set_caret(s, Pos::new(at.para + 1, 0));
        return sel_result(s);
    }
    let tail_empty = p.text.get(at.off..).is_none_or(|t| t.trim().is_empty());
    let next = if tail_empty { enter_next(p.element) } else { p.element };
    let n = s.script.split(at, next)?;
    set_caret(s, n);
    if tail_empty {
        convert(s, n.para, next)?;
    }
    sel_result(s)
}

fn at_end(p: &Para, off: usize) -> bool {
    off >= p.text.trim_end().len() || (p.element == Element::Parenthetical && p.text.ends_with(')') && off + 1 >= p.text.len())
}

fn tab(s: &mut Session, _: &Value) -> CmdResult {
    let at = s.sel.focus;
    let Some(p) = s.script.para(at.para).cloned() else { return sel_result(s) };
    if is_blank(&p) {
        convert(s, at.para, tab_empty(p.element))?;
    } else if at_end(&p, at.off) {
        new_after(s, at.para, tab_next(p.element))?;
    } else {
        convert(s, at.para, tab_empty(p.element))?;
    }
    sel_result(s)
}

fn back_tab(s: &mut Session, _: &Value) -> CmdResult {
    let at = s.sel.focus;
    let e = s.script.para(at.para).map_or(Element::Action, |p| p.element);
    convert(s, at.para, shift_tab(e))?;
    sel_result(s)
}

fn backspace(s: &mut Session, _: &Value) -> CmdResult {
    if !s.sel.is_caret() {
        delete_selection(s);
        return sel_result(s);
    }
    let at = s.script.clamp(s.sel.focus);
    let Some(p) = s.script.para(at.para).cloned() else { return sel_result(s) };
    if p.element == Element::Parenthetical && p.text == "()" && at.off == 1 {
        s.script.para_mut(at.para)?.text.clear();
        set_caret(s, Pos::new(at.para, 0));
    } else if at.off > 0 {
        let from = prev_boundary(&p.text, at.off);
        let a = s.script.delete_range(Pos::new(at.para, from), at);
        set_caret(s, a);
    } else if at.para > 0 {
        let prev = at.para - 1;
        let prev_len = s.script.para(prev).map_or(0, |q| q.len());
        if p.text.is_empty() {
            s.script.paras.remove(at.para);
            set_caret(s, Pos::new(prev, prev_len));
        } else if s.script.para(prev).is_some_and(|q| q.text.trim().is_empty()) {
            s.script.paras.remove(prev);
            set_caret(s, Pos::new(prev, 0));
        } else {
            let a = s.script.delete_range(Pos::new(prev, prev_len), at);
            set_caret(s, a);
        }
    }
    sel_result(s)
}

fn delete(s: &mut Session, _: &Value) -> CmdResult {
    if !s.sel.is_caret() {
        delete_selection(s);
        return sel_result(s);
    }
    let at = s.script.clamp(s.sel.focus);
    let Some(p) = s.script.para(at.para).cloned() else { return sel_result(s) };
    if at.off < p.text.len() {
        let to = next_boundary(&p.text, at.off);
        let a = s.script.delete_range(at, Pos::new(at.para, to));
        set_caret(s, a);
    } else if at.para + 1 < s.script.paras.len() {
        if p.text.is_empty() {
            s.script.paras.remove(at.para);
            set_caret(s, Pos::new(at.para, 0));
        } else {
            let a = s.script.delete_range(at, Pos::new(at.para + 1, 0));
            set_caret(s, a);
        }
    }
    sel_result(s)
}
