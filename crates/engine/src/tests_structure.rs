//! Structure points, the outline and the story map through commands.

use serde_json::json;

use crate::Session;

fn titles(s: &mut Session) -> Vec<String> {
    let v = s.run("outline.list", &json!({})).unwrap_or_default();
    v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| if i["type"] == "point" { format!("[{}]", i["title"].as_str().unwrap_or("")) } else { format!("{}", i["index"]) })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn structure_commands() {
    let mut s = Session::sample();
    let n = s.script.scenes().len();
    assert!(n >= 4);
    // Default: before the caret's scene.
    s.run("scene.goto", &json!({"index": 0})).unwrap();
    assert_eq!(s.run("structure.add", &json!({"kind": "act", "title": "Act One", "color": "blue"})).unwrap()["index"], 0);
    s.run("structure.add", &json!({"kind": "act", "title": "Act Two", "before": 2})).unwrap();
    s.run("structure.add", &json!({"kind": "sequence", "title": "The Call", "before": 1, "summary": "Mayday."})).unwrap();
    s.run("structure.add", &json!({"title": "Act Three", "before": "end"})).unwrap();
    assert_eq!(titles(&mut s), ["[Act One]", "0", "[The Call]", "1", "[Act Two]", "2", "3", "[Act Three]"]);
    let l = s.run("structure.list", &json!({})).unwrap();
    assert_eq!(l["points"][1]["kind"], "sequence");
    assert_eq!(l["points"][1]["summary"], "Mayday.");
    assert_eq!(l["points"][0]["color"], "blue");
    assert_eq!(l["points"][3]["scene"], serde_json::Value::Null);
    // Outline depths: scenes sit one deeper than the point above them.
    let o = s.run("outline.list", &json!({})).unwrap();
    assert_eq!(o["items"][3]["depth"], 2);
    assert_eq!(o["items"][2]["depth"], 1);
    // Edit, go to, delete, undo.
    s.run("structure.edit", &json!({"index": 1, "title": "Mayday", "kind": "section", "color": null})).unwrap();
    assert_eq!(s.script.structure_point(1).map(|p| p.title.as_str()), Some("Mayday"));
    s.run("structure.goto", &json!({"index": 2})).unwrap();
    assert_eq!(s.script.scene_of(s.sel.focus.para), Some(2));
    s.run("structure.delete", &json!({"index": 1})).unwrap();
    assert_eq!(s.script.structure_count(), 3);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.script.structure_count(), 4);
    // Bad params.
    assert!(s.run("structure.add", &json!({"kind": "chapter"})).is_err());
    assert!(s.run("structure.add", &json!({"before": 999})).is_err());
    assert!(s.run("structure.edit", &json!({"index": 99})).is_err());
    assert!(s.run("structure.edit", &json!({"index": 0, "color": "plaid"})).is_err());
}

#[test]
fn outline_move_and_card_moves() {
    let mut s = Session::sample();
    s.run("structure.add", &json!({"title": "One", "before": 0})).unwrap();
    s.run("structure.add", &json!({"title": "Two", "before": 2})).unwrap();
    assert_eq!(titles(&mut s), ["[One]", "0", "1", "[Two]", "2", "3"]);
    let heading3 = s.script.scenes()[3].heading.clone();
    // Drag the last scene under act one.
    s.run("outline.move", &json!({"from": 5, "to": 1})).unwrap();
    assert_eq!(s.script.scenes()[0].heading, heading3);
    assert_eq!(s.script.scene_of(s.sel.focus.para), Some(0), "the caret follows the scene");
    // Drag act two up: scene 1 joins it.
    s.run("outline.move", &json!({"from": 4, "to": 2})).unwrap();
    assert_eq!(titles(&mut s), ["[One]", "0", "[Two]", "1", "2", "3"]);
    s.run("edit.undo", &json!({})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(titles(&mut s), ["[One]", "0", "1", "[Two]", "2", "3"]);
    // Index cards: moving act two's first scene to the top keeps act two where it was.
    s.run("scene.move", &json!({"from": 2, "to": 0})).unwrap();
    assert_eq!(titles(&mut s), ["[One]", "0", "1", "2", "[Two]", "3"]);
    assert!(s.run("outline.move", &json!({"from": 99, "to": 0})).is_err());
    // Words are all still there.
    let before = Session::sample().script.word_count();
    assert_eq!(s.script.word_count(), before);
}

#[test]
fn story_map_scales_to_pages() {
    let mut s = Session::sample();
    s.run("structure.add", &json!({"title": "One", "before": 0})).unwrap();
    s.run("structure.add", &json!({"title": "Two", "before": 2})).unwrap();
    s.run("structure.add", &json!({"kind": "sequence", "title": "Inside", "before": 3})).unwrap();
    let m = s.run("storymap.get", &json!({})).unwrap();
    let pages = m["pages"].as_f64().unwrap();
    assert_eq!(pages as usize, s.layout().script_pages());
    let scenes = m["scenes"].as_array().unwrap();
    let mut last = -1.0;
    for sc in scenes {
        let (a, b) = (sc["start"].as_f64().unwrap(), sc["end"].as_f64().unwrap());
        assert!(a >= last && b >= a && b <= pages + 1e-3, "{sc}");
        last = a;
    }
    // Scene 2 starts on page 3 of the sample.
    assert_eq!(scenes[2]["start"].as_f64().unwrap().floor() as usize + 1, s.layout().page_number_of(s.script.scenes()[2].para));
    let pts = m["points"].as_array().unwrap();
    assert_eq!(pts[0]["at"].as_f64(), scenes[0]["start"].as_f64());
    // Act One spans to Act Two; the sequence inside act two ends with the script.
    assert_eq!(pts[0]["end"].as_f64(), pts[1]["at"].as_f64());
    assert!((pts[2]["end"].as_f64().unwrap() - pages).abs() < 1e-3);
}

#[test]
fn structure_survives_fdx_and_fountain_through_the_session() {
    let mut s = Session::sample();
    s.run("structure.add", &json!({"title": "One", "before": 0, "summary": "Storm."})).unwrap();
    s.run("structure.add", &json!({"title": "Two", "before": 2, "color": "pink"})).unwrap();
    let want = titles(&mut s);
    for fmt in ["fdx", "fountain"] {
        let text = if fmt == "fdx" { scriptcraft_fdx::write(&s.script) } else { scriptcraft_fountain::write(&s.script) };
        let script = if fmt == "fdx" { scriptcraft_fdx::parse(&text).unwrap() } else { scriptcraft_fountain::parse(&text) };
        let mut back = Session::new(script);
        assert_eq!(titles(&mut back), want, "{fmt}");
        assert_eq!(back.script.structure_point(1).and_then(|p| p.color).map(|c| c.id()), Some("pink"), "{fmt}");
        assert_eq!(back.script.structure_point(0).map(|p| p.summary.as_str()), Some("Storm."), "{fmt}");
    }
}
