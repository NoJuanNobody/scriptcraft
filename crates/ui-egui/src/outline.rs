//! The outline view: acts, sequences and sections with the scenes under them, in order. Drag a
//! row (by its grip) to rearrange: a scene keeps its text and lands under the point above it; a
//! point re-groups the scenes after it. Titles and summaries are edited in place.
//!
//! [`show`] draws into any `Ui`, so the view can sit in a split-screen pane as well as the
//! main area. The navigator's structure rows come from [`nav_points`]. The rows' screen
//! rectangles of the last frame are in egui temp data under [`rows_id`].

use egui::{Align2, Color32, FontId, Id, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use scriptcraft_doc::{CardColor, StructureKind, StructurePoint};
use serde_json::{Value, json};

use crate::chrome::elide;
use crate::{ScriptApp, View};

const ROW_H: f32 = 34.0;
const POINT_H: f32 = 44.0;
const INDENT: f32 = 26.0;

/// Colour of a structure point: its own, else one per level.
pub fn point_color(sp: &StructurePoint, accent: Color32) -> Color32 {
    match sp.color {
        Some(c) => crate::cards::card_rgb(c),
        None => match sp.kind {
            StructureKind::Act => accent,
            StructureKind::Sequence => accent.gamma_multiply(0.7),
            StructureKind::Section => accent.gamma_multiply(0.45),
        },
    }
}

/// Egui temp-data id of the outline rows' rectangles (`Vec<Rect>`) drawn last frame.
pub fn rows_id() -> Id {
    Id::new("outline-rows")
}

/// The drag in progress: the outline position being dragged.
fn drag_id() -> Id {
    Id::new("outline-drag")
}

pub fn show(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    let outline = scriptcraft_engine::cmd::structure::outline_json(&mut app.session);
    let items: Vec<Value> = outline["items"].as_array().cloned().unwrap_or_default();
    let mut actions: Vec<(&'static str, Value)> = Vec::new();
    let mut rows: Vec<Rect> = Vec::new();
    let dragging: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_id()));
    let pointer = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
    let released = ui.input(|i| i.pointer.any_released());
    let cur_scene = app.session.script.scene_of(app.session.sel.focus.para);
    egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("outline-scroll").show(ui, |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("Outline").size(18.0).strong().color(t.text));
            ui.add_space(12.0);
            for k in StructureKind::ALL {
                if ui
                    .button(format!("+ {}", k.label()))
                    .on_hover_text(format!("Add an {} before the current scene", k.label().to_lowercase()))
                    .clicked()
                {
                    actions.push(("structure.add", json!({"kind": k.id(), "title": default_title(app, k)})));
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Drag a row by its grip to rearrange · a scene goes under the act or sequence above it · click a scene to open it")
                    .size(11.5)
                    .color(t.text_soft),
            );
        });
        ui.add_space(10.0);
        if items.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(RichText::new("No scenes yet: add a scene heading (INT. / EXT.) in the page view.").color(t.text_soft));
            });
        }
        let width = (ui.available_width() - 40.0).clamp(320.0, 980.0);
        for (pos, it) in items.iter().enumerate() {
            let is_point = it["type"] == "point";
            let depth = it["depth"].as_u64().unwrap_or(0) as f32;
            let h = if is_point {
                POINT_H
            } else if it["summary"].as_str().is_some_and(|s| !s.is_empty()) {
                ROW_H + 14.0
            } else {
                ROW_H
            };
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let (full, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
                rows.push(full);
                let r = Rect::from_min_max(pos2(full.left() + depth * INDENT, full.top()), full.max);
                let grip = Rect::from_min_size(r.min + vec2(2.0, 0.0), vec2(18.0, r.height()));
                let gresp = ui.interact(grip, Id::new(("outline-grip", pos)), Sense::drag());
                if gresp.drag_started() {
                    ui.ctx().data_mut(|d| d.insert_temp(drag_id(), pos));
                }
                if gresp.hovered() || dragging == Some(pos) {
                    ui.ctx().set_cursor_icon(if dragging.is_some() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
                }
                let p = ui.painter();
                let faded = dragging == Some(pos);
                // Grip dots.
                for k in 0..3 {
                    for j in 0..2 {
                        let c = grip.center() + vec2(-3.0 + j as f32 * 6.0, -6.0 + k as f32 * 6.0);
                        p.circle_filled(c, 1.4, t.text_soft.gamma_multiply(0.7));
                    }
                }
                let body = Rect::from_min_max(pos2(grip.right() + 4.0, r.top() + 2.0), pos2(r.right(), r.bottom() - 2.0));
                if is_point {
                    let index = it["index"].as_u64().unwrap_or(0) as usize;
                    let Some(sp) = app.session.script.structure_point(index).cloned() else { return };
                    let col = point_color(&sp, t.accent);
                    p.rect_filled(body, 6.0, col.gamma_multiply(if faded { 0.08 } else { 0.16 }));
                    p.rect_filled(Rect::from_min_size(body.min, vec2(4.0, body.height())), 2.0, col);
                    p.text(
                        body.left_top() + vec2(12.0, 5.0),
                        Align2::LEFT_TOP,
                        sp.kind.label().to_uppercase(),
                        FontId::proportional(10.0),
                        t.text_soft,
                    );
                    if let Some(pg) = it["page"].as_str() {
                        p.text(body.right_top() + vec2(-40.0, 5.0), Align2::RIGHT_TOP, format!("p.{pg}"), FontId::proportional(11.0), t.text_soft);
                    }
                    // Title and summary, edited in place.
                    let size = match sp.kind {
                        StructureKind::Act => 16.0,
                        StructureKind::Sequence => 14.5,
                        StructureKind::Section => 13.5,
                    };
                    let mut title = sp.title.clone();
                    let tr =
                        Rect::from_min_max(body.left_top() + vec2(10.0, 17.0), pos2(body.left() + (body.width() * 0.42).max(180.0), body.bottom()));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tr));
                    let te = child.add(
                        egui::TextEdit::singleline(&mut title)
                            .id_salt(("outline-title", index))
                            .frame(egui::Frame::NONE)
                            .font(FontId::proportional(size))
                            .text_color(t.text)
                            .desired_width(tr.width()),
                    );
                    if te.changed() {
                        actions.push(("structure.edit", json!({"index": index, "title": title})));
                    }
                    let mut summary = sp.summary.clone();
                    let sr = Rect::from_min_max(pos2(tr.right() + 10.0, body.top() + 6.0), pos2(body.right() - 78.0, body.bottom() - 2.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(sr));
                    let se = child.add(
                        egui::TextEdit::singleline(&mut summary)
                            .id_salt(("outline-summary", index))
                            .hint_text("Summary…")
                            .frame(egui::Frame::NONE)
                            .font(FontId::proportional(12.5))
                            .text_color(t.text_soft)
                            .desired_width(sr.width()),
                    );
                    if se.changed() {
                        actions.push(("structure.edit", json!({"index": index, "summary": summary})));
                    }
                    // Colour and delete.
                    let mr = Rect::from_min_size(body.right_top() + vec2(-30.0, 10.0), vec2(24.0, 22.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(mr));
                    child.menu_button("…", |ui| point_menu(ui, index, &sp, &mut actions));
                } else {
                    let index = it["index"].as_u64().unwrap_or(0) as usize;
                    let on = cur_scene == Some(index);
                    let resp = ui.interact(body, Id::new(("outline-scene", pos)), Sense::click());
                    let fill = if faded {
                        t.card.gamma_multiply(0.5)
                    } else if on {
                        t.accent.gamma_multiply(0.12)
                    } else if resp.hovered() {
                        ui.visuals().widgets.hovered.weak_bg_fill
                    } else {
                        t.card
                    };
                    let p = ui.painter();
                    p.rect_filled(body, 5.0, fill);
                    p.rect_stroke(body, 5.0, Stroke::new(0.6, t.paper_edge), egui::StrokeKind::Inside);
                    if let Some(c) = it["color"].as_str().and_then(CardColor::parse) {
                        p.rect_filled(Rect::from_min_size(body.min + vec2(1.0, 4.0), vec2(3.0, body.height() - 8.0)), 1.5, crate::cards::card_rgb(c));
                    }
                    let num = it["number"].as_str().map(str::to_string).unwrap_or_else(|| (index + 1).to_string());
                    p.text(
                        body.left_top() + vec2(12.0, 9.0),
                        Align2::LEFT_TOP,
                        num,
                        FontId::monospace(12.0),
                        if on { t.accent } else { t.text_soft },
                    );
                    let heading = it["heading"].as_str().unwrap_or("");
                    p.text(
                        body.left_top() + vec2(44.0, 8.0),
                        Align2::LEFT_TOP,
                        elide(heading, ((body.width() - 120.0) / 7.6) as usize),
                        FontId::new(13.0, egui::FontFamily::Name("courier-bold".into())),
                        t.ink,
                    );
                    if let Some(s) = it["summary"].as_str().filter(|s| !s.is_empty()) {
                        p.text(
                            body.left_top() + vec2(44.0, 26.0),
                            Align2::LEFT_TOP,
                            elide(s, ((body.width() - 120.0) / 6.0) as usize),
                            FontId::proportional(11.5),
                            t.text_soft,
                        );
                    }
                    p.text(
                        body.right_top() + vec2(-12.0, 9.0),
                        Align2::RIGHT_TOP,
                        format!("p.{}", it["page"].as_str().unwrap_or("")),
                        FontId::proportional(11.0),
                        t.text_soft,
                    );
                    if resp.clicked() {
                        actions.push(("scene.goto", json!({"index": index})));
                    }
                }
            });
            ui.add_space(4.0);
        }
        ui.add_space(30.0);
    });
    // Row rectangles, for demos and tests.
    ui.ctx().data_mut(|d| d.insert_temp(rows_id(), rows.clone()));
    // Where a drop would land: the gap nearest the pointer (0 = before the first row).
    let gap = |q: egui::Pos2| rows.iter().position(|r| q.y < r.center().y).unwrap_or(rows.len());
    if let (Some(from), Some(q)) = (dragging, pointer) {
        let g = gap(q);
        let y = rows.get(g).map_or_else(|| rows.last().map_or(0.0, |r| r.bottom() + 2.0), |r| r.top() - 2.0);
        if let Some(first) = rows.first() {
            ui.painter().line_segment([pos2(first.left(), y), pos2(first.right(), y)], Stroke::new(2.5, t.accent));
        }
        if released {
            let to = if g > from { g - 1 } else { g };
            if to != from {
                actions.push(("outline.move", json!({"from": from, "to": to})));
            }
            ui.ctx().data_mut(|d| d.remove::<usize>(drag_id()));
        }
    }
    if dragging.is_some() && !ui.input(|i| i.pointer.any_down()) {
        ui.ctx().data_mut(|d| d.remove::<usize>(drag_id()));
    }
    for (id, p) in actions {
        app.run(id, p);
        if id == "scene.goto" {
            app.view = View::Page;
        }
    }
}

/// A title for a new point: `Act Two` after one act, and so on.
fn default_title(app: &ScriptApp, k: StructureKind) -> String {
    let n = app.session.script.structure_points().iter().filter(|(_, p)| p.kind == k).count() + 1;
    const WORDS: [&str; 10] = ["One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten"];
    match WORDS.get(n - 1) {
        Some(w) if k == StructureKind::Act => format!("Act {w}"),
        _ => format!("{} {n}", k.label()),
    }
}

fn point_menu(ui: &mut Ui, index: usize, sp: &StructurePoint, actions: &mut Vec<(&'static str, Value)>) {
    ui.label(RichText::new("Level").strong());
    for k in StructureKind::ALL {
        if ui.radio(sp.kind == k, k.label()).clicked() {
            actions.push(("structure.edit", json!({"index": index, "kind": k.id()})));
            ui.close();
        }
    }
    ui.separator();
    ui.label(RichText::new("Colour").strong());
    ui.horizontal(|ui| {
        for c in CardColor::ALL {
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            ui.painter().circle_filled(r.center(), 8.0, crate::cards::card_rgb(c));
            if sp.color == Some(c) {
                ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.5, ui.visuals().text_color()));
            }
            if resp.on_hover_text(c.id()).clicked() {
                actions.push(("structure.edit", json!({"index": index, "color": c.id()})));
                ui.close();
            }
        }
    });
    if ui.button("No colour").clicked() {
        actions.push(("structure.edit", json!({"index": index, "color": null})));
        ui.close();
    }
    ui.separator();
    if ui.button("Go to").clicked() {
        actions.push(("structure.goto", json!({"index": index})));
        ui.close();
    }
    if ui.button(format!("Delete {}", sp.kind.label())).clicked() {
        actions.push(("structure.delete", json!({"index": index})));
        ui.close();
    }
}

/// Navigator rows for the structure points stored before a scene (`first` is the reading-order
/// index of the first one). Clicking one goes there.
pub fn nav_points(app: &mut ScriptApp, ui: &mut Ui, points: &[StructurePoint], first: usize) {
    let t = app.tokens();
    for (k, sp) in points.iter().enumerate() {
        let depth = (sp.kind.level() - 1) as f32;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
        let col = point_color(sp, t.accent);
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let x = r.left() + 4.0 + depth * 12.0;
        ui.painter().rect_filled(Rect::from_min_size(pos2(x, r.top() + 5.0), vec2(3.0, 12.0)), 1.5, col);
        let size = if sp.kind == StructureKind::Act { 12.5 } else { 11.5 };
        ui.painter().text(
            pos2(x + 8.0, r.center().y),
            Align2::LEFT_CENTER,
            elide(&sp.title.to_uppercase(), ((r.right() - x - 16.0) / 7.0) as usize),
            FontId::proportional(size),
            if sp.kind == StructureKind::Act { t.text } else { t.text_soft },
        );
        if resp.clicked() {
            app.run("structure.goto", json!({"index": first + k}));
            app.view = View::Page;
        }
    }
}

/// On the page: a non-printing label above a scene heading that opens acts, sequences or
/// sections (`ACT TWO › THE CALL`), in the points' colour.
pub fn paint_marker(app: &ScriptApp, p: &egui::Painter, line: &scriptcraft_layout::Line, pr: Rect, z: f32) {
    if line.kind != scriptcraft_layout::LineKind::Body || line.start != 0 {
        return;
    }
    let Some(para) = line.para.and_then(|i| app.session.script.para(i)) else { return };
    let Some(first) = para.structure.first() else { return };
    let t = app.tokens();
    let label = para.structure.iter().map(|sp| sp.title.to_uppercase()).collect::<Vec<_>>().join("  ›  ");
    let at = pos2(pr.left() + 108.0 * z, pr.top() + (line.y - 10.5) * z);
    let col = point_color(first, t.accent);
    let g = p.layout_no_wrap(elide(&label, 70), FontId::proportional(9.5 * z), col);
    let chip = Rect::from_min_size(at + vec2(-4.0 * z, -z), g.size() + vec2(8.0 * z, 2.0 * z));
    p.rect_filled(chip, 3.0 * z, col.gamma_multiply(0.12));
    p.galley(at, g, col);
}
