# ScriptCraft roadmap

**Stage:** v0.1 MVP — writes, paginates, imports and exports real screenplays; agent-drivable.
**Headline numbers:** 33 of 63 Final Draft 13 catalog features implemented, 1 partial
([docs/parity-checklist.md](docs/parity-checklist.md)); 73 engine commands + 19 window commands;
~8.4k lines of Rust; 57 tests.

## v0.1 (this release)

- [x] Screenplay model, undo/redo, element flow, SmartType, dual dialogue
- [x] Industry pagination with (MORE)/(CONT'D), title page, scene numbers
- [x] Fountain and FDX round trips; PDF export (krilla, embedded Courier Prime)
- [x] Navigator, index cards with summaries and reordering, reports
- [x] egui window: menu bar, element picker, page view, status bar, dark mode, zoom
- [x] CLI, MCP server, keyed control channel, headless `ui_shot`

All open work is tracked as [GitHub issues](https://github.com/NoJuanNobody/scriptcraft/issues) under milestones.

## Next (v0.2): writing comfort

- [ ] [#1](https://github.com/NoJuanNobody/scriptcraft/issues/1) Inline bold, italic and underline
- [ ] [#2](https://github.com/NoJuanNobody/scriptcraft/issues/2) Spelling check (offline)
- [ ] [#3](https://github.com/NoJuanNobody/scriptcraft/issues/3) Element settings editor (indents, spacing, capitals, Enter/Tab targets)
- [ ] [#4](https://github.com/NoJuanNobody/scriptcraft/issues/4) Decide: Tab in an empty Action → Character or Transition?
- [ ] [#5](https://github.com/NoJuanNobody/scriptcraft/issues/5) Element picker on Enter in an empty paragraph
- [ ] [#6](https://github.com/NoJuanNobody/scriptcraft/issues/6) Copy and paste that keeps element types
- [ ] [#7](https://github.com/NoJuanNobody/scriptcraft/issues/7) Drag to reorder index cards; card colours and tags
- [ ] [#8](https://github.com/NoJuanNobody/scriptcraft/issues/8) Autosave, backups and recent files
- [ ] [#9](https://github.com/NoJuanNobody/scriptcraft/issues/9) Caret blink, wheel/pinch zoom, page-width zoom and a non-paginated speed view
- [ ] [#10](https://github.com/NoJuanNobody/scriptcraft/issues/10) Verify the unsaved-changes prompt on close in the native window
- [ ] [#11](https://github.com/NoJuanNobody/scriptcraft/issues/11) Remove the control key file when the app is terminated
- [ ] [#12](https://github.com/NoJuanNobody/scriptcraft/issues/12) Web build (wasm)

## v0.3: production

- [ ] [#13](https://github.com/NoJuanNobody/scriptcraft/issues/13) Revision mode with coloured revision pages
- [ ] [#14](https://github.com/NoJuanNobody/scriptcraft/issues/14) Locked pages, A/B pages, omitted scenes and locked scene numbers
- [ ] [#15](https://github.com/NoJuanNobody/scriptcraft/issues/15) Headers and footers editor; A4 paper
- [ ] [#16](https://github.com/NoJuanNobody/scriptcraft/issues/16) Script notes
- [ ] [#17](https://github.com/NoJuanNobody/scriptcraft/issues/17) Alternate dialogue lines
- [ ] [#18](https://github.com/NoJuanNobody/scriptcraft/issues/18) Production tagging and breakdown reports
- [ ] [#19](https://github.com/NoJuanNobody/scriptcraft/issues/19) Print dialog
- [ ] [#20](https://github.com/NoJuanNobody/scriptcraft/issues/20) Split dual dialogue across a page break

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
