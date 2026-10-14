//! Final Draft compatibility: fixtures written for these tests in the structure observed in
//! files saved by Final Draft (see `docs/fdx-format.md`). No Final Draft file is copied here.

use super::*;
use scriptcraft_doc::{CardColor, Paper, RevMark};

/// A small script in the shape Final Draft 12/13 saves (`Version="6"`): unrevised runs carry
/// `RevisionID="0"`, sets start at 1, the default header has the revision label at a centre
/// tab and the page number at a right tab, and the title page has its own header.
const FINAL_DRAFT_STYLE: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="6">
  <Content>
    <Paragraph Number="1" Type="Scene Heading">
      <SceneProperties Length="1/8" Page="1" Title=""/>
      <Text AdornmentStyle="0" Background="#FFFFFFFFFFFF" Color="#000000000000" Font="Courier Final Draft" RevisionID="0" Size="12" Style="">INT. LIGHTHOUSE - NIGHT</Text>
    </Paragraph>
    <Paragraph Type="Action">
      <ScriptNote Color="#000000000000" DateModified="1" DateTime="1" Name="" Type="">
        <Paragraph><Text>Brighter?</Text></Paragraph>
      </ScriptNote>
      <Text RevisionID="0">The lamp turns. </Text><Text RevisionID="2">A gull lands.</Text>
    </Paragraph>
    <Paragraph Type="Character"><Text RevisionID="0">MIRA</Text></Paragraph>
    <Paragraph Type="Dialogue">
      <Text RevisionID="0">Not </Text>
      <ScriptNote Color="#89895757CFCF" DateModified="1" DateTime="1" Name="" Type="">
        <Paragraph><Text>Line reading</Text></Paragraph>
      </ScriptNote>
      <Text RevisionID="0">tonight.</Text>
    </Paragraph>
  </Content>
  <HeaderAndFooter FooterFirstPage="Yes" FooterVisible="No" HeaderFirstPage="No" HeaderVisible="Yes" StartingPage="1">
    <Header>
      <Paragraph Alignment="Left" FirstIndent="0.00" Leading="Regular" LeftIndent="1.25" RightIndent="-1.00" SpaceBefore="0" Spacing="1" StartsNewPage="No">
        <Text>	</Text><DynamicLabel Type="Last Revised"/><Text>	</Text><DynamicLabel Type="Page #"/><Text>.</Text>
        <Tabstops><Tabstop Position="4.50" Type="Center"/><Tabstop Position="7.25" Type="Right"/></Tabstops>
      </Paragraph>
    </Header>
    <Footer><Paragraph Alignment="Right"><Text></Text></Paragraph></Footer>
  </HeaderAndFooter>
  <PageLayout BottomMargin="72" FooterMargin="36" HeaderMargin="36" TopMargin="72"><PageSize Height="11.00" Width="8.50"/></PageLayout>
  <TitlePage>
    <HeaderAndFooter FooterFirstPage="Yes" FooterVisible="No" HeaderFirstPage="No" HeaderVisible="Yes" StartingPage="1">
      <Header><Paragraph Alignment="Right"><Text> </Text><DynamicLabel Type="Page #"/><Text>.</Text></Paragraph></Header>
    </HeaderAndFooter>
    <Content>
      <Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"><Text Style="Underline+AllCaps">the keeper</Text></Paragraph>
      <Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"/>
      <Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"><Text>Written by</Text></Paragraph>
      <Paragraph Alignment="Center" LeftIndent="1.00" RightIndent="7.50"><Text>Ana Ruiz</Text></Paragraph>
      <Paragraph Alignment="Left" LeftIndent="1.00" RightIndent="7.50"><Text>ana@example.com</Text></Paragraph>
    </Content>
  </TitlePage>
  <MoresAndContinueds>
    <FontSpec Size="12" Style=""/>
    <DialogueBreaks AutomaticCharacterContinueds="No" BottomOfPage="Yes" DialogueBottom="(MORE)" DialogueTop="(CONT'D)" TopOfNext="Yes"/>
    <SceneBreaks ContinuedNumber="No" SceneBottom="(CONTINUED)" SceneBottomOfPage="No" SceneTop="CONTINUED:" SceneTopOfNext="No"/>
  </MoresAndContinueds>
  <LockedPages/>
  <Revisions ActiveSet="2" Location="7.75" RevisionMode="Yes" RevisionsShown="Active" ShowAllMarks="No" ShowAllSets="No" ShowPageColor="Yes">
    <Revision Color="#000000000000" FullRevision="No" ID="1" Mark="*" Name="Blue Rev. (mm/dd/yy)" PageColor="#C6C6EDEDFEFE" Style=""/>
    <Revision Color="#000000000000" FullRevision="No" ID="2" Mark="*" Name="Pink Rev. (10/12/26)" PageColor="#FBFBA4A4B4B4" Style=""/>
  </Revisions>
  <SceneNumberOptions LeftLocation="0.75" NumberScheme="1A" RightLocation="7.38" ShowNumbersOnLeft="Yes" ShowNumbersOnRight="Yes"/>
</FinalDraft>
"##;

#[test]
fn reads_a_final_draft_style_file() {
    let s = parse(FINAL_DRAFT_STYLE).unwrap();
    assert_eq!(s.paras.len(), 4);
    // RevisionID="0" is unrevised text; ID 2 is the second set (index 1).
    assert!(s.paras.iter().filter(|p| p.element != Element::Action).all(|p| p.revs.is_empty()), "unrevised runs carry no marks");
    assert_eq!(s.paras[1].revs, vec![RevMark { start: 16, end: 29, level: 1 }]);
    let r = &s.settings.revision;
    assert!(r.tracking && r.color_pages);
    assert_eq!(r.active, 1);
    assert_eq!((r.sets[0].name.as_str(), r.sets[0].date.as_str()), ("Blue", ""), "the (mm/dd/yy) placeholder is no date");
    assert_eq!((r.sets[1].name.as_str(), r.sets[1].date.as_str()), ("Pink", "10/12/26"));
    assert_eq!(r.sets[0].color, [0xc6, 0xed, 0xfe], "the page tint is PageColor, not the text Color");
    // Header: the revision label centred, the page number on the right; not the title page's.
    let pg = &s.settings.page;
    assert_eq!((pg.header.left.as_str(), pg.header.center.as_str(), pg.header.right.as_str()), ("", "{revision}", "{page}."));
    assert!(!pg.header_first_page && pg.footer.is_empty());
    assert_eq!(pg.paper, Paper::Letter);
    // Scene numbers shown (the heading has one); (MORE)/(CONT'D) on, automatic (CONT'D) off.
    assert!(s.settings.show_scene_numbers && s.settings.more_contd && !s.settings.auto_contd);
    // Notes anchored among the runs: at the start (whole paragraph) and mid-paragraph (the rest).
    let a = &s.paras[1].notes[0];
    assert_eq!((a.start, a.end, a.text.as_str()), (0, s.paras[1].text.len(), "Brighter?"));
    assert_eq!(a.color, CardColor::Yellow, "Final Draft's black default marker is our default colour");
    let b = &s.paras[3].notes[0];
    assert_eq!((s.paras[3].slice(b.start, b.end), b.text.as_str()), ("tonight.", "Line reading"));
    assert_ne!(b.color, CardColor::Yellow);
    // An underlined title doesn't fit the fields: the title page stays free-form.
    assert!(s.title.is_free_form());
    assert_eq!(s.title.lines[0].text, "THE KEEPER");
    assert_eq!((s.title.title.as_str(), s.title.author.as_str(), s.title.contact.as_str()), ("THE KEEPER", "Ana Ruiz", "ana@example.com"));
}

#[test]
fn writes_final_drafts_forms() {
    let s = parse(FINAL_DRAFT_STYLE).unwrap();
    let xml = write(&s);
    for want in [
        "RevisionMode=\"Yes\"",
        "ShowPageColor=\"Yes\"",
        "ActiveSet=\"2\"",
        "ID=\"1\" Mark=\"*\" Name=\"Blue Rev.\" PageColor=\"#C6C6EDEDFEFE\"",
        "Name=\"Pink Rev. (10/12/26)\"",
        "<Text RevisionID=\"2\">A gull lands.</Text>",
        "<PageSize Height=\"11.00\" Width=\"8.50\"/>",
        "<DynamicLabel Type=\"Last Revised\"/>",
        "<Tabstop Position=\"4.50\" Type=\"Center\"/><Tabstop Position=\"7.25\" Type=\"Right\"/>",
        "<DialogueBreaks AutomaticCharacterContinueds=\"No\" BottomOfPage=\"Yes\"",
        "ShowNumbersOnLeft=\"Yes\" ShowNumbersOnRight=\"Yes\"",
        "<Text>Not </Text>\n      <ScriptNote ID=\"1\"",
    ] {
        assert!(xml.contains(want), "missing {want}:\n{xml}");
    }
    // Retired ScriptCraft-only names are no longer written.
    for gone in ["ScriptCraftPaper", "ScriptCraftColorPages", "ShowSceneNumbers=", "RevisionMode=\"On", "Type=\"Title Page\"", "Type=\"Header\""] {
        assert!(!xml.contains(gone), "still writes {gone}");
    }
    // And it all comes back.
    let back = parse(&xml).unwrap();
    assert_eq!(back.paras, s.paras);
    assert_eq!(back.settings, s.settings);
    assert_eq!(back.title, s.title);
}

#[test]
fn older_scriptcraft_files_still_read() {
    // v0.3/v0.4 ScriptCraft: sets from ID 0 (RevisionID="0" is the first set), the page tint
    // in Color, RevisionMode="On", ScriptCraftColorPages, ScriptCraftPaper, ShowSceneNumbers.
    let xml = r##"<FinalDraft DocumentType="Script" Template="No" Version="5"><Content>
<Paragraph Type="Scene Heading" Number="3"><Text>INT. A - DAY</Text></Paragraph>
<Paragraph Type="Action"><Text RevisionID="0">White text. </Text><Text RevisionID="1">Blue text.</Text></Paragraph></Content>
<ScriptCraftProduction LockedPages="No" LockedSceneNumbers="No" ShowSceneNumbers="No"/>
<PageLayout BottomMargin="72" FooterMargin="36" HeaderMargin="36" TopMargin="72" ScriptCraftPaper="a4"/>
<Revisions ActiveSet="1" RevisionMode="On" ShowAllMarks="No" ShowAllSets="No" ScriptCraftShowMarks="Yes" ScriptCraftColorPages="Yes">
<Revision Color="#FFFFFFFFFFFF" FullRevision="No" ID="0" Mark="*" Name="White" ScriptCraftDate=""/>
<Revision Color="#CFCFE2E2F7F7" FullRevision="No" ID="1" Mark="*" Name="Blue" ScriptCraftDate="10/12/26"/>
</Revisions>
<SceneNumberOptions ShowNumbersOnLeft="Yes" ShowNumbersOnRight="Yes"/></FinalDraft>"##;
    let s = parse(xml).unwrap();
    assert_eq!(s.paras[1].revs, vec![RevMark { start: 0, end: 12, level: 0 }, RevMark { start: 12, end: 22, level: 1 }]);
    let r = &s.settings.revision;
    assert!(r.tracking && r.color_pages && r.show_marks);
    assert_eq!(r.active, 1);
    assert_eq!((r.sets[1].name.as_str(), r.sets[1].date.as_str(), r.sets[1].color), ("Blue", "10/12/26", [0xcf, 0xe2, 0xf7]));
    assert_eq!(s.settings.page.paper, Paper::A4);
    assert!(!s.settings.show_scene_numbers, "the older explicit option wins");
}

#[test]
fn hostile_compat_values_never_panic() {
    for xml in [
        r#"<FinalDraft><PageLayout><PageSize Width="-1" Height="NaN"/></PageLayout><Content/></FinalDraft>"#,
        r##"<FinalDraft><Content><Paragraph Type="Action"><Text RevisionID="99999999999">x</Text><ScriptNote><Paragraph><Text>n</Text></Paragraph></ScriptNote></Paragraph></Content><Revisions RevisionMode="maybe"><Revision ID="x" Name="()" PageColor="#zz"/><Revision Name=" Rev. (" /></Revisions></FinalDraft>"##,
        r#"<FinalDraft><Content/><HeaderAndFooter><Header><Paragraph><Text>a		b			c</Text><Tabstops><Tabstop Type="Up"/></Tabstops></Paragraph></Header></HeaderAndFooter><DialogueBreaks/><SceneNumberOptions/></FinalDraft>"#,
    ] {
        let s = parse(xml).unwrap();
        let _ = parse(&write(&s)).unwrap();
    }
}
