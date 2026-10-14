//! Revisions: coloured revision sets and the marks that show which text changed.
//!
//! A script has a list of revision sets in the industry order (White, Blue, Pink, Yellow,
//! Green, Goldenrod, Buff, Salmon, Cherry), one of them active. While revision mode is on,
//! every edit marks the text it changed with the active set ([`mark_changes`], run by the
//! engine after each command). Marks live on the paragraph as byte ranges ([`RevMark`]); a
//! later set wins where marks overlap. Pagination prints the set's mark (`*`) in the right
//! margin of every changed line and labels revised pages with the set's name and date.

use serde::{Deserialize, Serialize};

use crate::{Para, Script, next_boundary, prev_boundary};

/// Most revision sets a script keeps.
pub const MAX_SETS: usize = 20;

/// One revision set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RevisionSet {
    pub name: String,
    /// Page colour, sRGB.
    pub color: [u8; 3],
    /// Shown in page headers (`{revdate}`), free text such as `10/12/26`.
    pub date: String,
    /// The margin mark for changed lines.
    pub mark: String,
}

impl Default for RevisionSet {
    fn default() -> Self {
        RevisionSet { name: "Revision".into(), color: [0xff, 0xff, 0xff], date: String::new(), mark: "*".into() }
    }
}

impl RevisionSet {
    pub fn new(name: &str, color: [u8; 3]) -> Self {
        RevisionSet { name: name.into(), color, ..Default::default() }
    }
    /// The page label: `Blue Rev. 10/12/26` (no date: `Blue Rev.`).
    pub fn label(&self) -> String {
        let d = self.date.trim();
        if d.is_empty() { format!("{} Rev.", self.name.trim()) } else { format!("{} Rev. {d}", self.name.trim()) }
    }
    /// `#rrggbb`.
    pub fn hex(&self) -> String {
        let [r, g, b] = self.color;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
    fn clean(&mut self) {
        // Spaces are kept while typing; labels trim them.
        let one = |s: &str, n: usize| -> String { s.chars().filter(|c| !c.is_control()).take(n).collect() };
        self.name = one(&self.name, 40);
        if self.name.trim().is_empty() {
            self.name = "Revision".into();
        }
        self.date = one(&self.date, 40);
        self.mark = one(self.mark.trim(), 2);
        if self.mark.is_empty() {
            self.mark = "*".into();
        }
    }
}

/// `#rrggbb` (or `rrggbb`, or Final Draft's 48-bit `#rrrrggggbbbb`) to sRGB.
pub fn parse_color(c: &str) -> Option<[u8; 3]> {
    let h = c.trim().trim_start_matches('#');
    let step = match h.len() {
        6 => 2,
        12 => 4,
        _ => return None,
    };
    let ch = |i: usize| h.get(i * step..i * step + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    Some([ch(0)?, ch(1)?, ch(2)?])
}

/// The industry order of revision colours.
pub fn standard_sets() -> Vec<RevisionSet> {
    [
        ("White", [0xff, 0xff, 0xff]),
        ("Blue", [0xcf, 0xe2, 0xf7]),
        ("Pink", [0xf8, 0xd3, 0xdc]),
        ("Yellow", [0xfb, 0xf3, 0xb0]),
        ("Green", [0xd4, 0xee, 0xcf]),
        ("Goldenrod", [0xf3, 0xd9, 0x8b]),
        ("Buff", [0xf2, 0xe4, 0xc4]),
        ("Salmon", [0xf8, 0xc4, 0xb0]),
        ("Cherry", [0xf2, 0xa5, 0xb3]),
    ]
    .into_iter()
    .map(|(n, c)| RevisionSet::new(n, c))
    .collect()
}

/// Revision settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Revisions {
    /// Revision mode: edits are marked with the active set.
    pub tracking: bool,
    /// Index of the active set.
    pub active: usize,
    pub sets: Vec<RevisionSet>,
    /// Print and show the margin marks.
    pub show_marks: bool,
    /// Tint revised pages with their set's colour (PDF and page view).
    pub color_pages: bool,
}

impl Default for Revisions {
    fn default() -> Self {
        Revisions { tracking: false, active: 1, sets: standard_sets(), show_marks: true, color_pages: false }
    }
}

impl Revisions {
    pub fn is_default(&self) -> bool {
        *self == Revisions::default()
    }
    /// The active set's level (index), clamped.
    pub fn level(&self) -> u8 {
        self.active.min(self.sets.len().saturating_sub(1)).min(usize::from(u8::MAX)) as u8
    }
    /// The set at `level`.
    pub fn set(&self, level: u8) -> Option<&RevisionSet> {
        self.sets.get(usize::from(level))
    }
    /// The set named `name` (any case) or numbered `name` (its index).
    pub fn find(&self, name: &str) -> Option<usize> {
        let n = name.trim();
        n.parse::<usize>().ok().filter(|i| *i < self.sets.len()).or_else(|| self.sets.iter().position(|s| s.name.eq_ignore_ascii_case(n)))
    }
    /// Keep the invariants: 1..=MAX_SETS clean sets, active in range.
    pub fn normalize(&mut self) {
        if self.sets.is_empty() {
            self.sets = standard_sets();
        }
        self.sets.truncate(MAX_SETS);
        for s in &mut self.sets {
            s.clean();
        }
        self.active = self.active.min(self.sets.len().saturating_sub(1));
    }
}

/// Changed text: a byte range of the paragraph and the revision set (level) that changed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevMark {
    pub start: usize,
    pub end: usize,
    pub level: u8,
}

/// Marks made valid for `text`: in range, on char boundaries, non-empty, sorted, adjacent marks
/// of one level joined.
pub fn fix_marks(text: &str, marks: &[RevMark]) -> Vec<RevMark> {
    let mut out: Vec<RevMark> = Vec::new();
    let mut v: Vec<RevMark> = marks
        .iter()
        .map(|m| {
            let mut a = m.start.min(text.len());
            while a > 0 && !text.is_char_boundary(a) {
                a -= 1;
            }
            let mut b = m.end.min(text.len()).max(a);
            while b < text.len() && !text.is_char_boundary(b) {
                b += 1;
            }
            RevMark { start: a, end: b, level: m.level }
        })
        .filter(|m| m.end > m.start)
        .collect();
    v.sort_by_key(|m| (m.start, m.end));
    for mut m in v {
        match out.last_mut() {
            Some(l) if l.level == m.level && m.start <= l.end => l.end = l.end.max(m.end),
            Some(l) if m.start < l.end => {
                // Overlapping marks of different sets (only from files): keep the first.
                m.start = l.end;
                if m.end > m.start {
                    out.push(m);
                }
            }
            _ => out.push(m),
        }
    }
    out
}

/// Add a mark over `start..end` (taking that range from any other mark).
pub fn add_mark(p: &mut Para, start: usize, end: usize, level: u8) {
    let (start, end) = (start.min(p.text.len()), end.min(p.text.len()));
    if end <= start {
        return;
    }
    let mut out = Vec::with_capacity(p.revs.len() + 2);
    for m in &p.revs {
        if m.end <= start || m.start >= end {
            out.push(*m);
            continue;
        }
        if m.start < start {
            out.push(RevMark { start: m.start, end: start, level: m.level });
        }
        if m.end > end {
            out.push(RevMark { start: end, end: m.end, level: m.level });
        }
    }
    out.push(RevMark { start, end, level });
    p.revs = fix_marks(&p.text, &out);
}

/// Mark the character at (or, at the end, before) `off`: where text was deleted.
fn mark_point(p: &mut Para, off: usize, level: Option<u8>) {
    let Some(level) = level.filter(|_| !p.text.is_empty()) else { return };
    let a = if off >= p.text.len() { prev_boundary(&p.text, p.text.len()) } else { off };
    let b = next_boundary(&p.text, a);
    add_mark(p, a, b, level);
}

fn mark_all(p: &mut Para, level: Option<u8>) {
    let Some(level) = level else { return };
    let n = p.text.len();
    add_mark(p, 0, n, level);
}

/// Same printed content (text, element, styles, dual).
fn same(a: &Para, b: &Para) -> bool {
    a.element == b.element && a.text == b.text && a.spans == b.spans && a.dual == b.dual
}

/// Common prefix and suffix lengths in bytes, on char boundaries, not overlapping.
fn common(a: &str, b: &str) -> (usize, usize) {
    let mut p = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    while p > 0 && !(a.is_char_boundary(p) && b.is_char_boundary(p)) {
        p -= 1;
    }
    let max = a.len().min(b.len()) - p;
    let mut s = a.bytes().rev().zip(b.bytes().rev()).take(max).take_while(|(x, y)| x == y).count();
    while s > 0 && !(a.is_char_boundary(a.len() - s) && b.is_char_boundary(b.len() - s)) {
        s -= 1;
    }
    (p, s)
}

/// Mark what changed between an old paragraph and its edited version (marks of `new` are the
/// old ones, still at old offsets, when it was edited in place).
fn mark_para(old: &Para, new: &mut Para, level: Option<u8>) {
    if old.text == new.text {
        if old.element != new.element || old.dual != new.dual {
            mark_all(new, level);
            return;
        }
        // Only styles changed: mark the restyled characters.
        let (a, b) = (old.styles(), new.styles());
        let first = a.iter().zip(&b).position(|(x, y)| x != y);
        let last = a.iter().zip(&b).rposition(|(x, y)| x != y);
        if let (Some(f), Some(l), Some(level)) = (first, last, level) {
            let end = next_boundary(&new.text, l);
            add_mark(new, f, end, level);
        }
        return;
    }
    let (p, s) = common(&old.text, &new.text);
    let (oe, ne) = (old.text.len() - s, new.text.len() - s);
    let mut shifted = Vec::with_capacity(new.revs.len() + 1);
    for m in &new.revs {
        if m.end <= p {
            shifted.push(*m);
        } else if m.start >= oe {
            shifted.push(RevMark { start: m.start - oe + ne, end: m.end - oe + ne, level: m.level });
        } else {
            if m.start < p {
                shifted.push(RevMark { start: m.start, end: p, level: m.level });
            }
            if m.end > oe {
                shifted.push(RevMark { start: ne, end: m.end - oe + ne, level: m.level });
            }
        }
    }
    new.revs = fix_marks(&new.text, &shifted);
    if old.element != new.element {
        mark_all(new, level);
    } else if ne > p {
        if let Some(level) = level {
            add_mark(new, p, ne, level);
        }
    } else {
        mark_point(new, p, level);
    }
}

/// After an edit: keep existing marks on the text they marked and, in revision mode, mark
/// everything that differs between `old` and `new` with the active revision set.
/// Unchanged paragraphs at both ends are skipped; the changed middle is paired up in order,
/// extra new paragraphs are marked whole, and removed ones mark the place they were.
pub fn mark_changes(old: &Script, new: &mut Script) {
    let r = &new.settings.revision;
    let level = r.tracking.then(|| r.level());
    if level.is_none() && new.paras.iter().all(|p| p.revs.is_empty()) {
        return;
    }
    let (o, n) = (&old.paras, new.paras.len());
    let mut pre = 0;
    while let (Some(a), Some(b)) = (o.get(pre), new.paras.get(pre)) {
        if !same(a, b) {
            break;
        }
        pre += 1;
    }
    let mut suf = 0;
    while suf < o.len() - pre && suf < n - pre {
        match (o.get(o.len() - 1 - suf), new.paras.get(n - 1 - suf)) {
            (Some(a), Some(b)) if same(a, b) => suf += 1,
            _ => break,
        }
    }
    let (old_mid, new_mid) = (o.len() - suf - pre, n - suf - pre);
    for k in 0..new_mid {
        let Some(np) = new.paras.get_mut(pre + k) else { break };
        match o.get(pre + k).filter(|_| k < old_mid) {
            Some(op) => mark_para(op, np, level),
            None => mark_all(np, level),
        }
    }
    if old_mid > new_mid {
        let at = pre + new_mid;
        if at < n {
            if let Some(p) = new.paras.get_mut(at) {
                mark_point(p, 0, level);
            }
        } else if let Some(p) = at.checked_sub(1).and_then(|i| new.paras.get_mut(i)) {
            let end = p.text.len();
            mark_point(p, end, level);
        }
    }
}

/// Remove marks: of one level (`Some`) or all, in paragraphs `paras` (all when `None`).
pub fn clear_marks(s: &mut Script, level: Option<u8>, paras: Option<std::ops::RangeInclusive<usize>>) -> usize {
    let mut n = 0;
    for (i, p) in s.paras.iter_mut().enumerate() {
        if paras.as_ref().is_some_and(|r| !r.contains(&i)) {
            continue;
        }
        let before = p.revs.len();
        p.revs.retain(|m| level.is_some_and(|l| m.level != l));
        n += before - p.revs.len();
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, Pos};

    fn script(t: &[&str]) -> Script {
        let mut s = Script::from_paras(t.iter().map(|x| Para::new(Element::Action, *x)).collect()).unwrap();
        s.settings.revision.tracking = true;
        s
    }
    fn marked(p: &Para) -> Vec<(&str, u8)> {
        p.revs.iter().map(|m| (p.text.get(m.start..m.end).unwrap_or("?"), m.level)).collect()
    }

    #[test]
    fn typing_marks_the_new_text() {
        let old = script(&["The door opens.", "She waits."]);
        let mut s = old.clone();
        s.insert_text(Pos::new(0, 9), "slowly ").unwrap();
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[0]), vec![("slowly ", 1)]);
        assert!(s.paras[1].revs.is_empty());
        // More typing next to it joins the mark; a deletion marks where it was.
        let old = s.clone();
        s.insert_text(Pos::new(0, 16), "and ").unwrap();
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[0]), vec![("slowly and ", 1)]);
        let old = s.clone();
        s.delete_range(Pos::new(1, 0), Pos::new(1, 4));
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[1]), vec![("w", 1)]);
    }

    #[test]
    fn marks_shift_and_later_sets_win() {
        let mut s = script(&["abcdef"]);
        s.paras[0].revs = vec![RevMark { start: 4, end: 6, level: 1 }];
        let old = s.clone();
        s.settings.revision.active = 2;
        s.insert_text(Pos::new(0, 1), "XY").unwrap();
        mark_changes(&old, &mut s);
        assert_eq!(s.paras[0].text, "aXYbcdef");
        assert_eq!(marked(&s.paras[0]), vec![("XY", 2), ("ef", 1)]);
        // Retyping inside an older mark takes that part over.
        let old = s.clone();
        let _ = s.delete_range(Pos::new(0, 6), Pos::new(0, 7));
        s.insert_text(Pos::new(0, 6), "E").unwrap();
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[0]), vec![("XY", 2), ("E", 2), ("f", 1)]);
    }

    #[test]
    fn paragraphs_added_removed_and_restyled() {
        let old = script(&["One.", "Two.", "Three."]);
        let mut s = old.clone();
        s.paras.insert(1, Para::new(Element::Action, "New."));
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[1]), vec![("New.", 1)]);
        assert!(s.paras[0].revs.is_empty() && s.paras[2].revs.is_empty());
        let old = script(&["One.", "Two.", "Three."]);
        let mut s = old.clone();
        s.paras.remove(1);
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[1]), vec![("T", 1)]);
        let old = script(&["Plain words."]);
        let mut s = old.clone();
        s.set_flag(Pos::new(0, 6), Pos::new(0, 11), crate::Flag::Bold, true);
        mark_changes(&old, &mut s);
        assert_eq!(marked(&s.paras[0]), vec![("words", 1)]);
        let mut s2 = old.clone();
        s2.set_element(0, Element::General).unwrap();
        mark_changes(&old, &mut s2);
        assert_eq!(marked(&s2.paras[0]), vec![("Plain words.", 1)]);
    }

    #[test]
    fn unicode_and_hostile_marks() {
        let old = script(&["héllo wörld"]);
        let mut s = old.clone();
        s.insert_text(Pos::new(0, 3), "é").unwrap();
        mark_changes(&old, &mut s);
        for m in &s.paras[0].revs {
            assert!(s.paras[0].text.is_char_boundary(m.start) && s.paras[0].text.is_char_boundary(m.end));
        }
        let t = "héllo";
        let v = fix_marks(t, &[RevMark { start: 2, end: 99, level: 3 }, RevMark { start: 9, end: 1, level: 0 }]);
        assert_eq!(v, vec![RevMark { start: 1, end: 6, level: 3 }]);
        let mut r = Revisions { sets: Vec::new(), active: 99, ..Default::default() };
        r.normalize();
        assert_eq!(r.sets.len(), 9);
        assert_eq!(r.active, 8);
        assert_eq!(r.find("pink"), Some(2));
        assert_eq!(r.find("3"), Some(3));
        assert_eq!(r.sets[1].label(), "Blue Rev.");
        assert_eq!(parse_color("#FFFF0000FFFF"), Some([255, 0, 255]));
    }

    #[test]
    fn clearing() {
        let mut s = script(&["abc", "def"]);
        s.paras[0].revs = vec![RevMark { start: 0, end: 1, level: 1 }, RevMark { start: 1, end: 2, level: 2 }];
        s.paras[1].revs = vec![RevMark { start: 0, end: 3, level: 1 }];
        assert_eq!(clear_marks(&mut s, Some(1), Some(0..=0)), 1);
        assert_eq!(s.paras[0].revs.len(), 1);
        assert_eq!(clear_marks(&mut s, None, None), 2);
    }
}
