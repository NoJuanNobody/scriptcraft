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

/// The key for `port`: `SCRIPTCRAFT_CONTROL_KEY` when set, else the app's key file.
pub fn find_key(port: u16) -> Result<String, String> {
    if let Some(k) = std::env::var("SCRIPTCRAFT_CONTROL_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        return Ok(k);
    }
    let dir = settings_dir().ok_or("no settings folder: set SCRIPTCRAFT_CONTROL_KEY")?;
    let path = key_path(&dir, port);
    match std::fs::read_to_string(&path).map(|k| k.trim().to_string()) {
        Ok(k) if !k.is_empty() => Ok(k),
        _ => Err(format!("no control key: set SCRIPTCRAFT_CONTROL_KEY, or start the app with --control {port} (it writes {})", path.display())),
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
}
