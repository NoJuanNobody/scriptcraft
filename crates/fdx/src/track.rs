//! Tracked changes in FDX (ScriptCraft-only; Final Draft keeps only the current text).
//!
//! On a paragraph: `<ScriptCraftChange Kind="Insert" Range="start,length" Author="Ana"/>`,
//! `<ScriptCraftChange Kind="Delete" At="5" Author="Bo" Text="…" Elements="Action,Dialogue"/>`
//! (`&#10;` in `Text` is a removed paragraph break), `Kind="Break"` and
//! `Kind="Element" From="Action"`. Offsets count characters. Under the root,
//! `<ScriptCraftTrackChanges On="Yes"/>` when tracking is on.

use scriptcraft_doc::track::{Change, ChangeKind};
use scriptcraft_doc::{Element, Para, Script};

use crate::esc;

/// Write a paragraph's changes.
pub(crate) fn write_para(out: &mut String, indent: &str, p: &Para) {
    let chars = |b: usize| p.text.get(..b).map_or(0, |t| t.chars().count());
    for c in &p.changes {
        let who = esc(&c.author);
        match c.kind {
            ChangeKind::Insert => out.push_str(&format!(
                "{indent}  <ScriptCraftChange Kind=\"Insert\" Range=\"{},{}\" Author=\"{who}\"/>\n",
                chars(c.start),
                chars(c.end).saturating_sub(chars(c.start))
            )),
            ChangeKind::Delete => {
                let els: Vec<&str> = c.elements.iter().map(|e| e.name()).collect();
                out.push_str(&format!(
                    "{indent}  <ScriptCraftChange Kind=\"Delete\" At=\"{}\" Author=\"{who}\" Text=\"{}\" Elements=\"{}\"/>\n",
                    chars(c.start),
                    esc(&c.text).replace('\n', "&#10;"),
                    els.join(",")
                ));
            }
            ChangeKind::Break => out.push_str(&format!("{indent}  <ScriptCraftChange Kind=\"Break\" Author=\"{who}\"/>\n")),
            ChangeKind::Element => out.push_str(&format!(
                "{indent}  <ScriptCraftChange Kind=\"Element\" From=\"{}\" Author=\"{who}\"/>\n",
                c.elements.first().map_or("Action", |e| e.name())
            )),
        }
    }
}

/// The root marker.
pub(crate) fn write_root(out: &mut String, s: &Script) {
    if s.settings.track.on {
        out.push_str("  <ScriptCraftTrackChanges On=\"Yes\"/>\n");
    }
}

/// A change read from attributes; offsets in characters until [`to_bytes`].
pub(crate) fn read(attr: &dyn Fn(&str) -> Option<String>) -> Option<Change> {
    let kind = ChangeKind::parse(&attr("Kind")?)?;
    let num = |k: &str| attr(k).and_then(|v| v.trim().parse::<usize>().ok());
    let author = attr("Author").unwrap_or_default();
    let mut c = Change { kind, start: 0, end: 0, text: String::new(), elements: Vec::new(), author };
    match kind {
        ChangeKind::Insert => {
            let (a, l) = attr("Range").and_then(|r| crate::parse_range(&r))?;
            c.start = a;
            c.end = a.saturating_add(l);
        }
        ChangeKind::Delete => {
            c.start = num("At").unwrap_or(0);
            c.end = c.start;
            c.text = attr("Text").unwrap_or_default();
            c.elements = attr("Elements").map(|v| v.split(',').filter_map(Element::parse).collect()).unwrap_or_default();
        }
        ChangeKind::Break => {}
        ChangeKind::Element => c.elements = vec![attr("From").and_then(|v| Element::parse(&v))?],
    }
    Some(c)
}

/// Character offsets to byte offsets of `p`'s text.
pub(crate) fn to_bytes(p: &Para, v: Vec<Change>) -> Vec<Change> {
    let at = |chars: usize| p.text.char_indices().nth(chars).map_or(p.text.len(), |(b, _)| b);
    v.into_iter()
        .map(|mut c| {
            c.start = at(c.start);
            c.end = at(c.end);
            c
        })
        .collect()
}
