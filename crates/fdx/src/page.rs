//! Page setup in FDX, in Final Draft's form (see `docs/fdx-format.md`):
//!
//! - Paper: `<PageLayout><PageSize Width="8.50" Height="11.00"/></PageLayout>` (inches; A4 is
//!   8.27 × 11.69). Older ScriptCraft files' `ScriptCraftPaper` still reads.
//! - Header and footer: one `<Paragraph Alignment="Left">` each, the three slots separated by
//!   tabs to a centre and a right `<Tabstop>`, as Final Draft writes its default header. Older
//!   ScriptCraft files (a paragraph per slot, by `Alignment`) still read.
//! - Fields: `{page}` is `<DynamicLabel Type="Page #"/>`, `{revision}` `Last Revised`; Final
//!   Draft's `Collated Revisions` reads as `{revision}` and `Date` as `{date}`. ScriptCraft's
//!   other fields are `<DynamicLabel Type="ScriptCraft:title"/>` and so on.
//! - The title-page option is ScriptCraft-only (`ScriptCraftTitlePage`). The title page's own
//!   `<HeaderAndFooter>` (inside `<TitlePage>`) is not read.

use scriptcraft_doc::{HeaderFooter, PageSetup, Paper};

use crate::esc;

/// The field a `DynamicLabel Type` stands for, as template text (`{page}`).
pub(crate) fn dynamic_label(ty: &str) -> Option<String> {
    let t = ty.trim();
    if t.eq_ignore_ascii_case("Page #") {
        return Some("{page}".into());
    }
    if t.eq_ignore_ascii_case("Last Revised") || t.eq_ignore_ascii_case("Collated Revisions") {
        return Some("{revision}".into());
    }
    if t.eq_ignore_ascii_case("Date") {
        return Some("{date}".into());
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
            "revision" => Some("Last Revised".to_string()),
            "pages" | "title" | "author" | "date" | "revname" | "revdate" => Some(format!("ScriptCraft:{name}")),
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

/// Tab stop positions (inches from the paper's left edge): centre and right, as Final Draft
/// places them on Letter (4.50, 7.25).
fn stops(paper: Paper) -> (f32, f32) {
    let (w, _) = size(paper);
    (w / 2.0 + 0.25, w - 1.25)
}

/// One header or footer line: the left slot, then a tab and the centre slot, then a tab and
/// the right slot.
fn part(out: &mut String, tag: &str, hf: &HeaderFooter, paper: Paper) {
    out.push_str(&format!("    <{tag}>\n"));
    if !hf.is_empty() {
        let mut body = slot_children(hf.left.trim());
        if !hf.center.trim().is_empty() || !hf.right.trim().is_empty() {
            body.push_str("<Text>\t</Text>");
            body.push_str(&slot_children(hf.center.trim()));
        }
        if !hf.right.trim().is_empty() {
            body.push_str("<Text>\t</Text>");
            body.push_str(&slot_children(hf.right.trim()));
        }
        let (c, r) = stops(paper);
        out.push_str(&format!(
            "      <Paragraph Alignment=\"Left\" FirstIndent=\"0.00\" Leading=\"Regular\" LeftIndent=\"1.25\" RightIndent=\"-1.00\" SpaceBefore=\"0\" Spacing=\"1\" StartsNewPage=\"No\">{body}<Tabstops><Tabstop Position=\"{c:.2}\" Type=\"Center\"/><Tabstop Position=\"{r:.2}\" Type=\"Right\"/></Tabstops></Paragraph>\n"
        ));
    }
    out.push_str(&format!("    </{tag}>\n"));
}

/// The paper's size in inches (Final Draft's `PageSize`).
pub(crate) fn size(paper: Paper) -> (f32, f32) {
    match paper {
        Paper::Letter => (8.5, 11.0),
        Paper::A4 => (8.27, 11.69),
    }
}

/// A `PageSize` → the nearest paper we have.
pub(crate) fn paper_of(width: Option<String>, height: Option<String>) -> Option<Paper> {
    let num = |v: Option<String>| v.and_then(|v| v.trim().parse::<f32>().ok()).filter(|x| x.is_finite() && *x > 0.0);
    let (w, h) = (num(width)?, num(height)?);
    let d = |p: Paper| {
        let (pw, ph) = size(p);
        (pw - w).abs() + (ph - h).abs()
    };
    Some(if d(Paper::A4) < d(Paper::Letter) { Paper::A4 } else { Paper::Letter })
}

/// `<PageLayout>` and `<HeaderAndFooter>`.
pub(crate) fn write(out: &mut String, pg: &PageSetup) {
    let yn = |b: bool| if b { "Yes" } else { "No" };
    let (w, h) = size(pg.paper);
    out.push_str(&format!(
        "  <PageLayout BottomMargin=\"72\" FooterMargin=\"36\" HeaderMargin=\"36\" TopMargin=\"72\">\n    <PageSize Height=\"{h:.2}\" Width=\"{w:.2}\"/>\n  </PageLayout>\n"
    ));
    out.push_str(&format!(
        "  <HeaderAndFooter FooterFirstPage=\"{}\" FooterVisible=\"{}\" HeaderFirstPage=\"{}\" HeaderVisible=\"{}\" StartingPage=\"1\" ScriptCraftTitlePage=\"{}\">\n",
        yn(pg.footer_first_page),
        yn(!pg.footer.is_empty()),
        yn(pg.header_first_page),
        yn(!pg.header.is_empty()),
        yn(pg.header_on_title_page),
    ));
    part(out, "Header", &pg.header, pg.paper);
    part(out, "Footer", &pg.footer, pg.paper);
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
    /// A header or footer paragraph: text before the first tab goes to the slot its alignment
    /// picks; text after each tab to the slot of that tab stop (`Center`, `Right`; without tab
    /// stops the last piece is the right slot and others the centre). Pieces in one slot join.
    pub fn add(&mut self, footer: bool, align: &str, text: &str, tab_stops: &[String]) {
        let hf = if footer { &mut self.footer } else { &mut self.header };
        let pieces: Vec<&str> = text.split('\t').collect();
        let n = pieces.len();
        for (k, piece) in pieces.into_iter().enumerate() {
            let kind = if k == 0 {
                align.to_ascii_lowercase()
            } else if let Some(stop) = tab_stops.get(k - 1) {
                stop.to_ascii_lowercase()
            } else if k + 1 == n {
                "right".into()
            } else {
                "center".into()
            };
            let slot = match kind.as_str() {
                "center" | "centre" => &mut hf.center,
                "right" => &mut hf.right,
                _ => &mut hf.left,
            };
            let t = piece.trim();
            if t.is_empty() {
                continue;
            }
            if !slot.is_empty() {
                slot.push(' ');
            }
            slot.push_str(t);
        }
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
        assert!(xml.contains("<PageSize Height=\"11.69\" Width=\"8.27\"/>"), "{xml}");
        assert!(xml.contains("<DynamicLabel Type=\"Last Revised\"/>"));
        assert_eq!(xml.matches("<Paragraph Alignment=\"Left\" FirstIndent").count(), 2, "one line each for the header and footer");
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

    #[test]
    fn reads_final_drafts_default_header() {
        // Final Draft's default: one left-aligned line, the revision label at a centre tab and
        // the page number at a right tab; the title page has a header of its own.
        let xml = "<FinalDraft DocumentType=\"Script\" Version=\"5\"><Content><Paragraph Type=\"Action\"><Text>A.</Text></Paragraph></Content>
<HeaderAndFooter FooterFirstPage=\"Yes\" FooterVisible=\"No\" HeaderFirstPage=\"No\" HeaderVisible=\"Yes\" StartingPage=\"1\">
<Header><Paragraph Alignment=\"Left\" LeftIndent=\"1.25\" RightIndent=\"-1.00\"><Text>\t</Text><DynamicLabel Type=\"Last Revised\"/><Text>\t</Text><DynamicLabel Type=\"Page #\"/><Text>.</Text>
<Tabstops><Tabstop Position=\"4.50\" Type=\"Center\"/><Tabstop Position=\"7.25\" Type=\"Right\"/></Tabstops></Paragraph></Header>
<Footer><Paragraph Alignment=\"Right\"><Text></Text></Paragraph></Footer></HeaderAndFooter>
<PageLayout TopMargin=\"72\"><PageSize Height=\"11.69\" Width=\"8.26\"/></PageLayout>
<TitlePage><HeaderAndFooter HeaderVisible=\"Yes\" FooterVisible=\"Yes\"><Header><Paragraph Alignment=\"Right\"><Text> </Text><DynamicLabel Type=\"Page #\"/><Text>.</Text></Paragraph></Header></HeaderAndFooter><Content/></TitlePage></FinalDraft>";
        let s = crate::parse(xml).unwrap();
        let pg = &s.settings.page;
        assert_eq!((pg.header.left.as_str(), pg.header.center.as_str(), pg.header.right.as_str()), ("", "{revision}", "{page}."));
        assert!(pg.footer.is_empty());
        assert_eq!(pg.paper, scriptcraft_doc::Paper::A4);
        // Without tab stops: the last piece is the right slot.
        let mut r = super::Read::default();
        r.add(false, "Left", "{title}\t{page}.", &[]);
        assert_eq!((r.header.left.as_str(), r.header.right.as_str()), ("{title}", "{page}."));
    }

    #[test]
    fn reads_older_scriptcraft_page_setup() {
        let xml = r#"<FinalDraft DocumentType="Script" Version="5"><Content/><PageLayout ScriptCraftPaper="a4"/>
<HeaderAndFooter HeaderVisible="Yes"><Header><Paragraph Alignment="Left" Type="Header"><DynamicLabel Type="ScriptCraft:title"/></Paragraph>
<Paragraph Alignment="Right" Type="Header"><DynamicLabel Type="Page #"/><Text>.</Text></Paragraph></Header></HeaderAndFooter></FinalDraft>"#;
        let s = crate::parse(xml).unwrap();
        assert_eq!(s.settings.page.paper, scriptcraft_doc::Paper::A4);
        assert_eq!((s.settings.page.header.left.as_str(), s.settings.page.header.right.as_str()), ("{title}", "{page}."));
    }
}
