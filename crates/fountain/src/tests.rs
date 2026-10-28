use super::*;

const SRC: &str = "Title: The Long Night
Credit: Written by
Author: Ada Lovelace
Draft date: 1 May 2026
Contact:
    ada@example.com
    555-0100

FADE IN:

INT. DINER - NIGHT #1#

= Margo waits for a call.

Rain streaks the window. **MARGO** (40s) stirs her coffee.

MARGO
(quietly)
Not tonight.
Not again.

DEV (O.S.)
You said that last time.

MARGO ^
I meant it.

CUT TO:

.BACKSTAGE

@McCLANE
Yippee.

>THE END<

/* cut this */
[[note to self]]
";

#[test]
fn parses_elements() {
    let s = parse(SRC);
    let kinds: Vec<(Element, &str)> = s.paras.iter().map(|p| (p.element, p.text.as_str())).collect();
    assert_eq!(
        kinds,
        vec![
            (Element::Action, "FADE IN:"),
            (Element::SceneHeading, "INT. DINER - NIGHT"),
            (Element::Action, "Rain streaks the window. MARGO (40s) stirs her coffee."),
            (Element::Character, "MARGO"),
            (Element::Parenthetical, "(quietly)"),
            (Element::Dialogue, "Not tonight. Not again."),
            (Element::Character, "DEV (O.S.)"),
            (Element::Dialogue, "You said that last time."),
            (Element::Character, "MARGO"),
            (Element::Dialogue, "I meant it."),
            (Element::Transition, "CUT TO:"),
            (Element::SceneHeading, "BACKSTAGE"),
            (Element::Character, "MCCLANE"),
            (Element::Dialogue, "Yippee."),
            (Element::Action, "THE END"),
        ]
    );
    assert_eq!(s.paras[1].scene_number.as_deref(), Some("1"));
    assert_eq!(s.paras[1].summary, "Margo waits for a call.");
    assert!(s.paras[8].dual);
    assert_eq!(s.title.title, "The Long Night");
    assert_eq!(s.title.author, "Ada Lovelace");
    assert_eq!(s.title.contact, "ada@example.com\n555-0100");
    assert_eq!(s.title.draft_date, "1 May 2026");
}

#[test]
fn round_trip_is_stable() {
    let s = parse(SRC);
    let out = write(&s);
    let back = parse(&out);
    assert_eq!(back, s, "\n{out}");
    assert_eq!(write(&back), out);
}

#[test]
fn forcing_where_needed() {
    let s = Script::from_paras(vec![
        Para::new(Element::Action, "INT. NOT A HEADING"),
        Para::new(Element::Action, "BANG"),
        Para::new(Element::SceneHeading, "THE MOON"),
        Para::new(Element::Transition, "FADE OUT."),
        Para::new(Element::Character, "MARGO"),
        Para::new(Element::Dialogue, "(not a paren)"),
    ])
    .unwrap();
    let out = write(&s);
    assert!(out.contains("!INT. NOT A HEADING"), "{out}");
    assert!(out.contains("!BANG"), "{out}");
    assert!(out.contains(".THE MOON"), "{out}");
    assert!(out.contains(">FADE OUT."), "{out}");
    assert_eq!(parse(&out), s, "{out}");
}

#[test]
fn hostile_input() {
    for src in ["", "\n\n\n", "Title:", "/* never closed", "[[", "@", ".", ">", "=", "INT.", "A\r\nB\rC", "é:é\n\n.é #é#", "#", "!"] {
        let s = parse(src);
        assert!(!s.paras.is_empty());
        let _ = write(&s);
    }
    let big = "Hello world.\n\n".repeat(10_000);
    assert_eq!(parse(&big).paras.len(), 10_000);
}

#[test]
fn emphasis_round_trip() {
    use scriptcraft_doc::Style;
    let src = "A *quiet* **loud** ***both*** _under_ and _**mixed**_ room. Literal \\* star, snake\\_case, 2*3.\n";
    let s = parse(src);
    let p = &s.paras[0];
    assert_eq!(p.text, "A quiet loud both under and mixed room. Literal * star, snake_case, 2*3.");
    let st = |w: &str| {
        let i = p.text.find(w).unwrap();
        p.styles()[i]
    };
    assert_eq!(st("quiet"), Style { italic: true, ..Style::PLAIN });
    assert_eq!(st("loud"), Style { bold: true, ..Style::PLAIN });
    assert_eq!(st("both"), Style { bold: true, italic: true, ..Style::PLAIN });
    assert_eq!(st("under"), Style { underline: true, ..Style::PLAIN });
    assert_eq!(st("mixed"), Style { bold: true, underline: true, ..Style::PLAIN });
    assert!(st("Literal").is_plain());
    let out = write(&s);
    assert_eq!(parse(&out), s, "{out}");
}

#[test]
fn card_colour_and_tags() {
    let src = "INT. LAB - NIGHT\n\n= Ada wins.\n\n[[color: teal]]\n\n[[tags: climax, Ada]]\n\n[[an ordinary note]]\nBeakers bubble.\n";
    let s = parse(src);
    assert_eq!(s.paras[0].color, Some(scriptcraft_doc::CardColor::Teal));
    assert_eq!(s.paras[0].tags, vec!["climax", "Ada"]);
    assert_eq!(s.paras[0].summary, "Ada wins.");
    assert_eq!(s.paras[1].text, "Beakers bubble.");
    assert_eq!(parse(&write(&s)), s, "{}", write(&s));
}

#[test]
fn russian_screenplay_round_trips() {
    let source = "ИНТ. КУХНЯ - НОЧЬ\n\nЁжик ждёт у окна.\n\nАЛЁНА\n(тихо)\nВсё готово?\n\nНАТ. ДВОР - ДЕНЬ\n\nПадает снег.\n";
    let script = parse(source);
    assert_eq!(script.paras[0].element, Element::SceneHeading);
    assert_eq!(script.paras[2].element, Element::Character);
    assert_eq!(script.paras[4].element, Element::Dialogue);
    assert_eq!(script.paras[5].element, Element::SceneHeading);
    assert_eq!(parse(&write(&script)), script);
    assert!(!is_heading("ИНТЕРЕСНЫЙ ДЕНЬ"));
}

#[test]
fn headings_in_the_users_words() {
    let src = "WN. KUCHNIA – DZIEŃ\n\nPada deszcz.\n\nPL. OGRÓD - NOC\n";
    let pl = Sluglines::preset("pl").unwrap();
    let s = parse_with(src, &pl);
    assert_eq!(s.paras[0].element, Element::SceneHeading);
    assert_eq!(s.paras[2].element, Element::SceneHeading);
    // The default words don't know them: action.
    assert_eq!(parse(src).paras[0].element, Element::Action);
    // Written forced, so any Fountain app (and the default words) reads them back as headings.
    let out = write(&s);
    assert!(out.contains(".WN. KUCHNIA – DZIEŃ"), "{out}");
    assert_eq!(parse(&out).paras[0].element, Element::SceneHeading);
}
