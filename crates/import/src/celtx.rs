//! Celtx: legacy `.celtx` project archives and their script HTML.
//!
//! A legacy `.celtx` file is a ZIP holding `project.rdf` (the project's metadata) and one HTML file
//! per document. A screenplay document's paragraphs are `<p class="…">` with these classes:
//! `sceneheading`, `action`, `character`, `dialog`, `parenthetical`, `transition`, `shot` (and
//! `act` / `text` / others, read as General). The archive's screenplay is the HTML document with
//! the most such paragraphs. A `.celtx` that is just the HTML is read the same way.
//!
//! Not confirmed from public files, and so best effort: inline styling (read from `<b>`, `<i>`,
//! `<u>` and `font-weight` / `font-style` / `text-decoration` styles) and the title, taken from
//! the first `<dc:title>` in `project.rdf`.

use scriptcraft_doc::{Element, Script, Style};

use crate::html::{Tok, tokens};
use crate::shape::{Builder, Styled};
use crate::{Imported, zip};

fn element(class: &str) -> Option<Element> {
    Some(match class {
        "sceneheading" | "scene-heading" | "slugline" => Element::SceneHeading,
        "action" => Element::Action,
        "character" => Element::Character,
        "dialog" | "dialogue" => Element::Dialogue,
        "parenthetical" => Element::Parenthetical,
        "transition" => Element::Transition,
        "shot" => Element::Shot,
        "act" | "text" | "general" => Element::General,
        _ => return None,
    })
}

/// Paragraphs of a script document (and how many carried a known class).
fn read_html(src: &str) -> (Vec<(Element, Styled)>, usize) {
    let mut out: Vec<(Element, Styled)> = Vec::new();
    let mut known = 0;
    let mut cur: Option<(Element, Styled)> = None;
    let mut styles: Vec<(String, Style)> = Vec::new();
    for t in tokens(src) {
        match t {
            Tok::Open(name, attrs) if name == "p" || name == "div" && cur.is_none() => {
                if let Some(p) = cur.take() {
                    out.push(p);
                }
                let class = attrs.iter().find(|(k, _)| k == "class").map(|(_, v)| v.to_ascii_lowercase()).unwrap_or_default();
                let e = class.split_whitespace().find_map(element);
                if e.is_some() {
                    known += 1;
                }
                if name == "p" || e.is_some() {
                    cur = Some((e.unwrap_or(Element::Action), Styled::default()));
                }
                styles.clear();
            }
            Tok::Close(name) if name == "p" || name == "div" => {
                if let Some(p) = cur.take() {
                    out.push(p);
                }
            }
            Tok::Open(name, attrs) => {
                if name == "br" {
                    if let Some((_, t)) = cur.as_mut() {
                        t.push(' ', Style::PLAIN);
                    }
                    continue;
                }
                let mut st = styles.last().map_or(Style::PLAIN, |(_, s)| *s);
                let css = attrs.iter().find(|(k, _)| k == "style").map(|(_, v)| v.to_ascii_lowercase().replace(' ', "")).unwrap_or_default();
                match name.as_str() {
                    "b" | "strong" => st.bold = true,
                    "i" | "em" => st.italic = true,
                    "u" => st.underline = true,
                    _ => {}
                }
                st.bold |= css.contains("font-weight:bold") || css.contains("font-weight:700");
                st.italic |= css.contains("font-style:italic");
                st.underline |= css.contains("text-decoration:underline");
                if styles.len() < 64 {
                    styles.push((name, st));
                }
            }
            Tok::Close(name) => {
                if let Some(at) = styles.iter().rposition(|(n, _)| *n == name) {
                    styles.truncate(at);
                }
            }
            Tok::Text(s) => {
                if let Some((_, t)) = cur.as_mut() {
                    t.push_str(&s, styles.last().map_or(Style::PLAIN, |(_, s)| *s));
                }
            }
        }
    }
    if let Some(p) = cur.take() {
        out.push(p);
    }
    (out, known)
}

fn rdf_title(src: &str) -> Option<String> {
    let start = src.find("<dc:title")?;
    let rest = src.get(start..)?;
    let open_end = rest.find('>')?;
    let body = rest.get(open_end + 1..)?;
    let close = body.find("</dc:title>")?;
    let t = crate::html::decode(body.get(..close)?.trim());
    (!t.is_empty() && t.len() < 500).then_some(t)
}

pub fn import(bytes: &[u8]) -> Result<Imported, String> {
    let mut notes = Vec::new();
    let mut title = None;
    let (paras, known) = if zip::is_zip(bytes) {
        let z = zip::Zip::open(bytes)?;
        let mut best: Option<(Vec<(Element, Styled)>, usize, String)> = None;
        for e in z.entries.iter().filter(|e| {
            let n = e.name.to_ascii_lowercase();
            n.ends_with(".html") || n.ends_with(".htm") || n.ends_with(".xhtml")
        }) {
            let Ok(data) = z.read(e) else { continue };
            let (p, k) = read_html(&String::from_utf8_lossy(&data));
            if best.as_ref().is_none_or(|(_, bk, _)| k > *bk) {
                best = Some((p, k, e.name.clone()));
            }
        }
        if let Some(rdf) = z.read_named("project.rdf") {
            title = rdf_title(&String::from_utf8_lossy(&rdf));
        }
        let (p, k, name) = best.ok_or("no script document in the Celtx project")?;
        notes.push(format!("read the screenplay from {name}"));
        (p, k)
    } else {
        read_html(&String::from_utf8_lossy(bytes))
    };
    if known == 0 {
        return Err("no Celtx screenplay paragraphs found".into());
    }
    let mut b = Builder::default();
    for (e, t) in paras {
        b.start(e, t);
    }
    let mut script = Script::new();
    let paras = b.finish();
    if !paras.is_empty() {
        script.paras = paras;
    }
    if let Some(t) = title {
        script.title.title = t;
        script.settings.title_page = true;
    }
    Ok(Imported { script, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const HTML: &str = r#"<html><head><style>p.action{}</style></head><body>
<p class="sceneheading">INT. LIGHTHOUSE - NIGHT</p>
<p class="action">The lamp turns. <b>Slowly.</b></p>
<p class="character">NELL</p>
<p class="parenthetical">(whispering)</p>
<p class="dialog">Can you hear<br>that?</p>
<p class="transition">CUT TO:</p>
</body></html>"#;

    #[test]
    fn reads_html_and_archives() {
        let r = import(HTML.as_bytes()).unwrap();
        let got: Vec<(Element, &str)> = r.script.paras.iter().map(|p| (p.element, p.text.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (Element::SceneHeading, "INT. LIGHTHOUSE - NIGHT"),
                (Element::Action, "The lamp turns. Slowly."),
                (Element::Character, "NELL"),
                (Element::Parenthetical, "(whispering)"),
                (Element::Dialogue, "Can you hear that?"),
                (Element::Transition, "CUT TO:"),
            ]
        );
        assert!(r.script.paras[1].spans.iter().any(|s| s.style.bold));
        let rdf = r#"<rdf:RDF><cx:Project><dc:title>The Lamp</dc:title></cx:Project></rdf:RDF>"#;
        let z = zip::tests::build(&[("project.rdf", rdf.as_bytes()), ("notes.html", b"<p>hi</p>"), ("script-1.html", HTML.as_bytes())]);
        let r = import(&z).unwrap();
        assert_eq!(r.script.title.title, "The Lamp");
        assert_eq!(r.script.paras.len(), 6);
        assert!(import(b"<p>no classes</p>").is_err());
    }
}
