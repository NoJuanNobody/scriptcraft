use scriptcraft_doc::Element;

/// A test PDF: pages of `(x inches, baseline inches from the top, text)` in Courier 12, with
/// optional bold lines (`*` prefix) and a ToUnicode-free simple font.
pub fn build(pages: &[Vec<(f32, f32, &str)>]) -> Vec<u8> {
    let mut objs: Vec<String> = Vec::new();
    let n_pages = pages.len();
    // 1 catalog, 2 pages, 3 font, 4 bold font, then (page, content) pairs.
    objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    let kids: Vec<String> = (0..n_pages).map(|i| format!("{} 0 R", 5 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Count {n_pages} /Kids [{}] >>", kids.join(" ")));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".into());
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>".into());
    for (i, lines) in pages.iter().enumerate() {
        let mut c = String::new();
        for (x, y, t) in lines {
            let (font, t) = match t.strip_prefix('*') {
                Some(rest) => ("F2", rest),
                None => ("F1", *t),
            };
            let esc = t.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
            c.push_str(&format!("BT /{font} 12 Tf 1 0 0 1 {:.2} {:.2} Tm ({esc}) Tj ET\n", x * 72.0, 792.0 - y * 72.0));
        }
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            6 + 2 * i
        ));
        objs.push(format!("<< /Length {} >>\nstream\n{c}endstream", c.len()));
    }
    let mut out = String::from("%PDF-1.4\n");
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
    }
    let xref = out.len();
    out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1));
    for o in offs {
        out.push_str(&format!("{o:010} 00000 n \n"));
    }
    out.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1));
    out.into_bytes()
}

fn elements(r: &crate::Imported) -> Vec<(Element, String)> {
    r.script.paras.iter().map(|p| (p.element, p.text.clone())).collect()
}

#[test]
fn reads_a_typed_screenplay_by_indent() {
    let lh = 1.0 / 6.0;
    let mut y = 1.2;
    let mut page = Vec::new();
    let mut line = |x: f32, t: &'static str, gap: f32| {
        y += gap * lh;
        page.push((x, y, t));
    };
    line(1.0, "1", 0.0);
    line(1.5, "INT. KITCHEN - NIGHT", 0.0);
    line(7.6, "1", 0.0);
    line(1.5, "Rain on the window. Nell stirs a pot that has been", 2.0);
    line(1.5, "simmering for hours.", 1.0);
    line(3.7, "NELL", 2.0);
    line(3.1, "(to herself)", 1.0);
    line(2.5, "Not long now. Not long at all, if", 1.0);
    line(2.5, "the gas holds.", 1.0);
    line(6.5, "CUT TO:", 2.0);
    let pdf = build(&[page, vec![(7.0, 0.5, "2."), (1.5, 1.2, "*The kettle screams.")]]);
    let r = crate::import(&pdf, crate::Kind::Pdf).unwrap();
    assert_eq!(
        elements(&r),
        vec![
            (Element::SceneHeading, "INT. KITCHEN - NIGHT".into()),
            (Element::Action, "Rain on the window. Nell stirs a pot that has been simmering for hours.".into()),
            (Element::Character, "NELL".into()),
            (Element::Parenthetical, "(to herself)".into()),
            (Element::Dialogue, "Not long now. Not long at all, if the gas holds.".into()),
            (Element::Transition, "CUT TO:".into()),
            (Element::Action, "The kettle screams.".into()),
        ]
    );
    assert_eq!(r.script.paras[0].scene_number.as_deref(), Some("1"));
    assert!(r.script.paras[6].spans.first().is_some_and(|s| s.style.bold));
}

#[test]
fn title_page_and_continued_speech() {
    let title =
        vec![(3.5, 4.0, "NIGHT SIGNAL"), (3.75, 4.5, "Written by"), (3.6, 4.83, "Ann Lee"), (1.5, 9.5, "ann@example.com"), (6.0, 9.5, "May 2026")];
    let p1 = vec![(1.5, 1.2, "EXT. PIER - DAY"), (3.7, 1.53, "NELL"), (2.5, 1.7, "First half of a long"), (3.7, 9.8, "(MORE)")];
    let p2 = vec![(7.0, 0.5, "2."), (3.7, 1.2, "NELL (CONT'D)"), (2.5, 1.37, "speech that goes on.")];
    let r = crate::import(&build(&[title, p1, p2]), crate::Kind::Pdf).unwrap();
    let t = &r.script.title;
    assert_eq!((t.title.as_str(), t.credit.as_str(), t.author.as_str()), ("NIGHT SIGNAL", "Written by", "Ann Lee"));
    assert_eq!((t.contact.as_str(), t.draft_date.as_str()), ("ann@example.com", "May 2026"));
    assert_eq!(
        elements(&r),
        vec![
            (Element::SceneHeading, "EXT. PIER - DAY".into()),
            (Element::Character, "NELL".into()),
            (Element::Dialogue, "First half of a long speech that goes on.".into()),
        ]
    );
}

#[test]
fn dual_dialogue_side_by_side() {
    let p = vec![
        (1.5, 1.2, "INT. ROOM - DAY"),
        (2.6, 1.53, "ANNA"),
        (5.1, 1.53, "BEN"),
        (2.0, 1.7, "Left side."),
        (4.6, 1.7, "Right side,"),
        (4.6, 1.87, "two lines."),
        (1.5, 2.2, "They stop."),
    ];
    let r = crate::import(&build(&[p]), crate::Kind::Pdf).unwrap();
    assert_eq!(
        elements(&r),
        vec![
            (Element::SceneHeading, "INT. ROOM - DAY".into()),
            (Element::Character, "ANNA".into()),
            (Element::Dialogue, "Left side.".into()),
            (Element::Character, "BEN".into()),
            (Element::Dialogue, "Right side, two lines.".into()),
            (Element::Action, "They stop.".into()),
        ]
    );
    assert!(r.script.paras[3].dual);
}
