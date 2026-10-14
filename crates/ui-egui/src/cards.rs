//! Index cards: one card per scene with its heading, an editable summary, colour and tags.
//! Drag a card onto another to move the scene there; right-click for colour and tags.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use scriptcraft_doc::CardColor;
use serde_json::json;

use crate::ScriptApp;
use crate::chrome::elide;

const CARD_W: f32 = 270.0;
const CARD_H: f32 = 206.0;

pub fn card_rgb(c: CardColor) -> Color32 {
    let [r, g, b] = c.rgb();
    Color32::from_rgb(r, g, b)
}

pub fn show(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    let scenes = app.session.script.scenes();
    let l = app.session.layout();
    let reports = scriptcraft_engine::reports::scenes(&app.session.script, &l);
    let mut actions: Vec<(&str, serde_json::Value)> = Vec::new();
    let mut rects: Vec<Rect> = Vec::new();
    let mut drop_from: Option<usize> = None;
    let pointer = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
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
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("Drag a card to move its scene · right-click for colour and tags").size(11.5).color(t.text_soft));
        });
        ui.add_space(8.0);
        for row in scenes.chunks(cols) {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.spacing_mut().item_spacing.x = 18.0;
                for sc in row {
                    let (r, _) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::hover());
                    rects.push(r);
                    let resp = ui.interact(r, ui.id().with(("card", sc.index)), Sense::click_and_drag());
                    if resp.drag_started() {
                        app.card_drag = Some(sc.index);
                    }
                    if resp.drag_stopped() {
                        drop_from = app.card_drag.take();
                    }
                    if resp.dragged() || app.card_drag == Some(sc.index) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                    let dragging_this = app.card_drag == Some(sc.index);
                    let p = ui.painter();
                    let shadow =
                        egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if t.dark { 110 } else { 34 }) };
                    p.add(shadow.as_shape(r, egui::CornerRadius::same(6)));
                    p.rect_filled(r, 6.0, if dragging_this { t.card.gamma_multiply(0.6) } else { t.card });
                    let band = sc.color.map(card_rgb).unwrap_or(t.text_soft.gamma_multiply(0.55));
                    let top = Rect::from_min_size(r.min, vec2(r.width(), if sc.color.is_some() { 10.0 } else { 6.0 }));
                    p.rect_filled(top, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, band);
                    // Drop target highlight.
                    let target = app.card_drag.is_some_and(|d| d != sc.index) && pointer.is_some_and(|q| r.contains(q));
                    let stroke = if target { Stroke::new(2.5, t.accent) } else { Stroke::new(0.7, t.paper_edge) };
                    p.rect_stroke(r, 6.0, stroke, StrokeKind::Inside);
                    let num = sc.number.clone().unwrap_or_else(|| (sc.index + 1).to_string());
                    p.text(r.left_top() + vec2(12.0, 18.0), Align2::LEFT_TOP, num, FontId::monospace(13.0), band);
                    let info = reports
                        .get(sc.index)
                        .map(|x| format!("p.{} · {}", x.page, scriptcraft_engine::reports::eighths_label(x.eighths)))
                        .unwrap_or_default();
                    p.text(r.right_top() + vec2(-12.0, 19.0), Align2::RIGHT_TOP, info, FontId::proportional(11.0), t.text_soft);
                    p.text(
                        r.left_top() + vec2(12.0, 40.0),
                        Align2::LEFT_TOP,
                        elide(&sc.heading, 30),
                        FontId::new(13.0, egui::FontFamily::Name("courier-bold".into())),
                        t.ink,
                    );
                    // Tags as chips.
                    let mut x = r.left() + 12.0;
                    let y = r.bottom() - 52.0;
                    for tag in sc.tags.iter().take(4) {
                        let g = p.layout_no_wrap(elide(tag, 16), FontId::proportional(10.5), t.text);
                        let w = g.size().x + 12.0;
                        if x + w > r.right() - 12.0 {
                            break;
                        }
                        let chip = Rect::from_min_size(egui::pos2(x, y), vec2(w, 17.0));
                        p.rect_filled(chip, 8.5, band.gamma_multiply(0.22));
                        p.galley(chip.min + vec2(6.0, 2.0), g, t.text);
                        x += w + 4.0;
                    }
                    let cast = reports.get(sc.index).map(|x| x.characters.join(", ")).unwrap_or_default();
                    if !cast.is_empty() {
                        p.text(r.left_bottom() + vec2(12.0, -10.0), Align2::LEFT_BOTTOM, elide(&cast, 34), FontId::proportional(11.0), t.text_soft);
                    }
                    // Summary editor.
                    let body = Rect::from_min_max(r.min + vec2(10.0, 62.0), egui::pos2(r.right() - 10.0, y - 4.0));
                    let mut text = sc.summary.clone();
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
                    let edit = child.add(
                        egui::TextEdit::multiline(&mut text)
                            .id_salt(("card-summary", sc.index))
                            .frame(egui::Frame::NONE)
                            .hint_text("Summary…")
                            .desired_width(body.width())
                            .desired_rows(4)
                            .font(FontId::proportional(13.0)),
                    );
                    if edit.changed() {
                        actions.push(("scene.setSummary", json!({"index": sc.index, "summary": text})));
                    }
                    // Go to scene.
                    let br = Rect::from_min_size(r.right_bottom() + vec2(-34.0, -26.0), vec2(26.0, 20.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(br));
                    if child.add(egui::Button::new("↗").small().frame(false)).on_hover_text("Go to scene").clicked() {
                        actions.push(("scene.goto", json!({"index": sc.index})));
                    }
                    resp.context_menu(|ui| card_menu(app, ui, sc.index, sc.color, &sc.tags, &mut actions));
                }
            });
            ui.add_space(18.0);
        }
    });
    app.card_rects = rects.clone();
    // Drop: the scene moves to the card under the pointer.
    if let (Some(from), Some(q)) = (drop_from, pointer)
        && let Some(to) = rects.iter().position(|r| r.contains(q))
        && to != from
    {
        actions.push(("scene.move", json!({"from": from, "to": to})));
    }
    if !ui.input(|i| i.pointer.any_down()) && drop_from.is_none() {
        app.card_drag = None;
    }
    for (id, p) in actions {
        app.run(id, p);
        if id == "scene.goto" {
            crate::split::show_page(app);
        }
    }
}

fn card_menu(
    app: &mut ScriptApp,
    ui: &mut Ui,
    index: usize,
    color: Option<CardColor>,
    tags: &[String],
    actions: &mut Vec<(&'static str, serde_json::Value)>,
) {
    ui.label(RichText::new("Colour").strong());
    ui.horizontal(|ui| {
        for c in CardColor::ALL {
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            ui.painter().circle_filled(r.center(), 8.0, card_rgb(c));
            if color == Some(c) {
                ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.5, ui.visuals().text_color()));
            }
            if resp.on_hover_text(c.id()).clicked() {
                actions.push(("scene.setColor", json!({"index": index, "color": c.id()})));
                ui.close();
            }
        }
    });
    if ui.button("No colour").clicked() {
        actions.push(("scene.setColor", json!({"index": index, "color": null})));
        ui.close();
    }
    ui.separator();
    ui.label(RichText::new("Tags (comma separated)").strong());
    let buf = app.tag_edit.get_or_insert_with(|| (index, tags.join(", ")));
    if buf.0 != index {
        *buf = (index, tags.join(", "));
    }
    let r = ui.add(egui::TextEdit::singleline(&mut buf.1).desired_width(200.0));
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if ui.button("Apply tags").clicked() || enter {
        actions.push(("scene.setTags", json!({"index": index, "tags": buf.1})));
        app.tag_edit = None;
        ui.close();
    }
    ui.separator();
    if ui.button("Go to scene").clicked() {
        actions.push(("scene.goto", json!({"index": index})));
        ui.close();
    }
}
