//! Production breakdown: tagged items (props, wardrobe, vehicles…) per scene, and as CSV.

use scriptcraft_doc::Script;
use scriptcraft_layout::Layout;
use serde::Serialize;

use crate::reports;

/// The items of one category in a scene.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryItems {
    pub category: String,
    pub items: Vec<String>,
}

/// One scene's breakdown sheet.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneBreakdown {
    /// 1-based scene index (0 for text before the first scene heading).
    pub scene: usize,
    pub number: Option<String>,
    pub heading: String,
    pub location: String,
    pub time: String,
    /// Position of the scene's first page among script pages, from 1.
    pub page: usize,
    /// The scene's first page as printed (`"12A"` with locked pages).
    pub page_label: String,
    /// Length in eighths of a page.
    pub eighths: usize,
    pub length: String,
    /// Characters who speak in the scene.
    pub speaking: Vec<String>,
    pub categories: Vec<CategoryItems>,
}

/// One tagged item across the script.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemTotal {
    pub category: String,
    pub item: String,
    /// Times tagged.
    pub count: usize,
    /// Scenes (1-based) it appears in.
    pub scenes: Vec<usize>,
}

/// A tagged range's item name: its text, single-spaced.
fn item_name(s: &Script, para: usize, start: usize, end: usize) -> String {
    s.para(para).map(|p| p.slice(start, end).split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default()
}

fn add_item(cats: &mut Vec<CategoryItems>, category: &str, item: String) {
    if item.is_empty() {
        return;
    }
    let c = match cats.iter().position(|c| c.category == category) {
        Some(k) => k,
        None => {
            cats.push(CategoryItems { category: category.to_string(), items: Vec::new() });
            cats.len() - 1
        }
    };
    if let Some(c) = cats.get_mut(c)
        && !c.items.iter().any(|i| i.eq_ignore_ascii_case(&item))
    {
        c.items.push(item);
    }
}

fn sort_categories(cats: &mut [CategoryItems], order: &[String]) {
    cats.sort_by_key(|c| order.iter().position(|o| *o == c.category).unwrap_or(usize::MAX));
}

/// Every scene with its tagged items by category (scenes without tags included, so the sheet
/// lists the whole script). Text before the first heading appears as scene 0 when tagged.
pub fn breakdown(s: &Script, l: &Layout) -> Vec<SceneBreakdown> {
    let order = s.tag_categories_used();
    let mut out: Vec<SceneBreakdown> = Vec::new();
    let first = s.scenes().first().map_or(s.paras.len(), |sc| sc.para);
    let mut pre: Vec<CategoryItems> = Vec::new();
    for (i, p) in s.paras.iter().enumerate().take(first) {
        for t in &p.tagged {
            add_item(&mut pre, &t.category, item_name(s, i, t.start, t.end));
        }
    }
    if !pre.is_empty() {
        sort_categories(&mut pre, &order);
        out.push(SceneBreakdown {
            scene: 0,
            number: None,
            heading: String::new(),
            location: String::new(),
            time: String::new(),
            page: 1,
            page_label: l.page_label_of(0),
            eighths: 0,
            length: String::new(),
            speaking: Vec::new(),
            categories: pre,
        });
    }
    let scenes = s.scenes();
    for r in reports::scenes(s, l) {
        let Some(sc) = scenes.get(r.scene.saturating_sub(1)) else { continue };
        let mut cats = Vec::new();
        for i in sc.para..sc.end {
            for t in s.para(i).map(|p| p.tagged.as_slice()).unwrap_or(&[]) {
                add_item(&mut cats, &t.category, item_name(s, i, t.start, t.end));
            }
        }
        sort_categories(&mut cats, &order);
        out.push(SceneBreakdown {
            scene: r.scene,
            number: r.number,
            heading: r.heading,
            location: r.location,
            time: r.time,
            page: r.page,
            page_label: r.page_label,
            eighths: r.eighths,
            length: reports::eighths_label(r.eighths),
            speaking: r.characters,
            categories: cats,
        });
    }
    out
}

/// Every tagged item with how often and where it appears, by category then first appearance.
pub fn items(s: &Script) -> Vec<ItemTotal> {
    let order = s.tag_categories_used();
    let mut out: Vec<ItemTotal> = Vec::new();
    for (i, p) in s.paras.iter().enumerate() {
        let scene = s.scene_of(i).map_or(0, |x| x + 1);
        for t in &p.tagged {
            let item = item_name(s, i, t.start, t.end);
            if item.is_empty() {
                continue;
            }
            match out.iter_mut().find(|x| x.category == t.category && x.item.eq_ignore_ascii_case(&item)) {
                Some(x) => {
                    x.count += 1;
                    if !x.scenes.contains(&scene) {
                        x.scenes.push(scene);
                    }
                }
                None => out.push(ItemTotal { category: t.category.clone(), item, count: 1, scenes: vec![scene] }),
            }
        }
    }
    out.sort_by_key(|x| order.iter().position(|o| *o == x.category).unwrap_or(usize::MAX));
    out
}

/// One CSV cell: quoted when needed; a leading `= + - @` is defused so spreadsheets never run it.
fn cell(v: &str) -> String {
    let v = if v.starts_with(['=', '+', '-', '@']) { format!("'{v}") } else { v.to_string() };
    if v.contains([',', '"', '\n', '\r']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v }
}

/// The breakdown as CSV: one row per scene, a column per tag category in use (items joined
/// with `; `).
pub fn csv(s: &Script, l: &Layout) -> String {
    let cats = s.tag_categories_used();
    let mut out = String::new();
    let mut head = vec!["Scene".to_string(), "Number".into(), "Heading".into(), "Page".into(), "Length".into(), "Speaking".into()];
    head.extend(cats.iter().cloned());
    out.push_str(&head.iter().map(|h| cell(h)).collect::<Vec<_>>().join(","));
    out.push_str("\r\n");
    for b in breakdown(s, l) {
        let mut row = vec![
            b.scene.to_string(),
            b.number.clone().unwrap_or_default(),
            b.heading.clone(),
            b.page_label.clone(),
            b.length.clone(),
            b.speaking.join("; "),
        ];
        for c in &cats {
            row.push(b.categories.iter().find(|x| x.category == *c).map(|x| x.items.join("; ")).unwrap_or_default());
        }
        out.push_str(&row.iter().map(|v| cell(v)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}
