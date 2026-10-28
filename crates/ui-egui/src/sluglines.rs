//! The Scene Heading Abbreviations window: the user's words for interior, exterior and both
//! (`INT.`, `ИНТ.`, `INNEN.` …) and times of day, from a language set and edited by hand. The
//! words live in the settings (`[sluglines]`) and in `Session::sluglines`.

use egui::{RichText, vec2};
use serde_json::json;

use crate::ScriptApp;

/// The window's state.
#[derive(Default, Clone, Debug)]
pub struct Dialog {
    pub open: bool,
    /// What is typed in each field (interior, exterior, both, times), comma-separated.
    text: [String; 4],
}

const FIELDS: [(&str, &str, &str); 4] = [
    ("interior", "Interior", "INT."),
    ("exterior", "Exterior", "EXT., EST."),
    ("both", "Interior and exterior", "INT./EXT., I/E."),
    ("times", "Times of day", "DAY, NIGHT"),
];

fn current(app: &ScriptApp, i: usize) -> String {
    let w = &app.session.sluglines;
    [&w.interior, &w.exterior, &w.both, &w.times].get(i).map_or(String::new(), |l| l.join(", "))
}

/// A hint naming the user's first interior and exterior words (`INT. or EXT.`).
pub fn example(app: &ScriptApp) -> (String, String) {
    let w = &app.session.sluglines;
    let first = |l: &Vec<String>, fallback: &str| l.first().cloned().unwrap_or_else(|| fallback.to_string());
    (first(&w.interior, "INT."), first(&w.exterior, "EXT."))
}

pub fn dialog(app: &mut ScriptApp, ctx: &egui::Context) {
    if !app.sluglines.open {
        return;
    }
    let lang = app.prefs.appearance.language;
    let mut open = true;
    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut text = std::mem::take(&mut app.sluglines.text);
    let mut editing = [false; 4];
    egui::Window::new(lang.text("Scene Heading Abbreviations"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(
                RichText::new(lang.text(
                    "A paragraph that starts with one of these words is a scene heading: typing, Fountain files, the navigator, SmartType and reports use them. Separate words with commas.",
                ))
                .size(11.5),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(lang.text("Language set"));
                egui::ComboBox::from_id_salt("sluglines-preset").selected_text(lang.text("Choose…")).show_ui(ui, |ui| {
                    if ui.selectable_label(false, lang.text("English and Russian (standard)")).clicked() {
                        changes.push(json!({"preset": "default"}));
                    }
                    for (code, name) in scriptcraft_doc::Sluglines::presets() {
                        if ui.selectable_label(false, name).clicked() {
                            changes.push(json!({"preset": code}));
                        }
                    }
                });
            });
            ui.add_space(6.0);
            egui::Grid::new("sluglines").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                for (i, (key, label, hint)) in FIELDS.iter().enumerate() {
                    ui.label(lang.text(label));
                    let Some(buf) = text.get_mut(i) else { continue };
                    let resp = ui.add(egui::TextEdit::singleline(buf).hint_text(*hint).desired_width(300.0));
                    if resp.lost_focus() {
                        changes.push(json!({ *key: buf.clone() }));
                    }
                    if let Some(e) = editing.get_mut(i) {
                        *e = resp.has_focus();
                    }
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            if ui.button(lang.text("Reset to Standard")).clicked() {
                changes.push(json!({"preset": "default"}));
            }
        });
    for c in changes {
        app.run("sluglines.set", c);
    }
    // Fields not being edited show the words in use (cleaned up: upper case, no duplicates).
    for (i, buf) in text.iter_mut().enumerate() {
        if !editing.get(i).copied().unwrap_or(false) {
            *buf = current(app, i);
        }
    }
    app.sluglines.text = text;
    app.sluglines.open = open;
}
