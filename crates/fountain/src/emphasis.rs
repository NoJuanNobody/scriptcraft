//! Fountain emphasis: `*italic*`, `**bold**`, `***bold italic***`, `_underline_`; `\*` and `\_`
//! are literal. An opening marker with no matching closer later in the paragraph is literal.

use scriptcraft_doc::{Para, Style};

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Ch(char),
    Star(usize),
    Under,
}

fn tokens(s: &str) -> Vec<Tok> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::with_capacity(cs.len());
    let mut i = 0;
    while let Some(&c) = cs.get(i) {
        match c {
            '\\' if matches!(cs.get(i + 1), Some('*' | '_')) => {
                if let Some(&n) = cs.get(i + 1) {
                    out.push(Tok::Ch(n));
                }
                i += 2;
            }
            '*' => {
                let mut n = 0;
                while cs.get(i + n) == Some(&'*') {
                    n += 1;
                }
                // Runs longer than three: the extra stars are literal.
                for _ in 3..n {
                    out.push(Tok::Ch('*'));
                }
                out.push(Tok::Star(n.min(3)));
                i += n;
            }
            '_' => {
                out.push(Tok::Under);
                i += 1;
            }
            c => {
                out.push(Tok::Ch(c));
                i += 1;
            }
        }
    }
    out
}

/// Styled runs of a paragraph's raw Fountain text.
pub fn parse(s: &str) -> Vec<(String, Style)> {
    if !s.contains(['*', '_', '\\']) {
        return vec![(s.to_string(), Style::PLAIN)];
    }
    let toks = tokens(s);
    let mut runs: Vec<(String, Style)> = Vec::new();
    let mut st = Style::PLAIN;
    let push = |c: &str, st: Style, runs: &mut Vec<(String, Style)>| match runs.last_mut() {
        Some((t, s)) if *s == st => t.push_str(c),
        _ => runs.push((c.to_string(), st)),
    };
    for (k, t) in toks.iter().enumerate() {
        let later = |t: &Tok| toks.get(k + 1..).is_some_and(|r| r.contains(t));
        match t {
            Tok::Ch(c) => push(c.encode_utf8(&mut [0; 4]), st, &mut runs),
            Tok::Under if st.underline => st.underline = false,
            Tok::Under if later(t) => st.underline = true,
            Tok::Star(3) if st.bold && st.italic => (st.bold, st.italic) = (false, false),
            Tok::Star(3) if !st.bold && !st.italic && later(t) => (st.bold, st.italic) = (true, true),
            Tok::Star(2) if st.bold => st.bold = false,
            Tok::Star(2) if !st.bold && later(t) => st.bold = true,
            Tok::Star(1) if st.italic => st.italic = false,
            Tok::Star(1) if !st.italic && later(t) => st.italic = true,
            Tok::Under => push("_", st, &mut runs),
            Tok::Star(n) => push(&"*".repeat(*n), st, &mut runs),
        }
    }
    if runs.is_empty() {
        runs.push((String::new(), Style::PLAIN));
    }
    runs
}

/// Re-read a paragraph's text as Fountain emphasis (other fields kept).
pub fn apply(p: &mut Para) {
    let runs = parse(&p.text);
    let q = Para::styled(p.element, &runs);
    p.text = q.text;
    p.spans = q.spans;
}

fn escape(s: &str) -> String {
    s.replace('*', "\\*").replace('_', "\\_")
}

/// A paragraph's text with emphasis markers and escapes.
pub fn write(p: &Para) -> String {
    let mut out = String::new();
    for (t, st) in p.runs() {
        if t.is_empty() {
            continue;
        }
        // Markers hug the text: surrounding spaces go outside them.
        let core = t.trim();
        let (lead, trail) = (&t[..t.len() - t.trim_start().len()], &t[t.trim_end().len()..]);
        if core.is_empty() || st.is_plain() {
            out.push_str(&escape(t));
            continue;
        }
        let stars = match (st.bold, st.italic) {
            (true, true) => "***",
            (true, false) => "**",
            (false, true) => "*",
            _ => "",
        };
        let u = if st.underline { "_" } else { "" };
        out.push_str(&format!("{lead}{u}{stars}{}{stars}{u}{trail}", escape(core)));
    }
    out
}
