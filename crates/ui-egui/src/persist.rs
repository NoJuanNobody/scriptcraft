//! What the app keeps between runs.
//!
//! **Settings** go through [`craft_settings`] (storytold/craft-libs#22), like every Craft app:
//! one commented `config.toml` in the config folder, saved half a second after a change, with a
//! `.bak`, recovery of damaged files and schema migration. [`Prefs`] is ScriptCraft's settings
//! type; the old `prefs.json` is imported on the first run and left in place.
//!
//! **Data** is not settings and lives in the data folder (`scriptcraft_control_key::Folders`), so
//! a settings reset never loses work:
//! - `recovery.json`: the autosaved copy of a script with unsaved changes (removed once saved)
//! - `backups/`: the previous version of each file, kept before it is overwritten
//! - `dictionary.txt`: the user dictionary, one word per line
//!
//! Nothing here touches the disk unless the app opened its folders; tests use temp folders and
//! `SCRIPTCRAFT_CONFIG_DIR=` (empty) isolates a run completely.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use scriptcraft_doc::Script;
use serde::{Deserialize, Serialize};

/// The pre-0.3 settings file, imported once.
pub const LEGACY_PREFS: &str = "prefs.json";
pub const RECOVERY: &str = "recovery.json";
pub const BACKUPS: &str = "backups";
/// Most recent files remembered.
pub const MAX_RECENT: usize = 10;

/// The light/dark choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the system.
    #[default]
    System,
    Light,
    Dark,
}

/// ScriptCraft's settings: the sections of `config.toml`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub appearance: Appearance,
    pub editing: Editing,
    pub files: Files,
    pub speech: Speech,
}

/// `[appearance]`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: Theme,
    /// Page zoom (0.4 to 3.0).
    pub zoom: f64,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: Theme::System, zoom: 1.0 }
    }
}

/// `[editing]`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editing {
    /// Underline misspelled words as you type.
    pub spelling: bool,
    /// Enter in an empty paragraph shows the element list.
    pub element_list_on_enter: bool,
    /// The author name put on new script notes.
    pub note_author: String,
}

impl Default for Editing {
    fn default() -> Self {
        Editing { spelling: true, element_list_on_enter: true, note_author: String::new() }
    }
}

/// `[files]`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Files {
    /// Most recent first.
    pub recent: Vec<String>,
    /// Seconds between autosaves of a script with unsaved changes (0 = off).
    pub autosave_seconds: u32,
    /// Backups kept per file (0 = none).
    pub backups_kept: u32,
}

impl Default for Files {
    fn default() -> Self {
        Files { recent: Vec::new(), autosave_seconds: 60, backups_kept: 5 }
    }
}

/// `[speech]`: reading aloud.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speech {
    /// Words per minute (80 to 400).
    pub rate: u32,
    /// The narrator's voice for scripts that don't choose one ("" = automatic).
    pub narrator_voice: String,
    /// Read scene headings, parentheticals and transitions.
    pub read_headings: bool,
    pub read_parentheticals: bool,
    pub read_transitions: bool,
}

impl Default for Speech {
    fn default() -> Self {
        Speech { rate: 180, narrator_voice: String::new(), read_headings: true, read_parentheticals: false, read_transitions: false }
    }
}

impl craft_settings::Settings for Prefs {
    const APP_ID: &'static str = scriptcraft_control_key::APP_ID;
    const SCHEMA_VERSION: u32 = 1;
}

impl Prefs {
    /// Put `path` first in the recent list.
    pub fn touch_recent(&mut self, path: &str) {
        self.files.recent.retain(|p| p != path);
        self.files.recent.insert(0, path.to_string());
        self.files.recent.truncate(MAX_RECENT);
    }

    /// Clamp hand-edited or hostile values into range.
    pub fn sanitized(mut self) -> Prefs {
        let z = self.appearance.zoom;
        // Two decimals: `zoom = 1.1` in the file, not float noise.
        self.appearance.zoom = if z.is_finite() { (z.clamp(0.4, 3.0) * 100.0).round() / 100.0 } else { 1.0 };
        self.files.recent.truncate(MAX_RECENT);
        self.files.autosave_seconds = self.files.autosave_seconds.min(3600);
        self.files.backups_kept = self.files.backups_kept.min(100);
        self.speech.rate = self.speech.rate.clamp(80, 400);
        self.speech.narrator_voice = scriptcraft_doc::voices::clean_voice_name(&self.speech.narrator_voice).unwrap_or_default();
        self
    }
}

/// ScriptCraft 0.2's `prefs.json`.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct LegacyPrefs {
    recent: Vec<String>,
    dark: Option<bool>,
    zoom: Option<f64>,
    spelling: Option<bool>,
    enter_picker: Option<bool>,
    autosave_secs: Option<u32>,
    backups: Option<u32>,
}

/// Parse ScriptCraft 0.2's `prefs.json` into [`Prefs`] (for [`craft_settings::LegacyFile`]).
pub fn parse_legacy_prefs(bytes: &[u8]) -> Result<Prefs, String> {
    let old: LegacyPrefs = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let d = Prefs::default();
    let theme = match old.dark {
        Some(true) => Theme::Dark,
        Some(false) => Theme::Light,
        None => Theme::System,
    };
    Ok(Prefs {
        appearance: Appearance { theme, zoom: old.zoom.unwrap_or(d.appearance.zoom) },
        editing: Editing {
            spelling: old.spelling.unwrap_or(d.editing.spelling),
            element_list_on_enter: old.enter_picker.unwrap_or(d.editing.element_list_on_enter),
            note_author: String::new(),
        },
        files: Files {
            recent: old.recent,
            autosave_seconds: old.autosave_secs.unwrap_or(d.files.autosave_seconds),
            backups_kept: old.backups.unwrap_or(d.files.backups_kept),
        },
        speech: d.speech,
    }
    .sanitized())
}

/// The settings store for these folders: `config.toml` in the config folder (in memory when
/// isolated), importing the old `prefs.json` from the legacy folder on the first run.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_settings(f: &scriptcraft_control_key::Folders) -> (craft_settings::SettingsStore<Prefs>, craft_settings::LoadReport) {
    let location = f.config.clone().map_or(craft_settings::Location::Memory, craft_settings::Location::Dir);
    let mut opts = craft_settings::OpenOptions::new(location);
    if f.config.is_some()
        && let Some(old) = &f.legacy
    {
        opts = opts.legacy(vec![craft_settings::LegacyFile { path: old.join(LEGACY_PREFS), parse: parse_legacy_prefs }]);
    }
    craft_settings::SettingsStore::open_with(opts)
}

/// A one-line description of what went wrong while loading settings, if anything did.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_problems(r: &craft_settings::LoadReport) -> Option<String> {
    if !r.has_problems() {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(p) = &r.broken_moved_to {
        parts.push(format!("a damaged settings file was moved to {}", p.display()));
    }
    for e in &r.section_errors {
        parts.push(format!("[{}] was reset ({})", e.section, e.message));
    }
    if let Some(ro) = &r.read_only {
        parts.push(format!("settings are read-only: {ro}"));
    }
    if parts.is_empty() { None } else { Some(format!("Settings: {}", parts.join("; "))) }
}

/// An autosaved script.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recovery {
    /// Where the script lives, if it was saved before.
    pub path: Option<String>,
    /// Seconds since the Unix epoch.
    pub saved_at: u64,
    pub script: Script,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn write_recovery(dir: &Path, path: Option<&Path>, script: &Script) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let r = Recovery { path: path.map(|p| p.display().to_string()), saved_at: now_secs(), script: script.clone() };
    let tmp = dir.join(format!("{RECOVERY}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(&r).map_err(std::io::Error::other)?)?;
    std::fs::rename(tmp, dir.join(RECOVERY))
}

/// The autosaved script, if any (normalised; a corrupt file counts as none).
pub fn read_recovery(dir: &Path) -> Option<Recovery> {
    let t = std::fs::read(dir.join(RECOVERY)).ok()?;
    let mut r: Recovery = serde_json::from_slice(&t).ok()?;
    r.script.normalize().ok()?;
    Some(r)
}

pub fn clear_recovery(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(RECOVERY));
}

/// Copy `file` (when it exists) into `dir/backups` as `<stem>.<unix-time>.<ext>`, keeping the
/// newest `keep` copies of that file. Returns the backup's path.
pub fn backup(dir: &Path, file: &Path, keep: u32) -> std::io::Result<Option<PathBuf>> {
    if keep == 0 || !file.is_file() {
        return Ok(None);
    }
    let bdir = dir.join(BACKUPS);
    std::fs::create_dir_all(&bdir)?;
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("script").replace('.', "_");
    let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("bak");
    let list = |bdir: &Path| -> Vec<(u64, PathBuf)> {
        let mut v: Vec<(u64, PathBuf)> = std::fs::read_dir(bdir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let name = p.file_name()?.to_str()?.to_string();
                let rest = name.strip_prefix(&format!("{stem}."))?;
                let t: u64 = rest.split('.').next()?.parse().ok()?;
                Some((t, p))
            })
            .collect();
        v.sort();
        v
    };
    // Always newer than every existing backup, even within the same second.
    let n = now_secs().max(list(&bdir).last().map_or(0, |(t, _)| t + 1));
    let dest = bdir.join(format!("{stem}.{n}.{ext}"));
    std::fs::copy(file, &dest)?;
    // Prune the oldest.
    let mut mine = list(&bdir);
    while mine.len() > keep as usize {
        let (_, p) = mine.remove(0);
        let _ = std::fs::remove_file(p);
    }
    Ok(Some(dest))
}

pub const DICTIONARY: &str = "dictionary.txt";

/// The user dictionary in `dir` (empty when missing or unreadable).
pub fn load_words(dir: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(dir.join(DICTIONARY))
        .map(|t| t.lines().map(|l| scriptcraft_spell::key(l.trim())).filter(|w| !w.is_empty() && w.len() <= 64).take(100_000).collect())
        .unwrap_or_default()
}

/// Save the user dictionary into `dir`.
pub fn save_words(dir: &Path, words: &BTreeSet<String>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut t = words.iter().cloned().collect::<Vec<_>>().join("\n");
    t.push('\n');
    std::fs::write(dir.join(DICTIONARY), t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("scriptcraft-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn prefs_recovery_backups() {
        let dir = tmp("state");
        let folders = scriptcraft_control_key::Folders { config: Some(dir.clone()), data: Some(dir.clone()), rule: "test".into(), legacy: None };
        let (mut store, report) = open_settings(&folders);
        assert!(!report.has_problems());
        assert_eq!(store.get(), &Prefs::default());
        store.update(|p| {
            for i in 0..14 {
                p.touch_recent(&format!("/s/{i}.fdx"));
            }
            p.touch_recent("/s/3.fdx");
            p.appearance.theme = Theme::Dark;
            p.appearance.zoom = 1.5;
        });
        store.flush().unwrap();
        let toml = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(toml.starts_with("schema_version = 1"), "{toml}");
        assert!(toml.contains("[appearance]") && toml.contains("theme = \"dark\""), "{toml}");
        let (again, _) = open_settings(&folders);
        let q = again.get().clone();
        assert_eq!(q.files.recent.len(), MAX_RECENT);
        assert_eq!(q.files.recent[0], "/s/3.fdx");
        assert_eq!(q.appearance.zoom, 1.5);
        // A hand-edited value out of range is clamped when used.
        let wild = Prefs { appearance: Appearance { zoom: f64::NAN, ..Default::default() }, ..Default::default() };
        assert_eq!(wild.sanitized().appearance.zoom, 1.0);
        // Recovery.
        assert!(read_recovery(&dir).is_none());
        let s = scriptcraft_engine::sample::script();
        write_recovery(&dir, Some(Path::new("/s/x.fdx")), &s).unwrap();
        let r = read_recovery(&dir).unwrap();
        assert_eq!(r.script, s);
        assert_eq!(r.path.as_deref(), Some("/s/x.fdx"));
        std::fs::write(dir.join(RECOVERY), b"{garbage").unwrap();
        assert!(read_recovery(&dir).is_none());
        clear_recovery(&dir);
        // Backups keep the newest few.
        let f = dir.join("draft.fountain");
        assert_eq!(backup(&dir, &f, 3).unwrap(), None);
        for i in 0..5 {
            std::fs::write(&f, format!("v{i}")).unwrap();
            backup(&dir, &f, 3).unwrap();
        }
        let kept: Vec<_> = std::fs::read_dir(dir.join(BACKUPS)).unwrap().flatten().collect();
        assert_eq!(kept.len(), 3);
        let newest = kept.iter().map(|e| std::fs::read_to_string(e.path()).unwrap()).collect::<Vec<_>>();
        assert!(newest.contains(&"v4".to_string()) && !newest.contains(&"v0".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn imports_the_old_prefs_json_once_and_leaves_it() {
        let dir = tmp("legacy");
        let old = dir.join("old");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(
            old.join(LEGACY_PREFS),
            br#"{"recent":["/a.fdx"],"dark":true,"zoom":1.25,"spelling":false,"enterPicker":false,"autosaveSecs":30,"backups":2}"#,
        )
        .unwrap();
        let folders = scriptcraft_control_key::Folders {
            config: Some(dir.join("new")),
            data: Some(dir.join("data")),
            rule: "test".into(),
            legacy: Some(old.clone()),
        };
        let (store, report) = open_settings(&folders);
        assert!(matches!(report.source, craft_settings::Source::Legacy(_)), "{report:?}");
        let p = store.get();
        assert_eq!(p.appearance.theme, Theme::Dark);
        assert_eq!(p.appearance.zoom, 1.25);
        assert!(!p.editing.spelling && !p.editing.element_list_on_enter);
        assert_eq!((p.files.autosave_seconds, p.files.backups_kept), (30, 2));
        assert_eq!(p.files.recent, ["/a.fdx"]);
        assert!(old.join(LEGACY_PREFS).exists(), "the old file is left in place");
        assert!(parse_legacy_prefs(b"not json").is_err());
        // Isolated (no config folder): nothing is read or written.
        let iso = scriptcraft_control_key::Folders { config: None, data: None, rule: "isolated".into(), legacy: Some(old) };
        let (mut store, _) = open_settings(&iso);
        assert!(!store.is_persistent());
        assert_eq!(store.get(), &Prefs::default());
        store.update(|p| p.appearance.zoom = 2.0);
        store.flush().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dictionary_round_trip() {
        let dir = std::env::temp_dir().join(format!("scriptcraft-persist-{}", std::process::id()));
        let words = ["hesper".to_string(), "glorp".to_string()].into_iter().collect();
        super::save_words(&dir, &words).unwrap();
        assert_eq!(super::load_words(&dir), words);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(super::load_words(&dir).is_empty());
    }
}
