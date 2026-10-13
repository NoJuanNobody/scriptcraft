//! Records a narrated demo video of ScriptCraft: drives the real window offscreen (wgpu) from a
//! JSON-lines script, draws a pointer and a caption bar, and writes numbered PNG frames that
//! `ffmpeg` turns into a video.
//!
//! `cargo run --release -p scriptcraft-ui-egui --example ui_demo -- demo.jsonl frames/`
//!
//! Script lines (one action each):
//! - `{"title": "…", "sub": "…"}`: set the caption
//! - `{"hold": seconds}`: keep the picture
//! - `{"cmd": "id", "params": {…}}`: run a command (engine or `ui.*`)
//! - `{"type": "text"}`: type text, one character per frame
//! - `{"key": "Enter", "cmd": bool, "shift": bool, "alt": bool}`: press a key
//! - `{"point": target, "secs": s}`: glide the pointer to a target:
//!   `{"text": "STORMY"}` (scrolls it into view), `{"label": "Cancel"}`, `{"card": 2}`, `{"xy": [x, y]}`
//! - `{"click": "left"|"right", "count": n}`: click at the pointer
//! - `{"drag": target, "secs": s}`: press, glide to the target, release
//! - `{"copy": true}`, `{"paste": true}`: copy the selection; paste it at the caret
//! - `{"terminal": "transcript.txt", "line_secs": s}`, `{"terminal_off": true}`: show a terminal
//!   transcript line by line
//! - `{"pictures": "dir/", "frames_each": n, "hold_last": s}`, `{"pictures_off": true}`: play
//!   PNG frames recorded elsewhere (the browser build)
//! - `{"state": true}`: use a temporary settings folder (autosave, recent files, backups)
//! - `{"recovery": true}`: leave an autosaved copy there and offer it, as after a crash

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, pos2, vec2};
use egui_kittest::kittest::Queryable;
use scriptcraft_engine::Session;
use scriptcraft_ui_egui::ScriptApp;
use serde_json::{Value, json};

const FPS: f32 = 15.0;

static CAPTION: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));
static POINTER: Mutex<Option<Pos2>> = Mutex::new(None);
static PRESSED: Mutex<bool> = Mutex::new(false);
static CLIP: Mutex<String> = Mutex::new(String::new());
/// A full-screen terminal transcript (lines shown so far).
static TERMINAL: Mutex<Option<Vec<String>>> = Mutex::new(None);
/// A full-screen picture (frames recorded elsewhere, e.g. the browser build).
static PICTURE: Mutex<Option<egui::TextureHandle>> = Mutex::new(None);

struct Rec {
    dir: PathBuf,
    n: usize,
}

impl Rec {
    fn path(&self, i: usize) -> PathBuf {
        self.dir.join(format!("f{i:05}.png"))
    }
    fn shot(&mut self, h: &mut egui_kittest::Harness<'_, ScriptApp>) {
        step(h);
        match h.render() {
            Ok(img) => {
                if let Err(e) = img.save(self.path(self.n)) {
                    eprintln!("save failed: {e}");
                }
                self.n += 1;
            }
            Err(e) => eprintln!("render failed: {e}"),
        }
    }
    fn hold(&mut self, h: &mut egui_kittest::Harness<'_, ScriptApp>, secs: f32) {
        let frames = (secs * FPS).round().max(1.0) as usize;
        // Keep stepping a few frames (blinking caret, animations), copy the rest.
        for k in 0..frames {
            if k % 4 == 0 || self.n == 0 {
                self.shot(h);
            } else {
                let _ = std::fs::copy(self.path(self.n - 1), self.path(self.n));
                self.n += 1;
            }
        }
    }
}

fn step(h: &mut egui_kittest::Harness<'_, ScriptApp>) {
    let mut raw = std::mem::take(h.input_mut());
    h.state_mut().raw_input_hook(&mut raw);
    *h.input_mut() = raw;
    h.step();
    // What the desktop app does when Quit is chosen: ask the window to close.
    let app = h.state_mut();
    if std::mem::take(&mut app.close_wanted) {
        let _ = app.close_requested();
    }
}

/// Draw the caption bar, a badge and the pointer over the app.
fn overlay(ctx: &egui::Context) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("demo-overlay")));
    let screen = ctx.content_rect();
    if let Some(tex) = PICTURE.lock().ok().and_then(|g| g.clone()) {
        p.rect_filled(screen, 0.0, Color32::from_rgb(0x14, 0x16, 0x1a));
        let size = tex.size_vec2();
        let k = (screen.width() / size.x).min(screen.height() / size.y);
        let r = Rect::from_center_size(screen.center(), size * k);
        p.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let bar = Rect::from_min_size(screen.min, vec2(screen.width(), 30.0));
        p.rect_filled(bar, 0.0, Color32::from_rgba_unmultiplied(34, 37, 43, 235));
        p.text(
            bar.left_center() + vec2(14.0, 0.0),
            Align2::LEFT_CENTER,
            "Google Chrome  ·  http://127.0.0.1:8765  ·  ScriptCraft compiled to WebAssembly",
            FontId::proportional(14.0),
            Color32::from_rgb(0xc8, 0xcd, 0xe0),
        );
    }
    if let Some(lines) = TERMINAL.lock().ok().and_then(|g| g.clone()) {
        p.rect_filled(screen, 0.0, Color32::from_rgb(0x14, 0x16, 0x1a));
        let bar = Rect::from_min_size(screen.min, vec2(screen.width(), 40.0));
        p.rect_filled(bar, 0.0, Color32::from_rgb(0x22, 0x25, 0x2b));
        for (i, c) in
            [Color32::from_rgb(0xe0, 0x4f, 0x4f), Color32::from_rgb(0xe6, 0xc2, 0x2e), Color32::from_rgb(0x4c, 0xae, 0x4f)].iter().enumerate()
        {
            p.circle_filled(bar.left_center() + vec2(22.0 + 20.0 * i as f32, 0.0), 6.0, *c);
        }
        p.text(
            bar.center(),
            Align2::CENTER_CENTER,
            "Terminal: a real run of the desktop app",
            FontId::proportional(15.0),
            Color32::from_rgb(0x9a, 0x9d, 0xa6),
        );
        let font = FontId::new(17.0, egui::FontFamily::Name("courier".into()));
        for (i, l) in lines.iter().enumerate() {
            let color = if l.starts_with('$') { Color32::from_rgb(0x9d, 0xb2, 0xff) } else { Color32::from_rgb(0xe4, 0xe2, 0xdc) };
            p.text(pos2(screen.left() + 60.0, screen.top() + 70.0 + i as f32 * 25.0), Align2::LEFT_TOP, l, font.clone(), color);
        }
    }
    let (title, sub) = CAPTION.lock().map(|c| c.clone()).unwrap_or_default();
    if !title.is_empty() {
        let w = 820.0f32.min(screen.width() - 40.0);
        let r = Rect::from_center_size(pos2(screen.center().x, screen.bottom() - 70.0), vec2(w, if sub.is_empty() { 52.0 } else { 78.0 }));
        p.rect_filled(r, 14.0, Color32::from_rgba_unmultiplied(16, 18, 26, 236));
        p.rect_stroke(r, 14.0, Stroke::new(1.5, Color32::from_rgb(0x6f, 0x8c, 0xff)), egui::StrokeKind::Inside);
        p.text(pos2(r.center().x, r.top() + 26.0), Align2::CENTER_CENTER, &title, FontId::proportional(23.0), Color32::WHITE);
        if !sub.is_empty() {
            p.text(pos2(r.center().x, r.top() + 56.0), Align2::CENTER_CENTER, &sub, FontId::proportional(15.5), Color32::from_rgb(0xc8, 0xcd, 0xe0));
        }
    }
    let badge = Rect::from_min_size(pos2(screen.right() - 236.0, screen.top() + 52.0), vec2(220.0, 26.0));
    p.rect_filled(badge, 13.0, Color32::from_rgba_unmultiplied(59, 91, 219, 230));
    p.text(badge.center(), Align2::CENTER_CENTER, "ScriptCraft v0.2 · PR #33", FontId::proportional(13.5), Color32::WHITE);
    let covered = TERMINAL.lock().map(|g| g.is_some()).unwrap_or(false) || PICTURE.lock().map(|g| g.is_some()).unwrap_or(false);
    if let Some(at) = POINTER.lock().ok().and_then(|g| *g).filter(|_| !covered) {
        let pressed = PRESSED.lock().map(|g| *g).unwrap_or(false);
        if pressed {
            p.circle_filled(at, 14.0, Color32::from_rgba_unmultiplied(59, 91, 219, 70));
        }
        let s = 1.15;
        let pts: Vec<Pos2> = [(0.0, 0.0), (0.0, 17.0), (4.5, 13.0), (7.5, 19.5), (10.0, 18.5), (7.0, 12.0), (12.5, 12.0)]
            .iter()
            .map(|(x, y)| at + vec2(x * s, y * s))
            .collect();
        p.add(egui::Shape::convex_polygon(pts, Color32::BLACK, Stroke::new(1.3, Color32::WHITE)));
    }
}

fn set_pointer(app: &mut ScriptApp, at: Pos2) {
    if let Ok(mut g) = POINTER.lock() {
        *g = Some(at);
    }
    app.synthetic.push(egui::Event::PointerMoved(at));
}

fn pointer() -> Pos2 {
    POINTER.lock().ok().and_then(|g| *g).unwrap_or(pos2(700.0, 400.0))
}

/// Screen point of a target.
fn target(h: &mut egui_kittest::Harness<'_, ScriptApp>, rec: &mut Rec, t: &Value) -> Option<Pos2> {
    if let Some(xy) = t.get("xy").and_then(Value::as_array) {
        return Some(pos2(xy.first()?.as_f64()? as f32, xy.get(1)?.as_f64()? as f32));
    }
    if let Some(i) = t.get("card").and_then(Value::as_u64) {
        let r = h.state().card_rects.get(i as usize).copied()?;
        return Some(r.left_top() + vec2(r.width() * 0.5, 24.0));
    }
    if let Some(l) = t.get("label").and_then(Value::as_str) {
        let exact = h.query_all_by_label(l).next().map(|n| n.rect().center());
        return exact.or_else(|| h.query_all_by_label_contains(l).next().map(|n| n.rect().center()));
    }
    if let Some(text) = t.get("text").and_then(Value::as_str) {
        let skip = t.get("nth").and_then(Value::as_u64).unwrap_or(0) as usize;
        let at = {
            let s = &h.state().session.script;
            s.paras.iter().enumerate().flat_map(|(i, p)| p.text.match_indices(text).map(move |(o, _)| (i, o))).nth(skip)?
        };
        // Bring it into view (the caret goes there), then measure.
        let _ = h.state_mut().execute("caret.set", json!({"para": at.0, "off": at.1}));
        for _ in 0..3 {
            rec.shot(h);
        }
        let app = h.state_mut();
        let l = app.session.view_layout();
        let c = l.caret(scriptcraft_doc::Pos::new(at.0, at.1 + text.len() / 2))?;
        let pr = app.canvas.page_rects.get(c.page)?;
        let z = app.canvas.zoom;
        return Some(pr.min + vec2(c.x * z, (c.y + c.h * 0.6) * z));
    }
    None
}

fn glide(h: &mut egui_kittest::Harness<'_, ScriptApp>, rec: &mut Rec, to: Pos2, secs: f32) {
    let from = pointer();
    let n = (secs * FPS).round().max(1.0) as usize;
    for k in 1..=n {
        let q = k as f32 / n as f32;
        let e = q * q * (3.0 - 2.0 * q);
        set_pointer(h.state_mut(), from + (to - from) * e);
        rec.shot(h);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let script = args.next().expect("usage: ui_demo demo.jsonl frames/");
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "frames".into()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("frames dir");
    let lines: Vec<Value> = std::fs::read_to_string(&script)
        .expect("read script")
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad line {l}: {e}")))
        .collect();
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_pixels_per_point(1.5)
        .with_step_dt(1.0 / FPS)
        .with_max_steps(1_000_000)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut ScriptApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
                overlay(&ctx);
            },
            app,
        );
    let mut rec = Rec { dir: dir.clone(), n: 0 };
    let state_dir = std::env::temp_dir().join(format!("scriptcraft-demo-{}", std::process::id()));
    for _ in 0..4 {
        step(&mut h);
    }
    for l in lines {
        if let Some(t) = l.get("title").and_then(Value::as_str) {
            if let Ok(mut c) = CAPTION.lock() {
                *c = (t.to_string(), l.get("sub").and_then(Value::as_str).unwrap_or("").to_string());
            }
            rec.shot(&mut h);
        } else if let Some(s) = l.get("hold").and_then(Value::as_f64) {
            rec.hold(&mut h, s as f32);
        } else if let Some(id) = l.get("cmd").and_then(Value::as_str) {
            if let Err(e) = h.state_mut().execute(id, l.get("params").cloned().unwrap_or(json!({}))) {
                eprintln!("{id}: {e}");
            }
            rec.shot(&mut h);
            rec.shot(&mut h);
        } else if let Some(t) = l.get("type").and_then(Value::as_str) {
            for c in t.chars() {
                h.state_mut().synthetic.push(egui::Event::Text(c.to_string()));
                rec.shot(&mut h);
            }
        } else if let Some(k) = l.get("key").and_then(Value::as_str) {
            let b = |n: &str| l.get(n).and_then(Value::as_bool).unwrap_or(false);
            let cmd = b("cmd");
            let m = egui::Modifiers {
                alt: b("alt"),
                shift: b("shift"),
                command: cmd,
                mac_cmd: cmd && cfg!(target_os = "macos"),
                ctrl: cmd && !cfg!(target_os = "macos"),
            };
            let key = egui::Key::from_name(k).unwrap_or(egui::Key::Enter);
            let app = h.state_mut();
            app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
            app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m });
            for _ in 0..3 {
                rec.shot(&mut h);
            }
        } else if let Some(t) = l.get("point") {
            let secs = l.get("secs").and_then(Value::as_f64).unwrap_or(0.7) as f32;
            match target(&mut h, &mut rec, t) {
                Some(to) => glide(&mut h, &mut rec, to, secs),
                None => eprintln!("no target {t}"),
            }
        } else if let Some(b) = l.get("click").and_then(Value::as_str) {
            let button = if b == "right" { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
            let at = pointer();
            let count = l.get("count").and_then(Value::as_u64).unwrap_or(1);
            let app = h.state_mut();
            app.synthetic.push(egui::Event::PointerMoved(at));
            for _ in 0..count {
                app.synthetic.push(egui::Event::PointerButton { pos: at, button, pressed: true, modifiers: Default::default() });
                app.synthetic.push(egui::Event::PointerButton { pos: at, button, pressed: false, modifiers: Default::default() });
            }
            if let Ok(mut g) = PRESSED.lock() {
                *g = true;
            }
            rec.shot(&mut h);
            if let Ok(mut g) = PRESSED.lock() {
                *g = false;
            }
            for _ in 0..3 {
                rec.shot(&mut h);
            }
        } else if let Some(t) = l.get("drag") {
            let secs = l.get("secs").and_then(Value::as_f64).unwrap_or(1.0) as f32;
            let Some(to) = target(&mut h, &mut rec, t) else {
                eprintln!("no drag target {t}");
                continue;
            };
            let from = pointer();
            if let Ok(mut g) = PRESSED.lock() {
                *g = true;
            }
            h.state_mut().synthetic.push(egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
            rec.shot(&mut h);
            glide(&mut h, &mut rec, to, secs);
            h.state_mut().synthetic.push(egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
            if let Ok(mut g) = PRESSED.lock() {
                *g = false;
            }
            for _ in 0..3 {
                rec.shot(&mut h);
            }
        } else if l.get("copy").is_some() {
            if let Ok(v) = h.state_mut().execute("edit.copy", json!({}))
                && let Ok(mut c) = CLIP.lock()
            {
                *c = v["text"].as_str().unwrap_or("").to_string();
            }
            rec.shot(&mut h);
        } else if l.get("paste").is_some() {
            let text = CLIP.lock().map(|c| c.clone()).unwrap_or_default();
            if let Err(e) = h.state_mut().execute("edit.paste", json!({"text": text})) {
                eprintln!("paste: {e}");
            }
            rec.shot(&mut h);
            rec.shot(&mut h);
        } else if let Some(file) = l.get("terminal").and_then(Value::as_str) {
            let text = std::fs::read_to_string(file).unwrap_or_default();
            let secs = l.get("line_secs").and_then(Value::as_f64).unwrap_or(0.6) as f32;
            let mut shown = Vec::new();
            for line in text.lines() {
                for chunk in wrap(line, 96) {
                    shown.push(chunk);
                }
                if let Ok(mut g) = TERMINAL.lock() {
                    *g = Some(shown.clone());
                }
                rec.hold(&mut h, if line.starts_with('$') { secs * 1.4 } else { secs });
            }
        } else if l.get("terminal_off").is_some() {
            if let Ok(mut g) = TERMINAL.lock() {
                *g = None;
            }
            rec.shot(&mut h);
        } else if let Some(d) = l.get("pictures").and_then(Value::as_str) {
            let per = l.get("frames_each").and_then(Value::as_u64).unwrap_or(2) as usize;
            let mut files: Vec<PathBuf> = std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
            files.sort();
            let ctx = h.ctx.clone();
            for f in files {
                let Ok(img) = image::open(&f) else { continue };
                let img = img.to_rgba8();
                let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                let tex = ctx.load_texture("picture", ci, egui::TextureOptions::LINEAR);
                if let Ok(mut g) = PICTURE.lock() {
                    *g = Some(tex);
                }
                rec.shot(&mut h);
                for _ in 1..per {
                    let _ = std::fs::copy(rec.path(rec.n - 1), rec.path(rec.n));
                    rec.n += 1;
                }
            }
            let hold = l.get("hold_last").and_then(Value::as_f64).unwrap_or(2.0) as f32;
            rec.hold(&mut h, hold);
        } else if l.get("pictures_off").is_some() {
            if let Ok(mut g) = PICTURE.lock() {
                *g = None;
            }
            rec.shot(&mut h);
        } else if l.get("state").is_some() {
            let _ = std::fs::remove_dir_all(&state_dir);
            let _ = std::fs::create_dir_all(&state_dir);
            let file = state_dir.join("Night Signal.fountain");
            let _ = std::fs::write(&file, scriptcraft_engine::sample::FOUNTAIN);
            let app = h.state_mut();
            app.use_folders(scriptcraft_control_key::Folders {
                config: Some(state_dir.clone()),
                data: Some(state_dir.clone()),
                rule: "demo".into(),
                legacy: None,
            });
            let _ = app.execute("file.open", json!({"path": file.to_str().unwrap_or("")}));
            rec.shot(&mut h);
        } else if l.get("recovery").is_some() {
            let mut s = scriptcraft_engine::sample::script();
            s.paras.insert(1, scriptcraft_doc::Para::new(scriptcraft_doc::Element::Action, "These words were typed just before the crash."));
            let _ = scriptcraft_ui_egui::persist::write_recovery(&state_dir, Some(Path::new("Night Signal.fountain")), &s);
            h.state_mut().recovery_offer = scriptcraft_ui_egui::persist::read_recovery(&state_dir);
            rec.shot(&mut h);
        }
    }
    let _ = std::fs::remove_dir_all(&state_dir);
    println!("{} frames in {}", rec.n, dir.display());
}

/// Break a line at word boundaries into pieces of at most `n` characters.
fn wrap(line: &str, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in line.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > n {
            out.push(std::mem::take(&mut cur));
            cur.push_str("    ");
        }
        if !cur.is_empty() && !cur.ends_with("    ") {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    out.push(cur);
    out
}
