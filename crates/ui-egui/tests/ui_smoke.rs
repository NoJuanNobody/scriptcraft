//! The window runs headless (no GPU): typing through egui reaches the engine, every UI command
//! runs, and all views lay out without panicking.

use scriptcraft_engine::Session;
use scriptcraft_ui_egui::{ControlRequest, ScriptApp, UI_COMMANDS};
use serde_json::json;

fn harness(app: ScriptApp) -> egui_kittest::Harness<'static, ScriptApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(
        |ui, app: &mut ScriptApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    )
}

fn step(h: &mut egui_kittest::Harness<'static, ScriptApp>, n: usize) {
    for _ in 0..n {
        let mut raw = std::mem::take(h.input_mut());
        h.state_mut().raw_input_hook(&mut raw);
        *h.input_mut() = raw;
        h.step();
    }
}

#[test]
fn typing_through_the_window() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = ScriptApp::new(Session::new(Default::default())).with_control(rx);
    app.dialogs = false;
    let mut h = harness(app);
    step(&mut h, 4);
    let send = |m: &str, p: serde_json::Value| {
        let (req, reply) = ControlRequest::new(m, p);
        tx.send(req).unwrap();
        reply
    };
    let mut replies = Vec::new();
    for (m, p) in [
        ("ui.text", json!({"text": "int. lab - night"})),
        ("ui.key", json!({"key": "Enter"})),
        ("ui.text", json!({"text": "Beakers bubble."})),
        ("ui.key", json!({"key": "Tab"})),
        ("ui.text", json!({"text": "ada"})),
        ("ui.key", json!({"key": "Enter"})),
        ("ui.text", json!({"text": "Eureka."})),
    ] {
        replies.push(send(m, p));
        step(&mut h, 3);
    }
    for r in replies {
        assert_eq!(r.try_recv().unwrap()["ok"], true);
    }
    let s = &h.state().session.script;
    let got: Vec<(&str, &str)> = s.paras.iter().map(|p| (p.element.id(), p.text.as_str())).collect();
    assert_eq!(got, vec![("sceneHeading", "INT. LAB - NIGHT"), ("action", "Beakers bubble."), ("character", "ADA"), ("dialogue", "Eureka.")]);
    let r = send("ui.inspect", json!({}));
    step(&mut h, 2);
    assert_eq!(r.try_recv().unwrap()["result"]["element"], "dialogue");
}

#[test]
fn every_ui_command_and_view_runs() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    let mut h = harness(app);
    step(&mut h, 3);
    for (id, ..) in UI_COMMANDS {
        let _ = h.state_mut().execute(id, json!({}));
        step(&mut h, 2);
    }
    for v in ["ui.view.cards", "ui.view.reports", "ui.theme.toggle", "ui.view.page", "ui.zoomIn", "ui.toggleNavigator"] {
        h.state_mut().execute(v, json!({})).unwrap();
        step(&mut h, 2);
    }
    assert!(h.state_mut().execute("ui.nope", json!({})).is_err());
    let cmds = h.state().describe_commands();
    assert!(cmds.as_array().unwrap().iter().any(|c| c["id"] == "ui.view.cards"));
}

#[test]
fn enter_in_an_empty_paragraph_shows_the_element_list() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = ScriptApp::new(Session::new(Default::default())).with_control(rx);
    app.dialogs = false;
    let mut h = harness(app);
    step(&mut h, 4);
    let send = |m: &str, p: serde_json::Value| {
        let (req, _reply) = ControlRequest::new(m, p);
        tx.send(req).unwrap();
    };
    send("ui.text", json!({"text": "int. lab - night"}));
    step(&mut h, 3);
    send("ui.key", json!({"key": "Enter"}));
    step(&mut h, 3);
    // The new paragraph is an empty Action: Enter opens the list instead of changing it.
    send("ui.key", json!({"key": "Enter"}));
    step(&mut h, 3);
    assert!(h.state().canvas.picker.is_some());
    assert_eq!(h.state().session.element(), scriptcraft_doc::Element::Action);
    send("ui.key", json!({"key": "ArrowDown"}));
    send("ui.key", json!({"key": "Enter"}));
    step(&mut h, 4);
    assert!(h.state().canvas.picker.is_none());
    // The list opened on Scene Heading (Enter's usual target); one row down is Action.
    assert_eq!(h.state().session.element(), scriptcraft_doc::Element::Action);
    send("ui.key", json!({"key": "Enter"}));
    step(&mut h, 3);
    send("ui.text", json!({"text": "3"}));
    step(&mut h, 3);
    assert_eq!(h.state().session.element(), scriptcraft_doc::Element::Character);
    // Turned off, Enter changes the element directly.
    h.state_mut().enter_picker = false;
    send("ui.key", json!({"key": "Enter"}));
    step(&mut h, 3);
    assert_eq!(h.state().session.element(), scriptcraft_doc::Element::Action);
    assert_eq!(h.state().session.script.paras.len(), 2);
}

#[test]
fn autosave_backups_and_recent_files() {
    use scriptcraft_ui_egui::persist;
    let dir = std::env::temp_dir().join(format!("scriptcraft-ui-state-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("draft.fountain");
    std::fs::write(&file, scriptcraft_engine::sample::FOUNTAIN).unwrap();
    let mut app = ScriptApp::new(Session::new(Default::default())).with_state_dir(dir.clone());
    app.dialogs = false;
    app.prefs.autosave_secs = 1;
    let mut h = harness(app);
    step(&mut h, 3);
    h.state_mut().execute("file.open", json!({"path": file.to_str().unwrap()})).unwrap();
    assert_eq!(h.state().prefs.recent, vec![file.display().to_string()]);
    h.state_mut().execute("text.insert", json!({"text": "Edited. "})).unwrap();
    assert!(h.state().session.dirty);
    step(&mut h, 12);
    let r = persist::read_recovery(&dir).expect("autosaved");
    assert!(r.script.paras[0].text.starts_with("Edited."));
    // Saving keeps the previous version and removes the recovery copy.
    h.state_mut().execute("ui.save", json!({})).unwrap();
    assert!(!h.state().session.dirty);
    assert!(persist::read_recovery(&dir).is_none());
    let backups: Vec<_> = std::fs::read_dir(dir.join(persist::BACKUPS)).unwrap().flatten().collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read_to_string(backups[0].path()).unwrap(), scriptcraft_engine::sample::FOUNTAIN);
    // A recovery file left by a crash is offered on the next start.
    persist::write_recovery(&dir, Some(&file), &scriptcraft_engine::sample::script()).unwrap();
    let next = ScriptApp::new(Session::new(Default::default())).with_state_dir(dir.clone());
    assert!(next.recovery_offer.is_some());
    assert_eq!(next.prefs.recent.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_script_with_unsaved_changes_asks_first() {
    let mut app = ScriptApp::new(Session::sample());
    app.execute("text.insert", json!({"text": "x"})).unwrap();
    let before = app.session.script.paras.len();
    let r = app.execute("file.new", json!({})).unwrap();
    assert_eq!(r["pending"], "unsaved changes");
    assert_eq!(app.session.script.paras.len(), before, "nothing replaced yet");
    assert!(app.confirm_replace.is_some());
    // Agents and tests (no dialogs) are never asked.
    let mut quiet = ScriptApp::new(Session::sample());
    quiet.dialogs = false;
    quiet.execute("text.insert", json!({"text": "x"})).unwrap();
    quiet.execute("file.new", json!({})).unwrap();
    assert_eq!(quiet.session.script.paras.len(), 1);
}

#[test]
fn speed_view_and_zoom() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    app.canvas.blink = false;
    let mut h = harness(app);
    step(&mut h, 3);
    h.state_mut().execute("ui.view.speed", json!({})).unwrap();
    step(&mut h, 2);
    assert!(h.state().session.speed);
    assert_eq!(h.state().canvas.page_rects.len(), 1, "one continuous page");
    // Reports and the status bar keep real pages.
    assert_eq!(h.state_mut().session.layout().script_pages(), 4);
    h.state_mut().execute("caret.docEnd", json!({})).unwrap();
    h.state_mut().execute("caret.up", json!({})).unwrap();
    h.state_mut().execute("ui.zoomFit", json!({})).unwrap();
    step(&mut h, 2);
    let z = h.state().canvas.zoom;
    assert!((0.4..=3.0).contains(&z) && z > 1.0, "{z}");
    h.state_mut().execute("ui.view.page", json!({})).unwrap();
    step(&mut h, 2);
    assert!(!h.state().session.speed);
    assert!(h.state().canvas.page_rects.len() > 1);
}
