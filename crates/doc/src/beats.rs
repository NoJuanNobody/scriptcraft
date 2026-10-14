//! The beat board and bookmarks.
//!
//! The beat board is a free canvas of beat cards (title, text, colour, position and size) kept
//! with the script, for ideas before they become scenes. A beat can be linked to a scene: the
//! link lives on the scene's heading paragraph ([`Para::beats`]), so it follows the scene through
//! moves, edits and undo. Bookmarks are named marks on paragraphs ([`Para::bookmark`]).

use serde::{Deserialize, Serialize};

use crate::{CardColor, Element, Para, Script, sanitize};

/// Most beats a board holds.
pub const MAX_BEATS: usize = 2000;
/// Longest beat title, in characters.
pub const MAX_BEAT_TITLE: usize = 200;
/// Longest beat text, in characters.
pub const MAX_BEAT_TEXT: usize = 4000;
/// Longest bookmark name, in characters.
pub const MAX_BOOKMARK_CHARS: usize = 60;
/// A new beat card's size (points).
pub const BEAT_W: f32 = 240.0;
pub const BEAT_H: f32 = 120.0;
/// Smallest and largest beat card sides (points).
pub const BEAT_MIN: f32 = 60.0;
pub const BEAT_MAX: f32 = 2000.0;
/// The board extends this far from the origin in each direction (points).
pub const BOARD_EXTENT: f32 = 100_000.0;

/// One card on the beat board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Beat {
    /// Stable within the script; what commands and scene links refer to.
    pub id: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// The body (line breaks kept).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<CardColor>,
    /// Top-left corner and size on the board, in points.
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_w")]
    pub w: f32,
    #[serde(default = "default_h")]
    pub h: f32,
    /// The id in an FDX file (`ListItem Id`), kept so a round trip keeps it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
}

fn default_w() -> f32 {
    BEAT_W
}
fn default_h() -> f32 {
    BEAT_H
}

/// A finite value clamped to `lo..=hi` (`d` when not finite).
pub fn clamp_f(v: f32, lo: f32, hi: f32, d: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

/// A beat title: one line, trimmed, capped.
pub fn clean_beat_title(s: &str) -> String {
    sanitize(s).trim().chars().take(MAX_BEAT_TITLE).collect()
}

/// A beat body: line breaks kept, other control characters dropped, capped.
pub fn clean_beat_text(s: &str) -> String {
    let t: String = s.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || !c.is_control()).take(MAX_BEAT_TEXT).collect();
    t.trim_end().to_string()
}

/// A bookmark name: one line, trimmed, capped; `None` when empty.
pub fn clean_bookmark(s: &str) -> Option<String> {
    let t: String = sanitize(s).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_BOOKMARK_CHARS).collect();
    (!t.is_empty()).then_some(t)
}

impl Beat {
    pub fn new(id: u32, title: &str, text: &str, color: Option<CardColor>, x: f32, y: f32) -> Self {
        let mut b = Beat { id, title: clean_beat_title(title), text: clean_beat_text(text), color, x, y, w: BEAT_W, h: BEAT_H, key: String::new() };
        b.clean();
        b
    }

    /// Clamp the geometry, clean the text and give it a key.
    fn clean(&mut self) {
        self.x = clamp_f(self.x, -BOARD_EXTENT, BOARD_EXTENT, 0.0);
        self.y = clamp_f(self.y, -BOARD_EXTENT, BOARD_EXTENT, 0.0);
        self.w = clamp_f(self.w, BEAT_MIN, BEAT_MAX, BEAT_W);
        self.h = clamp_f(self.h, BEAT_MIN, BEAT_MAX, BEAT_H);
        if self.title.chars().count() > MAX_BEAT_TITLE || self.title.chars().any(char::is_control) {
            self.title = clean_beat_title(&self.title);
        }
        if self.text.chars().count() > MAX_BEAT_TEXT || self.text.chars().any(|c| c.is_control() && c != '\n') {
            self.text = clean_beat_text(&self.text);
        }
        let ok_key = !self.key.is_empty() && self.key.len() <= 64 && self.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !ok_key {
            self.key = beat_key(self.id);
        }
    }

    /// The card's one-line label: the title, else the first line of the text.
    pub fn label(&self) -> &str {
        if self.title.is_empty() { self.text.lines().next().unwrap_or("") } else { &self.title }
    }
}

/// A UUID-shaped key for a beat that has none (FDX `ListItem` ids are UUIDs).
pub fn beat_key(id: u32) -> String {
    format!("5c0bea70-0000-4000-8000-{:012x}", u64::from(id))
}

/// The beat board: the cards, in drawing order (later ones on top).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatBoard {
    #[serde(default)]
    pub beats: Vec<Beat>,
}

impl BeatBoard {
    pub fn is_empty(&self) -> bool {
        self.beats.is_empty()
    }
    pub fn get(&self, id: u32) -> Option<&Beat> {
        self.beats.iter().find(|b| b.id == id)
    }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Beat> {
        self.beats.iter_mut().find(|b| b.id == id)
    }
    /// An id no beat uses.
    pub fn next_id(&self) -> u32 {
        self.beats.iter().map(|b| b.id).max().map_or(1, |m| m.saturating_add(1))
    }
    /// Add a beat; `None` when the board is full.
    pub fn add(&mut self, title: &str, text: &str, color: Option<CardColor>, x: f32, y: f32) -> Option<u32> {
        if self.beats.len() >= MAX_BEATS {
            return None;
        }
        let id = self.next_id();
        if id == u32::MAX {
            return None;
        }
        self.beats.push(Beat::new(id, title, text, color, x, y));
        Some(id)
    }
    /// Remove a beat; `false` if there was none.
    pub fn remove(&mut self, id: u32) -> bool {
        let n = self.beats.len();
        self.beats.retain(|b| b.id != id);
        self.beats.len() != n
    }
    /// Put a beat on top of the others.
    pub fn raise(&mut self, id: u32) {
        if let Some(i) = self.beats.iter().position(|b| b.id == id) {
            let b = self.beats.remove(i);
            self.beats.push(b);
        }
    }
    /// A free spot for a new card: right of the rightmost card on the top row, or the origin.
    pub fn free_spot(&self) -> (f32, f32) {
        let n = self.beats.len();
        let col = (n % 5) as f32;
        let row = (n / 5) as f32;
        let (x, y) = (40.0 + col * (BEAT_W + 40.0), 40.0 + row * (BEAT_H + 40.0));
        if self.beats.iter().any(|b| (b.x - x).abs() < 1.0 && (b.y - y).abs() < 1.0) { (x + 24.0, y + 24.0) } else { (x, y) }
    }
    /// The bounding box of every card `(x0, y0, x1, y1)`, or `None` on an empty board.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        self.beats.iter().fold(None, |acc, b| {
            let r = (b.x, b.y, b.x + b.w, b.y + b.h);
            Some(match acc {
                None => r,
                Some((a, c, d, e)) => (a.min(r.0), c.min(r.1), d.max(r.2), e.max(r.3)),
            })
        })
    }
    /// Unique ids and keys, clean text and geometry, at most [`MAX_BEATS`].
    pub fn normalize(&mut self) {
        self.beats.truncate(MAX_BEATS);
        let mut seen_ids = std::collections::HashSet::new();
        let mut next = self.next_id();
        for b in &mut self.beats {
            if b.id == 0 || b.id == u32::MAX || !seen_ids.insert(b.id) {
                b.id = next;
                next = next.saturating_add(1);
                seen_ids.insert(b.id);
                b.key.clear();
            }
            b.clean();
        }
        let mut seen_keys = std::collections::HashSet::new();
        for b in &mut self.beats {
            if !seen_keys.insert(b.key.to_ascii_lowercase()) {
                b.key = beat_key(b.id);
                seen_keys.insert(b.key.clone());
            }
        }
    }
}

impl Script {
    /// Every bookmark in script order: (paragraph, name).
    pub fn bookmarks(&self) -> Vec<(usize, String)> {
        self.paras.iter().enumerate().filter_map(|(i, p)| p.bookmark.clone().map(|b| (i, b))).collect()
    }

    /// The paragraph a bookmark is on (names match without regard to case).
    pub fn bookmark_para(&self, name: &str) -> Option<usize> {
        let name = clean_bookmark(name)?;
        self.paras.iter().position(|p| p.bookmark.as_deref().is_some_and(|b| b.eq_ignore_ascii_case(&name)))
    }

    /// Set (or with `None` clear) the bookmark on a paragraph. A name another paragraph uses
    /// moves here. Returns the name set.
    pub fn set_bookmark(&mut self, para: usize, name: Option<&str>) -> Result<Option<String>, crate::DocError> {
        if para >= self.paras.len() {
            return Err(crate::DocError::NoPara(para));
        }
        let name = name.and_then(clean_bookmark);
        if let Some(n) = &name {
            for p in &mut self.paras {
                if p.bookmark.as_deref().is_some_and(|b| b.eq_ignore_ascii_case(n)) {
                    p.bookmark = None;
                }
            }
        }
        if let Some(p) = self.paras.get_mut(para) {
            p.bookmark = name.clone();
        }
        Ok(name)
    }

    /// The heading paragraph a beat is linked to.
    pub fn beat_para(&self, id: u32) -> Option<usize> {
        self.paras.iter().position(|p| p.element == Element::SceneHeading && p.beats.contains(&id))
    }

    /// The scene (index) a beat is linked to.
    pub fn beat_scene(&self, id: u32) -> Option<usize> {
        let para = self.beat_para(id)?;
        self.scenes().iter().position(|s| s.para == para)
    }

    /// Link a beat to a scene (`None` unlinks it). A beat links to one scene at most.
    pub fn link_beat(&mut self, id: u32, scene: Option<usize>) -> Result<(), crate::DocError> {
        let head = match scene {
            Some(i) => Some(self.scenes().get(i).map(|s| s.para).ok_or(crate::DocError::NoScene(i))?),
            None => None,
        };
        for p in &mut self.paras {
            p.beats.retain(|b| *b != id);
        }
        if let Some(h) = head
            && let Some(p) = self.paras.get_mut(h)
        {
            p.beats.push(id);
        }
        Ok(())
    }

    /// After the paragraphs `a+1..=b` were joined into `a`: `a` keeps the first bookmark and
    /// scene links among them.
    pub(crate) fn inherit_anchors(&mut self, a: usize, from: &[Para]) {
        let Some(p) = self.paras.get_mut(a) else { return };
        for q in from {
            if p.bookmark.is_none() {
                p.bookmark = q.bookmark.clone();
            }
            p.beats.extend(q.beats.iter().copied());
        }
    }

    /// Bookmarks unique and clean; beat links only on headings, to beats that exist, once each.
    pub(crate) fn normalize_boards(&mut self) {
        self.board.normalize();
        let ids: std::collections::HashSet<u32> = self.board.beats.iter().map(|b| b.id).collect();
        let mut linked = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for p in &mut self.paras {
            if let Some(b) = p.bookmark.take() {
                p.bookmark = clean_bookmark(&b).filter(|n| names.insert(n.to_lowercase()));
            }
            if p.element != Element::SceneHeading {
                p.beats.clear();
            } else if !p.beats.is_empty() {
                p.beats.retain(|id| ids.contains(id) && linked.insert(*id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pos;

    fn script() -> Script {
        Script::from_paras(vec![
            Para::new(Element::SceneHeading, "INT. A - DAY"),
            Para::new(Element::Action, "One."),
            Para::new(Element::SceneHeading, "INT. B - DAY"),
            Para::new(Element::Action, "Two."),
            Para::new(Element::SceneHeading, "INT. C - DAY"),
        ])
        .unwrap_or_default()
    }

    #[test]
    fn bookmarks_follow_their_paragraph() {
        let mut s = script();
        assert_eq!(s.set_bookmark(3, Some("  The  turn ")).ok().flatten().as_deref(), Some("The turn"));
        // Inserting paragraphs above moves the bookmark with its text.
        s.paras.insert(0, Para::new(Element::Action, "Cold open."));
        assert_eq!(s.bookmark_para("the turn"), Some(4));
        // The same name elsewhere moves it.
        s.set_bookmark(1, Some("THE TURN")).ok();
        assert_eq!(s.bookmarks(), vec![(1, "THE TURN".to_string())]);
        // Joining a bookmarked paragraph into the one before keeps it.
        s.set_bookmark(1, None).ok();
        s.set_bookmark(2, Some("one")).ok();
        let end1 = s.paras[1].len();
        s.delete_range(Pos::new(1, end1), Pos::new(2, 0));
        assert_eq!(s.bookmark_para("one"), Some(1));
        // Duplicates and junk are cleaned.
        s.paras[3].bookmark = Some("One".into());
        s.paras[4].bookmark = Some("\u{7}  ".into());
        s.normalize().ok();
        assert_eq!(s.bookmarks().len(), 1);
        assert!(s.set_bookmark(99, Some("x")).is_err());
    }

    #[test]
    fn beats_link_to_scenes_and_survive_moves() {
        let mut s = script();
        let a = s.board.add("Inciting incident", "She finds the key.\nIt opens nothing.", Some(CardColor::Blue), 10.0, 20.0).unwrap_or(0);
        let b = s.board.add("", "Midpoint", None, f32::NAN, 1e12).unwrap_or(0);
        assert_ne!(a, b);
        assert_eq!(s.board.get(b).map(|x| (x.x, x.y)), Some((0.0, BOARD_EXTENT)));
        assert_eq!(s.board.get(b).map(Beat::label), Some("Midpoint"));
        s.link_beat(a, Some(1)).ok();
        assert_eq!(s.beat_scene(a), Some(1));
        s.move_scene(1, 0).ok();
        assert_eq!(s.beat_scene(a), Some(0));
        // Relinking moves the link; a heading that becomes Action drops it.
        s.link_beat(a, Some(2)).ok();
        assert_eq!(s.beat_scene(a), Some(2));
        assert!(s.link_beat(a, Some(9)).is_err());
        let h = s.beat_para(a).unwrap_or(0);
        s.paras[h].element = Element::Action;
        s.normalize().ok();
        assert_eq!(s.beat_scene(a), None);
        // Links to missing beats, and second links to one beat, are dropped.
        s.paras[0].beats = vec![77, a, a];
        s.paras[2].beats = vec![a];
        s.normalize().ok();
        assert_eq!(s.paras[0].beats, vec![a]);
        assert!(s.paras[2].beats.is_empty());
        assert!(s.board.remove(a));
        s.normalize().ok();
        assert!(s.paras[0].beats.is_empty());
    }

    #[test]
    fn hostile_boards_are_cleaned() {
        let mut s = script();
        s.board.beats = vec![
            Beat {
                id: 0,
                title: "a\nb".into(),
                text: "x\u{1}".repeat(5000),
                color: None,
                x: f32::INFINITY,
                y: -1e9,
                w: 0.0,
                h: 1e9,
                key: "<bad>".into(),
            },
            Beat { id: 5, title: "t".into(), text: String::new(), color: None, x: 0.0, y: 0.0, w: 100.0, h: 100.0, key: "same".into() },
            Beat { id: 5, title: "u".into(), text: String::new(), color: None, x: 0.0, y: 0.0, w: 100.0, h: 100.0, key: "SAME".into() },
        ];
        s.normalize().ok();
        let ids: std::collections::HashSet<u32> = s.board.beats.iter().map(|b| b.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&0));
        let b0 = &s.board.beats[0];
        assert_eq!((b0.x, b0.y, b0.w, b0.h), (0.0, -BOARD_EXTENT, BEAT_MIN, BEAT_MAX));
        assert_eq!(b0.title, "a b");
        assert!(b0.text.chars().count() <= MAX_BEAT_TEXT && !b0.text.contains('\u{1}'));
        assert!(b0.key.starts_with("5c0bea70"));
        let keys: std::collections::HashSet<String> = s.board.beats.iter().map(|b| b.key.to_lowercase()).collect();
        assert_eq!(keys.len(), 3);
    }
}
