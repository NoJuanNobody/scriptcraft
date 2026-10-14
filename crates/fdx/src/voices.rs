//! Read-aloud voices in FDX. Final Draft's `<Cast>`/`<Actors>` blocks map characters to actors
//! (people), not to speech voices, so the voices use a ScriptCraft-only element:
//! `<ScriptCraftVoices Narrator="Samantha"><Voice Character="NELL" Name="Daniel"/></ScriptCraftVoices>`.

use scriptcraft_doc::{Script, Voices};

use crate::esc;

pub(crate) fn write(out: &mut String, s: &Script) {
    let v = &s.settings.voices;
    if v.is_default() {
        return;
    }
    let narrator = v.narrator.as_deref().map(|n| format!(" Narrator=\"{}\"", esc(n))).unwrap_or_default();
    out.push_str(&format!("  <ScriptCraftVoices{narrator}>\n"));
    for (character, voice) in &v.cast {
        out.push_str(&format!("    <Voice Character=\"{}\" Name=\"{}\"/>\n", esc(character), esc(voice)));
    }
    out.push_str("  </ScriptCraftVoices>\n");
}

/// Voices read so far.
#[derive(Default)]
pub(crate) struct Read {
    voices: Voices,
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
    pub(crate) fn apply(self, s: &mut Script) {
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
        let xml = crate::write(&s);
        assert!(xml.contains("<ScriptCraftVoices Narrator=\"Samantha\">"), "{xml}");
        let back = crate::parse(&xml).unwrap_or_default();
        assert_eq!(back.settings.voices, s.settings.voices);
        // No voices: nothing written; junk is cleaned.
        let plain = crate::write(&Script::default());
        assert!(!plain.contains("ScriptCraftVoices"));
        let junk = "<FinalDraft><Content><Paragraph Type=\"Action\"><Text>a</Text></Paragraph></Content><ScriptCraftVoices Narrator=\"  \"><Voice Character=\"nell (v.o.)\" Name=\" X \"/><Voice Name=\"Y\"/><Voice Character=\"Z\"/></ScriptCraftVoices></FinalDraft>";
        let back = crate::parse(junk).unwrap_or_default();
        assert_eq!(back.settings.voices.narrator, None);
        assert_eq!(back.settings.voices.cast.len(), 1);
        assert_eq!(back.settings.voices.cast.get("NELL").map(String::as_str), Some("X"));
    }
}
