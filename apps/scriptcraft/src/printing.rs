//! Printing for the desktop app, in child processes so the window never waits.
//!
//! - macOS and Linux: CUPS. `lpstat -e` lists the printers and `lpstat -d` names the default; a
//!   job goes out with `lp -d <printer> -n <copies> -t <title> <pdf>`.
//! - Windows: PowerShell. `Get-Printer` lists the printers (and `Win32_Printer` the default); a job
//!   is the PDF's registered print action (`Start-Process -Verb Print`, or `PrintTo` for a chosen
//!   printer), once per copy. This needs a PDF app that registers those actions.
//!
//! The PDF goes in `<temp>/scriptcraft-print/<pid>-<n>-<name>.pdf`. A job's file is removed once
//! `lp` has handed it to CUPS; files opened in the viewer are removed when the app exits; files a
//! previous run left behind (older than an hour) are removed at start.
//!
//! `SCRIPTCRAFT_PRINT=none` turns printing to printers off (the Print window then only writes the
//! PDF); `SCRIPTCRAFT_PRINT=dry-run` lists the real printers but never sends a job (for checks).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use scriptcraft_ui_egui::printing::{PrintHost, PrintJob, Printer, sent_message};

/// How long the printer list may take before it is given up.
const LIST_TIMEOUT: Duration = Duration::from_secs(3);
/// How long handing one job to the system may take.
const SEND_TIMEOUT: Duration = Duration::from_secs(120);
/// A cached printer list is reused this long.
const LIST_FRESH: Duration = Duration::from_secs(30);
/// Temporary files older than this from earlier runs are removed at start.
const STALE: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Backend {
    Cups,
    Windows,
}

/// The system's printing.
pub struct SystemPrint {
    backend: Backend,
    dry_run: bool,
    dir: PathBuf,
    n: usize,
    cached: Option<(Instant, Result<Vec<Printer>, String>)>,
    done: Arc<Mutex<Vec<Result<String, String>>>>,
    running: Arc<AtomicUsize>,
}

fn program(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    std::env::var_os("PATH")
        .and_then(|paths| std::env::split_paths(&paths).map(|d| d.join(&exe)).find(|p| p.is_file()))
        .or_else(|| ["/usr/bin", "/usr/local/bin", "/usr/sbin"].iter().map(|d| Path::new(d).join(name)).find(|p| p.is_file()))
}

fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up.
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Run `c` to completion within `limit`, killing it otherwise: (success, stdout, stderr).
fn run(mut c: Command, limit: Duration) -> Result<(bool, String, String), String> {
    let mut child: Child = c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the printing system did not answer in time".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim().chars().take(2000).collect::<String>();
    Ok((out.status.success(), text(&out.stdout), text(&out.stderr)))
}

/// The printing system for this computer, if any.
pub fn detect() -> Option<SystemPrint> {
    let choice = std::env::var("SCRIPTCRAFT_PRINT").unwrap_or_default();
    if choice.trim() == "none" {
        return None;
    }
    let backend = if cfg!(windows) {
        Backend::Windows
    } else if program("lp").is_some() {
        Backend::Cups
    } else {
        return None;
    };
    let dir = std::env::temp_dir().join("scriptcraft-print");
    remove_stale(&dir);
    Some(SystemPrint { backend, dry_run: choice.trim() == "dry-run", dir, n: 0, cached: None, done: Arc::default(), running: Arc::default() })
}

/// Remove files earlier runs left behind.
fn remove_stale(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let now = SystemTime::now();
    for e in rd.flatten() {
        let old = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age > STALE);
        if old && e.path().extension().is_some_and(|x| x == "pdf") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// `lpstat -e` (one destination per line) and `lpstat -d` ("system default destination: X").
pub fn parse_cups(list: &str, default: &str) -> Vec<Printer> {
    let def = default.lines().find_map(|l| l.split_once(':').map(|(_, d)| d.trim().to_string())).filter(|d| !d.is_empty());
    let mut v: Vec<Printer> = list
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty() && !n.contains(char::is_whitespace))
        .take(500)
        .map(|n| Printer { name: n.to_string(), default: def.as_deref() == Some(n) })
        .collect();
    v.dedup_by(|a, b| a.name == b.name);
    v
}

/// PowerShell output: one printer name per line, the default marked with a leading `*`.
pub fn parse_windows(out: &str) -> Vec<Printer> {
    out.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(500)
        .map(|l| match l.strip_prefix('*') {
            Some(n) => Printer { name: n.trim().to_string(), default: true },
            None => Printer { name: l.to_string(), default: false },
        })
        .filter(|p| !p.name.is_empty())
        .collect()
}

/// `lp`'s answer: "request id is Office-12 (1 file(s))" → `Office-12`.
pub fn cups_job_id(out: &str) -> Option<String> {
    out.split("request id is ").nth(1).and_then(|r| r.split_whitespace().next()).map(str::to_string)
}

fn powershell() -> Command {
    let mut c = command(Path::new("powershell"));
    c.args(["-NoProfile", "-NonInteractive", "-Command"]);
    c
}

impl SystemPrint {
    fn list(&self) -> Result<Vec<Printer>, String> {
        match self.backend {
            Backend::Cups => {
                let lpstat = program("lpstat").ok_or("lpstat was not found (is CUPS installed?)")?;
                let mut c = command(&lpstat);
                c.arg("-e");
                let (ok, out, err) = run(c, LIST_TIMEOUT)?;
                if !ok && out.is_empty() {
                    // "lpstat: No destinations added." is not an error worth showing.
                    return if err.to_lowercase().contains("no destinations") {
                        Ok(Vec::new())
                    } else {
                        Err(if err.is_empty() { "lpstat failed".into() } else { err })
                    };
                }
                let mut c = command(&lpstat);
                c.arg("-d");
                let default = run(c, LIST_TIMEOUT).map(|(_, o, _)| o).unwrap_or_default();
                Ok(parse_cups(&out, &default))
            }
            Backend::Windows => {
                let mut c = powershell();
                c.arg("$d = (Get-CimInstance Win32_Printer -Filter 'Default=True').Name; Get-Printer | ForEach-Object { if ($_.Name -eq $d) { '*' + $_.Name } else { $_.Name } }");
                let (ok, out, err) = run(c, LIST_TIMEOUT * 3)?;
                if !ok {
                    return Err(if err.is_empty() {
                        "the printer list could not be read".into()
                    } else {
                        err.lines().next().unwrap_or("").to_string()
                    });
                }
                Ok(parse_windows(&out))
            }
        }
    }

    /// Write `pdf` to a fresh temporary file.
    fn temp_file(&mut self, name: &str, pdf: &[u8]) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("could not prepare the print file: {e}"))?;
        self.n += 1;
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " ._-()".contains(c) { c } else { '-' }).take(120).collect();
        let safe = if safe.to_lowercase().ends_with(".pdf") { safe } else { format!("{safe}.pdf") };
        let path = self.dir.join(format!("{}-{}-{safe}", std::process::id(), self.n));
        std::fs::write(&path, pdf).map_err(|e| format!("could not write the print file: {e}"))?;
        Ok(path)
    }
}

impl PrintHost for SystemPrint {
    fn engine(&self) -> String {
        match self.backend {
            Backend::Cups => "CUPS (lp)".into(),
            Backend::Windows => "Windows print action".into(),
        }
    }

    fn printers(&mut self, refresh: bool) -> Result<Vec<Printer>, String> {
        if !refresh
            && let Some((t, l)) = &self.cached
            && t.elapsed() < LIST_FRESH
        {
            return l.clone();
        }
        let l = self.list();
        self.cached = Some((Instant::now(), l.clone()));
        l
    }

    fn send(&mut self, job: PrintJob) -> Result<(), String> {
        let path = self.temp_file(&job.name, &job.pdf)?;
        let to = job
            .printer
            .clone()
            .or_else(|| self.cached.as_ref().and_then(|(_, l)| l.as_ref().ok()?.iter().find(|p| p.default).map(|p| p.name.clone())));
        let to = to.unwrap_or_else(|| "the default printer".into());
        let copies = job.copies.clamp(1, scriptcraft_ui_egui::printing::MAX_COPIES);
        if self.dry_run {
            let _ = std::fs::remove_file(&path);
            if let Ok(mut d) = self.done.lock() {
                d.push(Ok(format!("Dry run, nothing printed: {}", sent_message(job.pages, copies, &to))));
            }
            return Ok(());
        }
        let mut runs: Vec<Command> = Vec::new();
        match self.backend {
            Backend::Cups => {
                let lp = program("lp").ok_or("lp was not found (is CUPS installed?)")?;
                let mut c = command(&lp);
                if let Some(p) = &job.printer {
                    c.arg("-d").arg(p);
                }
                c.arg("-n").arg(copies.to_string()).arg("-t").arg(job.name.chars().take(120).collect::<String>()).arg(&path);
                runs.push(c);
            }
            Backend::Windows => {
                // Names go through the environment, never into the script text.
                for _ in 0..copies {
                    let mut c = powershell();
                    if job.printer.is_some() {
                        c.arg("Start-Process -FilePath $env:SC_PRINT_FILE -Verb PrintTo -ArgumentList ('\"' + $env:SC_PRINTER + '\"') -WindowStyle Hidden");
                    } else {
                        c.arg("Start-Process -FilePath $env:SC_PRINT_FILE -Verb Print -WindowStyle Hidden");
                    }
                    c.env("SC_PRINT_FILE", &path).env("SC_PRINTER", job.printer.as_deref().unwrap_or(""));
                    runs.push(c);
                }
            }
        }
        let done = self.done.clone();
        let running = self.running.clone();
        let backend = self.backend;
        let pages = job.pages;
        running.fetch_add(1, Ordering::SeqCst);
        let spawned = std::thread::Builder::new().name("print-job".into()).spawn(move || {
            let mut result = Ok(sent_message(pages, copies, &to));
            for c in runs {
                match run(c, SEND_TIMEOUT) {
                    Ok((true, out, _)) => {
                        if backend == Backend::Cups
                            && let Some(id) = cups_job_id(&out)
                        {
                            result = Ok(format!("{} (job {id})", sent_message(pages, copies, &to)));
                        }
                    }
                    Ok((false, _, err)) => {
                        let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("the printing system refused the job").trim().to_string();
                        result = Err(first.trim_start_matches("lp: ").to_string());
                        break;
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            // CUPS has its own copy of the job now. Windows' print action reads the file
            // after Start-Process returns, so that file stays until the app exits.
            if backend == Backend::Cups {
                let _ = std::fs::remove_file(&path);
            }
            if let Ok(mut d) = done.lock() {
                d.push(result);
            }
            running.fetch_sub(1, Ordering::SeqCst);
        });
        if spawned.is_err() {
            self.running.fetch_sub(1, Ordering::SeqCst);
            return Err("could not start the print job".into());
        }
        Ok(())
    }

    fn finished(&mut self) -> Vec<Result<String, String>> {
        self.done.lock().map(|mut d| std::mem::take(&mut *d)).unwrap_or_default()
    }

    fn busy(&self) -> bool {
        self.running.load(Ordering::SeqCst) > 0
    }

    fn open_viewer(&mut self, name: &str, pdf: Vec<u8>) -> Result<String, String> {
        let path = self.temp_file(name, &pdf)?;
        let mut c = if cfg!(target_os = "macos") {
            let mut c = command(Path::new("open"));
            c.arg(&path);
            c
        } else if cfg!(windows) {
            let mut c = command(Path::new("cmd"));
            c.args(["/C", "start", ""]).arg(&path);
            c
        } else {
            let mut c = command(Path::new("xdg-open"));
            c.arg(&path);
            c
        };
        match c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(mut child) => {
                // Reap it in the background so the window never waits.
                let _ = std::thread::Builder::new().name("pdf-viewer".into()).spawn(move || {
                    let _ = child.wait();
                });
                Ok(path.display().to_string())
            }
            Err(_) => Err(format!("could not open a PDF viewer; the pages are in {}", path.display())),
        }
    }

    fn cleanup(&mut self) {
        let mine = format!("{}-", std::process::id());
        let Ok(rd) = std::fs::read_dir(&self.dir) else { return };
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(&mine) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cups_and_windows_lists() {
        let l = parse_cups("Office_LaserJet\nHome_Inkjet\n\nHome_Inkjet\n", "system default destination: Home_Inkjet\n");
        assert_eq!(l, vec![Printer { name: "Office_LaserJet".into(), default: false }, Printer { name: "Home_Inkjet".into(), default: true }]);
        assert!(parse_cups("", "no system default destination").is_empty());
        let w = parse_windows("Microsoft Print to PDF\r\n*Office LaserJet\r\n\r\n");
        assert_eq!(w.len(), 2);
        assert!(w[1].default && w[1].name == "Office LaserJet");
        assert_eq!(cups_job_id("request id is Office_LaserJet-12 (1 file(s))"), Some("Office_LaserJet-12".into()));
        assert_eq!(cups_job_id("garbage"), None);
    }

    #[test]
    fn temp_files_are_cleaned_up() {
        let dir = std::env::temp_dir().join(format!("scriptcraft-print-test-{}", std::process::id()));
        let mut h = SystemPrint {
            backend: Backend::Cups,
            dry_run: true,
            dir: dir.clone(),
            n: 0,
            cached: None,
            done: Arc::default(),
            running: Arc::default(),
        };
        let a = h.temp_file("My/Script:1.pdf", b"%PDF-1.7").unwrap_or_default();
        assert!(a.is_file());
        assert!(a.file_name().is_some_and(|n| !n.to_string_lossy().contains(['/', ':'])));
        // A dry run hands nothing to CUPS and removes its file at once.
        let job = PrintJob { name: "S.pdf".into(), pdf: b"%PDF".to_vec(), printer: None, copies: 2, pages: 3 };
        assert!(h.send(job).is_ok());
        let msgs = h.finished();
        assert!(matches!(&msgs[..], [Ok(m)] if m.starts_with("Dry run")), "{msgs:?}");
        h.cleanup();
        assert!(!a.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
