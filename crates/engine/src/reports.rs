//! Reports: characters, scenes, locations.

use scriptcraft_doc::{Element, HeadingSetting, Script, Sluglines, character_name};
use scriptcraft_layout::{Layout, LineKind};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterReport {
    pub name: String,
    /// Dialogue blocks spoken.
    pub speeches: usize,
    /// Words of dialogue.
    pub words: usize,
    /// Scenes (1-based) the character speaks in.
    pub scenes: Vec<usize>,
    /// Position of the first page among script pages, from 1.
    pub first_page: usize,
    /// The first page as printed: `"3"`, or `"12A"` with locked pages.
    pub first_page_label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneReport {
    /// 1-based scene index.
    pub scene: usize,
    pub number: Option<String>,
    pub heading: String,
    pub location: String,
    pub time: String,
    /// Position of the scene's first page among script pages, from 1.
    pub page: usize,
    /// The scene's first page as printed: `"3"`, or `"12A"` / `"12-13"` with locked pages.
    pub page_label: String,
    /// Length in eighths of a page.
    pub eighths: usize,
    pub characters: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationReport {
    pub location: String,
    pub scenes: usize,
    pub interior: bool,
    pub exterior: bool,
}

/// Every speaking character, most speeches first.
pub fn characters(s: &Script, l: &Layout) -> Vec<CharacterReport> {
    let mut out: Vec<CharacterReport> = Vec::new();
    let mut current: Option<usize> = None;
    for (i, p) in s.paras.iter().enumerate() {
        match p.element {
            Element::Character => {
                let name = character_name(&p.text);
                if name.is_empty() {
                    current = None;
                    continue;
                }
                let k = match out.iter().position(|c| c.name == name) {
                    Some(k) => k,
                    None => {
                        out.push(CharacterReport {
                            name,
                            speeches: 0,
                            words: 0,
                            scenes: Vec::new(),
                            first_page: l.page_number_of(i),
                            first_page_label: l.page_label_of(i),
                        });
                        out.len() - 1
                    }
                };
                if let Some(c) = out.get_mut(k) {
                    c.speeches += 1;
                    if let Some(sc) = s.scene_of(i).map(|x| x + 1)
                        && !c.scenes.contains(&sc)
                    {
                        c.scenes.push(sc);
                    }
                }
                current = Some(k);
            }
            Element::Dialogue => {
                if let Some(c) = current.and_then(|k| out.get_mut(k)) {
                    c.words += p.text.split_whitespace().count();
                }
            }
            Element::Parenthetical => {}
            _ => current = None,
        }
    }
    out.sort_by(|a, b| b.speeches.cmp(&a.speeches).then(b.words.cmp(&a.words)));
    out
}

/// Every scene with its page, length and cast.
pub fn scenes(s: &Script, l: &Layout, words: &Sluglines) -> Vec<SceneReport> {
    let mut lines = vec![0usize; s.paras.len()];
    for p in &l.pages {
        for ln in &p.lines {
            if ln.kind == LineKind::Body
                && let Some(c) = ln.para.and_then(|k| lines.get_mut(k))
            {
                *c += 1;
            }
        }
    }
    s.scenes()
        .into_iter()
        .map(|sc| {
            let n: usize = lines.get(sc.para..sc.end).map_or(0, |v| v.iter().sum::<usize>() + v.len().saturating_sub(1));
            // A page is 54 lines: an eighth is 6.75 lines.
            let eighths = ((n as f32 / 6.75).ceil() as usize).max(1);
            let mut cast: Vec<String> = Vec::new();
            for p in s.paras.get(sc.para..sc.end).unwrap_or(&[]) {
                if p.element == Element::Character {
                    let name = character_name(&p.text);
                    if !name.is_empty() && !cast.contains(&name) {
                        cast.push(name);
                    }
                }
            }
            let (_, location, time) = words.parts(&sc.heading);
            SceneReport {
                scene: sc.index + 1,
                number: sc.number.clone(),
                heading: sc.heading.clone(),
                location,
                time,
                page: l.page_number_of(sc.para),
                page_label: l.page_label_of(sc.para),
                eighths,
                characters: cast,
                summary: sc.summary.clone(),
            }
        })
        .collect()
}

/// Every location, by number of scenes; interior and exterior come from the user's `words`.
pub fn locations(s: &Script, words: &Sluglines) -> Vec<LocationReport> {
    let mut out: Vec<LocationReport> = Vec::new();
    for sc in s.scenes() {
        let (_, loc, _) = words.parts(&sc.heading);
        if loc.is_empty() {
            continue;
        }
        let setting = words.setting(&sc.heading);
        let int = matches!(setting, Some(HeadingSetting::Interior | HeadingSetting::Both));
        let ext = matches!(setting, Some(HeadingSetting::Exterior | HeadingSetting::Both));
        match out.iter_mut().find(|r| r.location == loc) {
            Some(r) => {
                r.scenes += 1;
                r.interior |= int;
                r.exterior |= ext;
            }
            None => out.push(LocationReport { location: loc, scenes: 1, interior: int, exterior: ext }),
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.scenes));
    out
}

/// Format eighths as a page count: `1 3/8`.
pub fn eighths_label(e: usize) -> String {
    match (e / 8, e % 8) {
        (0, r) => format!("{r}/8"),
        (w, 0) => format!("{w}"),
        (w, r) => format!("{w} {r}/8"),
    }
}
