//! Revisions in FDX: `<Revisions ActiveSet RevisionMode>` holding one `<Revision ID Name Color
//! Mark>` per set, and a `RevisionID` attribute on each revised `<Text>` run. The set's date and
//! the show-marks / coloured-pages options are ScriptCraft-only attributes.

use scriptcraft_doc::revision::{RevisionSet, Revisions, parse_color};
use scriptcraft_doc::{Para, RevMark, Script, Style};

use crate::esc;

/// Text runs split by style and by revision: (text, style, set).
pub(crate) fn runs(p: &Para) -> Vec<(&str, Style, Option<u8>)> {
    if p.revs.is_empty() {
        return p.runs().into_iter().map(|(t, st)| (t, st, None)).collect();
    }
    let styles = p.styles();
    let rev_at = |i: usize| p.revs.iter().find(|m| m.start <= i && i < m.end).map(|m| m.level);
    let mut out: Vec<(&str, Style, Option<u8>)> = Vec::new();
    let mut start = 0;
    let mut cur: Option<(Style, Option<u8>)> = None;
    for (i, _) in p.text.char_indices() {
        let k = (styles.get(i).copied().unwrap_or_default(), rev_at(i));
        if cur.is_some_and(|c| c != k) {
            if let Some((st, r)) = cur {
                out.push((p.text.get(start..i).unwrap_or(""), st, r));
            }
            start = i;
        }
        cur = Some(k);
    }
    match cur {
        Some((st, r)) => out.push((p.text.get(start..).unwrap_or(""), st, r)),
        None => out.push(("", Style::PLAIN, None)),
    }
    out
}

/// `<Revisions>`, when the script has revision marks or settings other than the defaults.
pub(crate) fn write(out: &mut String, s: &Script) {
    let r = &s.settings.revision;
    if r.is_default() && s.paras.iter().all(|p| p.revs.is_empty()) {
        return;
    }
    let yn = |b: bool| if b { "Yes" } else { "No" };
    out.push_str(&format!(
        "  <Revisions ActiveSet=\"{}\" RevisionMode=\"{}\" ShowAllMarks=\"No\" ShowAllSets=\"No\" ScriptCraftShowMarks=\"{}\" ScriptCraftColorPages=\"{}\">\n",
        r.active,
        if r.tracking { "On" } else { "Off" },
        yn(r.show_marks),
        yn(r.color_pages)
    ));
    for (i, st) in r.sets.iter().enumerate() {
        let [cr, cg, cb] = st.color;
        out.push_str(&format!(
            "    <Revision Color=\"#{cr:02X}{cr:02X}{cg:02X}{cg:02X}{cb:02X}{cb:02X}\" FullRevision=\"No\" ID=\"{i}\" Mark=\"{}\" Name=\"{}\" ScriptCraftDate=\"{}\"/>\n",
            esc(&st.mark),
            esc(&st.name),
            esc(&st.date)
        ));
    }
    out.push_str("  </Revisions>\n");
}

/// What was read from `<Revisions>`.
#[derive(Default)]
pub(crate) struct Read {
    pub seen: bool,
    pub active_id: Option<u32>,
    pub tracking: Option<bool>,
    pub show_marks: Option<bool>,
    pub color_pages: Option<bool>,
    /// (ID, set) in file order.
    pub sets: Vec<(u32, RevisionSet)>,
}

impl Read {
    pub fn add(&mut self, id: Option<String>, name: Option<String>, color: Option<String>, mark: Option<String>, date: Option<String>) {
        if self.sets.len() >= scriptcraft_doc::revision::MAX_SETS {
            return;
        }
        let id = id.and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(self.sets.len() as u32);
        let mut st = RevisionSet::new(name.as_deref().unwrap_or("Revision"), color.and_then(|c| parse_color(&c)).unwrap_or([0xff, 0xff, 0xff]));
        if let Some(m) = mark {
            st.mark = m;
        }
        st.date = date.unwrap_or_default();
        self.sets.push((id, st));
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.sets.iter().position(|(i, _)| *i == id)
    }

    /// Turn the `RevisionID`s read into the marks (stored raw) into set indices.
    pub fn remap(&self, paras: &mut [Para]) {
        if self.sets.is_empty() {
            return;
        }
        for p in paras {
            for m in &mut p.revs {
                m.level = self.index_of(u32::from(m.level)).unwrap_or(0).min(usize::from(u8::MAX)) as u8;
            }
        }
    }

    /// Apply the settings to the script.
    pub fn apply(self, s: &mut Script) {
        if !self.seen {
            return;
        }
        let r: &mut Revisions = &mut s.settings.revision;
        if !self.sets.is_empty() {
            r.active = self.active_id.and_then(|id| self.index_of(id)).unwrap_or(r.active);
            r.sets = self.sets.into_iter().map(|(_, st)| st).collect();
        }
        if let Some(b) = self.tracking {
            r.tracking = b;
        }
        if let Some(b) = self.show_marks {
            r.show_marks = b;
        }
        if let Some(b) = self.color_pages {
            r.color_pages = b;
        }
        r.normalize();
    }
}

/// Add the byte range a revised run covers to a paragraph being read.
pub(crate) fn push_run(revs: &mut Vec<RevMark>, start: usize, end: usize, id: Option<u32>) {
    let Some(id) = id else { return };
    if end <= start {
        return;
    }
    let level = id.min(u32::from(u8::MAX)) as u8;
    match revs.last_mut() {
        Some(l) if l.level == level && l.end == start => l.end = end,
        _ => revs.push(RevMark { start, end, level }),
    }
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::revision::add_mark;
    use scriptcraft_doc::{Element, Flag, Para, Pos, Script};

    #[test]
    fn revisions_round_trip() {
        let mut s = Script::from_paras(vec![Para::new(Element::Action, "The door opens slowly."), Para::new(Element::Action, "Quiet.")]).unwrap();
        s.set_flag(Pos::new(0, 4), Pos::new(0, 8), Flag::Bold, true);
        add_mark(&mut s.paras[0], 6, 14, 2);
        add_mark(&mut s.paras[1], 0, 6, 1);
        let r = &mut s.settings.revision;
        r.tracking = true;
        r.active = 2;
        r.color_pages = true;
        r.sets[2].date = "10/12/26".into();
        let xml = crate::write(&s);
        assert!(xml.contains("RevisionID=\"2\""), "{xml}");
        assert!(xml.contains("<Revision Color=\"#F8F8D3D3DCDC\""));
        let back = crate::parse(&xml).unwrap();
        assert_eq!(back.paras, s.paras, "text, styles and marks");
        assert_eq!(back.settings.revision, s.settings.revision);
        // No revisions: nothing written.
        assert!(!crate::write(&Script::new()).contains("<Revisions"));
    }

    #[test]
    fn revision_ids_map_to_sets() {
        let xml = r##"<FinalDraft DocumentType="Script" Version="5"><Content>
<Paragraph Type="Action"><Text>Old </Text><Text RevisionID="7">new</Text></Paragraph></Content>
<Revisions ActiveSet="7" RevisionMode="OffWithRevisions"><Revision ID="3" Name="First" Color="#FFFFFFFFFFFF" Mark="*"/><Revision ID="7" Name="Second" Color="#9999CCCCFFFF" Mark="+"/></Revisions></FinalDraft>"##;
        let s = crate::parse(xml).unwrap();
        let r = &s.settings.revision;
        assert_eq!(r.sets.len(), 2);
        assert_eq!(r.active, 1);
        assert!(!r.tracking);
        assert_eq!(r.sets[1].mark, "+");
        assert_eq!(s.paras[0].revs, vec![scriptcraft_doc::RevMark { start: 4, end: 7, level: 1 }]);
    }
}
