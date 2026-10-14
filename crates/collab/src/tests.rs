#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use scriptcraft_doc::{Element, Flag, Para, Pos, Script};

use crate::*;

fn script(lines: &[(Element, &str)]) -> Script {
    Script::from_paras(lines.iter().map(|(e, t)| Para::new(*e, *t)).collect()).unwrap()
}

fn sample() -> Script {
    script(&[
        (Element::SceneHeading, "INT. RADIO ROOM - NIGHT"),
        (Element::Action, "Nell taps the glass."),
        (Element::Character, "NELL"),
        (Element::Dialogue, "Don't you dare."),
        (Element::Action, "The needle drops."),
        (Element::Action, "Static on the radio."),
    ])
}

fn texts(s: &Script) -> Vec<String> {
    s.paras.iter().map(|p| p.text.clone()).collect()
}

/// An in-memory session: one host, clients with their own scripts and inboxes.
struct Sim {
    host: Host,
    host_local: Script,
    host_in: Vec<(u32, ToHost)>,
    clients: Vec<(Client, Script, Vec<ToPeer>)>,
}

impl Sim {
    fn new(doc: Script, n: usize) -> Sim {
        let mut sim = Sim { host: Host::new(doc.clone(), "k3y", "Ana"), host_local: doc, host_in: Vec::new(), clients: Vec::new() };
        for i in 0..n {
            let id = i as u32 + 1;
            assert!(sim.host.opened(id).is_empty());
            let outs = sim.host.receive(id, ToHost::Hello { proto: PROTO, key: "k3y".into(), name: format!("P{id}") });
            let Some(Out::To(_, ToPeer::Welcome { id: wid, version, script })) = outs.first().cloned() else { panic!("{outs:?}") };
            assert_eq!(wid, id);
            sim.clients.push((Client::new(id, version, (*script).clone()), *script, Vec::new()));
            sim.route(outs.into_iter().skip(1).collect());
        }
        sim
    }
    fn route(&mut self, outs: Vec<Out>) {
        for o in outs {
            match o {
                Out::To(id, m) => {
                    if let Some(c) = self.clients.get_mut(id as usize - 1) {
                        c.2.push(m);
                    }
                }
                Out::All(m) => {
                    for c in &mut self.clients {
                        c.2.push(m.clone());
                    }
                }
                Out::Close(_) => {}
            }
        }
    }
    /// The host's own window: commit local edits.
    fn host_tick(&mut self) {
        let outs = self.host.local(&self.host_local);
        self.route(outs);
    }
    /// Deliver up to `n` queued messages to client `i`, then let it send.
    fn client_tick(&mut self, i: usize, n: usize) {
        let msgs: Vec<ToPeer> = {
            let c = &mut self.clients[i];
            let k = n.min(c.2.len());
            c.2.drain(..k).collect()
        };
        for m in msgs {
            let (client, local, _) = &mut self.clients[i];
            let (ev, reply) = client.receive(m, local);
            if let Event::Script(s) = ev {
                *local = *s;
            }
            if let Some(r) = reply {
                self.host_in.push((i as u32 + 1, r));
            }
        }
        let (client, local, _) = &mut self.clients[i];
        if let Some(m) = client.outgoing(local) {
            self.host_in.push((i as u32 + 1, m));
        }
    }
    /// The host handles up to `n` queued messages.
    fn host_receive(&mut self, n: usize) {
        let k = n.min(self.host_in.len());
        let msgs: Vec<(u32, ToHost)> = self.host_in.drain(..k).collect();
        for (id, m) in msgs {
            let before = self.host.doc().clone();
            let outs = self.host.receive(id, m);
            if *self.host.doc() != before {
                // The host window takes the merged document (like the engine does).
                self.host_local = merge3(&before, self.host.doc(), &self.host_local);
            }
            self.route(outs);
        }
    }
    fn settle(&mut self) {
        for _ in 0..200 {
            self.host_tick();
            self.host_receive(usize::MAX);
            for i in 0..self.clients.len() {
                self.client_tick(i, usize::MAX);
            }
            let quiet = self.host_in.is_empty() && self.clients.iter().all(|c| c.2.is_empty() && !c.0.waiting() && c.1 == *c.0.shadow());
            if quiet && self.host_local == *self.host.doc() {
                break;
            }
        }
        for (i, c) in self.clients.iter().enumerate() {
            assert_eq!(texts(&c.1), texts(self.host.doc()), "client {i} diverged");
            assert_eq!(c.1, *self.host.doc(), "client {i} diverged (fields)");
        }
        assert_eq!(self.host_local, *self.host.doc());
    }
    fn local(&mut self, i: usize) -> &mut Script {
        &mut self.clients[i].1
    }
}

#[test]
fn edits_in_different_paragraphs_both_land() {
    let mut sim = Sim::new(sample(), 2);
    sim.local(0).insert_text(Pos::new(1, 4), " quietly").unwrap();
    sim.local(1).insert_text(Pos::new(4, 0), "Slowly, ").unwrap();
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let t = texts(sim.host.doc());
    assert_eq!(t[1], "Nell quietly taps the glass.");
    assert_eq!(t[4], "Slowly, The needle drops.");
}

#[test]
fn typing_in_the_same_paragraph_keeps_both() {
    let mut sim = Sim::new(sample(), 2);
    sim.local(0).insert_text(Pos::new(1, 0), "Then ").unwrap();
    sim.local(1).insert_text(Pos::new(1, 19), " twice").unwrap();
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    assert_eq!(texts(sim.host.doc())[1], "Then Nell taps the glass twice.");
    // The same point: both kept, in the host's order.
    let mut sim = Sim::new(sample(), 2);
    sim.local(0).insert_text(Pos::new(3, 0), "AAA ").unwrap();
    sim.local(1).insert_text(Pos::new(3, 0), "BBB ").unwrap();
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let d = &texts(sim.host.doc())[3];
    assert!(d.contains("AAA ") && d.contains("BBB ") && d.ends_with("Don't you dare."), "{d}");
}

#[test]
fn split_and_join_against_typing() {
    // One splits a paragraph, the other types in its tail.
    let mut sim = Sim::new(sample(), 2);
    sim.local(0).split(Pos::new(5, 7), Element::Action).unwrap();
    sim.local(1).insert_text(Pos::new(5, 20), " Then nothing.").unwrap();
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let t = texts(sim.host.doc());
    assert_eq!(t[5], "Static ");
    assert_eq!(t[6], "on the radio. Then nothing.");
    // One joins two paragraphs, the other types in the second.
    let mut sim = Sim::new(sample(), 2);
    let end = sim.local(0).paras[4].text.len();
    sim.local(0).delete_range(Pos::new(4, end), Pos::new(5, 0));
    sim.local(1).insert_text(Pos::new(5, 6), " loud").unwrap();
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let t = texts(sim.host.doc());
    assert_eq!(t.len(), 5);
    assert_eq!(t[4], "The needle drops.Static loud on the radio.");
}

#[test]
fn overlapping_deletes_delete_once() {
    let mut sim = Sim::new(sample(), 2);
    // "Nell taps the glass." : one deletes "taps the", the other "the glass".
    sim.local(0).delete_range(Pos::new(1, 5), Pos::new(1, 14));
    sim.local(1).delete_range(Pos::new(1, 10), Pos::new(1, 19));
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    assert_eq!(texts(sim.host.doc())[1], "Nell .");
    // A delete over a paragraph break while the other deletes inside the second paragraph.
    let mut sim = Sim::new(sample(), 2);
    sim.local(0).delete_range(Pos::new(4, 10), Pos::new(5, 6));
    sim.local(1).delete_range(Pos::new(5, 0), Pos::new(5, 10));
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    // Diff alignment may keep either space; the words are deleted once.
    assert_eq!(texts(sim.host.doc())[4].replace(' ', ""), "Theneedletheradio.");
}

#[test]
fn deleting_a_paragraph_wins_over_an_edit_in_it_but_new_paragraphs_survive() {
    let mut sim = Sim::new(sample(), 2);
    sim.local(0).paras.remove(4);
    sim.local(1).insert_text(Pos::new(4, 0), "X ").unwrap();
    sim.local(1).paras.insert(6, Para::new(Element::Transition, "CUT TO:"));
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let t = texts(sim.host.doc());
    assert!(!t.iter().any(|x| x.contains("needle")), "{t:?}");
    assert!(t.contains(&"CUT TO:".to_string()));
}

#[test]
fn host_edits_and_element_changes_merge() {
    let mut sim = Sim::new(sample(), 1);
    sim.host_local.insert_text(Pos::new(1, 0), "Host: ").unwrap();
    sim.local(0).set_element(4, Element::Shot).unwrap();
    sim.local(0).insert_text(Pos::new(1, 20), " Again.").unwrap();
    sim.client_tick(0, 0);
    sim.host_tick();
    sim.settle();
    let d = sim.host.doc();
    assert_eq!(d.paras[1].text, "Host: Nell taps the glass. Again.");
    assert_eq!(d.paras[4].element, Element::Shot);
}

#[test]
fn styles_notes_and_settings_survive_merges() {
    let base = sample();
    let mut ours = base.clone();
    ours.set_flag(Pos::new(1, 0), Pos::new(1, 4), Flag::Bold, true);
    ours.add_note(Pos::new(1, 10), Pos::new(1, 19), "check", None, "Ana");
    ours.settings.show_scene_numbers = true;
    let mut theirs = base.clone();
    theirs.insert_text(Pos::new(1, 0), "At last ").unwrap();
    theirs.title.title = "NIGHT SIGNAL".into();
    let m = merge3(&base, &ours, &theirs);
    let p = &m.paras[1];
    assert_eq!(p.text, "At last Nell taps the glass.");
    assert!(m.all_have(Pos::new(1, 8), Pos::new(1, 12), Flag::Bold));
    assert!(!m.all_have(Pos::new(1, 0), Pos::new(1, 7), Flag::Bold));
    assert_eq!(p.slice(p.notes[0].start, p.notes[0].end), "the glass");
    assert!(m.settings.show_scene_numbers);
    assert_eq!(m.title.title, "NIGHT SIGNAL");
    // Merging with no change on one side is exact.
    assert_eq!(merge3(&base, &ours, &base), ours);
    assert_eq!(merge3(&base, &base, &theirs), theirs);
}

#[test]
fn carets_follow_remote_changes() {
    let old = sample();
    let mut new = old.clone();
    new.insert_text(Pos::new(1, 0), "Then ").unwrap();
    assert_eq!(transform_pos(&old, &new, Pos::new(1, 5)), Pos::new(1, 10));
    assert_eq!(transform_pos(&old, &new, Pos::new(3, 2)), Pos::new(3, 2));
    // A paragraph inserted before moves it down.
    let mut new = old.clone();
    new.paras.insert(0, Para::new(Element::Action, "FADE IN:"));
    assert_eq!(transform_pos(&old, &new, Pos::new(2, 3)), Pos::new(3, 3));
    // A split before the caret carries it into the new paragraph.
    let mut new = old.clone();
    new.split(Pos::new(1, 5), Element::Action).unwrap();
    assert_eq!(transform_pos(&old, &new, Pos::new(1, 10)), Pos::new(2, 5));
    // Its paragraph deleted: the caret lands where it was.
    let mut new = old.clone();
    new.paras.remove(3);
    assert_eq!(transform_pos(&old, &new, Pos::new(3, 4)), Pos::new(3, 0));
    assert_eq!(transform_pos(&old, &new, Pos::new(99, 99)), new.clamp(Pos::new(99, 99)));
}

/// A tiny deterministic generator.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn random_edit(s: &mut Script, r: &mut Rng, tag: &str) {
    let p = r.below(s.paras.len());
    let len = s.paras[p].text.len();
    let mut off = r.below(len + 1);
    while !s.paras[p].text.is_char_boundary(off) {
        off -= 1;
    }
    match r.below(6) {
        0 | 1 => {
            s.insert_text(Pos::new(p, off), tag).unwrap();
        }
        2 => {
            let _ = s.split(Pos::new(p, off), Element::Action);
        }
        3 => {
            let q = (p + r.below(2)).min(s.paras.len() - 1);
            let e = s.clamp(Pos::new(q, r.below(30)));
            s.delete_range(Pos::new(p, off), e);
        }
        4 => {
            let _ = s.set_element(p, Element::ALL[r.below(Element::ALL.len())]);
        }
        _ => {
            if s.paras.len() > 2 {
                s.paras.remove(p);
            }
        }
    }
    let _ = s.normalize();
}

#[test]
fn three_writers_converge_under_random_interleaving() {
    for seed in 1..120u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut sim = Sim::new(sample(), 3);
        for step in 0..150 {
            match r.below(6) {
                0 => {
                    let tag = format!("h{step} ");
                    random_edit(&mut sim.host_local, &mut r, &tag);
                    sim.host_tick();
                }
                1 | 2 => {
                    let i = r.below(3);
                    let tag = format!("c{i}s{step}é ");
                    random_edit(sim.local(i), &mut r, &tag);
                }
                3 => {
                    let n = r.below(3);
                    sim.host_receive(n);
                }
                _ => {
                    let i = r.below(3);
                    let n = r.below(3);
                    sim.client_tick(i, n);
                }
            }
        }
        sim.settle();
    }
}

#[test]
fn old_versions_resync() {
    let mut sim = Sim::new(sample(), 1);
    sim.local(0).insert_text(Pos::new(2, 0), "OLD ").unwrap();
    // The host moves on far beyond its history before the client hears anything.
    for k in 0..(HISTORY + 5) {
        sim.host_local.insert_text(Pos::new(4, 0), &format!("{k}")).unwrap();
        sim.host_tick();
    }
    sim.client_tick(0, 0);
    sim.settle();
    assert!(sim.host.doc().paras[2].text.starts_with("OLD "));
}

#[test]
fn joining_needs_the_key_and_garbage_never_panics() {
    let mut h = Host::new(sample(), "secret", "Ana");
    h.opened(7);
    let outs = h.receive(7, ToHost::Hello { proto: PROTO, key: "nope".into(), name: "x".into() });
    assert!(matches!(outs.first(), Some(Out::To(7, ToPeer::Refused { .. }))));
    assert!(h.joined().is_empty());
    h.opened(8);
    let outs = h.receive(8, ToHost::Edit { base: 0, patch: Patch::default() });
    assert_eq!(outs, vec![Out::Close(8)]);
    h.opened(9);
    h.receive(9, ToHost::Hello { proto: PROTO + 1, key: "secret".into(), name: "x".into() });
    assert!(h.joined().is_empty());
    h.opened(10);
    h.receive(10, ToHost::Hello { proto: PROTO, key: "secret".into(), name: "\u{0}  Bo\nb  ".into() });
    assert_eq!(h.joined(), vec![10]);
    assert_eq!(h.peers()[1].name, "Bob");
    // Hostile patches resync instead of applying.
    let bad = Patch { hunks: vec![Hunk { at: 99, del: 5, ins: vec![] }], ..Patch::default() };
    assert!(matches!(h.receive(10, ToHost::Edit { base: 0, patch: bad }).first(), Some(Out::To(10, ToPeer::Resync { .. }))));
    let bad = Patch { hunks: vec![Hunk { at: 3, del: usize::MAX, ins: vec![] }], ..Patch::default() };
    assert!(apply(&sample(), &bad).is_err());
    let bad = Patch { hunks: vec![Hunk { at: 3, del: 0, ins: vec![] }, Hunk { at: 1, del: 0, ins: vec![] }], ..Patch::default() };
    assert!(apply(&sample(), &bad).is_err());
    assert!(h.receive(10, ToHost::Edit { base: 12345, patch: Patch::default() }).iter().any(|o| matches!(o, Out::To(10, ToPeer::Resync { .. }))));
    // Garbled lines decode to nothing.
    for g in [&b"{"[..], b"null", b"{\"t\":\"edit\",\"base\":-1}", b"\xff\xfe", b"{\"t\":\"hello\",\"proto\":1}"] {
        assert!(decode::<ToHost>(g).is_none());
        assert!(decode::<ToPeer>(g).is_none());
    }
    // A client given nonsense stays put and asks for a resync once.
    let mut c = Client::new(1, 5, sample());
    let (ev, r) = c.receive(ToPeer::Update { version: 99, origin: 3, patch: Patch::default() }, &sample());
    assert_eq!((ev, r), (Event::None, Some(ToHost::NeedSync)));
    let (_, r) = c.receive(ToPeer::Update { version: 99, origin: 3, patch: Patch::default() }, &sample());
    assert_eq!(r, None);
    assert!(keys_match("ab", "ab") && !keys_match("ab", "ac") && !keys_match("", "") && !keys_match("a", "ab"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn two_sessions_over_loopback() {
    use std::time::{Duration, Instant};
    let srv = net::Server::bind(0, false).unwrap();
    let mut host = Host::new(sample(), "k", "Ana");
    let mut host_local = sample();
    let mut links: Vec<(net::Link, Option<Client>, Script)> = (0..2)
        .map(|i| {
            let mut l = net::Link::connect(&format!("127.0.0.1:{}", srv.port()), Duration::from_secs(2)).unwrap();
            assert!(l.send(&ToHost::Hello { proto: PROTO, key: "k".into(), name: format!("G{i}") }));
            (l, None, Script::new())
        })
        .collect();
    let route = |srv: &net::Server, host: &Host, outs: Vec<Out>| {
        for o in outs {
            match o {
                Out::To(id, m) => srv.send(id, &m),
                Out::All(m) => srv.send_all(&host.joined(), &m),
                Out::Close(id) => srv.close(id),
            }
        }
    };
    let mut edited = false;
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10), "no convergence");
        for ev in srv.poll() {
            let outs = match ev {
                net::NetEvent::Opened(id) => host.opened(id),
                net::NetEvent::Msg(id, m) => {
                    let before = host.doc().clone();
                    let o = host.receive(id, m);
                    host_local = merge3(&before, host.doc(), &host_local);
                    o
                }
                net::NetEvent::Closed(id) => host.closed(id),
            };
            route(&srv, &host, outs);
        }
        for (l, c, local) in &mut links {
            for m in l.poll() {
                let Some(m) = m else { panic!("closed") };
                if let ToPeer::Welcome { id, version, script } = m {
                    *c = Some(Client::new(id, version, (*script).clone()));
                    *local = *script;
                    continue;
                }
                if let Some(c) = c {
                    let (ev, r) = c.receive(m, local);
                    if let Event::Script(s) = ev {
                        *local = *s;
                    }
                    if let Some(r) = r {
                        l.send(&r);
                    }
                }
            }
            if let Some(c) = c
                && let Some(m) = c.outgoing(local)
            {
                l.send(&m);
            }
        }
        if !edited && links.iter().all(|l| l.1.is_some()) {
            links[0].2.insert_text(Pos::new(1, 0), "One ").unwrap();
            links[1].2.insert_text(Pos::new(3, 0), "Two ").unwrap();
            host_local.insert_text(Pos::new(5, 0), "Host ").unwrap();
            edited = true;
        }
        let outs = host.local(&host_local);
        route(&srv, &host, outs);
        if edited && links.iter().all(|(_, c, local)| c.as_ref().is_some_and(|c| !c.waiting()) && local == host.doc()) && host_local == *host.doc() {
            let t = texts(host.doc());
            if t[1].starts_with("One ") && t[3].starts_with("Two ") && t[5].starts_with("Host ") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // A stranger with a garbage line and no key is dropped, and nothing panics.
    let mut l = net::Link::connect(&format!("127.0.0.1:{}", srv.port()), Duration::from_secs(2)).unwrap();
    l.send(&ToHost::Bye);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut closed = false;
    while Instant::now() < deadline && !closed {
        for ev in srv.poll() {
            let outs = match ev {
                net::NetEvent::Opened(id) => host.opened(id),
                net::NetEvent::Msg(id, m) => host.receive(id, m),
                net::NetEvent::Closed(id) => host.closed(id),
            };
            route(&srv, &host, outs);
        }
        closed = l.poll().iter().any(Option::is_none);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(closed, "the stranger was not disconnected");
    assert_eq!(host.joined().len(), 2);
}

#[test]
fn structure_bookmarks_and_beats_sync_while_the_other_types() {
    use scriptcraft_doc::{StructureKind, StructurePoint};
    let mut sim = Sim::new(sample(), 2);
    // Peer 0 adds an act, a bookmark, a beat linked to the scene and a point after the end.
    {
        let s = sim.local(0);
        s.add_structure(StructurePoint::new(StructureKind::Act, "Act One", "", None), Some(0));
        s.add_structure(StructurePoint::new(StructureKind::Act, "Act Two", "", None), None);
        s.set_bookmark(3, Some("Dare")).unwrap();
        let id = s.board.add("Storm", "It hits.", None, 10.0, 10.0).unwrap();
        s.link_beat(id, Some(0)).unwrap();
    }
    // Peer 1 types in the same heading and the bookmarked line, and adds its own beat.
    sim.local(1).insert_text(Pos::new(0, 0), "X").unwrap();
    sim.local(1).insert_text(Pos::new(3, 0), "Oh, ").unwrap();
    sim.local(1).board.add("Calm", "", None, 300.0, 10.0).unwrap();
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let d = sim.host.doc();
    assert_eq!(d.paras[0].structure.len(), 1);
    assert_eq!(d.structure_tail.len(), 1);
    assert_eq!(d.paras[3].bookmark.as_deref(), Some("Dare"));
    assert!(d.paras[3].text.starts_with("Oh, "));
    // Both beats kept (the same new id on both sides: one was renumbered), the link intact.
    let titles: Vec<&str> = d.board.beats.iter().map(|b| b.title.as_str()).collect();
    assert_eq!(titles.len(), 2, "{titles:?}");
    assert!(titles.contains(&"Storm") && titles.contains(&"Calm"));
    let storm = d.board.beats.iter().find(|b| b.title == "Storm").unwrap().id;
    assert_eq!(d.beat_scene(storm), Some(0));
}

#[test]
fn tracked_changes_sync_with_their_authors() {
    use scriptcraft_doc::{ChangeKind, track};
    let mut sim = Sim::new(sample(), 2);
    // Each peer records its own edit as a tracked change (as the engine does with track on).
    for (i, (pos, text, who)) in [(Pos::new(1, 0), "Then ", "Ana"), (Pos::new(4, 0), "Slowly, ", "Bo")].into_iter().enumerate() {
        let old = sim.local(i).clone();
        sim.local(i).insert_text(pos, text).unwrap();
        let mut new = sim.local(i).clone();
        track::follow(&old, &mut new, Some(who));
        *sim.local(i) = new;
    }
    // And peer 0 deletes a word in a paragraph nobody else touches.
    let old = sim.local(0).clone();
    sim.local(0).delete_range(Pos::new(5, 0), Pos::new(5, 7));
    let mut new = sim.local(0).clone();
    track::follow(&old, &mut new, Some("Ana"));
    *sim.local(0) = new;
    sim.client_tick(0, 0);
    sim.client_tick(1, 0);
    sim.settle();
    let d = sim.host.doc();
    let all: Vec<(usize, ChangeKind, String)> =
        d.paras.iter().enumerate().flat_map(|(i, p)| p.changes.iter().map(move |c| (i, c.kind, c.author.clone()))).collect();
    assert!(all.contains(&(1, ChangeKind::Insert, "Ana".into())), "{all:?}");
    assert!(all.contains(&(4, ChangeKind::Insert, "Bo".into())), "{all:?}");
    assert!(all.contains(&(5, ChangeKind::Delete, "Ana".into())), "{all:?}");
}
