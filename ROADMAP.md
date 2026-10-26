# ScriptCraft roadmap

**Stage:** v0.5.0 (released); #42's last items, #44 and #32 need Final Draft 13.
**Headline numbers:** 63 of 68 Final Draft 13 catalog features implemented
([docs/parity-checklist.md](docs/parity-checklist.md)); 191 engine commands + 53 window commands;
~38.7k lines of Rust; 291 tests; English & Russian UI with Cyrillic screenplay support (Courier Badi);
native (macOS, Windows, Linux) and browser builds; CI and automated releases on GitHub Actions.

## v0.1 (this release)

- [x] Screenplay model, undo/redo, element flow, SmartType, dual dialogue
- [x] Industry pagination with (MORE)/(CONT'D), title page, scene numbers
- [x] Fountain and FDX round trips; PDF export (krilla, embedded Courier Badi)
- [x] Navigator, index cards with summaries and reordering, reports
- [x] egui window: menu bar, element picker, page view, status bar, dark mode, zoom
- [x] CLI, MCP server, keyed control channel, headless `ui_shot`

All open work is tracked as [GitHub issues](https://github.com/NoJuanNobody/scriptcraft/issues) under milestones.

## Next (v0.2): writing comfort

- [x] [#1](https://github.com/NoJuanNobody/scriptcraft/issues/1) Inline bold, italic and underline
- [x] [#2](https://github.com/NoJuanNobody/scriptcraft/issues/2) Spelling check (offline)
- [x] [#3](https://github.com/NoJuanNobody/scriptcraft/issues/3) Element settings editor (indents, spacing, capitals, Enter/Tab targets)
- [x] [#4](https://github.com/NoJuanNobody/scriptcraft/issues/4) Decide: Tab in an empty Action → Character or Transition?
- [x] [#5](https://github.com/NoJuanNobody/scriptcraft/issues/5) Element picker on Enter in an empty paragraph
- [x] [#6](https://github.com/NoJuanNobody/scriptcraft/issues/6) Copy and paste that keeps element types
- [x] [#7](https://github.com/NoJuanNobody/scriptcraft/issues/7) Drag to reorder index cards; card colours and tags
- [x] [#8](https://github.com/NoJuanNobody/scriptcraft/issues/8) Autosave, backups and recent files
- [x] [#9](https://github.com/NoJuanNobody/scriptcraft/issues/9) Caret blink, wheel/pinch zoom, page-width zoom and a non-paginated speed view
- [x] [#10](https://github.com/NoJuanNobody/scriptcraft/issues/10) Verify the unsaved-changes prompt on close in the native window
- [x] [#11](https://github.com/NoJuanNobody/scriptcraft/issues/11) Remove the control key file when the app is terminated
- [x] [#12](https://github.com/NoJuanNobody/scriptcraft/issues/12) Web build (wasm)

## v0.3: production (done)

- [x] [#13](https://github.com/NoJuanNobody/scriptcraft/issues/13) Revision mode with coloured revision pages
- [x] [#14](https://github.com/NoJuanNobody/scriptcraft/issues/14) Locked pages, A/B pages, omitted scenes and locked scene numbers
- [x] [#15](https://github.com/NoJuanNobody/scriptcraft/issues/15) Headers and footers editor; A4 paper
- [x] [#16](https://github.com/NoJuanNobody/scriptcraft/issues/16) Script notes
- [x] [#17](https://github.com/NoJuanNobody/scriptcraft/issues/17) Alternate dialogue lines
- [x] [#18](https://github.com/NoJuanNobody/scriptcraft/issues/18) Production tagging and breakdown reports
- [x] [#19](https://github.com/NoJuanNobody/scriptcraft/issues/19) Print dialog
- [x] [#20](https://github.com/NoJuanNobody/scriptcraft/issues/20) Split dual dialogue across a page break

Follow-ups found while testing v0.3 (done in v0.5):

- [x] [#41](https://github.com/NoJuanNobody/scriptcraft/issues/41) Reports show counted page numbers instead of printed labels (12A) when pages are locked
- [x] [#43](https://github.com/NoJuanNobody/scriptcraft/issues/43) Print directly to a printer (printer list, copies), browser print, temp file cleanup
- [x] [#45](https://github.com/NoJuanNobody/scriptcraft/issues/45) Dual dialogue: a long cue plus (CONT'D) runs past the right column and margin after a page split

## v0.4: structure and collaboration (done)

- [x] [#21](https://github.com/NoJuanNobody/scriptcraft/issues/21) Outline view with structure points
- [x] [#22](https://github.com/NoJuanNobody/scriptcraft/issues/22) Beat board
- [x] [#23](https://github.com/NoJuanNobody/scriptcraft/issues/23) Story map
- [x] [#24](https://github.com/NoJuanNobody/scriptcraft/issues/24) Track changes and comparing two drafts
- [x] [#25](https://github.com/NoJuanNobody/scriptcraft/issues/25) Real-time collaboration
- [x] [#26](https://github.com/NoJuanNobody/scriptcraft/issues/26) Templates: TV single-camera, multi-camera, stage play
- [x] [#27](https://github.com/NoJuanNobody/scriptcraft/issues/27) Read aloud with cast voices, and dictation (speech to script)
- [x] [#28](https://github.com/NoJuanNobody/scriptcraft/issues/28) Bookmarks and split screen

## v0.5: fidelity and polish

File-format fidelity:

- [x] [#29](https://github.com/NoJuanNobody/scriptcraft/issues/29) Keep Shot and General elements in Fountain
- [x] [#30](https://github.com/NoJuanNobody/scriptcraft/issues/30) Keep all title page fields in FDX; support free-form title pages
- [x] [#31](https://github.com/NoJuanNobody/scriptcraft/issues/31) Import from Word, Celtx and PDF
- [ ] [#42](https://github.com/NoJuanNobody/scriptcraft/issues/42) Final Draft compatibility: map revisions, locks, omitted scenes, tags, alternates and page setup to Final Draft's own FDX — Final Draft's form written wherever public files confirm it ([docs/fdx-format.md](docs/fdx-format.md)); tags, omitted scenes, alternates, locks and tracked changes wait on #44
- [ ] [#44](https://github.com/NoJuanNobody/scriptcraft/issues/44) Get sample .fdx files from Final Draft 13 (tags, omitted scene, alternate line, locked pages)

Verification:

- [ ] [#32](https://github.com/NoJuanNobody/scriptcraft/issues/32) Check the typing flow against real Final Draft 13

## Progress log

- 2026-10-12 — v0.1: model, Fountain/FDX/PDF, pagination, engine with 73 commands, MCP, CLI,
  egui window with page view, navigator, index cards, reports, dark mode; README screenshots.
- 2026-10-12 — published at github.com/NoJuanNobody/scriptcraft; 32 issues filed for the known gaps.
- 2026-10-12 — settings through craft-settings (storytold/craft-libs#22), the Craft suite's
  shared config.toml store and folder rules; recovery, backups and dictionary in the data folder (#35).
- 2026-10-12 — v0.2 writing comfort (#1–#12): inline styles, spelling, element settings, element
  list on Enter, structured copy/paste, card colours/tags and drag reorder, autosave/recovery/
  backups/recent files, caret blink, zoom and speed view, verified quit prompt, signal-safe
  control key, browser build.
- 2026-10-13 — CI on GitHub Actions for Linux, macOS and Windows, plus a weekly run against the
  newest Rust and dependencies (#34).
- 2026-10-13 — v0.3 production (#13–#20): revision mode with coloured sets, margin asterisks and
  revised-page headers; locked pages with A/B pages, omitted scenes and locked scene numbers;
  header/footer editor and A4 paper; script notes; alternate dialogue lines; production tagging
  with a breakdown report and CSV; File › Print; dual dialogue split across page breaks.
- 2026-10-14 — v0.4 structure and collaboration (#21–#28): acts, sequences and sections with an
  Outline view and a story map; beat board with beats linked to scenes; bookmarks and split screen;
  track changes per author with accept/reject, and Compare Drafts (also `scriptcraft-cli compare`);
  LAN real-time collaboration with presence (docs/collaboration.md); TV single-camera,
  multi-camera and stage-play templates; read aloud with a voice per character and dictation.
- 2026-10-14 — v0.5 fidelity and polish (#29–#31, #41–#43, #45): Shot and General kept in
  Fountain; free-form and complete FDX title pages; Final Draft's own FDX forms for revisions,
  page setup, notes, voices and options (docs/fdx-format.md), which also fixed real Final Draft
  files opening fully revised; import from Word, Celtx and PDF; reports with printed page labels
  on locked pages; continued dual-dialogue cues kept in their column; printing to a printer.
- 2026-10-26 — v0.5.0 release (#48, #49, #50, #51, #53, #54): Russian localization and Cyrillic
  screenplay support with embedded Courier Badi v1.010; interactive architecture & workflow
  explainer and live browser app on GitHub Pages; multi-platform release builds for Windows (x64),
  macOS (Universal `.app` + CLI), Linux (`x86_64` tarball and `.deb`), and Web (WASM).
