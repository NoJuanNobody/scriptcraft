//! Fountain import and export, written from the public syntax description at
//! <https://fountain.io/syntax>.
//!
//! Supported: title page keys, scene headings (natural `INT.`/`EXT.`/`EST.`/`INT./EXT.`/`I/E`
//! and forced `.`), scene numbers (`#12A#`), action (forced `!`), characters (forced `@`, with
//! extensions), dual dialogue (`^`), parentheticals, dialogue, transitions (natural `… TO:` and
//! forced `>`), centered text (`>…<`, imported as Action), synopses (`=`, which become the scene's
//! index-card summary), sections (`#`, skipped), notes (`[[…]]`, removed, except ScriptCraft's
//! `[[color: …]]` and `[[tags: …]]` card notes under a heading), boneyard (`/* … */`,
//! removed), page breaks (`===`, ignored) and lyrics (`~`, imported as Action).
//!
//! Emphasis (`*italic*`, `**bold**`, `***bold italic***`, `_underline_`) becomes styled text, and
//! styled text is written back with markers (literal `*` and `_` are escaped).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod emphasis;

use scriptcraft_doc::{Element, MAX_PARAS, Para, Script, TitlePage};

/// Largest input we parse, in bytes (longer input is cut at a char boundary).
pub const MAX_INPUT: usize = 64 << 20;

/// Parse Fountain text. Never fails: anything unrecognised is Action.
pub fn parse(src: &str) -> Script {
    let mut cut = src.len().min(MAX_INPUT);
    while cut > 0 && !src.is_char_boundary(cut) {
        cut -= 1;
    }
    let src = src.get(..cut).unwrap_or("").replace("\r\n", "\n").replace('\r', "\n");
    // Index-card colour and tags travel as notes on their own line: `[[color: blue]]`,
    // `[[tags: chase, night]]`. They are kept (marked) before other notes are removed.
    let src = strip_between(&src, "/*", "*/")
        .split('\n')
        .map(|l| {
            let t = l.trim();
            match t.strip_prefix("[[").and_then(|x| x.strip_suffix("]]")) {
                Some(inner) if !inner.contains("[[") && (inner.trim_start().starts_with("color:") || inner.trim_start().starts_with("tags:")) => {
                    format!("\u{1}{}", inner.trim())
                }
                _ => l.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let src = strip_between(&src, "[[", "]]");
    let lines: Vec<&str> = src.split('\n').collect();
    let mut i = 0;
    let mut title = TitlePage::default();
    if lines.first().is_some_and(|l| title_key(l).is_some()) {
        i = parse_title_page(&lines, &mut title);
    }
    let mut paras: Vec<Para> = Vec::new();
    let mut prev_blank = true;
    let mut in_dialogue = false;
    while let Some(raw) = lines.get(i) {
        if paras.len() >= MAX_PARAS {
            break;
        }
        let line = raw.trim();
        let next_blank = lines.get(i + 1).is_none_or(|l| l.trim().is_empty() || l.starts_with('\u{1}'));
        i += 1;
        if let Some(meta) = line.strip_prefix('\u{1}') {
            if let Some(h) = paras.iter_mut().rev().find(|p| p.element == Element::SceneHeading) {
                if let Some(c) = meta.strip_prefix("color:") {
                    h.color = scriptcraft_doc::CardColor::parse(c);
                } else if let Some(t) = meta.strip_prefix("tags:") {
                    let mut all = std::mem::take(&mut h.tags);
                    all.extend(t.split(',').map(str::to_string));
                    h.tags = scriptcraft_doc::clean_tags(all.iter().map(String::as_str));
                }
            }
            continue;
        }
        if line.is_empty() {
            prev_blank = true;
            in_dialogue = false;
            continue;
        }
        let was_blank = std::mem::replace(&mut prev_blank, false);
        if in_dialogue {
            if line.starts_with('(') && raw.trim_start().ends_with(')') {
                paras.push(Para::new(Element::Parenthetical, line));
            } else if let Some(last) = paras.last_mut().filter(|p| p.element == Element::Dialogue) {
                last.text.push(' ');
                last.text.push_str(line);
            } else {
                paras.push(Para::new(Element::Dialogue, line));
            }
            continue;
        }
        // Forced elements.
        if let Some(rest) = line.strip_prefix('!') {
            push_action(&mut paras, rest.trim(), was_blank);
            continue;
        }
        if let Some(rest) = line.strip_prefix('@') {
            push_character(&mut paras, rest);
            in_dialogue = !next_blank;
            continue;
        }
        if line.starts_with('.') && !line.starts_with("..") {
            push_heading(&mut paras, line.get(1..).unwrap_or(""));
            continue;
        }
        if let Some(rest) = line.strip_prefix('>') {
            if let Some(centered) = rest.strip_suffix('<') {
                push_action(&mut paras, centered.trim(), true);
            } else {
                paras.push(Para::new(Element::Transition, rest.trim()));
            }
            continue;
        }
        if line.starts_with('=') {
            if line.chars().all(|c| c == '=') && line.len() >= 3 {
                continue; // page break
            }
            let syn = line.trim_start_matches('=').trim();
            if let Some(h) = paras.iter_mut().rev().find(|p| p.element == Element::SceneHeading) {
                if !h.summary.is_empty() {
                    h.summary.push(' ');
                }
                h.summary.push_str(syn);
            }
            continue;
        }
        if line.starts_with('#') {
            continue; // section
        }
        if let Some(rest) = line.strip_prefix('~') {
            push_action(&mut paras, rest.trim(), was_blank);
            continue;
        }
        // Natural elements.
        if was_blank && is_heading(line) {
            push_heading(&mut paras, line);
            continue;
        }
        if was_blank && next_blank && is_transition(line) {
            paras.push(Para::new(Element::Transition, line));
            continue;
        }
        if was_blank && !next_blank && is_character(line) {
            push_character(&mut paras, line);
            in_dialogue = true;
            continue;
        }
        push_action(&mut paras, line, was_blank);
    }
    for p in &mut paras {
        emphasis::apply(p);
    }
    let mut s = Script::from_paras(paras).unwrap_or_default();
    s.title = title;
    s
}

/// Remove everything between `open` and `close` (inclusive); an unclosed `open` runs to the end.
fn strip_between(s: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find(open) {
        out.push_str(rest.get(..a).unwrap_or(""));
        let after = rest.get(a + open.len()..).unwrap_or("");
        rest = match after.find(close) {
            Some(b) => after.get(b + close.len()..).unwrap_or(""),
            None => "",
        };
    }
    out.push_str(rest);
    out
}

fn title_key(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once(':')?;
    let k = k.trim();
    (!k.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') && k.len() <= 40 && k.chars().all(|c| c.is_alphanumeric() || c == ' '))
        .then_some((k, v.trim()))
}

/// Parse the title page; returns the index of the first body line.
fn parse_title_page(lines: &[&str], title: &mut TitlePage) -> usize {
    let mut i = 0;
    let mut key = String::new();
    let mut extra = String::new();
    let mut put = |title: &mut TitlePage, key: &str, v: &str| {
        let field = match key.to_lowercase().as_str() {
            "title" => "title",
            "credit" => "credit",
            "author" | "authors" => "author",
            "source" => "source",
            "draft date" | "date" => "draftDate",
            "contact" => "contact",
            "copyright" => "copyright",
            "notes" => "notes",
            _ => "",
        };
        if field.is_empty() {
            if !extra.is_empty() {
                extra.push('\n');
            }
            extra.push_str(v);
            return;
        }
        let cur = title.get(field).unwrap_or("").to_string();
        let joined = if cur.is_empty() { v.to_string() } else { format!("{cur}\n{v}") };
        title.set(field, &joined);
    };
    while let Some(l) = lines.get(i) {
        if l.trim().is_empty() {
            i += 1;
            break;
        }
        if let Some((k, v)) = title_key(l) {
            key = k.to_string();
            if !v.is_empty() {
                put(title, &key, v);
            }
        } else if !key.is_empty() {
            put(title, &key, l.trim());
        }
        i += 1;
    }
    if !extra.is_empty() {
        let notes = if title.notes.is_empty() { extra } else { format!("{}\n{extra}", title.notes) };
        title.notes = notes;
    }
    i
}

fn push_action(paras: &mut Vec<Para>, text: &str, new_para: bool) {
    if !new_para && let Some(last) = paras.last_mut().filter(|p| p.element == Element::Action) {
        last.text.push(' ');
        last.text.push_str(text);
        return;
    }
    paras.push(Para::new(Element::Action, text));
}

fn push_heading(paras: &mut Vec<Para>, text: &str) {
    let text = text.trim();
    let (text, number) = match text.strip_suffix('#').and_then(|t| t.rfind('#').map(|i| (t, i))) {
        Some((t, i)) => (t.get(..i).unwrap_or("").trim(), Some(t.get(i + 1..).unwrap_or("").trim().to_string())),
        None => (text, None),
    };
    let mut p = Para::new(Element::SceneHeading, text);
    p.scene_number = number.filter(|n| !n.is_empty());
    paras.push(p);
}

fn push_character(paras: &mut Vec<Para>, text: &str) {
    let t = text.trim();
    let (t, dual) = match t.strip_suffix('^') {
        Some(t) => (t.trim(), true),
        None => (t, false),
    };
    let mut p = Para::new(Element::Character, t);
    p.dual = dual;
    paras.push(p);
}

/// A natural scene heading: `INT.`, `EXT.`, `EST.`, `INT./EXT.`, `INT/EXT`, `I/E` followed by a
/// dot or space.
pub fn is_heading(line: &str) -> bool {
    let up = line.trim_start().to_uppercase();
    ["INT./EXT", "INT/EXT", "EXT./INT", "EXT/INT", "I/E", "INT", "EXT", "EST"]
        .iter()
        .any(|p| up.strip_prefix(p).is_some_and(|rest| rest.starts_with('.') || rest.starts_with(' ')))
}

fn has_lowercase(s: &str) -> bool {
    s.chars().any(char::is_lowercase)
}

/// A natural transition: all caps, ending in `TO:`.
pub fn is_transition(line: &str) -> bool {
    !has_lowercase(line) && line.ends_with("TO:")
}

/// A natural character cue: the name (before any extension) is all caps with a letter in it.
pub fn is_character(line: &str) -> bool {
    let name = line.trim_end_matches('^').split('(').next().unwrap_or("").trim();
    !name.is_empty() && name.chars().any(char::is_alphabetic) && !has_lowercase(name)
}

/// Write a script as Fountain.
pub fn write(s: &Script) -> String {
    let mut out = String::new();
    let t = &s.title;
    for (key, field) in [
        ("Title", "title"),
        ("Credit", "credit"),
        ("Author", "author"),
        ("Source", "source"),
        ("Draft date", "draftDate"),
        ("Contact", "contact"),
        ("Copyright", "copyright"),
        ("Notes", "notes"),
    ] {
        let v = t.get(field).unwrap_or("").trim();
        if v.is_empty() {
            continue;
        }
        if v.contains('\n') {
            out.push_str(&format!("{key}:\n"));
            for l in v.lines() {
                out.push_str(&format!("    {}\n", l.trim()));
            }
        } else {
            out.push_str(&format!("{key}: {v}\n"));
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    let mut prev: Option<Element> = None;
    for p in &s.paras {
        let encoded = emphasis::write(p);
        let text = encoded.trim();
        if text.is_empty() && p.summary.is_empty() {
            continue;
        }
        let in_block = matches!(prev, Some(Element::Character | Element::Parenthetical | Element::Dialogue));
        match p.element {
            Element::Parenthetical | Element::Dialogue if in_block => {
                if text.is_empty() {
                    continue;
                }
                let line = if p.element == Element::Parenthetical && !(text.starts_with('(') && text.ends_with(')')) {
                    format!("({text})")
                } else if p.element == Element::Dialogue && text.starts_with('(') && text.ends_with(')') {
                    // Would read back as a parenthetical.
                    format!("{text} ")
                } else {
                    text.to_string()
                };
                out.push_str(line.trim_end_matches(' '));
                if line.ends_with(' ') {
                    out.push(' ');
                }
                out.push('\n');
            }
            Element::SceneHeading => {
                blank(&mut out);
                if !is_heading(text) {
                    out.push('.');
                }
                out.push_str(text);
                if let Some(n) = p.scene_number.as_deref().filter(|n| !n.trim().is_empty()) {
                    out.push_str(&format!(" #{}#", n.trim()));
                }
                out.push('\n');
                if !p.summary.trim().is_empty() {
                    out.push('\n');
                    out.push_str(&format!("= {}\n", p.summary.trim()));
                }
                if let Some(c) = p.color {
                    out.push_str(&format!("\n[[color: {}]]\n", c.id()));
                }
                if !p.tags.is_empty() {
                    out.push_str(&format!("\n[[tags: {}]]\n", p.tags.join(", ")));
                }
            }
            Element::Character => {
                blank(&mut out);
                if !is_character(text) || text.starts_with('!') {
                    out.push('@');
                }
                out.push_str(text);
                if p.dual {
                    out.push_str(" ^");
                }
                out.push('\n');
            }
            Element::Transition => {
                blank(&mut out);
                if !is_transition(text) {
                    out.push('>');
                }
                out.push_str(text);
                out.push('\n');
            }
            Element::Action | Element::General | Element::Shot | Element::Parenthetical | Element::Dialogue => {
                if text.is_empty() {
                    continue;
                }
                blank(&mut out);
                if needs_force(text) {
                    out.push('!');
                }
                out.push_str(text);
                out.push('\n');
            }
        }
        prev = Some(p.element);
    }
    out
}

/// Action that would read back as another element (or a forcing mark) gets `!`.
fn needs_force(text: &str) -> bool {
    is_heading(text)
        || is_transition(text)
        || is_character(text)
        || text.starts_with(['!', '@', '.', '>', '=', '#', '~'])
        || text.starts_with("[[")
        || text.starts_with("/*")
}

fn blank(out: &mut String) {
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
}

#[cfg(test)]
mod tests;
