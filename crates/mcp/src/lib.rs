//! ScriptCraft's MCP server (Model Context Protocol, JSON-RPC 2.0 over stdio), structured after
//! WordCraft's (MIT OR Apache-2.0; see NOTICE).
//!
//! Two backends: [`Headless`] runs an in-process editing session; [`Remote`] talks to a running
//! ScriptCraft window through its control channel (`scriptcraft --control PORT`), so agents can
//! also take screenshots of the real UI.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod server;
pub mod tools;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use scriptcraft_engine::Session;
use serde_json::{Value, json};

pub use server::Server;

/// Something that answers control-channel methods (`engine.execute`, `engine.commands`,
/// `document.inspect`, `ui.screenshot`…).
pub trait Backend {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String>;
    fn describe(&self) -> String;
}

/// Methods that need the desktop app.
pub const NEEDS_APP: &str = "needs the desktop app: start `scriptcraft --control 7990` and run `scriptcraft-cli mcp --connect 127.0.0.1:7990`";

/// An in-process session.
pub struct Headless {
    pub session: Session,
}

impl Default for Headless {
    fn default() -> Self {
        Headless { session: Session::new(scriptcraft_doc::Script::new()) }
    }
}

impl Backend for Headless {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        match method {
            "engine.execute" | "command" => {
                let id = params.get("command").and_then(Value::as_str).ok_or("missing `command`")?.to_string();
                let p = params.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({}));
                self.session.run(&id, &p).map_err(|e| e.to_string())
            }
            "engine.commands" => Ok(self.session.registry.describe(Some(&self.session))),
            m if m.starts_with("ui.") => Err(format!("`{m}` {NEEDS_APP}")),
            other => self.session.run(other, &params).map_err(|e| e.to_string()),
        }
    }
    fn describe(&self) -> String {
        "headless session (no window)".into()
    }
}

/// A running ScriptCraft app, reached through its loopback control port.
pub struct Remote {
    addr: String,
    key: String,
    conn: (BufReader<TcpStream>, TcpStream),
    next_id: u64,
}

impl Remote {
    /// Connect to `127.0.0.1:PORT` (loopback only: the key never leaves the machine).
    pub fn connect(addr: &str) -> Result<Self, String> {
        let sa: std::net::SocketAddr = addr.parse().map_err(|_| format!("`{addr}` is not an IP:PORT address"))?;
        if !sa.ip().is_loopback() {
            return Err("only loopback addresses (127.0.0.1, ::1) are allowed".into());
        }
        let key = scriptcraft_control_key::find_key(sa.port())?;
        let s = TcpStream::connect_timeout(&sa, Duration::from_millis(800)).map_err(|e| format!("cannot connect to {addr}: {e}"))?;
        s.set_read_timeout(Some(Duration::from_secs(90))).map_err(|e| e.to_string())?;
        let r = s.try_clone().map_err(|e| e.to_string())?;
        Ok(Remote { addr: addr.to_string(), key, conn: (BufReader::new(r), s), next_id: 1 })
    }
}

impl Backend for Remote {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({"id": id, "key": self.key, "method": method, "params": params});
        let (r, w) = &mut self.conn;
        writeln!(w, "{req}").map_err(|e| e.to_string())?;
        let mut line = String::new();
        r.read_line(&mut line).map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&line).map_err(|e| format!("bad reply: {e}"))?;
        if v.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(v.get("error").and_then(Value::as_str).unwrap_or("request failed").to_string())
        }
    }
    fn describe(&self) -> String {
        format!("connected to the ScriptCraft app at {}", self.addr)
    }
}

#[cfg(test)]
mod tests;
