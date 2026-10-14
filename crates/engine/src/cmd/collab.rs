//! Real-time collaboration commands (#25): host, join, leave, status, peers.

use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("collab.host", "Host a Session", "File › Collaborate", host)
            .params(r#"{"port": n (default: any free port), "lan": bool (others on your network can join; default loopback only), "name": string} → {"key", "port", "addresses"}"#)
            .when(off)
            .pure(),
        CommandSpec::new("collab.join", "Join a Session", "File › Collaborate", join)
            .params(r#"{"addr": "host:port", "key": string, "name": string, "force": bool (replace a script with unsaved changes)}"#)
            .when(off)
            .pure(),
        CommandSpec::new("collab.leave", "Leave the Session", "File › Collaborate", leave).when(on).pure(),
        CommandSpec::new("collab.status", "Collaboration Status", "File › Collaborate", status).pure(),
        CommandSpec::new("collab.peers", "People in the Session", "File › Collaborate", peers).pure(),
    ]
}

fn off(s: &Session) -> Option<&'static str> {
    if s.collab.active() { Some("already in a collaboration session (leave it first)") } else { None }
}

fn on(s: &Session) -> Option<&'static str> {
    if s.collab.active() { None } else { Some("not in a collaboration session") }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn name(v: &Value, fallback: &str) -> String {
    scriptcraft_collab::clean_name(p::str(v, "name").unwrap_or(""), fallback)
}

#[cfg(target_arch = "wasm32")]
fn host(_: &mut Session, _: &Value) -> CmdResult {
    Err(CmdError::Failed("collaboration isn't available in the browser (it needs network sockets); use the desktop app".into()))
}

#[cfg(target_arch = "wasm32")]
fn join(s: &mut Session, v: &Value) -> CmdResult {
    host(s, v)
}

#[cfg(not(target_arch = "wasm32"))]
fn host(s: &mut Session, v: &Value) -> CmdResult {
    use scriptcraft_collab::net::{Server, new_key};
    let port = match v.get("port") {
        None | Some(Value::Null) => 0,
        Some(_) => {
            p::usize(v, "port").filter(|n| *n <= usize::from(u16::MAX)).ok_or_else(|| CmdError::Params("`port` must be 0–65535".into()))? as u16
        }
    };
    let lan = p::bool(v, "lan").unwrap_or(false);
    let key = new_key().ok_or_else(|| CmdError::Failed("no random source for a session key".into()))?;
    let srv = Server::bind(port, lan).map_err(|e| CmdError::Failed(format!("can't listen on port {port}: {e}")))?;
    let host = scriptcraft_collab::Host::new(s.script.clone(), &key, &name(v, "Host"));
    s.collab.role = Some(crate::collab::Role::Host { host: Box::new(host), srv });
    s.collab.ended = None;
    s.status = "Hosting a collaboration session".into();
    Ok(s.collab.status())
}

#[cfg(not(target_arch = "wasm32"))]
fn join(s: &mut Session, v: &Value) -> CmdResult {
    use std::time::{Duration, Instant};

    use scriptcraft_collab::net::Link;
    use scriptcraft_collab::{PROTO, ToHost, ToPeer};
    let addr = p::req_str(v, "addr")?.trim();
    let key = p::req_str(v, "key")?.trim();
    if addr.is_empty() || addr.len() > 200 || key.is_empty() || key.len() > 200 {
        return Err(CmdError::Params("`addr` (host:port) and `key` are required".into()));
    }
    if s.dirty && !p::bool(v, "force").unwrap_or(false) {
        return Err(CmdError::Failed("joining replaces the open script, which has unsaved changes: save it first, or pass \"force\": true".into()));
    }
    let mut link = Link::connect(addr, Duration::from_secs(4)).map_err(|e| CmdError::Failed(format!("can't reach {addr}: {e}")))?;
    if !link.send(&ToHost::Hello { proto: PROTO, key: key.to_string(), name: name(v, "Guest") }) {
        return Err(CmdError::Failed(format!("{addr} closed the connection")));
    }
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(CmdError::Failed(format!("{addr} didn't answer (is it a ScriptCraft session?)")));
        }
        match link.wait(left) {
            Some(Some(ToPeer::Welcome { id, version, script })) => {
                let client = scriptcraft_collab::Client::new(id, version, (*script).clone());
                s.replace_script(client.shadow().clone());
                s.path = None;
                s.collab.role = Some(crate::collab::Role::Client { client: Box::new(client), link });
                s.collab.ended = None;
                s.status = format!("Joined the session at {addr}");
                return Ok(s.collab.status());
            }
            Some(Some(ToPeer::Refused { reason })) => return Err(CmdError::Failed(format!("the host refused: {reason}"))),
            Some(Some(_)) => {}
            Some(None) | None => return Err(CmdError::Failed(format!("{addr} closed the connection"))),
        }
    }
}

fn leave(s: &mut Session, _: &Value) -> CmdResult {
    #[cfg(not(target_arch = "wasm32"))]
    match s.collab.role.take() {
        Some(crate::collab::Role::Client { mut link, .. }) => {
            link.send(&scriptcraft_collab::ToHost::Bye);
        }
        Some(crate::collab::Role::Host { host, srv }) => {
            srv.send_all(&host.joined(), &scriptcraft_collab::ToPeer::Bye { reason: "the host ended the session".into() });
        }
        None => {}
    }
    s.status = "Left the collaboration session; the script stays open here".into();
    Ok(json!({"left": true}))
}

fn status(s: &mut Session, _: &Value) -> CmdResult {
    Ok(s.collab.status())
}

fn peers(s: &mut Session, _: &Value) -> CmdResult {
    Ok(crate::collab::peers_json(&s.collab))
}
