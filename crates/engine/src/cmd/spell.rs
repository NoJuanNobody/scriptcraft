//! Spelling: offline check, suggestions, ignore and the user dictionary.
//!
//! Character names, locations and title-page words count as known, so the cast and places of
//! the script are never flagged.

use std::collections::HashSet;

use scriptcraft_doc::{Element, Pos, Script};
use scriptcraft_spell::{english, key, words};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    const L: &str = "Tools › Spelling";
    vec![
        CommandSpec::new("spell.check", "Check Spelling", L, check).params(r#"{"para": n (optional: one paragraph)}"#).pure(),
        CommandSpec::new("spell.next", "Next Misspelling", L, next).key("Mod+;").pure(),
        CommandSpec::new("spell.suggest", "Spelling Suggestions", L, suggest).params(r#"{"word": string, "count": n}"#).pure(),
        CommandSpec::new("spell.ignore", "Ignore Word", L, ignore).params(r#"{"word": string (default: the selection)}"#).pure(),
        CommandSpec::new("spell.addWord", "Add to Dictionary", L, add_word).params(r#"{"word": string (default: the selection)}"#).pure(),
    ]
}

/// Words the script itself vouches for: character names, locations, title page.
pub fn script_words(s: &Script) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut add = |t: &str| {
        for (a, b) in words(t) {
            if let Some(w) = t.get(a..b) {
                out.insert(key(w));
            }
        }
    };
    for p in &s.paras {
        if matches!(p.element, Element::Character | Element::SceneHeading) {
            add(&p.text);
        }
    }
    for f in scriptcraft_doc::TitlePage::FIELDS {
        add(s.title.get(f).unwrap_or(""));
    }
    out
}

/// Misspellings of the script: (paragraph, start, end).
pub fn misspellings(s: &Session, only: Option<usize>) -> Vec<(usize, usize, usize)> {
    let known = script_words(&s.script);
    let ok = |w: &str| known.contains(w) || s.ignored_words.contains(w) || s.user_words.contains(w);
    let mut out = Vec::new();
    for (i, p) in s.script.paras.iter().enumerate() {
        if only.is_some_and(|o| o != i) {
            continue;
        }
        for (a, b) in english().misspelled(&p.text, &ok) {
            out.push((i, a, b));
        }
    }
    out
}

fn check(s: &mut Session, v: &Value) -> CmdResult {
    let list = misspellings(s, p::usize(v, "para"));
    Ok(json!({
        "count": list.len(),
        "misspellings": list.iter().take(5000).map(|&(i, a, b)| json!({
            "para": i, "start": a, "end": b,
            "word": s.script.para(i).and_then(|p| p.text.get(a..b)).unwrap_or(""),
        })).collect::<Vec<_>>(),
    }))
}

fn next(s: &mut Session, _: &Value) -> CmdResult {
    let after = s.sel.ordered().1;
    let list = misspellings(s, None);
    let hit = list.iter().find(|&&(i, a, _)| Pos::new(i, a) >= after).or_else(|| list.first()).copied();
    let (i, a, b) = hit.ok_or_else(|| CmdError::Failed("no misspellings".into()))?;
    s.sel = Selection { anchor: Pos::new(i, a), focus: Pos::new(i, b) };
    let word = s.script.para(i).and_then(|p| p.text.get(a..b)).unwrap_or("").to_string();
    Ok(json!({"para": i, "start": a, "end": b, "word": word, "suggestions": english().suggest(&word, 8)}))
}

fn suggest(s: &mut Session, v: &Value) -> CmdResult {
    let word = p::req_str(v, "word")?;
    let n = p::usize(v, "count").unwrap_or(8).clamp(1, 50);
    let _ = s;
    Ok(json!({"word": word, "correct": english().check(word), "suggestions": english().suggest(word, n)}))
}

/// The `word` param, or the selected text.
fn word_param(s: &Session, v: &Value) -> Result<String, CmdError> {
    let w = match p::str(v, "word") {
        Some(w) => w.to_string(),
        None => {
            let (a, b) = s.sel.ordered();
            s.script.text_between(a, b)
        }
    };
    let w = key(w.trim());
    if w.is_empty() || w.len() > 64 || w.contains(char::is_whitespace) {
        return Err(CmdError::Params("`word` (a single word) is required".into()));
    }
    Ok(w)
}

fn ignore(s: &mut Session, v: &Value) -> CmdResult {
    let w = word_param(s, v)?;
    s.ignored_words.insert(w.clone());
    Ok(json!({"ignored": w}))
}

fn add_word(s: &mut Session, v: &Value) -> CmdResult {
    let w = word_param(s, v)?;
    s.user_words.insert(w.clone());
    Ok(json!({"added": w, "userWords": s.user_words}))
}
