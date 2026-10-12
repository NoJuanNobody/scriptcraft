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

## Next (v0.2): writing comfort

- [ ] Inline bold / italic / underline (model, FDX `Style`, Fountain emphasis, PDF faces)
- [ ] Spelling check (offline)
- [ ] Element settings: per-element indents, spacing, capitals, Enter/Tab targets (FD's Elements dialog)
- [ ] Clipboard with element types (paste screenplay structure between scripts)
- [ ] Drag-to-reorder index cards; colours and tags on cards
- [ ] Autosave and backups; recent files
- [ ] Mouse wheel zoom, page-width zoom, speed (non-paginated) view
- [ ] Web build (wasm) of the editor

## v0.3: production

- [ ] Revision mode: revision sets and colours, asterisks in the margin, coloured pages
- [ ] Locked pages (A/B pages) and omitted scenes; locked scene numbers (12A)
- [ ] Headers and footers editor; A4
- [ ] ScriptNotes; alternate dialogue (Alts)
- [ ] Production tagging and breakdown reports

## v0.4: structure and collaboration

- [ ] Outline Editor with structure points; Beat Board
- [ ] Track changes / compare two drafts
- [ ] Real-time collaboration
- [ ] Templates: TV (single and multi-camera), stage play, graphic novel
- [ ] Read aloud with cast voices; speech-to-script

## Known gaps in v0.1

- Final Draft was not available to observe; element flow follows documented conventions (see
  docs/typing-parity.md for the rows that are our own choice).
- Inline styles are not modelled: FDX `<Text Style>` runs are flattened; Fountain emphasis markers stay as literal text.
- Fountain has no Shot/General element: those export as Action.
- FDX title pages are free-form in Final Draft; ScriptCraft maps them onto fixed fields (title,
  credit, author, source, contact, draft date); copyright and notes export into the contact block.
- Dual dialogue moves as a whole and is not split across pages.
- No print dialog: export a PDF and print it. No file dialogs in the web build.

## Progress log

- 2026-10-12 — v0.1: model, Fountain/FDX/PDF, pagination, engine with 73 commands, MCP, CLI,
  egui window with page view, navigator, index cards, reports, dark mode; README screenshots.
