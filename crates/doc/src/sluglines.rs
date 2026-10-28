//! Scene-heading words ("sluglines"): the abbreviations that start a scene heading (`INT.`,
//! `EXT.`, `INT./EXT.`) and the times of day after the dash.
//!
//! Screenwriters outside the English-speaking world use their own words (`ИНТ.`/`НАТ.`,
//! `INNEN.`/`AUSSEN.`, `WN.`/`PL.`), and the same abbreviation can mean different things (`EST.`
//! is "establishing" in English but "exterior" in Italian). So the words are data: the user picks
//! a built-in set ([`Sluglines::preset`]) and edits it. Everything that recognises a scene heading
//! (typing, Fountain, SmartType, reports) goes through one [`Sluglines`].

use serde::{Deserialize, Serialize};

/// Most words in one list (hostile input is capped).
pub const MAX_WORDS: usize = 64;
/// Longest word kept, in characters.
pub const MAX_WORD_CHARS: usize = 40;

/// Whether a heading is set inside, outside or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Setting {
    Interior,
    Exterior,
    Both,
}

/// The scene-heading words in use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sluglines {
    /// Prefixes for interior scenes (`INT.`).
    pub interior: Vec<String>,
    /// Prefixes for exterior scenes (`EXT.`, `EST.`).
    pub exterior: Vec<String>,
    /// Prefixes for scenes both inside and outside (`INT./EXT.`, `I/E.`).
    pub both: Vec<String>,
    /// Times of day offered after the dash (`DAY`, `NIGHT`).
    pub times: Vec<String>,
}

/// Built-in sets: (code, name, interior, exterior, both, times).
type Preset = (&'static str, &'static str, &'static [&'static str], &'static [&'static str], &'static [&'static str], &'static [&'static str]);

const PRESETS: &[Preset] = &[
    (
        "en",
        "English",
        &["INT."],
        &["EXT.", "EST."],
        &["INT./EXT.", "EXT./INT.", "INT/EXT.", "EXT/INT.", "I/E.", "I/E"],
        &["DAY", "NIGHT", "MORNING", "AFTERNOON", "EVENING", "DAWN", "DUSK", "CONTINUOUS", "LATER", "MOMENTS LATER", "SAME TIME"],
    ),
    (
        "ru",
        "Русский",
        &["ИНТ."],
        &["НАТ."],
        &["ИНТ./НАТ.", "НАТ./ИНТ.", "ИНТ/НАТ.", "НАТ/ИНТ."],
        &["ДЕНЬ", "НОЧЬ", "УТРО", "ВЕЧЕР", "РАССВЕТ", "ЗАКАТ", "СУМЕРКИ", "ПОЗЖЕ", "ПРОДОЛЖЕНИЕ", "ТО ЖЕ ВРЕМЯ"],
    ),
    (
        "uk",
        "Українська",
        &["ІНТ."],
        &["НАТ.", "ЕКСТ."],
        &["ІНТ./НАТ.", "НАТ./ІНТ."],
        &["ДЕНЬ", "НІЧ", "РАНОК", "ВЕЧІР", "СВІТАНОК", "СУТІНКИ", "ПІЗНІШЕ", "ПРОДОВЖЕННЯ"],
    ),
    (
        "de",
        "Deutsch",
        &["INNEN.", "INT."],
        &["AUSSEN.", "EXT."],
        &["INNEN/AUSSEN.", "AUSSEN/INNEN.", "INT./EXT."],
        &["TAG", "NACHT", "MORGEN", "ABEND", "DÄMMERUNG", "SPÄTER", "FORTLAUFEND"],
    ),
    (
        "fr",
        "Français",
        &["INT."],
        &["EXT."],
        &["INT./EXT.", "EXT./INT."],
        &["JOUR", "NUIT", "MATIN", "SOIR", "AUBE", "CRÉPUSCULE", "PLUS TARD", "CONTINU"],
    ),
    (
        "es",
        "Español",
        &["INT."],
        &["EXT."],
        &["INT./EXT.", "EXT./INT."],
        &["DÍA", "NOCHE", "MAÑANA", "TARDE", "AMANECER", "ATARDECER", "MÁS TARDE", "CONTINUO"],
    ),
    (
        "it",
        "Italiano",
        &["INT."],
        &["EST."],
        &["INT./EST.", "EST./INT."],
        &["GIORNO", "NOTTE", "MATTINA", "SERA", "ALBA", "TRAMONTO", "PIÙ TARDI", "CONTINUA"],
    ),
    (
        "pt",
        "Português",
        &["INT."],
        &["EXT."],
        &["INT./EXT.", "EXT./INT."],
        &["DIA", "NOITE", "MANHÃ", "TARDE", "AMANHECER", "ENTARDECER", "MAIS TARDE", "CONTÍNUO"],
    ),
    ("pl", "Polski", &["WN."], &["PL."], &["WN./PL.", "PL./WN."], &["DZIEŃ", "NOC", "RANO", "WIECZÓR", "ŚWIT", "ZMIERZCH", "PÓŹNIEJ"]),
];

/// The dashes that may separate the location from the time of day.
const DASHES: [&str; 3] = [" - ", " – ", " — "];

impl Default for Sluglines {
    /// English and Russian together, as ScriptCraft has always read them.
    fn default() -> Self {
        let mut s = Sluglines::preset("en").unwrap_or_else(Sluglines::empty);
        if let Some(ru) = Sluglines::preset("ru") {
            s.interior.extend(ru.interior);
            s.exterior.extend(ru.exterior);
            s.both.extend(ru.both);
            s.times.extend(ru.times);
        }
        s
    }
}

fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

/// `text` without a leading `prefix`, compared without case, cut at a char boundary of `text`.
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let mut rest = text.char_indices();
    for pc in prefix.chars() {
        let (_, tc) = rest.next()?;
        if tc != pc && !tc.to_uppercase().eq(pc.to_uppercase()) {
            return None;
        }
    }
    Some(rest.next().map_or("", |(i, _)| text.get(i..).unwrap_or("")))
}

impl Sluglines {
    /// No words at all.
    pub fn empty() -> Self {
        Sluglines { interior: Vec::new(), exterior: Vec::new(), both: Vec::new(), times: Vec::new() }
    }

    /// The built-in sets: (code, name).
    pub fn presets() -> impl Iterator<Item = (&'static str, &'static str)> {
        PRESETS.iter().map(|p| (p.0, p.1))
    }

    /// A built-in set by language code (`"en"`, `"ru"`, `"de"` …).
    pub fn preset(code: &str) -> Option<Self> {
        let code = code.trim().to_ascii_lowercase();
        PRESETS.iter().find(|p| p.0 == code).map(|p| Sluglines { interior: owned(p.2), exterior: owned(p.3), both: owned(p.4), times: owned(p.5) })
    }

    /// Clean hand-edited or hostile lists: trimmed, upper case, no empties or duplicates, capped.
    pub fn sanitized(mut self) -> Self {
        let mut seen: Vec<String> = Vec::new();
        for list in [&mut self.interior, &mut self.exterior, &mut self.both] {
            *list = clean(std::mem::take(list), &mut seen);
        }
        self.times = clean(std::mem::take(&mut self.times), &mut Vec::new());
        self
    }

    /// Every prefix with its setting, longest first (so `INT./EXT.` wins over `INT.`).
    pub fn prefixes(&self) -> Vec<(&str, Setting)> {
        let mut v: Vec<(&str, Setting)> = self
            .interior
            .iter()
            .map(|w| (w.as_str(), Setting::Interior))
            .chain(self.exterior.iter().map(|w| (w.as_str(), Setting::Exterior)))
            .chain(self.both.iter().map(|w| (w.as_str(), Setting::Both)))
            .filter(|(w, _)| !w.trim().is_empty())
            .collect();
        v.sort_by_key(|(w, _)| std::cmp::Reverse(w.trim().chars().count()));
        v
    }

    /// The prefix a heading starts with: (the prefix as written in `text`, its setting). The
    /// prefix's letters must be followed by a dot or a space, as Fountain has it (`INT.` matches
    /// `INT. X`, `INT X` and `int.`, not `INTERIOR` or a bare `INT`); a dot in `text` is part of
    /// the match.
    pub fn match_prefix<'a>(&self, text: &'a str) -> Option<(&'a str, Setting)> {
        let t = text.trim_start();
        for (word, setting) in self.prefixes() {
            let stem = word.trim().trim_end_matches('.');
            if stem.is_empty() {
                continue;
            }
            let Some(rest) = strip_prefix_ci(t, stem) else { continue };
            if !(rest.starts_with('.') || rest.starts_with(' ')) {
                continue;
            }
            let rest = rest.strip_prefix('.').unwrap_or(rest);
            let len = t.len() - rest.len();
            return Some((t.get(..len).unwrap_or(""), setting));
        }
        None
    }

    /// A natural scene heading (Fountain): the line starts with a known prefix.
    pub fn is_heading(&self, line: &str) -> bool {
        self.match_prefix(line).is_some()
    }

    /// Text typed as an action that should become a scene heading: a prefix exactly as listed,
    /// then a space (`int. ` → heading; `Innen ist es dunkel` stays action even with `INNEN.`).
    pub fn starts_heading(&self, text: &str) -> bool {
        self.prefixes().iter().any(|(word, _)| strip_prefix_ci(text, word.trim()).is_some_and(|rest| rest.starts_with(' ')))
    }

    /// The setting of a heading, if it starts with a known prefix.
    pub fn setting(&self, heading: &str) -> Option<Setting> {
        self.match_prefix(heading).map(|(_, s)| s)
    }

    /// A heading split into (prefix, location, time): `"INT. DINER - NIGHT"` →
    /// `("INT.", "DINER", "NIGHT")`. Missing parts are empty. The time follows the last
    /// ` - `, ` – ` or ` — `.
    pub fn parts(&self, h: &str) -> (String, String, String) {
        let t = h.trim();
        let prefix = self.match_prefix(t).map_or("", |(p, _)| p);
        let rest = t.get(prefix.len()..).unwrap_or("").trim();
        let (loc, time) = match time_dash(rest) {
            Some((a, b)) => (rest.get(..a).unwrap_or(""), rest.get(b..).unwrap_or("")),
            None => (rest, ""),
        };
        (prefix.trim().to_string(), loc.trim().to_string(), time.trim().to_string())
    }
}

/// The last dash before a time of day in `s`: (where the location ends, where the time starts).
pub fn time_dash(s: &str) -> Option<(usize, usize)> {
    DASHES.iter().filter_map(|d| s.rfind(d).map(|i| (i, i + d.len()))).max_by_key(|(i, _)| *i)
}

fn clean(words: Vec<String>, seen: &mut Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for w in words {
        let w: String = w.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase().chars().take(MAX_WORD_CHARS).collect();
        if w.is_empty() || seen.contains(&w) || out.len() >= MAX_WORDS {
            continue;
        }
        seen.push(w.clone());
        out.push(w);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reads_english_and_russian() {
        let s = Sluglines::default();
        assert_eq!(s.parts("INT. DINER - NIGHT"), ("INT.".into(), "DINER".into(), "NIGHT".into()));
        assert_eq!(s.parts("int./ext. car - moving - day"), ("int./ext.".into(), "car - moving".into(), "day".into()));
        assert_eq!(s.parts("ИНТ. КУХНЯ – НОЧЬ"), ("ИНТ.".into(), "КУХНЯ".into(), "НОЧЬ".into()));
        assert_eq!(s.setting("НАТ./ИНТ. МАШИНА"), Some(Setting::Both));
        assert_eq!(s.setting("EST. CITY"), Some(Setting::Exterior));
        assert_eq!(s.setting("INTERIOR DESIGN"), None);
        assert!(s.is_heading("INT KITCHEN"));
        assert!(s.is_heading("i/e car"));
        assert!(!s.is_heading("INNEN. KÜCHE"));
        assert!(!s.is_heading("INT"));
    }

    #[test]
    fn presets_change_what_a_heading_is() {
        let de = Sluglines::preset("DE").unwrap();
        assert_eq!(de.parts("Innen. Küche - Tag"), ("Innen.".into(), "Küche".into(), "Tag".into()));
        assert_eq!(de.setting("INNEN/AUSSEN. AUTO"), Some(Setting::Both));
        let it = Sluglines::preset("it").unwrap();
        assert_eq!(it.setting("EST. PIAZZA - GIORNO"), Some(Setting::Exterior));
        let pl = Sluglines::preset("pl").unwrap();
        assert_eq!(pl.parts("WN. KUCHNIA – DZIEŃ").1, "KUCHNIA");
        assert!(!pl.is_heading("INT. KITCHEN"));
        assert!(Sluglines::preset("xx").is_none());
        assert!(Sluglines::presets().count() >= 9);
    }

    #[test]
    fn typing_needs_the_listed_word_and_a_space() {
        let de = Sluglines::preset("de").unwrap();
        assert!(de.starts_heading("innen. "));
        assert!(!de.starts_heading("Innen ist es dunkel."));
        assert!(!de.starts_heading("innen."));
        let en = Sluglines::default();
        assert!(en.starts_heading("i/e "));
        assert!(en.starts_heading("ИНТ. "));
        assert!(!en.starts_heading("Int the end"));
    }

    #[test]
    fn hostile_lists_are_cleaned() {
        let s = Sluglines {
            interior: vec!["  int.  ".into(), "".into(), "INT.".into(), "x".repeat(500)],
            exterior: vec!["int.".into(), "ext.".into()],
            both: (0..500).map(|i| format!("W{i}.")).collect(),
            times: vec!["day".into(), "DAY".into()],
        }
        .sanitized();
        assert_eq!(s.interior.first().map(String::as_str), Some("INT."));
        assert_eq!(s.interior.len(), 2);
        assert_eq!(s.interior.get(1).map(|w| w.chars().count()), Some(MAX_WORD_CHARS));
        assert_eq!(s.exterior, vec!["EXT.".to_string()]);
        assert_eq!(s.both.len(), MAX_WORDS);
        assert_eq!(s.times, vec!["DAY".to_string()]);
        // Odd words never panic.
        let odd = Sluglines { interior: vec![".".into(), "ß.".into(), "ǅ".into()], ..Sluglines::empty() };
        for h in ["", ".", "SS. X", "ß x", "ǆ", "ǅ.", "\u{0}"] {
            let _ = odd.parts(h);
            let _ = odd.starts_heading(h);
        }
        assert_eq!(Sluglines::empty().parts("INT. X - DAY"), ("".into(), "INT. X".into(), "DAY".into()));
    }
}
