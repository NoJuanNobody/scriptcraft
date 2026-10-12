//! Element types: set the element of the selected paragraphs; dual dialogue.

use scriptcraft_doc::Element;
use serde_json::Value;

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

macro_rules! element_cmd {
    ($f:ident, $e:expr) => {
        fn $f(s: &mut Session, _: &Value) -> CmdResult {
            apply(s, $e)
        }
    };
}

element_cmd!(scene_heading, Element::SceneHeading);
element_cmd!(action, Element::Action);
element_cmd!(character, Element::Character);
element_cmd!(parenthetical, Element::Parenthetical);
element_cmd!(dialogue, Element::Dialogue);
element_cmd!(transition, Element::Transition);
element_cmd!(shot, Element::Shot);
element_cmd!(general, Element::General);

pub fn specs() -> Vec<CommandSpec> {
    const L: &str = "Format › Elements";
    vec![
        CommandSpec::new("element.set", "Set Element", L, set)
            .params(r#"{"element": "sceneHeading"|"action"|"character"|"parenthetical"|"dialogue"|"transition"|"shot"|"general"}"#),
        CommandSpec::new("element.sceneHeading", "Scene Heading", L, scene_heading).key("Mod+1"),
        CommandSpec::new("element.action", "Action", L, action).key("Mod+2"),
        CommandSpec::new("element.character", "Character", L, character).key("Mod+3"),
        CommandSpec::new("element.parenthetical", "Parenthetical", L, parenthetical).key("Mod+4"),
        CommandSpec::new("element.dialogue", "Dialogue", L, dialogue).key("Mod+5"),
        CommandSpec::new("element.transition", "Transition", L, transition).key("Mod+6"),
        CommandSpec::new("element.shot", "Shot", L, shot).key("Mod+7"),
        CommandSpec::new("element.general", "General", L, general).key("Mod+8"),
        CommandSpec::new("dialogue.toggleDual", "Dual Dialogue", "Format", toggle_dual).key("Mod+D").when(dual_ok),
    ]
}

/// Set the element of every paragraph the selection touches.
fn apply(s: &mut Session, e: Element) -> CmdResult {
    let (a, b) = s.sel.ordered();
    if a.para == b.para {
        super::text::convert(s, a.para, e)?;
    } else {
        for i in a.para..=b.para.min(s.script.paras.len().saturating_sub(1)) {
            s.script.set_element(i, e)?;
        }
    }
    sel_result(s)
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "element")?;
    let e = Element::parse(name).ok_or_else(|| CmdError::Params(format!("unknown element `{name}`")))?;
    apply(s, e)
}

/// The character cue of the dialogue block holding paragraph `i`, if it follows another block.
fn dual_cue(s: &Session, i: usize) -> Option<usize> {
    let mut k = i;
    loop {
        let p = s.script.para(k)?;
        match p.element {
            Element::Character => break,
            Element::Parenthetical | Element::Dialogue => k = k.checked_sub(1)?,
            _ => return None,
        }
    }
    // The previous paragraph must end another dialogue block.
    let prev = s.script.para(k.checked_sub(1)?)?;
    matches!(prev.element, Element::Dialogue | Element::Parenthetical).then_some(k)
}

fn dual_ok(s: &Session) -> Option<&'static str> {
    dual_cue(s, s.sel.focus.para).is_none().then_some("put the caret in the second of two consecutive dialogue blocks")
}

fn toggle_dual(s: &mut Session, _: &Value) -> CmdResult {
    let k = dual_cue(s, s.sel.focus.para).ok_or_else(|| CmdError::Disabled("not in a second dialogue block".into()))?;
    let p = s.script.para_mut(k)?;
    p.dual = !p.dual;
    sel_result(s)
}
