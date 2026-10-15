//! Word (`.docx`, Office Open XML): `word/document.xml` with `word/styles.xml`.
//!
//! Each `w:p` is one paragraph. Its element comes from, in order:
//! 1. its paragraph style's name (or a style it is based on), when that names a screenplay
//!    element (`Scene Heading`, `Slugline`, `Action`, `Character`, `Dialogue`, `Parenthetical`,
//!    `Transition`, `Shot`, `General`, …);
//! 2. its left indent and alignment on the page (plus leading tabs or spaces), by the same
//!    Courier 12 conventions the PDF import uses, when the document's paragraphs sit at more than
//!    one indent;
//! 3. otherwise the text alone, read the way Fountain reads plain text (blank paragraphs separate
//!    elements; a line in capitals followed by text is a character and dialogue).
//!
//! Bold, italic and underline runs are kept; hidden text (`w:vanish`) is skipped. Paragraphs
//! before the first page break, when that page has no scene heading, become the title page.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use scriptcraft_doc::{Element, Script, Style};

use crate::shape::{self, Builder, Geometry, Styled, TitleBlocks};
use crate::{Imported, zip};

/// Twips per inch.
const TWIP: f32 = 1440.0;

#[derive(Clone, Debug, Default)]
struct PStyle {
    name: String,
    based_on: Option<String>,
    left: Option<f32>,
    first: Option<f32>,
    align: Option<String>,
    run: RunProps,
}

#[derive(Clone, Copy, Debug, Default)]
struct RunProps {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    caps: Option<bool>,
    hidden: Option<bool>,
}

impl RunProps {
    fn over(self, base: RunProps) -> RunProps {
        RunProps {
            bold: self.bold.or(base.bold),
            italic: self.italic.or(base.italic),
            underline: self.underline.or(base.underline),
            caps: self.caps.or(base.caps),
            hidden: self.hidden.or(base.hidden),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct WPara {
    style: Option<String>,
    left: Option<f32>,
    first: Option<f32>,
    align: Option<String>,
    text: Styled,
    lead: f32,
    page_break_before: bool,
    /// A page break inside or after this paragraph.
    page_break_after: bool,
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key || a.key.local_name().as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn on_off(e: &BytesStart) -> bool {
    !matches!(attr(e, b"w:val").as_deref(), Some("0" | "false" | "off" | "none"))
}

fn twips(v: Option<String>) -> Option<f32> {
    v?.trim().parse::<f32>().ok().filter(|x| x.is_finite()).map(|x| x.clamp(-20_000.0, 40_000.0) / TWIP)
}

/// Paragraph and run properties from `w:pPr` / `w:rPr` children.
fn props(e: &BytesStart, left: &mut Option<f32>, first: &mut Option<f32>, align: &mut Option<String>, run: &mut RunProps, in_rpr: bool) {
    match e.local_name().as_ref() {
        b"ind" => {
            if let Some(l) = twips(attr(e, b"w:left").or_else(|| attr(e, b"w:start"))) {
                *left = Some(l);
            }
            if let Some(h) = twips(attr(e, b"w:hanging")) {
                *first = Some(-h);
            } else if let Some(f) = twips(attr(e, b"w:firstLine")) {
                *first = Some(f);
            }
        }
        b"jc" => *align = attr(e, b"w:val"),
        b"b" if in_rpr => run.bold = Some(on_off(e)),
        b"i" if in_rpr => run.italic = Some(on_off(e)),
        b"u" if in_rpr => run.underline = Some(on_off(e)),
        b"caps" if in_rpr => run.caps = Some(on_off(e)),
        b"vanish" if in_rpr => run.hidden = Some(on_off(e)),
        _ => {}
    }
}

fn read_styles(xml: &str) -> (Vec<(String, PStyle)>, RunProps) {
    let mut out: Vec<(String, PStyle)> = Vec::new();
    let mut defaults = RunProps::default();
    let mut r = Reader::from_str(xml);
    let mut cur: Option<(String, PStyle)> = None;
    let (mut in_rpr, mut in_defaults) = (false, false);
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 4_000_000 {
            break;
        }
        let ev = match r.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match ev {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                b"style" => {
                    let is_para = attr(&e, b"w:type").is_none_or(|t| t == "paragraph");
                    cur = is_para.then(|| (attr(&e, b"w:styleId").unwrap_or_default(), PStyle::default()));
                }
                b"rPrDefault" => in_defaults = true,
                b"rPr" => in_rpr = true,
                b"name" => {
                    if let Some((_, s)) = cur.as_mut() {
                        s.name = attr(&e, b"w:val").unwrap_or_default();
                    }
                }
                b"basedOn" => {
                    if let Some((_, s)) = cur.as_mut() {
                        s.based_on = attr(&e, b"w:val");
                    }
                }
                _ => {
                    if let Some((_, s)) = cur.as_mut() {
                        let (mut l, mut f, mut a, mut run) = (s.left, s.first, s.align.clone(), s.run);
                        props(&e, &mut l, &mut f, &mut a, &mut run, in_rpr);
                        (s.left, s.first, s.align, s.run) = (l, f, a, run);
                    } else if in_defaults && in_rpr {
                        let (mut l, mut f, mut a) = (None, None, None);
                        props(&e, &mut l, &mut f, &mut a, &mut defaults, true);
                    }
                }
            },
            Event::End(e) => match e.local_name().as_ref() {
                b"style" => {
                    if let Some(s) = cur.take()
                        && out.len() < 10_000
                    {
                        out.push(s);
                    }
                }
                b"rPr" => in_rpr = false,
                b"rPrDefault" => in_defaults = false,
                _ => {}
            },
            _ => {}
        }
    }
    (out, defaults)
}

/// The element a style name stands for.
fn style_element(name: &str) -> Option<Element> {
    let n: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    Some(match n.as_str() {
        "sceneheading" | "slugline" | "scene" | "sceneheader" | "heading1screenplay" => Element::SceneHeading,
        "action" | "description" | "scenedescription" | "actiondescription" => Element::Action,
        "character" | "charactername" | "charactercue" | "speaker" => Element::Character,
        "dialogue" | "dialog" => Element::Dialogue,
        "parenthetical" | "parentheticals" | "parenthesis" | "parens" | "wryly" => Element::Parenthetical,
        "transition" | "transitions" => Element::Transition,
        "shot" | "shots" => Element::Shot,
        "general" | "newact" | "endofact" | "act" => Element::General,
        _ => return None,
    })
}

struct Styles {
    list: Vec<(String, PStyle)>,
    defaults: RunProps,
}

impl Styles {
    fn get(&self, id: &str) -> Option<&PStyle> {
        self.list.iter().find(|(k, _)| k == id).map(|(_, s)| s)
    }
    /// Follow `basedOn` (bounded).
    fn chain(&self, id: Option<&str>) -> Vec<&PStyle> {
        let mut out = Vec::new();
        let mut cur = id.map(str::to_string);
        while let Some(c) = cur {
            if out.len() >= 16 {
                break;
            }
            let Some(s) = self.get(&c) else { break };
            out.push(s);
            cur = s.based_on.clone();
        }
        out
    }
    fn element(&self, id: Option<&str>) -> Option<Element> {
        self.chain(id).iter().find_map(|s| style_element(&s.name).or_else(|| style_element(id.unwrap_or(""))))
    }
    fn run(&self, id: Option<&str>) -> RunProps {
        self.chain(id).iter().fold(RunProps::default(), |acc, s| acc.over(s.run)).over(self.defaults)
    }
    fn indent(&self, id: Option<&str>) -> (Option<f32>, Option<f32>, Option<String>) {
        let ch = self.chain(id);
        (ch.iter().find_map(|s| s.left), ch.iter().find_map(|s| s.first), ch.iter().find_map(|s| s.align.clone()))
    }
}

fn read_document(xml: &str, styles: &Styles) -> (Vec<WPara>, f32, f32) {
    let mut paras: Vec<WPara> = Vec::new();
    let (mut margin, mut page_w) = (1.0f32, 8.5f32);
    let mut r = Reader::from_str(xml);
    let mut cur: Option<WPara> = None;
    let (mut in_ppr, mut in_rpr, mut in_t) = (false, false, false);
    let mut run = RunProps::default();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 20_000_000 || paras.len() > scriptcraft_doc::MAX_PARAS {
            break;
        }
        let ev = match r.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match ev {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                b"p" => cur = Some(WPara::default()),
                b"pPr" => in_ppr = true,
                b"rPr" => in_rpr = true,
                b"r" => run = RunProps::default(),
                b"t" => in_t = true,
                b"pStyle" if in_ppr => {
                    if let Some(p) = cur.as_mut() {
                        p.style = attr(&e, b"w:val");
                    }
                }
                b"pageBreakBefore" if in_ppr => {
                    if let Some(p) = cur.as_mut() {
                        p.page_break_before = on_off(&e);
                    }
                }
                b"tab" if !in_ppr => {
                    if let Some(p) = cur.as_mut() {
                        p.text.push('\t', Style::PLAIN);
                    }
                }
                b"br" | b"cr" => {
                    if let Some(p) = cur.as_mut() {
                        if attr(&e, b"w:type").as_deref() == Some("page") {
                            p.page_break_after = true;
                        } else {
                            p.text.push(' ', Style::PLAIN);
                        }
                    }
                }
                b"pgMar" => {
                    if let Some(l) = twips(attr(&e, b"w:left")) {
                        margin = l.clamp(0.0, 4.0);
                    }
                }
                b"pgSz" => {
                    if let Some(w) = twips(attr(&e, b"w:w")) {
                        page_w = w.clamp(3.0, 20.0);
                    }
                }
                _ => {
                    if let Some(p) = cur.as_mut() {
                        if in_ppr && !in_rpr {
                            let mut ignored = RunProps::default();
                            let (mut l, mut f, mut a) = (p.left, p.first, p.align.clone());
                            props(&e, &mut l, &mut f, &mut a, &mut ignored, false);
                            (p.left, p.first, p.align) = (l, f, a);
                        } else if in_rpr && !in_ppr {
                            let (mut l, mut f, mut a) = (None, None, None);
                            props(&e, &mut l, &mut f, &mut a, &mut run, true);
                        }
                    }
                }
            },
            Event::Text(t) => {
                if in_t && let Some(p) = cur.as_mut() {
                    let s = t.decode().map(|c| c.into_owned()).unwrap_or_default();
                    let rp = run.over(styles.run(p.style.as_deref()));
                    if rp.hidden != Some(true) {
                        let st = Style { bold: rp.bold == Some(true), italic: rp.italic == Some(true), underline: rp.underline == Some(true) };
                        let s = if rp.caps == Some(true) { s.to_uppercase() } else { s };
                        p.text.push_str(&s, st);
                    }
                }
            }
            Event::GeneralRef(g) => {
                if in_t && let Some(p) = cur.as_mut() {
                    let name = String::from_utf8_lossy(g.as_ref()).into_owned();
                    let s = crate::html::decode(&format!("&{name};"));
                    p.text.push_str(&s, Style::PLAIN);
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"p" => {
                    if let Some(p) = cur.take() {
                        paras.push(p);
                    }
                }
                b"pPr" => in_ppr = false,
                b"rPr" => in_rpr = false,
                b"t" => in_t = false,
                _ => {}
            },
            _ => {}
        }
    }
    (paras, margin, page_w)
}

/// Leading tabs and spaces as indent (Courier: a tab is ½", a space 0.1"), removed from the text.
fn take_lead(p: &mut WPara) {
    let mut lead = 0.0;
    let mut n = 0;
    for (c, _) in &p.text.0 {
        match c {
            '\t' => lead += 0.5,
            ' ' | '\u{a0}' => lead += 0.1,
            _ => break,
        }
        n += 1;
    }
    p.text.0.drain(..n.min(p.text.0.len()));
    p.lead = lead;
}

/// Fountain markup for styled text, with Fountain's own markup characters escaped.
fn fountain_line(t: &Styled) -> String {
    let mut out = String::new();
    let mut cur = Style::PLAIN;
    let close = |out: &mut String, st: Style| {
        if st.underline {
            out.push('_');
        }
        if st.bold && st.italic {
            out.push_str("***");
        } else if st.bold {
            out.push_str("**");
        } else if st.italic {
            out.push('*');
        }
    };
    let open = |out: &mut String, st: Style| {
        if st.bold && st.italic {
            out.push_str("***");
        } else if st.bold {
            out.push_str("**");
        } else if st.italic {
            out.push('*');
        }
        if st.underline {
            out.push('_');
        }
    };
    for &(c, st) in &t.0 {
        if st != cur {
            close(&mut out, cur);
            open(&mut out, st);
            cur = st;
        }
        if matches!(c, '*' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    close(&mut out, cur);
    let trimmed = out.trim_start();
    if trimmed.starts_with(['#', '=', '~', '>', '@', '!', '[', '.']) && !trimmed.starts_with("...") { format!("!{out}") } else { out }
}

pub fn import(bytes: &[u8]) -> Result<Imported, String> {
    let z = zip::Zip::open(bytes).map_err(|_| "not a Word document (.docx)".to_string())?;
    let doc = z.read_named("word/document.xml").ok_or("not a Word document (no word/document.xml)")?;
    let (list, defaults) = z.read_named("word/styles.xml").map(|s| read_styles(&String::from_utf8_lossy(&s))).unwrap_or_default();
    let styles = Styles { list, defaults };
    let (mut paras, margin, page_w) = read_document(&String::from_utf8_lossy(&doc), &styles);
    for p in &mut paras {
        take_lead(p);
    }
    let mut notes = Vec::new();
    let mut script = Script::new();

    // Title page: everything before the first page break, if it holds no scene heading.
    let brk = paras
        .iter()
        .position(|p| p.page_break_after)
        .map(|i| i + 1)
        .or_else(|| paras.iter().skip(1).position(|p| p.page_break_before).map(|i| i + 1));
    let mut body_start = 0;
    if let Some(n) = brk.filter(|&n| n <= 60) {
        let head = paras.get(..n).unwrap_or(&[]);
        let script_like = head.iter().any(|p| {
            shape::is_heading(&p.text.text()) || styles.element(p.style.as_deref()).is_some_and(|e| e != Element::General && e != Element::Action)
        });
        if !script_like && head.iter().any(|p| !p.text.is_blank()) {
            let mut b = TitleBlocks::default();
            let mut gap = true;
            for p in head {
                let (sl, _, sa) = styles.indent(p.style.as_deref());
                let align = p.align.clone().or(sa).unwrap_or_default();
                let t = p.text.clean().text();
                if t.is_empty() {
                    gap = true;
                    continue;
                }
                match align.as_str() {
                    "center" => {
                        if gap || b.centered.is_empty() {
                            b.centered.push(Vec::new());
                        }
                        if let Some(g) = b.centered.last_mut() {
                            g.push(t);
                        }
                    }
                    "right" | "end" => b.right.push(t),
                    _ if p.left.or(sl).unwrap_or(0.0) > 2.0 => b.right.push(t),
                    _ => b.left.push(t),
                }
                gap = false;
            }
            script.title = shape::title_from(&b);
            script.settings.title_page = true;
            body_start = n;
        }
    }
    let body = paras.get(body_start..).unwrap_or(&[]);

    // Where each paragraph starts on the page.
    let place = |p: &WPara| -> (f32, f32, String) {
        let (sl, sf, sa) = styles.indent(p.style.as_deref());
        let left = p.left.or(sl).unwrap_or(0.0);
        let first = p.first.or(sf).unwrap_or(0.0);
        let x = margin + left + first.max(-left) + p.lead;
        let w = p.text.clean().text().chars().count() as f32 * 0.1;
        (x, x + w, p.align.clone().or(sa).unwrap_or_default())
    };
    let styled = body.iter().filter(|p| !p.text.is_blank()).filter(|p| styles.element(p.style.as_deref()).is_some()).count();
    let nonblank = body.iter().filter(|p| !p.text.is_blank()).count();
    let mut xs: Vec<f32> = body.iter().filter(|p| !p.text.is_blank()).map(|p| (place(p).0 * 10.0).round() / 10.0).collect();
    let heading_x: Vec<f32> = body.iter().filter(|p| shape::is_heading(&p.text.text())).map(|p| place(p).0).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    let by_style = styled * 2 >= nonblank.max(1);
    let by_indent = !by_style && xs.len() >= 2;

    let mut b = Builder::default();
    if by_style || by_indent {
        let base = heading_x.first().copied().unwrap_or_else(|| {
            let all: Vec<f32> = body.iter().filter(|p| !p.text.is_blank()).map(|p| place(p).0).collect();
            shape::base_margin(&all)
        });
        let g = Geometry { base, right: page_w - 1.0 };
        for p in body {
            if p.text.is_blank() {
                continue;
            }
            let (x, x_end, align) = place(p);
            let text = p.text.text();
            let e = styles.element(p.style.as_deref()).filter(|_| by_style).unwrap_or_else(|| match align.as_str() {
                "right" | "end" => Element::Transition,
                "center" if x <= base + 0.3 => Element::General,
                _ => shape::by_indent(x, if align == "right" { g.right } else { x_end }, g, &text),
            });
            let mut t = p.text.clone();
            if e == Element::Character {
                shape::strip_contd(&mut t);
            }
            b.start(e, t);
        }
        notes.push(if by_style { "elements read from paragraph styles".to_string() } else { "elements read from indents".to_string() });
    } else {
        // Plain text: read it as Fountain would.
        let has_blank = body.iter().any(|p| p.text.is_blank());
        let mut src = String::new();
        let texts: Vec<Styled> = body.iter().map(|p| p.text.clean()).collect();
        for (i, t) in texts.iter().enumerate() {
            src.push_str(&fountain_line(t));
            src.push('\n');
            if !has_blank && !t.is_blank() {
                let s = t.text();
                let next_is_text = texts.get(i + 1).is_some_and(|n| !n.is_blank());
                let cue =
                    shape::is_caps(&s) && s.chars().count() <= 40 && !shape::is_heading(&s) && !shape::is_transition_text(&s) && !s.ends_with(':');
                let paren = s.starts_with('(') && s.ends_with(')');
                if !(next_is_text && (cue || paren)) {
                    src.push('\n');
                }
            }
        }
        let parsed = scriptcraft_fountain::parse(&src);
        script.paras = parsed.paras;
        if script.title.is_empty() && !parsed.title.is_empty() {
            script.title = parsed.title;
            script.settings.title_page = true;
        }
        notes.push("no styles or indents: elements read from the text".into());
        return Ok(Imported { script, notes });
    }
    let paras = b.finish();
    if !paras.is_empty() {
        script.paras = paras;
    }
    Ok(Imported { script, notes })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal .docx with these paragraphs: `(style, left indent in twips, alignment, runs)`;
    /// a run starting with `*` is bold, `_` underlined.
    pub fn build(paras: &[(&str, u32, &str, &[&str])], title: &[(&str, &str)]) -> Vec<u8> {
        let mut body = String::new();
        for (align, text) in title {
            body.push_str(&format!("<w:p><w:pPr><w:jc w:val=\"{align}\"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"));
        }
        if !title.is_empty() {
            body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
        }
        for (style, left, align, runs) in paras {
            body.push_str("<w:p><w:pPr>");
            if !style.is_empty() {
                body.push_str(&format!("<w:pStyle w:val=\"{style}\"/>"));
            }
            if *left > 0 {
                body.push_str(&format!("<w:ind w:left=\"{left}\"/>"));
            }
            if !align.is_empty() {
                body.push_str(&format!("<w:jc w:val=\"{align}\"/>"));
            }
            body.push_str("</w:pPr>");
            for r in *runs {
                let (pr, t) = match (r.strip_prefix('*'), r.strip_prefix('_')) {
                    (Some(t), _) => ("<w:rPr><w:b/></w:rPr>", t),
                    (_, Some(t)) => ("<w:rPr><w:u w:val=\"single\"/></w:rPr>", t),
                    _ => ("", *r),
                };
                body.push_str(&format!("<w:r>{pr}<w:t xml:space=\"preserve\">{t}</w:t></w:r>"));
            }
            body.push_str("</w:p>");
        }
        let doc = format!(
            "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"12240\"/><w:pgMar w:left=\"2160\"/></w:sectPr></w:body></w:document>"
        );
        let styles = "<?xml version=\"1.0\"?><w:styles xmlns:w=\"x\">\
            <w:style w:type=\"paragraph\" w:styleId=\"SceneHeading\"><w:name w:val=\"Scene Heading\"/><w:rPr><w:caps/></w:rPr></w:style>\
            <w:style w:type=\"paragraph\" w:styleId=\"Char\"><w:name w:val=\"Character\"/></w:style>\
            <w:style w:type=\"paragraph\" w:styleId=\"Dlg\"><w:name w:val=\"Dialogue\"/></w:style>\
            <w:style w:type=\"paragraph\" w:styleId=\"MyDlg\"><w:name w:val=\"My Speech\"/><w:basedOn w:val=\"Dlg\"/></w:style>\
            <w:style w:type=\"paragraph\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
            </w:styles>";
        crate::zip::tests::build(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("word/document.xml", doc.as_bytes()),
            ("word/styles.xml", styles.as_bytes()),
        ])
    }

    fn els(r: &Imported) -> Vec<(Element, String)> {
        r.script.paras.iter().map(|p| (p.element, p.text.clone())).collect()
    }

    #[test]
    fn by_style_names() {
        let d = build(
            &[
                ("SceneHeading", 0, "", &["int. kitchen - night"]),
                ("Normal", 0, "", &["Rain. ", "*Hard", " rain."]),
                ("Char", 0, "", &["NELL (CONT'D)"]),
                ("MyDlg", 0, "", &["Not ", "_now", "."]),
            ],
            &[
                ("center", "NIGHT SIGNAL"),
                ("center", ""),
                ("center", "Written by"),
                ("center", ""),
                ("center", "Ann Lee"),
                ("left", "ann@example.com"),
            ],
        );
        let r = import(&d).unwrap();
        assert_eq!(
            els(&r),
            vec![
                (Element::SceneHeading, "INT. KITCHEN - NIGHT".into()),
                (Element::Action, "Rain. Hard rain.".into()),
                (Element::Character, "NELL".into()),
                (Element::Dialogue, "Not now.".into()),
            ]
        );
        assert!(r.script.paras[1].spans.iter().any(|s| s.style.bold && s.start == 6));
        assert!(r.script.paras[3].spans.iter().any(|s| s.style.underline));
        let t = &r.script.title;
        assert_eq!(
            (t.title.as_str(), t.credit.as_str(), t.author.as_str(), t.contact.as_str()),
            ("NIGHT SIGNAL", "Written by", "Ann Lee", "ann@example.com")
        );
    }

    #[test]
    fn by_indent_and_plain_text() {
        // Left margin 1.5" (2160 twips); indents relative to it.
        let d = build(
            &[
                ("", 0, "", &["EXT. PIER - DAY"]),
                ("", 0, "", &["Gulls."]),
                ("", 3168, "", &["NELL"]),
                ("", 2304, "", &["(calling)"]),
                ("", 1440, "", &["Hello?"]),
                ("", 0, "right", &["CUT TO:"]),
            ],
            &[],
        );
        assert_eq!(
            els(&import(&d).unwrap()),
            vec![
                (Element::SceneHeading, "EXT. PIER - DAY".into()),
                (Element::Action, "Gulls.".into()),
                (Element::Character, "NELL".into()),
                (Element::Parenthetical, "(calling)".into()),
                (Element::Dialogue, "Hello?".into()),
                (Element::Transition, "CUT TO:".into()),
            ]
        );
        let plain = build(
            &[("", 0, "", &["INT. HUT - NIGHT"]), ("", 0, "", &["Wind *howls*."]), ("", 0, "", &["NELL"]), ("", 0, "", &["Shut the door."])],
            &[],
        );
        assert_eq!(
            els(&import(&plain).unwrap()),
            vec![
                (Element::SceneHeading, "INT. HUT - NIGHT".into()),
                (Element::Action, "Wind *howls*.".into()),
                (Element::Character, "NELL".into()),
                (Element::Dialogue, "Shut the door.".into()),
            ]
        );
        assert!(import(b"PK\x03\x04garbage").is_err());
    }
}
