//! What the app keeps between runs, in its settings folder:
//!
//! - `dictionary.txt`: the user dictionary, one word per line
//! - `prefs.json`: recent files, theme, zoom, spelling, autosave and backup settings
//! - `recovery.json`: the autosaved copy of a script with unsaved changes (removed once saved)
//! - `backups/`: the previous version of each file, kept before it is overwritten
//!
//! Nothing here runs unless the app sets `ScriptApp::state_dir`; tests use temp folders.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use scriptcraft_doc::Script;
use serde::{Deserialize, Serialize};

pub const PREFS: &str = "prefs.json";
pub const RECOVERY: &str = "recovery.json";
pub const BACKUPS: &str = "backups";
/// Most recent files remembered.
pub const MAX_RECENT: usize = 10;

/// Preferences kept between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub recent: Vec<String>,
    pub dark: Option<bool>,
    pub zoom: f32,
    pub spelling: bool,
    pub enter_picker: bool,
    /// Seconds between autosaves of a script with unsaved changes (0 = off).
    pub autosave_secs: u32,
    /// Backups kept per file (0 = none).
    pub backups: u32,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { recent: Vec::new(), dark: None, zoom: 1.0, spelling: true, enter_picker: true, autosave_secs: 60, backups: 5 }
    }
}

impl Prefs {
    /// Put `path` first in the recent list.
    pub fn touch_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(MAX_RECENT);
    }
}

pub fn load_prefs(dir: &Path) -> Prefs {
    let mut p: Prefs = std::fs::read_to_string(dir.join(PREFS)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    p.recent.truncate(MAX_RECENT);
    p.zoom = if p.zoom.is_finite() { p.zoom.clamp(0.4, 3.0) } else { 1.0 };
    p.autosave_secs = p.autosave_secs.min(3600);
    p.backups = p.backups.min(100);
    p
}

pub fn save_prefs(dir: &Path, p: &Prefs) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut p = p.clone();
    p.zoom = if p.zoom.is_finite() { p.zoom.clamp(0.4, 3.0) } else { 1.0 };
    let bytes = serde_json::to_vec_pretty(&p).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(PREFS), bytes)
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
        let mut p = load_prefs(&dir);
        assert_eq!(p, Prefs::default());
        for i in 0..14 {
            p.touch_recent(&format!("/s/{i}.fdx"));
        }
        p.touch_recent("/s/3.fdx");
        assert_eq!(p.recent.len(), MAX_RECENT);
        assert_eq!(p.recent[0], "/s/3.fdx");
        p.zoom = f32::NAN;
        save_prefs(&dir, &p).unwrap();
        let q = load_prefs(&dir);
        assert_eq!(q.recent, p.recent);
        assert_eq!(q.zoom, 1.0);
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
    fn dictionary_round_trip() {
        let dir = std::env::temp_dir().join(format!("scriptcraft-persist-{}", std::process::id()));
        let words = ["hesper".to_string(), "glorp".to_string()].into_iter().collect();
        super::save_words(&dir, &words).unwrap();
        assert_eq!(super::load_words(&dir), words);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(super::load_words(&dir).is_empty());
    }
}
