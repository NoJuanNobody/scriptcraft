use serde_json::{Value, json};

use crate::{Session, catalog, cmd};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn hostile_params_never_panic() {
    let reg = cmd::registry();
    let junk = [
        json!(null),
        json!({}),
        json!({"text": 5, "para": 1e300, "off": -1, "index": "x", "element": 3, "from": 99999, "to": -5}),
        json!({"text": "\u{0}\n\r\t(é)", "para": 3, "off": 1, "index": 999, "from": 0, "to": 9999, "element": "nope", "anchor": {"para": 9e9}, "focus": 7, "fields": {"title": 5}}),
        json!({"text": "", "find": "", "replace": "", "page": 0, "summary": "s", "heading": "h", "number": "", "field": "bogus", "value": null}),
        json!({"xml": "<FinalDraft><Content><Paragraph", "path": 5, "extend": "yes", "count": 1e20}),
    ];
    for spec in reg.all() {
        // These reach outside the session (files).
        if spec.id.starts_with("file.") {
            continue;
        }
        for j in &junk {
            for start in [Session::sample(), Session::new(scriptcraft_doc::Script::new())] {
                let mut s = start;
                s.sel.focus = scriptcraft_doc::Pos::new(3, 2);
                let _ = s.run(spec.id, j);
                s.clamp_selection();
                let _ = s.layout();
                let _ = s.run("edit.undo", &json!({}));
                let _ = s.layout();
            }
        }
    }
}

#[test]
fn registry_ids_are_unique_and_catalog_commands_exist() {
    let reg = cmd::registry();
    let mut ids: Vec<&str> = reg.all().iter().map(|c| c.id).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate command ids");
    for f in catalog::FEATURES {
        for c in f.commands {
            assert!(c.starts_with("ui.") || reg.get(c).is_some(), "catalog names unknown command {c}");
        }
    }
    let (done, _, _) = catalog::counts();
    assert!(done >= 30, "parity floor");
    assert!(reg.by_shortcut("Mod+3").is_some_and(|c| c.id == "element.character"));
}

#[test]
fn sample_is_a_few_pages() {
    let mut s = Session::sample();
    let st = run(&mut s, "document.stats", json!({}));
    let pages = st["pages"].as_u64().unwrap();
    assert!((3..=6).contains(&pages), "{st}");
    assert_eq!(st["scenes"], 4);
    let l = s.layout();
    assert!(l.pages[0].title_page);
    assert!(s.script.paras.iter().any(|p| p.element == scriptcraft_doc::Element::Shot));
    assert!(s.script.paras.iter().any(|p| p.dual));
}

#[test]
fn sample_round_trips_through_fountain_and_fdx() {
    let s = Session::sample();
    let fdx = scriptcraft_fdx::write(&s.script);
    assert_eq!(scriptcraft_fdx::parse(&fdx).unwrap().paras, s.script.paras);
    let f = scriptcraft_fountain::write(&s.script);
    let back = scriptcraft_fountain::parse(&f);
    assert_eq!(back.paras.len(), s.script.paras.len());
    assert_eq!(scriptcraft_fountain::write(&back), f);
}

#[test]
fn reports() {
    let mut s = Session::sample();
    let c = run(&mut s, "report.characters", json!({}));
    assert_eq!(c[0]["name"], "NELL");
    assert!(c.as_array().unwrap().iter().any(|x| x["name"] == "TEO"));
    let sc = run(&mut s, "report.scenes", json!({}));
    assert_eq!(sc[0]["heading"], "EXT. GULL POINT WEATHER STATION - NIGHT");
    assert_eq!(sc[0]["page"], 1);
    assert!(sc[1]["characters"].as_array().unwrap().contains(&json!("NELL")));
    let loc = run(&mut s, "report.locations", json!({}));
    assert!(loc.as_array().unwrap().iter().any(|l| l["location"] == "WEATHER STATION - RADIO ROOM" || l["location"] == "WEATHER STATION"));
    assert_eq!(crate::reports::eighths_label(11), "1 3/8");
}

#[test]
fn scenes_cards_and_numbers() {
    let mut s = Session::sample();
    let l = run(&mut s, "scene.list", json!({}));
    assert_eq!(l["scenes"].as_array().unwrap().len(), 4);
    run(&mut s, "scene.setSummary", json!({"index": 0, "summary": "New summary"}));
    assert_eq!(s.script.scenes()[0].summary, "New summary");
    let first = s.script.scenes()[0].heading.clone();
    run(&mut s, "scene.move", json!({"from": 0, "to": 3}));
    assert_eq!(s.script.scenes()[3].heading, first);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.script.scenes()[0].heading, first);
    run(&mut s, "scene.numberAll", json!({}));
    assert_eq!(s.script.scenes()[2].number.as_deref(), Some("3"));
    let lay = s.layout();
    assert!(lay.pages.iter().flat_map(|p| &p.lines).any(|l| l.kind == scriptcraft_layout::LineKind::SceneNumber));
    run(&mut s, "scene.goto", json!({"index": 3}));
    assert_eq!(s.sel.focus.para, s.script.scenes()[3].para);
    run(&mut s, "scene.setColor", json!({"index": 1, "color": "pink"}));
    run(&mut s, "scene.setTags", json!({"index": 1, "tags": "radio, storm, radio, "}));
    let l = run(&mut s, "scene.list", json!({}));
    assert_eq!(l["scenes"][1]["color"], "pink");
    assert_eq!(l["scenes"][1]["tags"], json!(["radio", "storm"]));
    run(&mut s, "scene.setColor", json!({"index": 1, "color": null}));
    assert_eq!(s.script.scenes()[1].color, None);
    assert!(s.run("scene.setColor", &json!({"index": 1, "color": "plaid"})).is_err());
}

#[test]
fn find_and_replace() {
    let mut s = Session::sample();
    run(&mut s, "edit.find", json!({"text": "kettle"}));
    let (a, b) = s.sel.ordered();
    assert_eq!(s.script.text_between(a, b), "kettle");
    let r = run(&mut s, "edit.replaceAll", json!({"find": "KETTLE", "replace": "teapot"}));
    assert_eq!(r["count"], 2);
    assert!(s.script.paras.iter().all(|p| !p.text.to_lowercase().contains("kettle")));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.script.paras.iter().any(|p| p.text.contains("kettle")));
    assert_eq!(crate::cmd::edit::find_in("Straße STRASSE", "straße", 1, false), None);
    assert_eq!(crate::cmd::edit::find_in("aÉb", "é", 0, false), Some((1, 3)));
}

#[test]
fn clipboard_and_selection() {
    let mut s = Session::new(scriptcraft_doc::Script::new());
    run(&mut s, "text.insert", json!({"text": "int. a - day\nShe runs.\nLine two."}));
    assert_eq!(s.script.paras.len(), 3);
    assert_eq!(s.script.paras[1].element, scriptcraft_doc::Element::Action);
    run(&mut s, "select.text", json!({"text": "runs"}));
    let c = run(&mut s, "edit.cut", json!({}));
    assert_eq!(c["text"], "runs");
    assert_eq!(s.script.paras[1].text, "She .");
    run(&mut s, "edit.paste", json!({"text": "walks"}));
    assert_eq!(s.script.paras[1].text, "She walks.");
    run(&mut s, "select.all", json!({}));
    let c = run(&mut s, "edit.copy", json!({}));
    assert_eq!(c["text"], "INT. A - DAY\n\nShe walks.\n\nLine two.\n");
    assert_eq!(c["format"], "fountain");
}

#[test]
fn paste_keeps_elements() {
    use scriptcraft_doc::Element::*;
    let mut s = Session::sample();
    // Copy from the shot through part of NELL's first line (cue, parenthetical, dialogue).
    let shot = s.script.paras.iter().position(|p| p.element == Shot).unwrap();
    run(&mut s, "select.range", json!({"anchor": {"para": shot, "off": 0}, "focus": {"para": shot + 4, "off": 5}}));
    run(&mut s, "format.bold", json!({}));
    let c = run(&mut s, "edit.copy", json!({}));
    assert_eq!(c["paragraphs"], 5);
    let copied: Vec<_> = s.script.paras[shot..=shot + 4].to_vec();
    // Paste at the end of the script: everything comes back exactly (Shot and styles too).
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.enter", json!({}));
    let before = s.script.paras.len();
    run(&mut s, "edit.paste", json!({"text": c["text"]}));
    let n = s.script.paras.len();
    assert_eq!(n, before - 1 + 5, "the empty paragraph is replaced");
    let pasted = &s.script.paras[n - 5..];
    assert_eq!(pasted[0], copied[0]);
    assert_eq!(pasted[1..4], copied[1..4]);
    assert_eq!(pasted[4].text, copied[4].text[..5]);
    let els: Vec<_> = pasted.iter().map(|p| p.element).collect();
    assert_eq!(els, vec![Shot, Action, Character, Parenthetical, Dialogue]);
    assert!(!pasted[0].spans.is_empty());
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.script.paras.len(), before);
    // Fountain from elsewhere is parsed into elements.
    let mut t = Session::new(scriptcraft_doc::Script::new());
    run(&mut t, "edit.paste", json!({"text": "EXT. PIER - DAWN\n\nGulls wheel.\n\nSAM\n(quietly)\nNot again.\n"}));
    let els: Vec<_> = t.script.paras.iter().map(|p| p.element).collect();
    assert_eq!(els, vec![SceneHeading, Action, Character, Parenthetical, Dialogue]);
    assert_eq!(t.sel.focus, scriptcraft_doc::Pos::new(4, 10));
    // Plain lines without blank lines are typed as before.
    run(&mut t, "edit.paste", json!({"text": " Two\nlines"}));
    assert_eq!(t.script.paras.last().unwrap().text, "lines");
    // Pasting mid-paragraph splits it around the pasted paragraphs.
    run(&mut t, "caret.set", json!({"para": 1, "off": 5}));
    run(&mut t, "edit.paste", json!({"text": "ANA\nHi.\n\nShe waves.", "fountain": true}));
    let texts: Vec<_> = t.script.paras.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts[1..5], ["Gulls", "ANA", "Hi.", "She waves."]);
    assert_eq!(texts[5], " wheel.");
}

#[test]
fn caret_moves_by_line_through_the_layout() {
    let mut s = Session::sample();
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "caret.down", json!({}));
    assert_eq!(s.sel.focus.para, 1);
    run(&mut s, "caret.end", json!({}));
    run(&mut s, "caret.right", json!({"extend": true}));
    assert!(!s.sel.is_caret());
    run(&mut s, "caret.left", json!({}));
    assert!(s.sel.is_caret());
    run(&mut s, "caret.gotoPage", json!({"page": 2}));
    assert_eq!(s.layout().page_number_of(s.sel.focus.para), 2);
}

#[test]
fn dual_dialogue_toggle() {
    let mut s = Session::new(scriptcraft_doc::Script::new());
    run(&mut s, "text.insert", json!({"text": "x"}));
    for k in ["text.enter", "text.tab"] {
        run(&mut s, k, json!({}));
    }
    run(&mut s, "text.insert", json!({"text": "ana\nHi.\n"}));
    run(&mut s, "text.tab", json!({}));
    run(&mut s, "text.insert", json!({"text": "bo\nHey."}));
    run(&mut s, "dialogue.toggleDual", json!({}));
    let cue = s.script.paras.iter().find(|p| p.text == "BO").unwrap();
    assert!(cue.dual);
    run(&mut s, "caret.docStart", json!({}));
    assert!(s.run("dialogue.toggleDual", &json!({})).is_err());
}

#[test]
fn failed_commands_change_nothing() {
    let mut s = Session::sample();
    let before = s.script.clone();
    assert!(s.run("element.set", &json!({"element": "bogus"})).is_err());
    assert!(s.run("no.such", &json!({})).is_err());
    assert!(s.run("document.setFdx", &json!({"xml": "<html/>"})).is_err());
    assert_eq!(s.script, before);
    assert!(!s.can_undo());
}

#[test]
fn files_round_trip() {
    let dir = std::env::temp_dir().join(format!("scriptcraft-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::sample();
    for ext in ["fountain", "fdx"] {
        let p = dir.join(format!("night.{ext}"));
        run(&mut s, "file.saveAs", json!({"path": p.to_str().unwrap()}));
        assert!(!s.dirty);
        let mut t = Session::new(scriptcraft_doc::Script::new());
        run(&mut t, "file.open", json!({"path": p.to_str().unwrap()}));
        assert_eq!(t.script.scenes().len(), 4);
    }
    let pdf = dir.join("night.pdf");
    run(&mut s, "file.exportPdf", json!({"path": pdf.to_str().unwrap()}));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    assert!(s.run("file.exportPdf", &json!({"path": dir.join("x.fdx").to_str().unwrap()})).is_err());
    assert!(s.run("file.open", &json!({"path": dir.join("missing.fdx").to_str().unwrap()})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bold_italic_underline() {
    let mut s = Session::new(scriptcraft_doc::Script::new());
    run(&mut s, "text.insert", json!({"text": "x"}));
    run(&mut s, "text.enter", json!({}));
    run(&mut s, "text.insert", json!({"text": "She is "}));
    let st = run(&mut s, "format.bold", json!({}));
    assert_eq!(st["bold"], true);
    run(&mut s, "text.insert", json!({"text": "very"}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "text.insert", json!({"text": " late."}));
    let p = &s.script.paras[1];
    assert_eq!(p.text, "She is very late.");
    assert_eq!(p.spans, vec![scriptcraft_doc::Span { start: 7, end: 11, style: scriptcraft_doc::Style { bold: true, ..Default::default() } }]);
    run(&mut s, "select.text", json!({"text": "late"}));
    run(&mut s, "format.italic", json!({}));
    run(&mut s, "format.underline", json!({}));
    assert_eq!(run(&mut s, "format.state", json!({})), json!({"bold": false, "italic": true, "underline": true}));
    run(&mut s, "format.italic", json!({}));
    assert_eq!(run(&mut s, "format.state", json!({}))["italic"], false);
    run(&mut s, "select.all", json!({}));
    run(&mut s, "format.plain", json!({}));
    assert!(s.script.paras.iter().all(|p| p.spans.is_empty()));
    run(&mut s, "edit.undo", json!({}));
    assert!(!s.script.paras[1].spans.is_empty());
    // A caret move drops a pending style.
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "format.bold", json!({}));
    assert_eq!(run(&mut s, "format.state", json!({}))["bold"], true);
    run(&mut s, "caret.left", json!({}));
    assert_eq!(run(&mut s, "format.state", json!({}))["bold"], false);
}

#[test]
fn spelling() {
    let mut s = Session::sample();
    // Cast, places and title words are known; the sample's only unknowns are two real words.
    let r = run(&mut s, "spell.check", json!({}));
    let mut found: Vec<String> = r["misspellings"].as_array().unwrap().iter().map(|m| m["word"].as_str().unwrap().to_string()).collect();
    found.dedup();
    assert_eq!(found, vec!["dismasted", "Hesper"], "{r}");
    run(&mut s, "spell.addWord", json!({"word": "hesper"}));
    run(&mut s, "spell.ignore", json!({"word": "dismasted"}));
    assert_eq!(run(&mut s, "spell.check", json!({}))["count"], 0);
    run(&mut s, "select.text", json!({"text": "squat"}));
    run(&mut s, "text.insert", json!({"text": "sqaut"}));
    let r = run(&mut s, "spell.check", json!({}));
    assert_eq!(r["count"], 1);
    assert_eq!(r["misspellings"][0]["word"], "sqaut");
    run(&mut s, "caret.docStart", json!({}));
    let n = run(&mut s, "spell.next", json!({}));
    assert_eq!(n["word"], "sqaut");
    assert_eq!(n["suggestions"][0], "squat");
    let (a, b) = s.sel.ordered();
    assert_eq!(s.script.text_between(a, b), "sqaut");
    run(&mut s, "spell.ignore", json!({}));
    assert_eq!(run(&mut s, "spell.check", json!({}))["count"], 0);
    run(&mut s, "spell.addWord", json!({"word": "Glorp"}));
    assert!(s.user_words.contains("hesper"));
    assert!(s.user_words.contains("glorp"));
    assert!(s.run("spell.addWord", &json!({"word": "two words"})).is_err());
    assert_eq!(run(&mut s, "spell.suggest", json!({"word": "barometr"}))["suggestions"][0], "barometer");
}
