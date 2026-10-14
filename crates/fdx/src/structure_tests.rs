use super::*;
use scriptcraft_doc::{StructureKind, StructurePoint};

fn script() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::Action, "FADE IN:"),
        Para::new(Element::SceneHeading, "INT. A - DAY"),
        Para::new(Element::Action, "One."),
        Para::new(Element::SceneHeading, "INT. B - DAY"),
        Para::new(Element::Action, "Two."),
    ])
    .unwrap_or_default();
    s.add_structure(StructurePoint::new(StructureKind::Act, "Act One", "Set up <the> world & stakes.", Some(CardColor::Teal)), Some(0));
    s.add_structure(StructurePoint::new(StructureKind::Sequence, "Arrival", "", None), Some(0));
    s.add_structure(StructurePoint::new(StructureKind::Section, "Midpoint", "", None), Some(1));
    s.add_structure(StructurePoint::new(StructureKind::Act, "Act Two", "Not written yet.", None), None);
    s
}

#[test]
fn structure_round_trips_as_outline_paragraphs() {
    let s = script();
    let xml = write(&s);
    assert!(xml.contains("<Paragraph Type=\"Outline 1\" ScriptCraftColor="), "{xml}");
    assert!(xml.contains("<Paragraph Type=\"Outline 2\">") && xml.contains("<Paragraph Type=\"Outline 3\">"));
    assert!(xml.contains("<Paragraph Type=\"Summary\">\n      <Text>Set up &lt;the&gt; world &amp; stakes.</Text>"));
    assert!(xml.contains("<ElementSettings Type=\"Outline 1\">"));
    let back = parse(&xml).unwrap_or_default();
    assert_eq!(back.paras, s.paras);
    assert_eq!(back.structure_tail, s.structure_tail);
    // No outline: nothing extra is written.
    let plain = Script::from_paras(vec![Para::new(Element::SceneHeading, "INT. A - DAY")]).unwrap_or_default();
    assert!(!write(&plain).contains("Outline"));
}

/// Outline paragraphs as Final Draft writes them (structure observed in public files): hidden
/// `Outline N` paragraphs in Content, each followed by `Summary` paragraphs, before a scene.
#[test]
fn reads_final_draft_outline_paragraphs() {
    let x = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="6">
  <Content>
    <Paragraph Type="Outline 1" id="a1"><Text>ACT ONE</Text></Paragraph>
    <Paragraph Type="Summary"><Text>First line.</Text></Paragraph>
    <Paragraph Type="Summary"><Text>Second line.</Text></Paragraph>
    <Paragraph Type="Outline 2"><Text>Opening</Text></Paragraph>
    <Paragraph Type="Scene Heading"><Text>INT. ROOM - DAY</Text></Paragraph>
    <Paragraph Type="Action"><Text>A beat.</Text></Paragraph>
    <Paragraph Type="Summary"><Text>A stray summary stays text.</Text></Paragraph>
    <Paragraph Type="Outline 4"><Text>Deep</Text></Paragraph>
    <Paragraph Type="Scene Heading"><Text>EXT. ROAD - NIGHT</Text></Paragraph>
    <Paragraph Type="Outline 1"><Text>ACT TWO</Text></Paragraph>
  </Content>
  <ElementSettings Type="Outline 1"><Behavior PaginateAs="Outline" ReturnKey="Summary"/></ElementSettings>
</FinalDraft>"#;
    let s = parse(x).unwrap_or_default();
    let pts: Vec<(StructureKind, String, String)> =
        s.structure_points().into_iter().map(|(_, p)| (p.kind, p.title.clone(), p.summary.clone())).collect();
    assert_eq!(
        pts,
        vec![
            (StructureKind::Act, "ACT ONE".into(), "First line. Second line.".into()),
            (StructureKind::Sequence, "Opening".into(), String::new()),
            (StructureKind::Section, "Deep".into(), String::new()),
            (StructureKind::Act, "ACT TWO".into(), String::new()),
        ]
    );
    assert_eq!(s.paras.len(), 4);
    assert_eq!(s.paras[2].element, Element::General);
    assert_eq!(s.paras[3].structure.len(), 1);
    assert_eq!(s.structure_tail.len(), 1);
}

#[test]
fn hostile_outline_paragraphs() {
    for ty in ["Outline", "Outline 0", "Outline x", "Outline 99999999999999999999", "Outlines"] {
        let x = format!(
            "<FinalDraft><Content><Paragraph Type=\"{ty}\"><Text>t</Text></Paragraph><Paragraph Type=\"Scene Heading\"><Text>INT. A</Text></Paragraph></Content></FinalDraft>"
        );
        assert!(parse(&x).is_ok(), "{ty}");
    }
    let many: String = (0..6000).map(|i| format!("<Paragraph Type=\"Outline 1\"><Text>{i}</Text></Paragraph>")).collect();
    let s = parse(&format!("<FinalDraft><Content>{many}</Content></FinalDraft>")).unwrap_or_default();
    assert_eq!(s.structure_count(), scriptcraft_doc::MAX_STRUCTURE);
}
