//! ScriptCraft's egui front end: a Final Draft-style window with a menu bar, a toolbar with the
//! element picker, a scene navigator, the page view, index cards, reports and a status bar.
//!
//! The UI is thin: every action goes through [`ScriptApp::execute`], which runs `ui.*` commands
//! here and everything else through the engine's command registry.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod canvas;
mod cards;
mod chrome;
pub mod control;
pub mod icons;
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
    ("ui.find", "Find…", "Edit", "Mod+F"),
    ("ui.titlePage", "Title Page…", "Document", ""),
    ("ui.open", "Open…", "File", "Mod+O"),
    ("ui.save", "Save", "File", "Mod+S"),
    ("ui.saveAs", "Save As…", "File", "Mod+Shift+S"),
    ("ui.exportPdf", "Export PDF…", "File", "Mod+P"),
    ("ui.exportFdx", "Export Final Draft (FDX)…", "File", ""),
    ("ui.exportFountain", "Export Fountain…", "File", ""),
    ("ui.about", "About ScriptCraft", "Help", ""),
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
    pub about: bool,
    /// Native file dialogs are allowed (off in tests and headless screenshots).
    pub dialogs: bool,
    /// Ask before closing with unsaved changes.
    pub confirm_close: bool,
    pub quit_requested: bool,
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
            about: false,
            dialogs: true,
            confirm_close: false,
            quit_requested: false,
            status: None,
            control: None,
            synthetic: Vec::new(),
            pending_shots: Vec::new(),
            fonts_installed: false,
            fonts_frames: 0,
            sent_title: String::new(),
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
        let r = if id.starts_with("ui.") { self.ui_command(id, &params) } else { self.session.run(id, &params) };
        match &r {
            Ok(_) => {
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
            "ui.view.page" => self.view = View::Page,
            "ui.view.cards" => self.view = View::Cards,
            "ui.view.reports" => self.view = View::Reports,
            "ui.toggleNavigator" => self.navigator = !self.navigator,
            "ui.theme.toggle" => self.dark = !self.dark,
            "ui.theme.dark" => self.dark = true,
            "ui.theme.light" => self.dark = false,
            "ui.zoomIn" => self.canvas.zoom = (self.canvas.zoom * 1.1).min(3.0),
            "ui.zoomOut" => self.canvas.zoom = (self.canvas.zoom / 1.1).max(0.4),
            "ui.zoomReset" => self.canvas.zoom = 1.0,
            "ui.find" => {
                let mut f = self.find.take().unwrap_or_default();
                if let Some(t) = p.get("text").and_then(Value::as_str) {
                    f.find = t.to_string();
                }
                f.focus = true;
                self.find = Some(f);
            }
            "ui.titlePage" => self.title_dialog = !self.title_dialog,
            "ui.about" => self.about = true,
            "ui.open" => {
                let Some(path) = path.or_else(|| self.pick(false, "Open Script", &["fountain", "fdx", "spmd", "txt"], "")) else {
                    return Ok(json!({}));
                };
                return self.session.run("file.open", &json!({"path": path}));
            }
            "ui.save" => {
                if self.session.path.is_some() && path.is_none() {
                    return self.session.run("file.save", &json!({}));
                }
                return self.ui_command("ui.saveAs", p);
            }
            "ui.saveAs" => {
                let Some(path) = path.or_else(|| self.pick(true, "Save Script", &["fountain", "fdx"], "Untitled.fountain")) else {
                    return Ok(json!({}));
                };
                return self.session.run("file.saveAs", &json!({"path": path}));
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
        fonts.families.insert(egui::FontFamily::Name("courier-bold".into()), vec!["courier-prime-bold".into()]);
        ctx.set_fonts(fonts);
        self.fonts_installed = true;
    }

    /// Per-frame work before layout: control requests, screenshots, theme, title.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.install_fonts(ctx);
        ctx.set_visuals(self.tokens().visuals());
        let now = ctx.input(|i| i.time);
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
        let n = match self.synthetic.first() {
            Some(egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }) => 1,
            _ => self
                .synthetic
                .iter()
                .position(|e| matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }))
                .unwrap_or(self.synthetic.len()),
        };
        if let Some(egui::Event::PointerButton { pos, .. }) = self.synthetic.first() {
            raw.events.push(egui::Event::PointerMoved(*pos));
        }
        let n = n.clamp(1, self.synthetic.len());
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
        chrome::dialogs(self, &ctx);
    }
}
