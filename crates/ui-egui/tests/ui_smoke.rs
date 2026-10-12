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
