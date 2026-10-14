//! A forgiving HTML tokenizer: tags (name lowercased, attributes), text with entities decoded.
//! Comments, `<script>` and `<style>` contents are skipped. Never fails.

#[derive(Debug, PartialEq)]
pub enum Tok {
    Open(String, Vec<(String, String)>),
    Close(String),
    Text(String),
}

/// Most tokens produced.
const MAX_TOKENS: usize = 4_000_000;

pub fn tokens(src: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    let mut text_start = 0;
    let mut skip_until: Option<&'static str> = None;
    while i < b.len() && out.len() < MAX_TOKENS {
        if b.get(i) != Some(&b'<') {
            i += 1;
            continue;
        }
        if skip_until.is_none() && i > text_start {
            out.push(Tok::Text(decode(src.get(text_start..i).unwrap_or(""))));
        }
        if src.get(i..).is_some_and(|s| s.starts_with("<!--")) {
            i = src.get(i + 4..).and_then(|s| s.find("-->")).map_or(b.len(), |p| i + 4 + p + 3);
            text_start = i;
            continue;
        }
        let end = src.get(i..).and_then(|s| s.find('>')).map_or(b.len(), |p| i + p);
        let inner = src.get(i + 1..end).unwrap_or("");
        i = (end + 1).min(b.len());
        text_start = i;
        if let Some(name) = inner.strip_prefix('/') {
            let name = name.trim().to_ascii_lowercase();
            if skip_until == Some(if name == "script" { "script" } else { "style" }) && (name == "script" || name == "style") {
                skip_until = None;
            }
            if skip_until.is_none() {
                out.push(Tok::Close(name));
            }
            continue;
        }
        if skip_until.is_some() || inner.starts_with('!') || inner.starts_with('?') {
            continue;
        }
        let (name, rest) = inner.split_once(|c: char| c.is_whitespace()).unwrap_or((inner, ""));
        let name = name.trim_end_matches('/').to_ascii_lowercase();
        if name == "script" || name == "style" {
            skip_until = Some(if name == "script" { "script" } else { "style" });
            continue;
        }
        out.push(Tok::Open(name, attrs(rest)));
    }
    if skip_until.is_none() && text_start < b.len() {
        out.push(Tok::Text(decode(src.get(text_start..).unwrap_or(""))));
    }
    out
}

fn attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim().trim_end_matches('/');
    while !rest.is_empty() && out.len() < 64 {
        rest = rest.trim_start();
        let key_end = rest.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(rest.len());
        let key = rest.get(..key_end).unwrap_or("").to_ascii_lowercase();
        rest = rest.get(key_end..).unwrap_or("").trim_start();
        let mut val = String::new();
        if let Some(r) = rest.strip_prefix('=') {
            let r = r.trim_start();
            if let Some(q) = r.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let body = r.get(1..).unwrap_or("");
                let close = body.find(q).unwrap_or(body.len());
                val = decode(body.get(..close).unwrap_or(""));
                rest = body.get(close + 1..).unwrap_or("");
            } else {
                let e = r.find(char::is_whitespace).unwrap_or(r.len());
                val = decode(r.get(..e).unwrap_or(""));
                rest = r.get(e..).unwrap_or("");
            }
        }
        if key.is_empty() {
            break;
        }
        out.push((key, val));
    }
    out
}

/// Decode character references.
pub fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(rest.get(..p).unwrap_or(""));
        let after = rest.get(p + 1..).unwrap_or("");
        let semi = after.find(';').filter(|&k| k <= 10);
        let ent = semi.and_then(|k| after.get(..k));
        let c = ent.and_then(|e| match e {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "rsquo" => Some('’'),
            "lsquo" => Some('‘'),
            "rdquo" => Some('”'),
            "ldquo" => Some('“'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            _ => {
                let n = e.strip_prefix('#')?;
                let v = match n.strip_prefix(['x', 'X']) {
                    Some(h) => u32::from_str_radix(h, 16).ok()?,
                    None => n.parse().ok()?,
                };
                char::from_u32(v)
            }
        });
        match (c, semi) {
            (Some(c), Some(k)) => {
                out.push(c);
                rest = after.get(k + 1..).unwrap_or("");
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_loosely() {
        let t = tokens("<p class=\"action\">A &amp; B<br>C</p><!-- x --><style>p{}</style><P CLASS=dialog>&#8217;&bogus;</p>");
        assert_eq!(t[0], Tok::Open("p".into(), vec![("class".into(), "action".into())]));
        assert_eq!(t[1], Tok::Text("A & B".into()));
        assert_eq!(t[2], Tok::Open("br".into(), vec![]));
        assert!(t.contains(&Tok::Open("p".into(), vec![("class".into(), "dialog".into())])));
        assert!(t.contains(&Tok::Text("’&bogus;".into())));
        assert!(!t.iter().any(|x| *x == Tok::Text("p{}".into())));
        let _ = tokens("<<<<>>>&&&#xFFFFFFFF;<p a='b");
    }
}
