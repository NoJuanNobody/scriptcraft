<h1 align="center">ScriptCraft</h1>

<p align="center">
  <b>Screenwriting; an open-source, clean-room take on the Final Draft workflow, built in pure Rust.</b>
</p>

<p align="center">
  A fast, open-source screenwriting app that works the way screenwriters already work: elements that
  flow with Enter and Tab, SmartType, industry-standard pagination with (MORE) and (CONT'D), a scene
  navigator, index cards and reports. It reads and writes Final Draft (.fdx) and Fountain files,
  exports PDF, and every action can be driven by agents.<br>
  <i>A Craft app in the style of the <a href="https://getartcraft.com/apps">ArtCraft Crafting Apps</a>.</i>
</p>

<p align="center">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-2b47b5?style=flat-square&logo=rust&logoColor=white">
  <img alt="Runs on macOS, Windows and Linux" src="https://img.shields.io/badge/runs%20on-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-3b5bdb?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-2b47b5?style=flat-square">
  <img alt="Agent-drivable over MCP" src="https://img.shields.io/badge/agents-MCP%20%C2%B7%20CLI-3b5bdb?style=flat-square">
  <img alt="Version 0.1" src="https://img.shields.io/badge/version-0.1%20(MVP)-6f8cff?style=flat-square">
</p>

<br>

<p align="center">
  <img src="docs/images/hero.png" alt="ScriptCraft's page view: a Letter page in Courier with a scene of dialogue, character cues centred, a parenthetical, and (MORE) at the bottom of the page. The Navigator on the left lists four scenes with their summaries and page numbers; the toolbar shows the element picker set to Dialogue; the status bar reads Dialogue, Page 2 of 4, Scene 2 of 4, 699 words." width="100%">
  <br><sub><b>Night Signal</b>, ScriptCraft's built-in original sample: page view, element picker, navigator with scene summaries.</sub>
</p>

<p align="center">
  <a href="#a-tour">A tour</a> ·
  <a href="#what-works-today">What works today</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#for-agents-cli-mcp-and-the-control-channel">For agents</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#roadmap">Roadmap</a> ·
  <a href="#license-and-credits">License and credits</a>
</p>

## A tour

Every screenshot is ScriptCraft itself, rendered offscreen by its own UI harness
(`cargo run -p scriptcraft-ui-egui --example ui_shot`).

<table>
<tr>
<td width="50%" valign="top"><img src="docs/images/cards.png" alt="Index cards view: four cards in a grid, each with a coloured band, scene number, bold Courier scene heading, page and length in eighths, an editable summary and the cast along the bottom" width="100%"><p align="center"><sub><b>Index cards.</b> One card per scene with an editable summary, page, length in eighths and cast; reorder scenes from the cards.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/dark-smarttype.png" alt="Dark mode page view: a dual-dialogue exchange side by side, then a new character cue being typed with the SmartType popup offering TEO" width="100%"><p align="center"><sub><b>Dark mode and SmartType.</b> Character names, extensions, scene intros, locations, times and transitions complete as you type. Dual dialogue sits side by side.</sub></p></td>
</tr>
<tr>
<td colspan="2"><img src="docs/images/reports.png" alt="Reports view with three tables: characters with speeches, words, scenes and first page; scenes with heading, page, length and cast; locations with scene counts and INT/EXT" width="100%"><p align="center"><sub><b>Reports.</b> Character, scene and location reports, computed from the same pagination as the PDF.</sub></p></td>
</tr>
</table>

## Why ScriptCraft

- **Familiar.** Final Draft's element flow: Enter after a scene heading gives Action, Tab after
  Action gives a Character, Enter after a Character gives Dialogue, Tab in Dialogue gives a
  Parenthetical (with the parentheses typed for you). Headings, cues and transitions capitalise
  themselves. ⌘1–⌘8 pick an element.
- **Industry pages.** US Letter, 12-point Courier at ten characters and six lines per inch, standard
  indents, page numbers top-right, (MORE)/(CONT'D) when dialogue breaks across pages, no orphaned
  scene headings — the same layout on screen, in reports and in the PDF.
- **Your files.** Opens and saves Final Draft XML (.fdx) and Fountain (.fountain); exports PDF with
  embedded, selectable Courier Prime.
- **Built for agents.** Every action is a command with an id. The same 73 engine commands (plus 19
  window commands) drive the menus, toolbar, keyboard shortcuts, a command-line tool, a JSON
  control channel and an MCP server.
- **Never crashes on your script.** No `unwrap`/`panic` outside tests, hostile input is clamped, and
  a test runs every command with junk parameters.
- **Open.** MIT OR Apache-2.0, clean-room, every asset original or openly licensed.

## What works today

| Area | Highlights |
|---|---|
| **Writing** | Eight elements (Scene Heading, Action, Character, Parenthetical, Dialogue, Transition, Shot, General); Enter/Tab/Shift+Tab element flow ([docs/typing-parity.md](docs/typing-parity.md)); automatic capitals and parentheses; `int. ` in Action becomes a heading; SmartType; dual dialogue; undo/redo (typing coalesced); cut/copy/paste; find and replace |
| **Pages** | Letter, Courier 12, 54 lines; Action 1.5"–7.5", Character 3.7", Dialogue 2.5"–6.0", Parenthetical 3.1"–5.6", Transition right-aligned at 7.5"; page numbers from page 2; (MORE)/(CONT'D) across dialogue breaks (split at sentence ends, never after a parenthetical); headings and cues kept with what follows; action split only with two lines each side; automatic character (CONT'D); scene numbers in both margins; title page |
| **Structure** | Navigator (scenes with summaries and pages, characters), index cards with editable summaries and scene reordering, go to scene/page |
| **Reports** | Characters (speeches, words, scenes, first page), scenes (page, length in eighths, cast), locations (INT/EXT) |
| **Files** | `.fdx` open/save (incl. dual dialogue, scene numbers, index-card summaries, title page), `.fountain` open/save (the public fountain.io syntax), PDF export |
| **Window** | Menu bar, toolbar with element picker, page view with page breaks, status bar (element · page x of y · scene · words), light and dark themes, zoom |
| **Agents** | `scriptcraft-cli` (convert, run, report, commands, parity), MCP server (headless or bridged to the window), keyed loopback control channel, headless screenshots |

The full Final Draft 13 comparison — including what's not done yet (revisions, locked pages, Beat
Board, Outline Editor, collaboration, tagging, ScriptNotes, track changes, speech-to-script…) — is
in [docs/parity-checklist.md](docs/parity-checklist.md): **33 implemented · 1 partial · 29 not yet**.

## Quick start

Requires Rust 1.90+ (edition 2024).

```sh
git clone <this repo> scriptcraft && cd scriptcraft
cargo run --release -p scriptcraft                       # opens the sample script
cargo run --release -p scriptcraft -- --new              # blank script
cargo run --release -p scriptcraft -- my-script.fdx      # open a Final Draft or Fountain file
```

Writing a scene: type `int. diner - night`, **Enter** (Action), type the action, **Tab**
(Character), type `margo`, **Enter** (Dialogue), type the line, **Tab** for a parenthetical or
**Enter** to go back to Action. ⌘1–⌘8 (Ctrl on Windows/Linux) set an element directly.

## For agents: CLI, MCP and the control channel

```sh
scriptcraft-cli convert draft.fountain draft.pdf        # also .fdx ↔ .fountain
scriptcraft-cli report draft.fdx                        # characters, scenes, locations
scriptcraft-cli commands                                # every command id, shortcut, params
scriptcraft-cli run --cmd 'text.insert={"text":"INT. DINER - NIGHT"}' --cmd text.enter \
    --cmd 'text.insert={"text":"Rain on the glass."}' --cmd text.tab \
    --cmd 'text.insert={"text":"MARGO"}' --cmd text.enter \
    --cmd 'text.insert={"text":"Coffee?"}' --save scene.pdf
scriptcraft-cli mcp                                     # MCP server on stdio (headless)
scriptcraft-cli mcp --connect 127.0.0.1:7990            # drive the running app (scriptcraft --control 7990)
```

- [docs/mcp.md](docs/mcp.md): MCP tools (`list_commands`, `execute`, `batch`, `inspect_script`,
  `get_fountain`, `set_fountain`, `reports`, `screenshot`).
- [docs/control-protocol.md](docs/control-protocol.md): the JSON-lines control channel, its key
  file, and `ui_shot` for headless screenshots.

## Architecture

A Cargo workspace; lower layers never depend on higher ones (`cargo xtask layers`), and nothing
below the UI depends on egui.

| Layer | Crate | Job |
|---|---|---|
| L0 | `scriptcraft-geom` | Letter page and Courier metrics (10 cpi, 6 lpi, 54 lines) |
| L1 | `scriptcraft-doc` | screenplay model: element-typed paragraphs, title page, settings, clamped edit primitives |
| L1 | `scriptcraft-fonts`, `scriptcraft-control-key` | Courier Prime (OFL); the control key file location |
| L2 | `scriptcraft-fountain`, `scriptcraft-fdx` | Fountain and Final Draft XML import/export |
| L2 | `scriptcraft-layout` | pagination, (MORE)/(CONT'D), dual dialogue, caret and hit testing |
| L3 | `scriptcraft-pdf` | PDF export with krilla |
| L4 | `scriptcraft-engine` | Session, undo, **command registry**, typing flow, SmartType, reports, file I/O, Final Draft 13 catalog |
| L5 | `scriptcraft-mcp` | MCP server, headless or bridged to the app |
| L6 | `scriptcraft-ui-egui` | the egui window (swappable) and `ui_shot` |
| apps | `scriptcraft`, `scriptcraft-cli` | desktop app (eframe + wgpu), command line |

**Everything is a command:** a `CommandSpec` (id, label, menu location, shortcut, params doc,
`enabled`, `run`) in `crates/engine/src/cmd/*.rs`, dispatched identically by menus, shortcuts, CLI,
control channel and MCP. Quality gate: `cargo xtask ci` (fmt, clippy `-D warnings`, tests, assets,
layers, parity doc up to date).

## Roadmap

See [ROADMAP.md](ROADMAP.md). Next up: inline bold/italic/underline, revision mode with coloured
pages and locked pages, ScriptNotes, an Outline Editor and Beat Board, spelling, templates (TV,
stage play), and a web build.

## License and credits

MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Typeface:
[Courier Prime](https://github.com/quoteunquoteapps/CourierPrime) (SIL Open Font License 1.1).
Architecture patterns adapted from WordCraft (MIT OR Apache-2.0); see [NOTICE](NOTICE) and
[ATTRIBUTION.md](ATTRIBUTION.md).

ScriptCraft is a clean-room project: built from public file-format structure and widely documented
screenwriting conventions. It contains no Final Draft code, icons, images, templates, sample
scripts or fonts. Final Draft is a trademark of its owner; ScriptCraft is not affiliated with it.
