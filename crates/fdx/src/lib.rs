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
//! Text runs keep their `Style` (`Bold`, `Italic`, `Underline`, combined with `+`); other style
//! values (`AllCaps`, `Strikeout`…) are ignored.
//! Unknown paragraph types (`Cast List`, `New Act`, `Lyrics`…) import as General.
//!
//! Scene headings' `SceneProperties` carry the index-card summary and colour (`Color`, written
//! in Final Draft's 48-bit `#RRRRGGGGBBBB` form); card tags use a ScriptCraft-only
//! `ScriptCraftTags` attribute.
//!
//! `docs/fdx-format.md` lists every element and attribute ScriptCraft reads and writes, which
//! are Final Draft's own form (with the evidence) and which are ScriptCraft-only.
//!
//! Production locks use ScriptCraft-only names: `<ScriptCraftProduction LockedPages="Yes"
//! LockedSceneNumbers="Yes"/>` under the root, and on paragraphs `ScriptCraftOmitted="Yes"`
//! (an omitted scene's heading; its text stays in the file) and `ScriptCraftLockedPages="12,12A"`
//! (the locked pages that start with that paragraph). Scene numbers use `Number`, shown per
//! `<SceneNumberOptions ShowNumbersOnLeft ShowNumbersOnRight>`; (MORE)/(CONT'D) follow
//! `<MoresAndContinueds><DialogueBreaks>`.
//! Script notes are `<ScriptNote ID Author Color Name Type>` placed among the paragraph's text
//! runs where the note starts (Final Draft's form), holding the note's text as paragraphs, plus a
//! ScriptCraft `Range="start,length"` (characters) for the exact range; a note without one
//! covers the rest of its paragraph. Production tags and alternate lines have no FDX form we
//! know of and use ScriptCraft-only elements: `<ScriptCraftTag Range="start,length"
//! Category="Props"/>` and `<ScriptCraftAlternate><Text>…</Text></ScriptCraftAlternate>`.
//! Read-aloud voices: Final Draft's `<Cast>`/`<Actors>` (see `voices.rs`). The title page: see
//! `title.rs`.
//!
//! Acts, sequences and sections are Final Draft's outline paragraphs (`Type="Outline 1"`… with
//! their `Summary`), see the `outline` module.
//!
//! `<ElementSettings Type="…">` carry each element's indents and spacing (`ParagraphSpec`),
//! capitals (`FontSpec Style="AllCaps"`) and Enter target (`Behavior ReturnKey`). ScriptCraft's
//! Tab targets have no FDX equivalent and are not written.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod beats;
mod outline;
mod page;
mod revision;
mod title;
mod track;
mod voices;

use quick_xml::Reader;
use quick_xml::events::Event;
use scriptcraft_doc::{Align, Alternate, CardColor, Element, ElementFormats, MAX_PARA_BYTES, MAX_PARAS, Note, Para, Script, Style, Tagged};

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
    /// A header (`false`) or footer (`true`) paragraph.
    HeaderFooter(bool),
    /// The text of a script note.
    Note,
    /// A line of a beat-board card (`ListItem Type="Beat"`).
    Beat,
    /// Read and dropped (the title page's own header and footer).
    Skip,
}

/// A note read so far: character range, colour, author, text.
struct NoteIn {
    start: usize,
    len: usize,
    color: Option<CardColor>,
    author: String,
    text: String,
}

struct Open {
    kind: Kind,
    ty: Option<String>,
    number: Option<String>,
    align: String,
    text: String,
    runs: Vec<(String, Style)>,
    style: Style,
    summary: String,
    color: Option<scriptcraft_doc::CardColor>,
    tags: Vec<String>,
    omitted: bool,
    page_lock: Vec<String>,
    wrapper: bool,
    in_text: bool,
    notes: Vec<NoteIn>,
    note: Option<NoteIn>,
    /// Production tags: character range and category.
    tagged: Vec<(usize, usize, String)>,
    /// The alternate being read (its text runs), and those read.
    alt: Option<Vec<(String, Style)>>,
    alts: Vec<Alternate>,
    /// The current `<Text>`'s `RevisionID`.
    rev: Option<u32>,
    /// Revised byte ranges (levels are raw revision IDs until remapped).
    revs: Vec<scriptcraft_doc::RevMark>,
    /// `Bookmark`.
    bookmark: Option<String>,
    /// Linked beats (`ScriptCraftBeats`).
    beats: Vec<u32>,
    /// Tracked changes (character offsets until the paragraph is built).
    changes: Vec<scriptcraft_doc::Change>,
    /// The current `<Text>` is `AllCaps` (title page lines are uppercased).
    caps: bool,
    /// A title page line's `ScriptCraftField`.
    field: Option<String>,
    /// A header or footer line's tab stop types, in order.
    tabs: Vec<String>,
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
    let mut title_read = title::Read::default();
    // Characters seen in the current DualDialogue (the second and later are `dual`).
    let mut dual_chars: Option<usize> = None;
    let mut root_seen = false;
    let mut formats = ElementFormats::default();
    let mut cur_fmt: Option<(Element, scriptcraft_doc::ElementFormat)> = None;
    // (locked pages, locked scene numbers, show scene numbers — older files) when the file says.
    let mut production: Option<(bool, bool, Option<bool>)> = None;
    // `SceneNumberOptions`: numbers shown in either margin.
    let mut scene_number_options: Option<bool> = None;
    // `DialogueBreaks`: (MORE)/(CONT'D) at page breaks, automatic (CONT'D).
    let mut dialogue_breaks: Option<(Option<bool>, Option<bool>)> = None;
    let mut scene_letters = false;
    let mut track_on = false;
    let mut page_read = page::Read::default();
    let mut rev_read = revision::Read::default();
    let mut outline_read = outline::Read::default();
    let mut beat_read = beats::Read::default();
    let mut voices_read = voices::Read::default();
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
                let in_title = stack.iter().any(|n| n == "TitlePage");
                match name.as_str() {
                    "ElementSettings" if !in_title => {
                        cur_fmt = attr("Type").and_then(|t| Element::parse(&t)).map(|e| (e, formats.get(e)));
                    }
                    "ParagraphSpec" if stack.iter().any(|n| n == "ElementSettings") => {
                        if let Some((_, f)) = cur_fmt.as_mut() {
                            let num = |k: &str| attr(k).and_then(|v| v.trim().parse::<f32>().ok()).filter(|x| x.is_finite());
                            if let Some(x) = num("LeftIndent") {
                                f.left = x;
                            }
                            if let Some(x) = num("RightIndent") {
                                f.right = x;
                            }
                            if let Some(x) = num("SpaceBefore") {
                                f.before = (x / 12.0).round().clamp(0.0, 6.0) as u8;
                            }
                            if let Some(a) = attr("Alignment").and_then(|a| Align::parse(&a)) {
                                f.align = a;
                            }
                            if let Some(x) = num("Spacing") {
                                f.spacing = x.round().clamp(1.0, 3.0) as u8;
                            }
                            if let Some(v) = attr("StartsNewPage") {
                                f.new_page = v.trim().eq_ignore_ascii_case("yes");
                            }
                        }
                    }
                    "FontSpec" if stack.iter().any(|n| n == "ElementSettings") => {
                        if let Some((_, f)) = cur_fmt.as_mut() {
                            f.uppercase = attr("Style").is_some_and(|v| v.split('+').any(|x| x.trim() == "AllCaps"));
                        }
                    }
                    "Behavior" if stack.iter().any(|n| n == "ElementSettings") => {
                        if let Some((_, f)) = cur_fmt.as_mut()
                            && let Some(e) = attr("ReturnKey").and_then(|r| Element::parse(&r))
                        {
                            f.enter = e;
                        }
                    }
                    "Paragraph" => {
                        let kind = if in_title && stack.iter().any(|n| n == "HeaderAndFooter") {
                            Kind::Skip
                        } else if stack.iter().any(|n| n == "ListItem") {
                            Kind::Beat
                        } else if stack.iter().any(|n| n == "ScriptNote") {
                            Kind::Note
                        } else if stack.iter().any(|n| n == "HeaderAndFooter") {
                            Kind::HeaderFooter(stack.iter().any(|n| n == "Footer"))
                        } else if stack.iter().any(|n| n == "Summary") {
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
                            runs: Vec::new(),
                            style: Style::PLAIN,
                            summary: String::new(),
                            // Outline paragraphs' colour (scene headings take theirs from SceneProperties).
                            color: attr("ScriptCraftColor").and_then(|c| parse_color(&c)),
                            tags: Vec::new(),
                            omitted: attr("ScriptCraftOmitted").is_some_and(|v| v.trim().eq_ignore_ascii_case("yes")),
                            page_lock: attr("ScriptCraftLockedPages").map(|v| locked_labels(&v)).unwrap_or_default(),
                            wrapper: false,
                            in_text: false,
                            notes: Vec::new(),
                            note: None,
                            tagged: Vec::new(),
                            alt: None,
                            alts: Vec::new(),
                            rev: None,
                            revs: Vec::new(),
                            bookmark: attr("Bookmark"),
                            beats: attr("ScriptCraftBeats").map(|v| beat_read.links(&v)).unwrap_or_default(),
                            changes: Vec::new(),
                            caps: false,
                            field: attr("ScriptCraftField"),
                            tabs: Vec::new(),
                        };
                        if empty {
                            if let Kind::HeaderFooter(footer) = o.kind {
                                page_read.add(footer, &o.align, &o.text, &o.tabs);
                            } else if o.kind == Kind::Beat {
                                beat_read.line(&o.text);
                            } else if !outline_read.take(&o, paras.len()) {
                                finish(o, &mut open, &mut paras, &mut title_read, &mut dual_chars);
                            }
                        } else {
                            open.push(o);
                        }
                    }
                    "ListItem" => {
                        beat_read.list_item(attr("Id"), attr("Type"), attr("Title"), attr("Color"));
                        if empty {
                            beat_read.end_list_item();
                        }
                    }
                    "DisplayBoard" => beat_read.board(attr("Type").as_deref()),
                    "Item" if stack.iter().any(|n| n == "DisplayBoard") => {
                        beat_read.item(attr("Id"), attr("Left"), attr("Top"), attr("Width"), attr("Height"));
                    }
                    "ScriptCraftFormat" if !in_title => {
                        scene_letters = attr("SceneLetters").is_some_and(|v| v.trim().eq_ignore_ascii_case("yes"));
                    }
                    "ScriptCraftProduction" if !in_title => {
                        let yes = |k: &str| attr(k).is_some_and(|v| v.trim().eq_ignore_ascii_case("yes"));
                        production = Some((yes("LockedPages"), yes("LockedSceneNumbers"), page::yes(attr("ShowSceneNumbers"))));
                    }
                    "PageLayout" if !in_title => {
                        if let Some(p) = attr("ScriptCraftPaper").and_then(|p| scriptcraft_doc::Paper::parse(&p)) {
                            page_read.paper = Some(p);
                        }
                    }
                    "PageSize" if stack.iter().any(|n| n == "PageLayout") => {
                        if let Some(p) = page::paper_of(attr("Width"), attr("Height")) {
                            page_read.paper = Some(p);
                        }
                    }
                    "Tabstop" => {
                        if let (Some(o), Some(t)) = (open.last_mut().filter(|o| matches!(o.kind, Kind::HeaderFooter(_))), attr("Type"))
                            && o.tabs.len() < 16
                        {
                            o.tabs.push(t);
                        }
                    }
                    "SceneNumberOptions" if !in_title => {
                        let yes = |k: &str| attr(k).is_some_and(|v| v.trim().eq_ignore_ascii_case("yes"));
                        scene_number_options = Some(yes("ShowNumbersOnLeft") || yes("ShowNumbersOnRight"));
                    }
                    "DialogueBreaks" if !in_title => {
                        dialogue_breaks =
                            Some((page::yes(attr("BottomOfPage")).or(page::yes(attr("TopOfNext"))), page::yes(attr("AutomaticCharacterContinueds"))));
                    }
                    "HeaderAndFooter" if !in_title => {
                        page_read.seen = true;
                        page_read.header_visible = page::yes(attr("HeaderVisible"));
                        page_read.footer_visible = page::yes(attr("FooterVisible"));
                        page_read.header_first = page::yes(attr("HeaderFirstPage"));
                        page_read.footer_first = page::yes(attr("FooterFirstPage"));
                        page_read.title_page = page::yes(attr("ScriptCraftTitlePage"));
                    }
                    "DynamicLabel" => {
                        if let Some(o) = open.last_mut().filter(|o| matches!(o.kind, Kind::HeaderFooter(_)))
                            && let Some(f) = attr("Type").and_then(|t| page::dynamic_label(&t))
                        {
                            push_text(o, &f);
                        }
                    }
                    "Revisions" if !in_title => {
                        rev_read.seen = true;
                        rev_read.active_id = attr("ActiveSet").and_then(|v| v.trim().parse().ok());
                        rev_read.tracking = attr("RevisionMode").map(|v| revision::mode(&v));
                        rev_read.show_marks = page::yes(attr("ScriptCraftShowMarks"));
                        rev_read.color_pages = page::yes(attr("ShowPageColor")).or(page::yes(attr("ScriptCraftColorPages")));
                    }
                    "ScriptCraftVoices" if !in_title => voices_read.narrator(attr("Narrator")),
                    "Voice" if stack.iter().any(|n| n == "ScriptCraftVoices") => voices_read.add(attr("Character"), attr("Name")),
                    "Narrator" if stack.iter().any(|n| n == "Cast") => voices_read.cast_narrator(attr("Actor")),
                    "Member" if stack.iter().any(|n| n == "Cast") => voices_read.member(attr("Actor"), attr("Character")),
                    "Actor" if stack.iter().any(|n| n == "Actors") => voices_read.actor(attr("Name"), attr("MacVoice")),
                    "Revision" if stack.iter().any(|n| n == "Revisions") => {
                        rev_read.add(revision::SetIn {
                            id: attr("ID"),
                            name: attr("Name"),
                            color: attr("Color"),
                            page_color: attr("PageColor"),
                            mark: attr("Mark"),
                            sc_name: attr("ScriptCraftName"),
                            sc_date: attr("ScriptCraftDate"),
                        });
                    }
                    "SceneProperties" => {
                        let color = attr("Color").and_then(|c| parse_color(&c));
                        let tags = attr("ScriptCraftTags").map(|t| scriptcraft_doc::clean_tags(t.split(','))).unwrap_or_default();
                        if let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body) {
                            o.color = color;
                            o.tags = tags;
                        }
                    }
                    "ScriptNote" => {
                        // Final Draft anchors a note at its place among the text runs and gives
                        // no range: it covers the rest of the paragraph. ScriptCraft adds `Range`.
                        let here = open.last().filter(|o| o.kind == Kind::Body).map_or(0, |o| o.text.chars().count());
                        let (start, len) = attr("Range").and_then(|r| parse_range(&r)).unwrap_or((here, usize::MAX));
                        // Final Draft's default marker colour is black: our default note colour.
                        let color = attr("Color")
                            .or_else(|| attr("MarkerColor"))
                            .filter(|c| c.trim_start_matches('#').chars().any(|x| x != '0'))
                            .and_then(|c| parse_color(&c));
                        let author = attr("Author").or_else(|| attr("Writer")).unwrap_or_default();
                        if let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body) {
                            let n = NoteIn { start, len, color, author, text: String::new() };
                            if empty {
                                o.notes.push(n);
                            } else {
                                o.note = Some(n);
                            }
                        }
                    }
                    "ScriptCraftChange" => {
                        if let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body)
                            && let Some(c) = track::read(&attr)
                            && o.changes.len() < scriptcraft_doc::track::MAX_CHANGES
                        {
                            o.changes.push(c);
                        }
                    }
                    "ScriptCraftTrackChanges" if !in_title => {
                        track_on = attr("On").is_some_and(|v| v.trim().eq_ignore_ascii_case("yes"));
                    }
                    "ScriptCraftTag" => {
                        if let (Some((start, len)), Some(cat)) = (attr("Range").and_then(|r| parse_range(&r)), attr("Category"))
                            && let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body)
                        {
                            o.tagged.push((start, len, cat));
                        }
                    }
                    "ScriptCraftAlternate" if !empty => {
                        if let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body) {
                            o.alt = Some(Vec::new());
                        }
                    }
                    "Text" => {
                        let st = attr("Style").map(|v| parse_style(&v)).unwrap_or_default();
                        let caps = attr("Style").is_some_and(|v| v.split('+').any(|x| x.trim() == "AllCaps"));
                        let rev = attr("RevisionID").and_then(|v| v.trim().parse::<u32>().ok());
                        if let Some(o) = open.last_mut() {
                            o.in_text = !empty;
                            o.style = st;
                            o.rev = rev;
                            o.caps = caps;
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
                    "ListItem" => beat_read.end_list_item(),
                    "DisplayBoard" => beat_read.board(None),
                    "ScriptNote" => {
                        if let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body)
                            && let Some(n) = o.note.take()
                        {
                            o.notes.push(n);
                        }
                    }
                    "ScriptCraftAlternate" => {
                        if let Some(o) = open.last_mut().filter(|o| o.kind == Kind::Body)
                            && let Some(runs) = o.alt.take()
                        {
                            o.alts.push(scriptcraft_doc::alternate_from_runs(&runs));
                        }
                    }
                    "ElementSettings" => {
                        if let Some((e, f)) = cur_fmt.take() {
                            formats.set(e, f);
                        }
                    }
                    "Paragraph" => {
                        if let Some(o) = open.pop() {
                            if let Kind::HeaderFooter(footer) = o.kind {
                                page_read.add(footer, &o.align, &o.text, &o.tabs);
                            } else if o.kind == Kind::Beat {
                                beat_read.line(&o.text);
                            } else if !outline_read.take(&o, paras.len()) {
                                finish(o, &mut open, &mut paras, &mut title_read, &mut dual_chars);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                if let Some(o) = open.last_mut().filter(|o| o.in_text) {
                    push_text(o, &t.decode().map_err(|e| FdxError::Xml(e.to_string()))?);
                }
            }
            Event::CData(t) => {
                if let Some(o) = open.last_mut().filter(|o| o.in_text) {
                    push_text(o, &t.decode().map_err(|e| FdxError::Xml(e.to_string()))?);
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
                        push_text(o, &c);
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
    rev_read.remap(&mut paras);
    let tail = outline_read.apply(&mut paras);
    // Settings first: normalizing drops page locks unless pages are locked, and clamps revision
    // marks to the sets read.
    let mut s = Script { paras, structure_tail: tail, ..Script::default() };
    s.settings.elements = formats;
    s.settings.scene_letters = scene_letters;
    s.settings.track.on = track_on;
    // Final Draft shows a scene's number whenever it has one, in the margins its options say.
    if let Some(show) = scene_number_options {
        s.settings.show_scene_numbers = show && s.paras.iter().any(|p| p.scene_number.is_some());
    }
    if let Some((pages, numbers, show)) = production {
        (s.settings.pages_locked, s.settings.scene_numbers_locked) = (pages, numbers);
        if let Some(show) = show {
            s.settings.show_scene_numbers = show;
        }
    }
    if let Some((more, auto)) = dialogue_breaks {
        if let Some(b) = more {
            s.settings.more_contd = b;
        }
        if let Some(b) = auto {
            s.settings.auto_contd = b;
        }
    }
    page_read.apply(&mut s.settings.page);
    rev_read.apply(&mut s);
    beat_read.apply(&mut s);
    voices_read.apply(&mut s);
    s.normalize().map_err(|_| FdxError::TooLarge)?;
    s.title = title_read.apply();
    Ok(s)
}

fn push_text(o: &mut Open, s: &str) {
    let upper;
    let s = if o.caps && o.kind == Kind::Title {
        upper = s.to_uppercase();
        upper.as_str()
    } else {
        s
    };
    if let Some(runs) = o.alt.as_mut() {
        if runs.iter().map(|(t, _)| t.len()).sum::<usize>() + s.len() <= MAX_PARA_BYTES {
            match runs.last_mut() {
                Some((t, st)) if *st == o.style => t.push_str(s),
                _ => runs.push((s.to_string(), o.style)),
            }
        }
        return;
    }
    if o.text.len() + s.len() > MAX_PARA_BYTES {
        return;
    }
    let at = o.text.len();
    o.text.push_str(s);
    revision::push_run(&mut o.revs, at, o.text.len(), o.rev);
    match o.runs.last_mut() {
        Some((t, st)) if *st == o.style => t.push_str(s),
        _ => o.runs.push((s.to_string(), o.style)),
    }
}

/// A scene colour: `#RRGGBB` or Final Draft's 48-bit `#RRRRGGGGBBBB`. White means none.
fn parse_color(c: &str) -> Option<scriptcraft_doc::CardColor> {
    let h = c.trim().trim_start_matches('#');
    let step = match h.len() {
        6 => 2,
        12 => 4,
        _ => return None,
    };
    let ch = |i: usize| h.get(i * step..i * step + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    let rgb = [ch(0)?, ch(1)?, ch(2)?];
    if rgb.iter().all(|v| *v >= 0xf0) {
        return None;
    }
    Some(scriptcraft_doc::CardColor::nearest(rgb))
}

/// `12,12A,13` → locked page labels (at most 1000, each short).
fn locked_labels(v: &str) -> Vec<String> {
    v.split(',')
        .map(|l| l.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(12).collect::<String>())
        .filter(|l| !l.is_empty())
        .take(1000)
        .collect()
}

/// `"start,length"` (characters).
fn parse_range(r: &str) -> Option<(usize, usize)> {
    let (a, b) = r.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// A 48-bit colour attribute.
fn color_attr(c: CardColor) -> String {
    let [r, g, b] = c.rgb();
    format!("#{r:02X}{r:02X}{g:02X}{g:02X}{b:02X}{b:02X}")
}

/// `Bold+Italic+Underline` (any order, other values ignored).
fn parse_style(v: &str) -> Style {
    let mut st = Style::PLAIN;
    for part in v.split('+') {
        match part.trim() {
            "Bold" => st.bold = true,
            "Italic" => st.italic = true,
            "Underline" => st.underline = true,
            _ => {}
        }
    }
    st
}

fn style_attr(st: Style) -> String {
    let parts: Vec<&str> =
        [(st.bold, "Bold"), (st.italic, "Italic"), (st.underline, "Underline")].iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect();
    if parts.is_empty() { String::new() } else { format!(" Style=\"{}\"", parts.join("+")) }
}

fn finish(o: Open, open: &mut [Open], paras: &mut Vec<Para>, title_read: &mut title::Read, dual_chars: &mut Option<usize>) {
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
        Kind::Title => {
            let runs = if o.runs.is_empty() { vec![(o.text, Style::PLAIN)] } else { o.runs };
            title_read.add(&o.align, &runs, o.field);
        }
        Kind::Note => {
            if let Some(n) = open.iter_mut().rev().find(|p| p.kind == Kind::Body).and_then(|p| p.note.as_mut()) {
                if !n.text.is_empty() {
                    n.text.push('\n');
                }
                n.text.push_str(o.text.trim());
            }
        }
        Kind::HeaderFooter(_) | Kind::Beat | Kind::Skip => {}
        Kind::Body => {
            if o.wrapper && o.text.trim().is_empty() {
                return;
            }
            let element = o.ty.as_deref().and_then(Element::parse).unwrap_or(match o.ty.as_deref() {
                Some("Lyrics") | Some("Cast List") => Element::Action,
                _ => Element::General,
            });
            let mut p = if o.runs.iter().any(|(_, st)| !st.is_plain()) { Para::styled(element, &o.runs) } else { Para::new(element, o.text) };
            p.page_lock = o.page_lock;
            p.omitted = o.omitted && element == Element::SceneHeading;
            p.revs = o.revs;
            p.bookmark = o.bookmark;
            p.beats = o.beats;
            if element == Element::SceneHeading {
                p.scene_number = o.number.filter(|n| !n.trim().is_empty());
                p.summary = o.summary;
                p.color = o.color;
                p.tags = o.tags;
            }
            let at = |chars: usize| p.text.char_indices().nth(chars).map_or(p.text.len(), |(b, _)| b);
            let chars = p.text.chars().count();
            p.notes = o
                .notes
                .iter()
                .filter(|n| !n.text.trim().is_empty())
                .map(|n| {
                    // A Final Draft note (no range) after all the text covers the whole paragraph.
                    let start = if n.len == usize::MAX && n.start >= chars { 0 } else { n.start };
                    Note::new(at(start), at(start.saturating_add(n.len)), &n.text, n.color, &n.author)
                })
                .collect();
            p.tagged = o.tagged.iter().map(|(s, l, c)| Tagged { start: at(*s), end: at(s.saturating_add(*l)), category: c.clone() }).collect();
            p.alternates = o.alts;
            p.changes = track::to_bytes(&p, o.changes);
            p.fix_marks();
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
    para_xml_with(out, indent, p, None);
}

/// A paragraph, with the heading's linked beats' keys (`ScriptCraftBeats`) when it has any.
fn para_xml_with(out: &mut String, indent: &str, p: &Para, beat_links: Option<String>) {
    out.push_str(&format!("{indent}<Paragraph Type=\"{}\"", p.element.name()));
    if let Some(b) = &p.bookmark {
        out.push_str(&format!(" Bookmark=\"{}\"", esc(b)));
    }
    if let Some(l) = beat_links {
        out.push_str(&format!(" ScriptCraftBeats=\"{}\"", esc(&l)));
    }
    if let Some(n) = p.scene_number.as_deref().filter(|n| !n.trim().is_empty()) {
        out.push_str(&format!(" Number=\"{}\"", esc(n.trim())));
    }
    if p.omitted {
        out.push_str(" ScriptCraftOmitted=\"Yes\"");
    }
    if !p.page_lock.is_empty() {
        out.push_str(&format!(" ScriptCraftLockedPages=\"{}\"", esc(&p.page_lock.join(","))));
    }
    out.push_str(">\n");
    if p.element == Element::SceneHeading && (!p.summary.trim().is_empty() || p.color.is_some() || !p.tags.is_empty()) {
        let mut attrs = String::new();
        if let Some(c) = p.color {
            attrs.push_str(&format!(" Color=\"{}\"", color_attr(c)));
        }
        if !p.tags.is_empty() {
            attrs.push_str(&format!(" ScriptCraftTags=\"{}\"", esc(&p.tags.join(", "))));
        }
        out.push_str(&format!("{indent}  <SceneProperties{attrs} Title=\"\">\n"));
        if !p.summary.trim().is_empty() {
            out.push_str(&format!(
                "{indent}    <Summary>\n{indent}      <Paragraph Type=\"Action\">\n{indent}        <Text>{}</Text>\n{indent}      </Paragraph>\n{indent}    </Summary>\n",
                esc(p.summary.trim())
            ));
        }
        out.push_str(&format!("{indent}  </SceneProperties>\n"));
    }
    // Text runs, with each script note placed where it starts (as Final Draft writes them).
    let chars = |b: usize| p.text.get(..b).map_or(0, |t| t.chars().count());
    let mut notes: Vec<(usize, &Note)> = p.notes.iter().enumerate().collect();
    notes.sort_by_key(|(_, n)| n.start);
    let mut next_note = 0;
    let emit_notes = |out: &mut String, upto: usize, next_note: &mut usize| {
        while let Some((i, n)) = notes.get(*next_note).filter(|(_, n)| n.start <= upto) {
            out.push_str(&format!(
                "{indent}  <ScriptNote ID=\"{}\" Author=\"{}\" Color=\"{}\" Name=\"\" Type=\"\" Range=\"{},{}\">\n",
                i + 1,
                esc(&n.author),
                color_attr(n.color),
                chars(n.start),
                chars(n.end).saturating_sub(chars(n.start)),
            ));
            for line in n.text.lines() {
                out.push_str(&format!("{indent}    <Paragraph>\n{indent}      <Text>{}</Text>\n{indent}    </Paragraph>\n", esc(line)));
            }
            out.push_str(&format!("{indent}  </ScriptNote>\n"));
            *next_note += 1;
        }
    };
    let mut at = 0usize;
    for (t, st, rev) in revision::runs(p) {
        // Final Draft numbers sets from 1.
        let rev = rev.map(|r| format!(" RevisionID=\"{}\"", u32::from(r) + 1)).unwrap_or_default();
        // Split the run where notes start inside it.
        let mut cuts: Vec<usize> =
            notes.iter().map(|(_, n)| n.start).filter(|&b| b > at && b < at + t.len() && t.is_char_boundary(b - at)).map(|b| b - at).collect();
        cuts.dedup();
        let mut from = 0;
        for cut in cuts.into_iter().chain(std::iter::once(t.len())) {
            emit_notes(out, at + from, &mut next_note);
            out.push_str(&format!("{indent}  <Text{rev}{}>{}</Text>\n", style_attr(st), esc(t.get(from..cut).unwrap_or(""))));
            from = cut;
        }
        at += t.len();
    }
    emit_notes(out, usize::MAX, &mut next_note);
    for t in &p.tagged {
        out.push_str(&format!(
            "{indent}  <ScriptCraftTag Range=\"{},{}\" Category=\"{}\"/>\n",
            chars(t.start),
            chars(t.end).saturating_sub(chars(t.start)),
            esc(&t.category)
        ));
    }
    for a in &p.alternates {
        out.push_str(&format!("{indent}  <ScriptCraftAlternate>\n"));
        let alt = Para { text: a.text.clone(), spans: a.spans.clone(), ..Para::new(p.element, "") };
        for (t, st) in alt.runs() {
            out.push_str(&format!("{indent}    <Text{}>{}</Text>\n", style_attr(st), esc(t)));
        }
        out.push_str(&format!("{indent}  </ScriptCraftAlternate>\n"));
    }
    track::write_para(out, indent, p);
    out.push_str(&format!("{indent}</Paragraph>\n"));
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
        outline::write_points(&mut out, "    ", &p.structure);
        para_xml_with(&mut out, "    ", p, beats::links_attr(s, &p.beats));
        i += 1;
    }
    outline::write_points(&mut out, "    ", &s.structure_tail);
    out.push_str("  </Content>\n");
    beats::write(&mut out, s);
    let st = &s.settings;
    let yn = |b: bool| if b { "Yes" } else { "No" };
    if st.pages_locked || st.scene_numbers_locked {
        out.push_str(&format!(
            "  <ScriptCraftProduction LockedPages=\"{}\" LockedSceneNumbers=\"{}\"/>\n",
            yn(st.pages_locked),
            yn(st.scene_numbers_locked),
        ));
    }
    // Final Draft's (MORE)/(CONT'D) and scene number options. ScriptCraft has no scene
    // (CONTINUED) and prints numbers in both margins or neither.
    let more = yn(st.more_contd);
    out.push_str(&format!(
        "  <MoresAndContinueds>\n    <DialogueBreaks AutomaticCharacterContinueds=\"{}\" BottomOfPage=\"{more}\" DialogueBottom=\"(MORE)\" DialogueTop=\"(CONT&apos;D)\" TopOfNext=\"{more}\"/>\n    <SceneBreaks ContinuedNumber=\"No\" SceneBottom=\"(CONTINUED)\" SceneBottomOfPage=\"No\" SceneTop=\"CONTINUED:\" SceneTopOfNext=\"No\"/>\n  </MoresAndContinueds>\n",
        yn(st.auto_contd)
    ));
    let show = yn(st.show_scene_numbers);
    out.push_str(&format!(
        "  <SceneNumberOptions LeftLocation=\"0.75\" NumberScheme=\"1A\" RightLocation=\"7.38\" ShowNumbersOnLeft=\"{show}\" ShowNumbersOnRight=\"{show}\"/>\n"
    ));
    for e in Element::ALL {
        let f = s.settings.elements.get(e);
        let caps = if f.uppercase { " Style=\"AllCaps\"" } else { "" };
        out.push_str(&format!(
            "  <ElementSettings Type=\"{n}\">\n    <FontSpec Font=\"Courier Badi\" Size=\"12\"{caps}/>\n    <ParagraphSpec Alignment=\"{a}\" FirstIndent=\"0.00\" Leading=\"Regular\" LeftIndent=\"{l:.2}\" RightIndent=\"{r:.2}\" SpaceBefore=\"{sb}\" Spacing=\"{sp}\" StartsNewPage=\"{np}\"/>\n    <Behavior PaginateAs=\"{n}\" ReturnKey=\"{rk}\" Shortcut=\"{k}\"/>\n  </ElementSettings>\n",
            n = e.name(),
            a = f.align.name(),
            l = f.left,
            r = f.right,
            sb = u32::from(f.before) * 12,
            rk = f.enter.name(),
            k = e.index() + 1,
            sp = f.spacing,
            np = if f.new_page { "Yes" } else { "No" },
        ));
    }
    if s.structure_count() > 0 {
        outline::write_settings(&mut out);
    }
    if s.settings.scene_letters {
        out.push_str("  <ScriptCraftFormat SceneLetters=\"Yes\"/>\n");
    }
    page::write(&mut out, &s.settings.page);
    track::write_root(&mut out, s);
    revision::write(&mut out, s);
    voices::write(&mut out, s);
    title::write(&mut out, &s.title, s.settings.page.paper);
    out.push_str("</FinalDraft>\n");
    out
}

#[cfg(test)]
mod structure_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "annot_tests.rs"]
mod annot_tests;

#[cfg(test)]
#[path = "board_tests.rs"]
mod board_tests;
#[cfg(test)]
#[path = "compat_tests.rs"]
mod compat_tests;
#[cfg(test)]
#[path = "v04_tests.rs"]
mod v04_tests;
