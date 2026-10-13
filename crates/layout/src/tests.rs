use scriptcraft_doc::{Para, Script};

use super::*;

fn s(paras: Vec<Para>) -> Script {
    Script::from_paras(paras).unwrap()
}
fn p(e: Element, t: &str) -> Para {
    Para::new(e, t)
}
fn body(page: &Page) -> Vec<&Line> {
    page.lines.iter().filter(|l| l.kind == LineKind::Body).collect()
}

#[test]
fn wraps_words() {
    assert_eq!(wrap("aaa bbb ccc", 7), vec![(0, 7), (8, 11)]);
    assert_eq!(wrap("aaa bbb ccc", 8), vec![(0, 7), (8, 11)]);
    assert_eq!(wrap("abcdefghij", 4), vec![(0, 4), (4, 8), (8, 10)]);
    assert_eq!(wrap("", 4), vec![(0, 0)]);
    assert_eq!(wrap("ab   cd", 3), vec![(0, 2), (5, 7)]);
    assert_eq!(wrap("héllo wörld", 5), vec![(0, 6), (7, 13)]);
    assert_eq!(wrap("x", 0), vec![(0, 1)]);
    // Every byte is covered by a line or a skipped space.
    let t = "The quick brown fox jumps over the lazy dog and keeps running far away.";
    let w = wrap(t, 20);
    for pair in w.windows(2) {
        assert!(t[pair[0].1..pair[1].0].chars().all(|c| c == ' '));
    }
}

#[test]
fn element_indents() {
    let sc = s(vec![
        p(Element::SceneHeading, "INT. ROOM - DAY"),
        p(Element::Action, "A chair."),
        p(Element::Character, "ANA"),
        p(Element::Parenthetical, "(softly)"),
        p(Element::Dialogue, "Hello."),
        p(Element::Transition, "CUT TO:"),
    ]);
    let l = layout(&sc);
    let b = body(&l.pages[0]);
    let xs: Vec<f32> = b.iter().map(|l| l.x).collect();
    assert_eq!(xs, vec![108.0, 108.0, 3.7 * 72.0, 3.1 * 72.0, 180.0, 540.0 - 7.0 * 7.2]);
    let ys: Vec<f32> = b.iter().map(|l| l.y).collect();
    // Heading at the top, one blank line before action and cue, none inside dialogue.
    assert_eq!(ys, vec![72.0, 96.0, 120.0, 132.0, 144.0, 168.0]);
    assert_eq!(width_chars(Element::Dialogue), 35);
    assert_eq!(width_chars(Element::Action), 60);
    assert!(l.pages[0].lines.iter().all(|l| l.kind != LineKind::Header), "no number on page 1");
}

#[test]
fn fifty_four_lines_per_page() {
    let paras: Vec<Para> = (0..60).map(|i| p(Element::Action, &format!("Line {i}."))).collect();
    let l = layout(&s(paras));
    // One line plus one blank each: 27 on page one (rows 0, 2, … 52).
    assert_eq!(body(&l.pages[0]).len(), 27);
    assert_eq!(body(&l.pages[0]).last().unwrap().y, 72.0 + 52.0 * 12.0);
    assert_eq!(body(&l.pages[1])[0].y, 72.0, "space before is dropped at the top of a page");
    let num = l.pages[1].lines.iter().find(|l| l.kind == LineKind::Header).unwrap();
    assert_eq!(num.text, "2.");
    assert_eq!(num.y, 36.0);
    assert_eq!(num.x, 540.0 - 2.0 * 7.2);
    assert_eq!(l.script_pages(), 3);
    assert_eq!(l.page_number_of(59), 3);
}

#[test]
fn heading_never_orphaned() {
    // 26 one-line actions fill rows 0..=50; a heading would land on row 52 with nothing after.
    let mut paras: Vec<Para> = (0..26).map(|i| p(Element::Action, &format!("Line {i}."))).collect();
    paras.push(p(Element::SceneHeading, "EXT. FIELD - NIGHT"));
    paras.push(p(Element::Action, "Crickets."));
    let l = layout(&s(paras));
    let first2 = body(&l.pages[1]);
    assert_eq!(first2[0].text, "EXT. FIELD - NIGHT");
    assert_eq!(first2[1].text, "Crickets.");
}

#[test]
fn dialogue_breaks_with_more_and_contd() {
    let mut paras: Vec<Para> = (0..20).map(|i| p(Element::Action, &format!("Line {i}."))).collect();
    paras.push(p(Element::Character, "MARGO"));
    let speech = "This is a sentence that goes on. ".repeat(20);
    paras.push(p(Element::Dialogue, speech.trim()));
    let l = layout(&s(paras));
    let p1 = &l.pages[0];
    let more = p1.lines.iter().find(|l| l.kind == LineKind::More).expect("(MORE)");
    assert_eq!(more.text, "(MORE)");
    assert_eq!(more.x, 3.7 * 72.0);
    let contd = l.pages[1].lines.iter().find(|l| l.kind == LineKind::Contd).expect("(CONT'D)");
    assert_eq!(contd.text, "MARGO (CONT'D)");
    assert_eq!(contd.y, 72.0);
    // Lines on page 1 stop above the bottom margin, (MORE) included.
    assert!(p1.lines.iter().all(|l| l.y <= 72.0 + 53.0 * 12.0));
    // Split after a sentence end.
    let b1 = body(p1);
    let last = b1.last().unwrap();
    assert!(last.text.trim_end().ends_with('.'), "{}", last.text);
    // All dialogue lines are laid out exactly once.
    let lines: usize = l.pages.iter().map(|p| p.lines.iter().filter(|l| l.para == Some(21)).count()).sum();
    assert_eq!(lines, wrap(speech.trim(), 35).len());
}

#[test]
fn cue_stays_with_dialogue() {
    let mut paras: Vec<Para> = (0..26).map(|i| p(Element::Action, &format!("Line {i}."))).collect();
    paras.push(p(Element::Character, "DEV"));
    paras.push(p(Element::Dialogue, "Two lines of dialogue here, wrapping onto a second."));
    let l = layout(&s(paras));
    assert_eq!(body(&l.pages[1])[0].text, "DEV");
    assert!(l.pages[0].lines.iter().all(|l| l.kind != LineKind::More));
}

#[test]
fn automatic_contd() {
    let sc = s(vec![
        p(Element::SceneHeading, "INT. A - DAY"),
        p(Element::Character, "ANA"),
        p(Element::Dialogue, "One."),
        p(Element::Action, "She sits."),
        p(Element::Character, "ANA"),
        p(Element::Dialogue, "Two."),
        p(Element::SceneHeading, "INT. B - DAY"),
        p(Element::Character, "ANA"),
        p(Element::Dialogue, "Three."),
    ]);
    let l = layout(&sc);
    let cues: Vec<String> = body(&l.pages[0]).iter().filter(|l| l.element == Some(Element::Character)).map(|l| l.display()).collect();
    assert_eq!(cues, vec!["ANA", "ANA (CONT'D)", "ANA"]);
    let mut off = sc.clone();
    off.settings.auto_contd = false;
    assert!(body(&layout(&off).pages[0]).iter().all(|l| l.suffix.is_empty()));
}

#[test]
fn dual_dialogue_side_by_side() {
    let mut b = p(Element::Character, "DEV");
    b.dual = true;
    let sc = s(vec![p(Element::Character, "ANA"), p(Element::Dialogue, "Left side."), b, p(Element::Dialogue, "Right side.")]);
    let l = layout(&sc);
    let lines = body(&l.pages[0]);
    assert_eq!(lines[0].y, lines[2].y);
    assert!(lines[2].x > lines[0].x + 100.0);
}

#[test]
fn caret_and_hit_agree() {
    let sc = s(vec![
        p(Element::SceneHeading, "INT. ROOM - DAY"),
        p(Element::Action, "A long action line that certainly wraps onto a second line of text, maybe a third."),
        p(Element::Character, "ANA"),
        p(Element::Dialogue, "Words, words, words, and more words than fit."),
        p(Element::Transition, "CUT TO:"),
    ]);
    let l = layout(&sc);
    for (pi, para) in sc.paras.iter().enumerate() {
        for (off, _) in para.text.char_indices().chain(std::iter::once((para.text.len(), ' '))) {
            let pos = Pos::new(pi, off);
            let c = l.caret(pos).unwrap();
            let back = l.hit(c.page, c.x + 0.1, c.y + 6.0).unwrap();
            // A caret on a wrapped space lands at the end of the previous line.
            assert!(back == pos || (back.para == pi && sc.paras[pi].text[back.off..off].trim().is_empty()), "{pos:?} -> {back:?}");
        }
    }
    let (down, gx) = l.vertical(Pos::new(1, 5), 1, None).unwrap();
    assert_eq!(down.para, 1);
    let (up, _) = l.vertical(down, -1, Some(gx)).unwrap();
    assert_eq!(up, Pos::new(1, 5));
    assert!(l.vertical(Pos::new(0, 0), -1, None).is_none());
    assert_eq!(l.line_edge(Pos::new(1, 3), false), Some(Pos::new(1, 0)));
}

#[test]
fn title_page_first() {
    let mut sc = s(vec![p(Element::Action, "x")]);
    sc.title.title = "Night Shift".into();
    sc.title.author = "Ada".into();
    sc.title.draft_date = "May 2026".into();
    let l = layout(&sc);
    assert!(l.pages[0].title_page);
    assert_eq!(l.pages[0].number, None);
    assert_eq!(l.pages[1].number, Some(1));
    let t = &l.pages[0].lines[0];
    assert_eq!(t.text, "NIGHT SHIFT");
    assert!((t.x - (612.0 - 11.0 * 7.2) / 2.0).abs() < 0.01);
    assert_eq!(l.para_page[0], 1);
}

#[test]
fn hostile_layouts() {
    let long = "x".repeat(5000);
    let sc = s(vec![
        p(Element::Dialogue, &long),
        p(Element::Character, &long),
        p(Element::Dialogue, &"word ".repeat(2000)),
        p(Element::SceneHeading, &long),
    ]);
    let l = layout(&sc);
    assert!(l.pages.len() > 2);
    for pg in &l.pages {
        assert!(pg.lines.iter().all(|l| l.y < 792.0));
    }
    let _ = layout(&Script::new());
    let _ = l.hit(999, 0.0, 0.0);
    let _ = l.caret(Pos::new(999, 999));
    let _ = l.hit(0, f32::NAN, f32::INFINITY);
}

#[test]
fn styled_lines_carry_spans() {
    let mut sc = s(vec![p(Element::Action, "A long action line that certainly wraps onto a second line of text, maybe a third.")]);
    sc.set_flag(Pos::new(0, 55), Pos::new(0, 70), scriptcraft_doc::Flag::Bold, true);
    let l = layout(&sc);
    let b = body(&l.pages[0]);
    assert_eq!(b[0].spans.len(), 1);
    assert_eq!(b[1].spans.len(), 1);
    let segs = b[0].segments();
    assert!(segs.last().unwrap().2.bold);
    assert_eq!(segs.iter().map(|s| s.1).collect::<String>(), b[0].text);
    assert_eq!(b[1].segments()[0].0, 0);
}

#[test]
fn element_settings_move_text() {
    let mut sc = s(vec![p(Element::Action, "A chair."), p(Element::Transition, "CUT TO:")]);
    let mut f = sc.settings.elements.get(Element::Action);
    f.left = 2.0;
    f.before = 3;
    sc.settings.elements.set(Element::Action, f);
    let mut t = sc.settings.elements.get(Element::Transition);
    t.align = Align::Center;
    t.left = 1.5;
    sc.settings.elements.set(Element::Transition, t);
    let l = layout(&sc);
    let b = body(&l.pages[0]);
    assert_eq!(b[0].x, 144.0);
    assert!((b[1].x - (108.0 + 540.0 - 7.0 * 7.2) / 2.0).abs() < 0.01);
    // Hostile values are clamped.
    f.left = f32::NAN;
    f.right = -4.0;
    sc.settings.elements.set(Element::Action, f);
    let g = sc.settings.elements.get(Element::Action);
    assert!(g.left >= 0.0 && g.right > g.left);
    let _ = layout(&sc);
}

#[test]
fn speed_view_has_no_breaks() {
    let paras: Vec<Para> = (0..120).map(|i| p(Element::Action, &format!("Line {i}."))).collect();
    let mut sc = s(paras);
    sc.title.title = "T".into();
    let paged = layout(&sc);
    assert!(paged.pages.len() > 3);
    let l = layout_with(&sc, Mode::Speed);
    assert_eq!(l.pages.len(), 1);
    assert!(l.pages[0].lines.iter().all(|x| x.kind == LineKind::Body));
    assert_eq!(body(&l.pages[0]).len(), 120);
    assert!(l.page_height(0) > 2000.0);
    let c = l.caret(Pos::new(119, 3)).unwrap();
    assert_eq!(l.hit(0, c.x + 0.1, c.y + 6.0), Some(Pos::new(119, 3)));
}
