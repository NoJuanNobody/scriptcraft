//! Drafts: File › New from Template, Format › Apply Template, track changes and comparing
//! drafts.

use egui::Ui;
use scriptcraft_doc::Template;
use serde_json::json;

use crate::ScriptApp;

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
