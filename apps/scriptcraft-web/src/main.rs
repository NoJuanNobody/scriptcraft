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
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use scriptcraft_ui_egui::speech::{SpeechHost, Voice};
    use scriptcraft_ui_egui::{FileHost, Inbox, ScriptApp};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;
    use wasm_bindgen::closure::Closure;

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

    /// Speech in the browser: speech synthesis reads aloud; speech recognition (where the browser
    /// has it, e.g. Chrome, Edge and Safari) dictates. The browser decides which voices exist and
    /// whether recognition runs on the device or a service of the browser's vendor.
    struct BrowserSpeech {
        ctx: egui::Context,
        heard: Rc<RefCell<Vec<Result<String, String>>>>,
        listening: Rc<Cell<bool>>,
        recognition: Option<JsValue>,
        _callbacks: Vec<Closure<dyn FnMut(JsValue)>>,
    }

    fn synth() -> Option<web_sys::SpeechSynthesis> {
        web_sys::window().and_then(|w| w.speech_synthesis().ok())
    }

    /// The browser's speech recognition constructor, if it has one.
    fn recognition_ctor() -> Option<js_sys::Function> {
        let w = web_sys::window()?;
        ["SpeechRecognition", "webkitSpeechRecognition"]
            .iter()
            .find_map(|k| js_sys::Reflect::get(&w, &JsValue::from_str(k)).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()))
    }

    fn get(v: &JsValue, k: &str) -> JsValue {
        js_sys::Reflect::get(v, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
    }

    impl BrowserSpeech {
        fn new(ctx: egui::Context) -> Self {
            BrowserSpeech { ctx, heard: Rc::default(), listening: Rc::default(), recognition: None, _callbacks: Vec::new() }
        }

        fn recognizer(&mut self) -> Result<JsValue, String> {
            if let Some(r) = &self.recognition {
                return Ok(r.clone());
            }
            let ctor = recognition_ctor().ok_or("this browser has no speech recognition")?;
            let r = js_sys::Reflect::construct(&ctor, &js_sys::Array::new()).map_err(|_| "speech recognition could not start")?;
            let set = |k: &str, v: &JsValue| {
                let _ = js_sys::Reflect::set(&r, &JsValue::from_str(k), v);
            };
            set("continuous", &JsValue::TRUE);
            set("interimResults", &JsValue::FALSE);
            if let Some(lang) = web_sys::window().and_then(|w| w.navigator().language()) {
                set("lang", &JsValue::from_str(&lang));
            }
            let (heard, ctx) = (self.heard.clone(), self.ctx.clone());
            let on_result = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                let results = get(&ev, "results");
                let from = get(&ev, "resultIndex").as_f64().unwrap_or(0.0) as u32;
                let n = get(&results, "length").as_f64().unwrap_or(0.0) as u32;
                for i in from..n {
                    let res = js_sys::Reflect::get(&results, &JsValue::from(i)).unwrap_or(JsValue::UNDEFINED);
                    if get(&res, "isFinal").as_bool() == Some(true)
                        && let Some(t) = get(&get(&res, "0"), "transcript").as_string()
                    {
                        heard.borrow_mut().push(Ok(t));
                    }
                }
                ctx.request_repaint();
            });
            let (heard, ctx) = (self.heard.clone(), self.ctx.clone());
            let on_error = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                let e = get(&ev, "error").as_string().unwrap_or_else(|| "unknown".into());
                if e != "no-speech" && e != "aborted" {
                    heard.borrow_mut().push(Err(format!("Dictation stopped: {e}")));
                }
                ctx.request_repaint();
            });
            // Browsers stop listening after a silence; keep going until the user stops.
            let (listening, again) = (self.listening.clone(), r.clone());
            let on_end = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
                if listening.get()
                    && let Ok(start) = get(&again, "start").dyn_into::<js_sys::Function>()
                {
                    let _ = start.call0(&again);
                }
            });
            set("onresult", on_result.as_ref());
            set("onerror", on_error.as_ref());
            set("onend", on_end.as_ref());
            self._callbacks = vec![on_result, on_error, on_end];
            self.recognition = Some(r.clone());
            Ok(r)
        }
    }

    impl SpeechHost for BrowserSpeech {
        fn engine(&self) -> String {
            "browser speech synthesis".into()
        }
        fn voices(&mut self) -> Vec<Voice> {
            synth()
                .map(|s| {
                    s.get_voices()
                        .iter()
                        .filter_map(|v| v.dyn_into::<web_sys::SpeechSynthesisVoice>().ok())
                        .map(|v| Voice { name: v.name(), lang: v.lang() })
                        .collect()
                })
                .unwrap_or_default()
        }
        fn speak(&mut self, text: &str, voice: Option<&str>, rate: u32) -> Result<(), String> {
            let s = synth().ok_or("this browser has no speech synthesis")?;
            s.cancel();
            let u = web_sys::SpeechSynthesisUtterance::new_with_text(text).map_err(|_| "could not prepare speech")?;
            if let Some(name) = voice
                && let Some(v) = s.get_voices().iter().filter_map(|v| v.dyn_into::<web_sys::SpeechSynthesisVoice>().ok()).find(|v| v.name() == name)
            {
                u.set_voice(Some(&v));
            }
            u.set_rate((rate as f32 / 180.0).clamp(0.3, 3.0));
            s.speak(&u);
            Ok(())
        }
        fn speaking(&mut self) -> bool {
            synth().is_some_and(|s| s.speaking() || s.pending())
        }
        fn stop(&mut self) {
            if let Some(s) = synth() {
                s.cancel();
            }
        }
        fn can_listen(&self) -> bool {
            recognition_ctor().is_some()
        }
        fn listen(&mut self, on: bool) -> Result<(), String> {
            let r = self.recognizer()?;
            self.listening.set(on);
            let f = get(&r, if on { "start" } else { "stop" }).dyn_into::<js_sys::Function>().map_err(|_| "speech recognition is not usable")?;
            f.call0(&r).map(|_| ()).map_err(|_| "the browser refused the microphone".to_string())
        }
        fn listening(&self) -> bool {
            self.listening.get()
        }
        fn heard(&mut self) -> Vec<Result<String, String>> {
            self.heard.borrow_mut().drain(..).collect()
        }
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
                        app.speaker = Some(Box::new(BrowserSpeech::new(cc.egui_ctx.clone())));
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
