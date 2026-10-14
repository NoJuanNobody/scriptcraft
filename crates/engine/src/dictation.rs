//! Dictation: turn spoken (transcribed) phrases into script text and element flow. Whatever does
//! the speech recognition (the browser, the operating system's dictation, whisper.cpp, an agent's
//! own speech-to-text) hands phrases to `dictation.insert`, which runs the normal commands, so
//! SmartType, element flow and undo work as if typed. One phrase is one undo step.
//!
//! Spoken commands (case-insensitive):
//! - **Elements**, at the start of a phrase or after a sentence ends: "new scene" / "scene
//!   heading" + heading ("interior kitchen night" → `INT. KITCHEN - NIGHT`), "character" + name,
//!   "parenthetical" + text, "dialogue", "action", "shot", "transition" + text; "cut to", "fade
//!   in", "fade out", "dissolve to", "smash cut to", "match cut to" become that transition.
//!   After a character's name (ended by a pause, punctuation or "says"), the next words are
//!   their dialogue; after a parenthetical, dialogue again; after a heading, action.
//! - **Paragraphs**: "new paragraph", "new line", "next paragraph" (Enter, with element flow).
//! - **Punctuation**, anywhere: "period" / "full stop", "comma", "question mark", "exclamation
//!   point/mark", "colon", "semicolon", "dash", "ellipsis" / "dot dot dot", "open/close paren",
//!   "open/close quote", "hyphen", "apostrophe s".
//! - **Corrections**: "undo that", "scratch that", "delete that" (undo the last step).
//! - **Escape**: "literally" + word types the word itself ("literally period").

use scriptcraft_doc::Element;

/// Longest phrase taken (bytes).
pub const MAX_PHRASE: usize = 20_000;

/// One thing a phrase asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Words to type (space-separated as needed).
    Text(String),
    /// Punctuation: the text, whether it attaches to the previous word, and whether the next word
    /// attaches to it.
    Punct { text: &'static str, glue_before: bool, glue_after: bool },
    /// Start a new paragraph of this element (or reuse the current one when it is empty).
    Start(Element),
    /// The next words go into a new paragraph of this element (unless a command comes first).
    Then(Element),
    /// Enter, with element flow.
    NewParagraph,
    /// Undo the last step.
    Undo,
}

/// What the words being read belong to.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Text,
    /// A character's name: ends at punctuation, "says", an element word or the phrase end.
    Name,
    /// A parenthetical: ends at a sentence end, "dialogue", an element word or the phrase end.
    Paren,
    /// A scene heading: like a name, but its words are rewritten (`interior` → `INT.`).
    Heading,
}

const TRANSITIONS: [(&[&str], &str); 7] = [
    (&["smash", "cut", "to"], "SMASH CUT TO:"),
    (&["match", "cut", "to"], "MATCH CUT TO:"),
    (&["cut", "to"], "CUT TO:"),
    (&["fade", "in"], "FADE IN:"),
    (&["fade", "out"], "FADE OUT."),
    (&["fade", "to", "black"], "FADE TO BLACK."),
    (&["dissolve", "to"], "DISSOLVE TO:"),
];

/// Spoken punctuation: words, text, glue before, glue after.
const PUNCT: [(&[&str], &str, bool, bool); 21] = [
    (&["question", "mark"], "?", true, false),
    (&["exclamation", "point"], "!", true, false),
    (&["exclamation", "mark"], "!", true, false),
    (&["full", "stop"], ".", true, false),
    (&["dot", "dot", "dot"], "...", true, false),
    (&["open", "paren"], "(", false, true),
    (&["close", "paren"], ")", true, false),
    (&["open", "parenthesis"], "(", false, true),
    (&["close", "parenthesis"], ")", true, false),
    (&["open", "quote"], "\"", false, true),
    (&["close", "quote"], "\"", true, false),
    (&["end", "quote"], "\"", true, false),
    (&["apostrophe", "s"], "'s", true, false),
    (&["period"], ".", true, false),
    (&["comma"], ",", true, false),
    (&["colon"], ":", true, false),
    (&["semicolon"], ";", true, false),
    (&["ellipsis"], "...", true, false),
    (&["dash"], " -", true, false),
    (&["hyphen"], "-", true, true),
    (&["unquote"], "\"", true, false),
];

/// Times of day that end a spoken scene heading (`kitchen night` → `KITCHEN - NIGHT`).
const TIMES: [&str; 12] = ["day", "night", "morning", "evening", "afternoon", "dawn", "dusk", "later", "continuous", "moments", "sunset", "sunrise"];

/// A word as matched: lowercase, without surrounding punctuation.
fn bare(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').to_lowercase()
}

/// Punctuation the recognizer put after a word (`Nell.` → `.`).
fn trailing(w: &str) -> &str {
    let keep = w.trim_end_matches(['.', ',', '?', '!', ':', ';']);
    w.get(keep.len()..).unwrap_or("")
}

fn ends_sentence(p: &str) -> bool {
    p.contains(['.', '?', '!', ':'])
}

/// Whether `words[i..]` starts with `pat`.
fn at(words: &[String], i: usize, pat: &[&str]) -> bool {
    pat.iter().enumerate().all(|(k, p)| words.get(i + k).is_some_and(|w| w == p))
}

/// An element command at `words[i]`: the element, how many words it takes, and the mode of the
/// words after it.
fn element_word(words: &[String], i: usize) -> Option<(Element, usize, Mode)> {
    let w = words.get(i)?.as_str();
    Some(match w {
        "new" if at(words, i, &["new", "scene"]) => (Element::SceneHeading, 2, Mode::Heading),
        "scene" if at(words, i, &["scene", "heading"]) => (Element::SceneHeading, 2, Mode::Heading),
        "slug" if at(words, i, &["slug", "line"]) => (Element::SceneHeading, 2, Mode::Heading),
        "slugline" => (Element::SceneHeading, 1, Mode::Heading),
        "character" => (Element::Character, 1, Mode::Name),
        "parenthetical" => (Element::Parenthetical, 1, Mode::Paren),
        "dialogue" | "dialog" => (Element::Dialogue, 1, Mode::Text),
        "action" => (Element::Action, 1, Mode::Text),
        "shot" => (Element::Shot, 1, Mode::Text),
        "transition" => (Element::Transition, 1, Mode::Text),
        _ => return None,
    })
}

/// Turn a phrase into steps.
pub fn interpret(phrase: &str) -> Vec<Step> {
    let phrase = phrase.get(..MAX_PHRASE).unwrap_or_else(|| {
        let mut end = MAX_PHRASE.min(phrase.len());
        while !phrase.is_char_boundary(end) {
            end -= 1;
        }
        phrase.get(..end).unwrap_or("")
    });
    let raw: Vec<&str> = phrase.split_whitespace().collect();
    let words: Vec<String> = raw.iter().map(|w| bare(w)).collect();
    let mut out: Vec<Step> = Vec::new();
    let mut buf: Vec<String> = Vec::new();
    let mut mode = Mode::Text;
    let mut boundary = true;
    let flush = |out: &mut Vec<Step>, buf: &mut Vec<String>, mode: Mode| {
        if buf.is_empty() {
            return;
        }
        let mut text = buf.join(" ");
        if mode == Mode::Heading {
            text = spoken_heading(buf);
        }
        buf.clear();
        out.push(Step::Text(text));
    };
    // Ending a name, parenthetical or heading: what the next words go into.
    let finish = |out: &mut Vec<Step>, buf: &mut Vec<String>, mode: &mut Mode| {
        flush(out, buf, *mode);
        match *mode {
            Mode::Name | Mode::Paren => out.push(Step::Then(Element::Dialogue)),
            Mode::Heading => out.push(Step::Then(Element::Action)),
            Mode::Text => {}
        }
        *mode = Mode::Text;
    };
    let mut i = 0;
    while let Some(w) = words.get(i) {
        let original = raw.get(i).copied().unwrap_or("");
        // "literally X": X as typed.
        if w == "literally"
            && let Some(next) = raw.get(i + 1)
        {
            buf.push((*next).to_string());
            boundary = ends_sentence(trailing(next));
            i += 2;
            continue;
        }
        // Corrections.
        if (w == "undo" || w == "scratch" || w == "delete") && words.get(i + 1).is_some_and(|n| n == "that") {
            finish(&mut out, &mut buf, &mut mode);
            out.push(Step::Undo);
            boundary = true;
            i += 2;
            continue;
        }
        // Paragraph breaks.
        if at(&words, i, &["new", "paragraph"]) || at(&words, i, &["new", "line"]) || at(&words, i, &["next", "paragraph"]) {
            flush(&mut out, &mut buf, mode);
            mode = Mode::Text;
            out.push(Step::NewParagraph);
            boundary = true;
            i += 2;
            continue;
        }
        // Element commands: at a boundary, or to end a name, parenthetical or heading.
        if boundary || mode != Mode::Text {
            if let Some((pat, text)) = TRANSITIONS.iter().find(|(pat, _)| at(&words, i, pat)) {
                finish(&mut out, &mut buf, &mut mode);
                out.push(Step::Start(Element::Transition));
                out.push(Step::Text((*text).to_string()));
                out.push(Step::Then(Element::SceneHeading));
                boundary = true;
                i += pat.len();
                continue;
            }
            if let Some((e, n, next)) = element_word(&words, i) {
                finish(&mut out, &mut buf, &mut mode);
                match next {
                    Mode::Text => out.push(Step::Then(e)),
                    _ => out.push(Step::Start(e)),
                }
                mode = next;
                boundary = next == Mode::Text;
                i += n;
                continue;
            }
        }
        // "says" ends a character's name.
        if mode == Mode::Name && (w == "says" || w == "said") {
            finish(&mut out, &mut buf, &mut mode);
            boundary = true;
            i += 1;
            continue;
        }
        // Spoken punctuation.
        if let Some((pat, text, before, after)) = PUNCT.iter().find(|(pat, ..)| at(&words, i, pat)) {
            if mode == Mode::Name || (mode == Mode::Paren && ends_sentence(text)) {
                finish(&mut out, &mut buf, &mut mode);
            } else {
                flush(&mut out, &mut buf, mode);
                out.push(Step::Punct { text, glue_before: *before, glue_after: *after });
            }
            boundary = ends_sentence(text);
            i += pat.len();
            continue;
        }
        // A word, with whatever punctuation the recognizer gave it.
        let tail = trailing(original);
        let word = original.get(..original.len() - tail.len()).unwrap_or(original);
        match mode {
            Mode::Name | Mode::Heading => {
                if !word.is_empty() {
                    buf.push(word.to_string());
                }
                if !tail.is_empty() {
                    finish(&mut out, &mut buf, &mut mode);
                }
            }
            Mode::Paren => {
                if ends_sentence(tail) {
                    if !word.is_empty() {
                        buf.push(word.to_string());
                    }
                    finish(&mut out, &mut buf, &mut mode);
                } else {
                    buf.push(original.to_string());
                }
            }
            Mode::Text => buf.push(original.to_string()),
        }
        boundary = ends_sentence(tail);
        i += 1;
    }
    finish(&mut out, &mut buf, &mut mode);
    out
}

/// A spoken heading in screenplay form: `interior kitchen night` → `INT. KITCHEN - NIGHT`.
fn spoken_heading(words: &[String]) -> String {
    let lower: Vec<String> = words.iter().map(|w| bare(w)).collect();
    let (prefix, skip) = if at(&lower, 0, &["interior", "exterior"]) || at(&lower, 0, &["int", "ext"]) {
        ("INT./EXT.", 2)
    } else if at(&lower, 0, &["exterior", "interior"]) {
        ("EXT./INT.", 2)
    } else {
        match lower.first().map(String::as_str) {
            Some("interior" | "int" | "inside") => ("INT.", 1),
            Some("exterior" | "ext" | "outside") => ("EXT.", 1),
            _ => ("", 0),
        }
    };
    let rest: Vec<&String> = words.iter().skip(skip).collect();
    let has_dash = rest.iter().any(|w| w.as_str() == "-" || w.as_str() == "--");
    let mut body: Vec<String> = rest.iter().map(|w| (*w).clone()).collect();
    if !has_dash && body.len() >= 2 && body.last().is_some_and(|w| TIMES.contains(&bare(w).as_str())) {
        let last = body.pop().unwrap_or_default();
        body.push("-".into());
        body.push(last);
    }
    let body = body.join(" ").to_uppercase();
    match (prefix.is_empty(), body.is_empty()) {
        (true, _) => body,
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix} {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::Step::*;
    use super::*;

    fn t(s: &str) -> Step {
        Text(s.into())
    }

    #[test]
    fn elements_and_flow() {
        assert_eq!(
            interpret("new scene interior kitchen night"),
            vec![Start(Element::SceneHeading), t("INT. KITCHEN - NIGHT"), Then(Element::Action)]
        );
        assert_eq!(
            interpret("Scene heading exterior interior car moving - day"),
            vec![Start(Element::SceneHeading), t("EXT./INT. CAR MOVING - DAY"), Then(Element::Action)]
        );
        assert_eq!(
            interpret("Character Nell. Don't you dare."),
            vec![Start(Element::Character), t("Nell"), Then(Element::Dialogue), t("Don't you dare.")]
        );
        assert_eq!(
            interpret("character coast guard says copy that period"),
            vec![
                Start(Element::Character),
                t("coast guard"),
                Then(Element::Dialogue),
                t("copy that"),
                Punct { text: ".", glue_before: true, glue_after: false }
            ]
        );
        assert_eq!(
            interpret("character Teo parenthetical quietly, to Nell. Is it over question mark"),
            vec![
                Start(Element::Character),
                t("Teo"),
                Then(Element::Dialogue),
                Start(Element::Parenthetical),
                t("quietly, to Nell"),
                Then(Element::Dialogue),
                t("Is it over"),
                Punct { text: "?", glue_before: true, glue_after: false }
            ]
        );
        assert_eq!(interpret("action the kettle screams"), vec![Then(Element::Action), t("the kettle screams")]);
        assert_eq!(interpret("cut to"), vec![Start(Element::Transition), t("CUT TO:"), Then(Element::SceneHeading)]);
        assert_eq!(interpret("new paragraph"), vec![NewParagraph]);
    }

    #[test]
    fn commands_only_where_they_can_be() {
        // "character" and "action" mid-sentence are words, "period" is punctuation anywhere.
        assert_eq!(interpret("the main character takes action"), vec![t("the main character takes action")]);
        assert_eq!(
            interpret("a quiet moment period then action she runs"),
            vec![t("a quiet moment"), Punct { text: ".", glue_before: true, glue_after: false }, t("then action she runs")]
        );
        assert_eq!(interpret("it was a literally period drama"), vec![t("it was a period drama")]);
        assert_eq!(interpret("hello scratch that goodbye"), vec![t("hello"), Undo, t("goodbye")]);
        assert_eq!(interpret(""), vec![]);
        assert_eq!(interpret("   \n "), vec![]);
        assert_eq!(
            interpret("open quote hi close quote"),
            vec![Punct { text: "\"", glue_before: false, glue_after: true }, t("hi"), Punct { text: "\"", glue_before: true, glue_after: false }]
        );
    }

    #[test]
    fn hostile_phrases() {
        for p in
            ["character", "parenthetical", "literally", "new", "undo", "😀 character 😀. 😀", &"x ".repeat(30_000), "character ,,,, .", "\u{0}\u{7}"]
        {
            let _ = interpret(p);
        }
        assert_eq!(interpret("character"), vec![Start(Element::Character), Then(Element::Dialogue)]);
    }
}
