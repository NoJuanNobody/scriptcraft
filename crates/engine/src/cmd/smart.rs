//! SmartType commands.

use scriptcraft_doc::Pos;
use serde_json::{Value, json};

use super::{sel_result, set_caret};
use crate::smarttype::suggest;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("smarttype.suggest", "SmartType Suggestions", "Tools › SmartType", list).pure(),
        CommandSpec::new("smarttype.accept", "Accept SmartType Suggestion", "Tools › SmartType", accept).params(r#"{"index": n (default 0)}"#),
    ]
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    Ok(match suggest(&s.script, &s.sluglines, s.sel.focus) {
        Some(sg) => json!({"para": sg.para, "from": sg.from, "items": sg.items}),
        None => json!({"items": []}),
    })
}

fn accept(s: &mut Session, v: &Value) -> CmdResult {
    let i = p::usize(v, "index").unwrap_or(0);
    let sg = suggest(&s.script, &s.sluglines, s.sel.focus).ok_or_else(|| CmdError::Disabled("no suggestions here".into()))?;
    let item = sg.items.get(i).ok_or_else(|| CmdError::Params(format!("no suggestion {i}")))?.clone();
    let end = s.script.para(sg.para).map_or(0, |q| q.len());
    let at = s.script.delete_range(Pos::new(sg.para, sg.from), Pos::new(sg.para, end));
    let at = s.script.insert_text(at, &item)?;
    set_caret(s, at);
    sel_result(s)
}
