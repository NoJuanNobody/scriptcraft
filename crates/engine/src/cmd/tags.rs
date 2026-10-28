//! Production tagging: mark words as breakdown items (props, wardrobe, vehicles…), the
//! breakdown report per scene, and its CSV export.

use std::path::PathBuf;

use scriptcraft_doc::{TAG_CATEGORIES, category_rgb};
use serde_json::{Value, json};

use super::notes::range_param;
use crate::{CmdError, CmdResult, CommandSpec, Session, breakdown, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("tag.add", "Tag Selection", "Tools › Production Tagging", add).params(
            r#"{"category": string (a standard category or your own), "anchor": {"para", "off"}, "focus": {"para", "off"} (default: the selection)}"#,
        ),
        CommandSpec::new("tag.remove", "Remove Tags", "Tools › Production Tagging", remove)
            .params(r#"{"category": string (default: any), "anchor", "focus" (default: the selection, or the tag at the caret)}"#),
        CommandSpec::new("tag.list", "Tagged Items", "Tools › Production Tagging", list).pure(),
        CommandSpec::new("tag.categories", "Tag Categories", "Tools › Production Tagging", categories).pure(),
        CommandSpec::new("report.breakdown", "Breakdown Report", "Reports", report).pure(),
        CommandSpec::new("report.breakdownCsv", "Breakdown as CSV", "Reports", report_csv).pure(),
        CommandSpec::new("file.exportBreakdown", "Export Breakdown (CSV)", "File › Export", export).params(r#"{"path": ".csv"}"#).pure(),
    ]
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    let category = p::req_str(v, "category")?;
    let cat = scriptcraft_doc::clean_category(category).ok_or_else(|| CmdError::Params("the category is empty".into()))?;
    let (a, b) = range_param(s, v);
    if a == b {
        return Err(CmdError::Params("select the words to tag".into()));
    }
    let n = s.script.add_tag(a, b, &cat);
    if n == 0 {
        return Err(CmdError::Failed("nothing to tag there (already tagged, or only spaces)".into()));
    }
    Ok(json!({"tagged": n, "category": cat}))
}

fn remove(s: &mut Session, v: &Value) -> CmdResult {
    let (a, b) = range_param(s, v);
    let n = s.script.remove_tags(a, b, p::str(v, "category"));
    Ok(json!({"removed": n}))
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let mut tags = Vec::new();
    for (i, para) in s.script.paras.iter().enumerate() {
        for t in &para.tagged {
            tags.push(json!({
                "para": i, "start": t.start, "end": t.end, "category": t.category,
                "item": para.slice(t.start, t.end), "scene": s.script.scene_of(i).map(|x| x + 1), "page": l.page_number_of(i),
            }));
        }
    }
    Ok(json!({"tags": tags, "items": breakdown::items(&s.script)}))
}

fn categories(s: &mut Session, _: &Value) -> CmdResult {
    let used = s.script.tag_categories_used();
    let count = |c: &str| s.script.paras.iter().map(|p| p.tagged.iter().filter(|t| t.category == c).count()).sum::<usize>();
    let mut out: Vec<Value> =
        TAG_CATEGORIES.iter().map(|(n, rgb)| json!({"name": n, "color": hex(*rgb), "standard": true, "count": count(n)})).collect();
    for c in used.iter().filter(|c| !TAG_CATEGORIES.iter().any(|(n, _)| n == *c)) {
        out.push(json!({"name": c, "color": hex(category_rgb(c)), "standard": false, "count": count(c)}));
    }
    Ok(json!({"categories": out}))
}

fn report(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let scenes = breakdown::breakdown(&s.script, &l, &s.sluglines);
    Ok(json!({"scenes": scenes, "items": breakdown::items(&s.script), "categories": s.script.tag_categories_used()}))
}

fn report_csv(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    Ok(json!({"csv": breakdown::csv(&s.script, &l, &s.sluglines)}))
}

fn export(s: &mut Session, v: &Value) -> CmdResult {
    let path = PathBuf::from(p::req_str(v, "path")?);
    if !path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("csv")) {
        return Err(CmdError::Params("the path must end in .csv".into()));
    }
    let l = s.layout();
    let csv = breakdown::csv(&s.script, &l, &s.sluglines);
    crate::io::write_bytes(&path, csv.as_bytes()).map_err(CmdError::Failed)?;
    s.status = format!("Exported the breakdown to {}", path.display());
    Ok(json!({"path": path.display().to_string(), "bytes": csv.len()}))
}
