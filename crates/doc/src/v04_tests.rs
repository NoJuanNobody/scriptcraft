//! Structure points, beats and bookmarks together (outline + boards).
use super::*;

/// Three scenes; an act before the first, a sequence before the second; the second heading has
/// a bookmark and a linked beat.
pub(crate) fn v04() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. A - DAY"),
        Para::new(Element::Action, "In A."),
        Para::new(Element::SceneHeading, "INT. B - DAY"),
        Para::new(Element::Action, "In B."),
        Para::new(Element::SceneHeading, "INT. C - DAY"),
        Para::new(Element::Action, "In C."),
    ])
    .unwrap_or_default();
    s.add_structure(StructurePoint::new(StructureKind::Act, "Act One", "", None), Some(0));
    s.add_structure(StructurePoint::new(StructureKind::Sequence, "The Turn", "", None), Some(1));
    let id = s.board.add("Turn", "It turns.", None, 10.0, 20.0).unwrap_or(0);
    s.link_beat(id, Some(1)).ok();
    s.set_bookmark(2, Some("B mark")).ok();
    s
}

#[test]
fn moving_a_scene_keeps_its_bookmark_and_beat_link() {
    let mut s = v04();
    let id = s.board.beats[0].id;
    let at = s.move_scene(1, 2).unwrap_or(usize::MAX);
    s.normalize().ok();
    let h = &s.paras[at];
    assert_eq!(h.text, "INT. B - DAY");
    assert_eq!(h.bookmark.as_deref(), Some("B mark"));
    assert_eq!(h.beats, vec![id]);
    assert_eq!(s.beat_scene(id), Some(2));
    // The points stayed in place: the sequence now opens the scene that took B's slot.
    let order: Vec<(String, String)> =
        s.paras.iter().filter(|p| !p.structure.is_empty()).map(|p| (p.text.clone(), p.structure[0].title.clone())).collect();
    assert_eq!(order, vec![("INT. A - DAY".into(), "Act One".into()), ("INT. C - DAY".into(), "The Turn".into())]);
    // Nothing else picked up the anchors.
    assert_eq!(s.paras.iter().filter(|p| p.bookmark.is_some()).count(), 1);
}

#[test]
fn enter_at_start_of_a_heading_moves_all_its_anchors_down() {
    let mut s = v04();
    let next = s.split(Pos::new(2, 0), Element::SceneHeading).unwrap_or(Pos::new(0, 0));
    s.normalize().ok();
    let p = &s.paras[next.para];
    assert_eq!(p.text, "INT. B - DAY");
    assert_eq!(p.bookmark.as_deref(), Some("B mark"));
    assert_eq!(p.beats.len(), 1);
    assert_eq!(p.structure.len(), 1);
    assert!(s.paras[2].bookmark.is_none() && s.paras[2].beats.is_empty());
}
