use super::*;
use scriptcraft_doc::{Flag, Pos};

fn annotated() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. CAFÉ - NIGHT"),
        Para::new(Element::Action, "Ada sets down a brass key & a café menu."),
        Para::new(Element::Character, "ADA"),
        Para::new(Element::Dialogue, "Is it open?"),
    ])
    .unwrap_or_default();
    // "brass key" and "café menu" (a multi-byte char inside a range).
    s.add_tag(Pos::new(1, 16), Pos::new(1, 25), "Props");
    s.add_tag(Pos::new(1, 29), Pos::new(1, 40), "Set Dressing");
    s.add_tag(Pos::new(1, 0), Pos::new(1, 3), "Cast");
    s.add_note(Pos::new(1, 4), Pos::new(1, 8), "Gentler: \"places\"?\nOr cut.", Some(CardColor::Blue), "Jo <editor>");
    s.add_note(Pos::new(0, 0), Pos::new(0, 0), "Check the café exists", None, "");
    s.paras[3].alternates.push(scriptcraft_doc::Alternate::plain("Are you open?"));
    s.paras[3].alternates.push(scriptcraft_doc::Alternate::plain("Open?"));
    s.set_flag(Pos::new(3, 0), Pos::new(3, 2), Flag::Italic, true);
    s
}

#[test]
fn notes_tags_and_alternates_round_trip() {
    let s = annotated();
    let xml = write(&s);
    assert!(xml.contains("<ScriptNote Range=\"4,4\""), "{xml}");
    assert!(xml.contains("<ScriptCraftTag Range=\"30,9\" Category=\"Set Dressing\"/>"), "{xml}");
    let back = parse(&xml).unwrap_or_default();
    for (a, b) in s.paras.iter().zip(&back.paras) {
        assert_eq!(a.text, b.text);
        assert_eq!(a.notes, b.notes);
        assert_eq!(a.tagged, b.tagged);
        assert_eq!(a.alternates, b.alternates);
        assert_eq!(a.spans, b.spans);
    }
    assert_eq!(back.paras[1].slice(back.paras[1].tagged[2].start, back.paras[1].tagged[2].end), "café menu");
    assert_eq!(back.paras[1].notes[0].text, "Gentler: \"places\"?\nOr cut.");
    assert_eq!(back.paras[1].notes[0].author, "Jo <editor>");
    // Only the active line is script text: alternates never leak into the paragraph.
    assert_eq!(back.paras[3].text, "Is it open?");
    assert_eq!(back.paras.len(), s.paras.len());
}

#[test]
fn script_notes_from_other_writers() {
    // A note with no range or colour, a white marker, and a range past the end of the text.
    let xml = r##"<FinalDraft DocumentType="Script" Version="5"><Content>
      <Paragraph Type="Action"><Text>Short.</Text>
        <ScriptNote MarkerColor="#FFFFFFFFFFFF" Writer="Sam"><Paragraph><Text>First</Text></Paragraph><Paragraph><Text>Second</Text></Paragraph></ScriptNote>
        <ScriptNote Range="3,999"><Paragraph><Text>Long range</Text></Paragraph></ScriptNote>
        <ScriptNote Range="x,y"/>
        <ScriptCraftTag Range="0,999" Category="  props "/>
        <ScriptCraftTag Range="0,3"/>
        <ScriptCraftAlternate/>
      </Paragraph>
    </Content></FinalDraft>"##;
    let s = parse(xml).unwrap_or_default();
    let p = &s.paras[0];
    assert_eq!(p.text, "Short.");
    assert_eq!(p.notes.len(), 2, "empty notes are dropped");
    assert_eq!(p.notes[0].text, "First\nSecond");
    assert_eq!(p.notes[0].author, "Sam");
    assert_eq!(p.notes[0].color, CardColor::Yellow);
    assert_eq!((p.notes[1].start, p.notes[1].end), (3, 6));
    assert_eq!(p.tagged, vec![Tagged { start: 0, end: 6, category: "Props".into() }]);
    assert!(p.alternates.is_empty());
    // Note paragraphs never become script paragraphs.
    assert_eq!(s.paras.len(), 1);
}

/// v0.3 features together: locks, an omitted scene, notes, tags and alternates in one file.
#[test]
fn production_and_annotations_round_trip_together() {
    let mut s = annotated();
    s.paras.push(Para::new(Element::SceneHeading, "EXT. ROOF - DAWN"));
    s.paras.push(Para::new(Element::Action, "Omitted, but the words stay."));
    s.settings.pages_locked = true;
    s.settings.scene_numbers_locked = true;
    s.paras[0].scene_number = Some("4".into());
    s.paras[4].scene_number = Some("4A".into());
    s.paras[4].omitted = true;
    s.paras[0].page_lock = vec!["2".into()];
    s.add_note(Pos::new(5, 0), Pos::new(5, 7), "Restore for the director's cut?", Some(CardColor::Pink), "Ana");
    s.add_tag(Pos::new(4, 0), Pos::new(4, 4), "Location");
    let back = parse(&write(&s)).unwrap_or_default();
    assert_eq!(back.paras, s.paras);
    assert!(back.settings.pages_locked && back.settings.scene_numbers_locked);
}
