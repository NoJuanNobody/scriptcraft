//! Command implementations, one module per area. Each module exposes `specs()`.

pub mod caret;
pub mod edit;
pub mod element;
pub mod file;
pub mod format;
pub mod report;
pub mod scene;
pub mod smart;
pub mod text;
pub mod title;

use scriptcraft_doc::Pos;
use serde_json::{Value, json};

use crate::{Registry, Selection, Session};

/// The full registry.
pub fn registry() -> Registry {
    let mut v = Vec::new();
    v.extend(text::specs());
    v.extend(caret::specs());
    v.extend(edit::specs());
    v.extend(element::specs());
    v.extend(format::specs());
    v.extend(smart::specs());
    v.extend(scene::specs());
    v.extend(title::specs());
    v.extend(report::specs());
    v.extend(file::specs());
    Registry::new(v)
}

/// A position as JSON.
pub fn pos_json(p: Pos) -> Value {
    json!({"para": p.para, "off": p.off})
}

/// A position from JSON (`{"para": 3, "off": 5}`).
pub fn parse_pos(v: Option<&Value>) -> Option<Pos> {
    let v = v?;
    let para = crate::p::usize(v, "para")?;
    let off = crate::p::usize(v, "off").unwrap_or(0);
    Some(Pos::new(para, off))
}

/// The standard result: the selection after the command and the caret's element.
pub fn sel_result(s: &Session) -> Result<Value, crate::CmdError> {
    Ok(json!({"anchor": pos_json(s.sel.anchor), "focus": pos_json(s.sel.focus), "element": s.element().id()}))
}

/// Put the caret at `p` (clamped).
pub fn set_caret(s: &mut Session, p: Pos) {
    s.sel = Selection::caret(s.script.clamp(p));
}

/// Delete the selection; returns the collapsed position.
pub fn delete_selection(s: &mut Session) -> Pos {
    let (a, b) = s.sel.ordered();
    let at = if a == b { s.script.clamp(a) } else { s.script.delete_range(a, b) };
    s.sel = Selection::caret(at);
    at
}
