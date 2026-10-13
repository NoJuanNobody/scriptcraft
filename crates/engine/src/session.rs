//! The editing session: script, selection, undo history, layout cache, file path.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use scriptcraft_doc::{Element, Pos, Script};
use scriptcraft_layout::Layout;
use serde::Serialize;
use serde_json::Value;

use crate::{CmdError, Registry};

/// Most undo steps kept.
const UNDO_LIMIT: usize = 500;

/// A selection: `anchor` stays, `focus` moves (the caret).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Selection {
    pub anchor: Pos,
    pub focus: Pos,
}

impl Selection {
    pub const fn caret(p: Pos) -> Self {
        Selection { anchor: p, focus: p }
    }
    pub fn is_caret(&self) -> bool {
        self.anchor == self.focus
    }
    /// (start, end).
    pub fn ordered(&self) -> (Pos, Pos) {
        if self.anchor <= self.focus { (self.anchor, self.focus) } else { (self.focus, self.anchor) }
    }
}

#[derive(Clone)]
struct Snapshot {
    script: Script,
    sel: Selection,
    label: &'static str,
}

/// One open script and everything needed to edit it.
pub struct Session {
    pub script: Script,
    pub sel: Selection,
    pub registry: Arc<Registry>,
    /// Where the script was opened from / saved to.
    pub path: Option<PathBuf>,
    /// Changed since the last open or save.
    pub dirty: bool,
    /// The last status message (for the status bar).
    pub status: String,
    /// The column the caret keeps while moving up and down (points).
    pub goal_x: Option<f32>,
    /// The SmartType popup was dismissed for this paragraph (Escape); reset when typing.
    pub suggest_dismissed: bool,
    /// Style for the next typed text, set by Bold/Italic/Underline with no selection.
    pub pending_style: Option<scriptcraft_doc::Style>,
    /// Words Spelling ignores in this session (lowercase).
    pub ignored_words: std::collections::BTreeSet<String>,
    /// The user's own dictionary (lowercase); the app loads and saves it.
    pub user_words: std::collections::BTreeSet<String>,
    /// The last multi-paragraph copy: the text put on the clipboard and the paragraphs, so
    /// pasting it back keeps every element, style and flag.
    pub clip: Option<(String, Vec<scriptcraft_doc::Para>)>,
    history: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    typing_open: bool,
    layout: Option<Arc<Layout>>,
    /// Speed view: the page view shows one continuous page (reports still use real pages).
    pub speed: bool,
    speed_layout: Option<Arc<Layout>>,
    /// Counts changes to the script (anything that drops the layout cache), for caches keyed
    /// on the script's state.
    revision: u64,
}

impl Session {
    pub fn new(script: Script) -> Self {
        let mut s = Session {
            script,
            sel: Selection::default(),
            registry: Arc::new(crate::cmd::registry()),
            path: None,
            dirty: false,
            status: String::new(),
            goal_x: None,
            suggest_dismissed: false,
            pending_style: None,
            ignored_words: Default::default(),
            user_words: Default::default(),
            clip: None,
            history: Vec::new(),
            redo: Vec::new(),
            typing_open: false,
            layout: None,
            speed: false,
            speed_layout: None,
            revision: 0,
        };
        let _ = s.script.normalize();
        s
    }

    /// The sample screenplay.
    pub fn sample() -> Self {
        Session::new(crate::sample::script())
    }

    /// Replace the whole script (open, new): history is cleared.
    pub fn replace_script(&mut self, script: Script) {
        self.script = script;
        let _ = self.script.normalize();
        self.sel = Selection::default();
        self.history.clear();
        self.redo.clear();
        self.typing_open = false;
        self.invalidate();
        self.dirty = false;
        self.goal_x = None;
        self.pending_style = None;
    }

    /// The pagination of the current script (cached until the next change).
    pub fn layout(&mut self) -> Arc<Layout> {
        if let Some(l) = &self.layout {
            return l.clone();
        }
        let l = Arc::new(scriptcraft_layout::layout(&self.script));
        self.layout = Some(l.clone());
        l
    }

    /// The layout the page view shows: real pages, or one continuous page in speed view.
    /// Caret movement follows it.
    pub fn view_layout(&mut self) -> Arc<Layout> {
        if !self.speed {
            return self.layout();
        }
        if let Some(l) = &self.speed_layout {
            return l.clone();
        }
        let l = Arc::new(scriptcraft_layout::layout_with(&self.script, scriptcraft_layout::Mode::Speed));
        self.speed_layout = Some(l.clone());
        l
    }

    /// Drop the cached layout (after changing the script directly).
    pub fn invalidate(&mut self) {
        self.layout = None;
        self.speed_layout = None;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Changes so far (increases whenever the script may have changed).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The caret paragraph's element.
    pub fn element(&self) -> Element {
        self.script.para(self.sel.focus.para).map_or(Element::Action, |p| p.element)
    }

    /// Keep the selection on existing text.
    pub fn clamp_selection(&mut self) {
        self.sel.anchor = self.script.clamp(self.sel.anchor);
        self.sel.focus = self.script.clamp(self.sel.focus);
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// The label of the step Undo would revert.
    pub fn undo_label(&self) -> Option<&'static str> {
        self.history.last().map(|s| s.label)
    }

    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.history.pop() else { return false };
        let cur = Snapshot { script: std::mem::replace(&mut self.script, snap.script), sel: self.sel, label: snap.label };
        self.redo.push(cur);
        self.sel = snap.sel;
        self.after_change();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.redo.pop() else { return false };
        let cur = Snapshot { script: std::mem::replace(&mut self.script, snap.script), sel: self.sel, label: snap.label };
        self.history.push(cur);
        self.sel = snap.sel;
        self.after_change();
        true
    }

    fn after_change(&mut self) {
        self.typing_open = false;
        self.invalidate();
        self.dirty = true;
        self.clamp_selection();
    }

    /// Run a command by id. Parameters are hostile: commands validate them and return errors.
    /// A command that fails (or, as a last resort, panics) leaves the script as it was.
    pub fn run(&mut self, id: &str, params: &Value) -> Result<Value, CmdError> {
        let reg = self.registry.clone();
        let Some(spec) = reg.get(id) else { return Err(CmdError::Unknown(id.to_string())) };
        if let Some(why) = (spec.enabled)(self) {
            return Err(CmdError::Disabled(format!("{id}: {why}")));
        }
        let history_cmd = matches!(id, "edit.undo" | "edit.redo");
        let typing = id == "text.insert";
        // Consecutive typing is one undo step; anything else in between ends it.
        let coalesce = typing && self.typing_open;
        if !typing {
            self.typing_open = false;
        }
        if !matches!(id, "caret.up" | "caret.down") {
            self.goal_x = None;
        }
        if !id.starts_with("format.") && id != "text.insert" && id != "edit.paste" {
            self.pending_style = None;
        }
        if id.starts_with("text.") || id.starts_with("element.") {
            self.suggest_dismissed = false;
        }
        let before = (spec.mutates && !history_cmd).then(|| (self.script.clone(), self.sel));
        let result = catch_unwind(AssertUnwindSafe(|| (spec.run)(self, params)));
        let result = match result {
            Ok(r) => r,
            Err(_) => Err(CmdError::Failed(format!("{id}: internal error (the script was restored)"))),
        };
        self.clamp_selection();
        let Some((old, old_sel)) = before else {
            if history_cmd {
                self.typing_open = false;
            }
            return result;
        };
        match &result {
            Ok(_) if self.script != old => {
                // Revision marks follow the edit; in revision mode the edit is marked.
                scriptcraft_doc::revision::mark_changes(&old, &mut self.script);
                let _ = self.script.normalize();
                self.clamp_selection();
                if !coalesce {
                    self.history.push(Snapshot { script: old, sel: old_sel, label: spec.label });
                    if self.history.len() > UNDO_LIMIT {
                        self.history.remove(0);
                    }
                }
                self.typing_open = typing;
                self.redo.clear();
                self.invalidate();
                self.dirty = true;
            }
            Ok(_) => {}
            Err(_) => {
                self.script = old;
                self.sel = old_sel;
                self.invalidate();
            }
        }
        result
    }
}
