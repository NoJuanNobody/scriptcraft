//! The control channel's key file, one rule for the app (which writes it) and the MCP bridge
//! (which reads it): `<settings>/control-key.local-<port>`. Pattern adapted from WordCraft
//! (MIT OR Apache-2.0; see NOTICE).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

/// The app's settings folder: macOS `~/Library/Application Support/ScriptCraft`, Windows
/// `%APPDATA%\ScriptCraft`, otherwise `$XDG_CONFIG_HOME/scriptcraft` or `~/.config/scriptcraft`.
pub fn settings_dir() -> Option<PathBuf> {
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

/// `<settings>/control-key.local-<port>`.
pub fn key_path(settings: &Path, port: u16) -> PathBuf {
    settings.join(format!("control-key.local-{port}"))
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

/// The key for `port`: `SCRIPTCRAFT_CONTROL_KEY` when set, else the app's key file. A key file
/// left by an app that is no longer running is reported as stale.
pub fn find_key(port: u16) -> Result<String, String> {
    if let Some(k) = std::env::var("SCRIPTCRAFT_CONTROL_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        return Ok(k);
    }
    let dir = settings_dir().ok_or("no settings folder: set SCRIPTCRAFT_CONTROL_KEY")?;
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
}
