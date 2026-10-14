//! Revisions in FDX, in Final Draft's form (see `docs/fdx-format.md`): `<Revisions ActiveSet
//! RevisionMode="Yes|No" ShowPageColor="Yes|No">` holding one `<Revision ID Name Color PageColor
//! Mark FullRevision Style>` per set, and `RevisionID` on each `<Text>` run.
//!
//! Final Draft numbers sets from 1 and writes `RevisionID="0"` on unrevised text; `Color` is the
//! revised text's colour and `PageColor` the page tint; `Name` is the whole label
//! (`Blue Rev. (10/12/26)`). ScriptCraft writes set `i` as `ID="i+1"` and keeps its exact name
//! and date in `ScriptCraftName` / `ScriptCraftDate`; the show-marks option stays
//! ScriptCraft-only (`ScriptCraftShowMarks`).
//!
//! Older ScriptCraft files (sets from `ID="0"`, the tint in `Color`, `RevisionMode="On…"`,
//! `ScriptCraftColorPages`) still read.

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
        "  <Revisions ActiveSet=\"{}\" RevisionMode=\"{}\" ShowAllMarks=\"No\" ShowAllSets=\"No\" ShowPageColor=\"{}\" ScriptCraftShowMarks=\"{}\">\n",
        r.active + 1,
        yn(r.tracking),
        yn(r.color_pages),
        yn(r.show_marks),
    ));
    for (i, st) in r.sets.iter().enumerate() {
        let [cr, cg, cb] = st.color;
        out.push_str(&format!(
            "    <Revision Color=\"#000000000000\" FullRevision=\"No\" ID=\"{}\" Mark=\"{}\" Name=\"{}\" PageColor=\"#{cr:02X}{cr:02X}{cg:02X}{cg:02X}{cb:02X}{cb:02X}\" Style=\"\" ScriptCraftName=\"{}\" ScriptCraftDate=\"{}\"/>\n",
            i + 1,
            esc(&st.mark),
            esc(&fd_name(st)),
            esc(&st.name),
            esc(&st.date)
        ));
    }
    out.push_str("  </Revisions>\n");
}

/// Final Draft's set name: the whole label, the date in parentheses (`Blue Rev. (10/12/26)`).
fn fd_name(st: &RevisionSet) -> String {
    let d = st.date.trim();
    if d.is_empty() { format!("{} Rev.", st.name.trim()) } else { format!("{} Rev. ({d})", st.name.trim()) }
}

/// A Final Draft set name → (name, date): `Blue Rev. (10/12/26)` → (`Blue`, `10/12/26`). The
/// template placeholder `(mm/dd/yy)` is no date.
fn split_fd_name(n: &str) -> (String, String) {
    let mut name = n.trim();
    let mut date = "";
    if let Some(open) = name.rfind('(')
        && name.ends_with(')')
    {
        date = name.get(open + 1..name.len() - 1).unwrap_or("").trim();
        name = name.get(..open).unwrap_or("").trim_end();
        if date.chars().all(|c| c.is_ascii_alphabetic() || c == '/') {
            date = "";
        }
    }
    for suffix in [" Rev.", " Rev", " Revision"] {
        if let Some(n) = name.strip_suffix(suffix) {
            name = n.trim_end();
            break;
        }
    }
    (name.to_string(), date.to_string())
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

/// One `<Revision>`'s attributes.
#[derive(Default)]
pub(crate) struct SetIn {
    pub id: Option<String>,
    pub name: Option<String>,
    pub color: Option<String>,
    pub page_color: Option<String>,
    pub mark: Option<String>,
    pub sc_name: Option<String>,
    pub sc_date: Option<String>,
}

/// `RevisionMode`: Final Draft's `Yes`/`No`, older ScriptCraft files' `On`/`Off…`.
pub(crate) fn mode(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("yes") || v.starts_with("On")
}

impl Read {
    pub fn add(&mut self, a: SetIn) {
        if self.sets.len() >= scriptcraft_doc::revision::MAX_SETS {
            return;
        }
        let id = a.id.and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(self.sets.len() as u32 + 1);
        // The page tint: Final Draft's `PageColor`, or `Color` in older ScriptCraft files.
        let tint = a.page_color.or(a.color).and_then(|c| parse_color(&c)).unwrap_or([0xff, 0xff, 0xff]);
        let (name, date) = match a.sc_name {
            Some(n) => (n, a.sc_date.unwrap_or_default()),
            None => {
                let (n, d) = split_fd_name(a.name.as_deref().unwrap_or("Revision"));
                (n, a.sc_date.unwrap_or(d))
            }
        };
        let mut st = RevisionSet::new(&name, tint);
        if let Some(m) = a.mark {
            st.mark = m;
        }
        st.date = date;
        self.sets.push((id, st));
    }

    /// Final Draft's `RevisionID="0"` is unrevised text, unless the file has a set 0 (older
    /// ScriptCraft files numbered sets from 0).
    pub fn zero_is_unrevised(&self) -> bool {
        self.seen && !self.sets.iter().any(|(id, _)| *id == 0)
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.sets.iter().position(|(i, _)| *i == id)
    }

    /// Turn the `RevisionID`s read into the marks (stored raw) into set indices.
    pub fn remap(&self, paras: &mut [Para]) {
        let zero_none = self.zero_is_unrevised() || self.sets.is_empty();
        for p in paras {
            if zero_none {
                p.revs.retain(|m| m.level != 0);
            }
            if self.sets.is_empty() {
                continue;
            }
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
        assert!(xml.contains("RevisionID=\"3\""), "set 2 is Final Draft's ID 3: {xml}");
        assert!(xml.contains("PageColor=\"#F8F8D3D3DCDC\""));
        assert!(xml.contains("RevisionMode=\"Yes\"") && xml.contains("ShowPageColor=\"Yes\""));
        assert!(xml.contains("Name=\"Pink Rev. (10/12/26)\""), "{xml}");
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
