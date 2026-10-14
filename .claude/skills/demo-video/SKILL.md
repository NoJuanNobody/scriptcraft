---
name: demo-video
description: Record a narrated demo video of ScriptCraft that proves a PR or milestone works. It drives the real app window offscreen through its own menus, keys and clicks, shows the printed PDF, and then reviews the frames before sharing. Use when asked to "make a video", "make a clip", "record a demo", "prove it works", "verify the features in PR #N" or "show me v0.X working".
---

# ScriptCraft demo video

A tour is a JSON-lines script that `crates/ui-egui/examples/ui_demo.rs` plays against the real
`ScriptApp`. It renders offscreen with wgpu, so a locked screen or a hidden window doesn't matter. It draws a
caption bar, a pointer and a corner badge, and writes PNG frames at 15 fps. `docs/demo/record-tour.sh` builds
it, plays the tour and encodes an MP4 with ffmpeg. Past tours: `docs/demo/pr33-tour.jsonl` (v0.2),
`docs/demo/v03-tour.jsonl` (v0.3) and `docs/demo/v04-tour.jsonl` (v0.4: silent Read Aloud speaker, a second collaborator via `peer` lines). Copy the closest one.

Requirements: ffmpeg; on macOS, `swift` for PDF pages (`docs/demo/pdf-pages.swift`, PDFKit).

## 1. List what must be shown

- Read the PR body and its issues (`gh pr view N`, `gh issue view N`).
- Plan one segment per issue, in a sensible order. Later segments see earlier changes: once pages are
  locked, every later edit makes A pages; an omitted scene hides its text from `{"text": …}` targets.
- Get exact names from the code, not memory:
  - commands: `cargo run -q -p scriptcraft-cli -- commands`
  - window commands: the `("ui.…", "Label", …)` table in `crates/ui-egui/src/lib.rs`
  - menu and button labels: `crates/ui-egui/src/chrome.rs` and the dialog files
- Sample script text for targets is in `crates/engine/src/sample.fountain`. Scene and page facts:
  `cargo run -q -p scriptcraft-cli -- run --sample --cmd scene.list --cmd page.list`.

## 2. Write the tour: `docs/demo/<name>-tour.jsonl`

One action per line; `//` lines are comments. The full list is at the top of `ui_demo.rs`. The ones
you'll use most:

| Line | Does |
|---|---|
| `{"badge": "ScriptCraft v0.3 · PR #40"}` | corner label (first line) |
| `{"title": "#13  Revision mode", "sub": "…"}` | caption for the segment |
| `{"hold": 1.5}` | keep the picture for 1.5 s |
| `{"point": {"label": "Production"}, "secs": 0.7}` then `{"click": "left"}` | use a real menu or button (also `"right"`, `"count": 2`) |
| `{"point": {"text": "She taps the glass"}}` | glide to script text; it scrolls there and **moves the caret** |
| `{"type": "Not tonight."}` / `{"key": "P", "cmd": true}` | real typing and shortcuts |
| `{"cmd": "revision.mode", "params": {"on": true, "set": "Blue"}}` | any engine or `ui.*` command |
| `{"cmd": "file.exportPdf", "params": {"path": "@WORK@/out.pdf"}}` then `{"run": ["swift", "@ROOT@/docs/demo/pdf-pages.swift", "@WORK@/out.pdf", "@WORK@/pdf_frames"]}` then `{"pictures": "@WORK@/pdf_frames", "frames_each": 38, "bar": "Preview · out.pdf"}` and `{"pictures_off": true}` | show the printed pages, two per frame |

Rules that keep it honest and readable:
- **Show it through the UI wherever possible.** Menus, right-click menus, dialogs and keys prove the
  wiring. Use `cmd` lines for setup or bulk text.
- **Select text with `select.text` after pointing.** `point` places the caret, which drops the selection.
  A double-click selects only one word.
- **Avoid the bottom ~130 px of the window,** which the caption covers. Zoom out (`ui.zoomOut` ×3) to show
  a page break, then `ui.zoomReset`.
- Labels drawn with a painter, such as the tag category list, have no label to target. Hover the parent
  menu, then run the command.
- Close what you open (`ui.pageSetup`, `ui.revisions` and `ui.alternates` toggle; `Escape` closes menus).
- End with a card naming the PR, its issues and the CI status.

## 3. Record

```sh
docs/demo/record-tour.sh docs/demo/<name>-tour.jsonl docs/demo/<name>-tour.mp4 <scratch>/<name>
```

- `@ROOT@` is the repo; `@WORK@` is a fresh `<scratch>/<name>/run` folder for each recording.
- Settings are isolated (`SCRIPTCRAFT_CONFIG_DIR=`).
- A recording takes about 15 s plus the build.
- `no target {…}` on stderr means a `point` missed: fix the text or label.
- A `cmd` error is printed too: fix it, don't ignore it.

## 4. Review every segment before sharing (required)

Make contact sheets and **look at all of them**:

```sh
F=<scratch>/<name>/run/frames
mkdir -p <scratch>/<name>/sheets
ffmpeg -loglevel error -y -framerate 15 -i "$F/f%05d.png" \
  -vf "select='not(mod(n\,25))',scale=960:-1,tile=3x3" -vsync vfr <scratch>/<name>/sheets/s%02d.png
```

- Re-make the sheets after **every** recording; old sheets are stale.
- To look closely at one moment, make a sheet starting there (`-start_number N`, smaller `mod`), or
  read a frame or PDF page PNG directly.
- For each segment, check that the feature is **actually visible**:
  - the dialog opened;
  - the text changed as intended (not just the wrong word);
  - the page break really happened;
  - the status bar and headers say what the caption claims;
  - the PDF pages match the app.
- Anything wrong in the app itself is a finding. Note it, and file it if it isn't tracked yet. Never edit the tour
  to hide a real bug; say so in the report instead.

## 5. Share

- Commit the tour, the MP4 (keep it a few MB: `-crf 24`, 1440 px wide) and any tooling changes on
  the PR's branch. Run `cargo xtask ci` first if Rust changed.
- Comment on the PR with a link that plays: `https://github.com/<owner>/<repo>/blob/<branch>/docs/demo/<name>-tour.mp4`
  ("View raw" plays it). Add a short list of what each segment proves and any bugs it found.
- Tell the user the length, the link, and the findings.
