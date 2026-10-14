//! Bookmarks (named marks on paragraphs) and the beat board (cards for beats, linked to scenes).

use scriptcraft_doc::{CardColor, Element, Pos};
use serde_json::{Value, json};

use super::{sel_result, set_caret};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn has_bookmarks(s: &Session) -> Option<&'static str> {
    (!s.script.paras.iter().any(|p| p.bookmark.is_some())).then_some("the script has no bookmarks")
}

fn has_beats(s: &Session) -> Option<&'static str> {
    s.script.board.is_empty().then_some("the beat board is empty")
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("bookmark.add", "Add Bookmark", "Edit › Bookmarks", bookmark_add)
            .params(r#"{"name": string (default: the paragraph's first words), "para": n (default: the caret's)} — a name in use moves here"#),
        CommandSpec::new("bookmark.remove", "Remove Bookmark", "Edit › Bookmarks", bookmark_remove)
            .params(r#"{"name": string} or {"para": n} (default: the caret's paragraph)"#)
            .when(has_bookmarks),
        CommandSpec::new("bookmark.rename", "Rename Bookmark", "Edit › Bookmarks", bookmark_rename).params(r#"{"name": string, "to": string}"#),
        CommandSpec::new("bookmark.list", "Bookmarks", "Edit › Bookmarks", bookmark_list).pure(),
        CommandSpec::new("bookmark.goto", "Go to Bookmark", "Edit › Bookmarks", bookmark_goto)
            .params(r#"{"name": string} or {"index": n}"#)
            .pure()
            .when(has_bookmarks),
        CommandSpec::new("bookmark.next", "Next Bookmark", "Edit › Bookmarks", bookmark_next).pure().when(has_bookmarks),
        CommandSpec::new("bookmark.previous", "Previous Bookmark", "Edit › Bookmarks", bookmark_previous).pure().when(has_bookmarks),
        CommandSpec::new("beat.add", "New Beat", "View › Beat Board", beat_add).params(
            r#"{"title": string, "text": string, "color": colour name, "x": points, "y": points, "w": points, "h": points, "scene": n (link to scene n)} → {"id"}"#,
        ),
        CommandSpec::new("beat.edit", "Edit Beat", "View › Beat Board", beat_edit)
            .params(r#"{"id": n, "title": string, "text": string, "color": colour name or null} (each optional but id)"#),
        CommandSpec::new("beat.move", "Move Beat", "View › Beat Board", beat_move)
            .params(r#"{"id": n, "x": points, "y": points, "w": points, "h": points} (each optional but id; the card comes to the front)"#),
        CommandSpec::new("beat.delete", "Delete Beat", "View › Beat Board", beat_delete).params(r#"{"id": n}"#).when(has_beats),
        CommandSpec::new("beat.list", "Beats", "View › Beat Board", beat_list).pure(),
        CommandSpec::new("beat.link", "Link Beat to Scene", "View › Beat Board", beat_link)
            .params(r#"{"id": n, "scene": n (0-based) or null (unlink); default: the caret's scene}"#)
            .when(has_beats),
        CommandSpec::new("beat.goto", "Go to Beat's Scene", "View › Beat Board", beat_goto).params(r#"{"id": n}"#).pure().when(has_beats),
        CommandSpec::new("beat.toScene", "New Scene from Beat", "View › Beat Board", beat_to_scene)
            .params(r#"{"id": n, "after": n (scene to put it after; default: the end)} → a scene heading from the title, the text as its summary, linked"#)
            .when(has_beats),
    ]
}

// --- Bookmarks ---

fn bookmarks_json(s: &mut Session) -> Vec<Value> {
    let l = s.layout();
    s.script
        .bookmarks()
        .into_iter()
        .enumerate()
        .map(|(index, (para, name))| {
            let text: String = s.script.para(para).map_or(String::new(), |p| p.text.chars().take(80).collect());
            json!({"index": index, "name": name, "para": para, "text": text, "page": l.page_label_of(para), "scene": s.script.scene_of(para).map(|x| x + 1)})
        })
        .collect()
}

fn para_param(s: &Session, v: &Value) -> Result<usize, CmdError> {
    match v.get("para") {
        None | Some(Value::Null) => Ok(s.sel.focus.para),
        Some(_) => {
            let i = p::req_usize(v, "para")?;
            if i < s.script.paras.len() { Ok(i) } else { Err(CmdError::Params(format!("no paragraph {i}"))) }
        }
    }
}

fn bookmark_add(s: &mut Session, v: &Value) -> CmdResult {
    let para = para_param(s, v)?;
    let name = match v.get("name") {
        None | Some(Value::Null) => {
            let text = s.script.para(para).map_or("", |p| p.text.as_str());
            let words: String = text.split_whitespace().take(6).collect::<Vec<_>>().join(" ");
            if words.is_empty() { format!("Bookmark {}", s.script.bookmarks().len() + 1) } else { words }
        }
        Some(_) => p::req_str(v, "name")?.to_string(),
    };
    let set = s.script.set_bookmark(para, Some(&name)).map_err(|e| CmdError::Params(e.to_string()))?;
    let set = set.ok_or_else(|| CmdError::Params("the bookmark name is empty".into()))?;
    s.status = format!("Bookmarked “{set}”");
    Ok(json!({"name": set, "para": para}))
}

/// The paragraph named by `name` or `index`.
fn find_bookmark(s: &Session, v: &Value) -> Result<usize, CmdError> {
    if let Some(n) = p::str(v, "name") {
        return s.script.bookmark_para(n).ok_or_else(|| CmdError::Params(format!("no bookmark “{n}”")));
    }
    let i = p::req_usize(v, "index")?;
    s.script.bookmarks().get(i).map(|(p, _)| *p).ok_or_else(|| CmdError::Params(format!("no bookmark {i}")))
}

fn bookmark_remove(s: &mut Session, v: &Value) -> CmdResult {
    let para = if v.get("name").is_some() { find_bookmark(s, v)? } else { para_param(s, v)? };
    let name = s.script.para(para).and_then(|p| p.bookmark.clone()).ok_or_else(|| CmdError::Params("no bookmark there".into()))?;
    s.script.set_bookmark(para, None).map_err(|e| CmdError::Params(e.to_string()))?;
    Ok(json!({"removed": name}))
}

fn bookmark_rename(s: &mut Session, v: &Value) -> CmdResult {
    let para = find_bookmark(s, v)?;
    let to = p::req_str(v, "to")?;
    let set = s.script.set_bookmark(para, Some(to)).map_err(|e| CmdError::Params(e.to_string()))?;
    let set = set.ok_or_else(|| CmdError::Params("the new name is empty".into()))?;
    Ok(json!({"name": set, "para": para}))
}

fn bookmark_list(s: &mut Session, _: &Value) -> CmdResult {
    Ok(json!({"bookmarks": bookmarks_json(s)}))
}

fn go_para(s: &mut Session, para: usize) -> CmdResult {
    set_caret(s, Pos::new(para, 0));
    sel_result(s)
}

fn bookmark_goto(s: &mut Session, v: &Value) -> CmdResult {
    let para = find_bookmark(s, v)?;
    go_para(s, para)
}

fn bookmark_step(s: &mut Session, forward: bool) -> CmdResult {
    let at = s.sel.focus.para;
    let marks: Vec<usize> = s.script.bookmarks().into_iter().map(|(p, _)| p).collect();
    let next = if forward {
        marks.iter().copied().find(|p| *p > at).or_else(|| marks.first().copied())
    } else {
        marks.iter().rev().copied().find(|p| *p < at).or_else(|| marks.last().copied())
    };
    let para = next.ok_or_else(|| CmdError::Disabled("the script has no bookmarks".into()))?;
    go_para(s, para)
}

fn bookmark_next(s: &mut Session, _: &Value) -> CmdResult {
    bookmark_step(s, true)
}

fn bookmark_previous(s: &mut Session, _: &Value) -> CmdResult {
    bookmark_step(s, false)
}

// --- Beats ---

fn num(v: &Value, k: &str) -> Result<Option<f32>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(x) => x.as_f64().filter(|f| f.is_finite()).map(|f| Some(f as f32)).ok_or_else(|| CmdError::Params(format!("`{k}` must be a number"))),
    }
}

/// `color`: a colour name, or null/absent for none.
fn color(v: &Value) -> Result<Option<CardColor>, CmdError> {
    match v.get("color") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(c)) if c.trim().is_empty() || c.eq_ignore_ascii_case("none") => Ok(None),
        Some(Value::String(c)) => CardColor::parse(c).map(Some).ok_or_else(|| CmdError::Params(format!("unknown colour `{c}`"))),
        Some(_) => Err(CmdError::Params("`color` must be a colour name".into())),
    }
}

fn beat_id(s: &Session, v: &Value) -> Result<u32, CmdError> {
    let id = p::req_usize(v, "id")?;
    let id = u32::try_from(id).map_err(|_| CmdError::Params(format!("no beat {id}")))?;
    s.script.board.get(id).map(|b| b.id).ok_or_else(|| CmdError::Params(format!("no beat {id}")))
}

fn scene_param(s: &Session, v: &Value, k: &str) -> Result<Option<usize>, CmdError> {
    match v.get(k) {
        Some(Value::Null) => Ok(None),
        None => Ok(s.script.scene_of(s.sel.focus.para)),
        Some(_) => {
            let i = p::req_usize(v, k)?;
            if i < s.script.scenes().len() { Ok(Some(i)) } else { Err(CmdError::Params(format!("no scene {i}"))) }
        }
    }
}

fn beat_json(s: &Session, id: u32) -> Value {
    let Some(b) = s.script.board.get(id) else { return Value::Null };
    let scene = s.script.beat_scene(id);
    let heading = scene.and_then(|i| s.script.scenes().get(i).map(|x| x.heading.clone()));
    json!({
        "id": b.id, "title": b.title, "text": b.text, "color": b.color.map(CardColor::id),
        "x": b.x, "y": b.y, "w": b.w, "h": b.h, "scene": scene, "heading": heading,
    })
}

fn beat_add(s: &mut Session, v: &Value) -> CmdResult {
    let title = p::str(v, "title").unwrap_or("");
    let text = p::str(v, "text").unwrap_or("");
    let c = color(v)?;
    let (fx, fy) = s.script.board.free_spot();
    let x = num(v, "x")?.unwrap_or(fx);
    let y = num(v, "y")?.unwrap_or(fy);
    let (w, h) = (num(v, "w")?, num(v, "h")?);
    let scene = match v.get("scene") {
        None | Some(Value::Null) => None,
        Some(_) => scene_param(s, v, "scene")?,
    };
    let id = s.script.board.add(title, text, c, x, y).ok_or_else(|| CmdError::Failed("the beat board is full".into()))?;
    if let Some(b) = s.script.board.get_mut(id) {
        b.w = w.unwrap_or(b.w);
        b.h = h.unwrap_or(b.h);
    }
    s.script.board.normalize();
    if scene.is_some() {
        s.script.link_beat(id, scene).map_err(|e| CmdError::Params(e.to_string()))?;
    }
    Ok(json!({"id": id, "beat": beat_json(s, id)}))
}

fn beat_edit(s: &mut Session, v: &Value) -> CmdResult {
    let id = beat_id(s, v)?;
    let c = if v.get("color").is_some() { Some(color(v)?) } else { None };
    let b = s.script.board.get_mut(id).ok_or_else(|| CmdError::Params(format!("no beat {id}")))?;
    if let Some(t) = p::str(v, "title") {
        b.title = scriptcraft_doc::clean_beat_title(t);
    }
    if let Some(t) = p::str(v, "text") {
        b.text = scriptcraft_doc::clean_beat_text(t);
    }
    if let Some(c) = c {
        b.color = c;
    }
    Ok(json!({"beat": beat_json(s, id)}))
}

fn beat_move(s: &mut Session, v: &Value) -> CmdResult {
    let id = beat_id(s, v)?;
    let (x, y, w, h) = (num(v, "x")?, num(v, "y")?, num(v, "w")?, num(v, "h")?);
    s.script.board.raise(id);
    if let Some(b) = s.script.board.get_mut(id) {
        b.x = x.unwrap_or(b.x);
        b.y = y.unwrap_or(b.y);
        b.w = w.unwrap_or(b.w);
        b.h = h.unwrap_or(b.h);
    }
    s.script.board.normalize();
    Ok(json!({"beat": beat_json(s, id)}))
}

fn beat_delete(s: &mut Session, v: &Value) -> CmdResult {
    let id = beat_id(s, v)?;
    s.script.board.remove(id);
    Ok(json!({"deleted": id}))
}

fn beat_list(s: &mut Session, _: &Value) -> CmdResult {
    let ids: Vec<u32> = s.script.board.beats.iter().map(|b| b.id).collect();
    Ok(json!({"beats": ids.into_iter().map(|id| beat_json(s, id)).collect::<Vec<_>>()}))
}

fn beat_link(s: &mut Session, v: &Value) -> CmdResult {
    let id = beat_id(s, v)?;
    let scene = scene_param(s, v, "scene")?;
    if v.get("scene").is_none() && scene.is_none() {
        return Err(CmdError::Params("the caret is not in a scene: give `scene`".into()));
    }
    s.script.link_beat(id, scene).map_err(|e| CmdError::Params(e.to_string()))?;
    Ok(json!({"beat": beat_json(s, id)}))
}

fn beat_goto(s: &mut Session, v: &Value) -> CmdResult {
    let id = beat_id(s, v)?;
    let para = s.script.beat_para(id).ok_or_else(|| CmdError::Failed("this beat is not linked to a scene".into()))?;
    go_para(s, para)
}

fn beat_to_scene(s: &mut Session, v: &Value) -> CmdResult {
    let id = beat_id(s, v)?;
    let scenes = s.script.scenes();
    let at = match v.get("after") {
        None | Some(Value::Null) => s.script.paras.len(),
        Some(_) => {
            let i = p::req_usize(v, "after")?;
            scenes.get(i).map(|sc| sc.end).ok_or_else(|| CmdError::Params(format!("no scene {i}")))?
        }
    };
    if s.script.paras.len() + 2 > scriptcraft_doc::MAX_PARAS {
        return Err(CmdError::Failed("the script would be too large".into()));
    }
    let (title, text, c) = s.script.board.get(id).map(|b| (b.label().to_string(), b.text.clone(), b.color)).unwrap_or_default();
    let mut head = scriptcraft_doc::Para::new(Element::SceneHeading, if title.trim().is_empty() { "INT. " } else { title.trim() });
    head.summary = scriptcraft_doc::sanitize(&text).split_whitespace().collect::<Vec<_>>().join(" ");
    head.color = c;
    let at = at.min(s.script.paras.len());
    s.script.paras.insert(at, head);
    s.script.paras.insert(at + 1, scriptcraft_doc::Para::new(Element::Action, ""));
    let scene = s.script.scenes().iter().position(|sc| sc.para == at);
    s.script.link_beat(id, scene).map_err(|e| CmdError::Params(e.to_string()))?;
    set_caret(s, Pos::new(at + 1, 0));
    Ok(json!({"scene": scene, "para": at, "beat": beat_json(s, id)}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn bookmarks_through_commands() {
        let mut s = Session::sample();
        assert!(s.run("bookmark.next", &json!({})).is_err());
        let v = s.run("bookmark.add", &json!({"para": 4, "name": "Radio room"})).unwrap();
        assert_eq!(v["name"], "Radio room");
        s.run("bookmark.add", &json!({"para": 1})).unwrap();
        let l = s.run("bookmark.list", &json!({})).unwrap();
        assert_eq!(l["bookmarks"].as_array().map(Vec::len), Some(2));
        assert_eq!(l["bookmarks"][1]["para"], 4);
        // Typing above moves the bookmark with its paragraph; undo takes it back.
        s.run("caret.set", &json!({"para": 0, "off": 0})).unwrap();
        s.run("text.enter", &json!({})).unwrap();
        s.run("bookmark.goto", &json!({"name": "radio ROOM"})).unwrap();
        assert_eq!(s.sel.focus.para, 5);
        s.run("bookmark.previous", &json!({})).unwrap();
        assert_eq!(s.sel.focus.para, 2);
        s.run("bookmark.next", &json!({})).unwrap();
        assert_eq!(s.sel.focus.para, 5);
        s.run("bookmark.rename", &json!({"name": "Radio room", "to": "Mayday"})).unwrap();
        s.run("bookmark.remove", &json!({"name": "mayday"})).unwrap();
        assert_eq!(s.script.bookmarks().len(), 1);
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.script.bookmark_para("Mayday"), Some(5));
        assert!(s.run("bookmark.add", &json!({"para": 99999})).is_err());
        assert!(s.run("bookmark.add", &json!({"name": "  "})).is_err());
        assert!(s.run("bookmark.goto", &json!({"name": "nope"})).is_err());
    }

    #[test]
    fn beats_through_commands() {
        let mut s = Session::sample();
        let a = s.run("beat.add", &json!({"title": "Mayday", "text": "A voice in the static.", "color": "red", "scene": 1})).unwrap();
        let a = a["id"].as_u64().unwrap();
        let b = s.run("beat.add", &json!({"title": "Rescue", "x": 500, "y": 40})).unwrap()["id"].as_u64().unwrap();
        assert_ne!(a, b);
        let l = s.run("beat.list", &json!({})).unwrap();
        assert_eq!(l["beats"][0]["scene"], 1);
        assert_eq!(l["beats"][1]["x"], 500.0);
        // Moving a scene keeps the link.
        s.run("scene.move", &json!({"from": 1, "to": 3})).unwrap();
        assert_eq!(s.script.beat_scene(a as u32), Some(3));
        s.run("beat.goto", &json!({"id": a})).unwrap();
        assert_eq!(s.script.para(s.sel.focus.para).map(|p| p.element), Some(scriptcraft_doc::Element::SceneHeading));
        assert!(s.run("beat.goto", &json!({"id": b})).is_err());
        s.run("beat.move", &json!({"id": a, "x": -20, "y": 30, "w": 300})).unwrap();
        assert_eq!(s.script.board.beats.last().map(|x| (x.id, x.x, x.w)), Some((a as u32, -20.0, 300.0)));
        s.run("beat.edit", &json!({"id": b, "text": "Cutter on the lee side.", "color": "teal"})).unwrap();
        let n = s.script.scenes().len();
        let v = s.run("beat.toScene", &json!({"id": b})).unwrap();
        assert_eq!(s.script.scenes().len(), n + 1);
        assert_eq!(v["scene"], n);
        assert_eq!(s.script.scenes()[n].summary, "Cutter on the lee side.");
        s.run("beat.link", &json!({"id": b, "scene": null})).unwrap();
        assert_eq!(s.script.beat_scene(b as u32), None);
        s.run("beat.delete", &json!({"id": a})).unwrap();
        s.run("edit.undo", &json!({})).unwrap();
        assert!(s.script.board.get(a as u32).is_some());
        for bad in [json!({"id": 999}), json!({"id": -1}), json!({"id": a, "x": "far"}), json!({"id": a, "color": "plaid"})] {
            assert!(s.run("beat.move", &bad).is_err() || s.run("beat.edit", &bad).is_err());
        }
        assert!(s.run("beat.add", &json!({"scene": 999})).is_err());
    }
}
