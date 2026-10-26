//! Final Draft 13 feature catalog: what ScriptCraft implements, measured.
//!
//! Feature names only (no Final Draft wording beyond them). `cargo xtask parity` writes
//! `docs/parity-checklist.md` from this list; a test checks every command id exists.

use serde_json::{Value, json};

use crate::Registry;

/// How far along a feature is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Done,
    Partial,
    Missing,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Done => "implemented",
            Status::Partial => "partial",
            Status::Missing => "not yet",
        }
    }
}

/// One Final Draft feature.
pub struct Feature {
    pub area: &'static str,
    pub name: &'static str,
    pub status: Status,
    /// Command ids that implement it (`ui.*` ids live in the egui front end).
    pub commands: &'static [&'static str],
    pub note: &'static str,
}

const fn f(area: &'static str, name: &'static str, status: Status, commands: &'static [&'static str], note: &'static str) -> Feature {
    Feature { area, name, status, commands, note }
}

use Status::{Done, Missing, Partial};

/// The catalog.
pub const FEATURES: &[Feature] = &[
    // Writing
    f(
        "Writing",
        "Screenplay elements (Scene Heading, Action, Character, Parenthetical, Dialogue, Transition, Shot, General)",
        Done,
        &[
            "element.set",
            "element.sceneHeading",
            "element.action",
            "element.character",
            "element.parenthetical",
            "element.dialogue",
            "element.transition",
            "element.shot",
            "element.general",
        ],
        "",
    ),
    f("Writing", "Enter / Tab element flow", Done, &["text.enter", "text.tab", "text.shiftTab"], "see docs/typing-parity.md"),
    f("Writing", "Automatic capitals for headings, cues, transitions, shots", Done, &["text.insert"], ""),
    f("Writing", "Automatic parentheses for parentheticals", Done, &["text.tab"], ""),
    f("Writing", "SmartType: character names and extensions", Done, &["smarttype.suggest", "smarttype.accept"], ""),
    f("Writing", "SmartType: scene intros, locations, times of day", Done, &["smarttype.suggest", "smarttype.accept"], ""),
    f("Writing", "SmartType: transitions", Done, &["smarttype.suggest", "smarttype.accept"], ""),
    f(
        "Writing",
        "Dual dialogue",
        Done,
        &["dialogue.toggleDual"],
        "two blocks side by side; splits across page breaks with (MORE)/(CONT'D) per column",
    ),
    f("Writing", "Undo / redo", Done, &["edit.undo", "edit.redo"], ""),
    f("Writing", "Cut, copy, paste", Done, &["edit.cut", "edit.copy", "edit.paste"], "keeps elements and styles; Fountain on the clipboard"),
    f("Writing", "Find and replace", Done, &["edit.find", "edit.replace", "edit.replaceAll"], ""),
    f(
        "Writing",
        "Title page",
        Done,
        &["title.set", "title.get", "title.setLines", "title.makeFreeForm", "title.useFields"],
        "fields in the usual arrangement, or free-form lines (kept from Final Draft files)",
    ),
    f(
        "Writing",
        "Bold / italic / underline",
        Done,
        &["format.bold", "format.italic", "format.underline", "format.plain"],
        "kept in FDX, Fountain and PDF",
    ),
    f(
        "Writing",
        "Spelling check",
        Done,
        &["spell.check", "spell.next", "spell.suggest", "spell.ignore", "spell.addWord", "ui.toggleSpelling"],
        "offline, English (US and British)",
    ),
    f("Writing", "Thesaurus", Missing, &[], ""),
    f(
        "Writing",
        "ScriptNotes",
        Done,
        &["note.add", "note.list", "note.edit", "note.delete", "note.deleteAll", "note.goto", "note.next", "note.previous", "ui.notes"],
        "anchored to text, colour and author; Notes tab in the navigator, margin markers; kept in FDX, Fountain [[notes]] imported",
    ),
    f(
        "Writing",
        "Alternate dialogue (Alts)",
        Done,
        &["alt.add", "alt.list", "alt.choose", "alt.next", "alt.previous", "alt.edit", "alt.delete", "ui.alternates"],
        "only the active line prints and exports as script text; alternates kept in FDX (ScriptCraft-only)",
    ),
    f("Writing", "Format Assistant", Missing, &[], ""),
    f(
        "Writing",
        "Speech-to-script (dictation)",
        Done,
        &["dictation.insert", "ui.dictation"],
        "spoken element commands and punctuation; browser speech recognition, OS dictation into the Dictation box, whisper.cpp audio files",
    ),
    f(
        "Writing",
        "Read script aloud (text to speech, cast voices)",
        Done,
        &["speech.play", "speech.pause", "speech.stop", "speech.plan", "speech.setVoice", "speech.cast", "speech.voices", "ui.readAloud"],
        "offline system voices (macOS say, Windows SAPI, espeak-ng), browser speech synthesis; a voice per character, saved with the script",
    ),
    f(
        "Writing",
        "Element settings (indents, spacing, alignment, capitals, Enter/Tab)",
        Done,
        &["settings.elements", "settings.element", "settings.resetElements", "ui.elementSettings"],
        "kept in FDX ElementSettings (Tab targets ScriptCraft-only); font is always Courier 12",
    ),
    f(
        "Writing",
        "Templates (TV, stage play, multi-camera)",
        Done,
        &["template.new", "template.apply", "template.list", "template.current"],
        "feature, TV single-camera (teaser + acts), multi-camera (double-spaced dialogue, capitals, scene letters, a page per scene), stage play; ScriptCraft's own",
    ),
    // Pagination
    f("Pagination", "Industry pagination (Letter, Courier 12, 6 lines/inch, 54 lines)", Done, &["document.layout"], ""),
    f("Pagination", "(MORE) / (CONT'D) across dialogue page breaks", Done, &["settings.set"], ""),
    f("Pagination", "Automatic character (CONT'D)", Done, &["settings.set"], ""),
    f("Pagination", "No orphaned scene headings", Done, &["document.layout"], ""),
    f("Pagination", "Page numbers", Done, &["document.layout", "page.set"], "top right from page 2 by default; any header or footer slot"),
    f("Pagination", "Scene numbers in margins", Done, &["scene.numberAll", "scene.clearNumbers", "scene.setNumber", "view.toggleSceneNumbers"], ""),
    f(
        "Pagination",
        "Headers and footers editor",
        Done,
        &["page.get", "page.set", "page.reset", "ui.pageSetup"],
        "left/centre/right slots with page, pages, title, author, date and revision fields; first-page and title-page options",
    ),
    f("Pagination", "A4 paper", Done, &["page.set", "ui.pageSetup"], "58 lines per page; same indents"),
    // Navigation and structure
    f("Structure", "Navigator: scene list", Done, &["scene.list", "scene.goto"], ""),
    f("Structure", "Index cards with summaries", Done, &["scene.setSummary", "scene.setHeading", "ui.view.cards"], ""),
    f("Structure", "Reorder scenes", Done, &["scene.move"], "drag index cards"),
    f("Structure", "Index card colours and tags", Done, &["scene.setColor", "scene.setTags"], "kept in FDX and Fountain"),
    f("Structure", "Go to page", Done, &["caret.gotoPage"], ""),
    f(
        "Structure",
        "Beat Board",
        Done,
        &["beat.add", "beat.edit", "beat.move", "beat.delete", "beat.list", "beat.link", "beat.goto", "beat.toScene", "ui.view.beats"],
        "free canvas of cards (pan, zoom, drag, resize); link a beat to a scene; FDX ListItems + DisplayBoard",
    ),
    f(
        "Structure",
        "Outline Editor (structure points)",
        Done,
        &["structure.add", "structure.edit", "structure.delete", "structure.list", "outline.list", "outline.move", "ui.view.outline"],
        "acts, sequences and sections; drag scenes under them; kept in FDX (Outline paragraphs) and Fountain (# sections)",
    ),
    f("Structure", "Story Map", Done, &["storymap.get", "ui.toggleStoryMap"], "scenes and structure scaled to pages; click to go there"),
    f(
        "Structure",
        "Bookmarks",
        Done,
        &["bookmark.add", "bookmark.remove", "bookmark.rename", "bookmark.list", "bookmark.goto", "bookmark.next", "bookmark.previous"],
        "navigator tab; FDX Bookmark attribute",
    ),
    f(
        "Structure",
        "Split screen / multiple views",
        Done,
        &["ui.split", "ui.splitView", "ui.focusPane"],
        "two panes, side by side or stacked, each with its own view and scroll; one caret",
    ),
    // Production
    f("Production", "Character report", Done, &["report.characters", "ui.view.reports"], ""),
    f("Production", "Scene report (page, length in eighths, cast)", Done, &["report.scenes", "ui.view.reports"], ""),
    f("Production", "Location report", Done, &["report.locations", "ui.view.reports"], ""),
    f(
        "Production",
        "Revision mode and colored revision pages",
        Done,
        &[
            "revision.mode",
            "revision.get",
            "revision.set",
            "revision.editSet",
            "revision.addSet",
            "revision.clearMarks",
            "revision.pages",
            "print.render",
            "ui.revisions",
        ],
        "White, Blue, Pink, Yellow, Green… sets; * in the margin of changed lines; revision label in the page header; coloured pages; print revised pages only; kept in FDX",
    ),
    f(
        "Production",
        "Locked pages (A/B pages)",
        Done,
        &["page.lock", "page.relock", "page.unlock", "page.list"],
        "new text flows onto 12A, 12B; a page that lost text prints 12-13; kept in FDX",
    ),
    f("Production", "Omitted scenes", Done, &["scene.omit", "scene.unomit"], "prints the number and OMITTED; the text is kept; kept in FDX"),
    f(
        "Production",
        "Locked scene numbers (12A)",
        Done,
        &["scene.lockNumbers", "scene.unlockNumbers"],
        "inserted scenes are numbered after the one before; kept in FDX",
    ),
    f(
        "Production",
        "Production tagging / breakdown",
        Done,
        &["tag.add", "tag.remove", "tag.list", "tag.categories", "report.breakdown", "report.breakdownCsv", "file.exportBreakdown", "ui.toggleTags"],
        "standard and custom categories; breakdown per scene; CSV export; tags kept in FDX (ScriptCraft-only)",
    ),
    f(
        "Production",
        "Track changes / compare scripts",
        Done,
        &[
            "track.toggle",
            "track.list",
            "track.accept",
            "track.reject",
            "track.acceptAll",
            "track.rejectAll",
            "track.next",
            "track.previous",
            "compare.with",
            "ui.trackChanges",
            "ui.compare",
        ],
        "per-author insertions, deletions, new paragraphs and element changes; accept/reject one, all or by author; compare two drafts by paragraph with word diffs (also `scriptcraft-cli compare`); kept in FDX (ScriptCraft-only)",
    ),
    f("Production", "Watermarks", Missing, &[], ""),
    f(
        "Production",
        "Collaboration (real time)",
        Done,
        &["collab.host", "collab.join", "collab.leave", "collab.status", "collab.peers", "ui.collaborate"],
        "host or join with a session key over the local network; live merge of everyone's typing; others' carets and names on the page; desktop only",
    ),
    // Files
    f("Files", "Final Draft XML (.fdx) open and save", Done, &["file.open", "file.save", "file.exportFdx"], "bold, italic, underline kept"),
    f(
        "Files",
        "Fountain open and save",
        Done,
        &["file.open", "file.save", "file.exportFountain"],
        "Shot and General kept with a `[[shot]]` / `[[general]]` note; other apps read them as action",
    ),
    f("Files", "PDF export", Done, &["file.exportPdf"], "embedded Courier Badi, selectable text"),
    f(
        "Files",
        "Print",
        Done,
        &["print.pages", "print.render", "print.printers", "print.send", "ui.print", "ui.printNow"],
        "page range, title page, revised pages; printer and copies (CUPS on macOS and Linux, the default printer on Windows), the browser's print window on the web; or open in the PDF viewer",
    ),
    f("Files", "Templates (.fdxt)", Missing, &[], ""),
    f("Files", "Autosave, crash recovery and backups", Done, &[], "data folder; recovery offered after a crash, previous versions kept on save"),
    f("Files", "Recent files", Done, &["ui.openRecent", "ui.clearRecent"], ""),
    f("Files", "Settings shared with the other Craft apps (config.toml, folder rules)", Done, &[], "craft-settings from storytold/craft-libs"),
    f(
        "Files",
        "Import from other formats (Word, Celtx, PDF)",
        Done,
        &["file.import", "file.open", "ui.import"],
        "Word .docx by paragraph style, indent or text; legacy .celtx projects; PDF by indent (scene numbers, title page, dual dialogue); also `scriptcraft-cli convert`",
    ),
    // View
    f("View", "Page view with page breaks", Done, &["ui.view.page"], ""),
    f("View", "Dark mode", Done, &["ui.theme.toggle"], ""),
    f("View", "Zoom (menus, Ctrl/Cmd+wheel, pinch, fit page width)", Done, &["ui.zoomIn", "ui.zoomOut", "ui.zoomFit"], ""),
    f("View", "Speed view (no page breaks)", Done, &["view.speed", "ui.view.speed"], ""),
    f("View", "Focus / typewriter mode", Missing, &[], ""),
    f(
        "View",
        "Runs in the browser (WebAssembly)",
        Done,
        &["file.load", "document.export"],
        "cargo xtask web; open via the file picker, save as a download",
    ),
    // Automation (beyond Final Draft)
    f("Automation", "Command-line conversion", Done, &["file.open", "file.exportPdf"], "scriptcraft-cli convert"),
    f("Automation", "MCP server and JSON control channel", Done, &["document.inspect"], "scriptcraft-cli mcp"),
];

/// Counts: (done, partial, missing).
pub fn counts() -> (usize, usize, usize) {
    let c = |s: Status| FEATURES.iter().filter(|f| f.status == s).count();
    (c(Done), c(Partial), c(Missing))
}

/// The catalog as JSON, with each command's presence in `reg` (UI commands are assumed).
pub fn parity(reg: &Registry) -> Value {
    let (d, p, m) = counts();
    json!({
        "done": d, "partial": p, "missing": m, "total": FEATURES.len(),
        "features": FEATURES.iter().map(|f| json!({
            "area": f.area, "feature": f.name, "status": f.status.label(), "note": f.note,
            "commands": f.commands.iter().map(|c| json!({"id": c, "present": c.starts_with("ui.") || reg.get(c).is_some()})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// `docs/parity-checklist.md`.
pub fn markdown() -> String {
    let (d, p, m) = counts();
    let mut out = String::from("# Final Draft 13 parity checklist\n\n");
    out.push_str("Generated by `cargo xtask parity` from `crates/engine/src/catalog.rs`. Feature names are used for comparison only.\n\n");
    out.push_str(&format!("**{d} implemented · {p} partial · {m} not yet** (of {}).\n", FEATURES.len()));
    let mut area = "";
    for f in FEATURES {
        if f.area != area {
            area = f.area;
            out.push_str(&format!("\n## {area}\n\n| Feature | Status | Commands | Notes |\n|---|---|---|---|\n"));
        }
        let mark = match f.status {
            Done => "✅ implemented",
            Partial => "🟡 partial",
            Missing => "⬜ not yet",
        };
        let cmds = f.commands.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
        out.push_str(&format!("| {} | {mark} | {cmds} | {} |\n", f.name, f.note));
    }
    out
}
