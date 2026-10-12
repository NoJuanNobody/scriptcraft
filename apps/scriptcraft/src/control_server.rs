//! Loopback JSON-lines control server: one request per line, one reply per line. Every request
//! carries the window's key (written to `<settings>/control-key.local-<port>`, mode 0600, removed
//! on exit). Pattern adapted from WordCraft (MIT OR Apache-2.0; see NOTICE).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use scriptcraft_ui_egui::ControlRequest;
use serde_json::{Value, json};

/// Longer request lines are refused.
const MAX_LINE: u64 = 1 << 20;

#[derive(Clone)]
pub struct KeyFile(PathBuf);

impl KeyFile {
    pub fn remove(&self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn random_key() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn write_key(p: &std::path::Path, key: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(p);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let text = scriptcraft_control_key::key_file_contents(key, std::process::id());
    o.open(p).and_then(|mut f| f.write_all(text.as_bytes())).map_err(|e| e.to_string())
}

/// Bind `127.0.0.1:port`, write the key file and serve.
pub fn start(port: u16, ctx: egui::Context) -> Option<(Receiver<ControlRequest>, Option<KeyFile>)> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| eprintln!("control server: cannot bind 127.0.0.1:{port}: {e}")).ok()?;
    let key = random_key().map_err(|e| eprintln!("control server off: {e}")).ok()?;
    let kf = scriptcraft_control_key::settings_dir().map(|d| scriptcraft_control_key::key_path(&d, port)).and_then(|p| match write_key(&p, &key) {
        Ok(()) => {
            eprintln!("control server on 127.0.0.1:{port}, key in {}", p.display());
            Some(KeyFile(p))
        }
        Err(e) => {
            eprintln!("control key not written ({e}); requests will be refused");
            None
        }
    });
    let (tx, rx) = channel::<ControlRequest>();
    let key: Arc<str> = if kf.is_some() { key.into() } else { "".into() };
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, ctx, key) = (tx.clone(), ctx.clone(), key.clone());
            std::thread::spawn(move || serve(stream, tx, ctx, &key));
        }
    });
    Some((rx, kf))
}

fn serve(stream: TcpStream, tx: Sender<ControlRequest>, ctx: egui::Context, key: &str) {
    let Ok(read) = stream.try_clone() else { return };
    let mut out = stream;
    let _ = read.set_read_timeout(Some(Duration::from_secs(600)));
    let mut reader = BufReader::new(read);
    loop {
        let mut buf = Vec::new();
        match reader.by_ref().take(MAX_LINE + 1).read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if buf.len() as u64 > MAX_LINE {
            let _ = writeln!(out, "{}", json!({"ok": false, "error": "line too long"}));
            return;
        }
        let line = String::from_utf8_lossy(&buf).trim().to_string();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(out, "{}", json!({"ok": false, "error": format!("bad JSON: {e}")}));
                continue;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let given = msg.get("key").and_then(Value::as_str).unwrap_or("");
        if !scriptcraft_control_key::keys_match(given, key) {
            let _ = writeln!(out, "{}", json!({"id": id, "ok": false, "error": "missing or wrong control key"}));
            continue;
        }
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let (req, reply) = ControlRequest::new(method, params);
        if tx.send(req).is_err() {
            return;
        }
        ctx.request_repaint();
        let mut v = reply.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| json!({"ok": false, "error": "timed out"}));
        if let Some(o) = v.as_object_mut() {
            o.insert("id".into(), id);
        }
        if writeln!(out, "{v}").is_err() {
            return;
        }
    }
}
