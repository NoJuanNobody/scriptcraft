//! Drafts: File › New from Template, Format › Apply Template, track changes and comparing
//! drafts.

use egui::Ui;
use scriptcraft_doc::Template;
use serde_json::json;

use crate::ScriptApp;
use egui::text::LayoutJob;
use egui::{Color32, FontId, Painter, Rect, RichText, Stroke, TextFormat, pos2, vec2};
use scriptcraft_doc::track::{self, ChangeKind};
use scriptcraft_layout::{Line, LineKind};
use serde_json::Value;

/// File › New from Template.
pub(crate) fn new_menu(app: &mut ScriptApp, ui: &mut Ui) {
    ui.menu_button("New from Template", |ui| {
        for t in Template::ALL {
            if ui.button(t.name()).on_hover_text(t.description()).clicked() {
                app.run("template.new", json!({"template": t.id()}));
                ui.close();
            }
        }
    });
}

/// Format › Apply Template (formatting only; the text is kept).
pub(crate) fn apply_menu(app: &mut ScriptApp, ui: &mut Ui) {
    let cur = Template::detect(&app.session.script.settings);
    ui.menu_button("Template Formatting", |ui| {
        for t in Template::ALL {
            if ui.radio(cur == Some(t), t.name()).on_hover_text(t.description()).clicked() {
                app.run("template.apply", json!({"template": t.id()}));
                ui.close();
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Track changes and Compare Drafts.

/// Window state.
#[derive(Default)]
pub struct DraftsUi {
    /// The Changes window is open.
    pub changes: bool,
    /// Show only this author's changes in the window.
    pub only: Option<String>,
    /// The last comparison (the `compare.with` result) and whether its window is open.
    pub compare: Option<Value>,
    pub compare_open: bool,
    /// A file is on its way from the host (the browser) to compare with.
    pub compare_pending: bool,
}

/// A stable colour per author.
pub fn author_color(name: &str) -> Color32 {
    const C: [[u8; 3]; 6] = [[0x2f, 0x6f, 0xd6], [0xc2, 0x3b, 0x8a], [0x1f, 0x8a, 0x5b], [0xc9, 0x6a, 0x12], [0x7a, 0x4c, 0xd0], [0x13, 0x8a, 0x9a]];
    let h = name.bytes().fold(2166136261u32, |h, b| (h ^ u32::from(b)).wrapping_mul(16777619));
    let [r, g, b] = C.get(h as usize % C.len()).copied().unwrap_or([0x2f, 0x6f, 0xd6]);
    Color32::from_rgb(r, g, b)
}

fn col(line: &Line, off: usize) -> usize {
    let text = line.text.as_str();
    let rel = off.saturating_sub(line.start).min(text.len());
    text.get(..rel).map_or_else(|| text.chars().count(), |t| t.chars().count())
}

/// On a body line: insertions underlined and tinted in their author's colour, deletion points
/// as red marks, and a change bar in the left margin.
pub fn paint_changes(app: &ScriptApp, p: &Painter, line: &Line, pr: Rect, z: f32) {
    if line.kind != LineKind::Body {
        return;
    }
    let Some(pi) = line.para else { return };
    let Some(para) = app.session.script.para(pi) else { return };
    if para.changes.is_empty() {
        return;
    }
    let cw = scriptcraft_geom::CHAR_W;
    let lh = scriptcraft_geom::LINE_H;
    let top = pr.top() + line.y * z;
    let x_of = |off: usize| pr.left() + (line.x + col(line, off) as f32 * cw) * z;
    let last_line = line.end >= para.text.len();
    let mut any = false;
    for c in &para.changes {
        let color = author_color(&c.author);
        match c.kind {
            ChangeKind::Insert => {
                if c.end <= line.start || c.start >= line.end {
                    continue;
                }
                let (a, b) = (x_of(c.start.max(line.start)), x_of(c.end.min(line.end)));
                p.rect_filled(Rect::from_min_max(pos2(a, top), pos2(b, top + lh * z)), 0.0, color.gamma_multiply(0.14));
                let y = top + (lh - 0.6) * z;
                p.line_segment([pos2(a, y), pos2(b, y)], Stroke::new(1.3 * z, color));
                any = true;
            }
            ChangeKind::Delete => {
                let on_line =
                    c.start >= line.start && (c.start < line.end || (last_line && c.start <= line.end)) || (line.start == line.end && c.start == 0);
                if !on_line {
                    continue;
                }
                let x = x_of(c.start);
                let red = Color32::from_rgb(0xd0, 0x2a, 0x2a);
                p.line_segment([pos2(x, top - 0.5 * z), pos2(x, top + lh * z)], Stroke::new(1.4 * z, red));
                let tri = vec![pos2(x - 3.0 * z, top - 1.5 * z), pos2(x + 3.0 * z, top - 1.5 * z), pos2(x, top + 2.0 * z)];
                p.add(egui::Shape::convex_polygon(tri, red, Stroke::NONE));
                any = true;
            }
            ChangeKind::Break | ChangeKind::Element => {
                if line.start != 0 {
                    continue;
                }
                let glyph = if c.kind == ChangeKind::Break { "¶" } else { "E" };
                let pos = pos2(pr.left() + 14.0 * z, top + 0.5 * z);
                p.text(pos, egui::Align2::LEFT_TOP, glyph, FontId::proportional(9.5 * z), color);
                any = true;
            }
        }
    }
    if any {
        let x = pr.left() + 24.0 * z;
        p.line_segment([pos2(x, top), pos2(x, top + lh * z)], Stroke::new(1.6 * z, Color32::from_rgb(0xd0, 0x2a, 0x2a).gamma_multiply(0.75)));
    }
}

/// UI commands of this module.
pub(crate) fn command(app: &mut ScriptApp, id: &str, p: &Value) -> Result<Value, scriptcraft_engine::CmdError> {
    match id {
        "ui.trackChanges" => app.drafts.changes = p.get("open").and_then(Value::as_bool).unwrap_or(!app.drafts.changes),
        "ui.compareWindow" => app.drafts.compare_open = p.get("open").and_then(Value::as_bool).unwrap_or(!app.drafts.compare_open),
        "ui.compare" => {
            if let Some(path) = p.get("path").and_then(Value::as_str) {
                return run_compare(app, json!({"path": path}));
            }
            if let Some(h) = &app.host {
                app.drafts.compare_pending = true;
                h.open(app.inbox.clone());
                return Ok(json!({"pending": "choose a draft"}));
            }
            let Some(path) = app.pick(false, "Compare with Draft", &scriptcraft_engine::io::OPEN_EXTENSIONS, "") else { return Ok(json!({})) };
            return run_compare(app, json!({"path": path}));
        }
        _ => {}
    }
    Ok(json!({}))
}

/// Compare and open the window with the result.
pub(crate) fn run_compare(app: &mut ScriptApp, params: Value) -> Result<Value, scriptcraft_engine::CmdError> {
    let r = app.execute("compare.with", params)?;
    app.drafts.compare = Some(r.clone());
    app.drafts.compare_open = true;
    Ok(r)
}

/// Tools › Track Changes and Compare Drafts.
pub(crate) fn tools_menu(app: &mut ScriptApp, ui: &mut Ui) {
    ui.separator();
    ui.menu_button("Track Changes", |ui| {
        let on = app.session.script.settings.track.on;
        if ui.add(egui::Button::selectable(on, "Track Changes").shortcut_text(crate::chrome::pretty_key("Mod+Shift+E"))).clicked() {
            app.run("track.toggle", json!({}));
            ui.close();
        }
        if ui.button("Show Changes…").clicked() {
            app.run("ui.trackChanges", json!({"open": true}));
            ui.close();
        }
        ui.separator();
        let has = track::has_changes(&app.session.script);
        for (label, id) in [
            ("Next Change", "track.next"),
            ("Previous Change", "track.previous"),
            ("Accept Change", "track.accept"),
            ("Reject Change", "track.reject"),
        ] {
            if ui.add_enabled(has, egui::Button::new(label)).clicked() {
                app.run(id, json!({}));
                ui.close();
            }
        }
        ui.separator();
        if ui.add_enabled(has, egui::Button::new("Accept All Changes")).clicked() {
            app.run("track.acceptAll", json!({}));
            ui.close();
        }
        if ui.add_enabled(has, egui::Button::new("Reject All Changes")).clicked() {
            app.run("track.rejectAll", json!({}));
            ui.close();
        }
    });
    if ui.button("Compare with Draft…").clicked() {
        app.run("ui.compare", json!({}));
        ui.close();
    }
}

fn kind_label(k: ChangeKind) -> &'static str {
    match k {
        ChangeKind::Insert => "Inserted",
        ChangeKind::Delete => "Deleted",
        ChangeKind::Break => "New paragraph",
        ChangeKind::Element => "Element",
    }
}

/// The Changes and Compare windows.
pub(crate) fn dialogs(app: &mut ScriptApp, ctx: &egui::Context) {
    if app.session.author.is_empty() && !app.prefs.editing.note_author.trim().is_empty() {
        app.session.author = app.prefs.editing.note_author.clone();
    }
    if app.drafts.changes {
        changes_window(app, ctx);
    }
    if app.drafts.compare_open && app.drafts.compare.is_some() {
        compare_window(app, ctx);
    }
}

fn changes_window(app: &mut ScriptApp, ctx: &egui::Context) {
    let mut open = true;
    let mut run: Vec<(&'static str, Value)> = Vec::new();
    let list = track::list(&app.session.script);
    let authors = track::authors(&app.session.script);
    let soft = app.tokens().text_soft;
    egui::Window::new("Track Changes")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .default_pos(ctx.content_rect().right_top() + vec2(-460.0, 110.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let mut on = app.session.script.settings.track.on;
                if ui.checkbox(&mut on, "Track changes").changed() {
                    run.push(("track.toggle", json!({"on": on})));
                }
                ui.label(RichText::new("as").color(soft));
                let mut name = if app.session.author.is_empty() { app.prefs.editing.note_author.clone() } else { app.session.author.clone() };
                if ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0).hint_text("Your name")).changed() {
                    app.prefs.editing.note_author = name.clone();
                    app.session.author = name;
                }
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("track-author").selected_text(app.drafts.only.clone().unwrap_or_else(|| "Everyone".into())).show_ui(
                    ui,
                    |ui| {
                        if ui.selectable_label(app.drafts.only.is_none(), "Everyone").clicked() {
                            app.drafts.only = None;
                        }
                        for a in &authors {
                            if ui.selectable_label(app.drafts.only.as_deref() == Some(a), a).clicked() {
                                app.drafts.only = Some(a.clone());
                            }
                        }
                    },
                );
                let by = app.drafts.only.clone().map_or(json!({}), |a| json!({"author": a}));
                if ui.add_enabled(!list.is_empty(), egui::Button::new("Accept All")).clicked() {
                    run.push(("track.acceptAll", by.clone()));
                }
                if ui.add_enabled(!list.is_empty(), egui::Button::new("Reject All")).clicked() {
                    run.push(("track.rejectAll", by));
                }
                if ui.add_enabled(!list.is_empty(), egui::Button::new("◀")).on_hover_text("Previous change").clicked() {
                    run.push(("track.previous", json!({})));
                }
                if ui.add_enabled(!list.is_empty(), egui::Button::new("▶")).on_hover_text("Next change").clicked() {
                    run.push(("track.next", json!({})));
                }
            });
            ui.separator();
            if list.is_empty() {
                ui.label(RichText::new("No pending changes.").italics().color(soft));
            }
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for c in list.iter().filter(|c| app.drafts.only.as_ref().is_none_or(|a| &c.author == a)) {
                    let color = author_color(&c.author);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 4.0, color);
                        ui.label(RichText::new(format!("{} · {}", c.author, kind_label(c.kind))).size(11.5).color(soft));
                        if ui.small_button("Accept").clicked() {
                            run.push(("track.accept", json!({"para": c.para, "index": c.index})));
                        }
                        if ui.small_button("Reject").clicked() {
                            run.push(("track.reject", json!({"para": c.para, "index": c.index})));
                        }
                    });
                    let shown: String = {
                        let t = c.text.replace('\n', " ¶ ");
                        if t.chars().count() > 160 { format!("{}…", t.chars().take(160).collect::<String>()) } else { t }
                    };
                    let rich = match c.kind {
                        ChangeKind::Delete => RichText::new(shown).strikethrough().color(Color32::from_rgb(0xc0, 0x30, 0x30)),
                        ChangeKind::Insert => RichText::new(shown).underline().color(color),
                        _ => RichText::new(shown).color(color),
                    };
                    if ui.add(egui::Label::new(rich.monospace()).sense(egui::Sense::click()).wrap()).on_hover_text("Go to this change").clicked() {
                        run.push(("select.range", json!({"anchor": {"para": c.para, "off": c.start}, "focus": {"para": c.para, "off": c.end}})));
                    }
                    ui.add_space(4.0);
                }
            });
        });
    app.drafts.changes = open;
    for (id, v) in run {
        app.run(id, v);
    }
}

/// Word pieces as one wrapped text: removed words struck through in red, added ones underlined
/// in green.
fn pieces_job(pieces: &[Value], ink: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(12.0);
    for pc in pieces {
        let text = pc["text"].as_str().unwrap_or("");
        let mut f = TextFormat { font_id: font.clone(), color: ink, ..Default::default() };
        match pc["op"].as_str() {
            Some("removed") => {
                f.color = Color32::from_rgb(0xc0, 0x30, 0x30);
                f.strikethrough = Stroke::new(1.0, f.color);
            }
            Some("added") => {
                f.color = Color32::from_rgb(0x1f, 0x8a, 0x3b);
                f.underline = Stroke::new(1.0, f.color);
            }
            _ => {}
        }
        job.append(text, 0.0, f);
    }
    job.wrap.max_width = 520.0;
    job
}

fn compare_window(app: &mut ScriptApp, ctx: &egui::Context) {
    let Some(r) = app.drafts.compare.clone() else { return };
    let mut open = true;
    let mut go: Option<usize> = None;
    let (soft, ink) = (app.tokens().text_soft, app.tokens().text);
    egui::Window::new("Compare Drafts")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .default_pos(ctx.content_rect().left_top() + vec2(420.0, 110.0))
        .show(ctx, |ui| {
            let other = r["other"].as_str().unwrap_or("the other draft");
            let older = r["otherIs"].as_str() != Some("newer");
            ui.label(
                RichText::new(if older {
                    format!("Changes since {other} (the older draft)")
                } else {
                    format!("Changes from this script to {other} (the newer draft)")
                })
                .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "{} changed · {} added · {} removed · {} moved · {} unchanged",
                    r["changed"], r["added"], r["removed"], r["moved"], r["unchanged"]
                ))
                .color(soft),
            );
            ui.separator();
            egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                let diffs = r["diffs"].as_array().cloned().unwrap_or_default();
                if diffs.is_empty() {
                    ui.label(RichText::new("The drafts are the same.").italics().color(soft));
                }
                for d in diffs {
                    let kind = d["kind"].as_str().unwrap_or("");
                    let (tag, color) = match kind {
                        "added" => ("ADDED", Color32::from_rgb(0x1f, 0x8a, 0x3b)),
                        "removed" => ("REMOVED", Color32::from_rgb(0xc0, 0x30, 0x30)),
                        "moved" => ("MOVED", Color32::from_rgb(0x7a, 0x4c, 0xd0)),
                        _ => ("CHANGED", Color32::from_rgb(0xc9, 0x6a, 0x12)),
                    };
                    let here = if older { d["new"].as_u64() } else { d["old"].as_u64() };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tag).size(10.5).strong().color(color));
                        let el = match d["oldElement"].as_str() {
                            Some(o) => format!("{o} to {}", d["element"].as_str().unwrap_or("")),
                            None => d["element"].as_str().unwrap_or("").to_string(),
                        };
                        ui.label(RichText::new(el).size(10.5).color(soft));
                        ui.label(RichText::new(d["scene"].as_str().unwrap_or("")).size(10.5).color(soft));
                    });
                    let resp = if let Some(pieces) = d["pieces"].as_array().filter(|v| !v.is_empty()) {
                        ui.add(egui::Label::new(pieces_job(pieces, ink)).sense(egui::Sense::click()))
                    } else {
                        let t = d["text"].as_str().unwrap_or("");
                        let mut rt = RichText::new(t).monospace();
                        if kind == "removed" {
                            rt = rt.strikethrough().color(color);
                        } else if kind == "added" {
                            rt = rt.underline().color(color);
                        }
                        ui.add(egui::Label::new(rt).wrap().sense(egui::Sense::click()))
                    };
                    if let Some(i) = here
                        && resp.on_hover_text("Go to this paragraph").clicked()
                    {
                        go = Some(i as usize);
                    }
                    ui.add_space(5.0);
                }
            });
        });
    app.drafts.compare_open = open;
    if let Some(i) = go {
        app.run("caret.set", json!({"para": i, "off": 0}));
    }
}
