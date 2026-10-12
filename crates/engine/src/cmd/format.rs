//! Character formatting: bold, italic, underline.

use scriptcraft_doc::{Flag, Style};
use serde_json::{Value, json};

use crate::{CmdResult, CommandSpec, Session};

pub fn specs() -> Vec<CommandSpec> {
    const L: &str = "Format › Style";
    vec![
        CommandSpec::new("format.bold", "Bold", L, bold).key("Mod+B"),
        CommandSpec::new("format.italic", "Italic", L, italic).key("Mod+I"),
        CommandSpec::new("format.underline", "Underline", L, underline).key("Mod+U"),
        CommandSpec::new("format.plain", "Plain Text", L, plain).key("Mod+Shift+T"),
        CommandSpec::new("format.state", "Current Style", L, state).pure(),
    ]
}

/// The style at the caret: the pending style, or what typing would continue.
pub fn caret_style(s: &Session) -> Style {
    s.pending_style.unwrap_or_else(|| s.script.para(s.sel.focus.para).map_or(Style::PLAIN, |p| p.style_before(s.sel.focus.off)))
}

/// Toggle `f` on the selection; with only a caret, toggle it for the next typed text.
fn toggle(s: &mut Session, f: Flag) -> CmdResult {
    let (a, b) = s.sel.ordered();
    if a == b {
        let st = caret_style(s);
        s.pending_style = Some(st.with(f, !st.get(f)));
    } else {
        let on = !s.script.all_have(a, b, f);
        s.script.set_flag(a, b, f, on);
    }
    state(s, &Value::Null)
}

fn bold(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, Flag::Bold)
}
fn italic(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, Flag::Italic)
}
fn underline(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, Flag::Underline)
}

fn plain(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    if a == b {
        s.pending_style = Some(Style::PLAIN);
    } else {
        for f in [Flag::Bold, Flag::Italic, Flag::Underline] {
            s.script.set_flag(a, b, f, false);
        }
    }
    state(s, &Value::Null)
}

fn state(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    let get = |f: Flag| if a == b { caret_style(s).get(f) } else { s.script.all_have(a, b, f) };
    Ok(json!({"bold": get(Flag::Bold), "italic": get(Flag::Italic), "underline": get(Flag::Underline)}))
}
