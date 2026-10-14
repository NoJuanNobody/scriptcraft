//! The beat board and bookmarks in FDX.
//!
//! Observed in files Final Draft wrote (structure only):
//! - Bookmarks are a `Bookmark="name"` attribute on a script `<Paragraph>`.
//! - Beats are `<ListItems><ListItem Id="uuid" Type="Beat" Title="…" Color="#RRRRGGGGBBBB">`
//!   with a `<Content>` of `<Paragraph><Text>` lines; their places on the board are
//!   `<DisplayBoards><DisplayBoard Type="Beat"><Item Id="uuid" Left Top Width Height/>`, the
//!   `Item` sharing the `ListItem`'s id.
//!
//! A beat's link to a scene has no form we have seen; it is a ScriptCraft-only
//! `ScriptCraftBeats="uuid,uuid"` attribute on the scene heading's `<Paragraph>`.

use std::collections::HashMap;

use scriptcraft_doc::{Beat, CardColor, Script, clean_beat_text, clean_beat_title};

use crate::{color_attr, esc, parse_color};

/// What was read of the board: beats by key, in file order, and where the board puts them.
#[derive(Default)]
pub(crate) struct Read {
    ids: HashMap<String, u32>,
    order: Vec<String>,
    /// key → (title, text lines, colour), from `ListItem`s.
    items: HashMap<String, (String, Vec<String>, Option<CardColor>)>,
    /// key → (left, top, width, height), from the beat `DisplayBoard`.
    places: HashMap<String, (f32, f32, f32, f32)>,
    /// The `ListItem` being read, when it is a beat.
    current: Option<String>,
    /// Inside a `DisplayBoard Type="Beat"`.
    in_board: bool,
}

/// A key as written in the file: trimmed, at most 64 characters.
fn key(k: &str) -> Option<String> {
    let k: String = k.trim().chars().take(64).collect();
    (!k.is_empty()).then_some(k)
}

impl Read {
    /// The id for a beat key (seen on a paragraph link, a `ListItem` or a board `Item`).
    fn id(&mut self, k: &str) -> Option<u32> {
        let k = key(k)?;
        if let Some(id) = self.ids.get(&k) {
            return Some(*id);
        }
        if self.ids.len() >= scriptcraft_doc::MAX_BEATS * 2 {
            return None;
        }
        let id = u32::try_from(self.ids.len() + 1).ok()?;
        self.ids.insert(k.clone(), id);
        self.order.push(k);
        Some(id)
    }

    /// `ScriptCraftBeats="uuid,uuid"` on a heading → beat ids.
    pub fn links(&mut self, v: &str) -> Vec<u32> {
        v.split(',').take(scriptcraft_doc::MAX_BEATS).filter_map(|k| self.id(k)).collect()
    }

    /// `<ListItem>` starts; only `Type="Beat"` items are read.
    pub fn list_item(&mut self, id: Option<String>, ty: Option<String>, title: Option<String>, color: Option<String>) {
        self.current = None;
        if !ty.is_some_and(|t| t.trim().eq_ignore_ascii_case("beat")) {
            return;
        }
        let Some(k) = id.as_deref().and_then(key) else { return };
        if self.items.contains_key(&k) || self.id(&k).is_none() || self.items.len() >= scriptcraft_doc::MAX_BEATS {
            return;
        }
        let color = color.and_then(|c| parse_color(&c));
        self.items.insert(k.clone(), (title.unwrap_or_default(), Vec::new(), color));
        self.current = Some(k);
    }

    pub fn end_list_item(&mut self) {
        self.current = None;
    }

    /// A paragraph of the current beat's text.
    pub fn line(&mut self, text: &str) {
        if let Some(k) = &self.current
            && let Some((_, lines, _)) = self.items.get_mut(k)
            && lines.len() < 400
        {
            lines.push(text.to_string());
        }
    }

    /// `<DisplayBoard Type>` starts or ends.
    pub fn board(&mut self, ty: Option<&str>) {
        self.in_board = ty.is_some_and(|t| t.trim().eq_ignore_ascii_case("beat"));
    }

    /// A card's place on the beat board.
    pub fn item(&mut self, id: Option<String>, left: Option<String>, top: Option<String>, width: Option<String>, height: Option<String>) {
        if !self.in_board {
            return;
        }
        let Some(k) = id.as_deref().and_then(key) else { return };
        let num = |v: Option<String>, d: f32| v.and_then(|v| v.trim().parse::<f32>().ok()).filter(|x| x.is_finite()).unwrap_or(d);
        if self.places.len() < scriptcraft_doc::MAX_BEATS {
            self.places.insert(k, (num(left, 0.0), num(top, 0.0), num(width, scriptcraft_doc::BEAT_W), num(height, scriptcraft_doc::BEAT_H)));
        }
    }

    /// Put the beats on the script's board (in file order). Links to beats the file doesn't
    /// define are dropped when the script is normalized.
    pub fn apply(self, s: &mut Script) {
        let mut beats = Vec::new();
        for k in &self.order {
            let (Some(id), Some((title, lines, color))) = (self.ids.get(k), self.items.get(k)) else { continue };
            let (x, y, w, h) = self.places.get(k).copied().unwrap_or_else(|| {
                let n = beats.len() as f32;
                (40.0 + (n % 5.0) * 280.0, 40.0 + (n / 5.0).floor() * 160.0, scriptcraft_doc::BEAT_W, scriptcraft_doc::BEAT_H)
            });
            let mut b = Beat::new(*id, &clean_beat_title(title), &clean_beat_text(&lines.join("\n")), *color, x, y);
            b.w = w;
            b.h = h;
            b.key = k.clone();
            beats.push(b);
        }
        s.board.beats = beats;
    }
}

/// `<ListItems>` and the beat `<DisplayBoards>`, after `</Content>`.
pub(crate) fn write(out: &mut String, s: &Script) {
    if s.board.beats.is_empty() {
        return;
    }
    out.push_str("  <ListItems>\n");
    for b in &s.board.beats {
        let color = b.color.map(color_attr).unwrap_or_else(|| "#FFFFFFFFFFFF".into());
        out.push_str(&format!(
            "    <ListItem Color=\"{color}\" Id=\"{}\" Title=\"{}\" Type=\"Beat\">\n      <Content>\n",
            esc(&b.key),
            esc(&b.title)
        ));
        for line in b.text.lines() {
            out.push_str(&format!("        <Paragraph>\n          <Text>{}</Text>\n        </Paragraph>\n", esc(line)));
        }
        out.push_str("      </Content>\n    </ListItem>\n");
    }
    out.push_str("  </ListItems>\n");
    out.push_str("  <DisplayBoards>\n    <DisplayBoard Height=\"10000\" Type=\"Beat\" Width=\"24000\">\n");
    for b in &s.board.beats {
        out.push_str(&format!(
            "      <Item Height=\"{:.0}\" Id=\"{}\" Left=\"{:.0}\" Top=\"{:.0}\" Width=\"{:.0}\"/>\n",
            b.h,
            esc(&b.key),
            b.x,
            b.y,
            b.w
        ));
    }
    out.push_str("    </DisplayBoard>\n  </DisplayBoards>\n");
}

/// A heading's `ScriptCraftBeats` attribute value (the linked beats' keys), if any.
pub(crate) fn links_attr(s: &Script, beats: &[u32]) -> Option<String> {
    let keys: Vec<&str> = beats.iter().filter_map(|id| s.board.get(*id)).map(|b| b.key.as_str()).collect();
    (!keys.is_empty()).then(|| keys.join(","))
}
