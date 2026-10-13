# ScriptCraft roadmap

**Stage:** v0.3 (production) complete on branch `v0.3-production`; v0.2 writing comfort merged.
**Headline numbers:** 52 of 68 Final Draft 13 catalog features implemented
([docs/parity-checklist.md](docs/parity-checklist.md)); 133 engine commands + 37 window commands;
~18.5k lines of Rust; 133 tests; native and browser builds; CI on Linux, macOS and Windows.

## v0.1 (this release)

- [x] Screenplay model, undo/redo, element flow, SmartType, dual dialogue
- [x] Industry pagination with (MORE)/(CONT'D), title page, scene numbers
- [x] Fountain and FDX round trips; PDF export (krilla, embedded Courier Prime)
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

## v0.4: structure and collaboration

- [ ] [#21](https://github.com/NoJuanNobody/scriptcraft/issues/21) Outline view with structure points
- [ ] [#22](https://github.com/NoJuanNobody/scriptcraft/issues/22) Beat board
- [ ] [#23](https://github.com/NoJuanNobody/scriptcraft/issues/23) Story map
- [ ] [#24](https://github.com/NoJuanNobody/scriptcraft/issues/24) Track changes and comparing two drafts
- [ ] [#25](https://github.com/NoJuanNobody/scriptcraft/issues/25) Real-time collaboration
- [ ] [#26](https://github.com/NoJuanNobody/scriptcraft/issues/26) Templates: TV single-camera, multi-camera, stage play
- [ ] [#27](https://github.com/NoJuanNobody/scriptcraft/issues/27) Read aloud with cast voices, and dictation (speech to script)
- [ ] [#28](https://github.com/NoJuanNobody/scriptcraft/issues/28) Bookmarks and split screen

## File-format fidelity

- [ ] [#29](https://github.com/NoJuanNobody/scriptcraft/issues/29) Keep Shot and General elements in Fountain
- [ ] [#30](https://github.com/NoJuanNobody/scriptcraft/issues/30) Keep all title page fields in FDX; support free-form title pages
- [ ] [#31](https://github.com/NoJuanNobody/scriptcraft/issues/31) Import from Word, Celtx and PDF

## Verification

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
