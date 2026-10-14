//! Final Draft-style typing, pinned (see docs/typing-parity.md). Keys go through the same
//! commands the keyboard uses.

use scriptcraft_doc::{Element, Script};
use serde_json::json;

use crate::Session;

/// Type a sequence: `⏎` Enter, `⇥` Tab, `⇤` Shift+Tab, `⌫` Backspace; anything else is text.
fn keys(s: &mut Session, seq: &[&str]) {
    for k in seq {
        let r = match *k {
            "⏎" => s.run("text.enter", &json!({})),
            "⇥" => s.run("text.tab", &json!({})),
            "⇤" => s.run("text.shiftTab", &json!({})),
            "⌫" => s.run("text.backspace", &json!({})),
            t => s.run("text.insert", &json!({"text": t})),
        };
        r.unwrap();
    }
}

fn els(s: &Session) -> Vec<(Element, String)> {
    s.script.paras.iter().map(|p| (p.element, p.text.clone())).collect()
}

fn blank() -> Session {
    Session::new(Script::new())
}

#[test]
fn writing_a_scene_with_enter_and_tab() {
    let mut s = blank();
    keys(
        &mut s,
        &["int. diner - night", "⏎", "Rain on the glass.", "⇥", "margo", "⏎", "Coffee?", "⇥", "beat", "⏎", "Black.", "⏎", "⇥", "dev", "⏎", "Sure."],
    );
    use Element::*;
    assert_eq!(
        els(&s),
        vec![
            (SceneHeading, "INT. DINER - NIGHT".into()),
            (Action, "Rain on the glass.".into()),
            (Character, "MARGO".into()),
            (Dialogue, "Coffee?".into()),
            (Parenthetical, "(beat)".into()),
            (Dialogue, "Black.".into()),
            (Character, "DEV".into()),
            (Dialogue, "Sure.".into()),
        ]
    );
}

#[test]
fn enter_flow_table() {
    use Element::*;
    for (from, text, to) in [
        (SceneHeading, "INT. A - DAY", Action),
        (Action, "Hi.", Action),
        (Character, "ANA", Dialogue),
        (Parenthetical, "(soft)", Dialogue),
        (Dialogue, "Hello.", Action),
        (Transition, "CUT TO:", SceneHeading),
        (Shot, "ANGLE", Action),
        (General, "x", General),
    ] {
        let mut s = Session::new(Script::from_paras(vec![scriptcraft_doc::Para::new(from, text)]).unwrap());
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("text.enter", &json!({})).unwrap();
        assert_eq!(s.script.paras[1].element, to, "Enter after {from}");
        assert_eq!(s.sel.focus.para, 1);
    }
}

#[test]
fn empty_paragraph_enter_and_tab_change_element() {
    use Element::*;
    let mut s = blank();
    keys(&mut s, &["x", "⏎"]); // heading, then an empty Action
    assert_eq!(s.element(), Action);
    keys(&mut s, &["⇥"]);
    assert_eq!(s.element(), Character, "empty Action + Tab = Character");
    keys(&mut s, &["⇥"]);
    assert_eq!(s.element(), Transition, "empty Character + Tab = Transition");
    keys(&mut s, &["⇥"]);
    assert_eq!(s.element(), SceneHeading);
    keys(&mut s, &["⏎"]);
    assert_eq!(s.element(), Action, "empty Scene Heading + Enter = Action");
    keys(&mut s, &["⏎"]);
    assert_eq!(s.element(), SceneHeading, "empty Action + Enter = Scene Heading");
    keys(&mut s, &["⇤"]);
    assert_eq!(s.element(), Transition);
    assert_eq!(s.script.paras.len(), 2, "no paragraphs were added");
}

#[test]
fn tab_from_dialogue_makes_a_parenthetical_with_parens() {
    let mut s = blank();
    keys(&mut s, &["⇥", "⇥"]); // heading → Action → Character
    keys(&mut s, &["ana", "⏎", "Wait.", "⇥"]);
    assert_eq!(s.element(), Element::Parenthetical);
    let p = s.sel.focus;
    assert_eq!(s.script.paras[p.para].text, "()");
    assert_eq!(p.off, 1);
    // Typing `(` and `)` over the automatic ones does not double them.
    keys(&mut s, &["(", "beat", ")"]);
    assert_eq!(s.script.paras[p.para].text, "(beat)");
    keys(&mut s, &["⏎"]);
    assert_eq!(s.element(), Element::Dialogue);
    // An empty parenthetical: Backspace clears the pair.
    keys(&mut s, &["⇥"]);
    keys(&mut s, &["⌫"]);
    assert_eq!(s.script.paras[s.sel.focus.para].text, "");
}

#[test]
fn enter_before_closing_paren_leaves_the_parenthetical() {
    let mut s = blank();
    keys(&mut s, &["⇥", "⇥", "ana", "⇥"]);
    assert_eq!(s.element(), Element::Parenthetical);
    keys(&mut s, &["quietly", "⏎"]);
    assert_eq!(s.element(), Element::Dialogue);
    assert_eq!(s.script.paras[s.sel.focus.para - 1].text, "(quietly)");
}

#[test]
fn capitals_for_headings_cues_and_transitions() {
    let mut s = blank();
    keys(&mut s, &["ext. pier - dawn", "⏎", "Gulls.", "⇥", "sam (v.o.)", "⏎", "Not again.", "⏎", "⇥", "⇥", "smash cut to:"]);
    assert_eq!(s.script.paras[0].text, "EXT. PIER - DAWN");
    assert_eq!(s.script.paras[1].text, "Gulls.");
    assert_eq!(s.script.paras[2].text, "SAM (V.O.)");
    assert_eq!(s.script.paras[3].text, "Not again.");
    assert_eq!(s.script.paras[4], scriptcraft_doc::Para::new(Element::Transition, "SMASH CUT TO:"));
}

#[test]
fn action_typed_as_heading_becomes_a_heading() {
    let mut s = blank();
    keys(&mut s, &["x", "⏎", "int. "]);
    assert_eq!(s.element(), Element::SceneHeading);
    keys(&mut s, &["kitchen"]);
    assert_eq!(s.script.paras[1].text, "INT. KITCHEN");
    // Not in the middle of ordinary action.
    keys(&mut s, &["⏎", "She walks int. the room"]);
    assert_eq!(s.element(), Element::Action);
}

#[test]
fn enter_at_start_and_middle() {
    let mut s = blank();
    keys(&mut s, &["x", "⏎", "Hello world."]);
    s.run("caret.set", &json!({"para": 1, "off": 0})).unwrap();
    keys(&mut s, &["⏎"]);
    assert_eq!(els(&s)[1], (Element::Action, String::new()), "empty paragraph inserted above");
    assert_eq!(s.sel.focus, scriptcraft_doc::Pos::new(2, 0));
    s.run("caret.set", &json!({"para": 2, "off": 5})).unwrap();
    keys(&mut s, &["⏎"]);
    assert_eq!(els(&s)[2], (Element::Action, "Hello".into()));
    assert_eq!(els(&s)[3], (Element::Action, " world.".into()), "the tail keeps its element");
}

#[test]
fn backspace_joins_paragraphs() {
    let mut s = blank();
    keys(&mut s, &["x", "⏎", "One.", "⏎", "Two."]);
    s.run("caret.set", &json!({"para": 2, "off": 0})).unwrap();
    keys(&mut s, &["⌫"]);
    assert_eq!(s.script.paras[1].text, "One.Two.");
    assert_eq!(s.sel.focus, scriptcraft_doc::Pos::new(1, 4));
}

#[test]
fn typing_is_one_undo_step() {
    let mut s = blank();
    keys(&mut s, &["i", "n", "t", ".", " ", "x"]);
    assert_eq!(s.script.paras[0].text, "INT. X");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.script.paras[0].text, "");
    s.run("edit.redo", &json!({})).unwrap();
    assert_eq!(s.script.paras[0].text, "INT. X");
    keys(&mut s, &["⏎", "a"]);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.script.paras.len(), 2);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.script.paras.len(), 1);
}

#[test]
fn smarttype_suggestions() {
    let mut s = Session::sample();
    s.run("caret.docEnd", &json!({})).unwrap();
    keys(&mut s, &["⏎", "⇥", "⇥", "n"]);
    let r = s.run("smarttype.suggest", &json!({})).unwrap();
    assert_eq!(r["items"], json!(["NELL"]));
    s.run("smarttype.accept", &json!({})).unwrap();
    assert_eq!(s.script.paras[s.sel.focus.para].text, "NELL");
    keys(&mut s, &[" ("]);
    let r = s.run("smarttype.suggest", &json!({})).unwrap();
    assert_eq!(r["items"], json!(["(V.O.)", "(O.S.)", "(O.C.)", "(CONT'D)"]));
    s.run("smarttype.accept", &json!({"index": 1})).unwrap();
    assert_eq!(s.script.paras[s.sel.focus.para].text, "NELL (O.S.)");
    // Scene headings: prefix, then location, then time.
    keys(&mut s, &["⏎", "Hi.", "⏎", "⏎"]);
    assert_eq!(s.element(), Element::SceneHeading);
    keys(&mut s, &["i"]);
    let r = s.run("smarttype.suggest", &json!({})).unwrap();
    assert_eq!(r["items"], json!(["INT. ", "INT./EXT. ", "I/E. "]));
    s.run("smarttype.accept", &json!({})).unwrap();
    keys(&mut s, &["we"]);
    let r = s.run("smarttype.suggest", &json!({})).unwrap();
    assert_eq!(r["items"], json!(["WEATHER STATION - RADIO ROOM"]));
    s.run("smarttype.accept", &json!({})).unwrap();
    keys(&mut s, &[" - n"]);
    let r = s.run("smarttype.suggest", &json!({})).unwrap();
    assert_eq!(r["items"], json!(["NIGHT"]));
    s.run("smarttype.accept", &json!({})).unwrap();
    assert_eq!(s.script.paras[s.sel.focus.para].text, "INT. WEATHER STATION - RADIO ROOM - NIGHT");
    assert!(s.run("smarttype.accept", &json!({})).is_err());
}

#[test]
fn element_settings_change_the_flow() {
    // #4: empty Action + Tab goes to Character by default (Final Draft's convention) and can be
    // set to Transition per script.
    let mut s = blank();
    keys(&mut s, &["x", "⏎", "⇥"]);
    assert_eq!(s.element(), Element::Character);
    s.run("edit.undo", &json!({})).unwrap();
    s.run("settings.element", &json!({"element": "action", "tabEmpty": "transition"})).unwrap();
    keys(&mut s, &["⇥"]);
    assert_eq!(s.element(), Element::Transition);
    // Enter targets and capitals too.
    s.run("settings.element", &json!({"element": "dialogue", "enter": "character"})).unwrap();
    s.run("settings.element", &json!({"element": "general", "uppercase": true})).unwrap();
    s.run("element.set", &json!({"element": "character"})).unwrap();
    keys(&mut s, &["ana", "⏎", "Hi.", "⏎"]);
    assert_eq!(s.element(), Element::Character);
    s.run("element.set", &json!({"element": "general"})).unwrap();
    keys(&mut s, &["shout"]);
    assert_eq!(s.script.paras[s.sel.focus.para].text, "SHOUT");
    assert!(s.run("settings.element", &json!({"element": "action", "tab": "nope"})).is_err());
    assert!(s.run("settings.element", &json!({"element": "action", "left": 1e30, "right": -1})).is_ok());
    s.run("settings.resetElements", &json!({})).unwrap();
    assert!(s.script.settings.elements.is_default());
}

#[test]
fn russian_heading_typing_and_locations() {
    let mut s = Session::new(Default::default());
    s.run("element.set", &json!({"element":"action"})).unwrap();
    s.run("text.insert", &json!({"text":"инт. "})).unwrap();
    s.run("text.insert", &json!({"text":"кухня - ночь"})).unwrap();
    assert_eq!(s.script.paras[0].element, Element::SceneHeading);
    assert_eq!(s.script.paras[0].text, "ИНТ. КУХНЯ - НОЧЬ");
    let locations = crate::reports::locations(&s.script);
    assert_eq!(locations[0].location, "КУХНЯ");
    assert!(locations[0].interior);
    assert!(!locations[0].exterior);
    s.run("text.enter", &json!({})).unwrap();
    s.run("text.insert", &json!({"text":"инт./нат. "})).unwrap();
    s.run("text.insert", &json!({"text":"двор - день"})).unwrap();
    assert_eq!(s.script.paras[1].element, Element::SceneHeading);
    let locations = crate::reports::locations(&s.script);
    assert!(locations[1].interior && locations[1].exterior);
}
