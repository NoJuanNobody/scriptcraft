//! ScriptCraft real-time collaboration (#25): differential synchronisation with a central
//! sequencer. One window hosts the authoritative script; others join with a session key, send
//! patches against the version they last saw, and the host three-way merges them. Sync works on
//! the document, not on commands. Design: `docs/collaboration.md`.
//!
//! - [`diff`] / [`apply`]: paragraph patches between scripts.
//! - [`merge3`]: the three-way merge; [`transform_pos`] carries carets through a change.
//! - [`Host`] / [`Client`]: pure state machines (messages in, messages out).
//! - `net` (native only): the TCP transport.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod diff;
mod merge;
mod proto;
mod sync;

#[cfg(not(target_arch = "wasm32"))]
pub mod net;

pub use diff::{Hunk, Patch, PatchError, apply, diff, hunks};
pub use merge::{merge3, transform_pos};
pub use proto::{COLORS, MAX_LINE, PROTO, Peer, Sel, ToHost, ToPeer, clean_name, color_of, decode, encode};
pub use sync::{Client, Event, HISTORY, Host, MAX_PEERS, Out};

/// Compare keys in constant time (for equal lengths).
pub fn keys_match(given: &str, key: &str) -> bool {
    if key.is_empty() || given.len() != key.len() {
        return false;
    }
    given.bytes().zip(key.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

#[cfg(test)]
mod tests;
