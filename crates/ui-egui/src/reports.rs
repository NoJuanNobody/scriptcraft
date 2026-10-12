//! Reports view: characters, scenes and locations.

use egui::{RichText, Ui};
use scriptcraft_engine::reports;
use serde_json::json;

use crate::{ScriptApp, View};

pub fn show(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    let l = app.session.layout();
    let chars = reports::characters(&app.session.script, &l);
    let scenes = reports::scenes(&app.session.script, &l);
    let locs = reports::locations(&app.session.script);
    let mut goto = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.fill(t.panel).inner_margin(24.0).corner_radius(8.0).outer_margin(18.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.heading("Reports");
            ui.label(
                RichText::new(format!(
                    "{} pages · {} scenes · {} speaking characters · {} words",
                    l.script_pages(),
                    scenes.len(),
                    chars.len(),
                    app.session.script.word_count()
                ))
                .color(t.text_soft),
            );
            ui.add_space(14.0);
            ui.label(RichText::new("CHARACTERS").strong().size(12.0).color(t.accent));
            egui::Grid::new("chars").striped(true).num_columns(5).spacing([24.0, 6.0]).show(ui, |ui| {
                for h in ["Character", "Speeches", "Words", "Scenes", "First page"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for c in &chars {
                    ui.label(&c.name);
                    ui.label(c.speeches.to_string());
                    ui.label(c.words.to_string());
                    ui.label(c.scenes.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", "));
                    ui.label(c.first_page.to_string());
                    ui.end_row();
                }
            });
            ui.add_space(18.0);
            ui.label(RichText::new("SCENES").strong().size(12.0).color(t.accent));
            egui::Grid::new("scenes").striped(true).num_columns(5).spacing([24.0, 6.0]).show(ui, |ui| {
                for h in ["#", "Scene heading", "Page", "Length", "Characters"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for s in &scenes {
                    ui.label(s.number.clone().unwrap_or_else(|| s.scene.to_string()));
                    if ui.link(&s.heading).clicked() {
                        goto = Some(s.scene - 1);
                    }
                    ui.label(s.page.to_string());
                    ui.label(reports::eighths_label(s.eighths));
                    ui.label(s.characters.join(", "));
                    ui.end_row();
                }
            });
            ui.add_space(18.0);
            ui.label(RichText::new("LOCATIONS").strong().size(12.0).color(t.accent));
            egui::Grid::new("locs").striped(true).num_columns(3).spacing([24.0, 6.0]).show(ui, |ui| {
                for h in ["Location", "Scenes", "Int / Ext"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for x in &locs {
                    ui.label(&x.location);
                    ui.label(x.scenes.to_string());
                    ui.label(match (x.interior, x.exterior) {
                        (true, true) => "INT/EXT",
                        (true, false) => "INT",
                        (false, true) => "EXT",
                        _ => "",
                    });
                    ui.end_row();
                }
            });
        });
    });
    if let Some(i) = goto {
        app.run("scene.goto", json!({"index": i}));
        app.view = View::Page;
    }
}
