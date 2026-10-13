//! Production locks: locked scene numbers (inserted scenes become 12A, 12B…), omitted scenes,
//! and the labels used for A/B pages.

use std::collections::HashSet;

use crate::{Element, Script};

/// Longest page or scene label kept.
const MAX_LABEL: usize = 12;

/// The label after `label` for something inserted after it: `12` → `12A`, `12A` → `12B`,
/// `12Z` → `12ZA`, `A1` → `B1` (labels before the first, which carry a letter prefix).
pub fn next_label(label: &str) -> String {
    let l: String = label.trim().chars().filter(|c| c.is_ascii_alphanumeric()).take(MAX_LABEL).collect::<String>().to_uppercase();
    if l.is_empty() {
        return "A".into();
    }
    let prefix: String = l.chars().take_while(char::is_ascii_alphabetic).collect();
    let rest = l.get(prefix.len()..).unwrap_or("");
    // `A1`: letters before a number.
    if !prefix.is_empty() && !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
        return format!("{}{rest}", bump(&prefix));
    }
    if l.ends_with(|c: char| c.is_ascii_alphabetic()) {
        let digits: String = l.chars().take_while(char::is_ascii_digit).collect();
        let letters = l.get(digits.len()..).unwrap_or("");
        return format!("{digits}{}", bump(letters));
    }
    format!("{l}A")
}

/// `A` → `B`, `Z` → `ZA`, `AZ` → `AZA` (stays after the original in reading order).
fn bump(letters: &str) -> String {
    let mut v: Vec<char> = letters.chars().collect();
    match v.last_mut() {
        Some(c) if *c < 'Z' => *c = char::from(*c as u8 + 1),
        _ => v.push('A'),
    }
    v.into_iter().collect()
}

/// The page labels a printed label stands for: `12` → `["12"]`, `12-14` → `["12", "13", "14"]`
/// (a combined page counts for every page in its range).
pub fn expand_page_label(label: &str) -> Vec<String> {
    let label = label.trim();
    if let Some((a, b)) = label.split_once('-')
        && let (Ok(x), Ok(y)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>())
        && x <= y
        && y - x < 1000
    {
        return (x..=y).map(|n| n.to_string()).collect();
    }
    if label.is_empty() { Vec::new() } else { vec![label.chars().take(MAX_LABEL).collect()] }
}

impl Script {
    /// Keep production state consistent: locks only while enabled, numbers for new scenes when
    /// scene numbers are locked.
    pub(crate) fn normalize_production(&mut self) {
        if !self.settings.pages_locked {
            for p in &mut self.paras {
                p.page_lock.clear();
            }
        } else {
            for p in &mut self.paras {
                if p.page_lock.len() > 1000 {
                    p.page_lock.truncate(1000);
                }
                for l in &mut p.page_lock {
                    if l.len() > MAX_LABEL {
                        *l = l.chars().take(MAX_LABEL).collect();
                    }
                }
            }
        }
        if self.settings.scene_numbers_locked {
            self.fill_locked_numbers();
        }
    }

    /// Number every unnumbered scene after the scene before it (`12` → `12A`), or before the
    /// first numbered scene (`A1`). With no numbers at all, scenes are numbered 1, 2, 3…
    pub fn fill_locked_numbers(&mut self) {
        let heads: Vec<usize> = self.paras.iter().enumerate().filter(|(_, p)| p.element == Element::SceneHeading).map(|(i, _)| i).collect();
        let numbered = |s: &Script, i: &usize| s.paras.get(*i).and_then(|p| p.scene_number.as_ref()).is_some_and(|n| !n.trim().is_empty());
        if !heads.iter().any(|i| numbered(self, i)) {
            self.number_scenes();
            return;
        }
        let mut used: HashSet<String> =
            heads.iter().filter_map(|i| self.paras.get(*i)?.scene_number.as_ref()).map(|n| n.trim().to_uppercase()).collect();
        let first = heads.iter().find_map(|i| self.paras.get(*i)?.scene_number.clone()).unwrap_or_else(|| "1".into());
        let mut prev: Option<String> = None;
        for i in heads {
            let Some(p) = self.paras.get_mut(i) else { continue };
            if let Some(n) = p.scene_number.as_ref().filter(|n| !n.trim().is_empty()) {
                prev = Some(n.clone());
                continue;
            }
            let mut cand = match &prev {
                Some(pv) => next_label(pv),
                None => format!("A{}", first.trim().to_uppercase()),
            };
            // Taken by a later scene: go one level deeper (12B is taken → 12AA after 12A).
            if used.contains(&cand)
                && let Some(pv) = &prev
            {
                cand = format!("{}A", pv.trim().to_uppercase());
            }
            let mut guard = 0;
            while used.contains(&cand) && guard < 64 {
                cand = next_label(&cand);
                guard += 1;
            }
            used.insert(cand.clone());
            p.scene_number = Some(cand.clone());
            prev = Some(cand);
        }
    }

    /// Omit scene `index` (or restore it). Returns the heading's paragraph.
    pub fn set_omitted(&mut self, index: usize, omitted: bool) -> Result<usize, crate::DocError> {
        let para = self.scenes().get(index).map(|s| s.para).ok_or(crate::DocError::NoScene(index))?;
        self.para_mut(para)?.omitted = omitted;
        Ok(para)
    }

    /// Whether paragraph `i` is hidden inside an omitted scene (the heading itself is not).
    pub fn hidden(&self, i: usize) -> bool {
        let Some(p) = self.paras.get(i) else { return false };
        if p.element == Element::SceneHeading {
            return false;
        }
        self.paras.iter().take(i).rev().find(|q| q.element == Element::SceneHeading).is_some_and(|h| h.omitted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Para;

    #[test]
    fn labels() {
        assert_eq!(next_label("12"), "12A");
        assert_eq!(next_label("12A"), "12B");
        assert_eq!(next_label("12Z"), "12ZA");
        assert_eq!(next_label("A1"), "B1");
        assert_eq!(next_label(""), "A");
        assert_eq!(expand_page_label("12-14"), vec!["12", "13", "14"]);
        assert_eq!(expand_page_label("12A"), vec!["12A"]);
        assert!(expand_page_label(" ").is_empty());
    }

    fn heads(s: &Script) -> Vec<Option<String>> {
        s.paras.iter().filter(|p| p.element == Element::SceneHeading).map(|p| p.scene_number.clone()).collect()
    }

    #[test]
    fn locked_numbers() {
        let h = |t: &str| Para::new(Element::SceneHeading, t);
        let mut s = Script::from_paras(vec![h("INT. A"), h("INT. B"), h("INT. C")]).unwrap_or_default();
        s.settings.scene_numbers_locked = true;
        let _ = s.normalize();
        assert_eq!(heads(&s), vec![Some("1".into()), Some("2".into()), Some("3".into())]);
        // Insert scenes after 1, before 1 and after 3.
        s.paras.insert(1, h("INT. NEW"));
        s.paras.insert(2, h("INT. NEWER"));
        s.paras.insert(0, h("INT. FIRST"));
        s.paras.push(h("INT. LAST"));
        let _ = s.normalize();
        assert_eq!(heads(&s), ["A1", "1", "1A", "1B", "2", "3", "3A"].iter().map(|x| Some(x.to_string())).collect::<Vec<_>>());
        // Between 1A and an existing 1B.
        s.paras.insert(3, h("INT. SQUEEZED"));
        let _ = s.normalize();
        assert_eq!(heads(&s).get(3).cloned().flatten().as_deref(), Some("1AA"));
    }

    #[test]
    fn hidden_in_omitted_scene() {
        let mut s = Script::from_paras(vec![
            Para::new(Element::SceneHeading, "INT. A"),
            Para::new(Element::Action, "x"),
            Para::new(Element::SceneHeading, "INT. B"),
            Para::new(Element::Action, "y"),
        ])
        .unwrap_or_default();
        assert_eq!(s.set_omitted(0, true), Ok(0));
        assert!(!s.hidden(0));
        assert!(s.hidden(1));
        assert!(!s.hidden(3));
        assert!(s.set_omitted(9, true).is_err());
        // Only headings keep the flag.
        s.paras[1].omitted = true;
        let _ = s.normalize();
        assert!(!s.paras[1].omitted);
    }
}
