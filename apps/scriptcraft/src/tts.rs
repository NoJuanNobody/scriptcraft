//! Speech for the desktop app, with the system's own offline voices in a child process (so the
//! window never waits): `say` on macOS, System.Speech through PowerShell on Windows, `espeak-ng`
//! (or `espeak`, or speech-dispatcher's `spd-say`) on Linux. Stop kills the child.
//!
//! Speech to text: when whisper.cpp's `whisper-cli` and a model are found, audio files can be
//! transcribed on this computer (`SCRIPTCRAFT_WHISPER` and `SCRIPTCRAFT_WHISPER_MODEL` override
//! the search; models are looked for in `~/.whisper/models` and `~/.cache/whisper`). Non-WAV
//! audio is converted with `ffmpeg` when it is installed. Nothing is bundled or downloaded.
//!
//! `SCRIPTCRAFT_TTS` picks the engine (`say`, `sapi`, `espeak-ng`, `espeak`, `spd-say`, `none`);
//! `SCRIPTCRAFT_TTS_OUT=<dir>` makes `say` and espeak write each line to a numbered audio file
//! there instead of playing it (for checks without sound).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use scriptcraft_ui_egui::speech::{SpeechHost, Voice};

/// Longest text handed to the engine at once (characters).
const MAX_TEXT: usize = 8000;

#[derive(Clone, Debug, PartialEq)]
enum Backend {
    Say,
    Sapi,
    Espeak(PathBuf),
    SpdSay(PathBuf),
}

/// The system's text to speech (and whisper.cpp speech to text, when present).
pub struct SystemSpeech {
    backend: Backend,
    child: Option<Child>,
    out_dir: Option<PathBuf>,
    n: usize,
    whisper: Option<(PathBuf, PathBuf)>,
    heard: Arc<Mutex<Vec<Result<String, String>>>>,
}

/// A program on `PATH`.
fn find_in_path(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|d| d.join(&exe)).find(|p| p.is_file()))
}

fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up for each line.
        c.creation_flags(0x0800_0000);
    }
    c
}

/// The speech engine for this computer, if any.
pub fn detect() -> Option<SystemSpeech> {
    let choice = std::env::var("SCRIPTCRAFT_TTS").unwrap_or_default();
    let backend = match choice.trim() {
        "none" => return None,
        "say" => Some(Backend::Say),
        "sapi" => Some(Backend::Sapi),
        "espeak-ng" | "espeak" => find_in_path(choice.trim()).map(Backend::Espeak),
        "spd-say" => find_in_path("spd-say").map(Backend::SpdSay),
        _ => None,
    }
    .or_else(|| {
        if cfg!(target_os = "macos") && Path::new("/usr/bin/say").is_file() {
            Some(Backend::Say)
        } else if cfg!(windows) {
            Some(Backend::Sapi)
        } else {
            find_in_path("espeak-ng").or_else(|| find_in_path("espeak")).map(Backend::Espeak).or_else(|| find_in_path("spd-say").map(Backend::SpdSay))
        }
    })?;
    let out_dir = std::env::var_os("SCRIPTCRAFT_TTS_OUT").map(PathBuf::from).filter(|d| std::fs::create_dir_all(d).is_ok());
    Some(SystemSpeech { backend, child: None, out_dir, n: 0, whisper: find_whisper(), heard: Arc::default() })
}

/// whisper.cpp's command-line program and a model.
fn find_whisper() -> Option<(PathBuf, PathBuf)> {
    let bin = std::env::var_os("SCRIPTCRAFT_WHISPER").map(PathBuf::from).filter(|p| p.is_file()).or_else(|| find_in_path("whisper-cli"))?;
    let model = std::env::var_os("SCRIPTCRAFT_WHISPER_MODEL").map(PathBuf::from).filter(|p| p.is_file()).or_else(|| {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)?;
        let mut found: Vec<PathBuf> = [home.join(".whisper/models"), home.join(".cache/whisper")]
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flat_map(|r| r.flatten().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "bin") && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("ggml-")))
            .collect();
        found.sort();
        found.iter().find(|p| p.to_string_lossy().contains("base.en")).cloned().or_else(|| found.first().cloned())
    })?;
    Some((bin, model))
}

/// `say -v '?'`: `Eddy (English (US)) en_US    # Hello! …` → name and language.
fn parse_say_voices(out: &str) -> Vec<Voice> {
    out.lines()
        .filter_map(|l| {
            let head = l.split('#').next()?.trim();
            let (name, lang) = head.rsplit_once(char::is_whitespace)?;
            let name = name.trim();
            (!name.is_empty()).then(|| Voice { name: name.to_string(), lang: lang.trim().to_string() })
        })
        .collect()
}

/// `espeak-ng --voices`: ` 5  en-us  --/M  English_(America)  gmw/en-US …` → the language code.
fn parse_espeak_voices(out: &str) -> Vec<Voice> {
    out.lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let _pty = f.next()?;
            let lang = f.next()?;
            Some(Voice { name: lang.to_string(), lang: lang.to_string() })
        })
        .collect()
}

/// `spd-say -L`: `NAME  LANGUAGE  VARIANT` rows after a header.
fn parse_spd_voices(out: &str) -> Vec<Voice> {
    out.lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            Some(Voice { name: f.next()?.to_string(), lang: f.next().unwrap_or("").to_string() })
        })
        .collect()
}

/// Lines `name|culture` from PowerShell.
fn parse_sapi_voices(out: &str) -> Vec<Voice> {
    out.lines()
        .filter_map(|l| l.trim().split_once('|'))
        .map(|(n, c)| Voice { name: n.trim().to_string(), lang: c.trim().replace('-', "_") })
        .collect()
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

const SAPI_VOICES: &str = "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.GetInstalledVoices() | ForEach-Object { $_.VoiceInfo.Name + '|' + $_.VoiceInfo.Culture }";

fn run_capture(mut c: Command) -> String {
    c.stdin(Stdio::null()).stderr(Stdio::null());
    c.output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

impl SystemSpeech {
    /// The command that says `text` (read from stdin unless noted).
    fn speak_command(&mut self, voice: Option<&str>, rate: u32, text: &str) -> (Command, bool) {
        let rate = rate.clamp(80, 400);
        let out = self.out_dir.as_ref().map(|d| {
            self.n += 1;
            d.join(format!("{:04}", self.n))
        });
        match &self.backend {
            Backend::Say => {
                let mut c = command(Path::new("/usr/bin/say"));
                if let Some(v) = voice {
                    c.arg("-v").arg(v);
                }
                c.arg("-r").arg(rate.to_string());
                if let Some(o) = out {
                    c.arg("-o").arg(o.with_extension("aiff"));
                }
                c.args(["-f", "-"]);
                (c, true)
            }
            Backend::Espeak(bin) => {
                let mut c = command(bin);
                if let Some(v) = voice {
                    c.arg("-v").arg(v);
                }
                c.arg("-s").arg(rate.to_string());
                if let Some(o) = out {
                    c.arg("-w").arg(o.with_extension("wav"));
                }
                c.arg("--stdin");
                (c, true)
            }
            Backend::SpdSay(bin) => {
                let mut c = command(bin);
                let r = ((i64::from(rate) - 180) / 2).clamp(-100, 100);
                c.args(["-w", "-r"]).arg(r.to_string());
                if let Some(v) = voice {
                    c.arg("-y").arg(v);
                }
                // A leading space keeps text that starts with `-` from being read as an option.
                c.arg(format!(" {text}"));
                (c, false)
            }
            Backend::Sapi => {
                let r = ((i64::from(rate) - 180) / 20).clamp(-10, 10);
                let select = voice.map(|v| format!("try {{ $s.SelectVoice({}) }} catch {{}}; ", ps_quote(v))).unwrap_or_default();
                let script = format!(
                    "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; {select}$s.Rate = {r}; $t = [Console]::In.ReadToEnd(); $s.Speak($t)"
                );
                let mut c = command(Path::new("powershell"));
                c.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
                (c, true)
            }
        }
    }
}

impl SpeechHost for SystemSpeech {
    fn engine(&self) -> String {
        match &self.backend {
            Backend::Say => "macOS voices (say)".into(),
            Backend::Sapi => "Windows voices (System.Speech)".into(),
            Backend::Espeak(b) => b.file_name().and_then(|n| n.to_str()).unwrap_or("espeak").to_string(),
            Backend::SpdSay(_) => "speech-dispatcher (spd-say)".into(),
        }
    }

    fn voices(&mut self) -> Vec<Voice> {
        match &self.backend {
            Backend::Say => {
                let mut c = command(Path::new("/usr/bin/say"));
                c.args(["-v", "?"]);
                parse_say_voices(&run_capture(c))
            }
            Backend::Espeak(bin) => {
                let mut c = command(bin);
                c.arg("--voices");
                parse_espeak_voices(&run_capture(c))
            }
            Backend::SpdSay(bin) => {
                let mut c = command(bin);
                c.arg("-L");
                parse_spd_voices(&run_capture(c))
            }
            Backend::Sapi => {
                let mut c = command(Path::new("powershell"));
                c.args(["-NoProfile", "-NonInteractive", "-Command", SAPI_VOICES]);
                parse_sapi_voices(&run_capture(c))
            }
        }
    }

    fn speak(&mut self, text: &str, voice: Option<&str>, rate: u32) -> Result<(), String> {
        self.stop();
        let text: String = text.chars().take(MAX_TEXT).collect();
        let (mut c, stdin) = self.speak_command(voice, rate, &text);
        c.stdout(Stdio::null()).stderr(Stdio::null()).stdin(if stdin { Stdio::piped() } else { Stdio::null() });
        let mut child = c.spawn().map_err(|e| format!("{} could not start: {e}", self.engine()))?;
        if stdin && let Some(mut input) = child.stdin.take() {
            // Small text: the pipe takes it at once. Dropping `input` closes it.
            let _ = input.write_all(text.as_bytes());
        }
        self.child = Some(child);
        Ok(())
    }

    fn speaking(&mut self) -> bool {
        let Some(c) = self.child.as_mut() else { return false };
        match c.try_wait() {
            Ok(None) => true,
            _ => {
                self.child = None;
                false
            }
        }
    }

    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            // Reap it without making the window wait.
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        }
    }

    fn can_transcribe(&self) -> bool {
        self.whisper.is_some()
    }

    fn transcribe(&mut self, path: &str) -> Result<(), String> {
        let (bin, model) = self.whisper.clone().ok_or_else(|| "whisper.cpp (whisper-cli) and a model were not found".to_string())?;
        let input = PathBuf::from(path);
        if !input.is_file() {
            return Err(format!("no audio file at {path}"));
        }
        let heard = self.heard.clone();
        std::thread::spawn(move || {
            let r = transcribe(&bin, &model, &input);
            if let Ok(mut q) = heard.lock() {
                q.push(r);
            }
        });
        Ok(())
    }

    fn heard(&mut self) -> Vec<Result<String, String>> {
        self.heard.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
    }
}

impl Drop for SystemSpeech {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Run whisper.cpp on an audio file (converted to 16 kHz mono WAV with ffmpeg when possible).
fn transcribe(bin: &Path, model: &Path, input: &Path) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("scriptcraft-dictation-{}.wav", std::process::id()));
    let wav = match find_in_path("ffmpeg") {
        Some(ff) => {
            let ok = command(&ff)
                .args(["-loglevel", "error", "-y", "-i"])
                .arg(input)
                .args(["-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
                .arg(&tmp)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if ok { tmp.clone() } else { input.to_path_buf() }
        }
        None => input.to_path_buf(),
    };
    let out = command(bin)
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .args(["-nt", "-np"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("whisper-cli could not start: {e}"));
    let _ = std::fs::remove_file(&tmp);
    let out = out?;
    if !out.status.success() {
        return Err("whisper-cli could not transcribe that file (it needs 16 kHz WAV, or ffmpeg installed to convert)".into());
    }
    let text =
        String::from_utf8_lossy(&out.stdout).lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('[')).collect::<Vec<_>>().join(" ");
    if text.is_empty() { Err("nothing was heard in that file".into()) } else { Ok(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_lists() {
        let say = "Albert              en_US    # Hello! My name is Albert.\nEddy (English (UK)) en_GB    # Hello! My name is Eddy.\n\ngarbage\n";
        let v = parse_say_voices(say);
        assert_eq!(
            v,
            vec![Voice { name: "Albert".into(), lang: "en_US".into() }, Voice { name: "Eddy (English (UK))".into(), lang: "en_GB".into() }]
        );
        let es = "Pty Language       Age/Gender VoiceName          File                 Other Languages\n 5  af              --/M      Afrikaans          gmw/af\n 2  en-us           --/M      English_(America)  gmw/en-US\n";
        assert_eq!(parse_espeak_voices(es).iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), vec!["af", "en-us"]);
        assert_eq!(parse_spd_voices("NAME LANGUAGE VARIANT\nmale1 en none\n").first().map(|v| v.name.as_str()), Some("male1"));
        assert_eq!(
            parse_sapi_voices("Microsoft David Desktop|en-US\r\njunk\n"),
            vec![Voice { name: "Microsoft David Desktop".into(), lang: "en_US".into() }]
        );
        assert_eq!(ps_quote("O'Brien"), "'O''Brien'");
    }

    #[test]
    fn commands_are_built_without_running_anything() {
        let mut s =
            SystemSpeech { backend: Backend::Say, child: None, out_dir: Some(PathBuf::from("/tmp/x")), n: 0, whisper: None, heard: Arc::default() };
        let (c, stdin) = s.speak_command(Some("Daniel"), 1000, "-hello");
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(stdin);
        let out = PathBuf::from("/tmp/x").join("0001.aiff").display().to_string();
        assert_eq!(args, vec!["-v", "Daniel", "-r", "400", "-o", out.as_str(), "-f", "-"]);
        s.backend = Backend::SpdSay(PathBuf::from("spd-say"));
        let (c, stdin) = s.speak_command(None, 80, "-x");
        assert!(!stdin);
        assert_eq!(c.get_args().last().map(|a| a.to_string_lossy().into_owned()), Some(" -x".into()));
        s.backend = Backend::Sapi;
        let (c, _) = s.speak_command(Some("It's"), 400, "hi");
        let script = c.get_args().last().map(|a| a.to_string_lossy().into_owned()).unwrap_or_default();
        assert!(script.contains("SelectVoice('It''s')") && script.contains("$s.Rate = 10"));
        // Nothing to stop, nothing speaking.
        s.stop();
        assert!(!s.speaking());
    }
}
