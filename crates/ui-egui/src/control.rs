//! Programmatic control of the running app (agents, tests, the MCP bridge).
//!
//! Methods (JSON lines; see `docs/control-protocol.md`):
//! - `engine.execute {command, params}`: run any command (engine or `ui.*`)
//! - `engine.commands`: every command with its enablement
//! - `document.inspect`: script structure; `ui.inspect`: view state and geometry
//! - `ui.key {key, shift?, alt?, cmd?}`, `ui.text {text}`: keyboard input through egui
//! - `ui.click {x, y, count?, shift?, button?: "right"}`: pointer input in window points
//! - `ui.drag {x, y, toX, toY}`: press, move in steps and release
//! - `ui.screenshot {path?}`: PNG of the window (base64 `png` without a path)

use std::sync::mpsc::Sender;

use serde_json::{Value, json};

use crate::ScriptApp;

pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: Sender<Value>,
}

impl ControlRequest {
    pub fn new(method: impl Into<String>, params: Value) -> (Self, std::sync::mpsc::Receiver<Value>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (Self { method: method.into(), params, reply: tx }, rx)
    }
}

fn ok(v: Value) -> Value {
    json!({"ok": true, "result": v})
}
fn err(e: impl std::fmt::Display) -> Value {
    json!({"ok": false, "error": e.to_string()})
}

fn key_from(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name).or(match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => Some(egui::Key::Enter),
        "esc" | "escape" => Some(egui::Key::Escape),
        "tab" => Some(egui::Key::Tab),
        "backspace" => Some(egui::Key::Backspace),
        "delete" => Some(egui::Key::Delete),
        "left" => Some(egui::Key::ArrowLeft),
        "right" => Some(egui::Key::ArrowRight),
        "up" => Some(egui::Key::ArrowUp),
        "down" => Some(egui::Key::ArrowDown),
        "home" => Some(egui::Key::Home),
        "end" => Some(egui::Key::End),
        "space" => Some(egui::Key::Space),
        _ => None,
    })
}

fn mods(p: &Value) -> egui::Modifiers {
    let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
    let cmd = b("cmd") || b("ctrl");
    egui::Modifiers {
        alt: b("alt"),
        ctrl: cmd && !cfg!(target_os = "macos"),
        shift: b("shift"),
        mac_cmd: cmd && cfg!(target_os = "macos"),
        command: cmd,
    }
}

pub fn inspect(app: &mut ScriptApp, ctx: &egui::Context) -> Value {
    let r = ctx.content_rect();
    json!({
        "view": app.view.id(),
        "dark": app.dark,
        "zoom": app.canvas.zoom,
        "navigator": app.navigator,
        "window": [r.width(), r.height()],
        "pages": app.canvas.page_rects.iter().map(|r| [r.left(), r.top(), r.width(), r.height()]).collect::<Vec<_>>(),
        "caret": app.canvas.caret_screen.map(|c| [c.left(), c.top(), c.height()]),
        "smartType": app.canvas.suggest_shown,
        "element": app.session.element().id(),
        "dirty": app.session.dirty,
        "findBar": app.find.is_some(),
    })
}

/// Handle one request (screenshots are handled by the app, asynchronously).
pub fn handle(app: &mut ScriptApp, ctx: &egui::Context, req: &ControlRequest) -> Value {
    let p = &req.params;
    let f = |k: &str| p.get(k).and_then(Value::as_f64).map(|v| v as f32);
    match req.method.as_str() {
        "engine.execute" | "command" => {
            let Some(id) = p.get("command").or(p.get("id")).and_then(Value::as_str) else { return err("missing `command`") };
            let params = p.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({}));
            match app.execute(id, params) {
                Ok(v) => ok(v),
                Err(e) => err(e),
            }
        }
        "engine.commands" => ok(app.describe_commands()),
        "document.inspect" => match app.session.run("document.inspect", p) {
            Ok(v) => ok(v),
            Err(e) => err(e),
        },
        "ui.inspect" => ok(inspect(app, ctx)),
        "ui.key" => {
            let Some(key) = p.get("key").and_then(Value::as_str).and_then(key_from) else { return err("unknown or missing `key`") };
            let m = mods(p);
            app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
            app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m });
            ok(json!({}))
        }
        "ui.text" => {
            let Some(t) = p.get("text").and_then(Value::as_str) else { return err("missing `text`") };
            app.synthetic.push(egui::Event::Text(t.to_string()));
            ok(json!({}))
        }
        "ui.click" => {
            let (Some(x), Some(y)) = (f("x"), f("y")) else { return err("missing x/y") };
            let pos = egui::pos2(x, y);
            let m = mods(p);
            let count = p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3);
            let button =
                if p.get("button").and_then(Value::as_str) == Some("right") { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
            app.synthetic.push(egui::Event::PointerMoved(pos));
            for _ in 0..count {
                app.synthetic.push(egui::Event::PointerButton { pos, button, pressed: true, modifiers: m });
                app.synthetic.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: m });
            }
            ok(json!({}))
        }
        "ui.drag" => {
            let (Some(x), Some(y), Some(tx), Some(ty)) = (f("x"), f("y"), f("toX"), f("toY")) else { return err("missing x/y/toX/toY") };
            let m = mods(p);
            let button = egui::PointerButton::Primary;
            // One step per frame (a focus event separates frames), so egui sees a real drag.
            let sep = || egui::Event::WindowFocused(true);
            app.synthetic.push(egui::Event::PointerMoved(egui::pos2(x, y)));
            app.synthetic.push(egui::Event::PointerButton { pos: egui::pos2(x, y), button, pressed: true, modifiers: m });
            for k in 1..=8 {
                let q = k as f32 / 8.0;
                app.synthetic.push(sep());
                app.synthetic.push(egui::Event::PointerMoved(egui::pos2(x + (tx - x) * q, y + (ty - y) * q)));
            }
            app.synthetic.push(sep());
            app.synthetic.push(egui::Event::PointerButton { pos: egui::pos2(tx, ty), button, pressed: false, modifiers: m });
            ok(json!({}))
        }
        other => err(format!("unknown method `{other}`")),
    }
}

/// PNG-encode a screenshot; write it to `path` or return it as base64.
pub fn encode_shot(img: &egui::ColorImage, path: Option<&str>) -> Value {
    let [w, h] = img.size;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) else { return err("bad screenshot size") };
    let mut png = Vec::new();
    if let Err(e) = buf.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png) {
        return err(e);
    }
    match path {
        Some(p) => match std::fs::write(p, &png) {
            Ok(()) => ok(json!({"path": p, "width": w, "height": h})),
            Err(e) => err(e),
        },
        None => ok(json!({"png": base64(&png), "width": w, "height": h})),
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c.first().copied().unwrap_or(0), c.get(1).copied().unwrap_or(0), c.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].iter().enumerate() {
            if i <= c.len() {
                out.push(char::from(T[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }
}
