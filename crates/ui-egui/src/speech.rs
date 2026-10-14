//! Read aloud and dictation in the window.
//!
//! The engine plans what to read and with which voice (`speech.plan`); a [`SpeechHost`] plays it.
//! The desktop app's host uses the system's offline voices (`say` on macOS, SAPI on Windows,
//! espeak-ng or spd-say on Linux); the browser's uses speech synthesis. The window plays the plan
//! one utterance at a time, highlighting the paragraph being read and scrolling to it.
//!
//! Dictation: phrases from the host (the browser's speech recognition, a whisper.cpp
//! transcription) or typed into the Dictation window (where the operating system's own
//! dictation can type) go through `dictation.insert`.

use std::sync::{Arc, Mutex};

use egui::{Align2, Color32, Painter, Rect, RichText, vec2};
use scriptcraft_doc::{Pos, Script};
use scriptcraft_engine::CmdError;
use scriptcraft_layout::{Line, LineKind};
use serde_json::{Value, json};

use crate::ScriptApp;

/// A voice the speech engine offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voice {
    pub name: String,
    /// Language tag as the engine gives it (`en_US`, `en-GB`, `fr`).
    pub lang: String,
}

/// Speech for the window: text to speech, and optionally speech to text. Nothing here may block
/// for long: `speak` starts speaking and returns.
pub trait SpeechHost {
    /// What speaks (`macOS say`, `espeak-ng`, `browser speech synthesis`).
    fn engine(&self) -> String;
    /// The voices available.
    fn voices(&mut self) -> Vec<Voice>;
    /// Start saying `text` (stopping anything still being said). `rate` is in words per minute.
    fn speak(&mut self, text: &str, voice: Option<&str>, rate: u32) -> Result<(), String>;
    /// Still speaking.
    fn speaking(&mut self) -> bool;
    /// Stop speaking now.
    fn stop(&mut self);
    /// Live speech recognition is available (the browser).
    fn can_listen(&self) -> bool {
        false
    }
    /// Start or stop listening.
    fn listen(&mut self, _on: bool) -> Result<(), String> {
        Err("live dictation isn't available here".into())
    }
    fn listening(&self) -> bool {
        false
    }
    /// Audio files can be transcribed (whisper.cpp was found).
    fn can_transcribe(&self) -> bool {
        false
    }
    /// Transcribe an audio file in the background; the text arrives through [`SpeechHost::heard`].
    fn transcribe(&mut self, _path: &str) -> Result<(), String> {
        Err("no speech-to-text engine was found (install whisper.cpp's whisper-cli and a model)".into())
    }
    /// Phrases heard or transcribed since the last call, and errors as `Err`.
    fn heard(&mut self) -> Vec<Result<String, String>> {
        Vec::new()
    }
}

/// A host that says nothing: it records what it was asked to say and pretends each line lasts a
/// few frames. For tests, headless screenshots and demo videos.
pub struct SilentSpeaker {
    pub spoken: Arc<Mutex<Vec<(String, Option<String>)>>>,
    pub frames_per_line: u32,
    left: u32,
    voices: Vec<Voice>,
    /// Phrases to deliver as if heard (tests of dictation).
    pub to_hear: Arc<Mutex<Vec<String>>>,
}

impl SilentSpeaker {
    pub fn new(voices: &[&str]) -> Self {
        SilentSpeaker {
            spoken: Arc::default(),
            frames_per_line: 3,
            left: 0,
            voices: voices.iter().map(|v| Voice { name: (*v).to_string(), lang: "en_US".into() }).collect(),
            to_hear: Arc::default(),
        }
    }
}

impl SpeechHost for SilentSpeaker {
    fn engine(&self) -> String {
        "silent (no audio)".into()
    }
    fn voices(&mut self) -> Vec<Voice> {
        self.voices.clone()
    }
    fn speak(&mut self, text: &str, voice: Option<&str>, _rate: u32) -> Result<(), String> {
        if let Ok(mut s) = self.spoken.lock() {
            s.push((text.to_string(), voice.map(str::to_string)));
        }
        self.left = self.frames_per_line;
        Ok(())
    }
    fn speaking(&mut self) -> bool {
        self.left = self.left.saturating_sub(1);
        self.left > 0
    }
    fn stop(&mut self) {
        self.left = 0;
    }
    fn can_listen(&self) -> bool {
        true
    }
    fn listen(&mut self, _on: bool) -> Result<(), String> {
        Ok(())
    }
    fn heard(&mut self) -> Vec<Result<String, String>> {
        self.to_hear.lock().map(|mut q| q.drain(..).map(Ok).collect()).unwrap_or_default()
    }
}

/// Playing or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadState {
    #[default]
    Idle,
    Playing,
    Paused,
}

impl ReadState {
    fn id(self) -> &'static str {
        match self {
            ReadState::Idle => "idle",
            ReadState::Playing => "playing",
            ReadState::Paused => "paused",
        }
    }
}

/// One line queued for reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
    pub para: usize,
    pub text: String,
    pub voice: Option<String>,
    pub character: Option<String>,
}

/// Read Aloud state.
#[derive(Default)]
pub struct ReadAloud {
    pub window: bool,
    pub state: ReadState,
    pub queue: Vec<Queued>,
    pub pos: usize,
    started: bool,
    voices: Option<Vec<Voice>>,
}

impl ReadAloud {
    /// The paragraph being read (highlighted).
    pub fn reading(&self) -> Option<usize> {
        (self.state != ReadState::Idle).then(|| self.queue.get(self.pos).map(|q| q.para)).flatten()
    }
}

/// Dictation window state.
#[derive(Default)]
pub struct Dictation {
    pub window: bool,
    /// What the user typed or dictated into the box.
    pub text: String,
    /// Recent phrases inserted (newest last).
    pub log: Vec<String>,
    pub focus: bool,
}

/// Voices the narrator and characters get automatically: the engine's voices in English (or all
/// when none are), without novelty voices.
fn auto_voices(all: &[Voice]) -> Vec<String> {
    const NOVELTY: [&str; 20] = [
        "Albert",
        "Bad News",
        "Bahh",
        "Bells",
        "Boing",
        "Bubbles",
        "Cellos",
        "Good News",
        "Jester",
        "Organ",
        "Superstar",
        "Trinoids",
        "Whisper",
        "Wobble",
        "Zarvox",
        "Deranged",
        "Hysterical",
        "Pipe Organ",
        "Princess",
        "Junior",
    ];
    // Natural-sounding voices that systems ship, first when present.
    const PREFERRED: [&str; 12] = [
        "Samantha",
        "Daniel",
        "Karen",
        "Moira",
        "Tessa",
        "Rishi",
        "Alex",
        "Microsoft David Desktop",
        "Microsoft Zira Desktop",
        "Microsoft Mark",
        "en-us",
        "en-gb",
    ];
    let mut usable: Vec<&Voice> = all.iter().filter(|v| !NOVELTY.contains(&v.name.as_str())).collect();
    // Preferred first; voices with a variant in their name ("Eddy (English (US))") last.
    usable.sort_by_key(|v| (PREFERRED.iter().position(|p| *p == v.name).unwrap_or(PREFERRED.len()), v.name.contains(" (")));
    let en = |v: &&Voice| v.lang.to_lowercase().starts_with("en");
    let us = |v: &&Voice| matches!(v.lang.to_lowercase().replace('-', "_").as_str(), "en_us");
    let mut out: Vec<String> = usable.iter().filter(|v| us(v)).map(|v| v.name.clone()).collect();
    out.extend(usable.iter().filter(|v| en(v) && !us(v)).map(|v| v.name.clone()));
    if out.is_empty() {
        out = usable.iter().map(|v| v.name.clone()).collect();
    }
    out.dedup();
    out
}

/// The engine's voices (asked once).
pub fn voices(app: &mut ScriptApp) -> Vec<Voice> {
    if let Some(v) = &app.read_aloud.voices {
        return v.clone();
    }
    let v = app.speaker.as_mut().map(|s| s.voices()).unwrap_or_default();
    if !v.is_empty() {
        app.read_aloud.voices = Some(v.clone());
    }
    v
}

fn no_engine() -> CmdError {
    CmdError::Disabled("no speech engine is available (macOS: say; Windows: System.Speech; Linux: install espeak-ng or speech-dispatcher)".into())
}

/// `speech.play`, `pause`, `stop`, `status` and `voices`, answered by the window. `None` for any
/// other command.
pub fn command(app: &mut ScriptApp, id: &str, p: &Value) -> Option<Result<Value, CmdError>> {
    Some(match id {
        "speech.play" => play(app, p),
        "speech.pause" => {
            if app.read_aloud.state == ReadState::Playing {
                if let Some(s) = app.speaker.as_mut() {
                    s.stop();
                }
                app.read_aloud.state = ReadState::Paused;
                app.read_aloud.started = false;
            }
            Ok(status(app))
        }
        "speech.stop" => {
            stop(app);
            Ok(status(app))
        }
        "speech.status" => Ok(status(app)),
        "dictation.transcribe" => {
            let Some(path) = p.get("path").and_then(Value::as_str) else {
                return Some(Err(CmdError::Params("`path` (audio file) is required".into())));
            };
            let r = app.speaker.as_mut().map(|s| s.transcribe(path));
            match r {
                Some(Ok(())) => {
                    app.set_status("Transcribing…");
                    Ok(json!({"transcribing": path}))
                }
                Some(Err(e)) => Err(CmdError::Disabled(e)),
                None => Err(no_engine()),
            }
        }
        "speech.voices" => {
            if app.speaker.is_none() {
                return Some(Err(no_engine()));
            }
            let all = voices(app);
            let auto = auto_voices(&all);
            Ok(json!({
                "engine": app.speaker.as_ref().map(|s| s.engine()),
                "voices": all.iter().map(|v| json!({"name": v.name, "lang": v.lang, "automatic": auto.contains(&v.name)})).collect::<Vec<_>>(),
            }))
        }
        _ => return None,
    })
}

fn status(app: &ScriptApp) -> Value {
    let r = &app.read_aloud;
    let cur = r.queue.get(r.pos).filter(|_| r.state != ReadState::Idle);
    json!({
        "available": app.speaker.is_some(),
        "engine": app.speaker.as_ref().map(|s| s.engine()),
        "state": r.state.id(),
        "index": r.pos,
        "total": r.queue.len(),
        "para": cur.map(|q| q.para),
        "character": cur.and_then(|q| q.character.clone()),
        "voice": cur.and_then(|q| q.voice.clone()),
        "text": cur.map(|q| q.text.clone()),
    })
}

/// Start reading (from the caret, or `from`/`scene`), or resume when paused.
fn play(app: &mut ScriptApp, p: &Value) -> Result<Value, CmdError> {
    if app.speaker.is_none() {
        return Err(no_engine());
    }
    let fresh = ["from", "to", "scene"].iter().any(|k| p.get(*k).is_some());
    if app.read_aloud.state == ReadState::Paused && !fresh {
        app.read_aloud.state = ReadState::Playing;
        app.read_aloud.started = false;
        return Ok(status(app));
    }
    let all = voices(app);
    let mut v = p.clone();
    if !v.is_object() {
        v = json!({});
    }
    let prefs = app.prefs.speech.clone();
    if let Some(o) = v.as_object_mut() {
        if !fresh {
            o.insert("from".into(), json!(app.session.sel.ordered().0.para));
        }
        o.entry("headings").or_insert(json!(prefs.read_headings));
        o.entry("parentheticals").or_insert(json!(prefs.read_parentheticals));
        o.entry("transitions").or_insert(json!(prefs.read_transitions));
        o.insert("voices".into(), json!(auto_voices(&all)));
        if !prefs.narrator_voice.is_empty() {
            o.insert("narrator".into(), json!(prefs.narrator_voice));
        }
    }
    let plan = app.session.run("speech.plan", &v)?;
    let queue: Vec<Queued> = plan["utterances"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|u| Queued {
                    para: u["para"].as_u64().unwrap_or(0) as usize,
                    text: u["text"].as_str().unwrap_or("").to_string(),
                    voice: u["voice"].as_str().map(str::to_string),
                    character: u["character"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(s) = app.speaker.as_mut() {
        s.stop();
    }
    if queue.is_empty() {
        app.read_aloud.state = ReadState::Idle;
        return Err(CmdError::Failed("nothing to read there".into()));
    }
    let r = &mut app.read_aloud;
    r.queue = queue;
    r.pos = 0;
    r.started = false;
    r.state = ReadState::Playing;
    Ok(status(app))
}

/// Stop reading.
pub fn stop(app: &mut ScriptApp) {
    if let Some(s) = app.speaker.as_mut() {
        s.stop();
    }
    let r = &mut app.read_aloud;
    r.state = ReadState::Idle;
    r.started = false;
    r.queue.clear();
    r.pos = 0;
}

/// Every frame: advance the reading, insert dictated phrases.
pub fn tick(app: &mut ScriptApp, ctx: &egui::Context) {
    // Dictation from the host.
    let heard = app.speaker.as_mut().map(|s| s.heard()).unwrap_or_default();
    for h in heard {
        match h {
            Ok(text) => dictate(app, &text),
            Err(e) => app.set_status(e),
        }
    }
    if app.speaker.as_ref().is_some_and(|s| s.listening()) {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
    if app.read_aloud.state != ReadState::Playing {
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(80));
    let rate = app.prefs.speech.rate;
    if app.speaker.is_none() {
        stop(app);
        return;
    }
    if app.read_aloud.started {
        if app.speaker.as_mut().is_some_and(|s| s.speaking()) {
            return;
        }
        app.read_aloud.pos += 1;
        app.read_aloud.started = false;
    }
    let Some(q) = app.read_aloud.queue.get(app.read_aloud.pos).cloned() else {
        stop(app);
        app.set_status("Finished reading");
        return;
    };
    let spoke = app.speaker.as_mut().map_or_else(|| Err(String::from("no speech engine")), |s| s.speak(&q.text, q.voice.as_deref(), rate));
    match spoke {
        Ok(()) => {
            app.read_aloud.started = true;
            // Follow along: the caret moves to the line being read.
            if q.para < app.session.script.paras.len() {
                app.session.sel = scriptcraft_engine::Selection::caret(Pos::new(q.para, 0));
                app.canvas.scroll_to_caret = true;
            }
        }
        Err(e) => {
            stop(app);
            app.set_status(format!("Could not read aloud: {e}"));
        }
    }
}

/// Insert a dictated phrase.
pub fn dictate(app: &mut ScriptApp, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if app.execute("dictation.insert", json!({"text": text})).is_ok() {
        app.dictation.log.push(text.to_string());
        let n = app.dictation.log.len();
        if n > 6 {
            app.dictation.log.drain(..n - 6);
        }
    }
}

/// The paragraph being read gets a soft highlight on the page.
pub fn paint_reading(app: &ScriptApp, p: &Painter, line: &Line, pr: Rect, z: f32) {
    if line.kind != LineKind::Body || line.para.is_none() || line.para != app.read_aloud.reading() {
        return;
    }
    let w = line.text.chars().count().max(1) as f32 * scriptcraft_geom::CHAR_W;
    let r = Rect::from_min_size(pr.min + vec2((line.x - 4.0) * z, (line.y - 1.0) * z), vec2((w + 8.0) * z, (scriptcraft_geom::LINE_H + 2.0) * z));
    let c = app.tokens().accent;
    p.rect_filled(r, 3.0 * z, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), if app.dark { 60 } else { 34 }));
}

/// Characters who speak, for the voices list.
fn characters(s: &Script) -> Vec<String> {
    scriptcraft_engine::speech::characters(s)
}

/// The Read Aloud and Dictation windows.
pub fn windows(app: &mut ScriptApp, ctx: &egui::Context) {
    read_aloud_window(app, ctx);
    dictation_window(app, ctx);
}

fn read_aloud_window(app: &mut ScriptApp, ctx: &egui::Context) {
    if !app.read_aloud.window {
        return;
    }
    let mut open = true;
    let mut actions: Vec<(&str, Value)> = Vec::new();
    let all = voices(app);
    let auto = auto_voices(&all);
    let cast = app.session.run("speech.cast", &json!({"voices": auto})).unwrap_or_default();
    let t = app.tokens();
    let engine = app.speaker.as_ref().map(|s| s.engine());
    let mut prefs = app.prefs.speech.clone();
    let mut preview: Option<(String, Option<String>)> = None;
    let st = status(app);
    egui::Window::new("Read Aloud")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::RIGHT_TOP)
        .default_pos(ctx.content_rect().right_top() + vec2(-24.0, 96.0))
        .show(ctx, |ui| {
            let Some(engine) = engine else {
                ui.label(RichText::new("No speech engine is available here.").color(t.text_soft));
                ui.label(
                    RichText::new("macOS uses its built-in voices; Windows uses System.Speech; on Linux install espeak-ng or speech-dispatcher.")
                        .size(11.5)
                        .color(t.text_soft),
                );
                return;
            };
            ui.label(RichText::new(format!("Voices: {engine} ({} available)", all.len())).size(11.5).color(t.text_soft));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let state = app.read_aloud.state;
                let play_label = if state == ReadState::Paused { "Resume" } else { "Read from Caret" };
                if ui.add_enabled(state != ReadState::Playing, egui::Button::new(play_label)).clicked() {
                    actions.push(("speech.play", json!({})));
                }
                if ui.add_enabled(state == ReadState::Playing, egui::Button::new("Pause")).clicked() {
                    actions.push(("speech.pause", json!({})));
                }
                if ui.add_enabled(state != ReadState::Idle, egui::Button::new("Stop")).clicked() {
                    actions.push(("speech.stop", json!({})));
                }
            });
            if let (Some(i), Some(n)) = (st["index"].as_u64(), st["total"].as_u64())
                && n > 0
                && st["state"] != "idle"
            {
                let who = st["character"].as_str().unwrap_or("Narrator");
                ui.label(RichText::new(format!("{} of {n} · {who}", i + 1)).size(11.5).color(t.accent));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Speed");
                ui.add(egui::Slider::new(&mut prefs.rate, 80..=400).suffix(" wpm"));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut prefs.read_headings, "Scene headings");
                ui.checkbox(&mut prefs.read_parentheticals, "Parentheticals");
                ui.checkbox(&mut prefs.read_transitions, "Transitions");
            });
            ui.add_space(6.0);
            ui.label(RichText::new("CAST VOICES").size(11.0).strong().color(t.text_soft));
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                egui::Grid::new("cast-voices").num_columns(3).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                    for row in cast.as_array().map(Vec::as_slice).unwrap_or(&[]) {
                        let narrator = row["narrator"] == true;
                        let who = if narrator { "Narrator".to_string() } else { row["character"].as_str().unwrap_or("").to_string() };
                        let voice = row["voice"].as_str().map(str::to_string);
                        let chosen = row["chosen"] == true;
                        ui.label(if narrator { RichText::new(&who).italics() } else { RichText::new(&who) });
                        let shown = match (&voice, chosen) {
                            (Some(v), true) => v.clone(),
                            (Some(v), false) => format!("{v} (automatic)"),
                            (None, _) => "Automatic".into(),
                        };
                        egui::ComboBox::from_id_salt(("voice", &who)).width(190.0).selected_text(shown).show_ui(ui, |ui| {
                            let target = if narrator { json!({"narrator": true}) } else { json!({"character": who}) };
                            if ui.selectable_label(!chosen, "Automatic").clicked() {
                                let mut v = target.clone();
                                v["voice"] = Value::Null;
                                actions.push(("speech.setVoice", v));
                            }
                            for name in &auto {
                                if ui.selectable_label(chosen && voice.as_deref() == Some(name.as_str()), name).clicked() {
                                    let mut v = target.clone();
                                    v["voice"] = json!(name);
                                    actions.push(("speech.setVoice", v));
                                }
                            }
                        });
                        if ui.small_button("▶").on_hover_text("Hear this voice").clicked() {
                            let line =
                                if narrator { "Interior. Weather station, night.".to_string() } else { format!("This is {}.", title_case(&who)) };
                            preview = Some((line, voice));
                        }
                        ui.end_row();
                    }
                });
            });
            if characters(&app.session.script).is_empty() {
                ui.label(RichText::new("No characters speak in this script yet.").size(11.5).color(t.text_soft));
            }
            ui.label(RichText::new("Voices chosen here are saved with the script.").size(11.0).color(t.text_soft));
        });
    app.prefs.speech = prefs;
    if let Some((line, voice)) = preview
        && let Some(s) = app.speaker.as_mut()
    {
        let rate = app.prefs.speech.rate;
        if let Err(e) = s.speak(&line, voice.as_deref(), rate) {
            app.set_status(format!("Could not play the voice: {e}"));
        }
    }
    for (id, v) in actions {
        app.run(id, v);
    }
    app.read_aloud.window = open;
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the spoken commands are (shown in the Dictation window).
const CHEAT_SHEET: &str = "new scene interior kitchen night · character Nell · parenthetical quietly · dialogue · action · cut to · new paragraph · period, comma, question mark, dash · scratch that · literally <word>";

fn dictation_window(app: &mut ScriptApp, ctx: &egui::Context) {
    if !app.dictation.window {
        return;
    }
    let mut open = true;
    let mut insert: Option<String> = None;
    let mut listen: Option<bool> = None;
    let mut transcribe = false;
    let t = app.tokens();
    let can_listen = app.speaker.as_ref().is_some_and(|s| s.can_listen());
    let listening = app.speaker.as_ref().is_some_and(|s| s.listening());
    let can_transcribe = app.speaker.as_ref().is_some_and(|s| s.can_transcribe());
    let mod_key = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };
    egui::Window::new("Dictation")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::RIGHT_BOTTOM)
        .default_pos(ctx.content_rect().right_bottom() + vec2(-24.0, -48.0))
        .show(ctx, |ui| {
            if can_listen {
                let label = if listening { "Stop Listening" } else { "Start Listening" };
                if ui.button(label).clicked() {
                    listen = Some(!listening);
                }
                if listening {
                    ui.label(RichText::new("Listening… each pause inserts what you said.").size(11.5).color(t.accent));
                }
                ui.add_space(4.0);
            }
            let hint = if cfg!(target_os = "macos") {
                "Speak or type here (macOS: press Fn twice or Edit › Start Dictation), then Insert"
            } else if cfg!(target_os = "windows") {
                "Speak or type here (Windows: press Win+H for voice typing), then Insert"
            } else {
                "Type or paste dictated text here, then Insert"
            };
            let r = ui.add(egui::TextEdit::multiline(&mut app.dictation.text).desired_rows(3).desired_width(380.0).hint_text(hint));
            if std::mem::take(&mut app.dictation.focus) {
                r.request_focus();
            }
            let send = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!app.dictation.text.trim().is_empty(), egui::Button::new("Insert"))
                    .on_hover_text(format!("{mod_key}Return"))
                    .clicked()
                    || send
                {
                    insert = Some(std::mem::take(&mut app.dictation.text));
                }
                if can_transcribe
                    && ui.button("Transcribe Audio File…").on_hover_text("Speech to script with whisper.cpp (runs on this computer)").clicked()
                {
                    transcribe = true;
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new(CHEAT_SHEET).size(11.0).color(t.text_soft));
            if !app.dictation.log.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new("INSERTED").size(10.5).strong().color(t.text_soft));
                for l in app.dictation.log.iter().rev().take(4) {
                    ui.label(RichText::new(crate::chrome::elide(l, 70)).size(11.5));
                }
            }
        });
    if let Some(text) = insert {
        dictate(app, &text);
        app.dictation.focus = true;
    }
    if let Some(on) = listen
        && let Some(s) = app.speaker.as_mut()
        && let Err(e) = s.listen(on)
    {
        app.set_status(e);
    }
    if transcribe {
        transcribe_file(app);
    }
    app.dictation.window = open;
}

/// Pick an audio file and transcribe it (native, whisper.cpp).
fn transcribe_file(app: &mut ScriptApp) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !app.dialogs {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Transcribe Audio")
            .add_filter("Audio", &["wav", "mp3", "m4a", "aiff", "aif", "flac", "ogg"])
            .pick_file()
        else {
            return;
        };
        let path = path.display().to_string();
        let r = app.speaker.as_mut().map(|s| s.transcribe(&path));
        match r {
            Some(Ok(())) => app.set_status("Transcribing…"),
            Some(Err(e)) => app.set_status(e),
            None => {}
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = app;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_voices_prefer_english_and_skip_novelty() {
        let v = |n: &str, l: &str| Voice { name: n.into(), lang: l.into() };
        let all = vec![v("Amélie", "fr_CA"), v("Bubbles", "en_US"), v("Daniel", "en_GB"), v("Samantha", "en_US"), v("Fred", "en-US")];
        assert_eq!(auto_voices(&all), vec!["Samantha", "Fred", "Daniel"]);
        let all = vec![v("Eddy (English (US))", "en_US"), v("Aman", "en_IN"), v("Daniel", "en_GB"), v("Fred", "en_US")];
        assert_eq!(auto_voices(&all), vec!["Fred", "Eddy (English (US))", "Daniel", "Aman"]);
        assert_eq!(auto_voices(&[v("Amélie", "fr_CA")]), vec!["Amélie"]);
        assert_eq!(title_case("COAST GUARD"), "Coast Guard");
    }
}
