//! The title page in FDX: `<TitlePage><Content>` holding free-form `<Paragraph Alignment>`s,
//! one per line, blank ones for spacing, in Final Draft's shape (`LeftIndent="1.00"`,
//! `RightIndent="7.50"`, no `Type`). Final Draft has no title page fields.
//!
//! ScriptCraft writes its fields in the usual arrangement and marks each line with the field it
//! came from (`ScriptCraftField="copyright"`), so copyright, notes and the contact block come
//! back as separate fields. Without those marks (a Final Draft file), the lines are mapped onto
//! fields when that loses nothing; otherwise they are kept as a free-form title page
//! ([`TitlePage::lines`]) and written back as they are.

use scriptcraft_doc::{Align, Paper, Style, TitleLine, TitlePage};

use crate::{esc, style_attr};

/// The Final Draft paragraph attributes of a title page line.
fn para_open(align: Align, paper: Paper, field: Option<&str>) -> String {
    let right = if paper == Paper::A4 { "7.27" } else { "7.50" };
    let field = field.map(|f| format!(" ScriptCraftField=\"{f}\"")).unwrap_or_default();
    format!(
        "      <Paragraph Alignment=\"{}\" FirstIndent=\"0.00\" Leading=\"Regular\" LeftIndent=\"1.00\" RightIndent=\"{right}\" SpaceBefore=\"0\" Spacing=\"1\" StartsNewPage=\"No\"{field}>\n",
        align.name()
    )
}

/// `<TitlePage>`, when there is one.
pub(crate) fn write(out: &mut String, t: &TitlePage, paper: Paper) {
    if t.is_empty() {
        return;
    }
    out.push_str("  <TitlePage>\n    <Content>\n");
    let lines: Vec<(TitleLine, Option<&str>)> = if t.is_free_form() { t.lines.iter().map(|l| (l.clone(), None)).collect() } else { t.field_lines() };
    for (l, field) in lines {
        out.push_str(&para_open(l.align, paper, field));
        out.push_str(&format!("        <Text{}>{}</Text>\n      </Paragraph>\n", style_attr(l.style), esc(l.text.trim())));
    }
    out.push_str("    </Content>\n  </TitlePage>\n");
}

/// Title page lines read so far.
#[derive(Default)]
pub(crate) struct Read {
    lines: Vec<(TitleLine, Option<String>)>,
}

impl Read {
    /// A title page paragraph: its alignment, text runs and `ScriptCraftField`.
    pub fn add(&mut self, align: &str, runs: &[(String, Style)], field: Option<String>) {
        if self.lines.len() >= scriptcraft_doc::MAX_TITLE_LINES * 2 {
            return;
        }
        let text: String = runs.iter().map(|(t, _)| t.as_str()).collect();
        let styles: Vec<Style> = runs.iter().filter(|(t, _)| !t.trim().is_empty()).map(|(_, s)| *s).collect();
        let style = match styles.first() {
            Some(s) if styles.iter().all(|x| x == s) => *s,
            _ => Style::PLAIN,
        };
        let align = Align::parse(align).unwrap_or(Align::Left);
        self.lines.push((TitleLine { style, ..TitleLine::new(align, text.trim()) }, field));
    }

    pub fn apply(self) -> TitlePage {
        if self.lines.iter().any(|(_, f)| f.is_some()) {
            // Written by ScriptCraft: each marked line belongs to its field.
            let mut t = TitlePage::default();
            for (l, f) in &self.lines {
                if let Some(f) = f
                    && !l.is_blank()
                {
                    let v = t.get(f).map(|cur| if cur.is_empty() { l.text.clone() } else { format!("{cur}\n{}", l.text) });
                    if let Some(v) = v {
                        t.set(f, &v);
                    }
                }
            }
            return t;
        }
        TitlePage::from_lines(self.lines.into_iter().map(|(l, _)| l).collect())
    }
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Align, Element, Para, Script, Style, TitleLine, TitlePage};

    fn script(t: TitlePage) -> Script {
        let mut s = Script::from_paras(vec![Para::new(Element::Action, "A.")]).unwrap();
        s.title = t;
        s
    }

    #[test]
    fn every_field_round_trips_separately() {
        let t = TitlePage {
            title: "NIGHT SIGNAL".into(),
            credit: "Written by".into(),
            author: "Ada Lane\nBo Reyes".into(),
            source: "Based on a story by Cy".into(),
            draft_date: "Draft 2\nMay 2026".into(),
            contact: "ada@example.com\n555-0100".into(),
            copyright: "(c) 2026 Ada Lane".into(),
            notes: "WGA Registered".into(),
            lines: Vec::new(),
        };
        let xml = crate::write(&script(t.clone()));
        assert!(xml.contains("ScriptCraftField=\"copyright\""), "{xml}");
        assert!(!xml.contains("Type=\"Title Page\""), "Final Draft's title page paragraphs have no Type");
        assert_eq!(crate::parse(&xml).unwrap().title, t);
    }

    #[test]
    fn free_form_title_pages_round_trip() {
        // A Final Draft title page that doesn't fit the fields: a contact line at the top and
        // an underlined title.
        let xml = r#"<FinalDraft DocumentType="Script" Version="5"><Content><Paragraph Type="Action"><Text>A.</Text></Paragraph></Content>
<TitlePage><HeaderAndFooter HeaderVisible="Yes"><Header><Paragraph Alignment="Right"><Text>X</Text><DynamicLabel Type="Page #"/></Paragraph></Header></HeaderAndFooter>
<Content>
<Paragraph Alignment="Left" LeftIndent="1.00" RightIndent="7.50"><Text>Agent: Bo</Text></Paragraph>
<Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"/>
<Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"><Text Style="Underline+AllCaps">night signal</Text></Paragraph>
<Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"><Text>by</Text></Paragraph>
<Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"><Text>Ada </Text><Text>Lane</Text></Paragraph>
<Paragraph Alignment="Full" LeftIndent="1.00" RightIndent="7.50"><Text>WGA 123</Text></Paragraph>
</Content></TitlePage></FinalDraft>"#;
        let s = crate::parse(xml).unwrap();
        let t = &s.title;
        assert!(t.is_free_form());
        let underline = Style { underline: true, ..Style::PLAIN };
        assert_eq!(
            t.lines,
            vec![
                TitleLine::new(Align::Left, "Agent: Bo"),
                TitleLine::new(Align::Center, ""),
                TitleLine { style: underline, ..TitleLine::new(Align::Center, "NIGHT SIGNAL") },
                TitleLine::new(Align::Center, "by"),
                TitleLine::new(Align::Center, "Ada Lane"),
                TitleLine::new(Align::Left, "WGA 123"),
            ]
        );
        assert_eq!((t.title.as_str(), t.author.as_str(), t.contact.as_str()), ("NIGHT SIGNAL", "Ada Lane", "Agent: Bo\nWGA 123"));
        // The title page's own header is not the script's.
        assert!(!s.settings.page.header.right.contains('X'), "{:?}", s.settings.page.header);
        // Written back as it is.
        let back = crate::parse(&crate::write(&s)).unwrap();
        assert_eq!(back.title, s.title);
    }

    #[test]
    fn usual_final_draft_title_pages_become_fields() {
        let xml = r#"<FinalDraft DocumentType="Script" Version="5"><Content/><TitlePage><Content>
<Paragraph Alignment="Center"><Text>THE TITLE</Text></Paragraph><Paragraph Alignment="Center"/>
<Paragraph Alignment="Center"><Text>Written by</Text></Paragraph><Paragraph Alignment="Center"><Text>Ada Lane</Text></Paragraph>
<Paragraph Alignment="Left"><Text>ada@example.com</Text></Paragraph><Paragraph Alignment="Right"><Text>May 2026</Text></Paragraph>
</Content></TitlePage></FinalDraft>"#;
        let t = crate::parse(xml).unwrap().title;
        assert!(!t.is_free_form(), "{t:?}");
        assert_eq!((t.title.as_str(), t.credit.as_str(), t.author.as_str()), ("THE TITLE", "Written by", "Ada Lane"));
        assert_eq!((t.contact.as_str(), t.draft_date.as_str()), ("ada@example.com", "May 2026"));
    }
}
