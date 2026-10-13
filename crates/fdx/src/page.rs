//! Page setup in FDX: `<HeaderAndFooter>` with `<Header>`/`<Footer>` paragraphs, one per
//! non-empty slot (`Alignment` Left/Center/Right). `{page}` is written as
//! `<DynamicLabel Type="Page #"/>`; ScriptCraft's other fields as
//! `<DynamicLabel Type="ScriptCraft:title"/>` and so on. The paper size and the title-page
//! option are ScriptCraft-only attributes (`ScriptCraftPaper`, `ScriptCraftTitlePage`).

use scriptcraft_doc::{HeaderFooter, PageSetup, Paper};

use crate::esc;

/// The field a `DynamicLabel Type` stands for, as template text (`{page}`).
pub(crate) fn dynamic_label(ty: &str) -> Option<String> {
    let t = ty.trim();
    if t.eq_ignore_ascii_case("Page #") {
        return Some("{page}".into());
    }
    if t.eq_ignore_ascii_case("Last Revised") {
        return Some("{revdate}".into());
    }
    let f = t.strip_prefix("ScriptCraft:")?;
    matches!(f, "pages" | "title" | "author" | "date" | "revision" | "revname" | "revdate").then(|| format!("{{{f}}}"))
}

/// One slot's template as `<Text>` and `<DynamicLabel>` children.
fn slot_children(template: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    let text = |out: &mut String, t: &str| {
        if !t.is_empty() {
            out.push_str(&format!("<Text>{}</Text>", esc(t)));
        }
    };
    while let Some(i) = rest.find('{') {
        let Some(j) = rest.get(i..).and_then(|r| r.find('}')).map(|j| i + j) else { break };
        let name = rest.get(i + 1..j).unwrap_or("").trim().to_ascii_lowercase();
        let label = match name.as_str() {
            "page" => Some("Page #".to_string()),
            "pages" | "title" | "author" | "date" | "revision" | "revname" | "revdate" => Some(format!("ScriptCraft:{name}")),
            _ => None,
        };
        match label {
            Some(l) => {
                text(&mut out, rest.get(..i).unwrap_or(""));
                out.push_str(&format!("<DynamicLabel Type=\"{}\"/>", esc(&l)));
            }
            None => text(&mut out, rest.get(..=j).unwrap_or("")),
        }
        rest = rest.get(j + 1..).unwrap_or("");
    }
    text(&mut out, rest);
    out
}

fn part(out: &mut String, tag: &str, hf: &HeaderFooter) {
    out.push_str(&format!("    <{tag}>\n"));
    for (align, t) in [("Left", &hf.left), ("Center", &hf.center), ("Right", &hf.right)] {
        if !t.trim().is_empty() {
            out.push_str(&format!("      <Paragraph Alignment=\"{align}\" Type=\"{tag}\">{}</Paragraph>\n", slot_children(t)));
        }
    }
    out.push_str(&format!("    </{tag}>\n"));
}

/// `<PageLayout>` and `<HeaderAndFooter>`.
pub(crate) fn write(out: &mut String, pg: &PageSetup) {
    let yn = |b: bool| if b { "Yes" } else { "No" };
    out.push_str(&format!(
        "  <PageLayout BottomMargin=\"72\" FooterMargin=\"36\" HeaderMargin=\"36\" TopMargin=\"72\" ScriptCraftPaper=\"{}\"/>\n",
        pg.paper.id()
    ));
    out.push_str(&format!(
        "  <HeaderAndFooter FooterFirstPage=\"{}\" FooterVisible=\"{}\" HeaderFirstPage=\"{}\" HeaderVisible=\"{}\" StartingPage=\"1\" ScriptCraftTitlePage=\"{}\">\n",
        yn(pg.footer_first_page),
        yn(!pg.footer.is_empty()),
        yn(pg.header_first_page),
        yn(!pg.header.is_empty()),
        yn(pg.header_on_title_page),
    ));
    part(out, "Header", &pg.header);
    part(out, "Footer", &pg.footer);
    out.push_str("  </HeaderAndFooter>\n");
}

/// What was read from `<PageLayout>` / `<HeaderAndFooter>`.
#[derive(Default)]
pub(crate) struct Read {
    pub seen: bool,
    pub paper: Option<Paper>,
    pub header_visible: Option<bool>,
    pub footer_visible: Option<bool>,
    pub header_first: Option<bool>,
    pub footer_first: Option<bool>,
    pub title_page: Option<bool>,
    pub header: HeaderFooter,
    pub footer: HeaderFooter,
}

pub(crate) fn yes(v: Option<String>) -> Option<bool> {
    v.map(|v| v.trim().eq_ignore_ascii_case("yes"))
}

impl Read {
    /// A header or footer paragraph: its alignment picks the slot (several in one slot join).
    pub fn add(&mut self, footer: bool, align: &str, text: &str) {
        let hf = if footer { &mut self.footer } else { &mut self.header };
        let slot = match align.to_ascii_lowercase().as_str() {
            "center" | "centre" => &mut hf.center,
            "right" => &mut hf.right,
            _ => &mut hf.left,
        };
        let t = text.replace('\t', " ");
        let t = t.trim();
        if t.is_empty() {
            return;
        }
        if !slot.is_empty() {
            slot.push(' ');
        }
        slot.push_str(t);
    }

    pub fn apply(self, pg: &mut PageSetup) {
        if let Some(p) = self.paper {
            pg.paper = p;
        }
        if !self.seen {
            return;
        }
        pg.header = if self.header_visible == Some(false) { HeaderFooter::default() } else { self.header };
        pg.footer = if self.footer_visible == Some(false) { HeaderFooter::default() } else { self.footer };
        if let Some(b) = self.header_first {
            pg.header_first_page = b;
        }
        if let Some(b) = self.footer_first {
            pg.footer_first_page = b;
        }
        if let Some(b) = self.title_page {
            pg.header_on_title_page = b;
        }
        pg.header.clean();
        pg.footer.clean();
    }
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Element, Para, Script};

    #[test]
    fn page_setup_round_trips() {
        let mut s = Script::from_paras(vec![Para::new(Element::Action, "Hello.")]).unwrap();
        let pg = &mut s.settings.page;
        pg.paper = scriptcraft_doc::Paper::A4;
        pg.header.left = "{title} & co".into();
        pg.header.right = "{revision}  {page}.".into();
        pg.footer.center = "Page {page} of {pages} {odd}".into();
        pg.header_first_page = true;
        pg.footer_first_page = false;
        pg.header_on_title_page = true;
        let xml = crate::write(&s);
        assert!(xml.contains("<DynamicLabel Type=\"Page #\"/>"), "{xml}");
        assert!(xml.contains("ScriptCraftPaper=\"a4\""));
        let back = crate::parse(&xml).unwrap();
        assert_eq!(back.settings.page, s.settings.page);
        assert_eq!(back.paras.len(), 1, "header paragraphs are not script paragraphs");
        // Defaults survive too.
        let d = crate::parse(&crate::write(&Script::new())).unwrap();
        assert!(d.settings.page.is_default());
    }

    #[test]
    fn reads_a_typical_header() {
        let xml = r#"<FinalDraft DocumentType="Script" Version="5"><Content><Paragraph Type="Action"><Text>A.</Text></Paragraph></Content>
<HeaderAndFooter FooterFirstPage="Yes" FooterVisible="No" HeaderFirstPage="No" HeaderVisible="Yes" StartingPage="1">
<Header><Paragraph Alignment="Right" Type="Header"><Text>	</Text><DynamicLabel Type="Page #"/><Text>.</Text></Paragraph></Header>
<Footer><Paragraph Type="Footer"><Text>ignored</Text></Paragraph></Footer></HeaderAndFooter></FinalDraft>"#;
        let s = crate::parse(xml).unwrap();
        assert_eq!(s.settings.page.header.right, "{page}.");
        assert_eq!(s.settings.page.header.left, "");
        assert!(s.settings.page.footer.is_empty(), "invisible footer");
        assert_eq!(s.paras.len(), 1);
    }
}
