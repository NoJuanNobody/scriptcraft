//! Bookmarks: the navigator's Bookmarks tab and the Edit › Bookmarks menu.

use egui::{RichText, Ui, vec2};
use serde_json::json;

use crate::ScriptApp;
use crate::chrome::elide;

/// The Bookmarks tab: every bookmark with its page; click to go there, rename or remove.
pub fn tab(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    ui.horizontal(|ui| {
        if ui.button("Add Bookmark").on_hover_text("Bookmark the caret's paragraph").clicked() {
            app.run("bookmark.add", json!({}));
        }
        let none = app.session.script.paras.iter().all(|p| p.bookmark.is_none());
        if ui.add_enabled(!none, egui::Button::new("‹")).on_hover_text("Previous bookmark").clicked() {
            app.run("bookmark.previous", json!({}));
            crate::split::show_page(app);
        }
        if ui.add_enabled(!none, egui::Button::new("›")).on_hover_text("Next bookmark").clicked() {
            app.run("bookmark.next", json!({}));
            crate::split::show_page(app);
        }
    });
    ui.add_space(4.0);
    let marks = app.session.script.bookmarks();
    if marks.is_empty() {
        ui.label(RichText::new("No bookmarks yet. Put the caret on a paragraph and choose Add Bookmark (Edit › Bookmarks).").color(t.text_soft));
        return;
    }
    let l = app.session.layout();
    let cur = app.session.sel.focus.para;
    let mut action: Option<(&str, serde_json::Value)> = None;
    for (para, name) in marks {
        let on = cur == para;
        let editing = app.bookmark_edit.as_ref().is_some_and(|(p, _)| *p == para);
        if editing {
            let mut done = None;
            ui.horizontal(|ui| {
                if let Some((_, buf)) = app.bookmark_edit.as_mut() {
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(ui.available_width() - 60.0));
                    if !r.has_focus() && !r.lost_focus() {
                        r.request_focus();
                    }
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.small_button("OK").clicked() || enter {
                        done = Some(Some(buf.clone()));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        done = Some(None);
                    }
                }
            });
            if let Some(d) = done {
                if let Some(to) = d {
                    action = Some(("bookmark.rename", json!({"name": name, "to": to})));
                }
                app.bookmark_edit = None;
            }
            continue;
        }
        let text = app.session.script.para(para).map_or(String::new(), |p| p.text.clone());
        let resp = ui
            .scope(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), egui::Sense::click());
                let fill = if on {
                    t.accent.gamma_multiply(0.16)
                } else if resp.hovered() {
                    ui.visuals().widgets.hovered.weak_bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                };
                let p = ui.painter();
                p.rect_filled(r, 5.0, fill);
                // A ribbon.
                let o = r.left_top() + vec2(10.0, 6.0);
                let pts = vec![o, o + vec2(9.0, 0.0), o + vec2(9.0, 13.0), o + vec2(4.5, 9.0), o + vec2(0.0, 13.0)];
                p.add(egui::Shape::convex_polygon(pts, t.accent, egui::Stroke::NONE));
                let width = r.width() - 70.0;
                p.text(
                    r.left_top() + vec2(28.0, 5.0),
                    egui::Align2::LEFT_TOP,
                    elide(&name, (width / 6.6) as usize),
                    egui::FontId::proportional(12.5),
                    t.text,
                );
                p.text(
                    r.right_top() + vec2(-8.0, 5.0),
                    egui::Align2::RIGHT_TOP,
                    l.page_label_of(para),
                    egui::FontId::proportional(11.0),
                    t.text_soft,
                );
                p.text(
                    r.left_top() + vec2(28.0, 22.0),
                    egui::Align2::LEFT_TOP,
                    elide(&text, (width / 5.8) as usize),
                    egui::FontId::proportional(11.0),
                    t.text_soft,
                );
                resp
            })
            .inner;
        if resp.clicked() {
            action = Some(("bookmark.goto", json!({"name": name})));
        }
        if resp.double_clicked() {
            app.bookmark_edit = Some((para, name.clone()));
        }
        resp.context_menu(|ui| {
            if ui.button("Go to Bookmark").clicked() {
                action = Some(("bookmark.goto", json!({"name": name})));
                ui.close();
            }
            if ui.button("Rename…").clicked() {
                app.bookmark_edit = Some((para, name.clone()));
                ui.close();
            }
            if ui.button("Remove Bookmark").clicked() {
                action = Some(("bookmark.remove", json!({"name": name})));
                ui.close();
            }
        });
    }
    if let Some((id, p)) = action {
        let go = id == "bookmark.goto";
        if app.execute(id, p).is_ok() && go {
            crate::split::show_page(app);
        }
    }
}

/// Edit › Bookmarks.
pub fn menu(app: &mut ScriptApp, ui: &mut Ui) {
    ui.menu_button("Bookmarks", |ui| {
        let here = app.session.script.para(app.session.sel.focus.para).and_then(|p| p.bookmark.clone());
        if here.is_some() {
            if ui.button("Remove Bookmark").clicked() {
                app.run("bookmark.remove", json!({}));
                ui.close();
            }
        } else if ui.button("Add Bookmark").clicked() {
            app.run("bookmark.add", json!({}));
            ui.close();
        }
        let marks = app.session.script.bookmarks();
        for (label, id) in [("Next Bookmark", "bookmark.next"), ("Previous Bookmark", "bookmark.previous")] {
            if ui.add_enabled(!marks.is_empty(), egui::Button::new(label)).clicked() {
                app.run(id, json!({}));
                crate::split::show_page(app);
                ui.close();
            }
        }
        if ui.button("Show Bookmarks").clicked() {
            app.run("ui.bookmarks", json!({}));
            ui.close();
        }
        if !marks.is_empty() {
            ui.separator();
            for (_, name) in marks.into_iter().take(30) {
                if ui.button(elide(&name, 40)).clicked() {
                    app.run("bookmark.goto", json!({"name": name}));
                    crate::split::show_page(app);
                    ui.close();
                }
            }
        }
    });
}
