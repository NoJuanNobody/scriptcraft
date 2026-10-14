//! Window chrome: menu bar, toolbar, find bar, status bar, navigator, dialogs, shortcuts.

use egui::{Key, Modifiers, RichText, Ui, vec2};
use scriptcraft_doc::{Element, TitlePage};
use serde_json::json;

use crate::icons::{self, Icon};
use crate::{ScriptApp, UI_COMMANDS, View};

/// Run the command bound to a shortcut (UI commands first, then the engine registry).
pub fn dispatch_shortcut(app: &mut ScriptApp, key: Key, m: Modifiers) -> bool {
    let sc = crate::canvas::shortcut(key, m);
    // Clipboard shortcuts arrive as Copy/Cut/Paste events.
    if matches!(sc.as_str(), "Mod+C" | "Mod+X" | "Mod+V") {
        return false;
    }
    if let Some((id, ..)) = UI_COMMANDS.iter().find(|(_, _, _, k)| !k.is_empty() && k.eq_ignore_ascii_case(&sc)) {
        app.run(id, json!({}));
        return true;
    }
    let reg = app.session.registry.clone();
    let Some(spec) = reg.by_shortcut(&sc) else { return false };
    // File commands that need a path go through their UI versions (which ask for one).
    let id = match spec.id {
        "file.save" => "ui.save",
        "file.saveAs" => "ui.saveAs",
        "file.exportPdf" => "ui.exportPdf",
        other => other,
    };
    app.run(id, json!({}));
    true
}

/// Shortcuts that work wherever the keyboard focus is (when the page view does not have it).
pub fn global_shortcuts(app: &mut ScriptApp, ctx: &egui::Context) {
    // A page view with the keyboard handles its own keys.
    if crate::split::page_has_keys(app, ctx) {
        return;
    }
    let page_focused = (0..2).any(|i| ctx.memory(|m| m.has_focus(crate::canvas::canvas_id_for(i))));
    let keys: Vec<(Key, Modifiers)> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.command => Some((*key, *modifiers)),
                _ => None,
            })
            .collect()
    });
    let editing = ctx.memory(|m| m.focused().is_some()) && !page_focused;
    for (k, m) in keys {
        let sc = crate::canvas::shortcut(k, m);
        // Leave text-editing shortcuts to the focused text field.
        if editing && matches!(sc.as_str(), "Mod+Z" | "Mod+Shift+Z" | "Mod+A" | "Mod+C" | "Mod+X" | "Mod+V" | "Mod+Y") {
            continue;
        }
        dispatch_shortcut(app, k, m);
    }
}

fn item(app: &mut ScriptApp, ui: &mut Ui, label: &str, id: &str, key: &str) {
    let enabled = if id.starts_with("ui.") { true } else { app.session.registry.get(id).is_some_and(|c| (c.enabled)(&app.session).is_none()) };
    let mut b = egui::Button::new(label);
    if !key.is_empty() {
        b = b.shortcut_text(pretty_key(key));
    }
    if ui.add_enabled(enabled, b).clicked() {
        app.run(id, json!({}));
        ui.close();
    }
}

fn check(app: &mut ScriptApp, ui: &mut Ui, label: &str, on: bool, id: &str, params: serde_json::Value) {
    let mut v = on;
    if ui.checkbox(&mut v, label).clicked() {
        app.run(id, params);
        ui.close();
    }
}

/// `Mod+Shift+S` → `⇧⌘S` on macOS, `Ctrl+Shift+S` elsewhere.
pub fn pretty_key(k: &str) -> String {
    let k = k.split(" / ").next().unwrap_or(k);
    if cfg!(target_os = "macos") {
        let mut mods = String::new();
        let parts: Vec<&str> = k.split('+').collect();
        let (last, rest) = parts.split_last().map_or(("", &[][..]), |(l, r)| (*l, r));
        for p in rest {
            match *p {
                "Alt" => mods.push('⌥'),
                "Shift" => mods.push('⇧'),
                "Mod" => mods.push('⌘'),
                _ => {}
            }
        }
        let last = match last {
            "ArrowUp" => "↑",
            "ArrowDown" => "↓",
            "" => "+",
            l => l,
        };
        format!("{mods}{last}")
    } else {
        k.replace("Mod", "Ctrl")
    }
}

const ELEMENT_KEYS: [&str; 8] = ["Mod+1", "Mod+2", "Mod+3", "Mod+4", "Mod+5", "Mod+6", "Mod+7", "Mod+8"];

pub fn menu_bar(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    egui::Panel::top("menu").frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(8, 3))).show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                item(app, ui, "New Script", "file.new", "Mod+N");
                crate::drafts::new_menu(app, ui);
                item(app, ui, "Open…", "ui.open", "Mod+O");
                item(app, ui, "Import from Word, Celtx or PDF…", "ui.import", "");
                ui.menu_button("Open Recent", |ui| {
                    let recent = app.prefs.files.recent.clone();
                    if recent.is_empty() {
                        ui.label(RichText::new("No recent files").italics());
                    }
                    for p in recent {
                        let name = std::path::Path::new(&p).file_name().and_then(|n| n.to_str()).unwrap_or(&p).to_string();
                        if ui.button(name).on_hover_text(&p).clicked() {
                            app.run("ui.openRecent", json!({"path": p}));
                            ui.close();
                        }
                    }
                    ui.separator();
                    item(app, ui, "Clear Menu", "ui.clearRecent", "");
                });
                item(app, ui, "Open Sample Script", "file.sample", "");
                ui.separator();
                item(app, ui, "Save", "ui.save", "Mod+S");
                item(app, ui, "Save As…", "ui.saveAs", "Mod+Shift+S");
                ui.separator();
                item(app, ui, "Print…", "ui.print", "Mod+P");
                item(app, ui, "Export PDF…", "ui.exportPdf", "");
                item(app, ui, "Export Final Draft (FDX)…", "ui.exportFdx", "");
                item(app, ui, "Export Fountain…", "ui.exportFountain", "");
                ui.separator();
                item(app, ui, "Collaborate…", "ui.collaborate", "");
                ui.separator();
                item(app, ui, "Quit", "ui.quit", "Mod+Q");
            });
            ui.menu_button("Edit", |ui| {
                let undo = app.session.undo_label().map_or("Undo".to_string(), |l| format!("Undo {l}"));
                item(app, ui, &undo, "edit.undo", "Mod+Z");
                item(app, ui, "Redo", "edit.redo", "Mod+Shift+Z");
                ui.separator();
                if ui.add(egui::Button::new("Cut").shortcut_text(pretty_key("Mod+X"))).clicked() {
                    if let Ok(v) = app.execute("edit.cut", json!({})) {
                        ui.ctx().copy_text(v["text"].as_str().unwrap_or("").to_string());
                    }
                    ui.close();
                }
                if ui.add(egui::Button::new("Copy").shortcut_text(pretty_key("Mod+C"))).clicked() {
                    if let Ok(v) = app.execute("edit.copy", json!({})) {
                        ui.ctx().copy_text(v["text"].as_str().unwrap_or("").to_string());
                    }
                    ui.close();
                }
                item(app, ui, "Select All", "select.all", "Mod+A");
                ui.separator();
                item(app, ui, "Find and Replace…", "ui.find", "Mod+F");
                ui.separator();
                crate::bookmarks::menu(app, ui);
            });
            ui.menu_button("View", |ui| {
                let v = crate::split::active_view(app);
                check(app, ui, "Page View", v == View::Page, "ui.view.page", json!({}));
                check(app, ui, "Index Cards", v == View::Cards, "ui.view.cards", json!({}));
                check(app, ui, "Reports", v == View::Reports, "ui.view.reports", json!({}));
                check(app, ui, "Outline", v == View::Outline, "ui.view.outline", json!({}));
                check(app, ui, "Beat Board", v == View::BeatBoard, "ui.view.beats", json!({}));
                check(app, ui, "Story Map", app.story_map, "ui.toggleStoryMap", json!({}));
                ui.menu_button("Add Structure Point", |ui| {
                    for k in scriptcraft_doc::StructureKind::ALL {
                        if ui.button(format!("{} Before This Scene", k.label())).clicked() {
                            app.run("structure.add", json!({"kind": k.id()}));
                            ui.close();
                        }
                    }
                });
                check(app, ui, "Speed View (no page breaks)", app.session.speed, "ui.view.speed", json!({}));
                ui.separator();
                ui.menu_button("Split Screen", |ui| {
                    use crate::split::SplitMode;
                    for (label, mode) in [("One Pane", SplitMode::Off), ("Side by Side", SplitMode::SideBySide), ("Stacked", SplitMode::Stacked)] {
                        if ui.radio(app.split.mode == mode, label).clicked() {
                            app.run("ui.split", json!({"mode": mode.id()}));
                            ui.close();
                        }
                    }
                    if app.split.on() {
                        ui.separator();
                        ui.label(RichText::new("Second pane").strong());
                        for (label, v) in [
                            ("Page View", View::Page),
                            ("Index Cards", View::Cards),
                            ("Reports", View::Reports),
                            ("Outline", View::Outline),
                            ("Beat Board", View::BeatBoard),
                        ] {
                            if ui.radio(app.split.second == v, label).clicked() {
                                app.run("ui.splitView", json!({"pane": 1, "view": v.id()}));
                                ui.close();
                            }
                        }
                    }
                });
                ui.separator();
                check(app, ui, "Navigator", app.navigator, "ui.toggleNavigator", json!({}));
                check(app, ui, "Scene Numbers", app.session.script.settings.show_scene_numbers, "view.toggleSceneNumbers", json!({}));
                check(app, ui, "Dark Mode", app.dark, "ui.theme.toggle", json!({}));
                ui.separator();
                item(app, ui, "Zoom In", "ui.zoomIn", "Mod+=");
                item(app, ui, "Zoom Out", "ui.zoomOut", "Mod+-");
                item(app, ui, "Actual Size", "ui.zoomReset", "Mod+0");
                item(app, ui, "Fit Page Width", "ui.zoomFit", "Mod+9");
            });
            ui.menu_button("Format", |ui| {
                let cur = app.session.element();
                for (e, k) in Element::ALL.iter().zip(ELEMENT_KEYS) {
                    let b = egui::Button::selectable(cur == *e, e.name()).shortcut_text(pretty_key(k));
                    if ui.add(b).clicked() {
                        app.run("element.set", json!({"element": e.id()}));
                        ui.close();
                    }
                }
                ui.separator();
                item(app, ui, "Element Settings…", "ui.elementSettings", "");
                crate::drafts::apply_menu(app, ui);
                check(app, ui, "Element List on Enter in an Empty Paragraph", app.enter_picker, "ui.toggleEnterPicker", json!({}));
                ui.separator();
                item(app, ui, "Bold", "format.bold", "Mod+B");
                item(app, ui, "Italic", "format.italic", "Mod+I");
                item(app, ui, "Underline", "format.underline", "Mod+U");
                item(app, ui, "Plain Text", "format.plain", "Mod+Shift+T");
                ui.separator();
                item(app, ui, "Dual Dialogue", "dialogue.toggleDual", "Mod+D");
            });
            ui.menu_button("Document", |ui| {
                item(app, ui, "Title Page…", "ui.titlePage", "");
                item(app, ui, "Page Setup…", "ui.pageSetup", "");
                ui.separator();
                let tracking = app.session.script.settings.revision.tracking;
                check(app, ui, "Revision Mode", tracking, "revision.mode", json!({"on": !tracking}));
                item(app, ui, "Revisions…", "ui.revisions", "");
                ui.separator();
                item(app, ui, "Number Scenes", "scene.numberAll", "");
                item(app, ui, "Remove Scene Numbers", "scene.clearNumbers", "");
                ui.separator();
                let st = app.session.script.settings.clone();
                check(app, ui, "(MORE) and (CONT'D) at Page Breaks", st.more_contd, "settings.set", json!({"moreContd": !st.more_contd}));
                check(app, ui, "Automatic Character (CONT'D)", st.auto_contd, "settings.set", json!({"autoContd": !st.auto_contd}));
                check(app, ui, "Print Title Page", st.title_page, "settings.set", json!({"titlePage": !st.title_page}));
            });
            ui.menu_button("Production", |ui| production_menu(app, ui));
            ui.menu_button("Tools", |ui| {
                item(app, ui, "Next Misspelling", "spell.next", "Mod+;");
                check(app, ui, "Check Spelling as You Type", app.spelling, "ui.toggleSpelling", json!({}));
                crate::annotations::tools_menu(app, ui);
                ui.separator();
                ui.menu_button("Read Aloud", |ui| {
                    item(app, ui, "Read Aloud…", "ui.readAloud", "");
                    let state = app.read_aloud.state;
                    let label = if state == crate::speech::ReadState::Paused { "Resume Reading" } else { "Read from Caret" };
                    item(app, ui, label, "speech.play", "");
                    item(app, ui, "Pause Reading", "speech.pause", "");
                    item(app, ui, "Stop Reading", "speech.stop", "");
                });
                item(app, ui, "Dictation…", "ui.dictation", "");
                crate::drafts::tools_menu(app, ui);
            });
            ui.menu_button("Help", |ui| {
                item(app, ui, "About ScriptCraft", "ui.about", "");
                item(app, ui, "Open Sample Script", "file.sample", "");
            });
        });
    });
}

/// Production: locked pages, locked scene numbers, omitted scenes.
fn production_menu(app: &mut ScriptApp, ui: &mut Ui) {
    let st = app.session.script.settings.clone();
    check(app, ui, "Lock Pages", st.pages_locked, if st.pages_locked { "page.unlock" } else { "page.lock" }, json!({}));
    item(app, ui, "Re-lock Pages", "page.relock", "");
    ui.separator();
    let locked = st.scene_numbers_locked;
    check(app, ui, "Lock Scene Numbers", locked, if locked { "scene.unlockNumbers" } else { "scene.lockNumbers" }, json!({}));
    ui.separator();
    let focus = app.session.sel.focus.para;
    let omitted = app
        .session
        .script
        .scene_of(focus)
        .and_then(|i| app.session.script.scenes().get(i).map(|s| s.para))
        .and_then(|p| app.session.script.para(p))
        .is_some_and(|p| p.omitted);
    if omitted {
        item(app, ui, "Restore Omitted Scene", "scene.unomit", "");
    } else {
        item(app, ui, "Omit Scene", "scene.omit", "");
    }
}

fn sep(ui: &mut Ui) {
    ui.add_space(4.0);
    let t = ui.visuals().widgets.noninteractive.bg_stroke.color;
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, t);
    ui.add_space(4.0);
}

pub fn toolbar(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    egui::Panel::top("toolbar")
        .frame(
            egui::Frame::NONE
                .fill(t.chrome)
                .inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 6 })
                .stroke(egui::Stroke::new(1.0, t.chrome_line)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if icons::button(ui, Icon::Navigator, "Navigator", app.navigator).clicked() {
                    app.run("ui.toggleNavigator", json!({}));
                }
                sep(ui);
                for (icon, tip, id) in [
                    (Icon::New, "New script", "file.new"),
                    (Icon::Open, "Open…", "ui.open"),
                    (Icon::Save, "Save", "ui.save"),
                    (Icon::Pdf, "Export PDF…", "ui.exportPdf"),
                ] {
                    if icons::button(ui, icon, tip, false).clicked() {
                        app.run(id, json!({}));
                    }
                }
                sep(ui);
                for (icon, tip, id, enabled) in
                    [(Icon::Undo, "Undo", "edit.undo", app.session.can_undo()), (Icon::Redo, "Redo", "edit.redo", app.session.can_redo())]
                {
                    if ui.add_enabled_ui(enabled, |ui| icons::button(ui, icon, tip, false)).inner.clicked() {
                        app.run(id, json!({}));
                    }
                }
                sep(ui);
                // The element picker.
                let cur = app.session.element();
                ui.add_space(4.0);
                egui::ComboBox::from_id_salt("element")
                    .width(150.0)
                    .selected_text(RichText::new(cur.name()).strong())
                    .show_ui(ui, |ui| {
                        for (e, k) in Element::ALL.iter().zip(ELEMENT_KEYS) {
                            let label = format!("{:<16} {}", e.name(), pretty_key(k));
                            if ui.selectable_label(cur == *e, label).clicked() {
                                app.run("element.set", json!({"element": e.id()}));
                            }
                        }
                    })
                    .response
                    .on_hover_text("Element of the current paragraph (Enter and Tab change it as you type)");
                ui.add_space(4.0);
                let fs = scriptcraft_engine::cmd::format::caret_style(&app.session);
                let (a, b) = app.session.sel.ordered();
                let states: Vec<bool> = [scriptcraft_doc::Flag::Bold, scriptcraft_doc::Flag::Italic, scriptcraft_doc::Flag::Underline]
                    .iter()
                    .map(|f| if a == b { fs.get(*f) } else { app.session.script.all_have(a, b, *f) })
                    .collect();
                for (k, (label, tip, id, f)) in [
                    ("B", "Bold", "format.bold", scriptcraft_doc::Flag::Bold),
                    ("I", "Italic", "format.italic", scriptcraft_doc::Flag::Italic),
                    ("U", "Underline", "format.underline", scriptcraft_doc::Flag::Underline),
                ]
                .into_iter()
                .enumerate()
                {
                    let on = states.get(k).copied().unwrap_or(false);
                    let mut text = RichText::new(label).font(crate::canvas::courier_styled(15.0, scriptcraft_doc::Style::PLAIN.with(f, true)));
                    if on {
                        text = text.color(egui::Color32::WHITE);
                    }
                    let resp = ui.add(egui::Button::selectable(on, text).min_size(vec2(26.0, 24.0)));
                    if resp
                        .on_hover_text(format!(
                            "{tip} ({})",
                            pretty_key(match f {
                                scriptcraft_doc::Flag::Bold => "Mod+B",
                                scriptcraft_doc::Flag::Italic => "Mod+I",
                                scriptcraft_doc::Flag::Underline => "Mod+U",
                            })
                        ))
                        .clicked()
                    {
                        app.run(id, json!({}));
                    }
                }
                sep(ui);
                if icons::button(ui, Icon::Dual, "Dual dialogue", false).clicked() {
                    app.run("dialogue.toggleDual", json!({}));
                }
                if icons::button(ui, Icon::Numbers, "Scene numbers", app.session.script.settings.show_scene_numbers).clicked() {
                    app.run("view.toggleSceneNumbers", json!({}));
                }
                if icons::button(ui, Icon::Title, "Title page…", app.title_dialog).clicked() {
                    app.run("ui.titlePage", json!({}));
                }
                if icons::button(ui, Icon::Find, "Find and replace", app.find.is_some()).clicked() {
                    if app.find.is_some() {
                        app.find = None;
                    } else {
                        app.run("ui.find", json!({}));
                    }
                }
                sep(ui);
                for (icon, tip, id, v) in [
                    (Icon::Page, "Page view", "ui.view.page", View::Page),
                    (Icon::Cards, "Index cards", "ui.view.cards", View::Cards),
                    (Icon::Reports, "Reports", "ui.view.reports", View::Reports),
                    (Icon::Outline, "Outline", "ui.view.outline", View::Outline),
                    (Icon::Beats, "Beat board", "ui.view.beats", View::BeatBoard),
                ] {
                    if icons::button(ui, icon, tip, crate::split::active_view(app) == v).clicked() {
                        app.run(id, json!({}));
                    }
                }
                if icons::button(ui, Icon::StoryMap, "Story map", app.story_map).clicked() {
                    app.run("ui.toggleStoryMap", json!({}));
                }
                if icons::button(ui, Icon::Split, "Split screen", app.split.on()).clicked() {
                    app.run("ui.split", json!({}));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (icon, tip) = if app.dark { (Icon::Sun, "Light mode") } else { (Icon::Moon, "Dark mode") };
                    if icons::button(ui, icon, tip, false).clicked() {
                        app.run("ui.theme.toggle", json!({}));
                    }
                    ui.add_space(6.0);
                    if ui.small_button("+").on_hover_text("Zoom in").clicked() {
                        app.run("ui.zoomIn", json!({}));
                    }
                    ui.label(RichText::new(format!("{:.0}%", app.canvas.zoom * 100.0)).color(t.text_soft).monospace());
                    if ui.small_button("−").on_hover_text("Zoom out").clicked() {
                        app.run("ui.zoomOut", json!({}));
                    }
                });
            });
        });
}

pub fn find_bar(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    let Some(mut f) = app.find.take() else { return };
    let mut close = false;
    egui::Panel::top("find")
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(12, 6)).stroke(egui::Stroke::new(1.0, t.chrome_line)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find");
                let r = ui.add(egui::TextEdit::singleline(&mut f.find).desired_width(200.0).hint_text("text"));
                if f.focus {
                    r.request_focus();
                    f.focus = false;
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.label("Replace with");
                ui.add(egui::TextEdit::singleline(&mut f.replace).desired_width(160.0));
                ui.checkbox(&mut f.match_case, "Match case");
                if ui.button("Previous").clicked() {
                    app.run("edit.find", json!({"text": f.find, "matchCase": f.match_case, "backwards": true}));
                }
                if ui.button("Next").clicked() || enter {
                    app.run("edit.find", json!({"text": f.find, "matchCase": f.match_case}));
                    if enter {
                        r.request_focus();
                    }
                }
                if ui.button("Replace").clicked() {
                    app.run("edit.replace", json!({"find": f.find, "replace": f.replace, "matchCase": f.match_case}));
                }
                if ui.button("Replace All").clicked()
                    && let Ok(v) = app.execute("edit.replaceAll", json!({"find": f.find, "replace": f.replace, "matchCase": f.match_case}))
                {
                    app.set_status(format!("Replaced {} occurrence(s)", v["count"]));
                }
                if ui.button("Done").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            });
        });
    if !close {
        app.find = Some(f);
    } else {
        ui.memory_mut(|m| m.request_focus(crate::canvas::canvas_id()));
    }
}

pub fn status_bar(app: &mut ScriptApp, ui: &mut Ui, status: Option<String>) {
    let t = app.tokens();
    let l = app.session.layout();
    let focus = app.session.sel.focus;
    // The printed page number (`12A` with locked pages).
    let page = l.caret(focus).and_then(|c| l.pages.get(c.page)).map_or_else(|| "1".to_string(), |p| p.label.clone());
    let scenes = app.session.script.scenes().len();
    let scene = app.session.script.scene_of(focus.para).map_or("–".to_string(), |s| (s + 1).to_string());
    egui::Panel::bottom("status")
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(12, 4)).stroke(egui::Stroke::new(1.0, t.chrome_line)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let soft = |s: String| RichText::new(s).size(12.0).color(t.text_soft);
                ui.label(RichText::new(app.session.element().name()).size(12.0).strong().color(t.accent));
                ui.label(soft("·".into()));
                ui.label(soft(format!("Page {page} of {}", l.script_pages())));
                ui.label(soft("·".into()));
                ui.label(soft(format!("Scene {scene} of {scenes}")));
                ui.label(soft("·".into()));
                ui.label(soft(format!("{} words", app.session.script.word_count())));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let state = if app.session.dirty {
                        "Edited"
                    } else if app.session.path.is_some() {
                        "Saved"
                    } else {
                        ""
                    };
                    ui.label(soft(state.into()));
                    crate::collab_ui::status_badge(app, ui);
                    if let Some(s) = status {
                        ui.label(RichText::new(s).size(12.0).color(t.text));
                    }
                });
            });
        });
}

pub fn navigator(app: &mut ScriptApp, ui: &mut Ui) {
    let t = app.tokens();
    egui::Panel::left("navigator")
        .resizable(true)
        .default_size(280.0)
        .min_size(200.0)
        .frame(
            egui::Frame::NONE
                .fill(t.panel)
                .inner_margin(egui::Margin { left: 10, right: 8, top: 10, bottom: 8 })
                .stroke(egui::Stroke::new(1.0, t.chrome_line)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("NAVIGATOR").size(11.0).strong().color(t.text_soft));
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (i, name) in ["Scenes", "Characters", "Notes", "Bookmarks"].iter().enumerate() {
                    if ui.selectable_label(app.nav_tab == i, *name).clicked() {
                        app.nav_tab = i;
                    }
                }
            });
            ui.separator();
            let l = app.session.layout();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if app.nav_tab == 0 {
                    let cur = app.session.script.scene_of(app.session.sel.focus.para);
                    let scenes = app.session.script.scenes();
                    if scenes.is_empty() {
                        ui.label(RichText::new("No scenes yet. Start a paragraph with INT. or EXT.").color(t.text_soft));
                    }
                    // Structure points before each scene: (heading paragraph → reading-order index of its first point).
                    let mut first_point = std::collections::HashMap::new();
                    for (n, (slot, _)) in app.session.script.structure_points().iter().enumerate() {
                        first_point.entry(slot.para).or_insert(n);
                    }
                    for sc in scenes {
                        if let Some(&n) = first_point.get(&Some(sc.para)) {
                            let pts = app.session.script.para(sc.para).map(|p| p.structure.clone()).unwrap_or_default();
                            crate::outline::nav_points(app, ui, &pts, n);
                        }
                        let on = cur == Some(sc.index);
                        let num = sc.number.clone().unwrap_or_else(|| (sc.index + 1).to_string());
                        let resp = ui
                            .scope(|ui| {
                                let (r, resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), if sc.summary.is_empty() { 26.0 } else { 42.0 }),
                                    egui::Sense::click(),
                                );
                                let fill = if on {
                                    t.accent.gamma_multiply(0.16)
                                } else if resp.hovered() {
                                    ui.visuals().widgets.hovered.weak_bg_fill
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(r, 5.0, fill);
                                if let Some(c) = sc.color {
                                    let strip = egui::Rect::from_min_size(r.min + vec2(1.0, 4.0), vec2(3.0, r.height() - 8.0));
                                    ui.painter().rect_filled(strip, 1.5, crate::cards::card_rgb(c));
                                }
                                let p = ui.painter();
                                p.text(
                                    r.left_top() + vec2(8.0, 6.0),
                                    egui::Align2::LEFT_TOP,
                                    num,
                                    egui::FontId::monospace(12.0),
                                    if on { t.accent } else { t.text_soft },
                                );
                                let width = r.width() - 70.0;
                                let head = elide(&sc.heading, (width / 6.3) as usize);
                                p.text(r.left_top() + vec2(32.0, 6.0), egui::Align2::LEFT_TOP, head, egui::FontId::proportional(12.5), t.text);
                                p.text(
                                    r.right_top() + vec2(-8.0, 6.0),
                                    egui::Align2::RIGHT_TOP,
                                    l.page_label_of(sc.para),
                                    egui::FontId::proportional(11.0),
                                    t.text_soft,
                                );
                                if !sc.summary.is_empty() {
                                    p.text(
                                        r.left_top() + vec2(32.0, 23.0),
                                        egui::Align2::LEFT_TOP,
                                        elide(&sc.summary, (width / 5.6) as usize),
                                        egui::FontId::proportional(11.0),
                                        t.text_soft,
                                    );
                                }
                                resp
                            })
                            .inner;
                        if resp.clicked() {
                            app.run("scene.goto", json!({"index": sc.index}));
                            crate::split::show_page(app);
                        }
                    }
                    if let Some(&n) = first_point.get(&None) {
                        let pts = app.session.script.structure_tail.clone();
                        crate::outline::nav_points(app, ui, &pts, n);
                    }
                } else if app.nav_tab == 2 {
                    crate::annotations::notes_tab(app, ui);
                } else if app.nav_tab == 3 {
                    crate::bookmarks::tab(app, ui);
                } else {
                    let chars = scriptcraft_engine::reports::characters(&app.session.script, &l);
                    for c in chars {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&c.name).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("{} lines · p.{}", c.speeches, c.first_page_label)).size(11.0).color(t.text_soft));
                            });
                        });
                    }
                }
            });
        });
}

/// Cut `s` to `n` characters with an ellipsis.
pub fn elide(s: &str, n: usize) -> String {
    let n = n.max(4);
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>().trim_end()) }
}

pub fn dialogs(app: &mut ScriptApp, ctx: &egui::Context) {
    crate::production::dialogs(app, ctx);
    if app.title_dialog {
        let mut open = true;
        let mut tp: TitlePage = app.session.script.title.clone();
        let mut changed = false;
        let mut lines = tp.lines.clone();
        let mut lines_changed = false;
        let mut action: Option<&str> = None;
        egui::Window::new("Title Page").open(&mut open).resizable(false).collapsible(false).default_width(480.0).show(ctx, |ui| {
            if tp.is_free_form() {
                ui.label(RichText::new("Free-form title page: each row is one printed line, top to bottom.").size(11.5));
                ui.add_space(4.0);
                let mut remove = None;
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    egui::Grid::new("tp-lines").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
                        for (i, l) in lines.iter_mut().enumerate() {
                            egui::ComboBox::from_id_salt(("tp-align", i)).width(70.0).selected_text(l.align.name()).show_ui(ui, |ui| {
                                for a in [scriptcraft_doc::Align::Left, scriptcraft_doc::Align::Center, scriptcraft_doc::Align::Right] {
                                    lines_changed |= ui.selectable_value(&mut l.align, a, a.name()).changed();
                                }
                            });
                            lines_changed |= ui.add_sized([250.0, 22.0], egui::TextEdit::singleline(&mut l.text)).changed();
                            ui.horizontal(|ui| {
                                for (label, on) in [("B", &mut l.style.bold), ("I", &mut l.style.italic), ("U", &mut l.style.underline)] {
                                    if ui.selectable_label(*on, label).clicked() {
                                        *on = !*on;
                                        lines_changed = true;
                                    }
                                }
                            });
                            if ui.small_button("Delete").on_hover_text("Delete this line").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                });
                if let Some(i) = remove {
                    lines.remove(i);
                    lines_changed = true;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Add Line").clicked() {
                        lines.push(scriptcraft_doc::TitleLine::new(scriptcraft_doc::Align::Center, ""));
                        lines_changed = true;
                    }
                    if ui.button("Use Fields Instead").on_hover_text("Lay out the title page from its fields").clicked() {
                        action = Some("title.useFields");
                    }
                });
                return;
            }
            egui::Grid::new("tp").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                for (label, field, multi) in [
                    ("Title", "title", false),
                    ("Credit", "credit", false),
                    ("Author", "author", false),
                    ("Source", "source", false),
                    ("Draft date", "draftDate", false),
                    ("Contact", "contact", true),
                    ("Copyright", "copyright", false),
                    ("Notes", "notes", true),
                ] {
                    ui.label(label);
                    let mut v = tp.get(field).unwrap_or("").to_string();
                    let r = if multi {
                        ui.add(egui::TextEdit::multiline(&mut v).desired_rows(2).desired_width(300.0))
                    } else {
                        ui.add(egui::TextEdit::singleline(&mut v).desired_width(300.0))
                    };
                    if r.changed() {
                        tp.set(field, &v);
                        changed = true;
                    }
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            if ui.button("Free-Form Layout").on_hover_text("Edit the title page line by line").clicked() {
                action = Some("title.makeFreeForm");
            }
        });
        if changed {
            let fields = serde_json::to_value(&tp).unwrap_or_default();
            app.run("title.set", json!({"fields": fields}));
        }
        if lines_changed {
            app.run("title.setLines", json!({"lines": lines}));
        }
        if let Some(id) = action {
            app.run(id, json!({}));
        }
        app.title_dialog = open;
    }
    if app.element_dialog {
        let mut open = true;
        let mut changes: Vec<serde_json::Value> = Vec::new();
        let mut reset = false;
        egui::Window::new("Element Settings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("Indents in inches from the left edge of the paper; space before in blank lines.").size(11.5));
                ui.add_space(6.0);
                egui::Grid::new("elements").num_columns(11).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
                    for h in [
                        "Element",
                        "Left",
                        "Right",
                        "Before",
                        "Spacing",
                        "Align",
                        "Caps",
                        "New page",
                        "Enter goes to",
                        "Tab goes to",
                        "Tab when empty",
                    ] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for e in Element::ALL {
                        let f = app.session.script.settings.elements.get(e);
                        let id = e.id();
                        ui.label(e.name());
                        let (mut l, mut r, mut b, mut caps) = (f.left, f.right, f.before, f.uppercase);
                        let (mut sp, mut np) = (f.spacing, f.new_page);
                        if ui.add(egui::DragValue::new(&mut l).speed(0.05).range(0.0..=8.0).fixed_decimals(2)).changed() {
                            changes.push(json!({"element": id, "left": l}));
                        }
                        if ui.add(egui::DragValue::new(&mut r).speed(0.05).range(0.5..=8.5).fixed_decimals(2)).changed() {
                            changes.push(json!({"element": id, "right": r}));
                        }
                        if ui.add(egui::DragValue::new(&mut b).range(0..=6)).changed() {
                            changes.push(json!({"element": id, "before": b}));
                        }
                        if ui.add(egui::DragValue::new(&mut sp).range(1..=3)).on_hover_text("1 = single, 2 = double-spaced").changed() {
                            changes.push(json!({"element": id, "spacing": sp}));
                        }
                        egui::ComboBox::from_id_salt(("align", id)).width(70.0).selected_text(f.align.name()).show_ui(ui, |ui| {
                            for a in [scriptcraft_doc::Align::Left, scriptcraft_doc::Align::Center, scriptcraft_doc::Align::Right] {
                                if ui.selectable_label(f.align == a, a.name()).clicked() {
                                    changes.push(json!({"element": id, "align": a.name()}));
                                }
                            }
                        });
                        if ui.checkbox(&mut caps, "").changed() {
                            changes.push(json!({"element": id, "uppercase": caps}));
                        }
                        if ui.checkbox(&mut np, "").on_hover_text("Start each paragraph of this element on a new page").changed() {
                            changes.push(json!({"element": id, "newPage": np}));
                        }
                        for (key, cur) in [("enter", f.enter), ("tab", f.tab), ("tabEmpty", f.tab_empty)] {
                            egui::ComboBox::from_id_salt((key, id)).width(110.0).selected_text(cur.name()).show_ui(ui, |ui| {
                                for t in Element::ALL {
                                    if ui.selectable_label(cur == t, t.name()).clicked() {
                                        changes.push(json!({"element": id, key: t.id()}));
                                    }
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(8.0);
                if ui.button("Reset to Standard").clicked() {
                    reset = true;
                }
            });
        for c in changes {
            app.run("settings.element", c);
        }
        if reset {
            app.run("settings.resetElements", json!({}));
        }
        app.element_dialog = open;
    }
    if app.about {
        let mut open = true;
        egui::Window::new("About ScriptCraft").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.heading("ScriptCraft");
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("Open-source screenwriting in pure Rust, by the ArtCraft community.");
            ui.label("MIT OR Apache-2.0. Typeface: Courier Prime (SIL Open Font License 1.1).");
            ui.separator();
            let loc = app.settings_location();
            let settings = loc["file"].as_str().map_or_else(|| "not stored (this run keeps settings in memory)".to_string(), str::to_string);
            ui.label(RichText::new(format!("Settings: {settings}")).size(11.5));
            if let Some(rule) = loc["rule"].as_str() {
                ui.label(RichText::new(format!("Chosen by: {rule}")).size(11.5));
            }
            if let Some(d) = loc["data"].as_str() {
                ui.label(RichText::new(format!("Recovery, backups, dictionary: {d}")).size(11.5));
            }
            ui.hyperlink_to("getartcraft.com/apps", "https://getartcraft.com/apps");
        });
        app.about = open;
    }
    if let Some(r) = app.recovery_offer.clone() {
        let mut choice = None;
        egui::Window::new("Recover unsaved changes?").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(
            ctx,
            |ui| {
                let name = r
                    .path
                    .as_deref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("an untitled script")
                    .to_string();
                ui.label(format!(
                    "ScriptCraft did not close normally. An autosaved copy of {name} ({} scenes) is available.",
                    r.script.scenes().len()
                ));
                ui.horizontal(|ui| {
                    if ui.button("Recover").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("Discard").clicked() {
                        choice = Some(false);
                    }
                });
            },
        );
        match choice {
            Some(true) => {
                app.session.replace_script(r.script);
                app.session.path = r.path.map(std::path::PathBuf::from);
                app.session.dirty = true;
                app.recovery_offer = None;
                app.set_status("Recovered unsaved changes; save to keep them");
            }
            Some(false) => {
                app.recovery_offer = None;
                app.clear_recovery();
            }
            None => {}
        }
    }
    if let Some((id, params)) = app.confirm_replace.clone() {
        let mut choice = None;
        egui::Window::new("Save changes first?").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(
            ctx,
            |ui| {
                ui.label("This script has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Don't Save").clicked() {
                        choice = Some(2);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(0);
                    }
                });
            },
        );
        if let Some(c) = choice {
            if c == 1 {
                let _ = app.execute("ui.save", json!({}));
            }
            if c == 2 || (c == 1 && !app.session.dirty) {
                app.session.dirty = false;
                let _ = app.execute(&id, params);
            }
            app.confirm_replace = None;
        }
    }
    if app.confirm_close {
        egui::Window::new("Save changes?").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.label("This script has unsaved changes. Save them before closing?");
            ui.horizontal(|ui| {
                let save = ui.button("Save");
                let discard = ui.button("Don't Save");
                let cancel = ui.button("Cancel");
                for (n, r) in [("Save", &save), ("Don't Save", &discard), ("Cancel", &cancel)] {
                    app.dialog_buttons.push((n.to_string(), r.rect));
                }
                if save.clicked() {
                    let _ = app.execute("ui.save", json!({}));
                    app.confirm_close = false;
                    if !app.session.dirty {
                        app.quit_requested = true;
                    }
                }
                if discard.clicked() {
                    app.confirm_close = false;
                    app.quit_requested = true;
                }
                if cancel.clicked() {
                    app.confirm_close = false;
                }
            });
        });
    }
}
