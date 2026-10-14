use super::*;
use scriptcraft_doc::Style;

fn sample() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::Transition, "FADE IN:"),
        Para::new(Element::SceneHeading, "INT. DINER - NIGHT"),
        Para::new(Element::Action, "Rain & neon. <Flicker>. \"Quotes\" and 'apostrophes'."),
        Para::new(Element::Character, "MARGO"),
        Para::new(Element::Parenthetical, "(quietly)"),
        Para::new(Element::Dialogue, "Not tonight."),
        Para::new(Element::Character, "DEV"),
        Para::new(Element::Dialogue, "Then when?"),
        Para::new(Element::Shot, "CLOSE ON THE CLOCK"),
        Para::new(Element::General, "General text"),
        Para::new(Element::Transition, "CUT TO:"),
    ])
    .unwrap();
    s.paras[1].scene_number = Some("1".into());
    s.paras[1].summary = "Margo waits.".into();
    s.paras[1].color = Some(scriptcraft_doc::CardColor::Teal);
    s.paras[1].tags = vec!["opening".into(), "rain".into()];
    s.paras[6].dual = true;
    s.set_flag(scriptcraft_doc::Pos::new(2, 0), scriptcraft_doc::Pos::new(2, 4), scriptcraft_doc::Flag::Bold, true);
    s.set_flag(scriptcraft_doc::Pos::new(2, 2), scriptcraft_doc::Pos::new(2, 9), scriptcraft_doc::Flag::Underline, true);
    let mut f = s.settings.elements.get(Element::Action);
    f.left = 1.75;
    f.before = 2;
    f.enter = Element::Character;
    s.settings.elements.set(Element::Action, f);
    let mut t = s.settings.elements.get(Element::Transition);
    t.align = scriptcraft_doc::Align::Center;
    t.uppercase = false;
    s.settings.elements.set(Element::Transition, t);
    s.title.title = "The Long Night".into();
    s.title.credit = "Written by".into();
    s.title.author = "Ada Lovelace".into();
    s.title.source = "Based on a true story".into();
    s.title.contact = "ada@example.com\n555-0100".into();
    s.title.draft_date = "1 May 2026".into();
    s
}

#[test]
fn round_trip() {
    let s = sample();
    let x = write(&s);
    assert!(x.contains("<DualDialogue>"), "{x}");
    assert!(x.contains("Color=\"#2F2F9E9E9696\""), "{x}");
    assert_eq!(parse_color("#FFFFFFFFFFFF"), None);
    assert_eq!(parse_color("#E04F4F"), Some(scriptcraft_doc::CardColor::Red));
    assert_eq!(parse_color("nonsense"), None);
    assert!(x.contains("<Paragraph Type=\"Scene Heading\" Number=\"1\">"), "{x}");
    let back = parse(&x).unwrap();
    assert_eq!(back, s, "\n{x}");
    assert_eq!(write(&back), x);
}

#[test]
fn reads_observed_structure() {
    let x = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="4">
  <Content>
    <Paragraph Type="Scene Heading" Number="4A">
      <SceneProperties Length="1/8" Page="1" Title="">
        <Summary><Paragraph Alignment="Left"><Text>First </Text><Text>part.</Text></Paragraph></Summary>
      </SceneProperties>
      <Text>ext. pier - dawn</Text>
    </Paragraph>
    <Paragraph Type="Action"><Text Style="Bold">Waves</Text><Text> crash &amp; &#233;cho.</Text></Paragraph>
    <Paragraph Type="New Act"><Text>ACT TWO</Text></Paragraph>
    <Paragraph Type="Action"><Text/></Paragraph>
  </Content>
  <TitlePage><Content>
    <Paragraph Alignment="Center"><Text>PIER</Text></Paragraph>
    <Paragraph Alignment="Center"><Text>by</Text></Paragraph>
    <Paragraph Alignment="Center"><Text>Sam Q</Text></Paragraph>
    <Paragraph Alignment="Left"><Text>sam@example.com</Text></Paragraph>
  </Content></TitlePage>
</FinalDraft>"#;
    let s = parse(x).unwrap();
    assert_eq!(s.paras[0].text, "EXT. PIER - DAWN");
    assert_eq!(s.paras[0].scene_number.as_deref(), Some("4A"));
    assert_eq!(s.paras[0].summary, "First part.");
    assert_eq!(s.paras[1].text, "Waves crash & écho.");
    assert_eq!(s.paras[1].spans, vec![scriptcraft_doc::Span { start: 0, end: 5, style: Style { bold: true, ..Style::PLAIN } }]);
    assert_eq!(s.paras[2].element, Element::General);
    assert_eq!(s.paras[3].text, "");
    assert_eq!(s.title.title, "PIER");
    assert_eq!(s.title.credit, "by");
    assert_eq!(s.title.author, "Sam Q");
    assert_eq!(s.title.contact, "sam@example.com");
}

#[test]
fn hostile_input() {
    assert!(matches!(parse("<html/>"), Err(FdxError::NotFdx(_))));
    assert!(parse("").is_err());
    let _ = parse("<FinalDraft><Content><Paragraph Type=\"Action\"><Text>unclosed");
    let deep = format!("<FinalDraft>{}{}</FinalDraft>", "<a>".repeat(1000), "</a>".repeat(1000));
    let _ = parse(&deep);
    let s = parse("<FinalDraft><Content><Paragraph Type=\"Bogus\"><Text>x</Text></Paragraph></Content></FinalDraft>").unwrap();
    assert_eq!(s.paras[0].element, Element::General);
    let s = parse("<FinalDraft/>").unwrap();
    assert_eq!(s.paras.len(), 1);
}

#[test]
fn production_locks_round_trip() {
    let mut s = Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. A - DAY"),
        Para::new(Element::Action, "One."),
        Para::new(Element::SceneHeading, "INT. B - DAY"),
        Para::new(Element::Action, "Hidden text stays in the file."),
        Para::new(Element::SceneHeading, "INT. C - DAY"),
        Para::new(Element::Action, "Two."),
    ])
    .unwrap();
    s.settings.pages_locked = true;
    s.settings.scene_numbers_locked = true;
    s.paras[0].scene_number = Some("12".into());
    s.paras[2].scene_number = Some("12A".into());
    s.paras[4].scene_number = Some("13".into());
    s.paras[2].omitted = true;
    s.paras[0].page_lock = vec!["7".into(), "7A".into()];
    s.paras[5].page_lock = vec!["8".into()];
    let xml = write(&s);
    assert!(xml.contains("ScriptCraftOmitted=\"Yes\"") && xml.contains("ScriptCraftLockedPages=\"7,7A\""));
    assert!(xml.contains("<ScriptCraftProduction LockedPages=\"Yes\" LockedSceneNumbers=\"Yes\"/>"));
    let back = parse(&xml).unwrap();
    assert_eq!(back.paras, s.paras);
    assert!(back.settings.pages_locked && back.settings.scene_numbers_locked);
    // Without the production element, locks are off (and stray page locks are dropped).
    let plain = xml.replace("<ScriptCraftProduction LockedPages=\"Yes\" LockedSceneNumbers=\"Yes\"/>", "");
    let back = parse(&plain).unwrap();
    assert!(!back.settings.pages_locked && back.paras.iter().all(|p| p.page_lock.is_empty()));
    assert!(back.paras[2].omitted);
    // Hostile values are cleaned.
    let x = "<FinalDraft><Content><Paragraph Type=\"Action\" ScriptCraftLockedPages=\"&lt;x&gt;,,1-2,  \" ScriptCraftOmitted=\"Yes\"><Text>a</Text></Paragraph></Content><ScriptCraftProduction LockedPages=\"Yes\"/></FinalDraft>";
    let back = parse(x).unwrap();
    assert_eq!(back.paras[0].page_lock, vec!["x".to_string(), "1-2".to_string()]);
    assert!(!back.paras[0].omitted, "only headings are omitted");
}

#[test]
fn template_formats_round_trip() {
    let (s, _) = scriptcraft_doc::Template::TvMultiCamera.script(None, None);
    let xml = write(&s);
    assert!(xml.contains("Spacing=\"2\"") && xml.contains("StartsNewPage=\"Yes\"") && xml.contains("<ScriptCraftFormat SceneLetters=\"Yes\"/>"));
    let back = parse(&xml).unwrap();
    assert_eq!(back.settings.elements, s.settings.elements);
    assert!(back.settings.scene_letters);
    assert_eq!(back.paras, s.paras);
}

#[test]
fn tracked_changes_round_trip() {
    use scriptcraft_doc::Pos;
    use scriptcraft_doc::track::{self, ChangeKind};
    let old = Script::from_paras(vec![
        Para::new(Element::Action, "Café one & two."),
        Para::new(Element::Character, "ANA"),
        Para::new(Element::Dialogue, "Hi \"there\"."),
    ])
    .unwrap();
    let mut new = old.clone();
    new.insert_text(Pos::new(0, 6), "<big> ").unwrap();
    new.delete_range(Pos::new(0, 15), Pos::new(2, 0));
    new.set_element(0, Element::General).unwrap();
    track::follow(&old, &mut new, Some("Jo \"J\""));
    new.settings.track.on = true;
    let kinds: Vec<ChangeKind> = track::list(&new).iter().map(|c| c.kind).collect();
    assert!(kinds.contains(&ChangeKind::Insert) && kinds.contains(&ChangeKind::Delete) && kinds.contains(&ChangeKind::Element), "{kinds:?}");
    let xml = write(&new);
    assert!(xml.contains("<ScriptCraftTrackChanges On=\"Yes\"/>") && xml.contains("&#10;"), "{xml}");
    let back = parse(&xml).unwrap();
    assert!(back.settings.track.on);
    assert_eq!(back.paras, new.paras);
    // Rejecting everything in the reopened file gives the original back.
    let mut r = back;
    track::resolve_all(&mut r, false, None);
    assert_eq!(r.paras, old.paras);
    // Hostile values.
    let x = "<FinalDraft><Content><Paragraph Type=\"Action\"><Text>abc</Text><ScriptCraftChange Kind=\"Insert\" Range=\"2,999\" Author=\"\"/><ScriptCraftChange Kind=\"Delete\" At=\"99\" Text=\"a&#10;b\" Elements=\"Nope\"/><ScriptCraftChange Kind=\"Wat\"/><ScriptCraftChange Kind=\"Element\" From=\"x\"/></Paragraph></Content></FinalDraft>";
    let back = parse(x).unwrap();
    assert_eq!(track::list(&back).len(), 2);
}
