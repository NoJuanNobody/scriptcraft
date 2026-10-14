//! Structure points: acts, sequences and sections that group scenes (the outline).
//!
//! A structure point sits just before a scene: it is stored on that scene's heading paragraph
//! (`Para::structure`, in order), so it travels with the paragraph stream through every edit.
//! Points after the last scene (an act with no scenes yet) live in `Script::structure_tail`.
//! Points never print; they are the outline.
//!
//! The outline is the flat sequence of [`OutlineItem`]s: points and scenes in reading order.
//! Rearranging it ([`Script::move_outline_item`]) reorders the scenes' paragraphs and re-seats
//! every point before the scene that now follows it.

use serde::{Deserialize, Serialize};

use crate::{CardColor, Element, Para, Script, sanitize};

/// Most structure points a script may hold.
pub const MAX_STRUCTURE: usize = 5_000;
/// Longest title, in characters.
const MAX_TITLE: usize = 200;
/// Longest summary, in characters.
const MAX_SUMMARY: usize = 4_000;

/// The level of a structure point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StructureKind {
    /// An act (top level).
    Act,
    /// A sequence inside an act.
    Sequence,
    /// A free section or beat label (lowest level).
    Section,
}

impl StructureKind {
    pub const ALL: [StructureKind; 3] = [StructureKind::Act, StructureKind::Sequence, StructureKind::Section];

    pub fn id(self) -> &'static str {
        match self {
            StructureKind::Act => "act",
            StructureKind::Sequence => "sequence",
            StructureKind::Section => "section",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StructureKind::Act => "Act",
            StructureKind::Sequence => "Sequence",
            StructureKind::Section => "Section",
        }
    }

    pub fn parse(s: &str) -> Option<StructureKind> {
        let s = s.trim();
        StructureKind::ALL.into_iter().find(|k| k.id().eq_ignore_ascii_case(s) || k.label().eq_ignore_ascii_case(s))
    }

    /// 1 for acts, 2 for sequences, 3 for sections (the outline depth).
    pub fn level(self) -> usize {
        match self {
            StructureKind::Act => 1,
            StructureKind::Sequence => 2,
            StructureKind::Section => 3,
        }
    }

    /// The kind for an outline level (1, 2, 3 and deeper).
    pub fn from_level(level: usize) -> StructureKind {
        match level {
            0 | 1 => StructureKind::Act,
            2 => StructureKind::Sequence,
            _ => StructureKind::Section,
        }
    }
}

/// An act, sequence or section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructurePoint {
    pub kind: StructureKind,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<CardColor>,
}

impl StructurePoint {
    /// A cleaned point (control characters dropped, lengths capped).
    pub fn new(kind: StructureKind, title: &str, summary: &str, color: Option<CardColor>) -> Self {
        let mut p = StructurePoint { kind, title: title.to_string(), summary: summary.to_string(), color };
        p.clean();
        p
    }

    pub fn clean(&mut self) {
        let title: String = sanitize(&self.title).trim().chars().take(MAX_TITLE).collect();
        self.title = if title.is_empty() { self.kind.label().to_string() } else { title };
        self.summary = sanitize(&self.summary).trim().chars().take(MAX_SUMMARY).collect();
    }
}

/// One entry of the outline, in reading order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineItem {
    /// Structure point `n` (counting every point in reading order).
    Point(usize),
    /// Scene `n`.
    Scene(usize),
}

/// Where a structure point is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointSlot {
    /// The heading paragraph it precedes; `None` for the points after the last scene.
    pub para: Option<usize>,
    /// Its place in that paragraph's (or the tail's) list.
    pub slot: usize,
}

impl Script {
    /// Every structure point in reading order, with where it is stored.
    pub fn structure_points(&self) -> Vec<(PointSlot, &StructurePoint)> {
        let mut out = Vec::new();
        for (i, p) in self.paras.iter().enumerate() {
            for (k, sp) in p.structure.iter().enumerate() {
                out.push((PointSlot { para: Some(i), slot: k }, sp));
            }
        }
        for (k, sp) in self.structure_tail.iter().enumerate() {
            out.push((PointSlot { para: None, slot: k }, sp));
        }
        out
    }

    /// The number of structure points.
    pub fn structure_count(&self) -> usize {
        self.paras.iter().map(|p| p.structure.len()).sum::<usize>() + self.structure_tail.len()
    }

    /// Structure point `n` (reading order).
    pub fn structure_point(&self, n: usize) -> Option<&StructurePoint> {
        self.structure_points().into_iter().nth(n).map(|(_, p)| p)
    }

    /// Mutable structure point `n` (reading order).
    pub fn structure_point_mut(&mut self, n: usize) -> Option<&mut StructurePoint> {
        let slot = self.structure_points().into_iter().nth(n).map(|(s, _)| s)?;
        match slot.para {
            Some(i) => self.paras.get_mut(i)?.structure.get_mut(slot.slot),
            None => self.structure_tail.get_mut(slot.slot),
        }
    }

    /// The outline: points and scenes in reading order.
    pub fn outline(&self) -> Vec<OutlineItem> {
        let mut out = Vec::new();
        let (mut np, mut ns) = (0, 0);
        for p in &self.paras {
            for _ in &p.structure {
                out.push(OutlineItem::Point(np));
                np += 1;
            }
            if p.element == Element::SceneHeading {
                out.push(OutlineItem::Scene(ns));
                ns += 1;
            }
        }
        for _ in &self.structure_tail {
            out.push(OutlineItem::Point(np));
            np += 1;
        }
        out
    }

    /// The structure points in force at scene `scene` (the nearest act, sequence and section
    /// before it), deepest last, as reading-order indices.
    pub fn structure_path(&self, scene: usize) -> Vec<usize> {
        let mut path: Vec<(StructureKind, usize)> = Vec::new();
        let points = self.structure_points();
        for item in self.outline() {
            match item {
                OutlineItem::Point(n) => {
                    if let Some((_, sp)) = points.get(n) {
                        path.retain(|(k, _)| k.level() < sp.kind.level());
                        path.push((sp.kind, n));
                    }
                }
                OutlineItem::Scene(s) if s == scene => break,
                OutlineItem::Scene(_) => {}
            }
        }
        path.into_iter().map(|(_, n)| n).collect()
    }

    /// Insert a point before scene `before` (after any points already there), or after the last
    /// scene when `before` is `None` or past the end. Returns its reading-order index.
    pub fn add_structure(&mut self, point: StructurePoint, before: Option<usize>) -> Option<usize> {
        if self.structure_count() >= MAX_STRUCTURE {
            return None;
        }
        let mut point = point;
        point.clean();
        let head = before.and_then(|b| self.scenes().get(b).map(|s| s.para));
        match head.and_then(|h| self.paras.get_mut(h)) {
            Some(p) => p.structure.push(point),
            None => self.structure_tail.push(point),
        }
        let want_para = head;
        let points = self.structure_points();
        // The new point is the last one stored at that place.
        points.iter().rposition(|(s, _)| s.para == want_para)
    }

    /// Remove point `n`; returns it.
    pub fn remove_structure(&mut self, n: usize) -> Option<StructurePoint> {
        let slot = self.structure_points().into_iter().nth(n).map(|(s, _)| s)?;
        match slot.para {
            Some(i) => {
                let v = &mut self.paras.get_mut(i)?.structure;
                (slot.slot < v.len()).then(|| v.remove(slot.slot))
            }
            None => (slot.slot < self.structure_tail.len()).then(|| self.structure_tail.remove(slot.slot)),
        }
    }

    /// Move outline item `from` so that it ends up at position `to` of the outline (both
    /// indices into [`Script::outline`]). Scenes carry their paragraphs; every point is re-seated
    /// before the scene that follows it. Returns the moved item's new outline position.
    pub fn move_outline_item(&mut self, from: usize, to: usize) -> Option<usize> {
        let mut order = self.outline();
        if from >= order.len() {
            return None;
        }
        let to = to.min(order.len() - 1);
        let item = order.remove(from);
        order.insert(to, item);
        self.rebuild_outline(&order);
        Some(to)
    }

    /// Reassemble the paragraphs in the given outline order (a permutation of
    /// [`Script::outline`]). The paragraphs before the first scene stay first.
    fn rebuild_outline(&mut self, order: &[OutlineItem]) {
        let scenes = self.scenes();
        let first = scenes.first().map_or(self.paras.len(), |s| s.para);
        // Take every point out, in reading order.
        let mut points: Vec<Option<StructurePoint>> = Vec::new();
        for p in &mut self.paras {
            points.extend(std::mem::take(&mut p.structure).into_iter().map(Some));
        }
        points.extend(std::mem::take(&mut self.structure_tail).into_iter().map(Some));
        // Cut the scene blocks out.
        let mut paras = std::mem::take(&mut self.paras);
        let mut blocks: Vec<Option<Vec<Para>>> = Vec::with_capacity(scenes.len());
        for sc in scenes.iter().rev() {
            let end = sc.end.min(paras.len());
            let start = sc.para.min(end);
            blocks.push(Some(paras.drain(start..end).collect()));
        }
        blocks.reverse();
        paras.truncate(first.min(paras.len()));
        let mut pending: Vec<StructurePoint> = Vec::new();
        let mut used_scene = vec![false; blocks.len()];
        for item in order {
            match *item {
                OutlineItem::Point(n) => {
                    if let Some(p) = points.get_mut(n).and_then(Option::take) {
                        pending.push(p);
                    }
                }
                OutlineItem::Scene(n) => {
                    if let Some(mut block) = blocks.get_mut(n).and_then(Option::take) {
                        if let Some(h) = block.first_mut() {
                            h.structure = std::mem::take(&mut pending);
                        }
                        if let Some(u) = used_scene.get_mut(n) {
                            *u = true;
                        }
                        paras.extend(block);
                    }
                }
            }
        }
        // Anything the order missed keeps its place at the end (never lose text).
        for block in blocks.into_iter().flatten() {
            paras.extend(block);
        }
        pending.extend(points.into_iter().flatten());
        self.paras = paras;
        self.structure_tail = pending;
        if self.paras.is_empty() {
            self.paras.push(Para::new(Element::Action, ""));
        }
    }

    /// Keep points only on scene headings: points on any other paragraph move to the next
    /// heading (or the tail). Caps the count and cleans each point.
    pub(crate) fn fix_structure(&mut self) {
        let mut carry: Vec<StructurePoint> = Vec::new();
        let mut total = 0usize;
        for p in &mut self.paras {
            if p.element == Element::SceneHeading {
                if !carry.is_empty() {
                    let mut v = std::mem::take(&mut carry);
                    v.append(&mut p.structure);
                    p.structure = v;
                }
            } else if !p.structure.is_empty() {
                carry.append(&mut p.structure);
                continue;
            }
            for sp in &mut p.structure {
                sp.clean();
            }
            let room = MAX_STRUCTURE.saturating_sub(total);
            p.structure.truncate(room);
            total += p.structure.len();
        }
        if !carry.is_empty() {
            carry.append(&mut self.structure_tail);
            self.structure_tail = carry;
        }
        for sp in &mut self.structure_tail {
            sp.clean();
        }
        self.structure_tail.truncate(MAX_STRUCTURE.saturating_sub(total));
    }
}
