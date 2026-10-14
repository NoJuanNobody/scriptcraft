//! Structure points and the outline: acts, sequences and sections that group scenes, the outline
//! (points and scenes in order, rearranged by moving items), and the story map.
//! Points are numbered in reading order (`index`); outline items by their place in the outline.

use scriptcraft_doc::{CardColor, OutlineItem, Pos, StructureKind, StructurePoint};
use serde_json::{Value, json};

use super::set_caret;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn has_points(s: &Session) -> Option<&'static str> {
    (s.script.structure_count() == 0).then_some("the script has no acts, sequences or sections")
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("structure.list", "Structure Points", "View › Outline", list).pure(),
        CommandSpec::new("structure.add", "Add Structure Point", "View › Outline", add).params(
            r#"{"kind": "act"|"sequence"|"section", "title": string, "summary": string, "color": colour name, "before": scene index (default: the caret's scene) or "end"}"#,
        ),
        CommandSpec::new("structure.edit", "Edit Structure Point", "View › Outline", edit)
            .params(r#"{"index": n, "kind", "title", "summary", "color": colour name or null} (each optional but index)"#),
        CommandSpec::new("structure.delete", "Delete Structure Point", "View › Outline", delete).params(r#"{"index": n}"#).when(has_points),
        CommandSpec::new("structure.goto", "Go to Structure Point", "View › Outline", goto).params(r#"{"index": n}"#).pure().when(has_points),
        CommandSpec::new("outline.list", "Outline", "View › Outline", outline_list).pure(),
        CommandSpec::new("outline.move", "Move in Outline", "View › Outline", outline_move)
            .params(r#"{"from": n, "to": n} (positions in outline.list; a scene keeps its text, every point stays before the scene that follows it)"#),
        CommandSpec::new("storymap.get", "Story Map", "View › Story Map", storymap).pure(),
    ]
}

fn color_param(v: &Value) -> Result<Option<Option<CardColor>>, CmdError> {
    match v.get("color") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(c)) if c.is_empty() || c == "none" => Ok(Some(None)),
        Some(Value::String(c)) => CardColor::parse(c).map(|c| Some(Some(c))).ok_or_else(|| CmdError::Params(format!("unknown colour `{c}`"))),
        Some(_) => Err(CmdError::Params("`color` must be a colour name or null".into())),
    }
}

fn kind_param(v: &Value) -> Result<Option<StructureKind>, CmdError> {
    match v.get("kind") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(k)) => {
            StructureKind::parse(k).map(Some).ok_or_else(|| CmdError::Params(format!("unknown kind `{k}` (act, sequence, section)")))
        }
        Some(_) => Err(CmdError::Params("`kind` must be act, sequence or section".into())),
    }
}

fn str_param(v: &Value, k: &str) -> Result<Option<String>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) => Ok(Some(t.clone())),
        Some(_) => Err(CmdError::Params(format!("`{k}` must be a string"))),
    }
}

fn point_json(s: &Session, l: &scriptcraft_layout::Layout, index: usize, sp: &StructurePoint, para: Option<usize>) -> Value {
    let scene = para.and_then(|h| s.script.scene_of(h));
    json!({
        "index": index, "kind": sp.kind.id(), "title": sp.title, "summary": sp.summary, "color": sp.color.map(|c| c.id()),
        "scene": scene, "page": para.map(|h| l.page_label_of(h)),
    })
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let points: Vec<Value> = s.script.structure_points().into_iter().enumerate().map(|(i, (slot, sp))| point_json(s, &l, i, sp, slot.para)).collect();
    Ok(json!({"points": points}))
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    let kind = kind_param(v)?.unwrap_or(StructureKind::Act);
    let title = str_param(v, "title")?.unwrap_or_default();
    let summary = str_param(v, "summary")?.unwrap_or_default();
    let color = color_param(v)?.flatten();
    let scenes = s.script.scenes().len();
    let before = match v.get("before") {
        Some(Value::String(e)) if e.eq_ignore_ascii_case("end") => None,
        Some(Value::Null) => None,
        Some(_) => {
            let b = p::req_usize(v, "before")?;
            if b >= scenes {
                return Err(CmdError::Params(format!("no scene {b} (the script has {scenes})")));
            }
            Some(b)
        }
        None => (scenes > 0).then(|| s.script.scene_of(s.sel.focus.para).unwrap_or(0)),
    };
    let index = s
        .script
        .add_structure(StructurePoint::new(kind, &title, &summary, color), before)
        .ok_or_else(|| CmdError::Failed(format!("too many structure points (the limit is {})", scriptcraft_doc::MAX_STRUCTURE)))?;
    Ok(json!({"index": index}))
}

fn req_point(s: &Session, v: &Value) -> Result<usize, CmdError> {
    let i = p::req_usize(v, "index")?;
    let n = s.script.structure_count();
    (i < n).then_some(i).ok_or_else(|| CmdError::Params(format!("no structure point {i} (the script has {n})")))
}

fn edit(s: &mut Session, v: &Value) -> CmdResult {
    let i = req_point(s, v)?;
    let (kind, title, summary, color) = (kind_param(v)?, str_param(v, "title")?, str_param(v, "summary")?, color_param(v)?);
    let sp = s.script.structure_point_mut(i).ok_or_else(|| CmdError::Params(format!("no structure point {i}")))?;
    if let Some(k) = kind {
        sp.kind = k;
    }
    if let Some(t) = title {
        sp.title = t;
    }
    if let Some(t) = summary {
        sp.summary = t;
    }
    if let Some(c) = color {
        sp.color = c;
    }
    sp.clean();
    Ok(json!({"index": i}))
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let i = req_point(s, v)?;
    s.script.remove_structure(i).ok_or_else(|| CmdError::Params(format!("no structure point {i}")))?;
    Ok(json!({"index": i}))
}

fn goto(s: &mut Session, v: &Value) -> CmdResult {
    let i = req_point(s, v)?;
    let at = s.script.structure_points().get(i).map(|(slot, _)| slot.para);
    let pos = match at.flatten() {
        Some(para) => Pos::new(para, 0),
        None => s.script.end(),
    };
    set_caret(s, pos);
    super::sel_result(s)
}

/// The outline as JSON: points and scenes in order, with their depth (points by level, scenes
/// one deeper than the innermost point above them).
pub fn outline_json(s: &mut Session) -> Value {
    let l = s.layout();
    let points = s.script.structure_points();
    let scenes = s.script.scenes();
    let mut depth = 0usize;
    let items: Vec<Value> = s
        .script
        .outline()
        .into_iter()
        .enumerate()
        .filter_map(|(pos, it)| match it {
            OutlineItem::Point(n) => {
                let (slot, sp) = points.get(n)?;
                depth = sp.kind.level();
                let mut o = point_json(s, &l, n, sp, slot.para);
                if let Some(m) = o.as_object_mut() {
                    m.insert("type".into(), json!("point"));
                    m.insert("position".into(), json!(pos));
                    m.insert("depth".into(), json!(sp.kind.level() - 1));
                }
                Some(o)
            }
            OutlineItem::Scene(n) => {
                let sc = scenes.get(n)?;
                Some(json!({
                    "type": "scene", "position": pos, "index": n, "para": sc.para, "heading": sc.heading, "number": sc.number,
                    "summary": sc.summary, "color": sc.color.map(|c| c.id()), "page": l.page_label_of(sc.para), "depth": depth,
                }))
            }
        })
        .collect();
    json!({"items": items})
}

fn outline_list(s: &mut Session, _: &Value) -> CmdResult {
    Ok(outline_json(s))
}

fn outline_move(s: &mut Session, v: &Value) -> CmdResult {
    let from = p::req_usize(v, "from")?;
    let to = p::req_usize(v, "to")?;
    let order = s.script.outline();
    let n = order.len();
    if from >= n || to >= n {
        return Err(CmdError::Params(format!("the outline has {n} items")));
    }
    let item = order.get(from).copied();
    s.script.move_outline_item(from, to);
    // Follow a moved scene with the caret.
    if let Some(OutlineItem::Scene(_)) = item {
        let scene = s.script.outline().iter().take(to + 1).filter(|i| matches!(i, OutlineItem::Scene(_))).count().saturating_sub(1);
        if let Some(sc) = s.script.scenes().get(scene) {
            set_caret(s, Pos::new(sc.para, 0));
        }
    }
    Ok(json!({"position": to}))
}

fn storymap(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    serde_json::to_value(crate::storymap::story_map(&s.script, &l)).map_err(|e| CmdError::Failed(e.to_string()))
}
