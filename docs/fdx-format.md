# FDX format: what ScriptCraft reads and writes

`.fdx` is Final Draft's XML file format. ScriptCraft reads and writes it clean-room: from the
structure of files Final Draft saved. No Final Draft template, sample script or wording is in this
repository, and the test fixtures (`crates/fdx/src/*tests.rs`) are files we wrote ourselves in the
observed structure.

For each feature this page lists what ScriptCraft writes and how sure we are that it is Final
Draft's own form.

- **FD**: Final Draft's form, seen in files Final Draft saved.
- **FD, unconfirmed detail**: the structure is Final Draft's, but one detail is our best guess.
- **SC**: ScriptCraft-only. No Final Draft form is known. Final Draft ignores these names and
  drops them when it saves; ScriptCraft reads them back.

## Evidence

- **Public files.** On 2026-10-14, `gh search code --extension fdx FinalDraft` found 100 public
  `.fdx` files on GitHub. 51 of them were saved by Final Draft: they have the blocks only Final
  Draft writes (`SmartType`, `Macros`, `SplitState`). Their root `Version` is 1 to 6; 19 are
  `Version="6"`, the newest.
- **Structure only.** Only element names, attribute names and short attribute values were looked
  at, never the script text. The files were not kept.
- **Earlier research.** The research on 29 public files in
  [#42](https://github.com/NoJuanNobody/scriptcraft/issues/42#issuecomment-6143636042) is
  consistent with this page.
- **Not observed.** Some features don't appear in any public file Final Draft saved: tags with
  tagged text, omitted scenes, alternate lines, freshly locked pages, writer markup. These wait on
  the sample files requested in
  [#44](https://github.com/NoJuanNobody/scriptcraft/issues/44).

"n/51" below means the feature appears in n of the 51 Final Draft files.

## The script

| Feature | What ScriptCraft writes | Form | Evidence |
|---|---|---|---|
| Paragraphs | `<Content><Paragraph Type="Action"><Text>…</Text></Paragraph>` | FD | 51/51 |
| Text styles | `<Text Style="Bold+Italic+Underline">`. Reading also accepts `AllCaps` and others; `AllCaps` uppercases title page text. | FD | `Style="Bold…"` 33/51 |
| Dual dialogue | `<Paragraph><DualDialogue>…both blocks…</DualDialogue></Paragraph>` | FD | 2/51 |
| Scene numbers | `Number="12A"` on the heading | FD | 51/51 |
| Show scene numbers | `<SceneNumberOptions LeftLocation="0.75" NumberScheme="1A" RightLocation="7.38" ShowNumbersOnLeft ShowNumbersOnRight/>`. ScriptCraft prints numbers in both margins or in neither. On reading, numbers show when either side is `Yes` and a heading has a number. | FD | 50/51 |
| (MORE) / (CONT'D) | `<MoresAndContinueds><DialogueBreaks AutomaticCharacterContinueds BottomOfPage TopOfNext DialogueBottom="(MORE)" DialogueTop="(CONT'D)"/><SceneBreaks … SceneBottomOfPage="No" SceneTopOfNext="No"/>`. ScriptCraft has no scene (CONTINUED), so it writes `No` for it. | FD | 50/51 |
| Element settings | `<ElementSettings Type><FontSpec Style="AllCaps"/><ParagraphSpec Alignment LeftIndent RightIndent SpaceBefore Spacing StartsNewPage/><Behavior PaginateAs ReturnKey Shortcut/></ElementSettings>` | FD | `Spacing` and `StartsNewPage` 51/51, `StartsNewPage="Yes"` 33/51, `Spacing="2"` 1/51, `ReturnKey` 50/51 |
| Templates (double spacing, new page per scene) | `ParagraphSpec Spacing="2"` and `StartsNewPage="Yes"` | FD | as above |
| Scene letters (A, B, C…) | `<ScriptCraftFormat SceneLetters="Yes"/>` | SC | Every file has `NumberScheme="1A"`; Final Draft's letter scheme was not seen |
| Bookmarks | `Bookmark="name"` on a `<Paragraph>` | FD | 2/51 |

## Scenes, outline and boards

| Feature | What ScriptCraft writes | Form | Evidence |
|---|---|---|---|
| Card summary | `<SceneProperties><Summary><Paragraph><Text>…` | FD | 2/51 |
| Card colour | `<SceneProperties Color="#RRRRGGGGBBBB">` (48-bit) | FD | 3/51 |
| Card tags | `<SceneProperties ScriptCraftTags="a, b">` | SC | none seen |
| Acts, sequences, sections | Hidden `<Paragraph Type="Outline 1/2/3">` before the scene they open, followed by `Summary` paragraphs, plus `ElementSettings` for `Outline 1–3` | FD | `Type="Outline 1"` 20/51 |
| Structure point colour | `ScriptCraftColor` on the outline paragraph | SC | none seen |
| Beat board | `<ListItems><ListItem Id Type="Beat" Title Color><Content>…` and `<DisplayBoards><DisplayBoard Type="Beat"><Item Id Left Top Width Height/>` | FD | `ListItem Type="Beat"` 10/51, `DisplayBoard` 32/51, `Item Left=` 10/51 |
| Beat → scene link | `ScriptCraftBeats="id,id"` on the heading | SC | No link form seen. The `Item LaneId` (51 items) and `Lanes` belong to boards we don't read. |

## Notes and production

| Feature | What ScriptCraft writes | Form | Evidence |
|---|---|---|---|
| Script notes | `<ScriptNote ID Author Color Name Type>` holding `<Paragraph>`s, placed **among the paragraph's text runs where the note starts**, plus `Range="start,length"` (characters). On reading, a note without `Range` covers the rest of its paragraph, or the whole paragraph when it comes after all the text. Final Draft's default black marker colour reads as our default note colour. | FD, plus an SC `Range` | Notes inline 9/51: at the start of the paragraph in 5 files, between runs in 1 |
| Note types, unanchored notes | Not written. On reading, `<ScriptNoteDefinitions>` and `<UnanchoredScriptNotes>` are skipped, because ScriptCraft notes have no type and are always anchored. | FD (not mapped) | `ScriptNoteDefinition` 11/51, `UnanchoredScriptNotes` 18/51 |
| Revision sets | `<Revisions ActiveSet RevisionMode="Yes/No" ShowAllMarks="No" ShowAllSets="No" ShowPageColor ScriptCraftShowMarks>` holding `<Revision ID Name Color PageColor Mark FullRevision Style ScriptCraftName ScriptCraftDate/>` | FD, plus SC exact name and date | 50/51 |
| Set numbering | Set `i` is `ID="i+1"`, and text runs carry `RevisionID="i+1"`. On reading, `RevisionID="0"` is unrevised text, unless the file has a set `0` (older ScriptCraft files). | FD | IDs start at 1 in 50/50. `RevisionID="0"` is on 3,109 unrevised runs. |
| Set colours | `Color="#000000000000"` (the colour of revised text) and `PageColor` (the page tint) | FD | `PageColor` on every set |
| Set name and date | `Name="Pink Rev. (10/12/26)"`, Final Draft's whole label. On reading, `(mm/dd/yy)` is no date. | FD | Every set name has this pattern |
| Show revision marks | `ScriptCraftShowMarks` | SC | Final Draft's `ShowAllMarks` / `RevisionsShown` mean something else (which sets' marks show) |
| Deleted text | Not written or read | FD (not mapped) | Root `<DeletedText><DeletedTextLocation Offset RevisionID/>` in 3/51. `Offset` counts across the whole document in a unit we haven't confirmed; ScriptCraft marks a deletion on the character next to it. |
| Locked pages | `<ScriptCraftProduction LockedPages LockedSceneNumbers/>` and `ScriptCraftLockedPages="12,12A"` on the paragraph a locked page starts with | SC, **pending #44** | Root `<LockedPages/>` is in every file but empty in 48. One file has `<LockedPage LevelIndex LockLevel PageNumber Position/>`, but with positions past its text, so what `Position` counts is unknown. |
| Locked scene numbers | `LockedSceneNumbers` (above) | SC, **pending #44** | not seen |
| Omitted scenes | `ScriptCraftOmitted="Yes"` on the heading; its text stays | SC, **pending #44** | Only a hand-made, non-Final-Draft file had `<OmittedScene>` |
| Production tags | `<ScriptCraftTag Range Category/>` in the paragraph | SC, **pending #44** | `<TagData><TagCategories><TagCategory Id Name Number Color Style/>` in 30/51, always the standard category list. `TagDefinitions`, `Tags` and `TagNumber` on runs: 0/51. |
| Alternate lines | `<ScriptCraftAlternate><Text>…</Text></ScriptCraftAlternate>` in the paragraph | SC, **pending #44** | `<AltCollection/>` in 32/51, always empty |
| Tracked changes | `<ScriptCraftChange Kind …/>` per paragraph, `<ScriptCraftTrackChanges On/>` | SC | `<Writers/>` and `<WriterMarkup State/>` in 21/51, always empty, so Final Draft's writer markup is unknown |

## Page setup and the title page

| Feature | What ScriptCraft writes | Form | Evidence |
|---|---|---|---|
| Paper | `<PageLayout …><PageSize Height="11.00" Width="8.50"/></PageLayout>`, A4 `8.27 × 11.69`. On reading, the nearest of Letter and A4. | FD | `PageSize` 41/51; one A4 file (`8.26 × 11.69`) |
| Margins | `PageLayout TopMargin="72" BottomMargin="72" HeaderMargin="36" FooterMargin="36"` (points; fixed, not read) | FD | 48/51 |
| Header and footer | `<HeaderAndFooter HeaderVisible FooterVisible HeaderFirstPage FooterFirstPage StartingPage>`, with **one** `<Paragraph Alignment="Left">` per part: left slot, a tab, centre slot, a tab, right slot, and `<Tabstops><Tabstop Position="4.50" Type="Center"/><Tabstop Position="7.25" Type="Right"/></Tabstops>` | FD | Final Draft's default header is exactly this, with the revision label at the centre tab and `Page #.` at the right tab (42 of 51) |
| Header fields | `{page}` is `<DynamicLabel Type="Page #"/>` and `{revision}` is `Last Revised`. On reading, `Collated Revisions` becomes `{revision}` and `Date` becomes `{date}` (ours is the draft date). Other fields are SC: `ScriptCraft:title`, `pages`, `author`, `date`, `revname`, `revdate`. | FD, plus SC | `Page #` 97, `Collated Revisions` 25, `Last Revised` 22, `Date` 2, `Label` 3 (not read) |
| Header on the title page | `ScriptCraftTitlePage` on `HeaderAndFooter` | SC | Final Draft's title page has its own `<HeaderAndFooter>` inside `<TitlePage>`. ScriptCraft does not read it. |
| Title page | `<TitlePage><Content>`, one `<Paragraph Alignment FirstIndent="0.00" Leading="Regular" LeftIndent="1.00" RightIndent="7.50" SpaceBefore="0" Spacing="1" StartsNewPage="No">` per line, blank paragraphs for spacing, no `Type` | FD | 51/51 (`LeftIndent 1.00`, `RightIndent 7.50`; alignments Center, Left, Full, Right) |
| Title page fields | `ScriptCraftField="title"`, `credit`, `author`, `source`, `draftDate`, `contact`, `copyright` or `notes` on each line | SC | Final Draft has no title page fields |
| Free-form title pages | A Final Draft title page that doesn't fit the fields is kept as its lines (alignment, text, whole-line bold/italic/underline) and written back as it is. `Full` alignment reads as Left; mixed styles within a line become plain. | FD | 46 of 51 come out free-form, mostly because the title is `Underline+AllCaps` |

## Read aloud

| Feature | What ScriptCraft writes | Form | Evidence |
|---|---|---|---|
| Voices | `<Cast><Narrator Actor><Element Type="Character"/><Element Type="Dialogue"/></Narrator><Member Actor Character/></Cast>` and `<Actors><Actor MacVoice Name Pitch="Normal" Speed="Medium" WinVoice=""/></Actors>`, one actor per voice, named after it | FD, unconfirmed detail | `Cast` 48/51; the standard roster of 10 actors with `Pitch`/`Speed`; `<Narrator Actor="Man 1">` 48/51. Every public file leaves `MacVoice` empty, so whether Final Draft expects a plain voice name there is unconfirmed. On reading, an actor with no `MacVoice` is no assigned voice. |
| Older ScriptCraft voices | `<ScriptCraftVoices>` still reads; no longer written | SC | none seen |

## Still unknown

These need the four Final Draft 13 files requested in #44, or someone with Final Draft opening a
file ScriptCraft saved:

- **Production tags.** How Final Draft links tagged text to a category: `TagDefinitions`, `Tags`
  and `TagNumber` on text runs.
- **Omitted scenes.**
- **Alternate lines.** What goes inside `AltCollection`, and how the active line is chosen.
- **Locked pages and scene numbers.** What `LockedPage Position` counts, and which attribute
  marks a locked scene number such as `12A`.
- **Deleted text.** The unit of `DeletedTextLocation Offset`.
- **Writer markup.** `Writers` and `WriterMarkup`, which might hold tracked changes.
- **Voice names.** Whether `MacVoice` is a plain voice name.
- **A hand check.** That Final Draft shows ScriptCraft's revision sets, page colours, centred and
  right header slots, notes and outline paragraphs as intended (ties into #32).
