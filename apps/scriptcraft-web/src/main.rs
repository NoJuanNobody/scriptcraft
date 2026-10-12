//! ScriptCraft in the browser. Build with `cargo xtask web` (needs the `wasm32-unknown-unknown`
//! target and `wasm-bindgen-cli`); it writes `apps/scriptcraft-web/dist/`, which any static
//! server can host. File › Open uses the browser's file picker; Save and the exports download
//! the file. Fonts, the word list and the sample script are compiled in, so it works offline.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("scriptcraft-web runs in the browser: build it with `cargo xtask web` (target wasm32-unknown-unknown).");
}

#[cfg(target_arch = "wasm32")]
mod web {
    use scriptcraft_engine::Session;
    use scriptcraft_ui_egui::{FileHost, Inbox, ScriptApp};
    use wasm_bindgen::JsCast;

    /// Browser files: the file picker opens, a download saves.
    struct BrowserFiles {
        ctx: egui::Context,
    }

    impl FileHost for BrowserFiles {
        fn open(&self, inbox: Inbox) {
            let ctx = self.ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let picked = rfd::AsyncFileDialog::new().add_filter("Script", &["fountain", "fdx", "spmd", "txt"]).pick_file().await;
                if let Some(f) = picked {
                    let bytes = f.read().await;
                    if let Ok(mut q) = inbox.lock() {
                        q.push((f.file_name(), bytes));
                    }
                    ctx.request_repaint();
                }
            });
        }

        fn save(&self, name: &str, bytes: Vec<u8>) {
            if let Err(e) = download(name, &bytes) {
                log::error!("download failed: {e:?}");
            }
        }
    }

    /// Offer `bytes` as a download named `name`.
    fn download(name: &str, bytes: &[u8]) -> Result<(), wasm_bindgen::JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        web_sys::Url::revoke_object_url(&url)?;
        Ok(())
    }

    struct WebApp(ScriptApp);

    impl eframe::App for WebApp {
        fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            self.0.logic(ctx);
        }
        fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
            self.0.raw_input_hook(raw);
        }
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.0.ui(ui);
        }
    }

    pub fn start() {
        // Errors and panics go to the browser console.
        let _ = eframe::WebLogger::init(log::LevelFilter::Info);
        log::info!("ScriptCraft {} starting", env!("CARGO_PKG_VERSION"));
        wasm_bindgen_futures::spawn_local(async {
            let Some(canvas) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("scriptcraft"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            else {
                log::error!("no <canvas id=\"scriptcraft\"> on the page");
                return;
            };
            let r = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| {
                        let mut app = ScriptApp::new(Session::sample());
                        app.dark = cc.egui_ctx.system_theme() == Some(egui::Theme::Dark);
                        app.host = Some(Box::new(BrowserFiles { ctx: cc.egui_ctx.clone() }));
                        Ok(Box::new(WebApp(app)))
                    }),
                )
                .await;
            if let Err(e) = r {
                log::error!("ScriptCraft failed to start: {e:?}");
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}
