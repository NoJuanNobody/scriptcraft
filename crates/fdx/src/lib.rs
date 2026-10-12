//! Final Draft XML (`.fdx`) import and export.
//!
//! Implemented clean-room from the format's publicly visible structure (it is plain XML):
//!
//! ```xml
//! <FinalDraft DocumentType="Script" Template="No" Version="5">
//!   <Content>
//!     <Paragraph Type="Scene Heading" Number="1">
//!       <SceneProperties><Summary><Paragraph><Text>…</Text></Paragraph></Summary></SceneProperties>
//!       <Text>INT. DINER - NIGHT</Text>
//!     </Paragraph>
//!     <Paragraph Type="Action"><Text>Rain on the glass.</Text></Paragraph>
//!     <Paragraph><DualDialogue>…two dialogue blocks…</DualDialogue></Paragraph>
//!   </Content>
//!   <TitlePage><Content><Paragraph Alignment="Center"><Text>TITLE</Text></Paragraph>…</Content></TitlePage>
//! </FinalDraft>
//! ```
//!
//! Text runs (`<Text Style="Bold">`) are concatenated: inline styles are not modelled yet.
//! Unknown paragraph types (`Cast List`, `New Act`, `Lyrics`…) import as General.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use quick_xml::Reader;
use quick_xml::events::Event;
use scriptcraft_doc::{Element, MAX_PARA_BYTES, MAX_PARAS, Para, Script, TitlePage};

/// Largest input we parse, bytes.
pub const MAX_INPUT: usize = 128 << 20;
/// Deepest element nesting we follow.
const MAX_DEPTH: usize = 256;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum FdxError {
    #[error("not a Final Draft script (root element is `{0}`)")]
    NotFdx(String),
    #[error("malformed XML: {0}")]
    Xml(String),
    #[error("the file is too large")]
    TooLarge,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Body,
    Summary,
    Title,
}

struct Open {
    kind: Kind,
    ty: Option<String>,
    number: Option<String>,
    align: String,
    text: String,
    summary: String,
    wrapper: bool,
    in_text: bool,
}

/// Parse an FDX document.
pub fn parse(src: &str) -> Result<Script, FdxError> {
    if src.len() > MAX_INPUT {
        return Err(FdxError::TooLarge);
    }
    let mut r = Reader::from_str(src);
    let mut stack: Vec<String> = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    let mut paras: Vec<Para> = Vec::new();
    let mut title_lines: Vec<(String, String)> = Vec::new();
    // Characters seen in the current DualDialogue (the second and later are `dual`).
    let mut dual_chars: Option<usize> = None;
    let mut root_seen = false;
    loop {
        let ev = r.read_event().map_err(|e| FdxError::Xml(e.to_string()))?;
        let empty = matches!(ev, Event::Empty(_));
        match ev {
            Event::Start(e) | Event::Empty(e) if stack.len() < MAX_DEPTH => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if !root_seen {
                    root_seen = true;
                    if name != "FinalDraft" {
                        return Err(FdxError::NotFdx(name));
                    }
                }
                let attr = |k: &str| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().as_ref() == k.as_bytes())
                        .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.into_owned()))
                };
                match name.as_str() {
                    "Paragraph" => {
                        let kind = if stack.iter().any(|n| n == "Summary") {
                            Kind::Summary
                        } else if stack.iter().any(|n| n == "TitlePage") {
                            Kind::Title
                        } else {
                            Kind::Body
                        };
                        let o = Open {
                            kind,
                            ty: attr("Type"),
                            number: attr("Number"),
                            align: attr("Alignment").unwrap_or_default(),
                            text: String::new(),
                            summary: String::new(),
                            wrapper: false,
                            in_text: false,
                        };
                        if empty {
                            finish(o, &mut open, &mut paras, &mut title_lines, &mut dual_chars);
                        } else {
                            open.push(o);
                        }
                    }
                    "Text" => {
                        if let Some(o) = open.last_mut() {
                            o.in_text = !empty;
                        }
                    }
                    "DualDialogue" => {
                        if let Some(o) = open.last_mut() {
                            o.wrapper = true;
                        }
                        if !empty {
                            dual_chars = Some(0);
                        }
                    }
                    _ => {}
                }
                if !empty {
                    stack.push(name);
                }
            }
            Event::Start(_) => {
                stack.push(String::new());
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                stack.pop();
                match name.as_str() {
                    "Text" => {
                        if let Some(o) = open.last_mut() {
                            o.in_text = false;
                        }
                    }
                    "DualDialogue" => dual_chars = None,
                    "Paragraph" => {
                        if let Some(o) = open.pop() {
                            finish(o, &mut open, &mut paras, &mut title_lines, &mut dual_chars);
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                if let Some(o) = open.last_mut().filter(|o| o.in_text) {
                    push_text(&mut o.text, &t.decode().map_err(|e| FdxError::Xml(e.to_string()))?);
                }
            }
            Event::CData(t) => {
                if let Some(o) = open.last_mut().filter(|o| o.in_text) {
                    push_text(&mut o.text, &t.decode().map_err(|e| FdxError::Xml(e.to_string()))?);
                }
            }
            Event::GeneralRef(g) => {
                if let Some(o) = open.last_mut().filter(|o| o.in_text) {
                    let c = match g.resolve_char_ref() {
                        Ok(Some(c)) => Some(c.to_string()),
                        _ => match g.decode().map_err(|e| FdxError::Xml(e.to_string()))?.as_ref() {
                            "amp" => Some("&".into()),
                            "lt" => Some("<".into()),
                            "gt" => Some(">".into()),
                            "quot" => Some("\"".into()),
                            "apos" => Some("'".into()),
                            _ => None,
                        },
                    };
                    if let Some(c) = c {
                        push_text(&mut o.text, &c);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        if paras.len() > MAX_PARAS {
            return Err(FdxError::TooLarge);
        }
    }
    if !root_seen {
        return Err(FdxError::NotFdx(String::new()));
    }
    let mut s = Script::from_paras(paras).map_err(|_| FdxError::TooLarge)?;
    s.title = title_from_lines(&title_lines);
    Ok(s)
}

fn push_text(dst: &mut String, s: &str) {
    if dst.len() + s.len() <= MAX_PARA_BYTES {
        dst.push_str(s);
    }
}

fn finish(o: Open, open: &mut [Open], paras: &mut Vec<Para>, title_lines: &mut Vec<(String, String)>, dual_chars: &mut Option<usize>) {
    match o.kind {
        Kind::Summary => {
            if let Some(parent) = open.iter_mut().rev().find(|p| p.kind == Kind::Body) {
                let t = o.text.trim();
                if !t.is_empty() {
                    if !parent.summary.is_empty() {
                        parent.summary.push(' ');
                    }
                    parent.summary.push_str(t);
                }
            }
        }
        Kind::Title => title_lines.push((o.align, o.text)),
        Kind::Body => {
            if o.wrapper && o.text.trim().is_empty() {
                return;
            }
            let element = o.ty.as_deref().and_then(Element::parse).unwrap_or(match o.ty.as_deref() {
                Some("Lyrics") | Some("Cast List") => Element::Action,
                _ => Element::General,
            });
            let mut p = Para::new(element, o.text);
            if element == Element::SceneHeading {
                p.scene_number = o.number.filter(|n| !n.trim().is_empty());
                p.summary = o.summary;
            }
            if element == Element::Character
                && let Some(n) = dual_chars.as_mut()
            {
                p.dual = *n > 0;
                *n += 1;
            }
            paras.push(p);
        }
    }
}

fn is_credit(l: &str) -> bool {
    let l = l.trim().to_lowercase();
    l == "by"
        || l.ends_with(" by")
        || l.starts_with("written by")
        || l.starts_with("screenplay by")
        || l.starts_with("teleplay by")
        || l.starts_with("story by")
}

/// Map free-form title-page paragraphs onto fields: the first centred line is the title, a
/// "written by" line the credit, the next centred line the author, later centred lines the
/// source; left-aligned lines are the contact block, right-aligned ones the draft date.
fn title_from_lines(lines: &[(String, String)]) -> TitlePage {
    let mut t = TitlePage::default();
    let add = |slot: &mut String, v: &str| {
        if !slot.is_empty() {
            slot.push('\n');
        }
        slot.push_str(v);
    };
    let mut stage = 0; // 0 title, 1 credit, 2 author, 3 source
    for (align, text) in lines {
        let v = text.trim();
        if v.is_empty() {
            continue;
        }
        match align.as_str() {
            "Right" => add(&mut t.draft_date, v),
            "Left" | "Full" | "" if stage > 0 || align == "Left" => add(&mut t.contact, v),
            _ => match stage {
                0 => {
                    if is_credit(v) {
                        t.credit = v.to_string();
                        stage = 2;
                    } else {
                        add(&mut t.title, v);
                        stage = 1;
                    }
                }
                1 if is_credit(v) => {
                    t.credit = v.to_string();
                    stage = 2;
                }
                1 | 2 => {
                    add(&mut t.author, v);
                    stage = 3;
                }
                _ => add(&mut t.source, v),
            },
        }
    }
    t
}

/// XML-escape text (and drop characters XML 1.0 forbids).
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

fn para_xml(out: &mut String, indent: &str, p: &Para) {
    out.push_str(&format!("{indent}<Paragraph Type=\"{}\"", p.element.name()));
    if let Some(n) = p.scene_number.as_deref().filter(|n| !n.trim().is_empty()) {
        out.push_str(&format!(" Number=\"{}\"", esc(n.trim())));
    }
    out.push_str(">\n");
    if p.element == Element::SceneHeading && !p.summary.trim().is_empty() {
        out.push_str(&format!(
            "{indent}  <SceneProperties Title=\"\">\n{indent}    <Summary>\n{indent}      <Paragraph Type=\"Action\">\n{indent}        <Text>{}</Text>\n{indent}      </Paragraph>\n{indent}    </Summary>\n{indent}  </SceneProperties>\n",
            esc(p.summary.trim())
        ));
    }
    out.push_str(&format!("{indent}  <Text>{}</Text>\n{indent}</Paragraph>\n", esc(&p.text)));
}

/// The dialogue block starting at `i` (a Character and its Parenthetical/Dialogue lines): its end.
fn block_end(paras: &[Para], i: usize) -> usize {
    let mut j = i + 1;
    while paras.get(j).is_some_and(|p| matches!(p.element, Element::Parenthetical | Element::Dialogue)) {
        j += 1;
    }
    j
}

/// Write a script as FDX.
pub fn write(s: &Script) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    out.push_str("<FinalDraft DocumentType=\"Script\" Template=\"No\" Version=\"5\">\n  <Content>\n");
    let paras = &s.paras;
    let mut i = 0;
    while let Some(p) = paras.get(i) {
        if p.element == Element::Character && !p.dual {
            let end = block_end(paras, i);
            if paras.get(end).is_some_and(|n| n.element == Element::Character && n.dual) {
                let end2 = block_end(paras, end);
                out.push_str("    <Paragraph>\n      <DualDialogue>\n");
                for q in paras.get(i..end2).unwrap_or(&[]) {
                    para_xml(&mut out, "        ", q);
                }
                out.push_str("      </DualDialogue>\n    </Paragraph>\n");
                i = end2;
                continue;
            }
        }
        para_xml(&mut out, "    ", p);
        i += 1;
    }
    out.push_str("  </Content>\n");
    let t = &s.title;
    if !t.is_empty() {
        out.push_str("  <TitlePage>\n    <Content>\n");
        let mut line = |align: &str, text: &str| {
            out.push_str(&format!(
                "      <Paragraph Alignment=\"{align}\" Type=\"Title Page\">\n        <Text>{}</Text>\n      </Paragraph>\n",
                esc(text)
            ));
        };
        for _ in 0..12 {
            line("Center", "");
        }
        let centered: [&str; 4] = [&t.title, &t.credit, &t.author, &t.source];
        for (k, v) in centered.iter().enumerate() {
            for l in v.lines().filter(|l| !l.trim().is_empty()) {
                line("Center", l.trim());
            }
            if k == 0 || k == 2 {
                line("Center", "");
            }
        }
        for _ in 0..16 {
            line("Center", "");
        }
        for v in [&t.contact, &t.copyright, &t.notes] {
            for l in v.lines().filter(|l| !l.trim().is_empty()) {
                line("Left", l.trim());
            }
        }
        for l in t.draft_date.lines().filter(|l| !l.trim().is_empty()) {
            line("Right", l.trim());
        }
        out.push_str("    </Content>\n  </TitlePage>\n");
    }
    out.push_str("</FinalDraft>\n");
    out
}

#[cfg(test)]
mod tests;
