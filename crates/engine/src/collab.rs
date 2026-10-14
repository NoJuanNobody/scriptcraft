//! Real-time collaboration in a session (#25): hosting or joining, applied each frame by
//! [`Session::collab_tick`]. The merge and protocol live in `scriptcraft-collab`; this module
//! connects them to the session (remote changes, carets, undo history). `docs/collaboration.md`.

use scriptcraft_collab::{Peer, Sel};
use scriptcraft_doc::Script;
use serde_json::{Value, json};

use crate::{Selection, Session};

/// Undo steps kept while a session is shared (each remote change rebases them).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
const SHARED_UNDO: usize = 100;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum Role {
    Host { host: Box<scriptcraft_collab::Host>, srv: scriptcraft_collab::net::Server },
    Client { client: Box<scriptcraft_collab::Client>, link: scriptcraft_collab::net::Link },
}

/// The session's collaboration state.
#[derive(Default)]
pub struct Collab {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) role: Option<Role>,
    /// Why the last session ended (shown once).
    pub ended: Option<String>,
}

impl Collab {
    /// Hosting or joined.
    pub fn active(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.role.is_some();
        #[cfg(target_arch = "wasm32")]
        false
    }

    /// Everyone in the session (empty when off).
    pub fn peers(&self) -> Vec<Peer> {
        #[cfg(not(target_arch = "wasm32"))]
        match &self.role {
            Some(Role::Host { host, .. }) => return host.peers(),
            Some(Role::Client { client, .. }) => return client.peers().to_vec(),
            None => {}
        }
        Vec::new()
    }

    /// This window's id among the peers (the host is 0).
    pub fn my_id(&self) -> Option<u32> {
        #[cfg(not(target_arch = "wasm32"))]
        match &self.role {
            Some(Role::Host { .. }) => return Some(0),
            Some(Role::Client { client, .. }) => return Some(client.id()),
            None => {}
        }
        None
    }

    /// The other people's selections, with their names and colours.
    pub fn others(&self) -> Vec<Peer> {
        let me = self.my_id();
        self.peers().into_iter().filter(|p| Some(p.id) != me).collect()
    }

    /// `collab.status` as JSON.
    pub fn status(&self) -> Value {
        #[cfg(not(target_arch = "wasm32"))]
        match &self.role {
            Some(Role::Host { host, srv }) => {
                let mut addrs = vec![format!("127.0.0.1:{}", srv.port())];
                if srv.lan()
                    && let Some(ip) = scriptcraft_collab::net::lan_ip()
                {
                    addrs.insert(0, format!("{ip}:{}", srv.port()));
                }
                return json!({"role": "host", "port": srv.port(), "lan": srv.lan(), "addresses": addrs, "key": host.key(),
                    "version": host.version(), "peers": host.peers().len()});
            }
            Some(Role::Client { client, link }) => {
                return json!({"role": "client", "host": link.addr(), "id": client.id(), "version": client.version(),
                    "waiting": client.waiting(), "peers": client.peers().len()});
            }
            None => {}
        }
        json!({"role": "off", "ended": self.ended, "available": cfg!(not(target_arch = "wasm32"))})
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn sel_of(s: &Selection) -> Sel {
    Sel { anchor: s.anchor, focus: s.focus }
}

impl Session {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    /// Replace the script with a merged one from the session: carets follow, undo snapshots
    /// get the change too (so undo only undoes your own edits).
    pub(crate) fn apply_remote(&mut self, new: Script) {
        if new == self.script {
            return;
        }
        let old = std::mem::replace(&mut self.script, new);
        let t = |p| scriptcraft_collab::transform_pos(&old, &self.script, p);
        self.sel = Selection { anchor: t(self.sel.anchor), focus: t(self.sel.focus) };
        let new = self.script.clone();
        self.rebase_history(SHARED_UNDO, &mut |snap| scriptcraft_collab::merge3(&old, &new, snap));
        self.invalidate();
        self.dirty = true;
        self.clamp_selection();
    }

    /// Exchange changes with the session (call every frame or so while [`Collab::active`]).
    /// Returns whether the script changed.
    pub fn collab_tick(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use scriptcraft_collab::net::NetEvent;
            use scriptcraft_collab::{Event, Out};
            let Some(role) = self.collab.role.take() else { return false };
            let before = self.revision();
            let sel = sel_of(&self.sel);
            let role = match role {
                Role::Host { mut host, srv } => {
                    let route = |host: &scriptcraft_collab::Host, outs: Vec<Out>| {
                        for o in outs {
                            match o {
                                Out::To(id, m) => srv.send(id, &m),
                                Out::All(m) => srv.send_all(&host.joined(), &m),
                                Out::Close(id) => srv.close(id),
                            }
                        }
                    };
                    let outs = host.local(&self.script);
                    route(&host, outs);
                    let outs = host.local_sel(sel);
                    route(&host, outs);
                    for ev in srv.poll() {
                        let outs = match ev {
                            NetEvent::Opened(id) => host.opened(id),
                            NetEvent::Closed(id) => host.closed(id),
                            NetEvent::Msg(id, m) => {
                                let outs = host.receive(id, m);
                                if *host.doc() != self.script {
                                    // The host window has no unsent edits (committed above).
                                    self.apply_remote(host.doc().clone());
                                }
                                outs
                            }
                        };
                        route(&host, outs);
                    }
                    Some(Role::Host { host, srv })
                }
                Role::Client { mut client, mut link } => {
                    let mut ended = None;
                    for m in link.poll() {
                        let Some(m) = m else {
                            ended = Some("the host closed the session".to_string());
                            break;
                        };
                        let (ev, reply) = client.receive(m, &self.script);
                        if let Some(r) = reply {
                            link.send(&r);
                        }
                        match ev {
                            Event::Script(s) => self.apply_remote(*s),
                            Event::Ended(why) => {
                                ended = Some(why);
                                break;
                            }
                            Event::None => {}
                        }
                    }
                    if let Some(why) = ended {
                        self.status = format!("Collaboration ended: {why}");
                        self.collab.ended = Some(why);
                        None
                    } else {
                        if let Some(m) = client.outgoing(&self.script)
                            && !link.send(&m)
                        {
                            self.collab.ended = Some("lost the connection to the host".into());
                            self.status = "Collaboration ended: lost the connection to the host".into();
                            return before != self.revision();
                        }
                        if let Some(m) = client.presence(sel_of(&self.sel)) {
                            link.send(&m);
                        }
                        Some(Role::Client { client, link })
                    }
                }
            };
            self.collab.role = role;
            before != self.revision()
        }
        #[cfg(target_arch = "wasm32")]
        false
    }
}

/// The JSON form of the peers list (for `collab.peers` and the window).
pub fn peers_json(c: &Collab) -> Value {
    let me = c.my_id();
    let peers: Vec<Value> = c
        .peers()
        .iter()
        .map(|p| {
            let [r, g, b] = scriptcraft_collab::COLORS.get(usize::from(p.color)).copied().unwrap_or([0, 0, 0]);
            json!({"id": p.id, "name": p.name, "host": p.host, "you": Some(p.id) == me, "color": format!("#{r:02x}{g:02x}{b:02x}"),
                "caret": p.sel.map(|s| crate::cmd::pos_json(s.focus)), "anchor": p.sel.map(|s| crate::cmd::pos_json(s.anchor))})
        })
        .collect();
    json!({"peers": peers})
}
