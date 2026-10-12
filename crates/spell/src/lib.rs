//! Offline spelling: an English word list (Kevin Atkinson's ESDB/SCOWL, size 60, US and British
//! -ise spellings; licence in `words/LICENSE-SCOWL.txt`) with Damerau edit-distance suggestions.
//!
//! Words are letters with inner apostrophes (`don't`, `Nell's`); tokens with digits are skipped.
//! Matching ignores case, so all-caps screenplay text (`INT.`, character cues) is checked too.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::HashSet;
use std::sync::OnceLock;

const WORDS: &str = include_str!("../words/en.txt");
/// The more common words (ESDB size 35), preferred among equally close suggestions.
const COMMON: &str = include_str!("../words/common.txt");

/// Longest word we check or suggest for (longer tokens are skipped).
const MAX_WORD: usize = 40;

/// The word list.
pub struct Speller {
    words: HashSet<String>,
    common: HashSet<String>,
}

/// The built-in English speller (loaded once).
pub fn english() -> &'static Speller {
    static S: OnceLock<Speller> = OnceLock::new();
    let set = |s: &str| s.lines().map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect();
    S.get_or_init(|| Speller { words: set(WORDS), common: set(COMMON) })
}

/// Normalise a word for lookup: lowercase, curly apostrophes straight.
pub fn key(word: &str) -> String {
    word.replace('’', "'").to_lowercase()
}

/// Word tokens of `text` as byte ranges: letters with inner apostrophes.
pub fn words(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut digits = false;
    let cs: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(i, c)) in cs.iter().enumerate() {
        let inner_apostrophe = matches!(c, '\'' | '’') && start.is_some() && cs.get(k + 1).is_some_and(|(_, n)| n.is_alphabetic());
        if c.is_alphanumeric() || inner_apostrophe {
            if start.is_none() {
                start = Some(i);
                digits = false;
            }
            digits |= c.is_numeric();
        } else if let Some(s) = start.take()
            && !digits
        {
            out.push((s, i));
        }
    }
    if let Some(s) = start
        && !digits
    {
        out.push((s, text.len()));
    }
    out
}

impl Speller {
    /// Whether `word` is in the list (case-insensitive; a possessive `'s` is accepted on known words).
    pub fn check(&self, word: &str) -> bool {
        let k = key(word);
        if k.chars().count() > MAX_WORD || k.chars().count() <= 1 {
            return true;
        }
        self.words.contains(&k) || k.strip_suffix("'s").is_some_and(|b| self.words.contains(b))
    }

    /// Misspelled words of `text` (byte ranges), skipping those `known` accepts.
    pub fn misspelled(&self, text: &str, known: &dyn Fn(&str) -> bool) -> Vec<(usize, usize)> {
        words(text).into_iter().filter(|&(a, b)| text.get(a..b).is_some_and(|w| !self.check(w) && !known(&key(w)))).collect()
    }

    /// Up to `n` suggestions for `word`, closest first, in the word's capitalisation.
    pub fn suggest(&self, word: &str, n: usize) -> Vec<String> {
        let k = key(word);
        if k.is_empty() || k.chars().count() > MAX_WORD {
            return Vec::new();
        }
        let mut found: Vec<String> = Vec::new();
        let e1 = edits1(&k);
        let mut first: Vec<(u8, &String)> = e1.iter().filter(|(_, c)| self.words.contains(c)).map(|(r, c)| (*r, c)).collect();
        first.sort_by_key(|(r, c)| (!self.common.contains(*c), *r, c.len().abs_diff(k.len()), (*c).clone()));
        for (_, c) in first {
            if !found.contains(c) {
                found.push(c.clone());
            }
        }
        if found.len() < n {
            let mut second: Vec<String> = Vec::new();
            for (_, c) in e1.iter().take(2_000) {
                for (_, d) in edits1(c) {
                    if self.words.contains(&d) && !found.contains(&d) && !second.contains(&d) && d != k {
                        second.push(d);
                    }
                }
                if second.len() > 64 {
                    break;
                }
            }
            second.sort_by_key(|w| (!self.common.contains(w), w.len().abs_diff(k.len()), w.clone()));
            found.extend(second);
        }
        found.truncate(n);
        found.into_iter().map(|s| recase(word, &s)).collect()
    }
}

/// Every string one edit from `w`, ranked: transposition 0, replacement 1, deletion or
/// insertion 2 (the usual typing slips first).
fn edits1(w: &str) -> Vec<(u8, String)> {
    const ALPHA: &str = "abcdefghijklmnopqrstuvwxyz'";
    let cs: Vec<char> = w.chars().collect();
    let mut out = Vec::new();
    let s = |v: &[char]| v.iter().collect::<String>();
    for i in 0..=cs.len() {
        let (l, r) = cs.split_at(i);
        if let Some((_, rest)) = r.split_first() {
            out.push((2, s(&[l, rest].concat())));
        }
        if r.len() > 1 {
            let mut v = cs.clone();
            v.swap(i, i + 1);
            out.push((0, s(&v)));
        }
        for a in ALPHA.chars() {
            if let Some((_, rest)) = r.split_first() {
                out.push((1, s(&[l, &[a], rest].concat())));
            }
            out.push((2, s(&[l, &[a], r].concat())));
        }
    }
    out
}

/// `s` in the capitalisation pattern of `like` (ALL CAPS, Capitalised or lowercase).
fn recase(like: &str, s: &str) -> String {
    let letters: Vec<char> = like.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        s.to_uppercase()
    } else if letters.first().is_some_and(|c| c.is_uppercase()) {
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_and_suggests() {
        let sp = english();
        for w in ["the", "Weather", "STATION", "don't", "Nell's", "colour", "color", "a", "I"] {
            assert!(sp.check(w), "{w}");
        }
        for w in ["teh", "recieve", "barometr"] {
            assert!(!sp.check(w), "{w}");
        }
        assert_eq!(sp.suggest("teh", 3).first().map(String::as_str), Some("the"));
        assert!(sp.suggest("recieve", 5).contains(&"receive".to_string()));
        assert!(sp.suggest("Barometr", 5).contains(&"Barometer".to_string()));
        assert!(sp.suggest("STATOIN", 5).contains(&"STATION".to_string()));
        assert!(sp.suggest("", 5).is_empty());
    }

    #[test]
    fn tokens_and_misspellings() {
        assert_eq!(words("Don't stop—it's 9pm, Nell’s café!"), vec![(0, 5), (6, 10), (13, 17), (23, 31), (32, 37)]);
        let sp = english();
        let t = "Teh MARGUERITE's mast is gone, sez Nell.";
        let bad: Vec<&str> = sp.misspelled(t, &|w| w == "nell" || w == "marguerite").iter().map(|&(a, b)| &t[a..b]).collect();
        assert_eq!(bad, vec!["Teh", "sez"]);
        assert!(words(&"x".repeat(1000)).len() == 1);
    }
}
