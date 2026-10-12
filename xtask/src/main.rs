//! Developer tasks: `cargo xtask <ci|parity|assets|layers|stats>`.
//!
//! - `ci`: fmt check, clippy -D warnings, tests, assets, layers (run before every commit)
//! - `parity`: regenerate `docs/parity-checklist.md` from the engine's Final Draft 13 catalog
//! - `assets`: every font/image file in the repo has a row in ATTRIBUTION.md
//! - `layers`: crates only depend on lower layers; nothing below the UI depends on egui
//! - `stats`: lines of Rust per crate

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let r = match task.as_str() {
        "ci" => ci(),
        "parity" => parity(),
        "assets" => assets(),
        "layers" => layers(),
        "stats" => {
            stats();
            Ok(())
        }
        _ => Err("usage: cargo xtask <ci|parity|assets|layers|stats>".to_string()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cargo(args: &[&str]) -> Result<(), String> {
    eprintln!("$ cargo {}", args.join(" "));
    let ok = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| e.to_string())?
        .success();
    if ok { Ok(()) } else { Err(format!("cargo {} failed", args.join(" "))) }
}

fn ci() -> Result<(), String> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&["clippy", "--workspace", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace"])?;
    assets()?;
    layers()?;
    let doc = std::fs::read_to_string(root().join("docs/parity-checklist.md")).unwrap_or_default();
    if doc != scriptcraft_engine::catalog::markdown() {
        return Err("docs/parity-checklist.md is stale: run `cargo xtask parity`".into());
    }
    eprintln!("ci: all green");
    Ok(())
}

fn parity() -> Result<(), String> {
    let p = root().join("docs/parity-checklist.md");
    std::fs::write(&p, scriptcraft_engine::catalog::markdown()).map_err(|e| e.to_string())?;
    let (d, pa, m) = scriptcraft_engine::catalog::counts();
    eprintln!("wrote {} ({d} implemented, {pa} partial, {m} not yet)", p.display());
    Ok(())
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn assets() -> Result<(), String> {
    let r = root();
    let attribution = std::fs::read_to_string(r.join("ATTRIBUTION.md")).map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let mut files = Vec::new();
    walk(&r, &mut files);
    let exts = ["ttf", "otf", "woff", "woff2", "png", "jpg", "jpeg", "gif", "svg", "ico", "icns", "fountain", "fdx"];
    let word_lists = ["words/en.txt", "words/common.txt"];
    let mut missing = Vec::new();
    for f in files {
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let rel_s = f.to_string_lossy().replace('\\', "/");
        if !exts.contains(&ext.as_str()) && !word_lists.iter().any(|w| rel_s.ends_with(w)) {
            continue;
        }
        let rel = f.strip_prefix(&r).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        if !attribution.contains(&format!("`{rel}`")) {
            missing.push(rel);
        }
    }
    if missing.is_empty() {
        eprintln!("assets: every asset is listed in ATTRIBUTION.md");
        Ok(())
    } else {
        Err(format!("assets missing from ATTRIBUTION.md:\n  {}", missing.join("\n  ")))
    }
}

/// Layer of each crate (a crate may depend only on crates of a lower layer).
const LAYERS: &[(&str, u8)] = &[
    ("scriptcraft-geom", 0),
    ("scriptcraft-doc", 1),
    ("scriptcraft-fonts", 1),
    ("scriptcraft-spell", 1),
    ("scriptcraft-control-key", 1),
    ("scriptcraft-fountain", 2),
    ("scriptcraft-fdx", 2),
    ("scriptcraft-layout", 2),
    ("scriptcraft-pdf", 3),
    ("scriptcraft-engine", 4),
    ("scriptcraft-mcp", 5),
    ("scriptcraft-ui-egui", 6),
];

const UI_DEPS: &[&str] = &["egui", "eframe", "winit", "rfd", "egui_kittest"];

fn layers() -> Result<(), String> {
    let r = root();
    let mut errs = Vec::new();
    for (name, layer) in LAYERS {
        let dir = name.trim_start_matches("scriptcraft-");
        let toml = std::fs::read_to_string(r.join("crates").join(dir).join("Cargo.toml")).map_err(|e| format!("{name}: {e}"))?;
        for line in toml.lines() {
            let dep = line.split(['=', ' ']).next().unwrap_or("").trim();
            if let Some((_, l)) = LAYERS.iter().find(|(n, _)| *n == dep)
                && *l >= *layer
                && dep != *name
            {
                errs.push(format!("{name} (L{layer}) depends on {dep} (L{l})"));
            }
            if *layer < 6 && UI_DEPS.contains(&dep) {
                errs.push(format!("{name} (L{layer}) depends on UI crate {dep}"));
            }
        }
    }
    if errs.is_empty() {
        eprintln!("layers: ok");
        Ok(())
    } else {
        Err(errs.join("\n"))
    }
}

fn stats() {
    let r = root();
    let mut total = 0;
    let mut dirs: Vec<PathBuf> =
        ["crates", "apps"].iter().flat_map(|d| std::fs::read_dir(r.join(d)).into_iter().flatten().flatten().map(|e| e.path())).collect();
    dirs.push(r.join("xtask"));
    dirs.sort();
    for d in dirs {
        let mut files = Vec::new();
        walk(&d, &mut files);
        let n: usize = files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == "rs"))
            .map(|f| std::fs::read_to_string(f).map(|s| s.lines().count()).unwrap_or(0))
            .sum();
        total += n;
        println!("{:<28} {n:>6}", d.file_name().and_then(|n| n.to_str()).unwrap_or(""));
    }
    println!("{:<28} {total:>6}", "total");
}
