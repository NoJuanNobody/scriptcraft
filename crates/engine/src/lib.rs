//! ScriptCraft engine: the editing session and its commands.
//!
//! **Everything is a command.** Each user-visible action is a [`CommandSpec`] with a stable id
//! (`element.character`, `text.enter`), a label, its menu location, a default shortcut, a params
//! description and a `run` function. Menus, toolbar, shortcuts, the CLI, the JSON control channel
//! and the MCP server all call [`Session::run`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod b64;
pub mod breakdown;
pub mod catalog;
pub mod cmd;
pub mod io;
pub mod reports;
pub mod sample;
mod session;
pub mod smarttype;
pub mod typing;

use std::collections::HashMap;

use serde_json::Value;

pub use scriptcraft_doc as doc;
pub use scriptcraft_layout as layout;
pub use session::{Selection, Session};

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CmdError {
    #[error("unknown command `{0}`")]
    Unknown(String),
    #[error("bad parameters: {0}")]
    Params(String),
    #[error("not available: {0}")]
    Disabled(String),
    #[error("{0}")]
    Failed(String),
}

impl From<scriptcraft_doc::DocError> for CmdError {
    fn from(e: scriptcraft_doc::DocError) -> Self {
        CmdError::Failed(e.to_string())
    }
}

pub type CmdResult = Result<Value, CmdError>;

/// A registered command.
#[derive(Clone)]
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Where it lives in the UI: `Format › Elements`, `File`, `Edit`…
    pub location: &'static str,
    /// Default shortcut in a portable form (`Mod+1`, `Mod+Shift+Z`). `Mod` = Ctrl, or ⌘ on macOS.
    pub shortcut: &'static str,
    /// Parameter description for agents (`{"element": string}`).
    pub params: &'static str,
    /// Changes the document (undoable).
    pub mutates: bool,
    /// `None` = enabled; `Some(reason)` = disabled.
    pub enabled: fn(&Session) -> Option<&'static str>,
    pub run: fn(&mut Session, &Value) -> CmdResult,
}

fn always(_: &Session) -> Option<&'static str> {
    None
}

impl CommandSpec {
    pub const fn new(id: &'static str, label: &'static str, location: &'static str, run: fn(&mut Session, &Value) -> CmdResult) -> Self {
        CommandSpec { id, label, location, shortcut: "", params: "{}", mutates: true, enabled: always, run }
    }
    pub const fn key(mut self, s: &'static str) -> Self {
        self.shortcut = s;
        self
    }
    pub const fn params(mut self, p: &'static str) -> Self {
        self.params = p;
        self
    }
    /// Doesn't change the document (navigation, queries, export).
    pub const fn pure(mut self) -> Self {
        self.mutates = false;
        self
    }
    pub const fn when(mut self, f: fn(&Session) -> Option<&'static str>) -> Self {
        self.enabled = f;
        self
    }
}

/// All commands by id.
pub struct Registry {
    specs: Vec<CommandSpec>,
    by_id: HashMap<&'static str, usize>,
}

impl Registry {
    pub fn new(specs: Vec<CommandSpec>) -> Self {
        let by_id = specs.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        Registry { specs, by_id }
    }
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.by_id.get(id).and_then(|i| self.specs.get(*i))
    }
    pub fn all(&self) -> &[CommandSpec] {
        &self.specs
    }
    /// Command list as JSON (for agents).
    pub fn describe(&self, s: Option<&Session>) -> Value {
        Value::Array(
            self.specs
                .iter()
                .map(|c| {
                    let mut v = serde_json::json!({"id": c.id, "label": c.label, "location": c.location, "shortcut": c.shortcut, "params": c.params, "mutates": c.mutates});
                    if let (Some(s), Some(o)) = (s, v.as_object_mut()) {
                        o.insert("enabled".into(), Value::Bool((c.enabled)(s).is_none()));
                    }
                    v
                })
                .collect(),
        )
    }
    /// The command bound to a shortcut, if any.
    pub fn by_shortcut(&self, key: &str) -> Option<&CommandSpec> {
        self.specs.iter().find(|s| !s.shortcut.is_empty() && s.shortcut.split(" / ").any(|k| k.eq_ignore_ascii_case(key)))
    }
}

/// Parameter helpers.
pub mod p {
    use serde_json::Value;

    use crate::CmdError;

    pub fn str<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
        v.get(k).and_then(Value::as_str)
    }
    pub fn req_str<'a>(v: &'a Value, k: &str) -> Result<&'a str, CmdError> {
        str(v, k).ok_or_else(|| CmdError::Params(format!("`{k}` (string) is required")))
    }
    pub fn usize(v: &Value, k: &str) -> Option<usize> {
        v.get(k)
            .and_then(|x| x.as_u64().or_else(|| x.as_f64().filter(|f| *f >= 0.0 && f.is_finite()).map(|f| f as u64)))
            .map(|n| n.min(usize::MAX as u64) as usize)
    }
    pub fn req_usize(v: &Value, k: &str) -> Result<usize, CmdError> {
        usize(v, k).ok_or_else(|| CmdError::Params(format!("`{k}` (non-negative integer) is required")))
    }
    pub fn bool(v: &Value, k: &str) -> Option<bool> {
        v.get(k).and_then(Value::as_bool)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_annot;
#[cfg(test)]
mod tests_typing;
