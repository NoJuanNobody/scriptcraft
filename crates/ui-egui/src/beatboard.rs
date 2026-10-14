//! The beat board: a free canvas of beat cards. Drag the background (or scroll) to pan,
//! Ctrl/Cmd + wheel or pinch to zoom, double-click empty space for a new beat, drag a card to
//! move it and its corner to resize it, double-click a card to edit it, right-click for links
//! to scenes, colour and delete. Moves are one command each (on release), so undo takes back a
//! whole drag.

use egui::{Align2, Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use scriptcraft_doc::{Beat, CardColor};
use serde_json::json;

use crate::ScriptApp;
use crate::cards::card_rgb;
use crate::chrome::elide;

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 2.0;
const HANDLE: f32 = 14.0;

/// What a drag is doing.
#[derive(Clone, Copy, Debug)]
enum DragKind {
    Move,
    Resize,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    id: u32,
    kind: DragKind,
    /// Where the pointer went down (screen) and the card's geometry then.
    from: Pos2,
    orig: (f32, f32, f32, f32),
}

/// The beat being added or edited in the Beat window.
#[derive(Clone, Debug, Default)]
pub struct BeatEditor {
    /// `None` for a new beat.
    pub id: Option<u32>,
    pub title: String,
    pub text: String,
    pub color: Option<CardColor>,
    /// Linked scene (index).
    pub scene: Option<usize>,
    /// Where a new beat goes (board points).
    pub at: (f32, f32),
    pub grab: bool,
}

/// The board's view state (not saved with the script).
pub struct BoardState {
    /// The board point at the view's top-left corner.
    pub pan: Vec2,
    pub zoom: f32,
    pub selected: Option<u32>,
    drag: Option<Drag>,
    pub editor: Option<BeatEditor>,
    /// Screen rectangles of the cards last drawn (for demos and agents).
    pub rects: Vec<(u32, Rect)>,
    /// The view was framed around the cards once.
    framed: bool,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState { pan: Vec2::ZERO, zoom: 1.0, selected: None, drag: None, editor: None, rects: Vec::new(), framed: false }
    }
}

pub fn board_id() -> Id {
    Id::new("scriptcraft-beat-board")
}

/// Open the Beat window for a beat (or a new one at `at`).
pub fn edit(app: &mut ScriptApp, id: Option<u32>, at: (f32, f32)) {
    let ed = match id.and_then(|i| app.session.script.board.get(i).cloned()) {
        Some(b) => BeatEditor {
            id: Some(b.id),
            title: b.title.clone(),
            text: b.text.clone(),
            color: b.color,
            scene: app.session.script.beat_scene(b.id),
            at: (b.x, b.y),
            grab: true,
        },
        None => BeatEditor { at, grab: true, ..BeatEditor::default() },
    };
    app.board.editor = Some(ed);
}

/// Frame every card in a view of size `size`.
fn fit(app: &mut ScriptApp, size: Vec2) {
    let Some((x0, y0, x1, y1)) = app.session.script.board.bounds() else {
        app.board.pan = Vec2::ZERO;
        app.board.zoom = 1.0;
        return;
    };
    let margin = 40.0;
    let z = ((size.x - 2.0 * margin) / (x1 - x0).max(1.0)).min((size.y - 2.0 * margin) / (y1 - y0).max(1.0)).clamp(MIN_ZOOM, 1.0);
    app.board.zoom = z;
    app.board.pan = vec2(x0 - margin / z, y0 - margin / z);
}

pub fn show(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    // The header row.
    let mut new_at_center = false;
    let mut want_fit = false;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(RichText::new("Beat Board").strong().size(15.0));
        ui.add_space(8.0);
        if ui.button("+ New Beat").on_hover_text("Add a beat card (or double-click empty space)").clicked() {
            new_at_center = true;
        }
        ui.add_space(8.0);
        if ui.small_button("−").on_hover_text("Zoom out").clicked() {
            app.board.zoom = (app.board.zoom / 1.2).max(MIN_ZOOM);
        }
        ui.label(RichText::new(format!("{:.0}%", app.board.zoom * 100.0)).size(11.5).color(t.text_soft));
        if ui.small_button("+").on_hover_text("Zoom in").clicked() {
            app.board.zoom = (app.board.zoom * 1.2).min(MAX_ZOOM);
        }
        if ui.small_button("Fit").on_hover_text("Show every card").clicked() {
            want_fit = true;
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new("Drag to move · corner to resize · double-click to edit · right-click to link to a scene").size(11.0).color(t.text_soft),
        );
    });
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(ui.available_size().max(vec2(100.0, 100.0)), Sense::hover());
    let id = board_id().with(ui.id());
    let bg = ui.interact(rect, id, Sense::click_and_drag());
    if !app.board.framed {
        app.board.framed = true;
        fit(app, rect.size());
    }
    if want_fit {
        fit(app, rect.size());
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.desk);
    // Dotted grid.
    let z = app.board.zoom;
    let step = 40.0 * z;
    if step >= 8.0 {
        let ox = (-app.board.pan.x * z).rem_euclid(step);
        let oy = (-app.board.pan.y * z).rem_euclid(step);
        let dot = t.text_soft.gamma_multiply(0.22);
        let mut y = rect.top() + oy;
        while y < rect.bottom() {
            let mut x = rect.left() + ox;
            while x < rect.right() {
                painter.circle_filled(pos2(x, y), 1.0, dot);
                x += step;
            }
            y += step;
        }
    }
    let to_screen = |app: &ScriptApp, x: f32, y: f32| rect.min + (vec2(x, y) - app.board.pan) * app.board.zoom;
    let to_board = |app: &ScriptApp, p: Pos2| (p - rect.min) / app.board.zoom + app.board.pan;
    // Pan and zoom.
    let hovering = ui.rect_contains_pointer(rect);
    if hovering {
        let (zd, scroll, at) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
        if (zd - 1.0).abs() > 1e-4 {
            let anchor = at.unwrap_or(rect.center());
            let before = to_board(app, anchor);
            app.board.zoom = (app.board.zoom * zd).clamp(MIN_ZOOM, MAX_ZOOM);
            let after = to_board(app, anchor);
            app.board.pan += before - after;
        } else if scroll != Vec2::ZERO {
            app.board.pan -= scroll / app.board.zoom;
        }
    }
    if bg.dragged() && app.board.drag.is_none() {
        app.board.pan -= bg.drag_delta() / app.board.zoom;
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if bg.clicked() {
        app.board.selected = None;
        bg.request_focus();
    }
    if bg.double_clicked()
        && let Some(p) = bg.interact_pointer_pos()
    {
        let b = to_board(app, p);
        edit(app, None, (b.x, b.y));
    }
    if new_at_center {
        let c = to_board(app, rect.center() - vec2(scriptcraft_doc::BEAT_W, scriptcraft_doc::BEAT_H) * 0.5 * app.board.zoom);
        edit(app, None, (c.x, c.y));
    }
    let beats: Vec<Beat> = app.session.script.board.beats.clone();
    if beats.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, "Double-click anywhere to add a beat", FontId::proportional(15.0), t.text_soft);
    }
    // Cards, bottom to top.
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let mut rects = Vec::new();
    let mut actions: Vec<(&'static str, serde_json::Value)> = Vec::new();
    let mut open_editor: Option<u32> = None;
    let scenes = app.session.script.scenes();
    for b in &beats {
        // Live geometry while this card is dragged.
        let (mut x, mut y, mut w, mut h) = (b.x, b.y, b.w, b.h);
        if let (Some(d), Some(p)) = (app.board.drag.filter(|d| d.id == b.id), pointer) {
            let dv = (p - d.from) / app.board.zoom;
            match d.kind {
                DragKind::Move => (x, y) = (d.orig.0 + dv.x, d.orig.1 + dv.y),
                DragKind::Resize => {
                    w = (d.orig.2 + dv.x).clamp(scriptcraft_doc::BEAT_MIN, scriptcraft_doc::BEAT_MAX);
                    h = (d.orig.3 + dv.y).clamp(scriptcraft_doc::BEAT_MIN, scriptcraft_doc::BEAT_MAX);
                }
            }
        }
        let r = Rect::from_min_size(to_screen(app, x, y), vec2(w, h) * z);
        rects.push((b.id, r));
        if !r.intersects(rect) {
            continue;
        }
        let resp = ui.interact(r.intersect(rect), id.with(("beat", b.id)), Sense::click_and_drag());
        let handle_r = Rect::from_min_size(r.right_bottom() - vec2(HANDLE, HANDLE), vec2(HANDLE, HANDLE));
        let handle = ui.interact(handle_r.intersect(rect), id.with(("beat-size", b.id)), Sense::drag());
        for (resp, kind) in [(&resp, DragKind::Move), (&handle, DragKind::Resize)] {
            if resp.drag_started()
                && let Some(p) = ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos())
            {
                app.board.drag = Some(Drag { id: b.id, kind, from: p, orig: (b.x, b.y, b.w, b.h) });
                app.board.selected = Some(b.id);
                resp.request_focus();
            }
        }
        if (resp.drag_stopped() || handle.drag_stopped())
            && let Some(d) = app.board.drag.take().filter(|d| d.id == b.id)
        {
            let moved = (x, y, w, h) != d.orig;
            if moved {
                actions.push(("beat.move", json!({"id": b.id, "x": x.round(), "y": y.round(), "w": w.round(), "h": h.round()})));
            }
        }
        if resp.clicked() {
            app.board.selected = Some(b.id);
            resp.request_focus();
        }
        if resp.double_clicked() {
            open_editor = Some(b.id);
        }
        if handle.hovered() || handle.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        } else if resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        paint_card(&painter, app, b, r, &scenes, app.board.selected == Some(b.id));
        resp.context_menu(|ui| beat_menu(app, ui, b, &scenes, &mut actions, &mut open_editor));
    }
    app.board.rects = rects;
    if !ui.input(|i| i.pointer.any_down()) {
        app.board.drag = None;
    }
    // Keys, when the board has the keyboard.
    let focused = ui.memory(|m| m.focused().is_some_and(|f| f == id || app.session.script.board.beats.iter().any(|b| f == id.with(("beat", b.id)))));
    if focused && let Some(sel) = app.board.selected {
        let (del, enter, esc) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if del {
            actions.push(("beat.delete", json!({"id": sel})));
            app.board.selected = None;
        } else if enter {
            open_editor = Some(sel);
        } else if esc {
            app.board.selected = None;
        }
    }
    for (cmd, p) in actions {
        let go = matches!(cmd, "beat.goto" | "beat.toScene");
        if app.execute(cmd, p).is_ok() && go {
            crate::split::show_page(app);
        }
    }
    if let Some(i) = open_editor {
        edit(app, Some(i), (0.0, 0.0));
    }
}

fn paint_card(p: &egui::Painter, app: &ScriptApp, b: &Beat, r: Rect, scenes: &[scriptcraft_doc::Scene], selected: bool) {
    let t = app.tokens();
    let z = app.board.zoom;
    let shadow = egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if t.dark { 110 } else { 34 }) };
    p.add(shadow.as_shape(r, egui::CornerRadius::same(6)));
    p.rect_filled(r, 6.0, t.card);
    let band = b.color.map(card_rgb);
    if let Some(c) = band {
        let top = Rect::from_min_size(r.min, vec2(r.width(), (8.0 * z).max(3.0)));
        p.rect_filled(top, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, c);
    }
    let stroke = if selected { Stroke::new(2.5, t.accent) } else { Stroke::new(0.7, t.paper_edge) };
    p.rect_stroke(r, 6.0, stroke, StrokeKind::Inside);
    let pad = 10.0 * z;
    let inner = r.shrink(pad);
    let clip = p.with_clip_rect(r.shrink(2.0).intersect(p.clip_rect()));
    let mut y = inner.top() + 4.0 * z;
    if !b.title.is_empty() {
        let g = p.layout(b.title.clone(), FontId::proportional((14.0 * z).max(6.0)), t.text, inner.width().max(10.0));
        let h = g.size().y;
        clip.galley(pos2(inner.left(), y), g, t.text);
        y += h + 4.0 * z;
    }
    let linked = app.session.script.beat_scene(b.id);
    let foot = if linked.is_some() { 18.0 * z } else { 0.0 };
    if !b.text.is_empty() && y < inner.bottom() - foot {
        let g = p.layout(b.text.clone(), FontId::proportional((12.0 * z).max(5.0)), t.text_soft, inner.width().max(10.0));
        let body = p.with_clip_rect(Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), inner.bottom() - foot)).intersect(p.clip_rect()));
        body.galley(pos2(inner.left(), y), g, t.text_soft);
    }
    if let Some(i) = linked {
        let sc = scenes.get(i);
        let num = sc.and_then(|s| s.number.clone()).unwrap_or_else(|| (i + 1).to_string());
        let head = sc.map_or(String::new(), |s| s.heading.clone());
        let chars = ((inner.width() / (6.6 * z)).max(4.0)) as usize;
        clip.text(
            pos2(inner.left(), inner.bottom()),
            Align2::LEFT_BOTTOM,
            elide(&format!("Scene {num} · {head}"), chars),
            FontId::proportional((11.0 * z).max(5.0)),
            t.accent,
        );
    }
    // Resize grip.
    let g = r.right_bottom() - vec2(5.0, 5.0);
    for k in 1..=2 {
        let d = k as f32 * 4.0;
        clip.line_segment([g - vec2(d, 0.0), g - vec2(0.0, d)], Stroke::new(1.0, t.text_soft.gamma_multiply(0.6)));
    }
}

fn beat_menu(
    app: &mut ScriptApp,
    ui: &mut Ui,
    b: &Beat,
    scenes: &[scriptcraft_doc::Scene],
    actions: &mut Vec<(&'static str, serde_json::Value)>,
    open_editor: &mut Option<u32>,
) {
    app.board.selected = Some(b.id);
    if ui.button("Edit Beat…").clicked() {
        *open_editor = Some(b.id);
        ui.close();
    }
    let linked = app.session.script.beat_scene(b.id);
    if ui.add_enabled(linked.is_some(), egui::Button::new("Go to Scene")).clicked() {
        actions.push(("beat.goto", json!({"id": b.id})));
        ui.close();
    }
    ui.menu_button("Link to Scene", |ui| {
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for sc in scenes {
                let num = sc.number.clone().unwrap_or_else(|| (sc.index + 1).to_string());
                if ui.selectable_label(linked == Some(sc.index), format!("{num}  {}", elide(&sc.heading, 40))).clicked() {
                    actions.push(("beat.link", json!({"id": b.id, "scene": sc.index})));
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.add_enabled(linked.is_some(), egui::Button::new("Unlink")).clicked() {
            actions.push(("beat.link", json!({"id": b.id, "scene": null})));
            ui.close();
        }
    });
    if ui.button("New Scene from Beat").on_hover_text("Add a scene at the end, headed by this beat, and link them").clicked() {
        actions.push(("beat.toScene", json!({"id": b.id})));
        ui.close();
    }
    ui.separator();
    ui.horizontal(|ui| {
        for c in CardColor::ALL {
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            ui.painter().circle_filled(r.center(), 8.0, card_rgb(c));
            if b.color == Some(c) {
                ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.5, ui.visuals().text_color()));
            }
            if resp.on_hover_text(c.id()).clicked() {
                actions.push(("beat.edit", json!({"id": b.id, "color": c.id()})));
                ui.close();
            }
        }
    });
    if ui.button("No Colour").clicked() {
        actions.push(("beat.edit", json!({"id": b.id, "color": null})));
        ui.close();
    }
    ui.separator();
    if ui.button("Delete Beat").clicked() {
        actions.push(("beat.delete", json!({"id": b.id})));
        ui.close();
    }
}

/// The Beat window (new or edit).
pub fn dialog(app: &mut ScriptApp, ctx: &egui::Context) {
    let Some(mut ed) = app.board.editor.take() else { return };
    let t = app.tokens();
    let scenes = app.session.script.scenes();
    let (mut save, mut cancel, mut delete, mut goto, mut to_scene) = (false, false, false, false, false);
    let mut open = true;
    egui::Window::new(if ed.id.is_some() { "Beat" } else { "New Beat" })
        .id(Id::new("beat-editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut ed.title).hint_text("Title").desired_width(380.0).font(FontId::proportional(15.0)));
            if std::mem::take(&mut ed.grab) {
                r.request_focus();
            }
            ui.add(egui::TextEdit::multiline(&mut ed.text).hint_text("What happens…").desired_rows(5).desired_width(380.0));
            ui.horizontal(|ui| {
                ui.label("Colour");
                for c in std::iter::once(None).chain(CardColor::ALL.into_iter().map(Some)) {
                    let (rr, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    match c {
                        Some(c) => {
                            ui.painter().circle_filled(rr.center(), 7.5, card_rgb(c));
                        }
                        None => {
                            ui.painter().circle_stroke(rr.center(), 7.0, Stroke::new(1.2, t.text_soft));
                        }
                    }
                    if ed.color == c {
                        ui.painter().circle_stroke(rr.center(), 9.5, Stroke::new(1.5, ui.visuals().text_color()));
                    }
                    if resp.on_hover_text(c.map_or("none", CardColor::id)).clicked() {
                        ed.color = c;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Scene");
                let label = |i: Option<usize>| match i.and_then(|i| scenes.get(i)) {
                    Some(sc) => format!("{}  {}", sc.number.clone().unwrap_or_else(|| (sc.index + 1).to_string()), elide(&sc.heading, 36)),
                    None => "Not linked".to_string(),
                };
                egui::ComboBox::from_id_salt("beat-scene").width(300.0).selected_text(label(ed.scene)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut ed.scene, None, "Not linked");
                    for sc in &scenes {
                        ui.selectable_value(&mut ed.scene, Some(sc.index), label(Some(sc.index)));
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(if ed.id.is_some() { "Save" } else { "Add Beat" }).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if ed.id.is_some() {
                    if ui.add_enabled(ed.scene.is_some(), egui::Button::new("Go to Scene")).clicked() {
                        save = true;
                        goto = true;
                    }
                    if ui.button("New Scene from Beat").clicked() {
                        save = true;
                        to_scene = true;
                    }
                    if ui.button("Delete").clicked() {
                        delete = true;
                    }
                }
            });
        });
    if !open || cancel {
        return;
    }
    if delete {
        if let Some(id) = ed.id {
            app.run("beat.delete", json!({"id": id}));
        }
        return;
    }
    if !save {
        app.board.editor = Some(ed);
        return;
    }
    let color = ed.color.map(CardColor::id);
    let id = match ed.id {
        Some(id) => {
            app.run("beat.edit", json!({"id": id, "title": ed.title, "text": ed.text, "color": color}));
            if app.session.script.beat_scene(id) != ed.scene {
                app.run("beat.link", json!({"id": id, "scene": ed.scene}));
            }
            Some(id)
        }
        None => app
            .execute("beat.add", json!({"title": ed.title, "text": ed.text, "color": color, "x": ed.at.0, "y": ed.at.1, "scene": ed.scene}))
            .ok()
            .and_then(|v| v["id"].as_u64())
            .and_then(|i| u32::try_from(i).ok()),
    };
    app.board.selected = id;
    if let Some(id) = id {
        if to_scene {
            app.run("beat.toScene", json!({"id": id}));
            crate::split::show_page(app);
        } else if goto {
            app.run("beat.goto", json!({"id": id}));
            crate::split::show_page(app);
        }
    }
}
