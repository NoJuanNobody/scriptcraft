//! Printing to a printer. The window renders the chosen pages as PDF (`print.render`) and hands
//! them to the host's [`PrintHost`]: CUPS (`lp`) on macOS and Linux, the system's print action on
//! Windows, the browser's own print window on the web. Nothing here waits for a printer: `send`
//! starts the job and the outcome arrives through [`PrintHost::finished`] as a status message.
//!
//! `print.printers` and `print.send` are answered here; headless they return a clear error.

use std::sync::{Arc, Mutex};

use scriptcraft_engine::CmdError;
use serde_json::{Value, json};

use crate::ScriptApp;

/// Most copies of one print job.
pub const MAX_COPIES: u32 = 99;

/// A printer the host can send to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Printer {
    pub name: String,
    /// The system's default printer.
    pub default: bool,
}

/// One print job: the rendered PDF and where it goes.
#[derive(Clone, Debug)]
pub struct PrintJob {
    /// File name of the PDF (`My Script.pdf`), also the job's title.
    pub name: String,
    pub pdf: Vec<u8>,
    /// `None`: the default printer.
    pub printer: Option<String>,
    pub copies: u32,
    /// Pages in the PDF, for the status message.
    pub pages: usize,
}

/// Printing for the window. Nothing here may block for long.
pub trait PrintHost {
    /// What prints (`CUPS`, `Windows`, `browser`).
    fn engine(&self) -> String;
    /// The printers, freshest first; `refresh` asks the system again instead of a cached list.
    fn printers(&mut self, refresh: bool) -> Result<Vec<Printer>, String>;
    /// The printer and copies are chosen in the system's own print window (the browser), not in
    /// ScriptCraft's.
    fn chooses_in_dialog(&self) -> bool {
        false
    }
    /// Start printing `job`; returns once the job is handed off (or refused).
    fn send(&mut self, job: PrintJob) -> Result<(), String>;
    /// Outcomes of jobs since the last call: a message for the status bar, or an error.
    fn finished(&mut self) -> Vec<Result<String, String>>;
    /// A job is still being handed to the printer.
    fn busy(&self) -> bool {
        false
    }
    /// Open the PDF in the system's viewer (for previewing); the path or a message on success.
    fn open_viewer(&mut self, _name: &str, _pdf: Vec<u8>) -> Result<String, String> {
        Err("there is no PDF viewer here".into())
    }
    /// The app is closing: remove temporary files.
    fn cleanup(&mut self) {}
}

/// A host that prints nothing: it records each job. For tests, screenshots and demo videos.
pub struct FakePrinter {
    pub printers: Vec<Printer>,
    /// Jobs sent: (printer, copies, pages, PDF bytes).
    pub sent: Arc<Mutex<Vec<(Option<String>, u32, usize, usize)>>>,
    /// Refuse every job with this message.
    pub fail: Option<String>,
    done: Vec<Result<String, String>>,
    pub cleaned: Arc<Mutex<bool>>,
}

impl FakePrinter {
    pub fn new(printers: &[&str]) -> Self {
        FakePrinter {
            printers: printers.iter().enumerate().map(|(i, n)| Printer { name: (*n).to_string(), default: i == 0 }).collect(),
            sent: Arc::default(),
            fail: None,
            done: Vec::new(),
            cleaned: Arc::default(),
        }
    }
}

impl PrintHost for FakePrinter {
    fn engine(&self) -> String {
        "fake (prints nothing)".into()
    }
    fn printers(&mut self, _refresh: bool) -> Result<Vec<Printer>, String> {
        Ok(self.printers.clone())
    }
    fn send(&mut self, job: PrintJob) -> Result<(), String> {
        if let Some(e) = &self.fail {
            return Err(e.clone());
        }
        let to = job
            .printer
            .clone()
            .or_else(|| self.printers.iter().find(|p| p.default).map(|p| p.name.clone()))
            .unwrap_or_else(|| "the default printer".into());
        if let Ok(mut s) = self.sent.lock() {
            s.push((job.printer.clone(), job.copies, job.pages, job.pdf.len()));
        }
        self.done.push(Ok(sent_message(job.pages, job.copies, &to)));
        Ok(())
    }
    fn finished(&mut self) -> Vec<Result<String, String>> {
        std::mem::take(&mut self.done)
    }
    fn open_viewer(&mut self, name: &str, _pdf: Vec<u8>) -> Result<String, String> {
        Ok(format!("(fake viewer) {name}"))
    }
    fn cleanup(&mut self) {
        if let Ok(mut c) = self.cleaned.lock() {
            *c = true;
        }
    }
}

/// "Sent 12 pages to Office LaserJet" (with copies when more than one).
pub fn sent_message(pages: usize, copies: u32, to: &str) -> String {
    let p = if pages == 1 { "1 page".to_string() } else { format!("{pages} pages") };
    if copies > 1 { format!("Sent {copies} copies of {p} to {to}") } else { format!("Sent {p} to {to}") }
}

/// A printer name as given by an agent or the window: trimmed, no control characters, not an
/// option (`-x`), at most 200 characters.
pub fn clean_printer_name(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty() && t.chars().count() <= 200 && !t.starts_with('-') && !t.chars().any(char::is_control)).then(|| t.to_string())
}

/// Copies from params: 1 when absent, an error outside 1–99.
fn copies(p: &Value) -> Result<u32, CmdError> {
    match p.get("copies") {
        None | Some(Value::Null) => Ok(1),
        Some(v) => v
            .as_u64()
            .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))
            .filter(|n| (1..=u64::from(MAX_COPIES)).contains(n))
            .map(|n| n as u32)
            .ok_or_else(|| CmdError::Params(format!("`copies` must be a whole number from 1 to {MAX_COPIES}"))),
    }
}

/// The commands answered by the window (`None` for any other id).
pub fn command(app: &mut ScriptApp, id: &str, p: &Value) -> Option<Result<Value, CmdError>> {
    match id {
        "print.printers" => Some(printers(app, p)),
        "print.send" => Some(send(app, p)),
        _ => None,
    }
}

fn no_host() -> CmdError {
    CmdError::Disabled("printing to a printer needs the ScriptCraft window; print.render gives the PDF".into())
}

fn printers(app: &mut ScriptApp, p: &Value) -> Result<Value, CmdError> {
    let refresh = p.get("refresh").and_then(Value::as_bool).unwrap_or(false);
    let h = app.printer.as_mut().ok_or_else(no_host)?;
    let engine = h.engine();
    let in_dialog = h.chooses_in_dialog();
    let list = h.printers(refresh).map_err(CmdError::Failed)?;
    let default = list.iter().find(|x| x.default).map(|x| x.name.clone());
    Ok(json!({
        "printers": list.iter().map(|x| json!({"name": x.name, "default": x.default})).collect::<Vec<_>>(),
        "default": default,
        "engine": engine,
        "choosesInDialog": in_dialog,
    }))
}

fn send(app: &mut ScriptApp, p: &Value) -> Result<Value, CmdError> {
    let n = copies(p)?;
    let printer = match p.get("printer") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(Value::String(s)) => Some(clean_printer_name(s).ok_or_else(|| CmdError::Params("`printer` is not a valid printer name".into()))?),
        Some(_) => return Err(CmdError::Params("`printer` must be a printer name (see print.printers)".into())),
    };
    let h = app.printer.as_mut().ok_or_else(no_host)?;
    if let Some(name) = &printer
        && !h.chooses_in_dialog()
    {
        let list = h.printers(false).map_err(CmdError::Failed)?;
        if !list.iter().any(|x| &x.name == name) {
            let names: Vec<&str> = list.iter().map(|x| x.name.as_str()).collect();
            return Err(CmdError::Params(if names.is_empty() {
                format!("no printer named {name}: no printers were found")
            } else {
                format!("no printer named {name} (printers: {})", names.join(", "))
            }));
        }
    }
    let r = app.session.run("print.render", p)?;
    let name = r["name"].as_str().unwrap_or("Untitled.pdf").to_string();
    let pdf = scriptcraft_engine::b64::decode(r["data"].as_str().unwrap_or("")).map_err(CmdError::Failed)?;
    let pages = r["pages"].clone();
    let count = pages.as_array().map_or(0, Vec::len);
    let h = app.printer.as_mut().ok_or_else(no_host)?;
    let engine = h.engine();
    let in_dialog = h.chooses_in_dialog();
    h.send(PrintJob { name: name.clone(), pdf, printer: printer.clone(), copies: n, pages: count }).map_err(CmdError::Failed)?;
    app.set_status(if in_dialog {
        format!("Printing {name}…")
    } else {
        format!("Sending {name} to {}…", printer.as_deref().unwrap_or("the default printer"))
    });
    Ok(json!({"name": name, "pages": pages, "printer": printer, "copies": n, "engine": engine, "sent": true}))
}

/// Each frame: show the outcome of finished jobs, and keep checking while one is under way.
pub fn tick(app: &mut ScriptApp, ctx: &egui::Context) {
    let Some(h) = app.printer.as_mut() else { return };
    let done = h.finished();
    let busy = h.busy();
    for r in done {
        app.set_status(match r {
            Ok(m) => m,
            Err(e) => format!("Printing failed: {e}"),
        });
    }
    if busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// The app is closing: let the host remove its temporary files.
pub fn cleanup(app: &mut ScriptApp) {
    if let Some(h) = app.printer.as_mut() {
        h.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printer_names_and_copies() {
        assert_eq!(clean_printer_name(" Office_LaserJet "), Some("Office_LaserJet".into()));
        assert_eq!(clean_printer_name("-o evil"), None);
        assert_eq!(clean_printer_name("a\nb"), None);
        assert_eq!(clean_printer_name(""), None);
        assert_eq!(clean_printer_name(&"x".repeat(201)), None);
        assert_eq!(copies(&json!({})).ok(), Some(1));
        assert_eq!(copies(&json!({"copies": 3})).ok(), Some(3));
        assert_eq!(copies(&json!({"copies": 3.0})).ok(), Some(3));
        for bad in [json!(0), json!(-1), json!(100), json!(2.5), json!("2"), json!(u64::MAX), json!(f64::MAX)] {
            assert!(copies(&json!({"copies": bad})).is_err(), "{bad}");
        }
        assert_eq!(sent_message(1, 1, "P"), "Sent 1 page to P");
        assert_eq!(sent_message(12, 2, "P"), "Sent 2 copies of 12 pages to P");
    }
}
