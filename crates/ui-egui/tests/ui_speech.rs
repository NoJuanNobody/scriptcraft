//! Read aloud and dictation in the window, with a silent speech host (no audio in tests).

use scriptcraft_engine::Session;
use scriptcraft_ui_egui::ScriptApp;
use scriptcraft_ui_egui::speech::{ReadState, SilentSpeaker};
use serde_json::json;

fn harness(app: ScriptApp) -> egui_kittest::Harness<'static, ScriptApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(
        |ui, app: &mut ScriptApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    )
}

#[test]
fn reading_aloud_plays_every_line_with_cast_voices() {
    let speaker = SilentSpeaker::new(&["Samantha", "Daniel", "Karen", "Moira", "Tessa", "Fred"]);
    let spoken = speaker.spoken.clone();
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    app.speaker = Some(Box::new(speaker));
    let mut h = harness(app);
    h.step();
    // NELL gets a chosen voice; the others are automatic.
    h.state_mut().execute("speech.setVoice", json!({"character": "NELL", "voice": "Moira"})).unwrap();
    h.state_mut().execute("ui.readAloud", json!({})).unwrap();
    let st = h.state_mut().execute("speech.play", json!({"scene": 2})).unwrap();
    assert_eq!(st["state"], "playing");
    let total = st["total"].as_u64().unwrap() as usize;
    assert!(total >= 4);
    let mut highlighted = Vec::new();
    for _ in 0..(total * 5 + 20) {
        h.step();
        if let Some(p) = h.state().read_aloud.reading() {
            highlighted.push(p);
        }
        if h.state().read_aloud.state == ReadState::Idle {
            break;
        }
    }
    assert_eq!(h.state().read_aloud.state, ReadState::Idle, "finished");
    let lines = spoken.lock().unwrap().clone();
    assert_eq!(lines.len(), total);
    assert!(lines[0].0.starts_with("Interior."), "{lines:?}");
    // Dialogue in a character's voice, action in the narrator's.
    let plan =
        h.state_mut().execute("speech.plan", json!({"scene": 2, "voices": ["Samantha", "Daniel", "Karen", "Moira", "Tessa", "Fred"]})).unwrap();
    for (u, (text, voice)) in plan["utterances"].as_array().unwrap().iter().zip(&lines) {
        assert_eq!(u["text"].as_str(), Some(text.as_str()));
        assert_eq!(u["voice"].as_str(), voice.as_deref());
    }
    // The caret followed the reading, and the paragraphs were highlighted in order.
    assert!(highlighted.windows(2).all(|w| w[0] <= w[1]));
    assert!(!highlighted.is_empty());
    // Pause, resume, stop.
    h.state_mut().execute("speech.play", json!({})).unwrap();
    h.step();
    assert_eq!(h.state_mut().execute("speech.pause", json!({})).unwrap()["state"], "paused");
    assert_eq!(h.state_mut().execute("speech.play", json!({})).unwrap()["state"], "playing");
    assert_eq!(h.state_mut().execute("speech.stop", json!({})).unwrap()["state"], "idle");
    let v = h.state_mut().execute("speech.voices", json!({})).unwrap();
    assert_eq!(v["voices"].as_array().map(Vec::len), Some(6));
    // Opening another script stops reading.
    h.state_mut().execute("speech.play", json!({})).unwrap();
    h.state_mut().execute("file.new", json!({})).ok();
    assert_ne!(h.state().read_aloud.state, ReadState::Playing);
    h.step();
}

#[test]
fn no_speech_engine_says_so() {
    let mut app = ScriptApp::new(Session::sample());
    app.dialogs = false;
    let mut h = harness(app);
    h.step();
    assert!(h.state_mut().execute("speech.play", json!({})).is_err());
    assert_eq!(h.state_mut().execute("speech.status", json!({})).unwrap()["available"], false);
    h.state_mut().execute("ui.readAloud", json!({})).unwrap();
    h.state_mut().execute("ui.dictation", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
}

#[test]
fn dictation_from_the_host_and_the_window() {
    let speaker = SilentSpeaker::new(&["Samantha"]);
    let to_hear = speaker.to_hear.clone();
    let mut app = ScriptApp::new(Session::new(Default::default()));
    app.dialogs = false;
    app.speaker = Some(Box::new(speaker));
    let mut h = harness(app);
    h.state_mut().execute("ui.dictation", json!({})).unwrap();
    h.step();
    to_hear.lock().unwrap().extend([
        "new scene interior lab night".to_string(),
        "beakers bubble period".into(),
        "character Ada.".into(),
        "eureka exclamation point".into(),
    ]);
    h.step();
    h.step();
    let got: Vec<(String, String)> = h.state().session.script.paras.iter().map(|p| (p.element.id().to_string(), p.text.clone())).collect();
    let got: Vec<(&str, &str)> = got.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert_eq!(got, vec![("sceneHeading", "INT. LAB - NIGHT"), ("action", "Beakers bubble."), ("character", "ADA"), ("dialogue", "Eureka!")]);
    assert_eq!(h.state().dictation.log.len(), 4);
    // The window's box: typed (or OS-dictated) text goes in the same way.
    h.state_mut().execute("dictation.insert", json!({"text": "scratch that"})).unwrap();
    // The last phrase (its new dialogue paragraph too) is gone in one step.
    assert_eq!(h.state().session.script.paras.len(), 3);
    assert_eq!(h.state().session.script.paras.last().map(|p| p.text.as_str()), Some("ADA"));
    h.step();
}
