//! The host and client state machines: pure (messages in, messages out), no sockets.

use std::collections::{BTreeMap, VecDeque};

use scriptcraft_doc::Script;

use crate::diff::{apply, diff};
use crate::merge::merge3;
use crate::proto::{PROTO, Peer, Sel, ToHost, ToPeer, clean_name, color_of};

/// Versions the host keeps for edits made on older ones.
pub const HISTORY: usize = 64;
/// Most people in one session.
pub const MAX_PEERS: usize = 32;

/// Where a host message goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    To(u32, ToPeer),
    /// Every joined peer.
    All(ToPeer),
    Close(u32),
}

struct Conn {
    joined: bool,
    name: String,
    sel: Option<Sel>,
}

/// The host: the authoritative document and its version.
pub struct Host {
    doc: Script,
    version: u64,
    history: VecDeque<(u64, Script)>,
    conns: BTreeMap<u32, Conn>,
    key: String,
    name: String,
    sel: Option<Sel>,
}

impl Host {
    pub fn new(doc: Script, key: &str, name: &str) -> Self {
        let mut history = VecDeque::new();
        history.push_back((0, doc.clone()));
        Host { doc, version: 0, history, conns: BTreeMap::new(), key: key.to_string(), name: clean_name(name, "Host"), sel: None }
    }

    pub fn doc(&self) -> &Script {
        &self.doc
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    /// Joined peers' connection ids.
    pub fn joined(&self) -> Vec<u32> {
        self.conns.iter().filter(|(_, c)| c.joined).map(|(id, _)| *id).collect()
    }

    /// Everyone, the host first.
    pub fn peers(&self) -> Vec<Peer> {
        let mut v = vec![Peer { id: 0, name: self.name.clone(), color: color_of(0), sel: self.sel, host: true }];
        v.extend(self.conns.iter().filter(|(_, c)| c.joined).map(|(id, c)| Peer {
            id: *id,
            name: c.name.clone(),
            color: color_of(*id),
            sel: c.sel,
            host: false,
        }));
        v
    }

    /// A connection opened (`id` > 0, unique).
    pub fn opened(&mut self, id: u32) -> Vec<Out> {
        if id == 0 || self.conns.len() >= MAX_PEERS * 2 {
            return vec![Out::Close(id)];
        }
        self.conns.insert(id, Conn { joined: false, name: String::new(), sel: None });
        Vec::new()
    }

    pub fn closed(&mut self, id: u32) -> Vec<Out> {
        match self.conns.remove(&id) {
            Some(c) if c.joined => vec![Out::All(ToPeer::Peers { peers: self.peers() })],
            _ => Vec::new(),
        }
    }

    fn commit(&mut self, next: Script, origin: u32) -> Vec<Out> {
        let patch = diff(&self.doc, &next);
        self.doc = next;
        self.version += 1;
        self.history.push_back((self.version, self.doc.clone()));
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
        vec![Out::All(ToPeer::Update { version: self.version, origin, patch })]
    }

    /// The host's own window changed the script.
    pub fn local(&mut self, script: &Script) -> Vec<Out> {
        if *script == self.doc {
            return Vec::new();
        }
        self.commit(script.clone(), 0)
    }

    /// The host's own selection.
    pub fn local_sel(&mut self, sel: Sel) -> Vec<Out> {
        if self.sel == Some(sel) {
            return Vec::new();
        }
        self.sel = Some(sel);
        vec![Out::All(ToPeer::Peers { peers: self.peers() })]
    }

    fn resync(&self, id: u32) -> Out {
        Out::To(id, ToPeer::Resync { version: self.version, script: Box::new(self.doc.clone()) })
    }

    /// A message from connection `id`.
    pub fn receive(&mut self, id: u32, msg: ToHost) -> Vec<Out> {
        let Some(joined) = self.conns.get(&id).map(|c| c.joined) else { return vec![Out::Close(id)] };
        let full = self.joined().len() >= MAX_PEERS;
        if !joined {
            return match msg {
                ToHost::Hello { proto, key, name } => {
                    let reason = if proto != PROTO {
                        Some("this ScriptCraft speaks a different collaboration protocol")
                    } else if !crate::keys_match(&key, &self.key) {
                        Some("wrong session key")
                    } else if full {
                        Some("the session is full")
                    } else {
                        None
                    };
                    if let Some(r) = reason {
                        self.conns.remove(&id);
                        return vec![Out::To(id, ToPeer::Refused { reason: r.into() }), Out::Close(id)];
                    }
                    if let Some(c) = self.conns.get_mut(&id) {
                        c.joined = true;
                        c.name = clean_name(&name, &format!("Guest {id}"));
                    }
                    vec![
                        Out::To(id, ToPeer::Welcome { id, version: self.version, script: Box::new(self.doc.clone()) }),
                        Out::All(ToPeer::Peers { peers: self.peers() }),
                    ]
                }
                // Anything before a correct Hello closes the connection.
                _ => {
                    self.conns.remove(&id);
                    vec![Out::Close(id)]
                }
            };
        }
        match msg {
            ToHost::Hello { .. } => Vec::new(),
            ToHost::Presence { sel } => {
                if let Some(c) = self.conns.get_mut(&id) {
                    c.sel = Some(sel);
                }
                vec![Out::All(ToPeer::Peers { peers: self.peers() })]
            }
            ToHost::NeedSync => vec![self.resync(id)],
            ToHost::Bye => {
                self.conns.remove(&id);
                vec![Out::Close(id), Out::All(ToPeer::Peers { peers: self.peers() })]
            }
            ToHost::Edit { base, patch } => {
                let Some(b) = self.history.iter().find(|(v, _)| *v == base).map(|(_, s)| s.clone()) else {
                    return vec![self.resync(id)];
                };
                let Ok(theirs) = apply(&b, &patch) else { return vec![self.resync(id)] };
                let merged = merge3(&b, &self.doc, &theirs);
                // Always a new version, so the sender gets its acknowledgement.
                self.commit(merged, id)
            }
        }
    }
}

/// What a message did to the client's script.
#[derive(Debug, PartialEq)]
pub enum Event {
    None,
    /// Replace the local script with this one.
    Script(Box<Script>),
    /// The host ended the session or refused us.
    Ended(String),
}

/// A client: the host's document as last received (the shadow) and our unacknowledged edit.
pub struct Client {
    id: u32,
    version: u64,
    shadow: Script,
    /// Our script when we sent the edit in flight (rebased as host updates arrive).
    pending: Option<Script>,
    peers: Vec<Peer>,
    sel_sent: Option<Sel>,
    sync_asked: bool,
}

impl Client {
    /// After the host's welcome.
    pub fn new(id: u32, version: u64, mut script: Script) -> Self {
        let _ = script.normalize();
        Client { id, version, shadow: script, pending: None, peers: Vec::new(), sel_sent: None, sync_asked: false }
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn shadow(&self) -> &Script {
        &self.shadow
    }
    pub fn peers(&self) -> &[Peer] {
        &self.peers
    }
    /// An edit is waiting for the host.
    pub fn waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// Handle a host message given our current script.
    pub fn receive(&mut self, msg: ToPeer, local: &Script) -> (Event, Option<ToHost>) {
        match msg {
            ToPeer::Update { version, origin, patch } => {
                if version != self.version + 1 {
                    return (Event::None, self.ask_sync());
                }
                let Ok(next) = apply(&self.shadow, &patch) else { return (Event::None, self.ask_sync()) };
                let new_local = if origin == self.id {
                    let base = self.pending.take().unwrap_or_else(|| self.shadow.clone());
                    if *local == base { next.clone() } else { merge3(&base, &next, local) }
                } else {
                    self.pending = self.pending.take().map(|p| merge3(&self.shadow, &next, &p));
                    if *local == self.shadow { next.clone() } else { merge3(&self.shadow, &next, local) }
                };
                self.shadow = next;
                self.version = version;
                (if new_local == *local { Event::None } else { Event::Script(Box::new(new_local)) }, None)
            }
            ToPeer::Resync { version, script } => {
                let mut next = *script;
                let _ = next.normalize();
                let new_local = if *local == self.shadow { next.clone() } else { merge3(&self.shadow, &next, local) };
                self.shadow = next;
                self.version = version;
                // Whatever was in flight was not applied; it is in `local` and goes out again.
                self.pending = None;
                self.sync_asked = false;
                (if new_local == *local { Event::None } else { Event::Script(Box::new(new_local)) }, None)
            }
            ToPeer::Peers { peers } => {
                self.peers = peers.into_iter().take(MAX_PEERS + 1).collect();
                (Event::None, None)
            }
            ToPeer::Bye { reason } | ToPeer::Refused { reason } => (Event::Ended(reason), None),
            ToPeer::Welcome { .. } => (Event::None, None),
        }
    }

    fn ask_sync(&mut self) -> Option<ToHost> {
        if self.sync_asked {
            return None;
        }
        self.sync_asked = true;
        Some(ToHost::NeedSync)
    }

    /// The edit to send now, if our script differs from the host's and nothing is in flight.
    pub fn outgoing(&mut self, local: &Script) -> Option<ToHost> {
        if self.pending.is_some() || self.sync_asked || *local == self.shadow {
            return None;
        }
        self.pending = Some(local.clone());
        Some(ToHost::Edit { base: self.version, patch: diff(&self.shadow, local) })
    }

    /// Our selection, when it changed since we last sent it.
    pub fn presence(&mut self, sel: Sel) -> Option<ToHost> {
        if self.sel_sent == Some(sel) {
            return None;
        }
        self.sel_sent = Some(sel);
        Some(ToHost::Presence { sel })
    }
}
