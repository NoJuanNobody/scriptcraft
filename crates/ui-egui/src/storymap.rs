//! The story map: a horizontal strip of the script scaled to its pages. Acts, sequences and
//! sections run as bands above the scenes; a ruler marks the pages; the current scene is
//! outlined. Click a scene or a band to go there.
//!
//! [`strip`] draws into any `Ui` (a split-screen pane, for instance); [`panel`] docks it at the
//! bottom of the window when View › Story Map is on.

use egui::{Align2, Color32, FontId, Id, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use scriptcraft_doc::StructureKind;
use serde_json::json;

use crate::chrome::elide;
use crate::{ScriptApp, View};

/// Height of the strip, points.
pub const HEIGHT: f32 = 118.0;
const BAND_H: f32 = 17.0;

/// The story map docked at the bottom of the window.
pub fn panel(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    egui::Panel::bottom("storymap")
        .resizable(false)
        .exact_size(HEIGHT + 28.0)
        .frame(
            egui::Frame::NONE
                .fill(t.panel)
                .inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 6 })
                .stroke(Stroke::new(1.0, t.chrome_line)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("STORY MAP").size(11.0).strong().color(t.text_soft));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Hide the story map").clicked() {
                        app.run("ui.toggleStoryMap", json!({}));
                    }
                });
            });
            strip(app, ui);
        });
}

/// The map itself, filling the width available.
pub fn strip(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    let l = app.session.layout();
    let map = scriptcraft_engine::storymap::story_map(&app.session.script, &l);
    let cur = app.session.script.scene_of(app.session.sel.focus.para);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
    let p = ui.painter_at(r);
    let pages = map.pages.max(1.0);
    let x_of = |at: f32| r.left() + (at / pages).clamp(0.0, 1.0) * r.width();
    let mut go: Option<(&'static str, serde_json::Value)> = None;
    // Structure bands, one row per level.
    for sp in &map.points {
        let lvl = sp.kind.level() as f32 - 1.0;
        let y = r.top() + lvl * (BAND_H + 2.0);
        let band = Rect::from_min_max(pos2(x_of(sp.at), y), pos2(x_of(sp.end).max(x_of(sp.at) + 3.0), y + BAND_H));
        let col = app.session.script.structure_point(sp.index).map_or(t.accent, |x| crate::outline::point_color(x, t.accent));
        let resp = ui.interact(band, Id::new(("storymap-point", sp.index)), Sense::click());
        p.rect_filled(band.shrink2(vec2(0.5, 0.0)), 3.0, col.gamma_multiply(if resp.hovered() { 0.55 } else { 0.32 }));
        p.rect_filled(Rect::from_min_size(band.min, vec2(2.5, BAND_H)), 1.0, col);
        let size = if sp.kind == StructureKind::Act { 11.5 } else { 10.5 };
        let fits = ((band.width() - 8.0) / (size * 0.6)).max(0.0) as usize;
        if fits >= 3 {
            p.text(band.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, elide(&sp.title, fits), FontId::proportional(size), t.text);
        }
        if resp.on_hover_text(format!("{} · {}", sp.kind.label(), sp.title)).clicked() {
            go = Some(("structure.goto", json!({"index": sp.index})));
        }
    }
    // Scenes.
    let top = r.top() + 3.0 * (BAND_H + 2.0) + 4.0;
    let scene_h = 30.0;
    for sc in &map.scenes {
        let block = Rect::from_min_max(pos2(x_of(sc.start), top), pos2(x_of(sc.end).max(x_of(sc.start) + 2.0), top + scene_h));
        let resp = ui.interact(block, Id::new(("storymap-scene", sc.index)), Sense::click());
        let base = sc.color.map(crate::cards::card_rgb).unwrap_or(if sc.index % 2 == 0 {
            t.text_soft.gamma_multiply(0.35)
        } else {
            t.text_soft.gamma_multiply(0.22)
        });
        let fill = if resp.hovered() { base.gamma_multiply(1.4) } else { base };
        p.rect_filled(block.shrink2(vec2(0.5, 0.0)), 2.0, fill);
        if cur == Some(sc.index) {
            p.rect_stroke(block, 2.0, Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
        }
        let num = sc.number.clone().unwrap_or_else(|| (sc.index + 1).to_string());
        if block.width() >= 14.0 {
            p.text(block.left_top() + vec2(4.0, 3.0), Align2::LEFT_TOP, &num, FontId::monospace(10.0), t.text);
        }
        let fits = ((block.width() - 8.0) / 6.0).max(0.0) as usize;
        if fits >= 4 {
            p.text(block.left_bottom() + vec2(4.0, -3.0), Align2::LEFT_BOTTOM, elide(&sc.heading, fits), FontId::proportional(10.0), t.text);
        }
        if resp.on_hover_text(format!("{num}. {} · p.{}", sc.heading, sc.page)).clicked() {
            go = Some(("scene.goto", json!({"index": sc.index})));
        }
    }
    // The ruler: a tick per page, labelled at a readable step.
    let ry = top + scene_h + 4.0;
    p.line_segment([pos2(r.left(), ry), pos2(r.right(), ry)], Stroke::new(1.0, t.chrome_line));
    let per_page = r.width() / pages;
    let step = [1usize, 2, 5, 10, 20, 50].into_iter().find(|s| *s as f32 * per_page >= 28.0).unwrap_or(100);
    let whole = pages.ceil() as usize;
    for k in 0..=whole {
        let x = x_of(k as f32);
        let major = k % step == 0;
        p.line_segment([pos2(x, ry), pos2(x, ry + if major { 6.0 } else { 3.0 })], Stroke::new(1.0, t.text_soft.gamma_multiply(0.7)));
        if major && k < whole {
            p.text(pos2(x + 2.0, ry + 7.0), Align2::LEFT_TOP, format!("{}", k + 1), FontId::proportional(9.5), t.text_soft);
        }
    }
    p.text(r.right_bottom(), Align2::RIGHT_BOTTOM, format!("{} pages", map.pages as usize), FontId::proportional(9.5), Color32::from_gray(140));
    if let Some((id, v)) = go {
        app.run(id, v);
        app.view = View::Page;
    }
}
