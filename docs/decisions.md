# Architecture decisions

## 2026-10-28 — Scene-heading abbreviations are the user's

- The words that start a scene heading (interior, exterior, both) and the times of day are data:
  `scriptcraft_doc::Sluglines`, with built-in sets per language (`en`, `ru`, `uk`, `de`, `fr`,
  `es`, `it`, `pt`, `pl`) that the user picks and edits in **Tools › Scene Heading
  Abbreviations…**. One list cannot serve everyone: `EST.` is "establishing" in English and
  "exterior" in Italian.
- They are a user preference (`[sluglines]` in `config.toml`), not part of the script: Fountain
  and FDX have nowhere to keep them, and a writer uses the same words in every script. The app
  copies them into `Session::sluglines`; typing, Fountain reading, paste, SmartType and the scene
  and location reports read them from there. A missing section means the standard set (English
  and Russian), so existing profiles behave as before.
- The Fountain writer keeps the standard words: a heading in other words is written forced
  (`.INNEN. KÜCHE`), so every Fountain app and every ScriptCraft profile reads it back.
- Typing converts Action only when the word is typed exactly as listed and followed by a space
  (`innen. `), so ordinary sentences starting with the same word stay Action. Reading a heading
  also accepts the word without its dot (`INNEN KÜCHE`), as Fountain does for `INT`.
- Import from Word and PDF still recognises headings by the standard words only.

## 2026-10-14 — Russian support

- UI language lives in `[appearance].language` (`en` / `ru`); English remains the default. Settings schema 2 migrates schema 1 by adding the English default and preserving hand edits.
- Embedded JSON catalogues (`crates/ui-egui/locales/en.json`, `ru.json`) translate display labels and named placeholders. One registry entry generates the language enum, serialization codes and menu entries; adding a language needs no rendering-code changes. Catalogues are parsed once, missing translations fall back to English, and tests enforce matching keys/placeholders. Substitution scans templates once and never interprets braces in user-provided values. Engine command IDs, file-format keys, script text, revision names and custom categories remain unchanged; switching languages cannot rewrite a script. Technical engine/API diagnostics remain in English.
- Embed Courier Badi v1.010 (SIL OFL 1.1), a Courier Prime derivative with Dmitry Novikov's Cyrillic, in all four styles. The same faces serve the editor and PDF. Spacing glyphs advance 1228/2048 em (approximately 0.6), keeping the existing 10 cpi layout model; glyph coverage and metrics are tested. The new family also changes Latin outlines, particularly italic, so existing PDF appearance may differ slightly.
- Recognize Russian scene prefixes `ИНТ.` / `НАТ.` independently of interface language in typing, Fountain and location reports. Fountain export remains compatible through forced headings where necessary.
- Skip Cyrillic tokens in the English spelling dictionary. This avoids marking every Russian word as a spelling error; a Russian spelling dictionary is outside this feature.
