# Architecture decisions

## 2026-10-14 — Russian support

- UI language lives in `[appearance].language` (`en` / `ru`); English remains the default. Settings schema 2 migrates schema 1 by adding the English default and preserving hand edits.
- Embedded JSON catalogues (`crates/ui-egui/locales/en.json`, `ru.json`) translate display labels and named placeholders. One registry entry generates the language enum, serialization codes and menu entries; adding a language needs no rendering-code changes. Catalogues are parsed once, missing translations fall back to English, and tests enforce matching keys/placeholders. Substitution scans templates once and never interprets braces in user-provided values. Engine command IDs, file-format keys, script text, revision names and custom categories remain unchanged; switching languages cannot rewrite a script. Technical engine/API diagnostics remain in English.
- Embed Courier Badi v1.010 (SIL OFL 1.1), a Courier Prime derivative with Dmitry Novikov's Cyrillic, in all four styles. The same faces serve the editor and PDF. Spacing glyphs advance 1228/2048 em (approximately 0.6), keeping the existing 10 cpi layout model; glyph coverage and metrics are tested. The new family also changes Latin outlines, particularly italic, so existing PDF appearance may differ slightly.
- Recognize Russian scene prefixes `ИНТ.` / `НАТ.` independently of interface language in typing, Fountain and location reports. Fountain export remains compatible through forced headings where necessary.
- Skip Cyrillic tokens in the English spelling dictionary. This avoids marking every Russian word as a spelling error; a Russian spelling dictionary is outside this feature.
