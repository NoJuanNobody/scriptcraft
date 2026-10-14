//! Read aloud: the script as a list of utterances, each with the voice that reads it. Characters
//! speak their dialogue in their own voice; a narrator reads action and, optionally, scene
//! headings and transitions. Only what prints is read: the active alternate line, and nothing of
//! an omitted scene. The app plays the plan with the system's offline voices (`say`, SAPI,
//! espeak-ng, the browser's speech synthesis); agents get the plan itself.

use std::collections::BTreeMap;

use scriptcraft_doc::{Element, Script, voices::character_key};
use serde::Serialize;

/// Most utterances in one plan.
pub const MAX_UTTERANCES: usize = 20_000;

/// What to read.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// First and last paragraph (inclusive).
    pub from: usize,
    pub to: usize,
    pub headings: bool,
    pub action: bool,
    pub parentheticals: bool,
    pub transitions: bool,
    /// The narrator says each character's name before their line.
    pub character_names: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { from: 0, to: usize::MAX, headings: true, action: true, parentheticals: false, transitions: false, character_names: false }
    }
}

/// One thing to say.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Utterance {
    /// The paragraph it comes from.
    pub para: usize,
    pub element: &'static str,
    /// The speaking character (cue name), or `None` for the narrator.
    pub character: Option<String>,
    /// The text to say, cleaned up for speech (`INT.` → `Interior.`).
    pub text: String,
    /// The voice to use; `None` = the engine's default.
    pub voice: Option<String>,
}

/// A plan: the utterances and the voices chosen.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Plan {
    pub utterances: Vec<Utterance>,
    pub narrator: Option<String>,
    /// Character → voice, in order of first appearance.
    pub cast: Vec<(String, Option<String>)>,
}

/// Characters who speak, in order of first appearance (cue names without extensions).
pub fn characters(script: &Script) -> Vec<String> {
    let mut seen = Vec::new();
    for p in &script.paras {
        if p.element == Element::Character {
            let k = character_key(&p.text);
            if !k.is_empty() && !seen.contains(&k) {
                seen.push(k);
            }
        }
    }
    seen
}

/// Voices for the narrator and each character. Stored choices win when the engine has them (or
/// when `available` is empty: nothing to check against). Everyone else gets a different voice
/// from `available`, in order of first appearance, avoiding the narrator's.
pub fn assign(script: &Script, available: &[String], default_narrator: Option<&str>) -> (Option<String>, Vec<(String, Option<String>)>) {
    let usable = |v: &str| available.is_empty() || available.iter().any(|a| a == v);
    let stored = &script.settings.voices;
    let narrator = stored
        .narrator
        .clone()
        .filter(|v| usable(v))
        .or_else(|| default_narrator.filter(|v| usable(v)).map(str::to_string))
        .or_else(|| available.first().cloned());
    let chars = characters(script);
    let mut used: Vec<String> = narrator.iter().cloned().collect();
    let mut cast: Vec<(String, Option<String>)> = Vec::with_capacity(chars.len());
    for c in &chars {
        let v = stored.cast.get(c).filter(|v| usable(v)).cloned();
        if let Some(v) = &v {
            used.push(v.clone());
        }
        cast.push((c.clone(), v));
    }
    let mut pool: Vec<&String> = available.iter().filter(|v| !used.contains(v)).collect();
    if pool.is_empty() {
        pool = available.iter().filter(|v| Some(*v) != narrator.as_ref()).collect();
    }
    let mut k = 0;
    for (_, v) in cast.iter_mut().filter(|(_, v)| v.is_none()) {
        if let Some(x) = pool.get(k % pool.len().max(1)) {
            *v = Some((*x).clone());
            k += 1;
        } else {
            v.clone_from(&narrator);
        }
    }
    (narrator, cast)
}

/// The utterances for paragraphs `o.from..=o.to`, with voices from [`assign`].
pub fn plan(script: &Script, o: &Options, available: &[String], default_narrator: Option<&str>) -> Plan {
    let (narrator, cast) = assign(script, available, default_narrator);
    let voice_of = |name: &str| cast.iter().find(|(c, _)| c == name).and_then(|(_, v)| v.clone());
    let mut out = Vec::new();
    let mut speaker: Option<String> = None;
    let mut omitted = false;
    for (i, p) in script.paras.iter().enumerate() {
        if p.element == Element::SceneHeading {
            omitted = p.omitted;
        }
        if i > o.to || out.len() >= MAX_UTTERANCES {
            break;
        }
        match p.element {
            Element::Character => speaker = Some(character_key(&p.text)),
            Element::Dialogue | Element::Parenthetical => {}
            _ => speaker = None,
        }
        if i < o.from || omitted {
            continue;
        }
        let narrate = |text: String| Utterance { para: i, element: p.element.id(), character: None, text, voice: narrator.clone() };
        let u = match p.element {
            Element::SceneHeading if o.headings => Some(narrate(heading(&p.text))),
            Element::Action | Element::General | Element::Shot if o.action => Some(narrate(p.text.trim().to_string())),
            Element::Transition if o.transitions => Some(narrate(sentence(&p.text.trim().trim_end_matches(':').to_lowercase()))),
            Element::Character if o.character_names => {
                let name = character_key(&p.text);
                Some(narrate(sentence(&name.to_lowercase())))
            }
            Element::Parenthetical if o.parentheticals => {
                Some(narrate(p.text.trim().trim_start_matches('(').trim_end_matches(')').trim().to_string()))
            }
            Element::Dialogue => {
                let who = speaker.clone();
                let voice = who.as_deref().and_then(voice_of).or_else(|| narrator.clone());
                Some(Utterance { para: i, element: p.element.id(), character: who, text: p.text.trim().to_string(), voice })
            }
            _ => None,
        };
        if let Some(u) = u.filter(|u| u.text.chars().any(char::is_alphanumeric)) {
            out.push(u);
        }
    }
    Plan { utterances: out, narrator, cast }
}

/// A scene heading as it is said: `INT. KITCHEN - NIGHT` → `Interior. Kitchen, night.`
pub fn heading(t: &str) -> String {
    let t = t.trim();
    let upper = t.to_uppercase();
    const PREFIXES: [(&str, &str); 9] = [
        ("INT./EXT.", "Interior, exterior."),
        ("EXT./INT.", "Exterior, interior."),
        ("INT/EXT", "Interior, exterior."),
        ("I/E.", "Interior, exterior."),
        ("I/E", "Interior, exterior."),
        ("INT.", "Interior."),
        ("EXT.", "Exterior."),
        ("EST.", "Establishing."),
        ("INT ", "Interior."),
    ];
    let (lead, rest) = PREFIXES
        .iter()
        .find(|(p, _)| upper.starts_with(p))
        .map(|(p, said)| (Some(*said), t.get(p.len()..).unwrap_or("")))
        .or_else(|| upper.starts_with("EXT ").then(|| (Some("Exterior."), t.get(4..).unwrap_or(""))))
        .unwrap_or((None, t));
    let body = rest.trim().trim_start_matches('.').trim().replace(" - ", ", ").replace(" -- ", ", ").to_lowercase();
    let body = sentence(&body);
    match lead {
        Some(l) if body.is_empty() => l.to_string(),
        Some(l) => format!("{l} {body}"),
        None => body,
    }
}

/// Capitalize the first letter and end with a full stop.
fn sentence(t: &str) -> String {
    let t = t.trim();
    let mut c = t.chars();
    let Some(first) = c.next() else { return String::new() };
    let mut s: String = first.to_uppercase().chain(c).collect();
    if !s.ends_with(['.', '!', '?']) {
        s.push('.');
    }
    s
}

/// The cast as a map (for JSON).
pub fn cast_map(cast: &[(String, Option<String>)]) -> BTreeMap<String, Option<String>> {
    cast.iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Alternate, Para};

    use super::*;

    fn script() -> Script {
        let mut s = Script::from_paras(vec![
            Para::new(Element::SceneHeading, "INT. WEATHER STATION - RADIO ROOM - NIGHT"),
            Para::new(Element::Action, "Rain on the glass."),
            Para::new(Element::Character, "NELL (V.O.)"),
            Para::new(Element::Parenthetical, "(quietly)"),
            Para::new(Element::Dialogue, "Don't you dare."),
            Para::new(Element::Character, "TEO"),
            Para::new(Element::Dialogue, "Is it over?"),
            Para::new(Element::Transition, "CUT TO:"),
            Para::new(Element::SceneHeading, "EXT. HESPER ROCK - NIGHT"),
            Para::new(Element::Action, "This scene is omitted."),
            Para::new(Element::Character, "NELL"),
            Para::new(Element::Dialogue, "Hidden."),
            Para::new(Element::SceneHeading, "EXT. ROOF - DAWN"),
            Para::new(Element::Action, "   "),
        ])
        .unwrap_or_default();
        s.paras[8].omitted = true;
        s
    }

    #[test]
    fn plan_reads_what_prints() {
        let s = script();
        let p = plan(&s, &Options::default(), &[], None);
        let said: Vec<(usize, Option<&str>, &str)> = p.utterances.iter().map(|u| (u.para, u.character.as_deref(), u.text.as_str())).collect();
        assert_eq!(
            said,
            vec![
                (0, None, "Interior. Weather station, radio room, night."),
                (1, None, "Rain on the glass."),
                (4, Some("NELL"), "Don't you dare."),
                (6, Some("TEO"), "Is it over?"),
                (12, None, "Exterior. Roof, dawn."),
            ]
        );
        // Options: parentheticals, transitions and names; no headings or action.
        let o = Options { headings: false, action: false, parentheticals: true, transitions: true, character_names: true, ..Options::default() };
        let texts: Vec<String> = plan(&s, &o, &[], None).utterances.into_iter().map(|u| u.text).collect();
        assert_eq!(texts, vec!["Nell.", "quietly", "Don't you dare.", "Teo.", "Is it over?", "Cut to."]);
        // A range.
        let o = Options { from: 4, to: 6, ..Options::default() };
        assert_eq!(plan(&s, &o, &[], None).utterances.iter().map(|u| u.para).collect::<Vec<_>>(), vec![4, 6]);
    }

    #[test]
    fn only_the_active_alternate_is_read() {
        let mut s = script();
        s.paras[4].alternates.push(Alternate::plain("Not tonight."));
        let p = plan(&s, &Options::default(), &[], None);
        assert!(p.utterances.iter().any(|u| u.text == "Don't you dare."));
        assert!(!p.utterances.iter().any(|u| u.text.contains("tonight")));
    }

    #[test]
    fn voices_are_assigned() {
        let mut s = script();
        let avail: Vec<String> = ["Alex", "Daniel", "Karen", "Moira"].iter().map(|v| v.to_string()).collect();
        // Automatic: narrator first voice, characters the next ones in order.
        let p = plan(&s, &Options::default(), &avail, None);
        assert_eq!(p.narrator.as_deref(), Some("Alex"));
        assert_eq!(p.cast, vec![("NELL".into(), Some("Daniel".into())), ("TEO".into(), Some("Karen".into()))]);
        assert_eq!(p.utterances.iter().find(|u| u.para == 6).and_then(|u| u.voice.as_deref()), Some("Karen"));
        assert_eq!(p.utterances.first().and_then(|u| u.voice.as_deref()), Some("Alex"));
        // Stored choices win; others avoid them.
        s.settings.voices.cast.insert("TEO".into(), "Daniel".into());
        s.settings.voices.narrator = Some("Moira".into());
        let p = plan(&s, &Options::default(), &avail, Some("Alex"));
        assert_eq!(p.narrator.as_deref(), Some("Moira"));
        assert_eq!(cast_map(&p.cast).get("TEO"), Some(&Some("Daniel".into())));
        assert_eq!(cast_map(&p.cast).get("NELL"), Some(&Some("Alex".into())));
        // A stored voice this engine lacks falls back; no voices at all keeps stored names.
        s.settings.voices.cast.insert("TEO".into(), "Zed".into());
        assert_ne!(cast_map(&plan(&s, &Options::default(), &avail, None).cast).get("TEO"), Some(&Some("Zed".into())));
        assert_eq!(cast_map(&plan(&s, &Options::default(), &[], None).cast).get("TEO"), Some(&Some("Zed".into())));
        // One voice only: everyone shares it.
        let one = vec!["Solo".to_string()];
        let p = plan(&Script::default(), &Options::default(), &one, None);
        assert_eq!(p.narrator.as_deref(), Some("Solo"));
    }

    #[test]
    fn headings_said_aloud() {
        assert_eq!(heading("INT./EXT. CAR - MOVING - DAY"), "Interior, exterior. Car, moving, day.");
        assert_eq!(heading("ext. beach"), "Exterior. Beach.");
        assert_eq!(heading("INT."), "Interior.");
        assert_eq!(heading("THE VOID"), "The void.");
        assert_eq!(heading(""), "");
        assert_eq!(heading("I/E VAN"), "Interior, exterior. Van.");
    }

    #[test]
    fn structure_points_and_beats_are_not_read() {
        use scriptcraft_doc::{StructureKind, StructurePoint};
        let mut s = script();
        s.add_structure(StructurePoint::new(StructureKind::Act, "Act Secret", "Summary secret.", None), Some(0));
        s.add_structure(StructurePoint::new(StructureKind::Section, "Tail secret", "", None), None);
        let id = s.board.add("Beat secret", "Beat text secret.", None, 0.0, 0.0).unwrap_or(0);
        s.link_beat(id, Some(0)).ok();
        s.set_bookmark(1, Some("Mark secret")).ok();
        let p = plan(&s, &Options::default(), &[], None);
        assert!(!p.utterances.is_empty());
        assert!(p.utterances.iter().all(|u| !u.text.to_lowercase().contains("secret")), "{:?}", p.utterances);
    }
}
