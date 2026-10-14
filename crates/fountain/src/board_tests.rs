use super::*;
use scriptcraft_doc::{CardColor, Element, Para, Script};

fn boarded() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. CAFÉ - NIGHT"),
        Para::new(Element::Action, "Ada sets down a key."),
        Para::new(Element::Character, "ADA"),
        Para::new(Element::Dialogue, "Is it open?"),
        Para::new(Element::SceneHeading, "EXT. PIER - DAWN"),
        Para::new(Element::Action, "Fog. Nothing */ else."),
    ])
    .unwrap_or_default();
    let a = s.board.add("Inciting */ incident", "She finds the key.\nIt opens nothing.", Some(CardColor::Pink), 100.0, 220.0).unwrap_or(0);
    let b = s.board.add("Midpoint", "", None, 760.0, 100.0).unwrap_or(0);
    s.link_beat(a, Some(0)).ok();
    s.link_beat(b, Some(1)).ok();
    s.set_bookmark(2, Some("Ada speaks")).ok();
    s.set_bookmark(5, Some("Fog")).ok();
    s
}

#[test]
fn board_and_bookmarks_round_trip_in_a_boneyard_block() {
    let s = boarded();
    let text = write(&s);
    assert_eq!(text.matches("/* ScriptCraft board:").count(), 1, "{text}");
    assert!(text.trim_end().ends_with("*/"), "{text}");
    let back = parse(&text);
    assert_eq!(back.paras.len(), s.paras.len(), "{text}");
    assert_eq!(back.board, s.board);
    for (a, b) in s.paras.iter().zip(&back.paras) {
        assert_eq!((&a.text, &a.bookmark, &a.beats), (&b.text, &b.bookmark, &b.beats));
    }
    // A script without a board or bookmarks writes no block.
    assert!(!write(&Script::from_paras(vec![Para::new(Element::Action, "x")]).unwrap_or_default()).contains("ScriptCraft board"));
}

#[test]
fn anchors_find_their_paragraph_after_outside_edits() {
    let s = boarded();
    // Someone adds a scene at the top in another app.
    let text = format!("INT. NEW - DAY\n\nNew action.\n\n{}", write(&s));
    let back = parse(&text);
    assert_eq!(back.bookmark_para("Fog"), Some(7));
    assert_eq!(back.beat_scene(1), Some(1));
    // Garbage in the block is ignored; the script still reads.
    let bad = "INT. A - DAY\n\n/* ScriptCraft board: {\"beats\": [ {\"id\": \"x\"} ], */\n\nAction.\n";
    let back = parse(bad);
    assert!(back.board.is_empty());
    assert!(back.paras.iter().any(|p| p.text == "Action."));
}
