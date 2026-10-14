//! Wire messages: one JSON object per line.

use scriptcraft_doc::{Pos, Script};
use serde::{Deserialize, Serialize};

use crate::diff::Patch;

/// Protocol version; a host refuses other versions.
pub const PROTO: u32 = 1;
/// Longest line accepted (a whole script fits comfortably).
pub const MAX_LINE: usize = 32 << 20;
/// Longest display name.
pub const MAX_NAME: usize = 40;

/// A selection as peers see it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sel {
    pub anchor: Pos,
    pub focus: Pos,
}

/// Someone in the session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub id: u32,
    pub name: String,
    /// Index into [`COLORS`].
    pub color: u8,
    #[serde(default)]
    pub sel: Option<Sel>,
    /// The host.
    #[serde(default)]
    pub host: bool,
}

/// Presence colours (RGB), by peer id.
pub const COLORS: [[u8; 3]; 8] = [
    [0x3b, 0x5b, 0xdb],
    [0xe0, 0x31, 0x31],
    [0x2b, 0x8a, 0x3e],
    [0xe8, 0x59, 0x0c],
    [0x9c, 0x36, 0xb5],
    [0x0c, 0x85, 0x99],
    [0xc2, 0x25, 0x5c],
    [0x86, 0x6a, 0x10],
];

pub fn color_of(id: u32) -> u8 {
    (id % COLORS.len() as u32) as u8
}

/// A display name: trimmed, one line, at most [`MAX_NAME`] characters.
pub fn clean_name(s: &str, fallback: &str) -> String {
    let n: String = s.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect::<String>().trim().to_string();
    if n.is_empty() { fallback.to_string() } else { n }
}

/// Client → host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ToHost {
    Hello {
        proto: u32,
        key: String,
        name: String,
    },
    /// The client's script is `version` + `patch`.
    Edit {
        base: u64,
        patch: Patch,
    },
    Presence {
        sel: Sel,
    },
    /// The client lost track (send the whole document).
    NeedSync,
    Bye,
}

/// Host → client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ToPeer {
    Welcome {
        id: u32,
        version: u64,
        script: Box<Script>,
    },
    Refused {
        reason: String,
    },
    /// Version `version` is version `version - 1` + `patch`; `origin` made it (0 = the host).
    Update {
        version: u64,
        origin: u32,
        patch: Patch,
    },
    /// The whole document (after an edit on a version the host no longer has).
    Resync {
        version: u64,
        script: Box<Script>,
    },
    Peers {
        peers: Vec<Peer>,
    },
    Bye {
        reason: String,
    },
}

pub fn encode<T: Serialize>(m: &T) -> String {
    serde_json::to_string(m).unwrap_or_default()
}

/// A message from one line; `None` for anything malformed.
pub fn decode<T: for<'de> Deserialize<'de>>(line: &[u8]) -> Option<T> {
    if line.len() > MAX_LINE {
        return None;
    }
    serde_json::from_slice(line).ok()
}
