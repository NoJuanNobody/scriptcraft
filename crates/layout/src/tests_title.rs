//! Free-form title pages.

use scriptcraft_doc::{Align, Para, Script, Style, TitleLine};

use super::*;

#[test]
fn free_form_title_pages_print_their_lines() {
    let mut s = Script::from_paras(vec![Para::new(Element::Action, "A.")]).unwrap();
    let underline = Style { underline: true, ..Style::PLAIN };
    s.title.set_lines(vec![
        TitleLine::new(Align::Left, "Agent: Bo"),
        TitleLine::new(Align::Center, ""),
        TitleLine { style: underline, ..TitleLine::new(Align::Center, "NIGHT") },
        TitleLine::new(Align::Right, "May"),
        TitleLine::new(Align::Left, &"word ".repeat(30)),
    ]);
    let l = layout(&s);
    let page = &l.pages[0];
    assert!(page.title_page);
    let t: Vec<(&str, f32, f32)> = page.lines.iter().map(|x| (x.text.as_str(), x.x, x.y)).collect();
    assert_eq!(t[0], ("Agent: Bo", 72.0, TOP));
    assert_eq!(t[1].0, "NIGHT");
    assert!((t[1].1 - (72.0 + (468.0 - 5.0 * CHAR_W) / 2.0)).abs() < 0.01, "centred between 1\" margins");
    assert_eq!(t[1].2, TOP + 2.0 * LINE_H, "the blank line keeps its row");
    assert_eq!(page.lines[1].spans, vec![scriptcraft_doc::Span { start: 0, end: 5, style: underline }]);
    assert!((t[2].1 + 3.0 * CHAR_W - (612.0 - 72.0)).abs() < 0.01, "right-aligned at 1\" from the edge");
    assert_eq!(t.len(), 6, "the 149-character line wraps to three rows of at most 65");
    assert!(t.iter().all(|(text, x, _)| *x + text.chars().count() as f32 * CHAR_W <= 612.0 - 72.0 + 0.01));
    assert_eq!(l.pages[1].number, Some(1));
    // Hundreds of lines: one title page, rows past it dropped.
    s.title.set_lines(vec![TitleLine::new(Align::Center, "x"); 200]);
    let l = layout(&s);
    assert_eq!(l.pages[0].lines.len(), body_rows(Paper::Letter));
    assert!(!l.pages[1].title_page);
}
