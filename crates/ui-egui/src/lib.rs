//! ScriptCraft's egui front end: a Final Draft-style window with a menu bar, a toolbar with the
//! element picker, a scene navigator, the page view, index cards, reports and a status bar.
//!
//! The UI is thin: every action goes through [`ScriptApp::execute`], which runs `ui.*` commands
//! here and everything else through the engine's command registry.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod canvas;
pub mod cards;
mod chrome;
pub mod control;
pub mod icons;
pub mod persist;
mod reports;
pub mod theme;

use std::sync::mpsc::Receiver;

use scriptcraft_engine::{CmdError, Session};
use serde_json::{Value, json};

pub use control::ControlRequest;

/// The main view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Page,
    Cards,
    Reports,
}

impl View {
    pub fn id(self) -> &'static str {
        match self {
            View::Page => "page",
            View::Cards => "cards",
            View::Reports => "reports",
        }
    }
}

/// UI-only commands (`ui.*`): id, label, menu location, shortcut.
pub const UI_COMMANDS: &[(&str, &str, &str, &str)] = &[
    ("ui.view.page", "Page View", "View", "Mod+Alt+1"),
    ("ui.view.cards", "Index Cards", "View", "Mod+Alt+2"),
    ("ui.view.reports", "Reports", "View", "Mod+Alt+3"),
    ("ui.toggleNavigator", "Navigator", "View", "Mod+Alt+N"),
    ("ui.theme.toggle", "Dark Mode", "View", "Mod+Alt+D"),
    ("ui.theme.dark", "Dark Theme", "View", ""),
    ("ui.theme.light", "Light Theme", "View", ""),
    ("ui.zoomIn", "Zoom In", "View", "Mod+="),
    ("ui.zoomOut", "Zoom Out", "View", "Mod+-"),
    ("ui.zoomReset", "Actual Size", "View", "Mod+0"),
    ("ui.zoomFit", "Fit Page Width", "View", "Mod+9"),
    ("ui.view.speed", "Speed View", "View", "Mod+Alt+4"),
    ("ui.find", "Find…", "Edit", "Mod+F"),
    ("ui.toggleSpelling", "Check Spelling as You Type", "Tools", ""),
    ("ui.toggleEnterPicker", "Element List on Enter in an Empty Paragraph", "Format", ""),
    ("ui.titlePage", "Title Page…", "Document", ""),
    ("ui.elementSettings", "Element Settings…", "Format", ""),
    ("ui.open", "Open…", "File", "Mod+O"),
    ("ui.save", "Save", "File", "Mod+S"),
    ("ui.saveAs", "Save As…", "File", "Mod+Shift+S"),
    ("ui.exportPdf", "Export PDF…", "File", "Mod+P"),
    ("ui.exportFdx", "Export Final Draft (FDX)…", "File", ""),
    ("ui.exportFountain", "Export Fountain…", "File", ""),
    ("ui.about", "About ScriptCraft", "Help", ""),
    ("ui.quit", "Quit ScriptCraft", "File", "Mod+Q"),
    ("ui.openRecent", "Open Recent", "File", ""),
    ("ui.clearRecent", "Clear Recent Files", "File", ""),
];

/// The application state.
pub struct ScriptApp {
    pub session: Session,
    pub view: View,
    pub dark: bool,
    pub navigator: bool,
    /// Which navigator tab: 0 scenes, 1 characters.
    pub nav_tab: usize,
    pub canvas: canvas::CanvasState,
    pub find: Option<FindBar>,
    pub title_dialog: bool,
    pub element_dialog: bool,
    /// The index card being dragged.
    pub card_drag: Option<usize>,
    /// The tags field of an index card's menu: (scene, text).
    pub tag_edit: Option<(usize, String)>,
    pub about: bool,
    /// Native file dialogs are allowed (off in tests and headless screenshots).
    pub dialogs: bool,
    /// Ask before closing with unsaved changes.
    pub confirm_close: bool,
    pub quit_requested: bool,
    /// Quit was chosen (menu, Mod+Q, control channel): the host asks the window to close, which
    /// goes through [`ScriptApp::close_requested`] like the close button.
    pub close_wanted: bool,
    /// Underline misspelled words.
    pub spelling: bool,
    /// Enter in an empty paragraph shows the element list (otherwise it changes the element).
    pub enter_picker: bool,
    /// The settings folder for state kept between runs (user dictionary…); `None` in tests.
    pub state_dir: Option<std::path::PathBuf>,
    /// Preferences kept between runs (recent files, autosave and backup settings).
    pub prefs: persist::Prefs,
    /// An autosaved script found at start-up, offered for recovery.
    pub recovery_offer: Option<persist::Recovery>,
    /// Buttons of the open modal dialog this frame (label, screen rect), for `ui.inspect`.
    pub dialog_buttons: Vec<(String, egui::Rect)>,
    /// An open/new waiting for the user to save or discard unsaved changes.
    pub confirm_replace: Option<(String, Value)>,
    /// When the script was last autosaved (seconds of UI time).
    last_autosave: f64,
    /// Status-bar message and when it was set (seconds since start).
    status: Option<(String, f64)>,
    control: Option<Receiver<ControlRequest>>,
    /// Synthetic input from the control channel, fed in by `raw_input_hook`.
    pub synthetic: Vec<egui::Event>,
    pending_shots: Vec<(Option<String>, std::sync::mpsc::Sender<Value>)>,
    fonts_installed: bool,
    /// Frames since the fonts were installed (they take effect on the next frame).
    fonts_frames: u8,
    sent_title: String,
}

/// The find/replace bar.
#[derive(Default, Clone)]
pub struct FindBar {
    pub find: String,
    pub replace: String,
    pub match_case: bool,
    pub focus: bool,
}

impl ScriptApp {
    pub fn new(session: Session) -> Self {
        ScriptApp {
            session,
            view: View::Page,
            dark: false,
            navigator: true,
            nav_tab: 0,
            canvas: canvas::CanvasState::default(),
            find: None,
            title_dialog: false,
            element_dialog: false,
            card_drag: None,
            tag_edit: None,
            about: false,
            dialogs: true,
            confirm_close: false,
            quit_requested: false,
            close_wanted: false,
            spelling: true,
            enter_picker: true,
            state_dir: None,
            prefs: persist::Prefs::default(),
            recovery_offer: None,
            last_autosave: 0.0,
            confirm_replace: None,
            dialog_buttons: Vec::new(),
            status: None,
            control: None,
            synthetic: Vec::new(),
            pending_shots: Vec::new(),
            fonts_installed: false,
            fonts_frames: 0,
            sent_title: String::new(),
        }
    }

    /// Keep state (user dictionary, preferences, autosave, backups) in `dir` and load what is
    /// there; an autosaved script left by a run that did not end cleanly is offered back.
    pub fn with_state_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.session.user_words = persist::load_words(&dir);
        self.prefs = persist::load_prefs(&dir);
        if let Some(d) = self.prefs.dark {
            self.dark = d;
        }
        self.canvas.zoom = self.prefs.zoom;
        self.spelling = self.prefs.spelling;
        self.enter_picker = self.prefs.enter_picker;
        self.recovery_offer = persist::read_recovery(&dir);
        self.state_dir = Some(dir);
        self
    }

    /// Write preferences (the app calls this on exit). A script with no unsaved changes, or one
    /// the user chose not to save, leaves no recovery file behind.
    pub fn on_exit(&mut self) {
        let Some(dir) = self.state_dir.clone() else { return };
        self.prefs.dark = Some(self.dark);
        self.prefs.zoom = self.canvas.zoom;
        self.prefs.spelling = self.spelling;
        self.prefs.enter_picker = self.enter_picker;
        let _ = persist::save_prefs(&dir, &self.prefs);
        if !self.session.dirty || self.quit_requested {
            persist::clear_recovery(&dir);
        }
    }

    /// The process is being terminated: keep unsaved changes as a recovery copy (offered at the
    /// next start) and save preferences. Unlike [`ScriptApp::on_exit`] it never clears recovery.
    pub fn emergency_save(&mut self) {
        let Some(dir) = self.state_dir.clone() else { return };
        if self.session.dirty {
            let _ = persist::write_recovery(&dir, self.session.path.as_deref(), &self.session.script);
        }
        self.prefs.dark = Some(self.dark);
        self.prefs.zoom = self.canvas.zoom;
        let _ = persist::save_prefs(&dir, &self.prefs);
    }

    /// Before a save over an existing file: keep the old version in the backups folder.
    fn before_save(&mut self, id: &str, params: &Value) {
        let Some(dir) = self.state_dir.clone() else { return };
        if !matches!(id, "file.save" | "file.saveAs") {
            return;
        }
        let target = params.get("path").and_then(Value::as_str).map(std::path::PathBuf::from).or_else(|| self.session.path.clone());
        if let Some(t) = target
            && let Err(e) = persist::backup(&dir, &t, self.prefs.backups)
        {
            self.status = Some((format!("Backup failed: {e}"), 0.0));
        }
    }

    /// After a successful open or save: remember the file; a saved script needs no recovery copy.
    fn after_file(&mut self, id: &str) {
        let Some(dir) = self.state_dir.clone() else { return };
        if !matches!(id, "file.open" | "file.save" | "file.saveAs") {
            return;
        }
        if let Some(p) = self.session.path.as_ref().map(|p| p.display().to_string()) {
            self.prefs.touch_recent(&p);
            let _ = persist::save_prefs(&dir, &self.prefs);
        }
        if !self.session.dirty {
            persist::clear_recovery(&dir);
            self.recovery_offer = None;
        }
    }

    /// Autosave a script with unsaved changes every `prefs.autosave_secs`.
    fn autosave(&mut self, now: f64) {
        let Some(dir) = self.state_dir.clone() else { return };
        let every = f64::from(self.prefs.autosave_secs);
        if every <= 0.0 || !self.session.dirty || self.recovery_offer.is_some() || now - self.last_autosave < every {
            return;
        }
        self.last_autosave = now;
        if let Err(e) = persist::write_recovery(&dir, self.session.path.as_deref(), &self.session.script) {
            self.status = Some((format!("Autosave failed: {e}"), 0.0));
        }
    }

    /// Accept control-channel requests from `rx`.
    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control = Some(rx);
        self
    }

    pub fn tokens(&self) -> theme::Tokens {
        theme::Tokens::get(self.dark)
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), 0.0));
    }

    /// Run a command: `ui.*` here, everything else in the engine. Errors go to the status bar.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value, CmdError> {
        // Replacing a script with unsaved changes asks first (interactive windows only).
        if self.dialogs
            && self.session.dirty
            && matches!(id, "file.new" | "file.sample" | "file.open" | "ui.open" | "ui.openRecent")
            && self.confirm_replace.is_none()
        {
            self.confirm_replace = Some((id.to_string(), params));
            return Ok(json!({"pending": "unsaved changes"}));
        }
        if !id.starts_with("ui.") {
            self.before_save(id, &params);
        }
        let r = if id.starts_with("ui.") { self.ui_command(id, &params) } else { self.session.run(id, &params) };
        match &r {
            Ok(_) => {
                self.after_file(id);
                if id == "spell.addWord"
                    && let Some(dir) = &self.state_dir
                    && let Err(e) = persist::save_words(dir, &self.session.user_words)
                {
                    self.status = Some((format!("Could not save the dictionary: {e}"), 0.0));
                }
                if !id.starts_with("ui.") && !matches!(id, "document.inspect" | "document.stats" | "smarttype.suggest") {
                    self.canvas.scroll_to_caret = true;
                }
                if !self.session.status.is_empty() {
                    let s = std::mem::take(&mut self.session.status);
                    self.set_status(s);
                }
            }
            Err(e) => self.set_status(e.to_string()),
        }
        r
    }

    /// Run and ignore the result (menus, buttons).
    pub fn run(&mut self, id: &str, params: Value) {
        let _ = self.execute(id, params);
    }

    fn ui_command(&mut self, id: &str, p: &Value) -> Result<Value, CmdError> {
        let path = p.get("path").and_then(Value::as_str).map(str::to_string);
        match id {
            "ui.view.page" => {
                self.view = View::Page;
                self.session.speed = false;
            }
            "ui.view.cards" => self.view = View::Cards,
            "ui.view.reports" => self.view = View::Reports,
            "ui.toggleNavigator" => self.navigator = !self.navigator,
            "ui.theme.toggle" => self.dark = !self.dark,
            "ui.toggleSpelling" => self.spelling = !self.spelling,
            "ui.toggleEnterPicker" => self.enter_picker = !self.enter_picker,
            "ui.theme.dark" => self.dark = true,
            "ui.theme.light" => self.dark = false,
            "ui.zoomIn" => self.canvas.zoom = (self.canvas.zoom * 1.1).min(3.0),
            "ui.zoomOut" => self.canvas.zoom = (self.canvas.zoom / 1.1).max(0.4),
            "ui.zoomReset" => self.canvas.zoom = 1.0,
            "ui.zoomFit" => {
                let w = self.canvas.canvas_rect.map_or(900.0, |r| r.width());
                self.canvas.zoom = ((w - 2.0 * 28.0 - 16.0) / scriptcraft_geom::PAGE_W).clamp(0.4, 3.0);
            }
            "ui.view.speed" => {
                self.view = View::Page;
                return self.session.run("view.speed", p);
            }
            "ui.find" => {
                let mut f = self.find.take().unwrap_or_default();
                if let Some(t) = p.get("text").and_then(Value::as_str) {
                    f.find = t.to_string();
                }
                f.focus = true;
                self.find = Some(f);
            }
            "ui.titlePage" => self.title_dialog = !self.title_dialog,
            "ui.elementSettings" => self.element_dialog = !self.element_dialog,
            "ui.about" => self.about = true,
            "ui.quit" => self.close_wanted = true,
            "ui.openRecent" => {
                let path = path.or_else(|| self.prefs.recent.first().cloned()).ok_or_else(|| CmdError::Disabled("no recent files".into()))?;
                return self.execute("file.open", json!({"path": path}));
            }
            "ui.clearRecent" => {
                self.prefs.recent.clear();
                if let Some(dir) = &self.state_dir {
                    let _ = persist::save_prefs(dir, &self.prefs);
                }
            }
            "ui.open" => {
                let Some(path) = path.or_else(|| self.pick(false, "Open Script", &["fountain", "fdx", "spmd", "txt"], "")) else {
                    return Ok(json!({}));
                };
                return self.execute("file.open", json!({"path": path}));
            }
            "ui.save" => {
                if self.session.path.is_some() && path.is_none() {
                    return self.execute("file.save", json!({}));
                }
                return self.ui_command("ui.saveAs", p);
            }
            "ui.saveAs" => {
                let Some(path) = path.or_else(|| self.pick(true, "Save Script", &["fountain", "fdx"], "Untitled.fountain")) else {
                    return Ok(json!({}));
                };
                return self.execute("file.saveAs", json!({"path": path}));
            }
            "ui.exportPdf" | "ui.exportFdx" | "ui.exportFountain" => {
                let (ext, cmd) = match id {
                    "ui.exportPdf" => ("pdf", "file.exportPdf"),
                    "ui.exportFdx" => ("fdx", "file.exportFdx"),
                    _ => ("fountain", "file.exportFountain"),
                };
                let stem = self.session.path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).unwrap_or("Untitled").to_string();
                let Some(path) = path.or_else(|| self.pick(true, "Export", &[ext], &format!("{stem}.{ext}"))) else { return Ok(json!({})) };
                return self.session.run(cmd, &json!({"path": path}));
            }
            other => return Err(CmdError::Unknown(other.to_string())),
        }
        Ok(json!({"view": self.view.id(), "dark": self.dark, "zoom": self.canvas.zoom, "navigator": self.navigator}))
    }

    /// A native file dialog (none in tests, headless runs or the browser).
    fn pick(&self, save: bool, title: &str, exts: &[&str], name: &str) -> Option<String> {
        if !self.dialogs {
            return None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let d = rfd::FileDialog::new().set_title(title).add_filter("Script", exts);
            let r = if save { d.set_file_name(name).save_file() } else { d.pick_file() };
            r.map(|p| p.display().to_string())
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (save, title, exts, name);
            None
        }
    }

    /// Every command (engine and UI) with its enablement, for agents.
    pub fn describe_commands(&self) -> Value {
        let mut v = self.session.registry.describe(Some(&self.session));
        if let Some(a) = v.as_array_mut() {
            for (id, label, location, key) in UI_COMMANDS {
                a.push(json!({"id": id, "label": label, "location": location, "shortcut": key, "params": "{}", "mutates": false, "enabled": true}));
            }
        }
        v
    }

    fn install_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_installed {
            return;
        }
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert("courier-prime".into(), std::sync::Arc::new(egui::FontData::from_static(scriptcraft_fonts::REGULAR)));
        fonts.font_data.insert("courier-prime-bold".into(), std::sync::Arc::new(egui::FontData::from_static(scriptcraft_fonts::BOLD)));
        fonts.families.insert(egui::FontFamily::Name("courier".into()), vec!["courier-prime".into()]);
        fonts.font_data.insert("courier-prime-italic".into(), std::sync::Arc::new(egui::FontData::from_static(scriptcraft_fonts::ITALIC)));
        fonts.font_data.insert("courier-prime-bolditalic".into(), std::sync::Arc::new(egui::FontData::from_static(scriptcraft_fonts::BOLD_ITALIC)));
        fonts.families.insert(egui::FontFamily::Name("courier-bold".into()), vec!["courier-prime-bold".into()]);
        fonts.families.insert(egui::FontFamily::Name("courier-italic".into()), vec!["courier-prime-italic".into()]);
        fonts.families.insert(egui::FontFamily::Name("courier-bolditalic".into()), vec!["courier-prime-bolditalic".into()]);
        ctx.set_fonts(fonts);
        self.fonts_installed = true;
    }

    /// Per-frame work before layout: control requests, screenshots, theme, title.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.install_fonts(ctx);
        // Cmd/Ctrl + =/- zoom the pages (our shortcuts), not the whole interface.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        ctx.set_visuals(self.tokens().visuals());
        let now = ctx.input(|i| i.time);
        self.autosave(now);
        if let Some((_, t)) = &mut self.status
            && *t == 0.0
        {
            *t = now;
        }
        if self.status.as_ref().is_some_and(|(_, t)| now - *t > 6.0) {
            self.status = None;
        }
        // Finished screenshots.
        let shots: Vec<std::sync::Arc<egui::ColorImage>> = ctx.input(|i| {
            i.events.iter().filter_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }).collect()
        });
        for img in shots {
            for (path, reply) in self.pending_shots.drain(..) {
                let _ = reply.send(control::encode_shot(&img, path.as_deref()));
            }
        }
        let reqs: Vec<ControlRequest> = self.control.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        for req in reqs {
            if req.method == "ui.screenshot" {
                let path = req.params.get("path").and_then(Value::as_str).map(str::to_string);
                self.pending_shots.push((path, req.reply.clone()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                continue;
            }
            let v = control::handle(self, ctx, &req);
            let _ = req.reply.send(v);
        }
        if self.control.is_some() || !self.synthetic.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        let name = self.session.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("Untitled").to_string();
        let title = format!("{name}{} — ScriptCraft", if self.session.dirty { " •" } else { "" });
        if title != self.sent_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.sent_title = title;
        }
    }

    /// Feed synthetic control-channel input into egui (one pointer event per frame).
    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        if self.synthetic.is_empty() {
            return;
        }
        let pointer = |e: &egui::Event| matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. });
        // A pointer move with the clicks that follow it arrives in one frame (so a double click
        // is one); keys and text arrive together up to the next pointer event.
        let n = match self.synthetic.first() {
            Some(e) if pointer(e) => self.synthetic.iter().position(|e| !pointer(e)).unwrap_or(self.synthetic.len()),
            _ => self.synthetic.iter().position(pointer).unwrap_or(self.synthetic.len()),
        };
        raw.events.extend(self.synthetic.drain(..n));
    }

    /// The close button was pressed: `true` to close now, `false` to ask first.
    pub fn close_requested(&mut self) -> bool {
        if self.session.dirty && !self.quit_requested {
            self.confirm_close = true;
            return false;
        }
        true
    }

    /// Lay out the window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.fonts_frames < 2 {
            self.fonts_frames += 1;
            ctx.request_repaint();
            return;
        }
        chrome::global_shortcuts(self, &ctx);
        chrome::menu_bar(self, ui);
        chrome::toolbar(self, ui);
        if self.find.is_some() {
            chrome::find_bar(self, ui);
        }
        let status = self.status.as_ref().map(|(s, _)| s.clone());
        chrome::status_bar(self, ui, status);
        if self.navigator {
            chrome::navigator(self, ui);
        }
        let t = self.tokens();
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.desk)).show(ui, |ui| match self.view {
            View::Page => canvas::show(self, ui),
            View::Cards => cards::show(self, ui),
            View::Reports => reports::show(self, ui),
        });
        self.dialog_buttons.clear();
        chrome::dialogs(self, &ctx);
    }
}
