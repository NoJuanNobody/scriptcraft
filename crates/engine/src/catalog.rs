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
    f("Writing", "Dual dialogue", Done, &["dialogue.toggleDual"], "two blocks side by side"),
    f("Writing", "Undo / redo", Done, &["edit.undo", "edit.redo"], ""),
    f("Writing", "Cut, copy, paste", Done, &["edit.cut", "edit.copy", "edit.paste"], "plain text"),
    f("Writing", "Find and replace", Done, &["edit.find", "edit.replace", "edit.replaceAll"], ""),
    f("Writing", "Title page", Done, &["title.set", "title.get"], "fixed arrangement"),
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
    f("Writing", "ScriptNotes", Missing, &[], ""),
    f("Writing", "Alternate dialogue (Alts)", Missing, &[], ""),
    f("Writing", "Format Assistant", Missing, &[], ""),
    f("Writing", "Speech-to-script (dictation)", Missing, &[], ""),
    f("Writing", "Read script aloud (text to speech, cast voices)", Missing, &[], ""),
    f(
        "Writing",
        "Element settings (indents, spacing, alignment, capitals, Enter/Tab)",
        Done,
        &["settings.elements", "settings.element", "settings.resetElements", "ui.elementSettings"],
        "kept in FDX ElementSettings (Tab targets ScriptCraft-only); font is always Courier 12",
    ),
    f("Writing", "Templates (TV, stage play, multi-camera)", Missing, &[], "feature-film format only"),
    // Pagination
    f("Pagination", "Industry pagination (Letter, Courier 12, 6 lines/inch, 54 lines)", Done, &["document.layout"], ""),
    f("Pagination", "(MORE) / (CONT'D) across dialogue page breaks", Done, &["settings.set"], ""),
    f("Pagination", "Automatic character (CONT'D)", Done, &["settings.set"], ""),
    f("Pagination", "No orphaned scene headings", Done, &["document.layout"], ""),
    f("Pagination", "Page numbers", Done, &["document.layout"], "top right from page 2"),
    f("Pagination", "Scene numbers in margins", Done, &["scene.numberAll", "scene.clearNumbers", "scene.setNumber", "view.toggleSceneNumbers"], ""),
    f("Pagination", "Headers and footers editor", Missing, &[], ""),
    f("Pagination", "A4 paper", Missing, &[], ""),
    // Navigation and structure
    f("Structure", "Navigator: scene list", Done, &["scene.list", "scene.goto"], ""),
    f("Structure", "Index cards with summaries", Done, &["scene.setSummary", "scene.setHeading", "ui.view.cards"], ""),
    f("Structure", "Reorder scenes", Done, &["scene.move"], "move by card"),
    f("Structure", "Go to page", Done, &["caret.gotoPage"], ""),
    f("Structure", "Beat Board", Missing, &[], ""),
    f("Structure", "Outline Editor (structure points)", Missing, &[], ""),
    f("Structure", "Story Map", Missing, &[], ""),
    f("Structure", "Bookmarks", Missing, &[], ""),
    f("Structure", "Split screen / multiple views", Missing, &[], ""),
    // Production
    f("Production", "Character report", Done, &["report.characters", "ui.view.reports"], ""),
    f("Production", "Scene report (page, length in eighths, cast)", Done, &["report.scenes", "ui.view.reports"], ""),
    f("Production", "Location report", Done, &["report.locations", "ui.view.reports"], ""),
    f("Production", "Revision mode and colored revision pages", Missing, &[], ""),
    f("Production", "Locked pages (A/B pages)", Missing, &[], ""),
    f("Production", "Omitted scenes", Missing, &[], ""),
    f("Production", "Production tagging / breakdown", Missing, &[], ""),
    f("Production", "Track changes / compare scripts", Missing, &[], ""),
    f("Production", "Watermarks", Missing, &[], ""),
    f("Production", "Collaboration (real time)", Missing, &[], ""),
    // Files
    f("Files", "Final Draft XML (.fdx) open and save", Done, &["file.open", "file.save", "file.exportFdx"], "bold, italic, underline kept"),
    f("Files", "Fountain open and save", Done, &["file.open", "file.save", "file.exportFountain"], "Shot and General become Action"),
    f("Files", "PDF export", Done, &["file.exportPdf"], "embedded Courier Prime, selectable text"),
    f("Files", "Print", Partial, &["file.exportPdf"], "export a PDF and print it"),
    f("Files", "Templates (.fdxt)", Missing, &[], ""),
    f("Files", "Backup / autosave", Missing, &[], ""),
    f("Files", "Import from other formats (Word, Celtx, PDF)", Missing, &[], ""),
    // View
    f("View", "Page view with page breaks", Done, &["ui.view.page"], ""),
    f("View", "Dark mode", Done, &["ui.theme.toggle"], ""),
    f("View", "Zoom", Done, &["ui.zoomIn", "ui.zoomOut"], ""),
    f("View", "Speed / normal view", Missing, &[], ""),
    f("View", "Focus / typewriter mode", Missing, &[], ""),
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
