//! Page setup: paper size and the header and footer printed on every page.
//!
//! Headers and footers have three slots (left, centre, right). Each slot is text with fields in
//! braces, filled in per page by [`expand`]:
//!
//! | Field | Becomes |
//! |---|---|
//! | `{page}` | the page number (`12`, or `12A` on an added page) |
//! | `{pages}` | the number of script pages |
//! | `{title}` | the title page's title |
//! | `{author}` | the title page's author |
//! | `{date}` | the title page's draft date |
//! | `{revision}` | the page's revision label (`Blue Rev. 10/12/26`), empty on unrevised pages |
//! | `{revname}` / `{revdate}` | the page's revision name / date |
//!
//! Unknown fields are printed as typed. The default header is `{revision}  {page}.` on the right
//! from page 2, which prints exactly the classic page number until a revision exists.

use serde::{Deserialize, Serialize};

/// Paper sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Paper {
    /// US Letter, 8.5 × 11 in.
    #[default]
    Letter,
    /// ISO A4, 210 × 297 mm.
    A4,
}

impl Paper {
    pub const ALL: [Paper; 2] = [Paper::Letter, Paper::A4];
    pub fn id(self) -> &'static str {
        match self {
            Paper::Letter => "letter",
            Paper::A4 => "a4",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Paper::Letter => "US Letter",
            Paper::A4 => "A4",
        }
    }
    pub fn parse(s: &str) -> Option<Paper> {
        match s.trim().to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
            "letter" | "usletter" | "us" => Some(Paper::Letter),
            "a4" => Some(Paper::A4),
            _ => None,
        }
    }
    /// Width in points (1/72 inch).
    pub fn width_pt(self) -> f32 {
        match self {
            Paper::Letter => 612.0,
            Paper::A4 => 595.276,
        }
    }
    /// Height in points.
    pub fn height_pt(self) -> f32 {
        match self {
            Paper::Letter => 792.0,
            Paper::A4 => 841.89,
        }
    }
    /// The paper whose size is closest to `w × h` points.
    pub fn nearest(w: f32, h: f32) -> Paper {
        let d = |p: Paper| (p.width_pt() - w).abs() + (p.height_pt() - h).abs();
        if d(Paper::A4) < d(Paper::Letter) { Paper::A4 } else { Paper::Letter }
    }
}

/// Longest header or footer slot, in characters.
pub const MAX_SLOT_CHARS: usize = 200;

/// One header or footer: left, centre and right text with fields.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeaderFooter {
    pub left: String,
    pub center: String,
    pub right: String,
}

impl HeaderFooter {
    pub fn is_empty(&self) -> bool {
        self.left.trim().is_empty() && self.center.trim().is_empty() && self.right.trim().is_empty()
    }
    /// The slot named `left`, `center` or `right`.
    pub fn slot_mut(&mut self, name: &str) -> Option<&mut String> {
        match name.to_ascii_lowercase().as_str() {
            "left" => Some(&mut self.left),
            "center" | "centre" => Some(&mut self.center),
            "right" => Some(&mut self.right),
            _ => None,
        }
    }
    /// Clean every slot: one line, no control characters, at most [`MAX_SLOT_CHARS`].
    pub fn clean(&mut self) {
        for s in [&mut self.left, &mut self.center, &mut self.right] {
            *s = clean_slot(s);
        }
    }
}

/// One line, no control characters, capped.
pub fn clean_slot(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_SLOT_CHARS).collect()
}

/// Paper, header and footer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PageSetup {
    pub paper: Paper,
    pub header: HeaderFooter,
    pub footer: HeaderFooter,
    /// Print the header on the first script page too.
    pub header_first_page: bool,
    /// Print the footer on the first script page too.
    pub footer_first_page: bool,
    /// Print the header and footer on the title page as well.
    pub header_on_title_page: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            paper: Paper::Letter,
            header: HeaderFooter { left: String::new(), center: String::new(), right: "{revision}  {page}.".into() },
            footer: HeaderFooter::default(),
            header_first_page: false,
            footer_first_page: true,
            header_on_title_page: false,
        }
    }
}

impl PageSetup {
    pub fn is_default(&self) -> bool {
        *self == PageSetup::default()
    }
}

/// The values fields are filled with on one page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields<'a> {
    pub page: &'a str,
    pub pages: usize,
    pub title: &'a str,
    pub author: &'a str,
    pub date: &'a str,
    pub revision: &'a str,
    pub revname: &'a str,
    pub revdate: &'a str,
}

/// Fill the fields of a header or footer slot. Runs of spaces left by empty fields at the ends
/// are trimmed.
pub fn expand(template: &str, f: &Fields) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(rest.get(..i).unwrap_or(""));
        let after = rest.get(i + 1..).unwrap_or("");
        let Some(j) = after.find('}') else {
            out.push_str(rest.get(i..).unwrap_or(""));
            rest = "";
            break;
        };
        let name = after.get(..j).unwrap_or("");
        let pages = f.pages.to_string();
        let v = match name.trim().to_ascii_lowercase().as_str() {
            "page" => Some(f.page),
            "pages" => Some(pages.as_str()),
            "title" => Some(f.title),
            "author" => Some(f.author),
            "date" => Some(f.date),
            "revision" => Some(f.revision),
            "revname" => Some(f.revname),
            "revdate" => Some(f.revdate),
            _ => None,
        };
        match v {
            Some(v) => out.push_str(v.lines().next().unwrap_or("").trim()),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = after.get(j + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_fill_in() {
        let f = Fields { page: "12", pages: 90, title: "Night Signal", author: "A. Writer", date: "Oct 1", ..Fields::default() };
        assert_eq!(expand("{page}.", &f), "12.");
        assert_eq!(expand("{revision}  {page}.", &f), "12.");
        assert_eq!(expand("{title} - {page} of {pages}", &f), "Night Signal - 12 of 90");
        assert_eq!(expand("{AUTHOR}, {date} {nope} {", &f), "A. Writer, Oct 1 {nope} {");
        let r = Fields { page: "3A", revision: "Blue Rev. 10/12/26", ..f };
        assert_eq!(expand("{revision}  {page}.", &r), "Blue Rev. 10/12/26  3A.");
        assert_eq!(expand("", &r), "");
    }

    #[test]
    fn paper_sizes() {
        assert_eq!(Paper::parse("A4"), Some(Paper::A4));
        assert_eq!(Paper::parse("US Letter"), Some(Paper::Letter));
        assert_eq!(Paper::parse("legal"), None);
        assert_eq!(Paper::nearest(595.0, 842.0), Paper::A4);
        assert_eq!(Paper::nearest(612.0, 792.0), Paper::Letter);
        assert!(PageSetup::default().is_default());
        let mut h = HeaderFooter { left: "a\nb".into(), ..Default::default() };
        h.clean();
        assert_eq!(h.left, "a b");
    }
}
