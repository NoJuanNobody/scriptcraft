//! Script notes, alternate lines and production tagging through commands.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    match s.run(id, &v) {
        Ok(r) => r,
        Err(e) => panic!("{id} {v}: {e}"),
    }
}

fn session(fountain: &str) -> Session {
    Session::new(scriptcraft_fountain::parse(fountain))
}

const SCRIPT: &str = "INT. GARAGE - NIGHT\n\nAda wipes down a red motorbike. A wrench hangs from her belt.\n\nADA\nStart it.\n\nEXT. ROAD - DAWN\n\nThe red motorbike roars past a police car.\n\nBEN\nWas that her?\n";

#[test]
fn notes_through_commands() {
    let mut s = session(SCRIPT);
    // Select "red motorbike" (para 1, bytes 16..29) and add a note.
    run(&mut s, "select.range", json!({"anchor": {"para": 1, "off": 17}, "focus": {"para": 1, "off": 30}}));
    let r = run(&mut s, "note.add", json!({"text": "Make it a vintage one", "color": "blue", "author": "Jo"}));
    assert_eq!(r["index"], 0);
    run(&mut s, "note.add", json!({"text": "Night or dawn?", "anchor": {"para": 4, "off": 0}}));
    let l = run(&mut s, "note.list", json!({}));
    assert_eq!(l["count"], 2);
    assert_eq!(l["notes"][0]["quote"], "red motorbike");
    assert_eq!(l["notes"][0]["color"], "blue");
    assert_eq!(l["notes"][1]["scene"], 2);
    // Typing before the note keeps it on the same words.
    run(&mut s, "caret.set", json!({"para": 1, "off": 0}));
    run(&mut s, "text.insert", json!({"text": "Slowly, "}));
    assert_eq!(run(&mut s, "note.list", json!({}))["notes"][0]["quote"], "red motorbike");
    // Enter in the middle of the noted words: the note stays with its start.
    run(&mut s, "caret.set", json!({"para": 1, "off": 31}));
    run(&mut s, "text.enter", json!({}));
    assert_eq!(run(&mut s, "note.list", json!({}))["notes"][0]["quote"], "red mo");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "note.list", json!({}))["notes"][0]["quote"], "red motorbike");
    // Navigation selects the note's words.
    run(&mut s, "caret.set", json!({"para": 0, "off": 0}));
    let n = run(&mut s, "note.next", json!({}));
    assert_eq!(n["index"], 0);
    assert_eq!(s.script.text_between(s.sel.anchor, s.sel.focus), "red motorbike");
    assert_eq!(run(&mut s, "note.next", json!({}))["index"], 1);
    assert_eq!(run(&mut s, "note.next", json!({}))["index"], 0, "wraps");
    assert_eq!(run(&mut s, "note.previous", json!({}))["index"], 1, "wraps back");
    run(&mut s, "note.edit", json!({"index": 1, "text": "Dawn.", "color": "green"}));
    assert_eq!(run(&mut s, "note.list", json!({}))["notes"][1]["text"], "Dawn.");
    assert!(s.run("note.edit", &json!({"index": 1, "text": "  "})).is_err());
    assert!(s.run("note.add", &json!({"text": ""})).is_err());
    assert!(s.run("note.add", &json!({"text": "x", "color": "plaid"})).is_err());
    run(&mut s, "note.delete", json!({"index": 0}));
    assert_eq!(run(&mut s, "note.list", json!({}))["count"], 1);
    run(&mut s, "note.deleteAll", json!({}));
    assert!(s.run("note.next", &json!({})).is_err(), "disabled without notes");
    // Notes are in the saved FDX.
    run(&mut s, "edit.undo", json!({}));
    let xml = run(&mut s, "document.fdx", json!({}))["xml"].as_str().unwrap_or("").to_string();
    assert!(xml.contains("<ScriptNote"), "{xml}");
    let inspect = run(&mut s, "document.inspect", json!({}));
    assert_eq!(inspect["paragraphs"][4]["notes"], 1);
}

#[test]
fn alternates_only_the_active_line_prints() {
    let mut s = session(SCRIPT);
    run(&mut s, "caret.set", json!({"para": 3, "off": 0}));
    assert_eq!(s.element(), scriptcraft_doc::Element::Dialogue);
    run(&mut s, "alt.add", json!({"text": "Kick it over."}));
    let r = run(&mut s, "alt.add", json!({"text": "Fire it up.", "use": true}));
    assert_eq!(r["active"], "Fire it up.");
    assert_eq!(r["alternates"], json!(["Kick it over.", "Start it."]));
    assert_eq!(run(&mut s, "alt.next", json!({}))["active"], "Kick it over.");
    assert_eq!(run(&mut s, "alt.previous", json!({}))["active"], "Fire it up.");
    let r = run(&mut s, "alt.choose", json!({"index": 1}));
    assert_eq!(r["active"], "Start it.");
    // Only the active line is laid out, written to Fountain and printed.
    let layout = run(&mut s, "document.layout", json!({}));
    let text = layout.to_string();
    assert!(text.contains("Start it.") && !text.contains("Kick it over.") && !text.contains("Fire it up."));
    let fountain = run(&mut s, "document.fountain", json!({}))["text"].as_str().unwrap_or("").to_string();
    assert!(fountain.contains("Start it.") && !fountain.contains("Kick it over."), "{fountain}");
    let pdf = crate::io::encode(&s.script, crate::io::Format::Pdf).unwrap_or_default();
    assert!(!pdf.is_empty());
    // Every version is kept in FDX and comes back.
    let xml = run(&mut s, "document.fdx", json!({}))["xml"].as_str().unwrap_or("").to_string();
    let back = scriptcraft_fdx::parse(&xml).unwrap_or_default();
    assert_eq!(back.paras[3].text, "Start it.");
    assert_eq!(back.paras[3].alternates.len(), 2);
    let all = run(&mut s, "alt.list", json!({"all": true}));
    assert_eq!(all["lines"][0]["para"], 3);
    run(&mut s, "alt.edit", json!({"index": 0, "text": "Kick it."}));
    run(&mut s, "alt.delete", json!({"index": 1}));
    assert_eq!(run(&mut s, "alt.list", json!({}))["alternates"], json!(["Kick it."]));
    assert!(s.run("alt.choose", &json!({"index": 5})).is_err());
    // Not on action.
    run(&mut s, "caret.set", json!({"para": 1, "off": 0}));
    assert!(s.run("alt.add", &json!({"text": "x"})).is_err());
    assert!(s.run("alt.next", &json!({})).is_err());
}

#[test]
fn tagging_and_breakdown() {
    let mut s = session(SCRIPT);
    let tag = |s: &mut Session, para: usize, a: usize, b: usize, cat: &str| {
        run(s, "tag.add", json!({"category": cat, "anchor": {"para": para, "off": a}, "focus": {"para": para, "off": b}}));
    };
    tag(&mut s, 1, 17, 30, "Vehicles"); // red motorbike
    tag(&mut s, 1, 34, 40, "props"); // wrench
    tag(&mut s, 5, 4, 17, "Vehicles"); // red motorbike again
    tag(&mut s, 5, 31, 41, "Vehicles"); // police car
    tag(&mut s, 5, 31, 41, "=Picture Cars"); // a custom category
    assert!(s.run("tag.add", &json!({"category": "Props"})).is_err(), "nothing selected");
    let l = run(&mut s, "tag.list", json!({}));
    assert_eq!(l["tags"].as_array().map(Vec::len), Some(5));
    assert_eq!(l["tags"][1]["item"], "wrench");
    assert_eq!(l["tags"][1]["category"], "Props");
    let bike = l["items"].as_array().and_then(|v| v.iter().find(|x| x["item"] == "red motorbike")).cloned().unwrap_or_default();
    assert_eq!(bike["count"], 2);
    assert_eq!(bike["scenes"], json!([1, 2]));
    let r = run(&mut s, "report.breakdown", json!({}));
    assert_eq!(r["categories"], json!(["Vehicles", "Props", "=Picture Cars"]));
    assert_eq!(r["scenes"][0]["categories"][0], json!({"category": "Vehicles", "items": ["red motorbike"]}));
    assert_eq!(r["scenes"][1]["categories"][0]["items"], json!(["red motorbike", "police car"]));
    assert_eq!(r["scenes"][1]["speaking"], json!(["BEN"]));
    let csv = run(&mut s, "report.breakdownCsv", json!({}))["csv"].as_str().unwrap_or("").to_string();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "Scene,Number,Heading,Page,Length,Speaking,Vehicles,Props,'=Picture Cars");
    assert_eq!(lines[1], "1,,INT. GARAGE - NIGHT,1,2/8,ADA,red motorbike,wrench,");
    assert_eq!(lines[2], "2,,EXT. ROAD - DAWN,1,2/8,BEN,red motorbike; police car,,police car");
    let data = run(&mut s, "document.export", json!({"format": "csv"}));
    assert!(data["name"].as_str().is_some_and(|n| n.ends_with("Breakdown.csv")));
    assert_eq!(crate::b64::decode(data["data"].as_str().unwrap_or("")).unwrap_or_default(), csv.as_bytes());
    let cats = run(&mut s, "tag.categories", json!({}));
    assert!(cats["categories"].as_array().is_some_and(|c| c.iter().any(|x| x["name"] == "=Picture Cars" && x["standard"] == false)));
    // Removing at the caret, then by category over a range.
    run(&mut s, "caret.set", json!({"para": 1, "off": 35}));
    assert_eq!(run(&mut s, "tag.remove", json!({}))["removed"], 1);
    assert_eq!(
        run(&mut s, "tag.remove", json!({"category": "vehicles", "anchor": {"para": 0, "off": 0}, "focus": {"para": 6, "off": 0}}))["removed"],
        3
    );
    assert_eq!(run(&mut s, "tag.list", json!({}))["tags"].as_array().map(Vec::len), Some(1));
    // A file export (native).
    let dir = std::env::temp_dir().join(format!("sc-breakdown-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("b.csv");
    run(&mut s, "file.exportBreakdown", json!({"path": path.display().to_string()}));
    assert!(std::fs::read_to_string(&path).unwrap_or_default().starts_with("Scene,"));
    assert!(s.run("file.exportBreakdown", &json!({"path": dir.join("b.txt").display().to_string()})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
