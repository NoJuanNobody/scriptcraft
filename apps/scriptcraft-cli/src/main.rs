//! `scriptcraft-cli`: ScriptCraft without a window.
//!
//! ```text
//! scriptcraft-cli convert in.fountain out.pdf        # .fountain/.fdx → .pdf/.fdx/.fountain
//! scriptcraft-cli commands [--json]                  # every command id
//! scriptcraft-cli run [--open f | --sample] --cmd 'text.insert={"text":"Hi"}' … [--save out.fdx]
//! scriptcraft-cli new tv-multi-camera pilot.fdx       # a new script from a template
//! scriptcraft-cli compare draft1.fdx draft2.fdx       # what changed between two drafts
//! scriptcraft-cli report script.fdx                  # character, scene and location reports
//! scriptcraft-cli parity                             # Final Draft 13 parity checklist (Markdown)
//! scriptcraft-cli mcp [--connect 127.0.0.1:7990]     # MCP server on stdio
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use scriptcraft_engine::{Session, catalog};
use serde_json::{Value, json};

const USAGE: &str = "ScriptCraft command line

USAGE:
  scriptcraft-cli convert <in.fountain|in.fdx|in.docx|in.celtx|in.pdf> <out.pdf|out.fdx|out.fountain>
  scriptcraft-cli commands [--json]
  scriptcraft-cli run [--open <file> | --sample] [--cmd 'id' | --cmd 'id={json}']... [--save <file>]
  scriptcraft-cli new <feature|tv-single-camera|tv-multi-camera|stage-play> <out.fdx|out.fountain|out.pdf>
  scriptcraft-cli compare <older> <newer> [--json]
  scriptcraft-cli report <file>
  scriptcraft-cli parity
  scriptcraft-cli mcp [--connect 127.0.0.1:<port>]
  scriptcraft-cli version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("convert") => convert(args.get(1..).unwrap_or(&[])),
        Some("commands") => commands(args.iter().any(|a| a == "--json")),
        Some("run") => run(args.get(1..).unwrap_or(&[])),
        Some("report") => report(args.get(1..).unwrap_or(&[])),
        Some("new") => new_from_template(args.get(1..).unwrap_or(&[])),
        Some("compare") => compare(args.get(1..).unwrap_or(&[])),
        Some("parity") => {
            print!("{}", catalog::markdown());
            Ok(())
        }
        Some("mcp") => mcp(args.get(1..).unwrap_or(&[])),
        Some("version" | "--version" | "-V") => {
            println!("scriptcraft-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn exec(s: &mut Session, id: &str, params: Value) -> Result<Value, String> {
    s.run(id, &params).map_err(|e| e.to_string())
}

fn new_from_template(a: &[String]) -> Result<(), String> {
    let (Some(t), Some(output)) = (a.first(), a.get(1)) else { return Err(format!("new needs <template> <out>\n\n{USAGE}")) };
    let mut s = Session::new(scriptcraft_doc::Script::new());
    let r = exec(&mut s, "template.new", json!({"template": t}))?;
    let ext = std::path::Path::new(output).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let id = if ext == "pdf" { "file.exportPdf" } else { "file.saveAs" };
    exec(&mut s, id, json!({"path": output}))?;
    println!("{output}: new {} script", r["template"].as_str().unwrap_or(t));
    Ok(())
}

fn compare(a: &[String]) -> Result<(), String> {
    let files: Vec<&String> = a.iter().filter(|x| !x.starts_with("--")).collect();
    let (Some(older), Some(newer)) = (files.first(), files.get(1)) else { return Err(format!("compare needs <older> <newer>\n\n{USAGE}")) };
    let old = scriptcraft_engine::io::read(std::path::Path::new(older.as_str()))?;
    let new = scriptcraft_engine::io::read(std::path::Path::new(newer.as_str()))?;
    let r = scriptcraft_engine::compare::compare(&old, &new);
    if a.iter().any(|x| x == "--json") {
        println!("{}", serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?);
    } else {
        print!("{}", scriptcraft_engine::compare::text_report(&r));
    }
    Ok(())
}

fn convert(a: &[String]) -> Result<(), String> {
    let (Some(input), Some(output)) = (a.first(), a.get(1)) else { return Err(format!("convert needs <in> <out>\n\n{USAGE}")) };
    let mut s = Session::new(scriptcraft_doc::Script::new());
    exec(&mut s, "file.open", json!({"path": input}))?;
    let ext = std::path::Path::new(output).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let id = match ext.as_str() {
        "pdf" => "file.exportPdf",
        "fdx" => "file.exportFdx",
        "fountain" | "spmd" | "txt" => "file.exportFountain",
        _ => return Err(format!("unknown output type `.{ext}` (use .pdf, .fdx or .fountain)")),
    };
    let r = if ext == "txt" || ext == "spmd" {
        exec(&mut s, "file.saveAs", json!({"path": output}))?
    } else {
        exec(&mut s, id, json!({"path": output}))?
    };
    let pages = exec(&mut s, "document.stats", json!({}))?;
    println!("{} → {} ({} pages, {} scenes)", input, r["path"].as_str().unwrap_or(output), pages["pages"], pages["scenes"]);
    Ok(())
}

fn commands(as_json: bool) -> Result<(), String> {
    let s = Session::new(scriptcraft_doc::Script::new());
    if as_json {
        println!("{}", serde_json::to_string_pretty(&s.registry.describe(None)).unwrap_or_default());
        return Ok(());
    }
    for c in s.registry.all() {
        println!("{:<28} {:<34} {:<16} {}", c.id, c.label, c.shortcut, if c.params == "{}" { "" } else { c.params });
    }
    Ok(())
}

/// `id` or `id={json}`.
fn parse_cmd(spec: &str) -> Result<(String, Value), String> {
    match spec.split_once('=') {
        Some((id, p)) => Ok((id.trim().to_string(), serde_json::from_str(p).map_err(|e| format!("bad JSON for {id}: {e}"))?)),
        None => Ok((spec.trim().to_string(), json!({}))),
    }
}

fn run(a: &[String]) -> Result<(), String> {
    let mut s = Session::new(scriptcraft_doc::Script::new());
    let mut it = a.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--open" => {
                let f = it.next().ok_or("--open needs a file")?;
                exec(&mut s, "file.open", json!({"path": f}))?;
            }
            "--sample" => {
                exec(&mut s, "file.sample", json!({}))?;
            }
            "--cmd" => {
                let (id, p) = parse_cmd(it.next().ok_or("--cmd needs a command")?)?;
                let r = exec(&mut s, &id, p)?;
                println!("{}", json!({"command": id, "result": r}));
            }
            "--save" => {
                let f = it.next().ok_or("--save needs a file")?;
                let ext = std::path::Path::new(f).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                let id = if ext == "pdf" { "file.exportPdf" } else { "file.saveAs" };
                let r = exec(&mut s, id, json!({"path": f}))?;
                println!("{}", json!({"saved": r}));
            }
            other => return Err(format!("unknown argument `{other}`\n\n{USAGE}")),
        }
    }
    Ok(())
}

fn report(a: &[String]) -> Result<(), String> {
    let f = a.first().ok_or("report needs a file")?;
    let mut s = Session::new(scriptcraft_doc::Script::new());
    exec(&mut s, "file.open", json!({"path": f}))?;
    let stats = exec(&mut s, "document.stats", json!({}))?;
    println!("{f}: {} pages, {} scenes, {} words\n", stats["pages"], stats["scenes"], stats["words"]);
    println!("CHARACTERS");
    for c in exec(&mut s, "report.characters", json!({}))?.as_array().into_iter().flatten() {
        println!(
            "  {:<24} {:>3} speeches {:>5} words  first page {}",
            c["name"].as_str().unwrap_or(""),
            c["speeches"].as_u64().unwrap_or(0),
            c["words"].as_u64().unwrap_or(0),
            c["firstPageLabel"].as_str().unwrap_or("")
        );
    }
    println!("\nSCENES");
    for c in exec(&mut s, "report.scenes", json!({}))?.as_array().into_iter().flatten() {
        let e = c["eighths"].as_u64().unwrap_or(0) as usize;
        println!(
            "  {:>3}  p.{:<3} {:>6}  {}",
            c["scene"].as_u64().unwrap_or(0),
            c["pageLabel"].as_str().unwrap_or(""),
            scriptcraft_engine::reports::eighths_label(e),
            c["heading"].as_str().unwrap_or("")
        );
    }
    println!("\nLOCATIONS");
    for c in exec(&mut s, "report.locations", json!({}))?.as_array().into_iter().flatten() {
        println!("  {:<40} {} scene(s)", c["location"].as_str().unwrap_or(""), c["scenes"]);
    }
    Ok(())
}

fn mcp(a: &[String]) -> Result<(), String> {
    let backend: Box<dyn scriptcraft_mcp::Backend> = match a.iter().position(|x| x == "--connect") {
        Some(i) => Box::new(scriptcraft_mcp::Remote::connect(a.get(i + 1).ok_or("--connect needs 127.0.0.1:PORT")?)?),
        None => Box::new(scriptcraft_mcp::Headless::default()),
    };
    let mut server = scriptcraft_mcp::Server::new(backend);
    let stdin = std::io::stdin();
    server.serve(stdin.lock(), std::io::stdout()).map_err(|e| e.to_string())
}
