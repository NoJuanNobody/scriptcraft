//! Index cards: one card per scene with its heading and an editable summary.

use egui::{Align2, FontId, RichText, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;

use crate::chrome::elide;
use crate::{ScriptApp, View};

const CARD_W: f32 = 270.0;
const CARD_H: f32 = 190.0;

pub fn show(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    let scenes = app.session.script.scenes();
    let l = app.session.layout();
    let reports = scriptcraft_engine::reports::scenes(&app.session.script, &l);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(18.0);
        let avail = ui.available_width() - 36.0;
        let cols = ((avail + 18.0) / (CARD_W + 18.0)).floor().max(1.0) as usize;
        if scenes.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(RichText::new("No scenes yet: add a scene heading (INT. / EXT.) in the page view.").color(t.text_soft));
            });
        }
        let mut actions: Vec<(&str, serde_json::Value)> = Vec::new();
        for row in scenes.chunks(cols) {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.spacing_mut().item_spacing.x = 18.0;
                for sc in row {
                    let (r, _) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), egui::Sense::hover());
                    let p = ui.painter();
                    let shadow = egui::epaint::Shadow {
                        offset: [0, 2],
                        blur: 10,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(if t.dark { 110 } else { 34 }),
                    };
                    p.add(shadow.as_shape(r, egui::CornerRadius::same(6)));
                    p.rect_filled(r, 6.0, t.card);
                    let band = t.card_band[sc.index % t.card_band.len()];
                    let top = egui::Rect::from_min_size(r.min, vec2(r.width(), 6.0));
                    p.rect_filled(top, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, band);
                    p.rect_stroke(r, 6.0, Stroke::new(0.7, t.paper_edge), StrokeKind::Inside);
                    let num = sc.number.clone().unwrap_or_else(|| (sc.index + 1).to_string());
                    p.text(r.left_top() + vec2(12.0, 16.0), Align2::LEFT_TOP, num, FontId::monospace(13.0), band);
                    let info = reports
                        .get(sc.index)
                        .map(|x| format!("p.{} · {}", x.page, scriptcraft_engine::reports::eighths_label(x.eighths)))
                        .unwrap_or_default();
                    p.text(r.right_top() + vec2(-12.0, 17.0), Align2::RIGHT_TOP, info, FontId::proportional(11.0), t.text_soft);
                    p.text(
                        r.left_top() + vec2(12.0, 38.0),
                        Align2::LEFT_TOP,
                        elide(&sc.heading, 30),
                        FontId::new(13.0, egui::FontFamily::Name("courier-bold".into())),
                        t.ink,
                    );
                    let cast = reports.get(sc.index).map(|x| x.characters.join(", ")).unwrap_or_default();
                    if !cast.is_empty() {
                        p.text(r.left_bottom() + vec2(12.0, -10.0), Align2::LEFT_BOTTOM, elide(&cast, 38), FontId::proportional(11.0), t.text_soft);
                    }
                    // Summary editor.
                    let body = egui::Rect::from_min_max(r.min + vec2(10.0, 60.0), r.max - vec2(10.0, 30.0));
                    let mut text = sc.summary.clone();
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
                    let edit = child.add(
                        egui::TextEdit::multiline(&mut text)
                            .id_salt(("card", sc.index))
                            .frame(egui::Frame::NONE)
                            .hint_text("Summary…")
                            .desired_width(body.width())
                            .font(FontId::proportional(13.0)),
                    );
                    if edit.changed() {
                        actions.push(("scene.setSummary", json!({"index": sc.index, "summary": text})));
                    }
                    // Move left / right, go to scene.
                    let br = egui::Rect::from_min_size(r.right_bottom() + vec2(-78.0, -26.0), vec2(70.0, 20.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(br));
                    child.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if ui.add_enabled(sc.index > 0, egui::Button::new("◀").small().frame(false)).on_hover_text("Move earlier").clicked() {
                            actions.push(("scene.move", json!({"from": sc.index, "to": sc.index - 1})));
                        }
                        if ui.add(egui::Button::new("↗").small().frame(false)).on_hover_text("Go to scene").clicked() {
                            actions.push(("scene.goto", json!({"index": sc.index})));
                        }
                        if ui
                            .add_enabled(sc.index + 1 < scenes.len(), egui::Button::new("▶").small().frame(false))
                            .on_hover_text("Move later")
                            .clicked()
                        {
                            actions.push(("scene.move", json!({"from": sc.index, "to": sc.index + 1})));
                        }
                    });
                }
            });
            ui.add_space(18.0);
        }
        for (id, p) in actions {
            app.run(id, p);
            if id == "scene.goto" {
                app.view = View::Page;
            }
        }
    });
}
