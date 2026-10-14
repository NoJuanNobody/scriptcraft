//! Structure points, beats and bookmarks in one file.
use super::*;
use scriptcraft_doc::{StructureKind, StructurePoint};

fn v04() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. A - DAY"),
        Para::new(Element::Action, "In A."),
        Para::new(Element::SceneHeading, "INT. B - DAY"),
        Para::new(Element::Action, "In B."),
    ])
    .unwrap_or_default();
    s.add_structure(StructurePoint::new(StructureKind::Act, "Act One", "Setup.", None), Some(0));
    s.add_structure(StructurePoint::new(StructureKind::Sequence, "The Turn", "", None), Some(1));
    let id = s.board.add("Turn", "It turns.", None, 10.0, 20.0).unwrap_or(0);
    s.link_beat(id, Some(1)).ok();
    s.set_bookmark(2, Some("B mark")).ok();
    s
}

#[test]
fn structure_beats_and_bookmarks_round_trip_together() {
    let s = v04();
    let back = parse(&write(&s)).unwrap_or_default();
    assert_eq!(back.paras.iter().map(|p| p.text.clone()).collect::<Vec<_>>(), s.paras.iter().map(|p| p.text.clone()).collect::<Vec<_>>());
    for (a, b) in s.paras.iter().zip(&back.paras) {
        assert_eq!(a.structure, b.structure);
        assert_eq!(a.bookmark, b.bookmark);
        assert_eq!(a.beats.len(), b.beats.len());
    }
    assert_eq!(back.board.beats.len(), 1);
    assert_eq!(back.board.beats[0].title, "Turn");
    assert_eq!(back.beat_scene(back.board.beats[0].id), Some(1));
}
