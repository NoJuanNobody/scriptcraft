//! The story map: scenes and structure points placed along the script's pages, for a timeline
//! view of the structure against page count.

use scriptcraft_doc::{CardColor, Script, StructureKind};
use scriptcraft_layout::Layout;
use serde::Serialize;

/// A scene on the map. Positions are in pages from the start of the script (`0.0` is the top of
/// page 1; `2.5` is halfway down page 3).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapScene {
    pub index: usize,
    pub para: usize,
    pub start: f32,
    pub end: f32,
    pub heading: String,
    pub number: Option<String>,
    pub color: Option<CardColor>,
    /// The printed page label the scene starts on.
    pub page: String,
}

/// A structure point on the map, at the start of the scene it opens.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPoint {
    pub index: usize,
    pub kind: StructureKind,
    pub title: String,
    pub color: Option<CardColor>,
    pub at: f32,
    /// Where the point's span ends (the next point of the same or a higher level).
    pub end: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryMap {
    /// Script pages.
    pub pages: f32,
    pub scenes: Vec<MapScene>,
    pub points: Vec<MapPoint>,
}

/// The story map of `s` laid out as `l`.
pub fn story_map(s: &Script, l: &Layout) -> StoryMap {
    // Where each paragraph's first line sits, in pages.
    let mut at = vec![f32::NAN; s.paras.len()];
    for p in &l.pages {
        let Some(n) = p.number else { continue };
        let h = l.paper.height_pt().max(1.0);
        for line in &p.lines {
            if let Some(slot) = line.para.and_then(|k| at.get_mut(k))
                && slot.is_nan()
            {
                *slot = (n.saturating_sub(1)) as f32 + (line.y / h).clamp(0.0, 1.0);
            }
        }
    }
    let pages = (l.script_pages().max(1)) as f32;
    let pos = |para: usize| at.get(para).copied().filter(|v| v.is_finite()).unwrap_or(0.0);
    let scenes_doc = s.scenes();
    let mut scenes: Vec<MapScene> = scenes_doc
        .iter()
        .map(|sc| MapScene {
            index: sc.index,
            para: sc.para,
            start: pos(sc.para),
            end: pages,
            heading: sc.heading.clone(),
            number: sc.number.clone(),
            color: sc.color,
            page: l.page_label_of(sc.para),
        })
        .collect();
    for i in 1..scenes.len() {
        let next = scenes.get(i).map_or(pages, |x| x.start);
        if let Some(prev) = scenes.get_mut(i - 1) {
            prev.end = next.max(prev.start);
        }
    }
    let mut points: Vec<MapPoint> = s
        .structure_points()
        .into_iter()
        .enumerate()
        .map(|(index, (slot, sp))| MapPoint {
            index,
            kind: sp.kind,
            title: sp.title.clone(),
            color: sp.color,
            at: slot.para.map_or(pages, pos),
            end: pages,
        })
        .collect();
    for i in 0..points.len() {
        let (lvl, from) = match points.get(i) {
            Some(p) => (p.kind.level(), p.at),
            None => continue,
        };
        let end = points.iter().skip(i + 1).find(|q| q.kind.level() <= lvl).map_or(pages, |q| q.at);
        if let Some(p) = points.get_mut(i) {
            p.end = end.max(from);
        }
    }
    StoryMap { pages, scenes, points }
}
