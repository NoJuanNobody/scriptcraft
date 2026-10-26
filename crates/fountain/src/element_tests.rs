//! Shot and General survive Fountain (element notes at the end of the line).
use super::*;
use scriptcraft_doc::{Element, Note, Para, Script, StructureKind, StructurePoint};

fn all_eight() -> Script {
    let mut dual = Para::new(Element::Character, "BEN");
    dual.dual = true;
    Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. GARAGE - NIGHT"),
        Para::new(Element::Action, "Ada wipes down the bike."),
        Para::new(Element::Shot, "CLOSE ON THE KEYS"),
        Para::new(Element::Character, "ADA"),
        Para::new(Element::Parenthetical, "(quietly)"),
        Para::new(Element::Dialogue, "Start it."),
        dual,
        Para::new(Element::Dialogue, "Now?"),
        Para::new(Element::General, "A title card reads: three days later."),
        Para::new(Element::Shot, "ANGLE ON: ADA"),
        Para::new(Element::Shot, "insert - the map"),
        Para::new(Element::General, "INT. NOT A HEADING"),
        Para::new(Element::Shot, "!BANG"),
        Para::new(Element::Transition, "CUT TO:"),
    ])
    .unwrap_or_default()
}

#[test]
fn all_eight_elements_round_trip() {
    let s = all_eight();
    let out = write(&s);
    let back = parse(&out);
    let kinds = |s: &Script| s.paras.iter().map(|p| (p.element, p.text.clone(), p.dual)).collect::<Vec<_>>();
    assert_eq!(kinds(&back), kinds(&s), "\n{out}");
    assert_eq!(back, s, "\n{out}");
    assert_eq!(write(&back), out, "stable");
    // Written as action with the element note at the end of the line.
    assert!(out.contains("!CLOSE ON THE KEYS [[shot]]\n"), "{out}");
    assert!(out.contains("\nA title card reads: three days later. [[general]]\n"), "{out}");
    assert!(out.contains("!INT. NOT A HEADING [[general]]"), "{out}");
    assert!(out.contains("!!BANG [[shot]]"), "{out}");
}

#[test]
fn other_apps_read_them_as_action() {
    // What an app that ignores notes sees: the same lines without the notes are action.
    let out = write(&all_eight());
    let plain: String = out.lines().map(|l| element_note(l).map_or(l, |(rest, _)| rest)).collect::<Vec<_>>().join("\n");
    let s = parse(&plain);
    for t in ["CLOSE ON THE KEYS", "A title card reads: three days later.", "ANGLE ON: ADA", "INT. NOT A HEADING"] {
        assert_eq!(s.paras.iter().find(|p| p.text == t).map(|p| p.element), Some(Element::Action), "{t}");
    }
    // And a plain Fountain file with no notes reads exactly as before.
    assert!(parse("!CLOSE ON THE KEYS\n").paras.iter().all(|p| p.element == Element::Action));
}

#[test]
fn element_notes_are_case_insensitive_and_only_at_the_end() {
    let s = parse("CLOSE ON THE KEYS [[Shot]]\n\nThe [[shot]] rings out.\n\n[[general]]\nJust action.\n");
    assert_eq!(s.paras[0].element, Element::Shot);
    assert_eq!(s.paras[0].text, "CLOSE ON THE KEYS");
    // In the middle of a line, or on its own line, it is an ordinary note.
    assert_eq!(s.paras[1].element, Element::Action);
    assert!(s.paras[1].text.starts_with("The") && s.paras[1].text.ends_with("rings out."), "{:?}", s.paras[1].text);
    assert!(s.paras[1].notes.iter().any(|n: &Note| n.text == "shot"));
    assert_eq!(s.paras[2].element, Element::Action);
    assert!(s.paras[2].notes.iter().any(|n| n.text == "general"));
}

#[test]
fn coexists_with_sections_synopses_notes_emphasis_and_the_board() {
    let mut s = all_eight();
    s.add_structure(StructurePoint::new(StructureKind::Act, "Act One", "Setup.", None), Some(0));
    if let Some(p) = s.paras.get_mut(2) {
        p.notes.push(Note::new(0, 0, "Use the long lens", None, ""));
    }
    if let Some(p) = s.paras.get_mut(0) {
        p.summary = "Ada gets ready.".into();
    }
    s.set_bookmark(2, Some("Keys")).ok();
    let id = s.board.add("Keys", "The keys matter.", None, 0.0, 0.0).unwrap_or(0);
    s.link_beat(id, Some(0)).ok();
    let it = scriptcraft_doc::Style { italic: true, ..scriptcraft_doc::Style::PLAIN };
    s.paras.push(Para::styled(
        Element::Shot,
        &[("WIDE - ".into(), scriptcraft_doc::Style::PLAIN), ("the".into(), it), (" road".into(), scriptcraft_doc::Style::PLAIN)],
    ));
    let s = Script::from_paras(s.paras.clone())
        .map(|mut x| {
            x.board = s.board.clone();
            x.structure_tail = s.structure_tail.clone();
            x
        })
        .unwrap_or_default();
    let out = write(&s);
    let back = parse(&out);
    assert_eq!(back.paras.iter().map(|p| p.element).collect::<Vec<_>>(), s.paras.iter().map(|p| p.element).collect::<Vec<_>>(), "\n{out}");
    assert_eq!(back.paras[0].structure, s.paras[0].structure);
    assert_eq!(back.paras[0].summary, "Ada gets ready.");
    assert_eq!(back.paras[2].notes.first().map(|n| n.text.as_str()), Some("Use the long lens"));
    assert_eq!(back.paras[2].bookmark.as_deref(), Some("Keys"));
    assert_eq!(back.board.beats.len(), 1);
    assert_eq!(back.paras.last(), s.paras.last(), "the styled shot comes back whole\n{out}");
    assert!(back.paras.last().is_some_and(|p| p.styles().get(8).is_some_and(|st| st.italic)), "{out}");
}
