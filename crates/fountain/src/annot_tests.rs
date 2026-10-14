use super::*;

#[test]
fn notes_are_imported_not_dropped() {
    let src = "INT. LAB - NIGHT\n\n[[Is this the right lab?]]\nBeakers [[too many?]] bubble over.\n\nADA [[older?]]\nIt works. [[Punchier]]\n\n[[A note at the very end]]\n";
    let s = parse(src);
    let texts: Vec<&str> = s.paras.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["INT. LAB - NIGHT", "Beakers bubble over.", "ADA", "It works."]);
    let action = &s.paras[1];
    let notes: Vec<(usize, &str)> = action.notes.iter().map(|n| (n.start, n.text.as_str())).collect();
    assert_eq!(notes, [(0, "Is this the right lab?"), (8, "too many?")]);
    assert_eq!(s.paras[2].element, Element::Character);
    assert_eq!(s.paras[2].notes[0].text, "older?");
    assert_eq!(s.paras[3].element, Element::Dialogue);
    let last: Vec<(usize, &str)> = s.paras[3].notes.iter().map(|n| (n.start, n.text.as_str())).collect();
    assert_eq!(last, [(0, "A note at the very end"), (9, "Punchier")]);
}

#[test]
fn notes_round_trip_as_lines() {
    let src = "INT. LAB - NIGHT\n\n[[Check the lab]]\nBeakers bubble.\n\nADA\n[[Softer]]\nIt works.\n";
    let s = parse(src);
    let out = write(&s);
    assert!(out.contains("[[Check the lab]]\nBeakers bubble."), "{out}");
    let back = parse(&out);
    let texts: Vec<&str> = back.paras.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["INT. LAB - NIGHT", "Beakers bubble.", "ADA", "It works."]);
    assert_eq!(back.paras[1].notes[0].text, "Check the lab");
    assert_eq!(back.paras[3].notes[0].text, "Softer");
    assert_eq!(back.paras[2].element, Element::Character, "a note line inside a dialogue block keeps the block");
}

#[test]
fn hostile_note_markup() {
    for src in ["[[", "]]", "[[]]", "a [[b", "\u{E000}9\u{E001}", "x\u{E000}\u{E001}y [[n]]", "[[ [[x]] ]]", "@A [[n]]\n[[m]]"] {
        let s = parse(src);
        for p in &s.paras {
            assert!(!p.text.contains(['\u{E000}', '\u{E001}', '\u{2}']), "{src:?} → {:?}", p.text);
            for n in &p.notes {
                assert!(n.start <= n.end && n.end <= p.text.len());
            }
        }
        let _ = write(&s);
    }
}
