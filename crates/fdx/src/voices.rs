//! Read-aloud voices in FDX, in Final Draft's speech form (see `docs/fdx-format.md`):
//!
//! ```xml
//! <Cast>
//!   <Narrator Actor="Samantha"><Element Type="Character"/><Element Type="Dialogue"/></Narrator>
//!   <Member Actor="Daniel" Character="NELL"/>
//! </Cast>
//! <Actors>
//!   <Actor MacVoice="Daniel" Name="Daniel" Pitch="Normal" Speed="Medium" WinVoice=""/>
//! </Actors>
//! ```
//!
//! Final Draft maps each character (`Member`) and the narrator to an actor, and each actor to a
//! system voice (`MacVoice` / `WinVoice`; empty is the actor's default voice). ScriptCraft
//! writes one actor per voice, named after it, with the voice in `MacVoice`, and the
//! narrator's skipped elements as Final Draft's default (Character, Dialogue). Reading, an
//! actor with no `MacVoice` (Final Draft's generic "Man 1"…) is no assigned voice. Whether
//! Final Draft expects `MacVoice` to be the plain voice name is not confirmed (every public
//! file leaves it empty). Older ScriptCraft files'
//! `<ScriptCraftVoices Narrator><Voice Character Name/></ScriptCraftVoices>` still read.

use std::collections::{BTreeMap, BTreeSet};

use scriptcraft_doc::{Script, Voices};

use crate::esc;

pub(crate) fn write(out: &mut String, s: &Script) {
    let v = &s.settings.voices;
    if v.is_default() {
        return;
    }
    out.push_str("  <Cast>\n");
    if let Some(n) = &v.narrator {
        out.push_str(&format!(
            "    <Narrator Actor=\"{}\">\n      <Element Type=\"Character\"/>\n      <Element Type=\"Dialogue\"/>\n    </Narrator>\n",
            esc(n)
        ));
    }
    for (character, voice) in &v.cast {
        out.push_str(&format!("    <Member Actor=\"{}\" Character=\"{}\"/>\n", esc(voice), esc(character)));
    }
    out.push_str("  </Cast>\n  <Actors>\n");
    let actors: BTreeSet<&String> = v.narrator.iter().chain(v.cast.values()).collect();
    for a in actors {
        out.push_str(&format!("    <Actor MacVoice=\"{0}\" Name=\"{0}\" Pitch=\"Normal\" Speed=\"Medium\" WinVoice=\"\"/>\n", esc(a)));
    }
    out.push_str("  </Actors>\n");
}

/// Voices read so far.
#[derive(Default)]
pub(crate) struct Read {
    /// `<ScriptCraftVoices>` (older ScriptCraft files).
    voices: Voices,
    /// `<Narrator Actor>`.
    narrator_actor: Option<String>,
    /// `<Member Actor Character>`.
    members: Vec<(String, String)>,
    /// Actor name → its `MacVoice`.
    actors: BTreeMap<String, String>,
}

impl Read {
    pub(crate) fn narrator(&mut self, name: Option<String>) {
        self.voices.narrator = name;
    }
    pub(crate) fn add(&mut self, character: Option<String>, name: Option<String>) {
        if let (Some(c), Some(n)) = (character, name)
            && self.voices.cast.len() < scriptcraft_doc::voices::MAX_CAST_VOICES
        {
            self.voices.cast.insert(c, n);
        }
    }
    /// `<Cast><Narrator Actor>`.
    pub(crate) fn cast_narrator(&mut self, actor: Option<String>) {
        self.narrator_actor = actor;
    }
    /// `<Cast><Member Actor Character>`.
    pub(crate) fn member(&mut self, actor: Option<String>, character: Option<String>) {
        if let (Some(a), Some(c)) = (actor, character)
            && self.members.len() < scriptcraft_doc::voices::MAX_CAST_VOICES
        {
            self.members.push((c, a));
        }
    }
    /// `<Actors><Actor Name MacVoice>`.
    pub(crate) fn actor(&mut self, name: Option<String>, mac_voice: Option<String>) {
        if let (Some(n), Some(v)) = (name, mac_voice)
            && !v.trim().is_empty()
            && self.actors.len() < scriptcraft_doc::voices::MAX_CAST_VOICES
        {
            self.actors.insert(n, v);
        }
    }
    pub(crate) fn apply(mut self, s: &mut Script) {
        let voice_of = |actor: &str| self.actors.get(actor).cloned();
        if self.voices.narrator.is_none() {
            self.voices.narrator = self.narrator_actor.as_deref().and_then(voice_of);
        }
        for (character, actor) in &self.members {
            if let Some(v) = voice_of(actor)
                && self.voices.cast.len() < scriptcraft_doc::voices::MAX_CAST_VOICES
            {
                self.voices.cast.entry(character.clone()).or_insert(v);
            }
        }
        if !self.voices.is_default() {
            s.settings.voices = self.voices;
        }
    }
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Element, Para, Script};

    #[test]
    fn voices_round_trip() {
        let mut s = Script::from_paras(vec![Para::new(Element::Character, "NELL"), Para::new(Element::Dialogue, "Hi.")]).unwrap_or_default();
        s.settings.voices.narrator = Some("Samantha".into());
        s.settings.voices.cast.insert("NELL".into(), "Daniel & \"Co\"".into());
        s.settings.voices.cast.insert("COAST GUARD".into(), "Fred".into());
        s.settings.voices.cast.insert("TEO".into(), "Fred".into());
        let xml = crate::write(&s);
        assert!(xml.contains("<Narrator Actor=\"Samantha\">"), "{xml}");
        assert!(xml.contains("<Member Actor=\"Fred\" Character=\"TEO\"/>"));
        assert_eq!(xml.matches("<Actor MacVoice=\"Fred\" Name=\"Fred\"").count(), 1, "one actor per voice");
        assert!(!xml.contains("ScriptCraftVoices"));
        let back = crate::parse(&xml).unwrap_or_default();
        assert_eq!(back.settings.voices, s.settings.voices);
        // No voices: nothing written.
        let plain = crate::write(&Script::default());
        assert!(!plain.contains("<Cast>") && !plain.contains("ScriptCraftVoices"));
    }

    #[test]
    fn reads_final_draft_cast_and_older_files() {
        // Final Draft's roster: generic actors with default voices (no MacVoice) are no
        // assigned voice; one actor given a Mac voice is.
        let xml = r#"<FinalDraft><Content/><Cast><Narrator Actor="Man 1"><Element Type="Character"/><Element Type="Dialogue"/></Narrator>
<Member Actor="Man 1" Character="BEN"/><Member Actor="Woman 1" Character="ADA"/><Member Actor="" Character="CY"/></Cast>
<Actors><Actor MacVoice="" Name="Man 1" Pitch="Normal" Speed="Medium" WinVoice=""/><Actor MacVoice="Karen" Name="Woman 1" Pitch="Normal" Speed="Medium" WinVoice=""/></Actors></FinalDraft>"#;
        let v = crate::parse(xml).unwrap_or_default().settings.voices;
        assert_eq!(v.narrator, None);
        assert_eq!(v.cast.len(), 1);
        assert_eq!(v.cast.get("ADA").map(String::as_str), Some("Karen"));
        // v0.4 ScriptCraft files.
        let junk = "<FinalDraft><Content><Paragraph Type=\"Action\"><Text>a</Text></Paragraph></Content><ScriptCraftVoices Narrator=\"  \"><Voice Character=\"nell (v.o.)\" Name=\" X \"/><Voice Name=\"Y\"/><Voice Character=\"Z\"/></ScriptCraftVoices></FinalDraft>";
        let back = crate::parse(junk).unwrap_or_default();
        assert_eq!(back.settings.voices.narrator, None);
        assert_eq!(back.settings.voices.cast.len(), 1);
        assert_eq!(back.settings.voices.cast.get("NELL").map(String::as_str), Some("X"));
    }
}
