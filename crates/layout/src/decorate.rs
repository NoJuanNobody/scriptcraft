//! Page decorations added after pagination: headers and footers (with their fields filled in).

use scriptcraft_doc::page_setup::{Fields, HeaderFooter, expand};
use scriptcraft_doc::{Paper, Script};
use scriptcraft_geom::{CHAR_W, LINE_H, inch};

use crate::{LineKind, Page, deco};

/// Rows of body text on a page of `paper` (54 on Letter, 58 on A4).
pub fn body_rows(paper: Paper) -> usize {
    let r = (paper.height_pt() - scriptcraft_geom::TOP - scriptcraft_geom::BOTTOM) / LINE_H;
    if r.is_finite() && r >= 1.0 { r as usize } else { 1 }
}

/// Header and footer text of one page, by slot (left, centre, right), fields filled in. A slot
/// left with only punctuation (`{page}.` on the title page) prints nothing.
pub fn slots(hf: &HeaderFooter, f: &Fields) -> [String; 3] {
    let one = |t: &str| {
        let v = expand(t, f);
        if v.chars().any(char::is_alphanumeric) { v } else { String::new() }
    };
    [one(&hf.left), one(&hf.center), one(&hf.right)]
}

/// The revision label of a page (`Blue Rev. 10/12/26`), name and date: empty when unrevised.
pub(crate) fn revision_fields(script: &Script, level: Option<u8>) -> (String, String, String) {
    let Some(set) = level.and_then(|l| script.settings.revision.set(l)) else {
        return (String::new(), String::new(), String::new());
    };
    (set.label(), set.name.clone(), set.date.clone())
}

/// The highest revision on each page; with marks shown, the set's mark goes in the right
/// margin of every line with revised text.
pub(crate) fn revisions(script: &Script, pages: &mut [Page], paper: Paper) -> Vec<Option<u8>> {
    let r = &script.settings.revision;
    let x = paper.width_pt() - inch(0.4);
    let mut out = Vec::with_capacity(pages.len());
    for page in pages.iter_mut() {
        let mut top: Option<u8> = None;
        let mut marks = Vec::new();
        for l in page.lines.iter().filter(|l| l.kind == LineKind::Body) {
            let Some(p) = l.para.and_then(|i| script.paras.get(i)) else { continue };
            let lvl = p.revs.iter().filter(|m| m.start < l.end.max(l.start + 1) && m.end > l.start).map(|m| m.level).max();
            if let Some(lvl) = lvl {
                top = top.max(Some(lvl));
                if r.show_marks {
                    let mark = r.set(lvl).map_or("*", |s| s.mark.as_str());
                    marks.push(deco(LineKind::RevisionMark, x, l.y, mark));
                }
            }
        }
        page.lines.extend(marks);
        out.push(top);
    }
    out
}

/// Add the header and footer lines to every page. `page_rev[i]` is the highest revision on page
/// `i` (for the `{revision}` fields).
pub(crate) fn headers(script: &Script, pages: &mut [Page], page_rev: &[Option<u8>], paper: Paper) {
    let setup = &script.settings.page;
    let total = pages.iter().filter(|p| !p.title_page).count();
    let t = &script.title;
    let page_w = paper.width_pt();
    let (left, right) = (inch(1.5), page_w - inch(1.0));
    let footer_y = paper.height_pt() - inch(0.5) - LINE_H;
    for (i, page) in pages.iter_mut().enumerate() {
        let first_script = !page.title_page && page.number == Some(1);
        // The printed label (12A when pages are locked), else the page's number.
        let label = if page.label.is_empty() { page.number.map(|n| n.to_string()).unwrap_or_default() } else { page.label.clone() };
        let (revision, revname, revdate) = revision_fields(script, page_rev.get(i).copied().flatten());
        let f = Fields {
            page: &label,
            pages: total,
            title: t.title.lines().next().unwrap_or("").trim(),
            author: t.author.lines().next().unwrap_or("").trim(),
            date: t.draft_date.lines().next().unwrap_or("").trim(),
            revision: &revision,
            revname: &revname,
            revdate: &revdate,
        };
        let title = page.title_page;
        let show = |first_page: bool| if title { setup.header_on_title_page } else { !first_script || first_page };
        let add = |hf: &HeaderFooter, y: f32, kind: LineKind, page: &mut Page| {
            let [l, c, r] = slots(hf, &f);
            let w = |s: &str| s.chars().count() as f32 * CHAR_W;
            if !l.is_empty() {
                page.lines.push(deco(kind, left, y, &l));
            }
            if !c.is_empty() {
                page.lines.push(deco(kind, ((page_w - w(&c)) / 2.0).max(0.0), y, &c));
            }
            if !r.is_empty() {
                page.lines.push(deco(kind, (right - w(&r)).max(0.0), y, &r));
            }
        };
        if show(setup.header_first_page) {
            add(&setup.header, inch(0.5), LineKind::Header, page);
        }
        if show(setup.footer_first_page) {
            add(&setup.footer, footer_y, LineKind::Footer, page);
        }
    }
}
