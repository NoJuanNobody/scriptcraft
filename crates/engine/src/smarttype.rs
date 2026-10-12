//! SmartType: autocompletion of character names, extensions, scene-heading prefixes, locations,
//! times of day and transitions, from built-in lists and what the script already uses.

use scriptcraft_doc::{Element, HEADING_PREFIXES, Pos, Script, character_name, heading_parts};
use serde::Serialize;

/// Most suggestions offered at once.
pub const MAX_ITEMS: usize = 12;

pub const PREFIXES: [&str; 5] = ["INT. ", "EXT. ", "INT./EXT. ", "EST. ", "I/E. "];
pub const TIMES: [&str; 11] =
    ["DAY", "NIGHT", "MORNING", "AFTERNOON", "EVENING", "DAWN", "DUSK", "CONTINUOUS", "LATER", "MOMENTS LATER", "SAME TIME"];
pub const EXTENSIONS: [&str; 4] = ["(V.O.)", "(O.S.)", "(O.C.)", "(CONT'D)"];
pub const TRANSITIONS: [&str; 9] =
    ["CUT TO:", "DISSOLVE TO:", "SMASH CUT TO:", "MATCH CUT TO:", "JUMP CUT TO:", "FADE TO:", "FADE OUT.", "FADE TO BLACK.", "INTERCUT WITH:"];

/// Suggestions for the text before the caret: replace `text[from..]` with one of `items`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Suggest {
    pub para: usize,
    pub from: usize,
    pub items: Vec<String>,
}

fn filter(cands: impl IntoIterator<Item = String>, typed: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in cands {
        if c.starts_with(typed) && c != typed && !out.contains(&c) {
            out.push(c);
        }
        if out.len() >= MAX_ITEMS {
            break;
        }
    }
    out
}

/// What SmartType offers at `pos` (only with the caret at the end of its paragraph).
pub fn suggest(script: &Script, pos: Pos) -> Option<Suggest> {
    let p = script.para(pos.para)?;
    if pos.off != p.text.len() {
        return None;
    }
    let text = p.text.as_str();
    let others = || script.paras.iter().enumerate().filter(move |(i, _)| *i != pos.para).map(|(_, q)| q);
    let (from, items) = match p.element {
        Element::Character => {
            if let Some(i) = text.rfind('(') {
                (i, filter(EXTENSIONS.iter().map(|s| s.to_string()), text.get(i..)?))
            } else {
                let names = others().filter(|q| q.element == Element::Character).map(|q| character_name(&q.text)).filter(|n| !n.is_empty());
                (0, filter(names, text))
            }
        }
        Element::SceneHeading => {
            let (prefix, _, _) = heading_parts(text);
            if let Some(i) = text.rfind(" - ") {
                let from = i + 3;
                let typed = text.get(from..)?;
                let known = others().filter(|q| q.element == Element::SceneHeading).map(|q| heading_parts(&q.text).2).filter(|t| !t.is_empty());
                (from, filter(TIMES.iter().map(|s| s.to_string()).chain(known), typed))
            } else if prefix.is_empty() || !text.contains(' ') && HEADING_PREFIXES.iter().any(|h| h.trim() == text.trim()) {
                (0, filter(PREFIXES.iter().map(|s| s.to_string()), text))
            } else {
                // Typing the location after the prefix.
                let from = text.find(prefix.as_str()).map_or(0, |i| i + prefix.len());
                let from = from + text.get(from..).map_or(0, |t| t.len() - t.trim_start().len());
                let typed = text.get(from..)?;
                let locs = others().filter(|q| q.element == Element::SceneHeading).map(|q| heading_parts(&q.text).1).filter(|l| !l.is_empty());
                (from, filter(locs, typed))
            }
        }
        Element::Transition => (0, filter(TRANSITIONS.iter().map(|s| s.to_string()), text)),
        _ => return None,
    };
    (!items.is_empty()).then_some(Suggest { para: pos.para, from, items })
}
