//! Collaboration through real sessions and commands over 127.0.0.1 (#25).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::Session;

type Job = Box<dyn FnOnce(&mut Session) -> Value + Send>;

/// A hosting session ticking on its own thread (like another window).
struct HostThread {
    tx: Sender<(Job, Sender<Value>)>,
    handle: Option<std::thread::JoinHandle<()>>,
    addr: String,
    key: String,
}

impl HostThread {
    fn start() -> HostThread {
        let (tx, rx) = channel::<(Job, Sender<Value>)>();
        let (info_tx, info_rx) = channel();
        let handle = std::thread::spawn(move || {
            let mut s = Session::sample();
            let st = s.run("collab.host", &json!({"name": "Ana"})).unwrap();
            info_tx.send(st).unwrap();
            loop {
                s.collab_tick();
                match rx.recv_timeout(Duration::from_millis(2)) {
                    Ok((job, reply)) => {
                        let v = job(&mut s);
                        let _ = reply.send(v);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => return,
                }
            }
        });
        let st = info_rx.recv().unwrap();
        HostThread { tx, handle: Some(handle), addr: format!("127.0.0.1:{}", st["port"]), key: st["key"].as_str().unwrap().to_string() }
    }
    fn with(&self, f: impl FnOnce(&mut Session) -> Value + Send + 'static) -> Value {
        let (rtx, rrx) = channel();
        self.tx.send((Box::new(f), rtx)).unwrap();
        rrx.recv().unwrap()
    }
    fn texts(&self) -> Vec<String> {
        let v = self.with(|s| json!(s.script.paras.iter().map(|p| p.text.clone()).collect::<Vec<_>>()));
        serde_json::from_value(v).unwrap()
    }
}

impl Drop for HostThread {
    fn drop(&mut self) {
        let (tx, _) = channel();
        drop(std::mem::replace(&mut self.tx, tx));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn texts(s: &Session) -> Vec<String> {
    s.script.paras.iter().map(|p| p.text.clone()).collect()
}

/// Tick the clients until they and the host agree (and nothing is in flight).
fn converge(h: &HostThread, cs: &mut [&mut Session]) -> Vec<String> {
    let start = Instant::now();
    loop {
        for c in cs.iter_mut() {
            c.collab_tick();
        }
        let ht = h.texts();
        let idle = cs.iter().all(|c| c.collab.status()["waiting"] == json!(false));
        if idle && cs.iter().all(|c| texts(c) == ht) {
            // One more round: nothing left to send.
            std::thread::sleep(Duration::from_millis(20));
            for c in cs.iter_mut() {
                c.collab_tick();
            }
            if cs.iter().all(|c| texts(c) == h.texts()) {
                return ht;
            }
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no convergence: host {ht:?}");
        std::thread::sleep(Duration::from_millis(3));
    }
}

fn join(h: &HostThread, name: &str) -> Session {
    let mut s = Session::new(scriptcraft_doc::Script::new());
    let v = s.run("collab.join", &json!({"addr": h.addr, "key": h.key, "name": name})).unwrap();
    assert_eq!(v["role"], "client");
    s
}

#[test]
fn three_windows_edit_live_and_converge() {
    let h = HostThread::start();
    let mut a = join(&h, "Bea");
    let mut b = join(&h, "Cy");
    assert_eq!(texts(&a), h.texts());
    // Bad key and a busy session are refused.
    let mut x = Session::new(scriptcraft_doc::Script::new());
    let e = x.run("collab.join", &json!({"addr": h.addr, "key": "nope", "name": "X"})).unwrap_err();
    assert!(e.to_string().contains("wrong session key"), "{e}");
    assert!(a.run("collab.host", &json!({})).is_err());

    // Everyone types at once: different paragraphs, the same paragraph, a split.
    a.run("caret.set", &json!({"para": 2, "off": 0})).unwrap();
    a.run("text.insert", &json!({"text": "Bea was here. "})).unwrap();
    b.run("caret.set", &json!({"para": 2, "off": 5})).unwrap();
    b.run("text.insert", &json!({"text": "[Cy]"})).unwrap();
    b.run("caret.set", &json!({"para": 4, "off": 8})).unwrap();
    b.run("text.enter", &json!({})).unwrap();
    h.with(|s| {
        s.run("caret.set", &json!({"para": 1, "off": 0})).unwrap();
        s.run("text.insert", &json!({"text": "ANA "})).unwrap()
    });
    let t = converge(&h, &mut [&mut a, &mut b]);
    let all = t.join("\n");
    assert!(all.contains("Bea was here. ") && all.contains("[Cy]") && all.contains("ANA "), "{all}");

    // Presence: names and carets reach everyone.
    let start = Instant::now();
    loop {
        a.collab_tick();
        b.collab_tick();
        let p = a.run("collab.peers", &json!({})).unwrap();
        let names: Vec<&str> = p["peers"].as_array().unwrap().iter().filter_map(|x| x["name"].as_str()).collect();
        if names == ["Ana", "Bea", "Cy"] && p["peers"][2]["caret"].is_object() {
            assert_eq!(p["peers"][1]["you"], true);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "{p}");
        std::thread::sleep(Duration::from_millis(5));
    }

    // Undo only undoes your own typing.
    let before = texts(&b);
    a.run("edit.undo", &json!({})).unwrap();
    let t = converge(&h, &mut [&mut a, &mut b]);
    let all = t.join("\n");
    assert!(!all.contains("Bea was here."), "{all}");
    assert!(all.contains("[Cy]") && all.contains("ANA "), "{all}");
    assert_ne!(before, t);

    // Leaving keeps the script; the host ending the session tells the others.
    b.run("collab.leave", &json!({})).unwrap();
    assert_eq!(b.collab.status()["role"], "off");
    assert_eq!(texts(&b), t);
    h.with(|s| s.run("collab.leave", &json!({})).unwrap());
    let start = Instant::now();
    while a.collab.active() {
        a.collab_tick();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(a.collab.ended.as_deref().unwrap_or("").contains("ended"), "{:?}", a.collab.ended);
}

#[test]
fn joining_protects_unsaved_work_and_bad_addresses_fail_cleanly() {
    let h = HostThread::start();
    let mut s = Session::sample();
    s.run("text.insert", &json!({"text": "unsaved"})).unwrap();
    let e = s.run("collab.join", &json!({"addr": h.addr, "key": h.key})).unwrap_err();
    assert!(e.to_string().contains("unsaved"));
    assert!(s.run("collab.join", &json!({"addr": h.addr, "key": h.key, "force": true})).is_ok());
    let mut t = Session::new(scriptcraft_doc::Script::new());
    for addr in ["", "nowhere", "127.0.0.1:1", "256.1.1.1:80", "::::"] {
        assert!(t.run("collab.join", &json!({"addr": addr, "key": "k"})).is_err(), "{addr}");
    }
    assert_eq!(t.run("collab.status", &json!({})).unwrap()["role"], "off");
    assert!(t.run("collab.leave", &json!({})).is_err());
    assert!(t.run("collab.host", &json!({"port": 70000})).is_err());
}
