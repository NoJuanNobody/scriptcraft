//! The beat board, bookmarks and beat links in Fountain.
//!
//! Fountain has no form for these, so ScriptCraft keeps them in one boneyard block at the end of
//! the file, which other Fountain apps ignore:
//!
//! ```text
//! /* ScriptCraft board: {"beats": [...], "anchors": [{"para": 12, "text": "INT. PIER", "bookmark": "Fog"}]} */
//! ```
//!
//! Anchors find their paragraph again by its text (nearest to the paragraph number written), so
//! they survive the file being edited elsewhere as long as that paragraph's text does.

use scriptcraft_doc::{Beat, Script};
use serde::{Deserialize, Serialize};

const MARK: &str = "/* ScriptCraft board:";
/// How much of a paragraph's text an anchor keeps to find it again.
const ANCHOR_CHARS: usize = 80;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Meta {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    beats: Vec<Beat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    anchors: Vec<Anchor>,
}

#[derive(Serialize, Deserialize)]
struct Anchor {
    para: usize,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bookmark: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    beats: Vec<u32>,
}

fn key_text(t: &str) -> String {
    t.trim().chars().take(ANCHOR_CHARS).collect::<String>().to_uppercase()
}

/// Remove ScriptCraft's board block from the source (the last one counts).
pub(crate) fn extract(src: &str) -> (String, Option<Meta>) {
    let Some(a) = src.rfind(MARK) else { return (src.to_string(), None) };
    let after = src.get(a + MARK.len()..).unwrap_or("");
    let Some(b) = after.find("*/") else { return (src.to_string(), None) };
    let json = after.get(..b).unwrap_or("");
    let meta = serde_json::from_str::<Meta>(json.trim()).ok();
    let mut out = src.get(..a).unwrap_or("").to_string();
    out.push_str(after.get(b + 2..).unwrap_or(""));
    (out, meta)
}

/// Put the board and anchors back on a parsed script (before it is normalized).
pub(crate) fn apply(meta: Meta, s: &mut Script) {
    s.board.beats = meta.beats.into_iter().take(scriptcraft_doc::MAX_BEATS).collect();
    s.board.normalize();
    let keys: Vec<String> = s.paras.iter().map(|p| key_text(&p.text)).collect();
    for a in meta.anchors.into_iter().take(scriptcraft_doc::MAX_PARAS) {
        let want = key_text(&a.text);
        let Some(i) = keys.iter().enumerate().filter(|(_, k)| **k == want).map(|(i, _)| i).min_by_key(|i| i.abs_diff(a.para)) else {
            continue;
        };
        if let Some(p) = s.paras.get_mut(i) {
            if p.bookmark.is_none() {
                p.bookmark = a.bookmark;
            }
            p.beats.extend(a.beats.into_iter().take(scriptcraft_doc::MAX_BEATS));
        }
    }
    let _ = s.normalize();
}

/// The board block, when the script has a board or bookmarks.
pub(crate) fn write(out: &mut String, s: &Script) {
    let anchors: Vec<Anchor> = s
        .paras
        .iter()
        .enumerate()
        .filter(|(_, p)| p.bookmark.is_some() || !p.beats.is_empty())
        .map(|(i, p)| Anchor {
            para: i,
            text: p.text.trim().chars().take(ANCHOR_CHARS).collect(),
            bookmark: p.bookmark.clone(),
            beats: p.beats.clone(),
        })
        .collect();
    if s.board.is_empty() && anchors.is_empty() {
        return;
    }
    let meta = Meta { beats: s.board.beats.clone(), anchors };
    let Ok(json) = serde_json::to_string(&meta) else { return };
    // `*/` would end the block early; `\/` is the same character in JSON.
    let json = json.replace("*/", "*\\/");
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(&format!("{MARK} {json} */\n"));
}
