# Attribution

Every non-code asset in this repository, with its author, source and licence. `cargo xtask assets`
(part of `cargo xtask ci`) fails if a font, image or sample script is missing from this list.

ScriptCraft contains **no** Final Draft icons, images, templates, sample scripts, fonts or wording
(beyond feature names used for comparison). All icons are drawn in code (`crates/ui-egui/src/icons.rs`,
the window icon in `apps/scriptcraft/src/main.rs`).

## Fonts

| File | Author | Source | Licence |
|---|---|---|---|
| `crates/fonts/fonts/CourierBadi-Regular.ttf` | Alan Dague-Greene (Courier Prime), Dmitry Novikov (Cyrillic), LaPingvino and the Courier Badi contributors | https://github.com/LaPingvino/courier-badi/releases/tag/v1.010 (`fonts/ttf`, not the contrast variant) | SIL Open Font License 1.1 (`crates/fonts/fonts/OFL.txt`) |
| `crates/fonts/fonts/CourierBadi-Bold.ttf` | same | same | SIL OFL 1.1 |
| `crates/fonts/fonts/CourierBadi-Italic.ttf` | same | same | SIL OFL 1.1 |
| `crates/fonts/fonts/CourierBadi-BoldItalic.ttf` | same | same | SIL OFL 1.1 |

## Word lists

| File | Author | Source | Licence |
|---|---|---|---|
| `crates/spell/words/en.txt` | Kevin Atkinson (English Speller Database / SCOWL), from 12dicts and ENABLE2K (public domain) | https://app.aspell.net/create (size 60, US + British -ise) | ESDB licence: free use, copy, modification and distribution with the copyright notice (`crates/spell/words/LICENSE-SCOWL.txt`) |
| `crates/spell/words/common.txt` | same | same (size 35) | same |

## Sample script

| File | Author | Licence |
|---|---|---|
| `crates/engine/src/sample.fountain` ("Night Signal") | The ScriptCraft contributors, written for this project | CC0 1.0 (public domain dedication) |

## Images

| File | Author | Source | Licence |
|---|---|---|---|
| `docs/images/hero.png` | The ScriptCraft contributors | Screenshot of ScriptCraft rendered by `ui_shot` | MIT OR Apache-2.0 |
| `docs/images/cards.png` | The ScriptCraft contributors | Screenshot of ScriptCraft rendered by `ui_shot` | MIT OR Apache-2.0 |
| `docs/images/reports.png` | The ScriptCraft contributors | Screenshot of ScriptCraft rendered by `ui_shot` | MIT OR Apache-2.0 |
| `docs/demo/pr33-tour.mp4` | The ScriptCraft contributors | Recording of ScriptCraft by `ui_demo` (docs/demo/record.sh) | MIT OR Apache-2.0 |
| `docs/images/dark-smarttype.png` | The ScriptCraft contributors | Screenshot of ScriptCraft rendered by `ui_shot` | MIT OR Apache-2.0 |
| `docs/explainer/poster.png` | The ScriptCraft contributors | Frame of the explainer video (`ui_shot` screenshots; Anton and JetBrains Mono, both SIL OFL 1.1) | MIT OR Apache-2.0 |
| `docs/explainer/explainer.mp4` | The ScriptCraft contributors | Narrated explainer: `ui_shot` screenshots, `scriptcraft-cli` output, Kokoro TTS voice; Anton and JetBrains Mono (SIL OFL 1.1) | MIT OR Apache-2.0 |
| `docs/images/russian.png` | The ScriptCraft contributors | Screenshot of ScriptCraft rendered by `ui_shot` from `docs/demo/russian-tour.jsonl` | MIT OR Apache-2.0 |
