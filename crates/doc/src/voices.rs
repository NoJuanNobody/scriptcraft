//! Read-aloud voices kept with the script: the narrator's voice (action, scene headings) and a
//! voice per character. Voice names are whatever the reader's speech engine calls them
//! ("Samantha", "en-us"); a name the engine doesn't have falls back to an automatic voice.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Most characters with an assigned voice.
pub const MAX_CAST_VOICES: usize = 500;
/// Longest voice name kept (characters).
const MAX_VOICE_NAME: usize = 80;

/// Who reads what.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Voices {
    /// The narrator's voice; `None` = automatic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub narrator: Option<String>,
    /// Character (cue name in capitals, without extensions) → voice.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub cast: BTreeMap<String, String>,
}

impl Voices {
    pub fn is_default(&self) -> bool {
        self.narrator.is_none() && self.cast.is_empty()
    }

    /// Drop empty or oversized entries (after reading a file).
    pub fn normalize(&mut self) {
        self.narrator = self.narrator.as_deref().and_then(clean_voice_name);
        let cast = std::mem::take(&mut self.cast);
        for (k, v) in cast {
            if self.cast.len() >= MAX_CAST_VOICES {
                break;
            }
            let k = character_key(&k);
            if let (false, Some(v)) = (k.is_empty(), clean_voice_name(&v)) {
                self.cast.insert(k, v);
            }
        }
    }
}

/// A voice name as stored: trimmed, no control characters, at most 80 characters; `None` when
/// nothing is left.
pub fn clean_voice_name(s: &str) -> Option<String> {
    let v: String = s.chars().filter(|c| !c.is_control()).take(MAX_VOICE_NAME).collect();
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// The character a cue names: `NELL (V.O.)` → `NELL`, `TEO (CONT'D) ^` → `TEO`.
pub fn character_key(cue: &str) -> String {
    let name = cue.split('(').next().unwrap_or("");
    let name = name.trim().trim_end_matches('^').trim();
    name.chars().filter(|c| !c.is_control()).take(120).collect::<String>().to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_names() {
        assert_eq!(character_key("Nell (V.O.)"), "NELL");
        assert_eq!(character_key("  COAST GUARD (V.O.) (CONT'D) "), "COAST GUARD");
        assert_eq!(character_key("TEO ^"), "TEO");
        assert_eq!(character_key("(beat)"), "");
        assert_eq!(clean_voice_name("  Samantha \u{7}"), Some("Samantha".into()));
        assert_eq!(clean_voice_name("   "), None);
        assert_eq!(clean_voice_name(&"x".repeat(500)).map(|v| v.len()), Some(80));
        let mut v = Voices { narrator: Some(" ".into()), cast: [("nell (v.o.)".into(), " Daniel ".into()), ("".into(), "Fred".into())].into() };
        v.normalize();
        assert_eq!(v.narrator, None);
        assert_eq!(v.cast.get("NELL").map(String::as_str), Some("Daniel"));
        assert_eq!(v.cast.len(), 1);
    }
}
