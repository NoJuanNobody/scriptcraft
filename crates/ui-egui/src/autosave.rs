//! Autosave off the UI thread: the app hands a copy of the script to a writer thread, which
//! serialises it and writes this run's recovery copy durably ([`persist::write_recovery_file`]).
//! A slow or stalled disk never freezes typing. Jobs run in order; a write superseded by a later
//! write or clear in the queue is skipped.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use scriptcraft_doc::Script;

use crate::persist;

enum Job {
    Write {
        path: Option<PathBuf>,
        script: Box<Script>,
    },
    Clear,
    /// Answer once every earlier job is done.
    Flush(Sender<()>),
}

/// Writes this run's recovery copy.
pub struct Autosaver {
    slot: persist::RecoverySlot,
    jobs: Option<Sender<Job>>,
    errors: Receiver<String>,
}

impl Autosaver {
    /// Claim this run's recovery copy in the data folder and start the writer.
    pub fn start(data: &std::path::Path) -> std::io::Result<Autosaver> {
        let slot = persist::RecoverySlot::claim(data)?;
        let (err_tx, errors) = std::sync::mpsc::channel();
        let jobs = spawn(slot.file.clone(), err_tx);
        Ok(Autosaver { slot, jobs, errors })
    }

    /// Where this run's copy is written.
    pub fn file(&self) -> &std::path::Path {
        &self.slot.file
    }

    /// Write `script` (saved at `path`, if it was) as the recovery copy, in the background.
    pub fn write(&self, path: Option<PathBuf>, script: &Script) {
        self.send(Job::Write { path, script: Box::new(script.clone()) });
    }

    /// Remove the recovery copy (after the earlier writes).
    pub fn clear(&self) {
        self.send(Job::Clear);
    }

    /// Wait until every write and clear sent so far is on disk (exit, termination, recovery).
    pub fn flush(&self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.send(Job::Flush(tx));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(10));
    }

    /// The latest error from the writer, if any since the last call.
    pub fn error(&self) -> Option<String> {
        self.errors.try_iter().last()
    }

    fn send(&self, job: Job) {
        match &self.jobs {
            Some(tx) => {
                if let Err(std::sync::mpsc::SendError(job)) = tx.send(job) {
                    // The writer is gone (it never panics, but be sure): write here instead.
                    run(&self.slot.file, vec![job], None);
                }
            }
            None => run(&self.slot.file, vec![job], None),
        }
    }
}

impl Drop for Autosaver {
    fn drop(&mut self) {
        // Let queued jobs finish before the slot's lock goes.
        self.flush();
    }
}

/// The writer thread (`None` where there are no threads: the web build writes inline).
fn spawn(file: PathBuf, errors: Sender<String>) -> Option<Sender<Job>> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (file, errors);
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("scriptcraft-autosave".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut batch = vec![first];
                    batch.extend(rx.try_iter());
                    run(&file, batch, Some(&errors));
                }
            })
            .ok()
            .map(|_| tx)
    }
}

/// Do a batch of jobs in order, skipping writes a later write or clear replaces.
fn run(file: &std::path::Path, batch: Vec<Job>, errors: Option<&Sender<String>>) {
    let last_change = batch.iter().rposition(|j| !matches!(j, Job::Flush(_)));
    for (i, job) in batch.into_iter().enumerate() {
        match job {
            Job::Write { path, script } if Some(i) == last_change => {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| persist::write_recovery_file(file, path.as_deref(), &script)));
                let msg = match r {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e.to_string()),
                    Err(_) => Some("internal error".to_string()),
                };
                if let (Some(m), Some(tx)) = (msg, errors) {
                    let _ = tx.send(m);
                }
            }
            Job::Write { .. } => {}
            Job::Clear => {
                let _ = std::fs::remove_file(file);
            }
            Job::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}
