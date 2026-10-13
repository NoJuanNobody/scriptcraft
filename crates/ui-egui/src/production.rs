//! Production dialogs: Page Setup (paper, headers and footers).

use egui::{RichText, vec2};
use serde_json::json;

use crate::ScriptApp;

/// Which production dialogs are open.
#[derive(Default, Clone, Debug)]
pub struct Dialogs {
    pub page_setup: bool,
}

pub fn dialogs(app: &mut ScriptApp, ctx: &egui::Context) {
    if app.production.page_setup {
        page_setup(app, ctx);
    }
}

fn page_setup(app: &mut ScriptApp, ctx: &egui::Context) {
    let mut open = true;
    let pg = app.session.script.settings.page.clone();
    let title_page = app.session.script.settings.title_page;
    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut reset = false;
    egui::Window::new("Page Setup")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Paper");
                for p in scriptcraft_doc::Paper::ALL {
                    if ui.selectable_label(pg.paper == p, p.name()).clicked() && pg.paper != p {
                        changes.push(json!({"paper": p.id()}));
                    }
                }
                ui.label(RichText::new(format!("{} lines per page", scriptcraft_layout::body_rows(pg.paper))).size(11.5).weak());
            });
            ui.add_space(6.0);
            egui::Grid::new("hf").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("");
                for h in ["Left", "Centre", "Right"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (what, hf) in [("header", &pg.header), ("footer", &pg.footer)] {
                    ui.label(if what == "header" { "Header" } else { "Footer" });
                    for (slot, text) in [("left", &hf.left), ("center", &hf.center), ("right", &hf.right)] {
                        let mut v = text.clone();
                        if ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut v).hint_text("—")).changed() {
                            changes.push(json!({what: {slot: v}}));
                        }
                    }
                    ui.end_row();
                }
            });
            ui.label(RichText::new("Fields: {page} {pages} {title} {author} {date} {revision} {revname} {revdate}").size(11.5).weak());
            ui.add_space(6.0);
            for (label, key, on) in [
                ("Header on the first page", "headerFirstPage", pg.header_first_page),
                ("Footer on the first page", "footerFirstPage", pg.footer_first_page),
                ("Header and footer on the title page", "headerOnTitlePage", pg.header_on_title_page),
                ("Print the title page", "titlePage", title_page),
            ] {
                let mut v = on;
                if ui.checkbox(&mut v, label).changed() {
                    changes.push(json!({key: v}));
                }
            }
            ui.add_space(6.0);
            if ui.button("Reset Header and Footer").clicked() {
                reset = true;
            }
        });
    for c in changes {
        app.run("page.set", c);
    }
    if reset {
        app.run("page.reset", json!({}));
    }
    app.production.page_setup = open;
}
