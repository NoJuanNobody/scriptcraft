//! Paper sizes, headers and footers.

use scriptcraft_doc::{HeaderFooter, Para, Script};

use super::*;

fn action_lines(n: usize) -> Script {
    Script::from_paras((0..n).map(|i| Para::new(Element::Action, format!("Line {i}."))).collect()).unwrap()
}
fn body(page: &Page) -> Vec<&Line> {
    page.lines.iter().filter(|l| l.kind == LineKind::Body).collect()
}
fn kind<'a>(page: &'a Page, k: LineKind) -> Vec<&'a Line> {
    page.lines.iter().filter(|l| l.kind == k).collect()
}

#[test]
fn a4_has_58_lines() {
    assert_eq!(body_rows(Paper::Letter), 54);
    assert_eq!(body_rows(Paper::A4), 58);
    let mut s = action_lines(60);
    let letter = layout(&s);
    s.settings.page.paper = Paper::A4;
    let a4 = layout(&s);
    // One line plus one blank each: 27 on a Letter page, 29 on A4 (rows 0, 2, … 56).
    assert_eq!(body(&letter.pages[0]).len(), 27);
    assert_eq!(body(&a4.pages[0]).len(), 29);
    assert_eq!(body(&a4.pages[0]).last().unwrap().y, 72.0 + 56.0 * 12.0);
    assert_eq!(a4.paper, Paper::A4);
    assert!((a4.page_width() - 595.276).abs() < 0.01);
    assert!((a4.page_height(0) - 841.89).abs() < 0.01);
    assert_eq!(letter.page_height(0), 792.0);
    // The page number sits an inch from the right edge of either paper.
    let n = kind(&a4.pages[1], LineKind::Header)[0];
    assert_eq!(n.text, "2.");
    assert!((n.x + 2.0 * CHAR_W - (595.276 - 72.0)).abs() < 0.01);
    // Indents are measured from the left edge, so text sits where it does on Letter.
    assert_eq!(body(&a4.pages[0])[0].x, body(&letter.pages[0])[0].x);
}

#[test]
fn headers_and_footers_fill_their_fields() {
    let mut s = action_lines(60);
    s.title.title = "Night Signal\nsecond line".into();
    s.title.author = "A. Writer".into();
    s.title.draft_date = "October 1".into();
    s.settings.title_page = false;
    s.settings.page.header = HeaderFooter { left: "{title}".into(), center: "".into(), right: "{page} of {pages}".into() };
    s.settings.page.footer = HeaderFooter { left: "".into(), center: "{author} - {date}".into(), right: "".into() };
    let l = layout(&s);
    assert_eq!(l.script_pages(), 3);
    // Page 1: no header by default, footer shown.
    assert!(kind(&l.pages[0], LineKind::Header).is_empty());
    let f = kind(&l.pages[0], LineKind::Footer);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].text, "A. Writer - October 1");
    assert_eq!(f[0].y, 792.0 - 36.0 - 12.0);
    assert!((f[0].x - (612.0 - 21.0 * CHAR_W) / 2.0).abs() < 0.01, "centred");
    // Page 2: both header slots.
    let h = kind(&l.pages[1], LineKind::Header);
    let texts: Vec<&str> = h.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(texts, vec!["Night Signal", "2 of 3"]);
    assert_eq!(h[0].x, 108.0);
    assert_eq!(h[0].y, 36.0);
    assert_eq!(h[1].x, 540.0 - 6.0 * CHAR_W);
    // The page is no taller for having a footer.
    assert_eq!(l.page_height(0), 792.0);
}

#[test]
fn revision_marks_and_page_labels() {
    let mut s = action_lines(60);
    s.settings.revision.sets[1].date = "10/12/26".into();
    // A Blue change on page 2 (paragraph 30) and a Pink one on page 3 (paragraph 58).
    scriptcraft_doc::revision::add_mark(&mut s.paras[30], 0, 4, 1);
    scriptcraft_doc::revision::add_mark(&mut s.paras[58], 5, 7, 2);
    let l = layout(&s);
    assert_eq!(l.page_rev, vec![None, Some(1), Some(2)]);
    let marks = kind(&l.pages[1], LineKind::RevisionMark);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].text, "*");
    assert_eq!(marks[0].x, 612.0 - 28.8);
    let line = body(&l.pages[1]).into_iter().find(|x| x.para == Some(30)).unwrap();
    assert_eq!(marks[0].y, line.y);
    assert_eq!(kind(&l.pages[1], LineKind::Header)[0].text, "Blue Rev. 10/12/26  2.");
    assert_eq!(kind(&l.pages[2], LineKind::Header)[0].text, "Pink Rev.  3.");
    assert!(kind(&l.pages[0], LineKind::RevisionMark).is_empty());
    // Marks hidden: pages still know their revision.
    s.settings.revision.show_marks = false;
    let l = layout(&s);
    assert!(l.pages.iter().all(|p| kind(p, LineKind::RevisionMark).is_empty()));
    assert_eq!(l.page_rev[2], Some(2));
}

#[test]
fn first_page_and_title_page_options() {
    let mut s = action_lines(60);
    s.title.title = "T".into();
    s.settings.page.header_first_page = true;
    s.settings.page.footer = HeaderFooter { right: "Draft".into(), ..Default::default() };
    s.settings.page.footer_first_page = false;
    let l = layout(&s);
    assert!(l.pages[0].title_page);
    assert!(l.pages[0].lines.iter().all(|x| !matches!(x.kind, LineKind::Header | LineKind::Footer)), "title page bare by default");
    let p1 = &l.pages[1];
    assert_eq!(kind(p1, LineKind::Header)[0].text, "1.");
    assert!(kind(p1, LineKind::Footer).is_empty());
    assert_eq!(kind(&l.pages[2], LineKind::Footer)[0].text, "Draft");
    s.settings.page.header_on_title_page = true;
    let l = layout(&s);
    assert_eq!(kind(&l.pages[0], LineKind::Footer)[0].text, "Draft");
    assert!(kind(&l.pages[0], LineKind::Header).is_empty(), "{{page}} is empty on the title page");
    // Speed view has no headers or footers.
    let sp = layout_with(&s, Mode::Speed);
    assert!(sp.pages.iter().flat_map(|p| &p.lines).all(|x| !matches!(x.kind, LineKind::Header | LineKind::Footer)));
}
