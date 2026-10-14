use super::*;
use scriptcraft_doc::CardColor;

fn boarded() -> Script {
    let mut s = Script::from_paras(vec![
        Para::new(Element::SceneHeading, "INT. CAFÉ - NIGHT"),
        Para::new(Element::Action, "Ada sets down a key."),
        Para::new(Element::SceneHeading, "EXT. PIER - DAWN"),
        Para::new(Element::Action, "Fog."),
    ])
    .unwrap_or_default();
    let a = s.board.add("Inciting incident", "She finds the key.\nIt opens <nothing> & no one.", Some(CardColor::Pink), 100.0, 220.0).unwrap_or(0);
    let b = s.board.add("Midpoint", "", None, 760.0, 100.0).unwrap_or(0);
    let _c = s.board.add("", "Loose idea", Some(CardColor::Teal), -40.0, 900.0).unwrap_or(0);
    s.link_beat(a, Some(0)).ok();
    s.link_beat(b, Some(1)).ok();
    s.set_bookmark(1, Some("The key")).ok();
    s.set_bookmark(3, Some("Fog & \"weather\"")).ok();
    s
}

#[test]
fn beats_links_and_bookmarks_round_trip() {
    let s = boarded();
    let xml = write(&s);
    assert!(xml.contains("Bookmark=\"The key\""), "{xml}");
    assert!(xml.contains("<ListItem Color=\"#D6D65C5CA8A8\""), "{xml}");
    assert!(xml.contains("Type=\"Beat\""), "{xml}");
    assert!(xml.contains("<DisplayBoard Height=\"10000\" Type=\"Beat\""), "{xml}");
    assert!(xml.contains("ScriptCraftBeats=\""), "{xml}");
    let back = parse(&xml).unwrap_or_default();
    // Beat text never becomes script paragraphs.
    assert_eq!(back.paras.len(), s.paras.len());
    assert_eq!(back.board, s.board);
    for (a, b) in s.paras.iter().zip(&back.paras) {
        assert_eq!(a.text, b.text);
        assert_eq!(a.bookmark, b.bookmark);
        assert_eq!(a.beats, b.beats);
    }
    assert_eq!(back.beat_scene(1), Some(0));
    assert_eq!(back.bookmark_para("fog & \"weather\""), Some(3));
}

/// The structure observed in files Final Draft wrote (text here is our own).
#[test]
fn reads_observed_beat_board() {
    let x = r##"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="6">
  <Content>
    <Paragraph Type="Scene Heading" Bookmark="Start"><Text>INT. ROOM - DAY</Text></Paragraph>
    <Paragraph Type="Action"><Text>A chair.</Text></Paragraph>
  </Content>
  <ListItems>
    <ListItem Color="#E2E29898DDDD" Id="5760921c-c95f-4c59-87aa-000000000001" Title="THE CHAIR" Type="Beat">
      <Content>
        <Paragraph OutlineLevel="0"><Text>It wobbles.</Text></Paragraph>
        <Paragraph OutlineLevel="1"><Text Style="Bold">Nobody fixes it.</Text></Paragraph>
      </Content>
    </ListItem>
    <ListItem Id="84099921-351d-442b-87aa-000000000002" Title="Plain" Type="Beat"><Content/></ListItem>
    <ListItem EndCap="Arrow" EndPoint="84099921-351d-442b-87aa-000000000002" Id="aaaa" StartPoint="5760921c-c95f-4c59-87aa-000000000001" Type="PeerLink"/>
  </ListItems>
  <DisplayBoards>
    <DisplayBoard Height="10000" Id="b1" ScrollOrigin="0,0" Type="Beat" Width="24000" ZoomLevel="100.000">
      <Item Height="100" Id="5760921c-c95f-4c59-87aa-000000000001" Left="760" Top="460" Width="240"/>
    </DisplayBoard>
    <DisplayBoard Height="90" Id="b2" Type="StoryMap" Width="14600">
      <Item Height="55" Id="84099921-351d-442b-87aa-000000000002" Left="9999" Top="9999" Width="87"/>
    </DisplayBoard>
  </DisplayBoards>
</FinalDraft>"##;
    let s = parse(x).unwrap_or_default();
    assert_eq!(s.paras.len(), 2);
    assert_eq!(s.paras[0].bookmark.as_deref(), Some("Start"));
    assert_eq!(s.board.beats.len(), 2);
    let a = &s.board.beats[0];
    assert_eq!((a.title.as_str(), a.text.as_str()), ("THE CHAIR", "It wobbles.\nNobody fixes it."));
    assert_eq!((a.x, a.y, a.w, a.h), (760.0, 460.0, 240.0, 100.0));
    assert!(a.color.is_some());
    assert_eq!(a.key, "5760921c-c95f-4c59-87aa-000000000001");
    // Not on the beat board (only on the story map): a default place, not the story map's.
    let b = &s.board.beats[1];
    assert_eq!(b.title, "Plain");
    assert!(b.x < 9000.0);
    // Its keys are kept on the way out.
    assert!(write(&s).contains("Id=\"5760921c-c95f-4c59-87aa-000000000001\""));
}

#[test]
fn hostile_board_input() {
    let x = r##"<FinalDraft><Content>
<Paragraph Type="Scene Heading" ScriptCraftBeats="missing, ,k1,k1" Bookmark="   "><Text>INT. A - DAY</Text></Paragraph>
<Paragraph Type="Action" ScriptCraftBeats="k1"><Text>x</Text></Paragraph>
</Content>
<ListItems><ListItem Id="k1" Type="Beat" Title="T"><Content><Paragraph><Text>a</Text></Paragraph></Content></ListItem>
<ListItem Id="k1" Type="Beat" Title="Dup"/><ListItem Type="Beat" Title="no id"/></ListItems>
<DisplayBoards><DisplayBoard Type="Beat"><Item Id="k1" Left="1e30" Top="NaN" Width="-5" Height="abc"/></DisplayBoard></DisplayBoards>
</FinalDraft>"##;
    let s = parse(x).unwrap_or_default();
    assert_eq!(s.paras.len(), 2);
    assert_eq!(s.paras[0].bookmark, None);
    assert_eq!(s.board.beats.len(), 1);
    assert_eq!(s.paras[0].beats.len(), 1);
    assert!(s.paras[1].beats.is_empty());
    let b = &s.board.beats[0];
    assert!(b.x.is_finite() && b.y.is_finite() && b.w >= scriptcraft_doc::BEAT_MIN && b.h >= scriptcraft_doc::BEAT_MIN);
}
