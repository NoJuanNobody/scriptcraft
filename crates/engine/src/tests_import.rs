//! Import round trips: a script exported by ScriptCraft (PDF) comes back with the same elements
//! and text; `.docx` / `.celtx` / `.pdf` open through the file commands.

use scriptcraft_doc::{Element, Script, Span, Style};
use serde_json::json;

use crate::Session;
use crate::io::{self, Format};

fn body(s: &Script) -> Vec<(Element, String)> {
    s.paras.iter().filter(|p| !p.text.trim().is_empty()).map(|p| (p.element, p.text.clone())).collect()
}

/// Paragraphs of `a` matched in order in `b` (longest common subsequence), as a share of `a`.
fn accuracy(a: &[(Element, String)], b: &[(Element, String)]) -> f64 {
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i][j] = if a[i] == b[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    dp[0][0] as f64 / a.len().max(1) as f64
}

fn through_pdf(s: &Script) -> Script {
    let pdf = io::encode(s, Format::Pdf).unwrap();
    io::parse(&pdf, Format::Pdf).unwrap()
}

#[test]
fn sample_round_trips_through_pdf() {
    let s = crate::sample::script();
    let back = through_pdf(&s);
    assert_eq!(body(&back), body(&s));
    assert_eq!(back.title.author, s.title.author);
    assert_eq!(back.title.credit, s.title.credit);
    assert_eq!(back.title.draft_date, s.title.draft_date);
    let dual = back.paras.iter().filter(|p| p.dual).count();
    assert_eq!(dual, s.paras.iter().filter(|p| p.dual).count());
}

#[test]
fn long_script_with_scene_numbers_and_styles_round_trips() {
    let one = crate::sample::script();
    let mut s = one.clone();
    for _ in 0..3 {
        s.paras.extend(one.paras.iter().cloned());
    }
    // Bold, italic and underline on some action.
    for p in s.paras.iter_mut().filter(|p| p.element == Element::Action).step_by(3) {
        let end = p.text.find(' ').unwrap_or(p.text.len());
        p.spans = vec![Span { start: 0, end, style: Style { bold: true, italic: false, underline: true } }];
    }
    s.settings.show_scene_numbers = true;
    s.normalize().unwrap();
    s.number_scenes();
    let back = through_pdf(&s);
    let acc = accuracy(&body(&s), &body(&back));
    eprintln!("PDF round trip: {:.1}% of {} paragraphs", acc * 100.0, body(&s).len());
    if std::env::var_os("SC_DIFF").is_some() {
        let (a, b) = (body(&s), body(&back));
        for x in &a {
            if !b.contains(x) {
                eprintln!("MISSING {x:?}");
            }
        }
        for x in &b {
            if !a.contains(x) {
                eprintln!("EXTRA   {x:?}");
            }
        }
    }
    assert!(acc >= 0.99, "accuracy {acc}");
    let numbers: Vec<_> = back.paras.iter().filter_map(|p| p.scene_number.clone()).collect();
    assert_eq!(numbers.len(), s.paras.iter().filter(|p| p.element == Element::SceneHeading).count());
    assert_eq!(numbers.first().map(String::as_str), Some("1"));
    let styled = back.paras.iter().filter(|p| p.spans.iter().any(|x| x.style.bold && x.style.underline)).count();
    let want = s.paras.iter().filter(|p| !p.spans.is_empty()).count();
    assert!(styled * 10 >= want * 9, "styled {styled} of {want}");
}

#[test]
fn file_commands_import() {
    let mut s = Session::new(Script::new());
    let pdf = io::encode(&crate::sample::script(), Format::Pdf).unwrap();
    let v = s.run("file.load", &json!({"name": "draft.pdf", "data": crate::b64::encode(&pdf)})).unwrap();
    assert_eq!(v["format"], "PDF");
    assert!(s.dirty && s.path.is_none(), "an imported script starts unsaved, not tied to its file");
    assert!(s.script.paras.len() > 50);
    let html = br#"<p class="sceneheading">INT. A - DAY</p><p class="character">NELL</p><p class="dialog">Hi.</p>"#;
    let v = s.run("file.import", &json!({"name": "x.celtx", "data": crate::b64::encode(html)})).unwrap();
    assert_eq!((v["paragraphs"].as_u64(), v["format"].as_str()), (Some(3), Some("Celtx")));
    assert!(s.run("file.import", &json!({"name": "x.fdx", "data": "AA=="})).is_err());
    assert!(s.run("file.import", &json!({"name": "x.pdf", "data": crate::b64::encode(b"%PDF-1.4 junk")})).is_err());
    // Saving back as .docx is refused; .fdx works.
    let dir = std::env::temp_dir().join(format!("sc-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(s.run("file.saveAs", &json!({"path": dir.join("x.docx")})).is_err());
    s.run("file.saveAs", &json!({"path": dir.join("x.fdx")})).unwrap();
    let pdf_path = dir.join("y.pdf");
    std::fs::write(&pdf_path, &pdf).unwrap();
    s.run("file.open", &json!({"path": pdf_path})).unwrap();
    assert!(s.path.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_template_and_a4_round_trip_through_pdf() {
    use scriptcraft_doc::{Paper, Template};
    for t in Template::ALL {
        for paper in [Paper::Letter, Paper::A4] {
            let mut s = crate::sample::script();
            t.apply(&mut s.settings);
            s.settings.page.paper = paper;
            s.normalize().unwrap();
            s.number_scenes();
            let back = through_pdf(&s);
            let acc = accuracy(&body(&s), &body(&back));
            eprintln!("{} on {paper:?}: {:.1}%", t.id(), acc * 100.0);
            if std::env::var_os("SC_DIFF").is_some() {
                let (a, b) = (body(&s), body(&back));
                for x in a.iter().filter(|x| !b.contains(x)).take(6) {
                    eprintln!("  MISSING {:?}", (x.0, x.1.chars().take(70).collect::<String>()));
                }
                for x in b.iter().filter(|x| !a.contains(x)).take(6) {
                    eprintln!("  EXTRA   {:?}", (x.0, x.1.chars().take(70).collect::<String>()));
                }
            }
            assert!(acc >= 0.98, "{} {paper:?}: {acc}", t.id());
        }
    }
}
