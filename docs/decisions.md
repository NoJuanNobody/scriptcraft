# Architecture decisions

## 2026-10-28 — Autosave: one durable recovery copy per running app

- Autosave still never writes the user's script file; it keeps a recovery copy in the data folder.
- Each running app claims its own copy, `recovery/run-<time>-<pid>-<n>.json`, with a `.lock`
  beside it held (an OS file lock) for the life of the process. Two windows used to share
  `recovery.json`, so one could overwrite or clear the other's unsaved work. A copy whose lock is
  free belongs to a run that ended without cleaning up; those (and the old `recovery.json`) are
  offered at start-up, newest first, one after another, and on demand from **File › Recover
  Autosaved Copy…**. A copy being offered stays locked so a second window doesn't offer it too.
- Copies are written on a background thread (serialising and writing a long script no longer
  stalls a frame); queued writes superseded by newer ones are skipped. Exit, termination signals
  and recovery wait for the writer.
- Writes are durable: temporary file, `fsync`, rename, then `fsync` of the folder (Unix), so a
  power cut leaves the old copy or the new one, never a torn file.
- Recovering a copy writes it to this run's own copy before the old one is removed, so a crash
  at that moment loses nothing. An unreadable copy is never offered and never deleted.

## 2026-10-14 — Russian support

- UI language lives in `[appearance].language` (`en` / `ru`); English remains the default. Settings schema 2 migrates schema 1 by adding the English default and preserving hand edits.
- Embedded JSON catalogues (`crates/ui-egui/locales/en.json`, `ru.json`) translate display labels and named placeholders. One registry entry generates the language enum, serialization codes and menu entries; adding a language needs no rendering-code changes. Catalogues are parsed once, missing translations fall back to English, and tests enforce matching keys/placeholders. Substitution scans templates once and never interprets braces in user-provided values. Engine command IDs, file-format keys, script text, revision names and custom categories remain unchanged; switching languages cannot rewrite a script. Technical engine/API diagnostics remain in English.
- Embed Courier Badi v1.010 (SIL OFL 1.1), a Courier Prime derivative with Dmitry Novikov's Cyrillic, in all four styles. The same faces serve the editor and PDF. Spacing glyphs advance 1228/2048 em (approximately 0.6), keeping the existing 10 cpi layout model; glyph coverage and metrics are tested. The new family also changes Latin outlines, particularly italic, so existing PDF appearance may differ slightly.
- Recognize Russian scene prefixes `ИНТ.` / `НАТ.` independently of interface language in typing, Fountain and location reports. Fountain export remains compatible through forced headings where necessary.
- Skip Cyrillic tokens in the English spelling dictionary. This avoids marking every Russian word as a spelling error; a Russian spelling dictionary is outside this feature.
