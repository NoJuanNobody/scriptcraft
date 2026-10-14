//! Page Setup and Print through the window (headless, no GPU).

use scriptcraft_engine::Session;
use scriptcraft_ui_egui::ScriptApp;
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

#[test]
fn print_dialog_and_print_file() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    let mut h = harness(app);
    h.run_steps(3);
    h.state_mut().execute("ui.print", json!({})).unwrap();
    h.run_steps(3);
    let form = h.state().production.print.clone().expect("print dialog open");
    assert!(form.all && form.title_page);
    assert!(h.state().dialog_buttons.iter().any(|(n, _)| n == "Print…"));
    // Mod+P toggles the dialog.
    h.state_mut().execute("ui.print", json!({})).unwrap();
    assert!(h.state().production.print.is_none());
    // Printing pages 2–3 writes a PDF of just those pages; headless runs never open a viewer.
    let r = h.state_mut().execute("ui.printNow", json!({"from": 2, "to": 3, "titlePage": false})).unwrap();
    assert_eq!(r["pages"], json!(["2", "3"]));
    assert_eq!(r["opened"], false);
    let path = std::path::PathBuf::from(r["path"].as_str().unwrap());
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let _ = std::fs::remove_file(&path);
    assert!(h.state_mut().execute("ui.printNow", json!({"from": 50, "titlePage": false})).is_err());
}

#[test]
fn page_setup_dialog() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    let mut h = harness(app);
    h.state_mut().execute("ui.pageSetup", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().production.page_setup);
    h.state_mut().execute("page.set", json!({"paper": "a4"})).unwrap();
    h.run_steps(2);
    assert_eq!(h.state_mut().session.layout().paper, scriptcraft_doc::Paper::A4);
}
