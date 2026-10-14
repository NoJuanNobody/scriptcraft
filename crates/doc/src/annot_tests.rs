use super::*;
use crate::{Element, Pos, Script};

fn script(lines: &[(Element, &str)]) -> Script {
    Script::from_paras(lines.iter().map(|(e, t)| Para::new(*e, *t)).collect()).unwrap_or_default()
}

#[test]
fn notes_and_tags_follow_edits() {
    let mut s = script(&[(Element::Action, "Ada lifts the old revolver."), (Element::Action, "It is empty.")]);
    // "old revolver" = 14..26
    assert_eq!(s.add_tag(Pos::new(0, 13), Pos::new(0, 26), "props"), 1, "spaces are left out");
    assert_eq!(s.paras[0].tagged[0], Tagged { start: 14, end: 26, category: "Props".into() });
    assert_eq!(s.add_note(Pos::new(0, 4), Pos::new(0, 9), "Stronger verb?", None, "Jo"), Some(0));
    // Typing before both moves them; typing inside the tag grows it; typing at its end does not.
    s.insert_text(Pos::new(0, 0), "Then ").ok();
    assert_eq!(s.paras[0].slice(s.paras[0].tagged[0].start, s.paras[0].tagged[0].end), "old revolver");
    assert_eq!(s.paras[0].slice(s.paras[0].notes[0].start, s.paras[0].notes[0].end), "lifts");
    let t = s.paras[0].tagged[0].clone();
    s.insert_text(Pos::new(0, t.start + 3), ", rusty").ok();
    s.insert_text(Pos::new(0, s.paras[0].tagged[0].end), " pistol").ok();
    assert_eq!(s.paras[0].slice(s.paras[0].tagged[0].start, s.paras[0].tagged[0].end), "old, rusty revolver");
    // Deleting the tagged words removes the tag; deleting the note's text leaves a point note.
    let t = s.paras[0].tagged[0].clone();
    s.delete_range(Pos::new(0, t.start), Pos::new(0, t.end));
    assert!(s.paras[0].tagged.is_empty());
    let n = s.paras[0].notes[0].clone();
    s.delete_range(Pos::new(0, n.start), Pos::new(0, n.end));
    assert_eq!((s.paras[0].notes[0].start, s.paras[0].notes[0].end), (n.start, n.start));
    assert_eq!(s.paras[0].notes[0].text, "Stronger verb?");
    assert_eq!(s.paras[0].notes[0].author, "Jo");
}

#[test]
fn marks_survive_split_and_join() {
    let mut s = script(&[(Element::Action, "A dog barks. A cat hisses.")]);
    s.add_tag(Pos::new(0, 2), Pos::new(0, 5), "Animals");
    s.add_tag(Pos::new(0, 15), Pos::new(0, 18), "Animals");
    s.add_note(Pos::new(0, 13), Pos::new(0, 26), "Second beat", Some(CardColor::Blue), "");
    let at = s.split(Pos::new(0, 13), Element::Action).unwrap_or_default();
    assert_eq!(at, Pos::new(1, 0));
    assert_eq!(s.paras[0].tagged.len(), 1);
    assert_eq!(s.paras[1].slice(s.paras[1].tagged[0].start, s.paras[1].tagged[0].end), "cat");
    assert_eq!(s.paras[1].notes[0].start, 0);
    // Join them back (backspace at the start of the second paragraph).
    s.delete_range(Pos::new(0, 13), Pos::new(1, 0));
    assert_eq!(s.paras.len(), 1);
    let found: Vec<&str> = s.paras[0].tagged.iter().map(|t| s.paras[0].slice(t.start, t.end)).collect();
    assert_eq!(found, ["dog", "cat"]);
    assert_eq!(s.paras[0].slice(s.paras[0].notes[0].start, s.paras[0].notes[0].end), "A cat hisses.");
    // A delete across paragraphs keeps the tail's marks.
    let mut s = script(&[(Element::Action, "One two"), (Element::Action, "three four")]);
    s.add_tag(Pos::new(1, 6), Pos::new(1, 10), "Props");
    s.delete_range(Pos::new(0, 3), Pos::new(1, 5));
    assert_eq!(s.paras[0].text, "One four");
    assert_eq!(s.paras[0].slice(s.paras[0].tagged[0].start, s.paras[0].tagged[0].end), "four");
}

#[test]
fn stale_offsets_are_clamped() {
    let mut s = script(&[(Element::Action, "héllo")]);
    s.paras[0].notes.push(Note::new(2, 999, "x", None, ""));
    s.paras[0].tagged.push(Tagged { start: 2, end: 999, category: "  set   dressing ".into() });
    s.paras[0].tagged.push(Tagged { start: 9, end: 9, category: "Props".into() });
    s.paras[0].tagged.push(Tagged { start: 0, end: 3, category: ",;".into() });
    s.normalize().ok();
    let p = &s.paras[0];
    assert_eq!((p.notes[0].start, p.notes[0].end), (1, 6), "onto char boundaries and the text");
    assert_eq!(p.tagged, vec![Tagged { start: 1, end: 6, category: "Set Dressing".into() }]);
    // Text changed behind the primitives' back (a heading retyped): still in range.
    s.paras[0].text = "hi".into();
    s.normalize().ok();
    assert!(s.paras[0].notes[0].end <= 2 && s.paras[0].tagged[0].end <= 2);
}

#[test]
fn alternates_cycle_and_choose() {
    let mut p = Para::new(Element::Dialogue, "Coffee?");
    p.alternates.push(Alternate::plain("Tea?"));
    p.alternates.push(Alternate::plain("Anything?"));
    assert!(p.cycle_alternates(true));
    assert_eq!(p.text, "Tea?");
    assert!(p.cycle_alternates(true));
    assert_eq!(p.text, "Anything?");
    assert!(p.cycle_alternates(false));
    assert!(p.cycle_alternates(false));
    assert_eq!(p.text, "Coffee?");
    let alts: Vec<&str> = p.alternates.iter().map(|a| a.text.as_str()).collect();
    assert_eq!(alts, ["Tea?", "Anything?"]);
    assert!(p.use_alternate(1));
    assert_eq!(p.text, "Anything?");
    assert!(!p.use_alternate(7));
    assert!(!Para::new(Element::Dialogue, "x").cycle_alternates(true));
}

#[test]
fn remove_tags_and_categories() {
    let mut s = script(&[(Element::Action, "A red car and a gun.")]);
    s.add_tag(Pos::new(0, 2), Pos::new(0, 9), "Vehicles");
    s.add_tag(Pos::new(0, 16), Pos::new(0, 19), "Props");
    s.add_tag(Pos::new(0, 16), Pos::new(0, 19), "Weapons");
    assert_eq!(s.tag_categories_used(), ["Vehicles", "Props", "Weapons"]);
    assert_eq!(s.remove_tags(Pos::new(0, 17), Pos::new(0, 17), Some("props")), 1);
    assert_eq!(s.remove_tags(Pos::new(0, 0), Pos::new(0, 20), None), 2);
    assert_eq!(s.add_tag(Pos::new(0, 0), Pos::new(0, 1), " ; "), 0);
    assert_eq!(category_rgb("Weapons"), category_rgb("Weapons"));
}
