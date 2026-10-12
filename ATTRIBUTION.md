# Attribution

Every non-code asset in this repository, with its author, source and licence. `cargo xtask assets`
(part of `cargo xtask ci`) fails if a font, image or sample script is missing from this list.

ScriptCraft contains **no** Final Draft icons, images, templates, sample scripts, fonts or wording
(beyond feature names used for comparison). All icons are drawn in code (`crates/ui-egui/src/icons.rs`,
the window icon in `apps/scriptcraft/src/main.rs`).

## Fonts

| File | Author | Source | Licence |
|---|---|---|---|
| `crates/fonts/fonts/CourierPrime-Regular.ttf` | Alan Dague-Greene for Quote-Unquote Apps (The Courier Prime Project Authors) | https://github.com/quoteunquoteapps/CourierPrime via https://github.com/google/fonts/tree/main/ofl/courierprime | SIL Open Font License 1.1 (`crates/fonts/fonts/OFL.txt`) |
| `crates/fonts/fonts/CourierPrime-Bold.ttf` | same | same | SIL OFL 1.1 |
| `crates/fonts/fonts/CourierPrime-Italic.ttf` | same | same | SIL OFL 1.1 |
| `crates/fonts/fonts/CourierPrime-BoldItalic.ttf` | same | same | SIL OFL 1.1 |

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
| `docs/images/dark-smarttype.png` | The ScriptCraft contributors | Screenshot of ScriptCraft rendered by `ui_shot` | MIT OR Apache-2.0 |
