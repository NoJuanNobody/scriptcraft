//! Production dialogs: Page Setup (paper, headers and footers) and Print, plus printing itself.

use egui::{RichText, vec2};
use scriptcraft_engine::CmdError;
use serde_json::{Value, json};

use crate::ScriptApp;

/// Which production dialogs are open.
#[derive(Default, Clone, Debug)]
pub struct Dialogs {
    pub page_setup: bool,
    /// The Print dialog's choices while it is open.
    pub print: Option<PrintForm>,
    pub revisions: bool,
}

/// The Print dialog's choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintForm {
    pub all: bool,
    pub from: usize,
    pub to: usize,
    pub title_page: bool,
    pub revised_only: bool,
    /// The printer chosen (`None`: the system's default).
    pub printer: Option<String>,
    pub copies: u32,
    /// The printers found when the dialog opened, or why there are none.
    pub printers: Result<Vec<crate::printing::Printer>, String>,
}

impl PrintForm {
    pub fn params(&self) -> Value {
        let mut v = json!({"titlePage": self.title_page, "revisedOnly": self.revised_only});
        if let Some(o) = v.as_object_mut() {
            if !self.all {
                o.insert("from".into(), json!(self.from));
                o.insert("to".into(), json!(self.to));
            }
            if let Some(p) = &self.printer {
                o.insert("printer".into(), json!(p));
            }
            if self.copies > 1 {
                o.insert("copies".into(), json!(self.copies));
            }
        }
        v
    }
}

pub fn dialogs(app: &mut ScriptApp, ctx: &egui::Context) {
    if app.production.page_setup {
        page_setup(app, ctx);
    }
    if app.production.print.is_some() {
        print_dialog(app, ctx);
    }
    if app.production.revisions {
        revisions(app, ctx);
    }
}

fn rgb(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

fn revisions(app: &mut ScriptApp, ctx: &egui::Context) {
    let mut open = true;
    let r = app.session.script.settings.revision.clone();
    let marks: usize = app.session.script.paras.iter().map(|p| p.revs.len()).sum();
    let pages = app.session.layout().page_rev.iter().filter(|x| x.is_some()).count();
    let mut runs: Vec<(&str, serde_json::Value)> = Vec::new();
    egui::Window::new("Revisions")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| {
            let mut tracking = r.tracking;
            if ui.checkbox(&mut tracking, RichText::new("Revision mode: mark changes").strong()).changed() {
                runs.push(("revision.mode", json!({"on": tracking})));
            }
            ui.label(RichText::new("New and changed text is marked with the active set; pages with changes show its label.").size(11.5).weak());
            ui.add_space(6.0);
            egui::Grid::new("revsets").num_columns(5).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
                for h in ["Active", "Colour", "Set", "Date", "Mark"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (i, st) in r.sets.iter().enumerate() {
                    if ui.radio(r.active == i, "").clicked() && r.active != i {
                        runs.push(("revision.set", json!({"active": i})));
                    }
                    let (rect, _) = ui.allocate_exact_size(vec2(26.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, rgb(st.color));
                    ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(0.6, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);
                    let mut name = st.name.clone();
                    if ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut name)).changed() {
                        runs.push(("revision.editSet", json!({"set": i, "name": name})));
                    }
                    let mut date = st.date.clone();
                    if ui.add_sized([90.0, 20.0], egui::TextEdit::singleline(&mut date).hint_text("date")).changed() {
                        runs.push(("revision.editSet", json!({"set": i, "date": date})));
                    }
                    let mut mark = st.mark.clone();
                    if ui.add_sized([30.0, 20.0], egui::TextEdit::singleline(&mut mark)).changed() && !mark.trim().is_empty() {
                        runs.push(("revision.editSet", json!({"set": i, "mark": mark})));
                    }
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            let (mut show, mut color) = (r.show_marks, r.color_pages);
            if ui.checkbox(&mut show, "Show revision marks in the margin").changed() {
                runs.push(("revision.set", json!({"showMarks": show})));
            }
            if ui.checkbox(&mut color, "Colour revised pages").changed() {
                runs.push(("revision.set", json!({"colorPages": color})));
            }
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{marks} mark(s) on {pages} page(s).")).size(11.5).weak());
            ui.horizontal(|ui| {
                if ui.add_enabled(marks > 0, egui::Button::new("Clear Marks in Selection")).clicked() {
                    runs.push(("revision.clearMarks", json!({"selection": true})));
                }
                if ui.add_enabled(marks > 0, egui::Button::new("Clear All Marks")).clicked() {
                    runs.push(("revision.clearMarks", json!({})));
                }
            });
        });
    for (id, p) in runs {
        app.run(id, p);
    }
    app.production.revisions = open;
}

/// Open (or close) the Print dialog.
pub fn toggle_print(app: &mut ScriptApp) {
    if app.production.print.take().is_some() {
        return;
    }
    let pages = app.session.layout().script_pages().max(1);
    let title_page = app.session.script.settings.title_page && !app.session.script.title.is_empty();
    // Ask the system for its printers each time the dialog opens (they come and go).
    let printers = app.printer.as_mut().map_or_else(|| Ok(Vec::new()), |h| h.printers(true));
    let printer = printers.as_ref().ok().and_then(|l| l.iter().find(|p| p.default).or(l.first())).map(|p| p.name.clone());
    app.production.print = Some(PrintForm { all: true, from: 1, to: pages, title_page, revised_only: false, printer, copies: 1, printers });
}

/// Open in Viewer: render the chosen pages and hand the PDF to the system's viewer, whose Print
/// command can print it too. In the browser it is downloaded. Without a print host (tests and
/// headless runs) the PDF is only written to a temporary file. Printing straight to a printer is
/// `print.send` ([`crate::printing`]).
pub fn print_now(app: &mut ScriptApp, p: &Value) -> Result<Value, CmdError> {
    let r = app.session.run("print.render", p)?;
    let name = r["name"].as_str().unwrap_or("Untitled.pdf").to_string();
    let bytes = scriptcraft_engine::b64::decode(r["data"].as_str().unwrap_or("")).map_err(CmdError::Failed)?;
    let pages = r["pages"].clone();
    if let Some(h) = &app.host {
        h.save(&name, bytes);
        app.set_status(format!("Downloaded {name}"));
        return Ok(json!({"name": name, "pages": pages, "downloaded": true}));
    }
    if let Some(h) = app.printer.as_mut().filter(|_| app.dialogs) {
        return match h.open_viewer(&name, bytes) {
            Ok(path) => {
                app.set_status(format!("Opened {name} in the PDF viewer"));
                Ok(json!({"name": name, "pages": pages, "path": path, "opened": true}))
            }
            Err(e) => Err(CmdError::Failed(e)),
        };
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = std::env::temp_dir().join("scriptcraft-print");
        std::fs::create_dir_all(&dir).map_err(|e| CmdError::Failed(format!("could not prepare the print file: {e}")))?;
        let path = dir.join(format!("{}-{}", std::process::id(), name.replace(['/', '\\'], "-")));
        std::fs::write(&path, &bytes).map_err(|e| CmdError::Failed(format!("could not write the print file: {e}")))?;
        app.set_status(format!("Print file written to {}", path.display()));
        Ok(json!({"name": name, "pages": pages, "path": path.display().to_string(), "opened": false}))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = bytes;
        Err(CmdError::Disabled("printing needs the page's file host".into()))
    }
}

fn print_dialog(app: &mut ScriptApp, ctx: &egui::Context) {
    let Some(mut form) = app.production.print.clone() else { return };
    let mut open = true;
    let mut go: Option<&str> = None;
    let mut cancelled = false;
    let pages = app.session.layout().script_pages().max(1);
    let has_title = !app.session.script.title.is_empty();
    let has_printer = app.printer.is_some();
    let in_dialog = app.printer.as_ref().is_some_and(|h| h.chooses_in_dialog());
    egui::Window::new("Print")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| {
            ui.radio_value(&mut form.all, true, format!("All pages (1–{pages})"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut form.all, false, "Pages");
                let a = ui.add(egui::DragValue::new(&mut form.from).range(1..=pages));
                ui.label("to");
                let b = ui.add(egui::DragValue::new(&mut form.to).range(1..=pages));
                if a.changed() || b.changed() {
                    form.all = false;
                    form.to = form.to.max(form.from);
                }
            });
            ui.add_enabled_ui(has_title, |ui| ui.checkbox(&mut form.title_page, "Include the title page"));
            let revised = app.session.layout().page_rev.iter().any(Option::is_some);
            ui.add_enabled_ui(revised || form.revised_only, |ui| ui.checkbox(&mut form.revised_only, "Only revised pages"));
            ui.add_space(4.0);
            let shown = app.session.run("print.pages", &form.params()).ok().map_or(0, |v| v["count"].as_u64().unwrap_or(0));
            ui.add_space(4.0);
            if has_printer && !in_dialog {
                printer_choice(ui, &mut form);
            }
            let note = if !has_printer {
                format!("{shown} page(s). The pages are written as a PDF.")
            } else if in_dialog {
                format!("{shown} page(s). Choose the printer and copies in the browser's print window.")
            } else {
                format!("{shown} page(s).")
            };
            ui.label(RichText::new(note).size(11.5).weak());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // With no list at all (it couldn't be read) the default printer may still work.
                let can_print = shown > 0 && (!has_printer || in_dialog || form.printers.as_ref().map_or(true, |l| !l.is_empty()));
                let print = ui.add_enabled(can_print, egui::Button::new("Print…"));
                app.dialog_buttons.push(("Print…".into(), print.rect));
                if print.clicked() {
                    go = Some(if has_printer { "print.send" } else { "ui.printNow" });
                }
                if has_printer {
                    let label = if app.host.is_some() { "Download PDF" } else { "Open in Viewer" };
                    let view = ui.add_enabled(shown > 0, egui::Button::new(label));
                    app.dialog_buttons.push((label.into(), view.rect));
                    if view.clicked() {
                        go = Some("ui.printNow");
                    }
                }
                let cancel = ui.button("Cancel");
                app.dialog_buttons.push(("Cancel".into(), cancel.rect));
                if cancel.clicked() {
                    cancelled = true;
                }
            });
        });
    if let Some(id) = go {
        app.production.print = None;
        if let Err(e) = app.execute(id, form.params()) {
            app.set_status(format!("Could not print: {e}"));
        }
        return;
    }
    app.production.print = (open && !cancelled).then_some(form);
}

/// Printer and copies in the Print window.
fn printer_choice(ui: &mut egui::Ui, form: &mut PrintForm) {
    egui::Grid::new("printer").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Printer");
        match &form.printers {
            Ok(list) if !list.is_empty() => {
                let shown = form.printer.clone().unwrap_or_else(|| "Default printer".into());
                egui::ComboBox::from_id_salt("printer-list").selected_text(shown).width(220.0).show_ui(ui, |ui| {
                    for p in list {
                        let label = if p.default { format!("{} (default)", p.name) } else { p.name.clone() };
                        if ui.selectable_label(form.printer.as_deref() == Some(p.name.as_str()), label).clicked() {
                            form.printer = Some(p.name.clone());
                        }
                    }
                });
            }
            Ok(_) => {
                ui.label(RichText::new("No printers found. Add one in the system's settings.").weak());
            }
            Err(e) => {
                ui.label(RichText::new(format!("Default printer ({e})")).weak());
            }
        }
        ui.end_row();
        ui.label("Copies");
        ui.add(egui::DragValue::new(&mut form.copies).range(1..=crate::printing::MAX_COPIES));
        ui.end_row();
    });
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
