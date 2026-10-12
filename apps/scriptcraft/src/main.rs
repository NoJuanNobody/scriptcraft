//! The ScriptCraft desktop app.
//!
//! Usage: `scriptcraft [--new | --sample] [--control <port>] [file.fountain|file.fdx]`
//!
//! With no file it opens the sample script (`--new` starts blank). `--control <port>` (or
//! `SCRIPTCRAFT_CONTROL_PORT`) starts a localhost JSON-lines control server; every request carries
//! the key the app writes to `<settings>/control-key.local-<port>` (see docs/control-protocol.md).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod control_server;

use scriptcraft_engine::Session;
use scriptcraft_ui_egui::ScriptApp;
use serde_json::json;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Set by the signal handler: the process is being terminated.
static TERMINATING: AtomicBool = AtomicBool::new(false);

struct App(ScriptApp, Option<control_server::KeyFile>);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if TERMINATING.load(Ordering::SeqCst) {
            // SIGTERM/SIGINT/SIGHUP: keep unsaved work as a recovery copy, drop the key, exit.
            self.0.emergency_save();
            if let Some(k) = &self.1 {
                k.remove();
            }
            std::process::exit(0);
        }
        self.0.logic(ctx);
        if std::mem::take(&mut self.0.close_wanted) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let closing = ctx.input(|i| i.viewport().close_requested());
        if closing && !self.0.close_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if self.0.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(raw);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
    fn on_exit(&mut self) {
        self.0.on_exit();
        if let Some(k) = &self.1 {
            k.remove();
        }
    }
}

/// The window icon, drawn in code: a page with a clapperboard stripe.
fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) * 4;
            let inside = (10..54).contains(&x) && (6..58).contains(&y);
            let stripe = inside && (6..18).contains(&y);
            let line = inside && y > 24 && y % 7 == 0 && (16..48).contains(&x);
            let c: [u8; 4] = if stripe {
                if ((x + 64 - y) / 6) % 2 == 0 { [0x1e, 0x20, 0x26, 255] } else { [0xf2, 0xf2, 0xf2, 255] }
            } else if line {
                [0x3b, 0x5b, 0xdb, 255]
            } else if inside {
                [0xfb, 0xfb, 0xfb, 255]
            } else {
                [0, 0, 0, 0]
            };
            if let Some(px) = rgba.get_mut(i..i + 4) {
                px.copy_from_slice(&c);
            }
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

/// On SIGTERM/SIGINT/SIGHUP, wake the UI so it saves and exits; if it does not within 3 s
/// (frozen), remove the key file and exit anyway.
fn install_signal_handler(ctx: Arc<Mutex<Option<egui::Context>>>, key: Arc<Mutex<Option<control_server::KeyFile>>>) {
    let r = ctrlc::set_handler(move || {
        TERMINATING.store(true, Ordering::SeqCst);
        if let Some(c) = ctx.lock().ok().and_then(|g| g.clone()) {
            c.request_repaint();
        }
        let key = key.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if let Some(k) = key.lock().ok().and_then(|g| g.clone()) {
                k.remove();
            }
            std::process::exit(1);
        });
    });
    if let Err(e) = r {
        eprintln!("scriptcraft: no signal handler ({e})");
    }
}

fn main() -> eframe::Result<()> {
    let ctx_slot: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
    let key_slot: Arc<Mutex<Option<control_server::KeyFile>>> = Arc::new(Mutex::new(None));
    install_signal_handler(ctx_slot.clone(), key_slot.clone());
    let mut control_port: Option<u16> = std::env::var("SCRIPTCRAFT_CONTROL_PORT").ok().and_then(|p| p.parse().ok());
    let mut file: Option<String> = None;
    let mut blank = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--control" => control_port = args.next().and_then(|p| p.parse().ok()),
            "--new" => blank = true,
            "--sample" => blank = false,
            "--help" | "-h" => {
                println!("scriptcraft [--new | --sample] [--control <port>] [file.fountain|file.fdx]");
                return Ok(());
            }
            other => file = Some(other.to_string()),
        }
    }
    let mut session = if blank { Session::new(Default::default()) } else { Session::sample() };
    if let Some(f) = &file
        && let Err(e) = session.run("file.open", &json!({"path": f}))
    {
        eprintln!("scriptcraft: {e}");
    }
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("ScriptCraft")
            .with_icon(icon()),
        ..Default::default()
    };
    // macOS: winit's default app menu has a Quit item (Cmd+Q) that calls `terminate:` directly,
    // which would skip the unsaved-changes prompt. Without it, Cmd+Q reaches the app as a
    // shortcut (ui.quit) and goes through the same prompt as the close button.
    #[cfg(target_os = "macos")]
    {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_default_menu(false);
        }));
    }
    eframe::run_native(
        "ScriptCraft",
        options,
        Box::new(move |cc| {
            if let Ok(mut g) = ctx_slot.lock() {
                *g = Some(cc.egui_ctx.clone());
            }
            let mut app = ScriptApp::new(session);
            app.dark = cc.egui_ctx.system_theme() == Some(egui::Theme::Dark);
            if let Some(dir) = scriptcraft_control_key::settings_dir() {
                app = app.with_state_dir(dir);
            }
            let mut key_file = None;
            if let Some(port) = control_port
                && let Some((rx, kf)) = control_server::start(port, cc.egui_ctx.clone())
            {
                app = app.with_control(rx);
                if let Ok(mut g) = key_slot.lock() {
                    g.clone_from(&kf);
                }
                key_file = kf;
            }
            Ok(Box::new(App(app, key_file)))
        }),
    )
}
