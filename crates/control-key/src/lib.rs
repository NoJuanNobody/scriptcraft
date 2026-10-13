//! Where ScriptCraft keeps things on disk, with the Craft suite's folder rules from
//! [`craft-settings`](https://github.com/storytold/craft-libs/tree/main/crates/settings)
//! (storytold/craft-libs#22). One rule set for the app (which writes) and the MCP bridge (which
//! reads the control key).
//!
//! - **Config folder** (`config.toml`): the first match of a `--config-dir` override,
//!   `SCRIPTCRAFT_CONFIG_DIR` (set but empty = isolated: nothing is read or written), a
//!   `portable` marker next to the executable (folder `config/` beside it), then the platform
//!   folder (`~/Library/Application Support/scriptcraft`, `%APPDATA%\scriptcraft`,
//!   `~/.config/scriptcraft`).
//! - **Data folder** (autosave recovery, backups, user dictionary, control key): not settings,
//!   so a settings reset never loses them. The platform data folder by default; `data/` inside
//!   the config folder when that was chosen by an override, the variable or portable mode, so
//!   everything moves together. None when isolated.
//! - **Legacy folder**: where ScriptCraft 0.2 kept everything (`prefs.json`, …), read once for
//!   the import and never modified.
//!
//! The key file pattern is adapted from WordCraft (MIT OR Apache-2.0; see NOTICE).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

/// The Craft-suite application id (lowercase on every platform, craft-libs#12).
pub const APP_ID: &str = "scriptcraft";

/// The folders ScriptCraft uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    /// Where `config.toml` lives; `None` when isolated or in memory.
    pub config: Option<PathBuf>,
    /// Where recovery, backups, the dictionary and the control key live; `None` when isolated.
    pub data: Option<PathBuf>,
    /// Which rule chose them, for the About box and `ui.inspect`.
    pub rule: String,
    /// The pre-craft-settings folder, for importing old files.
    pub legacy: Option<PathBuf>,
}

/// The folders for this process: `config_dir` is a `--config-dir` override.
#[cfg(not(target_arch = "wasm32"))]
pub fn folders(config_dir: Option<PathBuf>) -> Folders {
    let location = config_dir.map_or(craft_settings::Location::Auto, craft_settings::Location::Dir);
    folders_with(&location, &craft_settings::Probe::default(), craft_settings::paths::data_dir(APP_ID), legacy_settings_dir())
}

/// [`folders`] with every input supplied (tests).
#[cfg(not(target_arch = "wasm32"))]
pub fn folders_with(
    location: &craft_settings::Location,
    probe: &craft_settings::Probe,
    platform_data: Option<PathBuf>,
    legacy: Option<PathBuf>,
) -> Folders {
    use craft_settings::Rule;
    let r = craft_settings::paths::resolve(APP_ID, &craft_settings::Portable::DEFAULT, location, probe);
    let data = match (&r.rule, &r.dir) {
        (Rule::Isolated(_) | Rule::Memory, _) | (_, None) => None,
        (Rule::Platform, Some(_)) => platform_data.or_else(|| r.dir.clone()),
        (_, Some(dir)) => Some(dir.join("data")),
    };
    // The old folder is the platform default's predecessor: import from it only when the platform
    // default is used, so a redirected or portable run never reads the user's real profile.
    let legacy = if r.rule == Rule::Platform { legacy } else { None };
    Folders { config: r.dir, data, rule: r.rule.to_string(), legacy }
}

/// The folders for the browser build: nothing is stored yet.
#[cfg(target_arch = "wasm32")]
pub fn folders(_config_dir: Option<PathBuf>) -> Folders {
    Folders { config: None, data: None, rule: "browser: settings are not stored".into(), legacy: None }
}

/// Where ScriptCraft 0.2 kept its settings: macOS `~/Library/Application Support/ScriptCraft`,
/// Windows `%APPDATA%\ScriptCraft`, otherwise `$XDG_CONFIG_HOME/scriptcraft` or
/// `~/.config/scriptcraft`.
pub fn legacy_settings_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/ScriptCraft"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("ScriptCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("scriptcraft"))
    }
}

/// `<data>/control-key.local-<port>`.
pub fn key_path(data: &Path, port: u16) -> PathBuf {
    data.join(format!("control-key.local-{port}"))
}

/// The key file's contents: the key, then the app's process id.
pub fn key_file_contents(key: &str, pid: u32) -> String {
    format!("{key}\n{pid}\n")
}

/// Parse a key file: (key, process id if recorded). Older files hold only the key.
pub fn parse_key_file(text: &str) -> Option<(String, Option<u32>)> {
    let mut lines = text.lines();
    let key = lines.next()?.trim().to_string();
    if key.is_empty() {
        return None;
    }
    let pid = lines.next().and_then(|l| l.trim().parse().ok());
    Some((key, pid))
}

/// Whether process `pid` is running (Unix: `kill -0`; elsewhere assumed running).
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(true)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// The key for `port`: `SCRIPTCRAFT_CONTROL_KEY` when set, else the app's key file in the data
/// folder (the same rules the app used, so set `SCRIPTCRAFT_CONFIG_DIR` the same way for both).
/// A key file left by an app that is no longer running is reported as stale.
pub fn find_key(port: u16) -> Result<String, String> {
    if let Some(k) = std::env::var("SCRIPTCRAFT_CONTROL_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        return Ok(k);
    }
    let f = folders(None);
    let dir = f.data.ok_or_else(|| format!("no data folder ({}): set SCRIPTCRAFT_CONTROL_KEY", f.rule))?;
    let path = key_path(&dir, port);
    match std::fs::read_to_string(&path).ok().as_deref().and_then(parse_key_file) {
        Some((_, Some(pid))) if !process_alive(pid) => Err(format!(
            "stale control key in {} (the app that wrote it, process {pid}, is not running): start the app with --control {port}",
            path.display()
        )),
        Some((k, _)) => Ok(k),
        None => Err(format!("no control key: set SCRIPTCRAFT_CONTROL_KEY, or start the app with --control {port} (it writes {})", path.display())),
    }
}

/// Compare keys in time independent of where they differ; an empty key never matches.
pub fn keys_match(given: &str, key: &str) -> bool {
    let (a, b) = (given.as_bytes(), key.as_bytes());
    !b.is_empty() && a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_matching() {
        assert_eq!(key_path(Path::new("/s"), 7990), Path::new("/s/control-key.local-7990"));
        assert!(keys_match("abc", "abc"));
        assert!(!keys_match("abd", "abc"));
        assert!(!keys_match("", ""));
        assert!(!keys_match("ab", "abc"));
    }

    #[test]
    fn key_files_record_the_process() {
        let t = key_file_contents("abc", 42);
        assert_eq!(parse_key_file(&t), Some(("abc".into(), Some(42))));
        assert_eq!(parse_key_file("abc\n"), Some(("abc".into(), None)));
        assert_eq!(parse_key_file(""), None);
        assert!(process_alive(std::process::id()));
        #[cfg(unix)]
        assert!(!process_alive(u32::MAX / 2));
    }

    fn probe(var: Option<&'static str>, exe: Option<PathBuf>) -> craft_settings::Probe {
        craft_settings::Probe {
            env: Box::new(move |name| (name == "SCRIPTCRAFT_CONFIG_DIR").then_some(var).flatten().map(Into::into)),
            exe_dir: exe,
            platform_config_dir: Some(PathBuf::from("/home/u/.config")),
        }
    }

    #[test]
    fn folder_rules() {
        use craft_settings::Location;
        let data = Some(PathBuf::from("/home/u/.local/share/scriptcraft"));
        // Platform default: config and data in their platform folders, lowercase id.
        let f = folders_with(&Location::Auto, &probe(None, None), data.clone(), None);
        assert_eq!(f.config, Some(PathBuf::from("/home/u/.config/scriptcraft")));
        assert_eq!(f.data, data);
        let old = Some(PathBuf::from("/home/u/.config/ScriptCraft-old"));
        assert_eq!(folders_with(&Location::Auto, &probe(None, None), data.clone(), old.clone()).legacy, old);
        // The variable moves everything together, and never reads the real old profile.
        let f = folders_with(&Location::Auto, &probe(Some("/tmp/sc"), None), data.clone(), old.clone());
        assert_eq!(f.legacy, None);
        assert_eq!(f.config, Some(PathBuf::from("/tmp/sc")));
        assert_eq!(f.data, Some(PathBuf::from("/tmp/sc/data")));
        assert!(f.rule.contains("SCRIPTCRAFT_CONFIG_DIR"));
        // Set but empty: isolated, nothing anywhere.
        let f = folders_with(&Location::Auto, &probe(Some(""), None), data.clone(), None);
        assert_eq!((f.config, f.data), (None, None));
        assert!(f.rule.contains("isolated"));
        // --config-dir wins over the variable.
        let f = folders_with(&Location::Dir("/x".into()), &probe(Some("/tmp/sc"), None), data.clone(), None);
        assert_eq!((f.config, f.data), (Some(PathBuf::from("/x")), Some(PathBuf::from("/x/data"))));
        // Memory: nothing.
        let f = folders_with(&Location::Memory, &probe(None, None), data.clone(), None);
        assert_eq!(f.data, None);
        // Portable marker next to the executable.
        let exe = std::env::temp_dir().join(format!("sc-portable-{}", std::process::id()));
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join("portable"), b"").unwrap();
        let f = folders_with(&Location::Auto, &probe(None, Some(exe.clone())), data, None);
        assert_eq!(f.config, Some(exe.join("config")));
        assert_eq!(f.data, Some(exe.join("config/data")));
        let _ = std::fs::remove_dir_all(&exe);
    }

    #[test]
    fn legacy_folder_is_the_apps_old_folder() {
        if let Some(d) = legacy_settings_dir() {
            let name = d.file_name().and_then(|n| n.to_str()).unwrap_or("");
            assert!(name == "scriptcraft" || name == "ScriptCraft", "{}", d.display());
        }
    }
}
