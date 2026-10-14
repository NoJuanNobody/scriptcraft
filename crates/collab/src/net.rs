//! TCP transport (native only): newline-delimited JSON, a reader thread per connection, writes
//! from the caller's thread with a timeout.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::proto::{MAX_LINE, ToHost, ToPeer, decode, encode};

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// What happened on the host's sockets.
#[derive(Debug)]
pub enum NetEvent {
    Opened(u32),
    Msg(u32, ToHost),
    Closed(u32),
}

type Conns = Arc<Mutex<HashMap<u32, TcpStream>>>;

/// The host's listening socket and its connections.
pub struct Server {
    rx: Receiver<NetEvent>,
    conns: Conns,
    port: u16,
    lan: bool,
    stop: Arc<AtomicBool>,
}

/// Read lines from `stream` until it closes, decoding each; garbage lines are skipped.
fn read_lines<T: for<'de> serde::Deserialize<'de>>(stream: TcpStream, mut each: impl FnMut(T) -> bool) {
    let mut r = BufReader::new(stream);
    loop {
        let mut buf = Vec::new();
        match r.by_ref().take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if buf.len() > MAX_LINE {
            return;
        }
        if let Some(m) = decode::<T>(buf.trim_ascii())
            && !each(m)
        {
            return;
        }
    }
}

fn write_line(s: &mut TcpStream, line: &str) -> bool {
    let mut b = line.as_bytes().to_vec();
    b.push(b'\n');
    s.write_all(&b).is_ok()
}

impl Server {
    /// Listen on `port` (0: any free port) on loopback, or on every interface with `lan`.
    pub fn bind(port: u16, lan: bool) -> std::io::Result<Server> {
        let ip = if lan { "0.0.0.0" } else { "127.0.0.1" };
        let listener = TcpListener::bind((ip, port))?;
        let port = listener.local_addr()?.port();
        let (tx, rx) = channel();
        let conns: Conns = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2) = (conns.clone(), stop.clone());
        std::thread::spawn(move || accept_loop(listener, tx, c2, s2));
        Ok(Server { rx, conns, port, lan, stop })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn lan(&self) -> bool {
        self.lan
    }

    /// Events since the last poll.
    pub fn poll(&self) -> Vec<NetEvent> {
        self.rx.try_iter().collect()
    }

    /// Send to one connection; a connection that can't take it is closed.
    pub fn send(&self, id: u32, m: &ToPeer) {
        self.send_line(id, &encode(m));
    }

    fn send_line(&self, id: u32, line: &str) {
        let Ok(mut g) = self.conns.lock() else { return };
        let ok = g.get_mut(&id).is_some_and(|s| write_line(s, line));
        if !ok && let Some(s) = g.remove(&id) {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    /// Send one message to several connections (encoded once).
    pub fn send_all(&self, ids: &[u32], m: &ToPeer) {
        let line = encode(m);
        for id in ids {
            self.send_line(*id, &line);
        }
    }

    pub fn close(&self, id: u32) {
        if let Ok(mut g) = self.conns.lock()
            && let Some(s) = g.remove(&id)
        {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut g) = self.conns.lock() {
            for (_, s) in g.drain() {
                let _ = s.shutdown(Shutdown::Both);
            }
        }
        // Wake the accept loop so it sees `stop`.
        let _ = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], self.port)), Duration::from_millis(200));
    }
}

fn accept_loop(listener: TcpListener, tx: Sender<NetEvent>, conns: Conns, stop: Arc<AtomicBool>) {
    let next = AtomicU32::new(1);
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else { continue };
        let id = next.fetch_add(1, Ordering::SeqCst);
        let _ = stream.set_nodelay(true);
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        let (Ok(read), Ok(mut map)) = (stream.try_clone(), conns.lock()) else { continue };
        map.insert(id, stream);
        drop(map);
        if tx.send(NetEvent::Opened(id)).is_err() {
            return;
        }
        let tx = tx.clone();
        std::thread::spawn(move || {
            read_lines::<ToHost>(read, |m| tx.send(NetEvent::Msg(id, m)).is_ok());
            let _ = tx.send(NetEvent::Closed(id));
        });
    }
}

/// A client's connection to a host.
pub struct Link {
    rx: Receiver<Option<ToPeer>>,
    stream: TcpStream,
    addr: String,
}

impl Link {
    /// Connect to `addr` (`host:port`).
    pub fn connect(addr: &str, timeout: Duration) -> std::io::Result<Link> {
        let sa =
            addr.to_socket_addrs()?.next().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("no address for {addr}")))?;
        let stream = TcpStream::connect_timeout(&sa, timeout)?;
        let _ = stream.set_nodelay(true);
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        let read = stream.try_clone()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            read_lines::<ToPeer>(read, |m| tx.send(Some(m)).is_ok());
            let _ = tx.send(None);
        });
        Ok(Link { rx, stream, addr: addr.to_string() })
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn send(&mut self, m: &ToHost) -> bool {
        write_line(&mut self.stream, &encode(m))
    }

    /// Messages since the last poll; `None` marks the connection closing.
    pub fn poll(&self) -> Vec<Option<ToPeer>> {
        self.rx.try_iter().collect()
    }

    /// Wait up to `timeout` for the next message.
    pub fn wait(&self, timeout: Duration) -> Option<Option<ToPeer>> {
        self.rx.recv_timeout(timeout).ok()
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// This machine's address on the local network, if it has one (no packet is sent).
pub fn lan_ip() -> Option<std::net::IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    s.local_addr().ok().map(|a| a.ip()).filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
}

/// A random session key (128 bits, hex).
pub fn new_key() -> Option<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).ok()?;
    Some(b.iter().map(|x| format!("{x:02x}")).collect())
}
