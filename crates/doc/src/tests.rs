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
