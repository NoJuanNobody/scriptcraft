//! What the app keeps between runs, in its settings folder: the user dictionary (one word per
//! line). Nothing here runs unless the app sets `ScriptApp::state_dir`; tests use temp folders.

use std::collections::BTreeSet;
use std::path::Path;

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
