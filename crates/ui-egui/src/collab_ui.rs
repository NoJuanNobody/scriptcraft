//! Real-time collaboration in the window (#25): the Collaborate window, other people's carets
//! and selections on the page, the status-bar indicator, and the per-frame sync.

use egui::{Color32, FontId, Rect, RichText, Ui, pos2, vec2};
use scriptcraft_layout::{Layout, LineKind};
use serde_json::{Value, json};

use crate::ScriptApp;

/// The Collaborate window's fields.
#[derive(Default)]
pub struct CollabDialog {
    pub open: bool,
    pub name: String,
    pub lan: bool,
    pub port: String,
    pub addr: String,
    pub key: String,
    /// Replace an unsaved script when joining.
    pub replace: bool,
}

/// How often a shared session exchanges changes while idle.
const POLL: std::time::Duration = std::time::Duration::from_millis(40);

fn color(i: u8) -> Color32 {
    let [r, g, b] = scriptcraft_collab::COLORS.get(usize::from(i)).copied().unwrap_or([90, 90, 90]);
    Color32::from_rgb(r, g, b)
}

/// Exchange changes with the session; called every frame.
pub fn tick(app: &mut ScriptApp, ctx: &egui::Context) {
    if !app.session.collab.active() {
        return;
    }
    if app.session.collab_tick() {
        ctx.request_repaint();
    }
    if !app.session.status.is_empty() {
        let s = std::mem::take(&mut app.session.status);
        app.set_status(s);
    }
    ctx.request_repaint_after(POLL);
}

/// `ui.collaborate`: open or close the window (`{"open": bool}`).
pub fn toggle(app: &mut ScriptApp, p: &Value) {
    app.collab.open = p.get("open").and_then(Value::as_bool).unwrap_or(!app.collab.open);
    if app.collab.name.is_empty() {
        app.collab.name = app.prefs.editing.note_author.clone();
    }
}

/// Other people's selections and carets, with name tags, on the visible pages.
pub fn paint_peers(app: &ScriptApp, ui: &Ui, layout: &Layout, rects: &[Rect]) {
    let others = app.session.collab.others();
    if others.is_empty() {
        return;
    }
    let z = app.canvas.zoom;
    let clip = ui.clip_rect();
    let p = ui.painter();
    for peer in others {
        let Some(sel) = peer.sel else { continue };
        let c = color(peer.color);
        let (a, b) = if sel.anchor <= sel.focus { (sel.anchor, sel.focus) } else { (sel.focus, sel.anchor) };
        let (a, b) = (app.session.script.clamp(a), app.session.script.clamp(b));
        if a != b {
            for (pi, page) in layout.pages.iter().enumerate() {
                let Some(pr) = rects.get(pi).filter(|r| r.intersects(clip)) else { continue };
                for line in page.lines.iter().filter(|l| l.kind == LineKind::Body) {
                    if let Some((c0, c1)) = crate::canvas::selected_cols(line, a, b) {
                        p.rect_filled(crate::canvas::cols_rect(*pr, z, line, c0, c1), 0.0, c.gamma_multiply(0.22));
                    }
                }
            }
        }
        let Some(cp) = layout.caret(app.session.script.clamp(sel.focus)) else { continue };
        let Some(pr) = rects.get(cp.page) else { continue };
        let r = Rect::from_min_size(pr.min + vec2(cp.x * z, cp.y * z), vec2(2.0, cp.h * z));
        if !r.intersects(clip) {
            continue;
        }
        p.rect_filled(r, 0.0, c);
        let font = FontId::proportional(11.0);
        let galley = p.layout_no_wrap(peer.name.clone(), font, Color32::WHITE);
        let tag = Rect::from_min_size(pos2(r.left(), r.top() - galley.size().y - 3.0), galley.size() + vec2(8.0, 2.0));
        p.rect_filled(tag, 3.0, c);
        p.galley(tag.min + vec2(4.0, 1.0), galley, Color32::WHITE);
    }
}

/// The status-bar indicator: who is in the session.
pub fn status_badge(app: &mut ScriptApp, ui: &mut Ui) {
    if !app.session.collab.active() {
        return;
    }
    let peers = app.session.collab.peers();
    let host = app.session.collab.my_id() == Some(0);
    let others = peers.len().saturating_sub(1);
    let label = match (host, others) {
        (true, 0) => "Hosting · waiting for others".to_string(),
        (true, n) => format!("Hosting · {n} joined"),
        (false, n) => format!("Shared with {n} {}", if n == 1 { "other" } else { "others" }),
    };
    // Right to left: the label, then everyone's colour dots before it.
    let r = ui.add(egui::Button::new(RichText::new(label).size(12.0).color(app.tokens().accent)).frame(false));
    for p in peers.iter().rev() {
        let (rect, resp) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color(p.color));
        resp.on_hover_text(&p.name);
    }
    if r.on_hover_text("People in this session (File › Collaborate…)").clicked() {
        app.collab.open = true;
    }
}

fn copy_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        ui.add(egui::Label::new(RichText::new(value).monospace()).selectable(true));
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(value.to_string());
        }
    });
}

/// The File › Collaborate… window.
pub fn window(app: &mut ScriptApp, ctx: &egui::Context) {
    if !app.collab.open {
        return;
    }
    let mut open = true;
    let mut action: Option<(&'static str, Value)> = None;
    let status = app.session.collab.status();
    let peers = app.session.collab.peers();
    egui::Window::new("Collaborate").open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
        let d = &mut app.collab;
        match status["role"].as_str().unwrap_or("off") {
            "off" => {
                if status["available"] == json!(false) {
                    ui.label("Collaboration needs the desktop app (the browser can't open network connections).");
                    return;
                }
                if let Some(e) = status["ended"].as_str() {
                    ui.label(RichText::new(format!("The last session ended: {e}.")).italics());
                    ui.add_space(4.0);
                }
                ui.horizontal(|ui| {
                    ui.label("Your name");
                    ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(180.0).hint_text("shown to the others"));
                });
                ui.separator();
                ui.label(RichText::new("Host this script").strong());
                ui.label(RichText::new("Others join with your address and a session key.").small());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut d.lan, "Allow people on my network");
                    ui.label("Port");
                    ui.add(egui::TextEdit::singleline(&mut d.port).desired_width(60.0).hint_text("any"));
                });
                if ui.button("Host Session").clicked() {
                    let mut v = json!({"name": d.name, "lan": d.lan});
                    if let Ok(p) = d.port.trim().parse::<u16>() {
                        v["port"] = json!(p);
                    }
                    action = Some(("collab.host", v));
                }
                ui.separator();
                ui.label(RichText::new("Join a session").strong());
                egui::Grid::new("collab-join").num_columns(2).show(ui, |ui| {
                    ui.label("Address");
                    ui.add(egui::TextEdit::singleline(&mut d.addr).desired_width(220.0).hint_text("192.168.1.20:7878"));
                    ui.end_row();
                    ui.label("Session key");
                    ui.add(egui::TextEdit::singleline(&mut d.key).desired_width(220.0));
                    ui.end_row();
                });
                if app.session.dirty {
                    ui.checkbox(&mut d.replace, "Replace my unsaved script with the shared one");
                }
                let ok = !d.addr.trim().is_empty() && !d.key.trim().is_empty() && (!app.session.dirty || d.replace);
                if ui.add_enabled(ok, egui::Button::new("Join Session")).clicked() {
                    action = Some(("collab.join", json!({"addr": d.addr.trim(), "key": d.key.trim(), "name": d.name, "force": d.replace})));
                }
            }
            role => {
                if role == "host" {
                    ui.label("You are hosting. Share an address and the key with your collaborators.");
                    for a in status["addresses"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        copy_row(ui, "Address", a);
                    }
                    copy_row(ui, "Key", status["key"].as_str().unwrap_or(""));
                    if status["lan"] == json!(false) {
                        ui.label(RichText::new("Only this computer can join (turn on network access when hosting).").small());
                    }
                } else {
                    ui.label(format!("Joined the session at {}.", status["host"].as_str().unwrap_or("?")));
                }
                ui.separator();
                ui.label(RichText::new("People").strong());
                for p in &peers {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 5.0, color(p.color));
                        let mut name = p.name.clone();
                        if p.host {
                            name.push_str("  (host)");
                        }
                        if Some(p.id) == app.session.collab.my_id() {
                            name.push_str("  (you)");
                        }
                        ui.label(name);
                    });
                }
                ui.add_space(6.0);
                let label = if role == "host" { "End Session" } else { "Leave Session" };
                if ui.button(label).clicked() {
                    action = Some(("collab.leave", json!({})));
                }
            }
        }
    });
    app.collab.open = open;
    if let Some((id, v)) = action {
        if id != "collab.leave" && !app.collab.name.trim().is_empty() {
            app.prefs.editing.note_author = scriptcraft_doc::clean_author(&app.collab.name);
        }
        app.run(id, v);
    }
}
