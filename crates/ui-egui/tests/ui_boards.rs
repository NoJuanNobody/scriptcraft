//! Split screen, the beat board and bookmarks, driven through the headless window.

use egui::{Event, PointerButton, Pos2, vec2};
use scriptcraft_engine::Session;
use scriptcraft_ui_egui::{ScriptApp, View};
use serde_json::json;

fn harness(app: ScriptApp) -> egui_kittest::Harness<'static, ScriptApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 860.0)).build_ui_state(
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

fn app() -> egui_kittest::Harness<'static, ScriptApp> {
    let mut a = ScriptApp::new(Session::sample());
    a.dialogs = false;
    a.canvas.blink = false;
    let mut h = harness(a);
    step(&mut h, 4);
    h
}

fn press(h: &mut egui_kittest::Harness<'static, ScriptApp>, at: Pos2, down: bool) {
    let m = egui::Modifiers::default();
    h.state_mut().synthetic.push(Event::PointerMoved(at));
    h.state_mut().synthetic.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: down, modifiers: m });
    step(h, 2);
}

fn type_text(h: &mut egui_kittest::Harness<'static, ScriptApp>, t: &str) {
    h.state_mut().synthetic.push(Event::Text(t.to_string()));
    step(h, 2);
}

fn count(h: &egui_kittest::Harness<'static, ScriptApp>, needle: &str) -> usize {
    h.state().session.script.paras.iter().map(|p| p.text.to_uppercase().matches(&needle.to_uppercase()).count()).sum()
}

#[test]
fn split_screen_two_views_one_caret() {
    let mut h = app();
    let v = h.state_mut().execute("ui.split", json!({"mode": "side"})).unwrap();
    assert_eq!(v["split"], "sideBySide");
    assert_eq!(v["second"], "cards");
    step(&mut h, 3);
    let [a, b] = h.state().split.rects;
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(a.right() < b.left() && a.width() > 300.0 && b.width() > 300.0);
    // Cards in the second pane, the page in the first: typing reaches the script once.
    assert!(!h.state().card_rects.is_empty());
    h.state_mut().execute("caret.set", json!({"para": 3, "off": 0})).unwrap();
    step(&mut h, 2);
    type_text(&mut h, "Zq1 ");
    assert_eq!(count(&h, "Zq1"), 1);
    // Two page views: each its own canvas; still one caret, keys handled once.
    h.state_mut().execute("ui.splitView", json!({"pane": 1, "view": "page"})).unwrap();
    step(&mut h, 3);
    assert!(!h.state().split.canvas.page_rects.is_empty() && !h.state().canvas.page_rects.is_empty());
    type_text(&mut h, "Zq2 ");
    assert_eq!(count(&h, "Zq2"), 1);
    // Click in the second pane: it takes focus; zoom goes to it, typing still goes in once.
    let c = b.center() + vec2(0.0, 40.0);
    press(&mut h, c, true);
    press(&mut h, c, false);
    assert_eq!(h.state().split.focus, 1);
    let z0 = (h.state().canvas.zoom, h.state().split.canvas.zoom);
    h.state_mut().execute("ui.zoomIn", json!({})).unwrap();
    assert_eq!(h.state().canvas.zoom, z0.0);
    assert!(h.state().split.canvas.zoom > z0.1);
    type_text(&mut h, "Zq3 ");
    assert_eq!(count(&h, "Zq3"), 1);
    // View commands go to the focused pane.
    h.state_mut().execute("ui.view.reports", json!({})).unwrap();
    assert_eq!((h.state().view, h.state().split.second), (View::Page, View::Reports));
    // Stacked, then back to one pane.
    h.state_mut().execute("ui.split", json!({"mode": "stacked"})).unwrap();
    step(&mut h, 3);
    let [a, b] = h.state().split.rects;
    assert!(a.unwrap().bottom() < b.unwrap().top());
    assert!(h.state_mut().execute("ui.split", json!({"mode": "diagonal"})).is_err());
    assert!(h.state_mut().execute("ui.splitView", json!({"view": "nope"})).is_err());
    h.state_mut().execute("ui.split", json!({})).unwrap();
    step(&mut h, 2);
    assert!(!h.state().split.on());
    assert!(h.state_mut().execute("ui.focusPane", json!({"pane": 1})).is_err());
}

#[test]
fn beat_board_drag_is_one_undo_step() {
    let mut h = app();
    let a = h
        .state_mut()
        .execute("beat.add", json!({"title": "Mayday", "text": "A voice in the static.", "color": "red", "x": 40, "y": 40, "scene": 1}))
        .unwrap();
    let a = a["id"].as_u64().unwrap() as u32;
    h.state_mut().execute("beat.add", json!({"title": "Rescue", "x": 360, "y": 40})).unwrap();
    h.state_mut().execute("ui.view.beats", json!({})).unwrap();
    step(&mut h, 4);
    let rects = h.state().board.rects.clone();
    assert_eq!(rects.len(), 2);
    let r = rects.iter().find(|(id, _)| *id == a).map(|(_, r)| *r).unwrap();
    let zoom = h.state().board.zoom;
    let undo_before = h.state().session.can_undo();
    let x0 = h.state().session.script.board.get(a).unwrap().x;
    let start = r.center();
    press(&mut h, start, true);
    for k in 1..=10 {
        h.state_mut().synthetic.push(Event::PointerMoved(start + vec2(12.0 * k as f32, 6.0 * k as f32)));
        step(&mut h, 1);
    }
    press(&mut h, start + vec2(120.0, 60.0), false);
    let b = h.state().session.script.board.get(a).unwrap().clone();
    assert!((b.x - (x0 + 120.0 / zoom)).abs() <= 1.0, "{} vs {}", b.x, x0 + 120.0 / zoom);
    assert_eq!(h.state().board.selected, Some(a));
    // One undo takes the whole drag back.
    assert!(h.state().session.can_undo() || undo_before);
    h.state_mut().execute("edit.undo", json!({})).unwrap();
    assert_eq!(h.state().session.script.board.get(a).unwrap().x, x0);
    // The Beat window opens and lays out; the board survives a second pane.
    h.state_mut().execute("ui.editBeat", json!({"id": a})).unwrap();
    step(&mut h, 3);
    assert!(h.state().board.editor.is_some());
    h.state_mut().execute("ui.split", json!({"mode": "side"})).unwrap();
    step(&mut h, 3);
    assert!(h.state_mut().execute("ui.editBeat", json!({"id": 999})).is_err());
}

#[test]
fn bookmarks_tab_lists_and_jumps() {
    let mut h = app();
    h.state_mut().execute("bookmark.add", json!({"para": 49, "name": "Wheelhouse"})).unwrap();
    h.state_mut().execute("bookmark.add", json!({"para": 4})).unwrap();
    h.state_mut().execute("ui.bookmarks", json!({})).unwrap();
    step(&mut h, 3);
    assert_eq!(h.state().nav_tab, 3);
    h.state_mut().execute("bookmark.goto", json!({"name": "wheelhouse"})).unwrap();
    step(&mut h, 3);
    assert_eq!(h.state().session.sel.focus.para, 49);
    // Bookmarks are saved in FDX and come back.
    let v = h.state_mut().execute("document.export", json!({"format": "fdx"})).unwrap();
    let data = v["data"].as_str().unwrap().to_string();
    h.state_mut().execute("file.load", json!({"name": "x.fdx", "data": data})).unwrap();
    assert_eq!(h.state().session.script.bookmark_para("Wheelhouse"), Some(49));
}
