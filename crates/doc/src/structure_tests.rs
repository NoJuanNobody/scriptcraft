use super::*;

fn sp(kind: StructureKind, t: &str) -> StructurePoint {
    StructurePoint::new(kind, t, "", None)
}

/// Five scenes: A..E, each a heading and one action.
fn five() -> Script {
    let mut v = Vec::new();
    v.push(Para::new(Element::Action, "FADE IN:"));
    for c in ["A", "B", "C", "D", "E"] {
        v.push(Para::new(Element::SceneHeading, format!("INT. {c} - DAY")));
        v.push(Para::new(Element::Action, format!("In {c}.")));
    }
    Script::from_paras(v).unwrap_or_default()
}

fn headings(s: &Script) -> String {
    s.scenes().iter().map(|x| x.heading.chars().nth(5).unwrap_or('?')).collect()
}

fn outline_str(s: &Script) -> String {
    let pts = s.structure_points();
    s.outline()
        .iter()
        .map(|i| match i {
            OutlineItem::Point(n) => pts.get(*n).map(|(_, p)| format!("[{}]", p.title)).unwrap_or_default(),
            OutlineItem::Scene(n) => s.scenes().get(*n).map(|x| x.heading.chars().nth(5).unwrap_or('?').to_string()).unwrap_or_default(),
        })
        .collect()
}

#[test]
fn add_list_and_outline() {
    let mut s = five();
    assert_eq!(s.add_structure(sp(StructureKind::Act, "One"), Some(0)), Some(0));
    assert_eq!(s.add_structure(sp(StructureKind::Sequence, "Arrival"), Some(1)), Some(1));
    assert_eq!(s.add_structure(sp(StructureKind::Act, "Two"), Some(3)), Some(2));
    assert_eq!(s.add_structure(sp(StructureKind::Act, "Three"), None), Some(3));
    assert_eq!(outline_str(&s), "[One]A[Arrival]BC[Two]DE[Three]");
    assert_eq!(s.structure_path(2), vec![0, 1]);
    assert_eq!(s.structure_path(4), vec![2]);
    // A point added before a scene that has one goes after it.
    assert_eq!(s.add_structure(sp(StructureKind::Sequence, "Fallout"), Some(3)), Some(3));
    assert_eq!(outline_str(&s), "[One]A[Arrival]BC[Two][Fallout]DE[Three]");
    assert_eq!(s.remove_structure(3).map(|p| p.title), Some("Fallout".into()));
    assert!(s.remove_structure(99).is_none());
}

#[test]
fn moving_scenes_keeps_points_in_place() {
    let mut s = five();
    s.add_structure(sp(StructureKind::Act, "One"), Some(0));
    s.add_structure(sp(StructureKind::Act, "Two"), Some(2));
    // The first scene of act two moves to the top: it joins act one; act two now opens at D.
    let at = s.move_scene(2, 0);
    assert_eq!(headings(&s), "CABDE");
    assert_eq!(at.ok(), s.scenes().first().map(|x| x.para));
    assert_eq!(outline_str(&s), "[One]CAB[Two]DE");
    // Moving down: B lands after E, in act two.
    s.move_scene(2, 4).ok();
    assert_eq!(outline_str(&s), "[One]CA[Two]DEB");
    // Text and the prelude are intact.
    assert_eq!(s.paras.first().map(|p| p.text.as_str()), Some("FADE IN:"));
    assert_eq!(s.paras.len(), 11);
}

#[test]
fn outline_moves_points_and_scenes() {
    let mut s = five();
    s.add_structure(sp(StructureKind::Act, "One"), Some(0));
    s.add_structure(sp(StructureKind::Act, "Two"), Some(3));
    assert_eq!(outline_str(&s), "[One]ABC[Two]DE");
    // Move [Two] up before C: C joins act two.
    s.move_outline_item(4, 3);
    assert_eq!(outline_str(&s), "[One]AB[Two]CDE");
    // Move scene E right under [One].
    let pos_e = s.outline().iter().position(|i| *i == OutlineItem::Scene(4)).unwrap_or(0);
    s.move_outline_item(pos_e, 1);
    assert_eq!(outline_str(&s), "[One]EAB[Two]CD");
    // A point moved to the very end closes the outline.
    s.move_outline_item(0, 7);
    assert_eq!(outline_str(&s), "EAB[Two]CD[One]");
    assert_eq!(s.structure_tail.len(), 1);
    assert!(s.move_outline_item(99, 0).is_none());
    // Hostile targets clamp.
    assert!(s.move_outline_item(0, usize::MAX).is_some());
    assert_eq!(s.paras.len(), 11);
}

#[test]
fn points_survive_edits() {
    let mut s = five();
    s.add_structure(sp(StructureKind::Act, "Two"), Some(2));
    let c = s.scenes().get(2).map_or(0, |x| x.para);
    // Joining C's heading into the action before it: the act moves to the next heading (D).
    s.delete_range(Pos::new(c - 1, 5), Pos::new(c, 3));
    assert_eq!(s.scenes().len(), 4);
    assert_eq!(outline_str(&s), "AB[Two]DE");
    // Turning D's heading into action: the act moves on to E.
    let d = s.scenes().get(2).map_or(0, |x| x.para);
    s.set_element(d, Element::Action).ok();
    assert_eq!(outline_str(&s), "AB[Two]E");
    // Enter at the start of E's heading: the point follows the heading text down.
    let e = s.scenes().get(2).map_or(0, |x| x.para);
    s.split(Pos::new(e, 0), Element::SceneHeading).ok();
    assert_eq!(s.paras.get(e + 1).map(|p| p.structure.len()), Some(1));
    // Deleting everything keeps the point (in the tail).
    let end = s.end();
    s.delete_range(Pos::new(0, 0), end);
    assert_eq!(s.structure_count(), 1);
}

#[test]
fn normalize_cleans_and_caps() {
    let mut s = five();
    let long = "x".repeat(10_000);
    s.paras[1].structure.push(StructurePoint { kind: StructureKind::Act, title: format!("\u{7}{long}"), summary: long.clone(), color: None });
    // On a non-heading paragraph: moves to the next heading.
    s.paras[2].structure.push(StructurePoint { kind: StructureKind::Section, title: "   ".into(), summary: String::new(), color: None });
    s.normalize().ok();
    assert_eq!(s.paras[2].structure.len(), 0);
    assert_eq!(s.paras[3].structure.first().map(|p| p.title.as_str()), Some("Section"));
    let t = &s.paras[1].structure[0];
    assert!(t.title.chars().count() <= 200 && !t.title.contains('\u{7}'));
    assert!(t.summary.chars().count() <= 4000);
    // The cap holds.
    s.paras[1].structure = vec![sp(StructureKind::Section, "x"); MAX_STRUCTURE + 50];
    s.normalize().ok();
    assert_eq!(s.structure_count(), MAX_STRUCTURE);
    assert!(s.add_structure(sp(StructureKind::Act, "more"), None).is_none());
}

#[test]
fn serde_round_trip() {
    let mut s = five();
    s.add_structure(StructurePoint::new(StructureKind::Act, "One", "Setup.", Some(CardColor::Blue)), Some(0));
    s.add_structure(sp(StructureKind::Act, "Two"), None);
    let j = serde_json::to_string(&s).unwrap_or_default();
    let back: Script = serde_json::from_str(&j).unwrap_or_default();
    assert_eq!(back, s);
}
