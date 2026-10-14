//! Printing to a printer through the window, with a fake print host (nothing is ever printed).

use scriptcraft_engine::Session;
use scriptcraft_ui_egui::ScriptApp;
use scriptcraft_ui_egui::printing::FakePrinter;
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

fn status(h: &egui_kittest::Harness<'static, ScriptApp>) -> String {
    h.state().status_message().unwrap_or_default().to_string()
}

#[test]
fn printers_and_send() {
    let fake = FakePrinter::new(&["Office_LaserJet", "Home_Inkjet"]);
    let sent = fake.sent.clone();
    let cleaned = fake.cleaned.clone();
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    app.printer = Some(Box::new(fake));
    let mut h = harness(app);
    h.run_steps(2);

    let v = h.state_mut().execute("print.printers", json!({})).unwrap();
    assert_eq!(v["default"], "Office_LaserJet");
    assert_eq!(v["printers"].as_array().map(Vec::len), Some(2));
    assert_eq!(v["choosesInDialog"], false);

    // The Print window lists the printers, picks the default, and offers Open in Viewer too.
    h.state_mut().execute("ui.print", json!({})).unwrap();
    h.run_steps(3);
    let form = h.state().production.print.clone().expect("print dialog open");
    assert_eq!(form.printer.as_deref(), Some("Office_LaserJet"));
    assert_eq!(form.copies, 1);
    for b in ["Print…", "Open in Viewer", "Cancel"] {
        assert!(h.state().dialog_buttons.iter().any(|(n, _)| n == b), "{b}");
    }
    h.state_mut().execute("ui.print", json!({})).unwrap();

    // Two copies of pages 2–3 on the second printer.
    let r = h.state_mut().execute("print.send", json!({"printer": "Home_Inkjet", "copies": 2, "from": 2, "to": 3, "titlePage": false})).unwrap();
    assert_eq!(r["pages"], json!(["2", "3"]));
    assert_eq!(r["copies"], 2);
    h.run_steps(2);
    assert_eq!(status(&h), "Sent 2 copies of 2 pages to Home_Inkjet");
    {
        let s = sent.lock().unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].0.as_deref(), s[0].1, s[0].2), (Some("Home_Inkjet"), 2, 2));
        assert!(s[0].3 > 1000, "a whole PDF");
    }
    // No printer named: the default.
    h.state_mut().execute("print.send", json!({"titlePage": false, "to": 1})).unwrap();
    h.run_steps(2);
    assert_eq!(status(&h), "Sent 1 page to Office_LaserJet");

    // Refusals: unknown printer, an option smuggled in as a name, bad copies, empty range.
    for bad in [
        json!({"printer": "Nope"}),
        json!({"printer": "-o x"}),
        json!({"printer": 5}),
        json!({"copies": 0}),
        json!({"copies": 1000}),
        json!({"copies": "two"}),
        json!({"from": 50, "titlePage": false}),
    ] {
        assert!(h.state_mut().execute("print.send", bad.clone()).is_err(), "{bad}");
    }
    assert_eq!(sent.lock().unwrap().len(), 2);

    // Closing the app lets the host remove its temporary files.
    h.state_mut().on_exit();
    assert!(*cleaned.lock().unwrap());
}

#[test]
fn failures_show_in_the_status_bar() {
    let mut fake = FakePrinter::new(&["P"]);
    fake.fail = Some("no default destination".into());
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    app.printer = Some(Box::new(fake));
    let mut h = harness(app);
    let e = h.state_mut().execute("print.send", json!({})).unwrap_err();
    assert!(e.to_string().contains("no default destination"));
}

#[test]
fn without_a_print_host() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    let mut h = harness(app);
    for id in ["print.send", "print.printers"] {
        let e = h.state_mut().execute(id, json!({})).unwrap_err();
        assert!(e.to_string().contains("needs the ScriptCraft window"), "{id}: {e}");
    }
    // The Print window still writes a PDF and has no printer choice.
    h.state_mut().execute("ui.print", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().dialog_buttons.iter().any(|(n, _)| n == "Print…"));
    assert!(!h.state().dialog_buttons.iter().any(|(n, _)| n == "Open in Viewer"));
}

#[test]
fn hostile_print_params_never_panic() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    app.printer = Some(Box::new(FakePrinter::new(&["P"])));
    let mut h = harness(app);
    let junk = [
        json!(null),
        json!({}),
        json!({"printer": "\u{0}", "copies": -1e300, "from": "zz", "to": [1]}),
        json!({"printer": "P".repeat(10_000), "copies": 1e20, "refresh": "yes"}),
        json!({"printer": {"name": "P"}, "copies": null, "titlePage": 3, "revisedOnly": "x"}),
    ];
    for id in ["print.send", "print.printers"] {
        for j in &junk {
            let _ = h.state_mut().execute(id, j.clone());
        }
    }
    h.run_steps(2);
}
