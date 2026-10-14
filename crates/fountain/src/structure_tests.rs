use super::*;
use scriptcraft_doc::{CardColor, StructureKind, StructurePoint};

const SRC: &str = "Title: Test

# Act One

= The setup.

[[color: blue]]

FADE IN:

## The Arrival

INT. HOUSE - DAY

= Card summary stays on the scene.

Ada waits.

### Beat: knock

EXT. STREET - NIGHT

Rain.

# Act Two

INT. CAR - NIGHT

Driving.

# Act Three
";

#[test]
fn sections_become_structure_points() {
    let s = parse(SRC);
    let pts: Vec<(StructureKind, String, String, Option<CardColor>)> =
        s.structure_points().into_iter().map(|(_, p)| (p.kind, p.title.clone(), p.summary.clone(), p.color)).collect();
    assert_eq!(pts.len(), 5);
    assert_eq!(pts[0], (StructureKind::Act, "Act One".into(), "The setup.".into(), Some(CardColor::Blue)));
    assert_eq!(pts[1].0, StructureKind::Sequence);
    assert_eq!(pts[2], (StructureKind::Section, "Beat: knock".into(), String::new(), None));
    // Act One opens the first scene (FADE IN: is not a scene); Act Three has no scenes yet.
    let heads = s.scenes();
    assert_eq!(s.paras[heads[0].para].structure.len(), 2);
    assert_eq!(heads[0].summary, "Card summary stays on the scene.");
    assert_eq!(s.structure_tail.len(), 1);
    assert_eq!(s.structure_tail[0].title, "Act Three");
    // Sections are not script text.
    assert!(!s.paras.iter().any(|p| p.text.contains('#') || p.text.contains("Act One")));
}

#[test]
fn sections_round_trip() {
    let s = parse(SRC);
    let out = write(&s);
    assert!(out.contains("# Act One\n\n= The setup.\n\n[[color: blue]]"), "{out}");
    assert!(out.contains("## The Arrival") && out.contains("### Beat: knock") && out.trim_end().ends_with("# Act Three"));
    let back = parse(&out);
    assert_eq!(
        back.structure_points().into_iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
        s.structure_points().into_iter().map(|(_, p)| p.clone()).collect::<Vec<_>>()
    );
    assert_eq!(back.paras, s.paras);
    assert_eq!(back.structure_tail, s.structure_tail);
}

#[test]
fn hostile_sections() {
    let s = parse("#\n\n###########\n\n=\n\n# \u{7}x\n\nINT. A - DAY\n");
    assert!(s.structure_count() >= 2);
    assert!(s.structure_points().iter().all(|(_, p)| !p.title.is_empty() && !p.title.contains('\u{7}')));
    let many: String = (0..6000).map(|i| format!("# P{i}\n\n")).collect();
    let s = parse(&many);
    assert_eq!(s.structure_count(), scriptcraft_doc::MAX_STRUCTURE);
    let _ = StructurePoint::new(StructureKind::Act, "", "", None);
}
