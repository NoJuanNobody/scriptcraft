//! Production locks: locked pages (A/B pages), locked scene numbers (12A) and omitted scenes.

use scriptcraft_doc::{Element, expand_page_label};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("page.lock", "Lock Pages", "Production › Lock Pages", lock_pages)
            .when(|s| if s.script.settings.pages_locked { Some("pages are already locked") } else { None }),
        CommandSpec::new("page.relock", "Re-lock Pages", "Production › Lock Pages", lock_pages)
            .when(|s| if s.script.settings.pages_locked { None } else { Some("pages are not locked") }),
        CommandSpec::new("page.unlock", "Unlock Pages", "Production › Lock Pages", unlock_pages)
            .when(|s| if s.script.settings.pages_locked { None } else { Some("pages are not locked") }),
        CommandSpec::new("page.list", "Pages", "Production › Lock Pages", list_pages).pure(),
        CommandSpec::new("scene.lockNumbers", "Lock Scene Numbers", "Production › Scene Numbers", lock_numbers),
        CommandSpec::new("scene.unlockNumbers", "Unlock Scene Numbers", "Production › Scene Numbers", unlock_numbers),
        CommandSpec::new("scene.omit", "Omit Scene", "Production › Omit Scenes", omit)
            .params(r#"{"index": n (0-based; default: the caret's scene), "omit": bool (default true)}"#),
        CommandSpec::new("scene.unomit", "Restore Omitted Scene", "Production › Omit Scenes", unomit)
            .params(r#"{"index": n (0-based; default: the caret's scene)}"#),
    ]
}

/// Lock (or re-lock) every page break where it is now.
fn lock_pages(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let starts = scriptcraft_layout::unit_starts(&s.script);
    for p in &mut s.script.paras {
        p.page_lock.clear();
    }
    let mut anchor: Option<usize> = None;
    let mut pages = 0usize;
    for page in l.pages.iter().filter(|p| !p.title_page) {
        pages += 1;
        let labels = expand_page_label(&page.label);
        let first = page.lines.iter().find(|x| x.kind == scriptcraft_layout::LineKind::Body && x.para.is_some());
        let starts_unit =
            first.and_then(|x| Some((x.para?, x.start))).filter(|&(para, start)| start == 0 && starts.get(para).copied().unwrap_or(false));
        match (starts_unit, anchor) {
            (Some((para, _)), _) if Some(para) != anchor => {
                if let Some(pa) = s.script.paras.get_mut(para) {
                    pa.page_lock = labels;
                }
                anchor = Some(para);
            }
            (_, Some(a)) => {
                if let Some(pa) = s.script.paras.get_mut(a) {
                    pa.page_lock.extend(labels);
                }
            }
            (_, None) => {}
        }
    }
    s.script.settings.pages_locked = true;
    s.status = format!("Locked {pages} pages");
    Ok(json!({"pages": pages}))
}

fn unlock_pages(s: &mut Session, _: &Value) -> CmdResult {
    for p in &mut s.script.paras {
        p.page_lock.clear();
    }
    s.script.settings.pages_locked = false;
    Ok(json!({}))
}

fn list_pages(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let labels: Vec<&str> = l.pages.iter().filter(|p| !p.title_page).map(|p| p.label.as_str()).collect();
    Ok(json!({"locked": s.script.settings.pages_locked, "pages": labels}))
}

fn lock_numbers(s: &mut Session, _: &Value) -> CmdResult {
    s.script.settings.scene_numbers_locked = true;
    s.script.settings.show_scene_numbers = true;
    s.script.fill_locked_numbers();
    Ok(json!({"scenes": s.script.scenes().len()}))
}

fn unlock_numbers(s: &mut Session, _: &Value) -> CmdResult {
    s.script.settings.scene_numbers_locked = false;
    Ok(json!({}))
}

/// The scene from `index`, or the caret's scene.
fn scene_index(s: &Session, v: &Value) -> Result<usize, CmdError> {
    match v.get("index") {
        None | Some(Value::Null) => s.script.scene_of(s.sel.focus.para).ok_or_else(|| CmdError::Disabled("the caret is not in a scene".into())),
        Some(_) => p::req_usize(v, "index"),
    }
}

fn omit(s: &mut Session, v: &Value) -> CmdResult {
    let index = scene_index(s, v)?;
    let on = p::bool(v, "omit").unwrap_or(true);
    let para = s.script.set_omitted(index, on)?;
    if on {
        // The caret leaves the hidden text for the heading.
        if s.script.hidden(s.sel.focus.para) || s.sel.focus.para == para {
            super::set_caret(s, scriptcraft_doc::Pos::new(para, 0));
        }
        let heading = s.script.paras.get(para).filter(|p| p.element == Element::SceneHeading).and_then(|p| p.scene_number.clone());
        s.status = match heading {
            Some(n) => format!("Scene {n} omitted"),
            None => "Scene omitted".into(),
        };
    }
    Ok(json!({"index": index, "para": para, "omitted": on}))
}

fn unomit(s: &mut Session, v: &Value) -> CmdResult {
    let index = scene_index(s, v)?;
    let para = s.script.set_omitted(index, false)?;
    Ok(json!({"index": index, "para": para, "omitted": false}))
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Element, Para, Pos, Script};
    use serde_json::json;

    use crate::Session;

    fn long_script() -> Script {
        let mut v = Vec::new();
        for sc in 0..12 {
            v.push(Para::new(Element::SceneHeading, format!("INT. ROOM {sc} - DAY")));
            for k in 0..6 {
                v.push(Para::new(Element::Action, format!("Scene {sc} beat {k}. Something happens in the room and keeps on happening.")));
            }
        }
        Script::from_paras(v).unwrap()
    }

    fn labels(s: &mut Session) -> Vec<String> {
        s.run("page.list", &json!({})).unwrap()["pages"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn locking_keeps_the_layout() {
        let mut s = Session::new(long_script());
        let before = s.layout();
        s.run("page.lock", &json!({})).unwrap();
        assert!(s.script.settings.pages_locked);
        assert_eq!(*s.layout(), *before, "locking changes nothing on the page");
        assert!(s.run("page.lock", &json!({})).is_err(), "already locked");
        s.run("page.unlock", &json!({})).unwrap();
        assert!(s.script.paras.iter().all(|p| p.page_lock.is_empty()));
        assert!(s.run("page.unlock", &json!({})).is_err());
    }

    #[test]
    fn new_text_makes_ab_pages_and_cut_text_combines_pages() {
        let mut s = Session::new(long_script());
        let n = labels(&mut s).len();
        assert!(n >= 4);
        s.run("page.lock", &json!({})).unwrap();
        // A long insertion on page 1 pushes text onto 1A, 1B; page 2 still starts where it did.
        let page2 = s.layout().pages[1].lines.iter().find_map(|l| l.para).unwrap();
        let first2 = s.script.paras[page2].text.clone();
        s.sel = crate::Selection::caret(Pos::new(1, 0));
        for k in 0..30 {
            s.run("text.insert", &json!({"text": format!("Added beat {k}. ")})).unwrap();
            s.run("text.enter", &json!({})).unwrap();
        }
        let l = labels(&mut s);
        assert_eq!(l.first().map(String::as_str), Some("1"));
        assert_eq!(l.get(1).map(String::as_str), Some("1A"));
        assert!(l.contains(&"2".to_string()));
        assert_eq!(l.len(), n + l.iter().filter(|x| x.ends_with('A') || x.ends_with('B')).count());
        let lay = s.layout();
        let idx2 = lay.pages.iter().position(|p| p.label == "2").unwrap();
        let shown = lay.pages[idx2].lines.iter().find_map(|x| x.para).unwrap();
        assert_eq!(s.script.paras[shown].text, first2, "page 2 still starts with the same text");
        // The printed number is the label.
        assert!(lay.pages[1].lines.iter().any(|x| x.kind == scriptcraft_layout::LineKind::Header && x.text.contains("1A.")));
        // Go to Page takes printed labels.
        s.run("caret.gotoPage", &json!({"page": "1A."})).unwrap();
        assert_eq!(s.layout().caret(s.sel.focus).map(|c| c.page), Some(1));
        assert!(s.run("caret.gotoPage", &json!({"page": "99Z"})).is_err());
        // Print takes and reports printed labels too.
        let v = s.run("print.pages", &json!({"from": "1A", "to": "1A.", "titlePage": false})).unwrap();
        assert_eq!(v["pages"], json!(["1A"]));
        let v = s.run("print.pages", &json!({"from": 1, "to": "2", "titlePage": false})).unwrap();
        assert_eq!(v["pages"].as_array().map(|a| a.first().cloned()), Some(Some(json!("1"))));
        assert!(v["pages"].as_array().is_some_and(|a| a.contains(&json!("1A")) && a.last() == Some(&json!("2"))));
        assert!(s.run("print.pages", &json!({"from": "99Z"})).is_err());
        // Re-locking keeps the A pages as locked pages.
        s.run("page.relock", &json!({})).unwrap();
        assert_eq!(labels(&mut s), l);
        // Deleting most of page 2's text: it shrinks, the numbers it lost stay with it.
        s.run("edit.undo", &json!({})).unwrap();
        let mut s2 = Session::new(long_script());
        s2.run("page.lock", &json!({})).unwrap();
        let lay = s2.layout();
        let start3 = lay.pages[2].lines.iter().find_map(|x| x.para).unwrap();
        let start4 = lay.pages[3].lines.iter().find_map(|x| x.para).unwrap();
        // Delete everything from the start of page 3 to the start of page 4.
        s2.sel = crate::Selection { anchor: Pos::new(start3, 0), focus: Pos::new(start4, 0) };
        s2.run("text.delete", &json!({})).unwrap();
        let l2 = labels(&mut s2);
        assert!(l2.iter().any(|x| x.contains('-')), "{l2:?}");
        assert!(!l2.contains(&"3".to_string()) || !l2.contains(&"4".to_string()));
    }

    #[test]
    fn locked_scene_numbers_and_omitted_scenes() {
        let mut s = Session::new(long_script());
        s.run("scene.lockNumbers", &json!({})).unwrap();
        assert!(s.script.settings.scene_numbers_locked && s.script.settings.show_scene_numbers);
        assert_eq!(s.script.scenes()[2].number.as_deref(), Some("3"));
        assert!(s.run("scene.numberAll", &json!({})).is_err(), "renumbering is off while locked");
        // A new scene after scene 3 becomes 3A.
        let at = s.script.scenes()[3].para;
        s.sel = crate::Selection::caret(Pos::new(at - 1, s.script.paras[at - 1].text.len()));
        s.run("text.enter", &json!({})).unwrap();
        s.run("element.set", &json!({"element": "sceneHeading"})).unwrap();
        s.run("text.insert", &json!({"text": "ext. yard - day"})).unwrap();
        assert_eq!(s.script.scenes()[3].number.as_deref(), Some("3A"));
        assert_eq!(s.script.scenes()[4].number.as_deref(), Some("4"));
        // Omit scene 5: prints its number and OMITTED, its text is kept and hidden.
        let para = s.script.scenes()[5].para;
        s.run("scene.omit", &json!({"index": 5})).unwrap();
        assert!(s.script.paras[para].omitted);
        let l = s.layout();
        let line = l.pages.iter().flat_map(|p| p.lines.iter()).find(|x| x.para == Some(para)).unwrap();
        assert_eq!(line.display(), "OMITTED");
        assert!(l.pages.iter().flat_map(|p| p.lines.iter()).any(|x| x.kind == scriptcraft_layout::LineKind::SceneNumber && x.text == "5"));
        assert!(!l.pages.iter().flat_map(|p| p.lines.iter()).any(|x| x.para == Some(para + 1)), "omitted text is not shown");
        assert_eq!(s.script.scenes()[6].number.as_deref(), Some("6"));
        s.run("scene.unomit", &json!({"index": 5})).unwrap();
        assert!(!s.script.paras[para].omitted);
        s.run("edit.undo", &json!({})).unwrap();
        assert!(s.script.paras[para].omitted);
        s.run("scene.unlockNumbers", &json!({})).unwrap();
        s.run("scene.numberAll", &json!({})).unwrap();
        assert_eq!(s.script.scenes()[3].number.as_deref(), Some("4"));
    }

    #[test]
    fn locks_survive_fdx() {
        let mut s = Session::new(long_script());
        s.run("scene.lockNumbers", &json!({})).unwrap();
        s.run("page.lock", &json!({})).unwrap();
        s.sel = crate::Selection::caret(Pos::new(1, 0));
        for k in 0..20 {
            s.run("text.insert", &json!({"text": format!("Added beat {k}. ")})).unwrap();
            s.run("text.enter", &json!({})).unwrap();
        }
        s.run("scene.omit", &json!({"index": 4})).unwrap();
        let before = labels(&mut s);
        assert!(before.iter().any(|l| l == "1A"), "{before:?}");
        let back = scriptcraft_fdx::parse(&scriptcraft_fdx::write(&s.script)).unwrap();
        assert_eq!(back.paras, s.script.paras);
        let mut s2 = Session::new(back);
        assert_eq!(labels(&mut s2), before);
        assert_eq!(*s2.layout(), *s.layout());
    }
}
