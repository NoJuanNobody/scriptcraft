//! Compare the open script with another draft.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::io::{self, Format};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("compare.with", "Compare with Draft", "Tools › Compare Drafts", with)
            .params(r#"{"path": ".fdx|.fountain"} or {"name": "draft.fdx", "text": string | "data": base64}; "otherIs": "older" (default) | "newer" → {"added", "removed", "changed", "moved", "unchanged", "diffs": [{"kind", "old", "new", "element", "text", "pieces": [{"op": "same"|"added"|"removed", "text"}], "scene"}]}"#)
            .pure(),
    ]
}

fn with(s: &mut Session, v: &Value) -> CmdResult {
    let (other, name) = match p::str(v, "path") {
        Some(path) => {
            let path = PathBuf::from(path);
            (
                io::read_noted_with(&path, &s.sluglines).map(|(script, _, _)| script).map_err(CmdError::Failed)?,
                path.file_name().and_then(|n| n.to_str()).unwrap_or("draft").to_string(),
            )
        }
        None => {
            let name = p::req_str(v, "name")?;
            let format = Format::of(std::path::Path::new(name)).ok_or_else(|| CmdError::Params(format!("unknown file type: {name}")))?;
            let bytes = match (p::str(v, "text"), p::str(v, "data")) {
                (Some(t), _) => t.as_bytes().to_vec(),
                (None, Some(d)) => crate::b64::decode(d).map_err(CmdError::Params)?,
                _ => return Err(CmdError::Params("`path`, or `name` with `text` or `data` (base64), is required".into())),
            };
            if bytes.len() as u64 > io::MAX_FILE {
                return Err(CmdError::Failed(format!("{name} is too large")));
            }
            (io::parse_noted_with(&bytes, format, &s.sluglines).map(|(script, _)| script).map_err(CmdError::Failed)?, name.to_string())
        }
    };
    let newer = match p::str(v, "otherIs") {
        None | Some("older") => false,
        Some("newer") => true,
        Some(x) => return Err(CmdError::Params(format!("`otherIs` must be \"older\" or \"newer\", not `{x}`"))),
    };
    let r = if newer { crate::compare::compare(&s.script, &other) } else { crate::compare::compare(&other, &s.script) };
    let mut out = serde_json::to_value(&r).map_err(|e| CmdError::Failed(e.to_string()))?;
    if let Some(o) = out.as_object_mut() {
        o.insert("other".into(), json!(name));
        o.insert("otherIs".into(), json!(if newer { "newer" } else { "older" }));
    }
    s.status = format!("Compared with {name}: {} changed, {} added, {} removed, {} moved", r.changed, r.added, r.removed, r.moved);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn compare_with_text() {
        let mut s = Session::sample();
        let older = String::from_utf8(crate::io::encode(&s.script, crate::io::Format::Fdx).unwrap()).unwrap();
        s.run("select.text", &json!({"text": "Don't you dare."})).unwrap();
        s.run("text.insert", &json!({"text": "Not tonight, you don't."})).unwrap();
        let r = s.run("compare.with", &json!({"name": "old.fdx", "text": older})).unwrap();
        assert_eq!(r["changed"], 1, "{r}");
        assert_eq!(r["diffs"][0]["kind"], "changed");
        let r = s.run("compare.with", &json!({"name": "old.fdx", "text": older, "otherIs": "newer"})).unwrap();
        assert_eq!(r["changed"], 1);
        assert!(s.run("compare.with", &json!({"name": "x.doc", "text": "a"})).is_err());
        assert!(s.run("compare.with", &json!({"name": "x.fdx", "text": "<nope"})).is_err());
        assert!(s.run("compare.with", &json!({"path": "/definitely/not/here.fdx"})).is_err());
        assert!(s.run("compare.with", &json!({})).is_err());
    }
}
