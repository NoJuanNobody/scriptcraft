//! Script notes, alternate lines and production tags in the window: highlights and margin
//! markers on the page, the Notes tab of the navigator, the note editor, the alternates picker,
//! the Tools menu entries, the page's context menu entries and the breakdown in Reports.

use egui::{Align2, Color32, FontId, Painter, Rect, RichText, Stroke, Ui, pos2, vec2};
use scriptcraft_doc::{CardColor, Element, Pos, TAG_CATEGORIES, category_rgb};
use scriptcraft_geom::{CHAR_W, LINE_H};
use scriptcraft_layout::{Line, LineKind};
use serde_json::{Value, json};

use crate::ScriptApp;

/// The note being written or edited.
#[derive(Clone, Debug)]
pub struct NoteEditor {
    /// The note's index when editing; `None` for a new note.
    pub index: Option<usize>,
    /// Where a new note is anchored (the selection when the editor opened).
    pub anchor: Pos,
    pub focus: Pos,
    pub text: String,
    pub color: CardColor,
    pub author: String,
    /// Give the text field the keyboard on the next frame.
    pub grab: bool,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn note_color(c: CardColor) -> Color32 {
    rgb(c.rgb())
}

pub fn tag_color(category: &str) -> Color32 {
    rgb(category_rgb(category))
}

/// The column of byte offset `off` within a laid-out line.
fn col(line: &Line, off: usize) -> usize {
    line.text.get(..off.clamp(line.start, line.end).saturating_sub(line.start).min(line.text.len())).map_or(0, |t| t.chars().count())
}

/// Screen rectangle of `[a, b)` on a line, if the range touches it.
fn span_rect(line: &Line, pr: Rect, z: f32, a: usize, b: usize) -> Option<Rect> {
    if b <= line.start || a >= line.end || b <= a {
        return None;
    }
    let (c0, c1) = (col(line, a), col(line, b));
    let x0 = pr.left() + (line.x + c0 as f32 * CHAR_W) * z;
    let x1 = pr.left() + (line.x + c1 as f32 * CHAR_W) * z;
    Some(Rect::from_min_max(pos2(x0, pr.top() + line.y * z), pos2(x1, pr.top() + (line.y + LINE_H) * z)))
}

/// Behind a body line's text: note ranges tinted in their colour, tagged items (when tags are
/// shown) tinted and underlined in their category's colour.
pub fn paint_behind(app: &ScriptApp, p: &Painter, line: &Line, pr: Rect, z: f32) {
    if line.kind != LineKind::Body {
        return;
    }
    let Some(para) = line.para.and_then(|i| app.session.script.para(i)) else { return };
    for n in &para.notes {
        if let Some(r) = span_rect(line, pr, z, n.start, n.end) {
            p.rect_filled(r, 0.0, note_color(n.color).gamma_multiply(if app.dark { 0.30 } else { 0.24 }));
        }
    }
    if app.show_tags {
        for t in &para.tagged {
            if let Some(r) = span_rect(line, pr, z, t.start, t.end) {
                let c = tag_color(&t.category);
                p.rect_filled(r, 2.0 * z, c.gamma_multiply(0.22));
                p.line_segment([pos2(r.left(), r.bottom() - 0.6 * z), pos2(r.right(), r.bottom() - 0.6 * z)], Stroke::new(1.4 * z, c));
            }
        }
    }
}

/// In the left margin: a note marker beside the line where each note starts, and an "ALT"
/// badge on the first line of a paragraph that has alternates.
pub fn paint_margin(app: &ScriptApp, p: &Painter, line: &Line, pr: Rect, z: f32) {
    if line.kind != LineKind::Body {
        return;
    }
    let Some(para) = line.para.and_then(|i| app.session.script.para(i)) else { return };
    let y = pr.top() + line.y * z;
    let here: Vec<&scriptcraft_doc::Note> =
        para.notes.iter().filter(|n| n.start >= line.start && (n.start < line.end || (n.start == line.end && line.end == para.text.len()))).collect();
    // The first line of an empty paragraph has start == end == 0.
    let here: Vec<&scriptcraft_doc::Note> = if line.start == line.end { para.notes.iter().collect() } else { here };
    for (k, n) in here.iter().enumerate().take(3) {
        let x = pr.left() + (30.0 + k as f32 * 4.0) * z;
        let r = Rect::from_min_size(pos2(x, y + 1.0 * z + k as f32 * 2.0 * z), vec2(10.0 * z, 10.0 * z));
        p.rect_filled(r, 1.5 * z, note_color(n.color));
        let fold = [r.right_top() + vec2(-3.5 * z, 0.0), r.right_top() + vec2(0.0, 3.5 * z), r.right_top() + vec2(-3.5 * z, 3.5 * z)];
        p.add(egui::Shape::convex_polygon(fold.to_vec(), Color32::from_black_alpha(70), Stroke::NONE));
        p.rect_stroke(r, 1.5 * z, Stroke::new(0.6 * z, Color32::from_black_alpha(90)), egui::StrokeKind::Inside);
    }
    if !para.alternates.is_empty() && line.start == 0 {
        let label = format!("ALT {}", para.alternates.len() + 1);
        let pos = pos2(pr.left() + 50.0 * z, y + 1.5 * z);
        let galley = p.layout_no_wrap(label, FontId::proportional(7.5 * z), app.tokens().accent);
        let r = Rect::from_min_size(pos, galley.size() + vec2(6.0 * z, 2.0 * z));
        p.rect_stroke(r, 3.0 * z, Stroke::new(0.8 * z, app.tokens().accent), egui::StrokeKind::Inside);
        p.galley(pos + vec2(3.0 * z, 1.0 * z), galley, app.tokens().accent);
    }
}

/// Open the note editor for a new note on the selection.
pub fn open_new_note(app: &mut ScriptApp) {
    let (a, b) = app.session.sel.ordered();
    app.note_editor = Some(NoteEditor {
        index: None,
        anchor: a,
        focus: b,
        text: String::new(),
        color: CardColor::Yellow,
        author: app.prefs.editing.note_author.clone(),
        grab: true,
    });
}

/// Open the note editor on note `index`.
pub fn open_note(app: &mut ScriptApp, index: usize) -> bool {
    let refs = app.session.script.note_refs();
    let Some(n) = refs.get(index).and_then(|r| app.session.script.note(*r)).cloned() else { return false };
    let at = refs.get(index).map_or(Pos::default(), |r| Pos::new(r.para, n.start));
    app.note_editor = Some(NoteEditor { index: Some(index), anchor: at, focus: at, text: n.text, color: n.color, author: n.author, grab: true });
    true
}

/// The Notes tab of the navigator.
pub fn notes_tab(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    ui.horizontal(|ui| {
        if ui.button("Add Note…").on_hover_text("Add a note to the selected words (or at the caret)").clicked() {
            open_new_note(app);
        }
        let any = app.session.script.paras.iter().any(|p| !p.notes.is_empty());
        if ui.add_enabled(any, egui::Button::new("‹").small()).on_hover_text("Previous note").clicked() {
            app.run("note.previous", json!({}));
            crate::split::show_page(app);
        }
        if ui.add_enabled(any, egui::Button::new("›").small()).on_hover_text("Next note").clicked() {
            app.run("note.next", json!({}));
            crate::split::show_page(app);
        }
    });
    ui.add_space(4.0);
    let notes = scriptcraft_engine::cmd::notes::notes_json(&mut app.session);
    if notes.is_empty() {
        ui.label(RichText::new("No notes yet. Select some words and choose Add Note, or right-click the page.").color(t.text_soft));
        return;
    }
    let mut goto = None;
    let mut edit = None;
    let mut delete = None;
    for n in &notes {
        let index = n["index"].as_u64().unwrap_or(0) as usize;
        let color = CardColor::parse(n["color"].as_str().unwrap_or("")).map_or(t.accent, note_color);
        let frame = egui::Frame::NONE
            .fill(color.gamma_multiply(if t.dark { 0.16 } else { 0.12 }))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(8, 6));
        let resp = frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, color);
                    let mut meta = format!("p.{}", n["page"].as_u64().unwrap_or(1));
                    if let Some(sc) = n["scene"].as_u64() {
                        meta.push_str(&format!(" · scene {sc}"));
                    }
                    if let Some(a) = n["author"].as_str().filter(|a| !a.is_empty()) {
                        meta.push_str(&format!(" · {a}"));
                    }
                    ui.label(RichText::new(meta).size(11.0).color(t.text_soft));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").on_hover_text("Delete note").clicked() {
                            delete = Some(index);
                        }
                        if ui.small_button("Edit").clicked() {
                            edit = Some(index);
                        }
                    });
                });
                let quote = n["quote"].as_str().unwrap_or("");
                if !quote.is_empty() {
                    ui.label(RichText::new(format!("“{}”", crate::chrome::elide(quote, 48))).italics().size(11.5).color(t.text_soft));
                }
                ui.label(RichText::new(n["text"].as_str().unwrap_or("")).size(12.5).color(t.text));
            })
            .response
            .interact(egui::Sense::click());
        if resp.clicked() {
            goto = Some(index);
        }
        ui.add_space(4.0);
    }
    if let Some(i) = goto {
        app.run("note.goto", json!({"index": i}));
        crate::split::show_page(app);
    }
    if let Some(i) = edit {
        open_note(app, i);
    }
    if let Some(i) = delete {
        app.run("note.delete", json!({"index": i}));
    }
}

/// The note editor and the alternates picker.
pub fn dialogs(app: &mut ScriptApp, ctx: &egui::Context) {
    note_editor(app, ctx);
    alternates_window(app, ctx);
}

fn note_editor(app: &mut ScriptApp, ctx: &egui::Context) {
    let Some(mut ed) = app.note_editor.take() else { return };
    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    let mut delete = false;
    let title = if ed.index.is_some() { "Edit Script Note" } else { "Add Script Note" };
    let quote = if ed.index.is_none() && ed.anchor != ed.focus { app.session.script.text_between(ed.anchor, ed.focus) } else { String::new() };
    egui::Window::new(title)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 120.0))
        .show(ctx, |ui| {
            if !quote.is_empty() {
                ui.label(RichText::new(format!("On “{}”", crate::chrome::elide(&quote, 60))).italics().color(app.tokens().text_soft));
            }
            let r = ui.add(egui::TextEdit::multiline(&mut ed.text).desired_rows(4).desired_width(360.0).hint_text("Note"));
            if std::mem::take(&mut ed.grab) {
                r.request_focus();
            }
            ui.horizontal(|ui| {
                ui.label("Colour");
                for c in CardColor::ALL {
                    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::click());
                    ui.painter().rect_filled(rect.shrink(2.0), 4.0, note_color(c));
                    if ed.color == c {
                        ui.painter().rect_stroke(rect, 5.0, Stroke::new(2.0, app.tokens().text), egui::StrokeKind::Inside);
                    }
                    if resp.on_hover_text(c.id()).clicked() {
                        ed.color = c;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Author");
                ui.add(egui::TextEdit::singleline(&mut ed.author).desired_width(200.0));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = !ed.text.trim().is_empty();
                if ui.add_enabled(ok, egui::Button::new(if ed.index.is_some() { "Save" } else { "Add Note" })).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if ed.index.is_some() && ui.button("Delete").clicked() {
                    delete = true;
                }
            });
        });
    if save {
        let author = scriptcraft_doc::clean_author(&ed.author);
        app.prefs.editing.note_author = author.clone();
        let mut v = json!({"text": ed.text, "color": ed.color.id(), "author": author});
        match ed.index {
            Some(i) => {
                v["index"] = json!(i);
                app.run("note.edit", v);
            }
            None => {
                v["anchor"] = json!({"para": ed.anchor.para, "off": ed.anchor.off});
                v["focus"] = json!({"para": ed.focus.para, "off": ed.focus.off});
                app.run("note.add", v);
            }
        }
        return;
    }
    if delete && let Some(i) = ed.index {
        app.run("note.delete", json!({"index": i}));
        return;
    }
    if open && !cancel {
        app.note_editor = Some(ed);
    }
}

fn alternates_window(app: &mut ScriptApp, ctx: &egui::Context) {
    if !app.alt_window {
        return;
    }
    let mut open = true;
    let mut actions: Vec<(&str, Value)> = Vec::new();
    let i = app.session.sel.focus.para;
    let para = app.session.script.para(i).cloned();
    egui::Window::new("Alternate Lines")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 120.0))
        .show(ctx, |ui| {
            let t = app.tokens();
            let Some(p) = para.filter(|p| p.element == Element::Dialogue || !p.alternates.is_empty()) else {
                ui.label(RichText::new("Put the caret in a line of dialogue to keep other versions of it.").color(t.text_soft));
                return;
            };
            ui.label(RichText::new("The chosen line is the one that prints. The others are kept with it.").size(11.5).color(t.text_soft));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let _ = ui.radio(true, "");
                ui.label(RichText::new(&p.text).strong());
                ui.label(RichText::new("(in the script)").size(11.0).color(t.text_soft));
            });
            for (k, a) in p.alternates.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.radio(false, "").on_hover_text("Use this line").clicked() {
                        actions.push(("alt.choose", json!({"index": k})));
                    }
                    let mut text = a.text.clone();
                    if ui.add(egui::TextEdit::singleline(&mut text).desired_width(320.0)).changed() && !text.trim().is_empty() {
                        actions.push(("alt.edit", json!({"index": k, "text": text})));
                    }
                    if ui.small_button("×").on_hover_text("Delete this alternate").clicked() {
                        actions.push(("alt.delete", json!({"index": k})));
                    }
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(p.element == Element::Dialogue, egui::Button::new("Add Alternate"))
                    .on_hover_text("Add a copy of the line to rework")
                    .clicked()
                {
                    actions.push(("alt.add", json!({})));
                }
                let many = !p.alternates.is_empty();
                if ui.add_enabled(many, egui::Button::new("Previous")).clicked() {
                    actions.push(("alt.previous", json!({})));
                }
                if ui.add_enabled(many, egui::Button::new("Next")).clicked() {
                    actions.push(("alt.next", json!({})));
                }
            });
        });
    for (id, v) in actions {
        app.run(id, v);
    }
    app.alt_window = open;
}

/// Script Notes, Alternates and Production Tagging in the Tools menu.
pub fn tools_menu(app: &mut ScriptApp, ui: &mut Ui) {
    ui.separator();
    ui.menu_button("Script Notes", |ui| {
        if ui.button("Add Script Note…").clicked() {
            open_new_note(app);
            ui.close();
        }
        menu_item(app, ui, "Next Note", "note.next");
        menu_item(app, ui, "Previous Note", "note.previous");
        menu_item(app, ui, "Show Notes", "ui.notes");
        ui.separator();
        menu_item(app, ui, "Delete All Notes", "note.deleteAll");
    });
    ui.menu_button("Alternate Lines", |ui| {
        menu_item(app, ui, "Alternate Lines…", "ui.alternates");
        menu_item(app, ui, "Add Alternate", "alt.add");
        menu_item(app, ui, "Next Alternate", "alt.next");
        menu_item(app, ui, "Previous Alternate", "alt.previous");
    });
    ui.menu_button("Production Tagging", |ui| {
        tag_menu(app, ui);
        menu_item(app, ui, "Remove Tags from Selection", "tag.remove");
        ui.separator();
        let mut on = app.show_tags;
        if ui.checkbox(&mut on, "Highlight Tagged Items").clicked() {
            app.run("ui.toggleTags", json!({}));
            ui.close();
        }
        menu_item(app, ui, "Breakdown Report", "ui.breakdown");
        menu_item(app, ui, "Export Breakdown (CSV)…", "ui.exportBreakdown");
    });
}

fn menu_item(app: &mut ScriptApp, ui: &mut Ui, label: &str, id: &str) {
    let enabled = id.starts_with("ui.") || app.session.registry.get(id).is_some_and(|c| (c.enabled)(&app.session).is_none());
    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
        app.run(id, json!({}));
        ui.close();
    }
}

/// "Tag Selection As" with every standard category and any custom ones in use.
fn tag_menu(app: &mut ScriptApp, ui: &mut Ui) {
    let has_sel = !app.session.sel.is_caret();
    ui.add_enabled_ui(has_sel, |ui| {
        ui.menu_button("Tag Selection As", |ui| {
            let mut cats: Vec<String> = TAG_CATEGORIES.iter().map(|(n, _)| (*n).to_string()).collect();
            for c in app.session.script.tag_categories_used() {
                if !cats.contains(&c) {
                    cats.push(c);
                }
            }
            for c in cats {
                let (r, resp) = ui.allocate_exact_size(vec2(180.0, 20.0), egui::Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                ui.painter().rect_filled(Rect::from_min_size(r.left_center() + vec2(4.0, -5.0), vec2(10.0, 10.0)), 2.0, tag_color(&c));
                ui.painter().text(r.left_center() + vec2(20.0, 0.0), Align2::LEFT_CENTER, &c, FontId::proportional(13.0), ui.visuals().text_color());
                if resp.clicked() {
                    app.run("tag.add", json!({"category": c}));
                    app.show_tags = true;
                    ui.close();
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Other");
                let r = ui.add(egui::TextEdit::singleline(&mut app.custom_category).desired_width(120.0).hint_text("category"));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.custom_category.trim().is_empty() {
                    let c = std::mem::take(&mut app.custom_category);
                    app.run("tag.add", json!({"category": c}));
                    app.show_tags = true;
                    ui.close();
                }
            });
        });
    });
}

/// Entries for the page's right-click menu.
pub fn context_items(app: &mut ScriptApp, ui: &mut Ui) {
    let (a, _) = app.session.sel.ordered();
    let para = app.session.script.para(a.para).cloned();
    if ui.button("Add Script Note…").clicked() {
        open_new_note(app);
        ui.close();
    }
    // Edit a note at the caret.
    let refs = app.session.script.note_refs();
    let at = refs.iter().position(|r| r.para == a.para && app.session.script.note(*r).is_some_and(|n| n.start <= a.off && a.off <= n.end));
    if let Some(i) = at
        && ui.button("Edit Script Note…").clicked()
    {
        open_note(app, i);
        ui.close();
    }
    tag_menu(app, ui);
    if para.as_ref().is_some_and(|p| p.tagged.iter().any(|t| t.start <= a.off && a.off <= t.end)) || !app.session.sel.is_caret() {
        menu_item(app, ui, "Remove Tags", "tag.remove");
    }
    if para.is_some_and(|p| p.element == Element::Dialogue || !p.alternates.is_empty()) && ui.button("Alternate Lines…").clicked() {
        app.alt_window = true;
        ui.close();
    }
    ui.separator();
}

/// The BREAKDOWN section of the Reports view.
pub fn breakdown_section(app: &mut ScriptApp, ui: &mut Ui, goto: &mut Option<usize>) {
    let t = app.tokens();
    let l = app.session.layout();
    let sheets = scriptcraft_engine::breakdown::breakdown(&app.session.script, &l);
    let cats = app.session.script.tag_categories_used();
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("BREAKDOWN").strong().size(12.0).color(t.accent));
        if ui.small_button("Export CSV…").clicked() {
            app.run("ui.exportBreakdown", json!({}));
        }
    });
    if cats.is_empty() {
        ui.label(
            RichText::new("Nothing tagged yet. Select words on the page and choose Tag Selection As from the right-click menu.").color(t.text_soft),
        );
        return;
    }
    egui::Grid::new("breakdown").striped(true).num_columns(4 + cats.len()).spacing([18.0, 6.0]).show(ui, |ui| {
        for h in ["#", "Scene heading", "Page", "Length"] {
            ui.label(RichText::new(h).strong());
        }
        for c in &cats {
            ui.label(RichText::new(c).strong().color(tag_color(c)));
        }
        ui.end_row();
        for b in &sheets {
            ui.label(b.number.clone().unwrap_or_else(|| b.scene.to_string()));
            if b.scene == 0 {
                ui.label(RichText::new("(before the first scene)").italics());
            } else if ui.link(&b.heading).clicked() {
                *goto = Some(b.scene - 1);
            }
            ui.label(&b.page_label);
            ui.label(&b.length);
            for c in &cats {
                ui.label(b.categories.iter().find(|x| x.category == *c).map(|x| x.items.join(", ")).unwrap_or_default());
            }
            ui.end_row();
        }
    });
}
