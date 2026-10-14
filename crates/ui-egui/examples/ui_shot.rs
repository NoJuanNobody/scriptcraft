//! Headless UI screenshots: renders the whole ScriptCraft window offscreen (wgpu), so it works
//! with a locked screen or a hidden window. Structure follows WordCraft's `ui_shot` (MIT OR
//! Apache-2.0; see NOTICE).
//!
//! `cargo run -p scriptcraft-ui-egui --example ui_shot -- script.jsonl`
//!
//! The script is JSON lines: control-channel requests (`{"method": …, "params": …}`, see
//! `docs/control-protocol.md`), `{"shot": "/abs/out.png"}` to save the window as PNG, or
//! `{"steps": n}` to run extra frames, `{"speaker": "silent"}` for a silent speech engine. The window is 1440×900 points at 2× and opens the sample
//! script unless the first line is `{"empty": true}`; `{"width": w, "height": h}` resizes it.

use scriptcraft_engine::Session;
use scriptcraft_ui_egui::{ControlRequest, ScriptApp};

static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn main() {
    let path = std::env::args().nth(1).expect("usage: ui_shot script.jsonl");
    let script = std::fs::read_to_string(&path).expect("read script");
    let (tx, rx) = std::sync::mpsc::channel::<ControlRequest>();
    let lines: Vec<serde_json::Value> =
        script.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).expect("json line")).collect();
    let session = if lines.first().is_some_and(|l| l.get("empty").is_some()) { Session::new(Default::default()) } else { Session::sample() };
    let mut app = ScriptApp::new(session).with_control(rx);
    app.dialogs = false;
    app.canvas.blink = false;
    // `{"speaker": "silent"}` anywhere: a silent speech engine with a few voices (Read Aloud).
    if lines.iter().any(|l| l.get("speaker").is_some()) {
        let mut s = scriptcraft_ui_egui::speech::SilentSpeaker::new(&["Samantha", "Daniel", "Karen", "Moira", "Tessa", "Fred", "Rishi"]);
        s.frames_per_line = 40;
        app.speaker = Some(Box::new(s));
    }
    let w = lines.iter().find_map(|l| l.get("width").and_then(|v| v.as_f64())).unwrap_or(1440.0) as f32;
    let h = lines.iter().find_map(|l| l.get("height").and_then(|v| v.as_f64())).unwrap_or(900.0) as f32;
    let mut harness =
        egui_kittest::Harness::builder().with_size(egui::vec2(w, h)).with_pixels_per_point(2.0).with_max_steps(1_000_000).wgpu().build_ui_state(
            |ui, app: &mut ScriptApp| {
                if !READY.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
    READY.store(true, std::sync::atomic::Ordering::Relaxed);
    step_n(&mut harness, 6);
    for l in lines {
        if let Some(p) = l.get("shot").and_then(|v| v.as_str()) {
            step_n(&mut harness, 4);
            match harness.render() {
                Ok(img) => {
                    img.save(p).expect("save png");
                    println!("{p}");
                }
                Err(e) => eprintln!("render failed: {e}"),
            }
        } else if let Some(n) = l.get("steps").and_then(|v| v.as_u64()) {
            step_n(&mut harness, n as usize);
        } else if let Some(m) = l.get("method").and_then(|v| v.as_str()) {
            let (req, reply) = ControlRequest::new(m, l.get("params").cloned().unwrap_or_default());
            tx.send(req).expect("send");
            for _ in 0..60 {
                step(&mut harness);
                if let Ok(r) = reply.try_recv() {
                    let s = r.to_string();
                    let cut = if std::env::var_os("UI_SHOT_FULL").is_some() { s.len() } else { s.len().min(300) };
                    println!("{m}: {}", s.get(..cut).unwrap_or(&s));
                    break;
                }
            }
            step_n(&mut harness, 3);
        }
    }
}

fn step(harness: &mut egui_kittest::Harness<'_, ScriptApp>) {
    let mut raw = std::mem::take(harness.input_mut());
    harness.state_mut().raw_input_hook(&mut raw);
    *harness.input_mut() = raw;
    harness.step();
}

fn step_n(harness: &mut egui_kittest::Harness<'_, ScriptApp>, n: usize) {
    for _ in 0..n {
        step(harness);
    }
}
