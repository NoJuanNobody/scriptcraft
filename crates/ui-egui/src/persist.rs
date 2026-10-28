//! What the app keeps between runs.
//!
//! **Settings** go through [`craft_settings`] (storytold/craft-libs#22), like every Craft app:
//! one commented `config.toml` in the config folder, saved half a second after a change, with a
//! `.bak`, recovery of damaged files and schema migration. [`Prefs`] is ScriptCraft's settings
//! type; the old `prefs.json` is imported on the first run and left in place.
//!
//! **Data** is not settings and lives in the data folder (`scriptcraft_control_key::Folders`), so
//! a settings reset never loses work:
//! - `recovery/`: the autosaved copy of a script with unsaved changes, one per running app
//!   (removed once saved); `recovery.json` from older versions is still offered
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
/// Where ScriptCraft 0.5 and earlier kept its one recovery copy (still read).
pub const RECOVERY: &str = "recovery.json";
/// Recovery copies, one per running app (see [`RecoverySlot`]).
pub const RECOVERY_DIR: &str = "recovery";
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
    pub language: crate::i18n::Language,
    /// Page zoom (0.4 to 3.0).
    pub zoom: f64,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: Theme::System, language: Default::default(), zoom: 1.0 }
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
    const SCHEMA_VERSION: u32 = 2;

    fn migrate(from: u32, doc: &mut craft_settings::DocumentMut) -> Result<(), craft_settings::MigrateError> {
        // Version 2 adds only an optional field; missing language keeps existing profiles in English.
        if from == 1
            && let Some(appearance) = doc.get_mut("appearance").and_then(|item| item.as_table_like_mut())
            && !appearance.contains_key("language")
        {
            appearance.insert("language", "en".into());
        }
        Ok(())
    }
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
        appearance: Appearance { theme, zoom: old.zoom.unwrap_or(d.appearance.zoom), ..Default::default() },
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

/// Write `bytes` to `file` so that a crash or power cut leaves either the old or the new
/// contents, never a torn file: a temporary file next to it, flushed to disk, renamed over it,
/// then the folder flushed so the rename itself survives.
pub fn write_durably(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = file.parent().ok_or_else(|| std::io::Error::other("no folder"))?;
    std::fs::create_dir_all(dir)?;
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!("{name}.tmp"));
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    close(f);
    std::fs::rename(&tmp, file)?;
    // Windows can't open a folder as a file; its rename is already durable enough.
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

/// Autosave `script` (saved at `path`, if it was) into the recovery copy `file`.
pub fn write_recovery_file(file: &Path, path: Option<&Path>, script: &Script) -> std::io::Result<()> {
    let r = Recovery { path: path.map(|p| p.display().to_string()), saved_at: now_secs(), script: script.clone() };
    write_durably(file, &serde_json::to_vec(&r).map_err(std::io::Error::other)?)
}

/// A recovery copy (normalised; a missing or corrupt file counts as none).
pub fn read_recovery_file(file: &Path) -> Option<Recovery> {
    let t = std::fs::read(file).ok()?;
    let mut r: Recovery = serde_json::from_slice(&t).ok()?;
    r.script.normalize().ok()?;
    Some(r)
}

/// The recovery copy ScriptCraft 0.5 and earlier kept directly in the data folder (still read).
pub fn read_recovery(dir: &Path) -> Option<Recovery> {
    read_recovery_file(&dir.join(RECOVERY))
}

/// Close a file (and release its lock) before the path is renamed or removed: Windows can't
/// remove an open file. (A function, not `drop`, as the browser build's `File` has no `Drop`.)
fn close(_file: std::fs::File) {}

/// Hold `file` locked for this process (`None` when another process holds it). The lock goes
/// away with the process, so a crashed run's copy is free again at the next start.
fn try_lock(file: &Path) -> std::io::Result<Option<std::fs::File>> {
    let f = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(file)?;
    match f.try_lock() {
        Ok(()) => Ok(Some(f)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// This run's own recovery copy: `recovery/run-<time>-<pid>.json` in the data folder, with a
/// `.lock` beside it held while the app runs. Every running ScriptCraft has its own, so two
/// windows never overwrite (or clear) each other's unsaved work, and a copy whose lock is free
/// was left by a run that ended without cleaning up.
#[derive(Debug)]
pub struct RecoverySlot {
    pub file: PathBuf,
    lock_path: PathBuf,
    lock: Option<std::fs::File>,
}

impl RecoverySlot {
    pub fn claim(data: &Path) -> std::io::Result<RecoverySlot> {
        let dir = data.join(RECOVERY_DIR);
        std::fs::create_dir_all(&dir)?;
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stem = format!("run-{}-{}-{n}", now_secs(), std::process::id());
        let lock_path = dir.join(format!("{stem}.lock"));
        let lock = try_lock(&lock_path)?.ok_or_else(|| std::io::Error::other("recovery copy is in use"))?;
        Ok(RecoverySlot { file: dir.join(format!("{stem}.json")), lock_path, lock: Some(lock) })
    }

    /// Remove the copy (the script was saved, or the user chose not to keep it).
    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

impl Drop for RecoverySlot {
    fn drop(&mut self) {
        // Unlock first: Windows can't remove an open file.
        if let Some(f) = self.lock.take() {
            close(f);
        }
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// A recovery copy left by a run that did not end cleanly, offered at start-up. Its lock is
/// held while it is offered, so a second window started now doesn't offer it too.
#[derive(Debug)]
pub struct Orphan {
    pub file: PathBuf,
    pub recovery: Recovery,
    lock: Option<(PathBuf, std::fs::File)>,
}

impl Orphan {
    /// The user recovered or discarded it: remove the copy.
    pub fn remove(mut self) {
        let _ = std::fs::remove_file(&self.file);
        if let Some((path, f)) = self.lock.take() {
            close(f);
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Recovery copies nobody is using, newest first: those in `data/recovery/` whose run is gone,
/// and the single copy older versions kept in `data` (and in the `legacy` folder).
pub fn orphans(data: &Path, legacy: Option<&Path>) -> Vec<Orphan> {
    let mut out = Vec::new();
    let dir = data.join(RECOVERY_DIR);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    files.sort();
    for file in files {
        match file.extension().and_then(|e| e.to_str()) {
            Some("json") => {}
            // A lock left without its copy by a run that ended: tidy it away.
            Some("lock") if !file.with_extension("json").exists() => {
                if let Ok(Some(lock)) = try_lock(&file) {
                    close(lock);
                    let _ = std::fs::remove_file(&file);
                }
                continue;
            }
            _ => continue,
        }
        let lock_path = file.with_extension("lock");
        let Ok(Some(lock)) = try_lock(&lock_path) else { continue };
        match read_recovery_file(&file) {
            Some(recovery) => out.push(Orphan { file, recovery, lock: Some((lock_path, lock)) }),
            // Unreadable: leave it for a person to look at, offer nothing.
            None => {
                close(lock);
                let _ = std::fs::remove_file(&lock_path);
            }
        }
    }
    for d in std::iter::once(data).chain(legacy) {
        let file = d.join(RECOVERY);
        if let Some(recovery) = read_recovery_file(&file) {
            out.push(Orphan { file, recovery, lock: None });
        }
    }
    out.sort_by_key(|o| std::cmp::Reverse(o.recovery.saved_at));
    out
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
    fn version_one_settings_migrate_without_losing_hand_edits() {
        use craft_settings::Settings;
        let mut doc: craft_settings::DocumentMut =
            "schema_version = 1\n[appearance]\ntheme = \"dark\"\nzoom = 1.25\ncustom = \"keep\"\n".parse().unwrap();
        Prefs::migrate(1, &mut doc).unwrap();
        assert_eq!(doc["appearance"]["language"].as_str(), Some("en"));
        assert_eq!(doc["appearance"]["theme"].as_str(), Some("dark"));
        assert_eq!(doc["appearance"]["custom"].as_str(), Some("keep"));
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
            p.appearance.language = crate::i18n::Language::Russian;
        });
        store.flush().unwrap();
        let toml = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(toml.starts_with("schema_version = 2"), "{toml}");
        assert!(toml.contains("[appearance]") && toml.contains("theme = \"dark\""), "{toml}");
        let (again, _) = open_settings(&folders);
        let q = again.get().clone();
        assert_eq!(q.files.recent.len(), MAX_RECENT);
        assert_eq!(q.files.recent[0], "/s/3.fdx");
        assert_eq!(q.appearance.zoom, 1.5);
        assert_eq!(q.appearance.language, crate::i18n::Language::Russian);
        // A hand-edited value out of range is clamped when used.
        let wild = Prefs { appearance: Appearance { zoom: f64::NAN, ..Default::default() }, ..Default::default() };
        assert_eq!(wild.sanitized().appearance.zoom, 1.0);
        // Recovery copies round-trip; a corrupt one counts as none.
        assert!(read_recovery(&dir).is_none());
        let s = scriptcraft_engine::sample::script();
        let f = dir.join("copy.json");
        write_recovery_file(&f, Some(Path::new("/s/x.fdx")), &s).unwrap();
        let r = read_recovery_file(&f).unwrap();
        assert_eq!(r.script, s);
        assert_eq!(r.path.as_deref(), Some("/s/x.fdx"));
        assert!(!dir.join("copy.json.tmp").exists());
        std::fs::write(&f, b"{garbage").unwrap();
        assert!(read_recovery_file(&f).is_none());
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
    fn each_run_has_its_own_recovery_copy_and_leftovers_are_offered() {
        let dir = tmp("slots");
        let s = scriptcraft_engine::sample::script();
        // Two windows at once: separate copies, and neither offers the other's live copy.
        let a = RecoverySlot::claim(&dir).unwrap();
        let b = RecoverySlot::claim(&dir).unwrap();
        assert_ne!(a.file, b.file);
        write_recovery_file(&a.file, None, &Script::new()).unwrap();
        write_recovery_file(&b.file, Some(Path::new("/s/b.fdx")), &s).unwrap();
        assert!(orphans(&dir, None).is_empty());
        b.clear();
        assert!(read_recovery_file(&a.file).is_some(), "clearing one window's copy keeps the other's");
        // A run that ends without clearing (a crash drops its lock) leaves its copy to offer.
        write_recovery_file(&b.file, Some(Path::new("/s/b.fdx")), &s).unwrap();
        drop(b);
        // The copy older versions kept, here and in the legacy folder, is offered too.
        let old = dir.join("old");
        let mut older = s.clone();
        older.paras.truncate(1);
        write_durably(&old.join(RECOVERY), &serde_json::to_vec(&Recovery { path: None, saved_at: 1, script: older }).unwrap()).unwrap();
        std::fs::write(dir.join(RECOVERY_DIR).join("broken.json"), b"{").unwrap();
        let found = orphans(&dir, Some(&old));
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].recovery.path.as_deref(), Some("/s/b.fdx"), "newest first");
        assert_eq!(found[1].recovery.saved_at, 1);
        // While offered here, another start doesn't offer it again.
        assert_eq!(orphans(&dir, Some(&old)).len(), 1);
        for o in found {
            o.remove();
        }
        assert!(orphans(&dir, Some(&old)).is_empty());
        // A broken copy is never offered, nor deleted.
        assert!(dir.join(RECOVERY_DIR).join("broken.json").exists());
        drop(a);
        let left: Vec<_> = std::fs::read_dir(dir.join(RECOVERY_DIR)).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 2, "a's copy and the broken one; no lock files: {left:?}");
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
