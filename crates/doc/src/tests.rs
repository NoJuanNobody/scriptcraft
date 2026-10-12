use super::*;

fn script() -> Script {
    Script::from_paras(vec![
        Para::new(Element::SceneHeading, "int. diner - night"),
        Para::new(Element::Action, "Rain on the glass."),
        Para::new(Element::Character, "margo (v.o.)"),
        Para::new(Element::Dialogue, "Coffee?"),
        Para::new(Element::SceneHeading, "EXT. PARKING LOT - CONTINUOUS"),
        Para::new(Element::Character, "MARGO"),
        Para::new(Element::Dialogue, "Keys."),
    ])
    .unwrap()
}

#[test]
fn uppercase_elements() {
    let s = script();
    assert_eq!(s.paras[0].text, "INT. DINER - NIGHT");
    assert_eq!(s.paras[2].text, "MARGO (V.O.)");
    assert_eq!(s.paras[1].text, "Rain on the glass.");
}

#[test]
fn insert_split_delete() {
    let mut s = script();
    let p = s.insert_text(Pos::new(1, 4), " hard").unwrap();
    assert_eq!(s.paras[1].text, "Rain hard on the glass.");
    assert_eq!(p, Pos::new(1, 9));
    let n = s.split(Pos::new(1, 9), Element::Action).unwrap();
    assert_eq!(n, Pos::new(2, 0));
    assert_eq!(s.paras[2].text, " on the glass.");
    let at = s.delete_range(Pos::new(2, 0), Pos::new(1, 9));
    assert_eq!(at, Pos::new(1, 9));
    assert_eq!(s.paras[1].text, "Rain hard on the glass.");
    // Across paragraphs: the first keeps its element.
    s.delete_range(Pos::new(0, 3), Pos::new(1, 4));
    assert_eq!(s.paras[0].text, "INT HARD ON THE GLASS.");
    assert_eq!(s.paras[0].element, Element::SceneHeading);
}

#[test]
fn hostile_positions_clamp() {
    let mut s = script();
    s.paras[1].text = "héllo".into();
    assert_eq!(s.clamp(Pos::new(1, 2)), Pos::new(1, 1));
    assert_eq!(s.clamp(Pos::new(999, 999)), Pos::new(6, 5));
    s.insert_text(Pos::new(usize::MAX, usize::MAX), "x\n\ty").unwrap();
    assert_eq!(s.paras[6].text, "Keys.x  y");
    s.delete_range(Pos::new(usize::MAX, 0), Pos::new(0, usize::MAX));
    let mut e = Script { paras: vec![], ..Script::new() };
    e.normalize().unwrap();
    assert_eq!(e.paras.len(), 1);
}

#[test]
fn scenes_characters_locations() {
    let s = script();
    let sc = s.scenes();
    assert_eq!(sc.len(), 2);
    assert_eq!((sc[0].para, sc[0].end), (0, 4));
    assert_eq!(s.characters(), vec!["MARGO"]);
    assert_eq!(s.locations(), vec!["DINER", "PARKING LOT"]);
    assert_eq!(s.scene_of(3), Some(0));
    assert_eq!(s.scene_of(6), Some(1));
    assert_eq!(heading_parts("INT./EXT. CAR - DAY"), ("INT./EXT.".into(), "CAR".into(), "DAY".into()));
}

#[test]
fn move_scenes() {
    let mut s = script();
    assert_eq!(s.move_scene(1, 0).unwrap(), 0);
    assert_eq!(s.paras[0].text, "EXT. PARKING LOT - CONTINUOUS");
    assert_eq!(s.paras[3].text, "INT. DINER - NIGHT");
    assert_eq!(s.move_scene(0, 1).unwrap(), 4);
    assert_eq!(s, script());
    assert!(s.move_scene(5, 0).is_err());
}

#[test]
fn words() {
    let t = "Hello, wide world";
    assert_eq!(word_right(t, 0), 5);
    assert_eq!(word_right(t, 5), 11);
    assert_eq!(word_left(t, 17), 12);
    assert_eq!(word_left(t, 12), 7);
    assert_eq!(prev_boundary("é", 2), 0);
    assert_eq!(next_boundary("é", 0), 2);
}

#[test]
fn json_round_trip() {
    let mut s = script();
    s.paras[0].summary = "Margo waits.".into();
    s.title.title = "Night Shift".into();
    let j = serde_json::to_string(&s).unwrap();
    let back: Script = serde_json::from_str(&j).unwrap();
    assert_eq!(back, s);
}

fn bold() -> Style {
    Style { bold: true, ..Style::PLAIN }
}

#[test]
fn styles_survive_edits() {
    let mut s = Script::from_paras(vec![Para::new(Element::Action, "Hello world")]).unwrap();
    s.set_flag(Pos::new(0, 6), Pos::new(0, 11), Flag::Bold, true);
    assert_eq!(s.paras[0].spans, vec![Span { start: 6, end: 11, style: bold() }]);
    assert!(s.all_have(Pos::new(0, 6), Pos::new(0, 11), Flag::Bold));
    assert!(!s.all_have(Pos::new(0, 0), Pos::new(0, 11), Flag::Bold));
    // Typing inside or right after a bold word continues it; before it does not.
    s.insert_text(Pos::new(0, 11), "!").unwrap();
    assert_eq!(s.paras[0].spans[0].end, 12);
    s.insert_text(Pos::new(0, 0), ">> ").unwrap();
    assert_eq!(s.paras[0].spans[0], Span { start: 9, end: 15, style: bold() });
    // Split and join keep them.
    s.split(Pos::new(0, 11), Element::Action).unwrap();
    assert_eq!(s.paras[0].spans, vec![Span { start: 9, end: 11, style: bold() }]);
    assert_eq!(s.paras[1].spans, vec![Span { start: 0, end: 4, style: bold() }]);
    s.delete_range(Pos::new(0, 11), Pos::new(1, 0));
    assert_eq!(s.paras[0].text, ">> Hello world!");
    assert_eq!(s.paras[0].spans, vec![Span { start: 9, end: 15, style: bold() }]);
    // Deleting part of the run shrinks it.
    s.delete_range(Pos::new(0, 10), Pos::new(0, 12));
    assert_eq!(s.paras[0].spans, vec![Span { start: 9, end: 13, style: bold() }]);
    assert_eq!(s.paras[0].runs(), vec![(">> Hello ", Style::PLAIN), ("wld!", bold())]);
}

#[test]
fn capitals_keep_styles() {
    let mut p = Para::styled(Element::Action, &[("straße ".into(), Style::PLAIN), ("noir".into(), bold())]);
    assert_eq!(p.spans, vec![Span { start: 8, end: 12, style: bold() }]);
    let mut s = Script::from_paras(vec![p.clone()]).unwrap();
    s.set_element(0, Element::SceneHeading).unwrap();
    assert_eq!(s.paras[0].text, "STRASSE NOIR");
    assert_eq!(s.paras[0].spans, vec![Span { start: 8, end: 12, style: bold() }]);
    // Hostile spans are clamped.
    p.spans = vec![Span { start: 99, end: 2, style: bold() }, Span { start: 1, end: 3, style: bold() }];
    let s = Script::from_paras(vec![p]).unwrap();
    assert_eq!(s.paras[0].spans, vec![Span { start: 1, end: 3, style: bold() }]);
}
