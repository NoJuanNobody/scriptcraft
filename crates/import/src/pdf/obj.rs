//! PDF objects and a tolerant, panic-free lexer for both file syntax and content streams.

/// A PDF object.
#[derive(Clone, Debug, PartialEq)]
pub enum Obj {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Name(Vec<u8>),
    Str(Vec<u8>),
    Array(Vec<Obj>),
    Dict(Dict),
    Ref(u32),
    Stream(Dict, Vec<u8>),
    /// A bare keyword (content-stream operators).
    Op(Vec<u8>),
}

pub static NULL: Obj = Obj::Null;

/// A dictionary (keys without the slash).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dict(pub Vec<(Vec<u8>, Obj)>);

impl Dict {
    pub fn get(&self, k: &str) -> &Obj {
        self.0.iter().rev().find(|(key, _)| key == k.as_bytes()).map_or(&NULL, |(_, v)| v)
    }
}

impl Obj {
    pub fn num(&self) -> Option<f64> {
        match self {
            Obj::Int(i) => Some(*i as f64),
            Obj::Real(r) if r.is_finite() => Some(*r),
            _ => None,
        }
    }
    pub fn int(&self) -> Option<i64> {
        match self {
            Obj::Int(i) => Some(*i),
            Obj::Real(r) if r.is_finite() => Some(*r as i64),
            _ => None,
        }
    }
    pub fn name(&self) -> Option<&[u8]> {
        match self {
            Obj::Name(n) => Some(n),
            _ => None,
        }
    }
    pub fn is_name(&self, n: &str) -> bool {
        self.name() == Some(n.as_bytes())
    }
    pub fn dict(&self) -> Option<&Dict> {
        match self {
            Obj::Dict(d) | Obj::Stream(d, _) => Some(d),
            _ => None,
        }
    }
    pub fn array(&self) -> Option<&[Obj]> {
        match self {
            Obj::Array(a) => Some(a),
            _ => None,
        }
    }
}

const MAX_DEPTH: usize = 48;
/// Most elements in one array or dictionary.
const MAX_ITEMS: usize = 1 << 20;

pub fn is_ws(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

pub fn is_delim(c: u8) -> bool {
    matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

/// A lexer over PDF bytes.
pub struct Lex<'a> {
    pub b: &'a [u8],
    pub i: usize,
    /// File syntax: `n g R` is a reference. Off in content streams.
    pub refs: bool,
}

impl<'a> Lex<'a> {
    pub fn new(b: &'a [u8], refs: bool) -> Lex<'a> {
        Lex { b, i: 0, refs }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn at(&self, j: usize) -> Option<u8> {
        self.b.get(self.i.saturating_add(j)).copied()
    }
    pub fn done(&self) -> bool {
        self.i >= self.b.len()
    }

    pub fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if is_ws(c) {
                self.i += 1;
            } else if c == b'%' {
                while let Some(c) = self.peek() {
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    /// The bytes of a regular token (no delimiters or whitespace).
    fn word(&mut self) -> &'a [u8] {
        let s = self.i;
        while let Some(c) = self.peek() {
            if is_ws(c) || is_delim(c) {
                break;
            }
            self.i += 1;
        }
        self.b.get(s..self.i).unwrap_or(&[])
    }

    /// Whether the next token is this keyword (without consuming it).
    pub fn at_keyword(&self, k: &[u8]) -> bool {
        let end = self.i.saturating_add(k.len());
        self.b.get(self.i..end) == Some(k) && self.b.get(end).is_none_or(|&c| is_ws(c) || is_delim(c))
    }

    /// The next object, or `None` at the end or on a stray delimiter (which is skipped).
    pub fn obj(&mut self) -> Option<Obj> {
        self.obj_at(0)
    }

    fn obj_at(&mut self, depth: usize) -> Option<Obj> {
        self.skip_ws();
        let c = self.peek()?;
        if depth > MAX_DEPTH {
            self.i += 1;
            return None;
        }
        match c {
            b'/' => {
                self.i += 1;
                let w = self.word();
                Some(Obj::Name(unescape_name(w)))
            }
            b'(' => Some(Obj::Str(self.literal())),
            b'<' if self.at(1) == Some(b'<') => {
                self.i += 2;
                Some(Obj::Dict(self.dict_body(depth)))
            }
            b'<' => Some(Obj::Str(self.hex())),
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        None => break,
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        _ => {}
                    }
                    let before = self.i;
                    match self.obj_at(depth + 1) {
                        Some(o) if v.len() < MAX_ITEMS => v.push(o),
                        Some(_) => {}
                        None if self.i == before => self.i += 1,
                        None => {}
                    }
                }
                Some(Obj::Array(v))
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => Some(self.number()),
            b')' | b'>' | b']' | b'{' | b'}' => {
                self.i += 1;
                None
            }
            _ => {
                let w = self.word();
                if w.is_empty() {
                    self.i += 1;
                    return None;
                }
                Some(match w {
                    b"true" => Obj::Bool(true),
                    b"false" => Obj::Bool(false),
                    b"null" => Obj::Null,
                    _ => Obj::Op(w.to_vec()),
                })
            }
        }
    }

    fn dict_body(&mut self, depth: usize) -> Dict {
        let mut d = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(b'>') if self.at(1) == Some(b'>') => {
                    self.i += 2;
                    break;
                }
                Some(b'/') => {
                    self.i += 1;
                    let k = unescape_name(self.word());
                    let before = self.i;
                    let v = self.obj_at(depth + 1).unwrap_or(Obj::Null);
                    if self.i == before && self.peek().is_some() {
                        self.i += 1;
                    }
                    if d.len() < MAX_ITEMS {
                        d.push((k, v));
                    }
                }
                Some(_) => {
                    // A value without a key: skip it.
                    let before = self.i;
                    let _ = self.obj_at(depth + 1);
                    if self.i == before {
                        self.i += 1;
                    }
                }
            }
        }
        Dict(d)
    }

    fn number(&mut self) -> Obj {
        let w = self.word();
        let s = std::str::from_utf8(w).unwrap_or("0");
        let n = if s.contains('.') {
            Obj::Real(s.parse::<f64>().unwrap_or_else(|_| lenient_real(s)))
        } else {
            match s.parse::<i64>() {
                Ok(i) => Obj::Int(i),
                Err(_) => Obj::Real(lenient_real(s)),
            }
        };
        // `n g R` (file syntax only).
        if self.refs
            && let Obj::Int(num) = n
        {
            let save = self.i;
            self.skip_ws();
            let g = self.word();
            if !g.is_empty() && g.iter().all(u8::is_ascii_digit) {
                self.skip_ws();
                if self.peek() == Some(b'R') && self.at(1).is_none_or(|c| is_ws(c) || is_delim(c)) {
                    self.i += 1;
                    return Obj::Ref(u32::try_from(num).unwrap_or(0));
                }
            }
            self.i = save;
        }
        n
    }

    fn literal(&mut self) -> Vec<u8> {
        self.i += 1;
        let mut out = Vec::new();
        let mut depth = 1usize;
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let Some(e) = self.peek() else { break };
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.i += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }

    fn hex(&mut self) -> Vec<u8> {
        self.i += 1;
        let mut digits = Vec::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == b'>' {
                break;
            }
            if let Some(v) = hex_val(c) {
                digits.push(v);
            }
        }
        if digits.len() % 2 == 1 {
            digits.push(0);
        }
        digits.chunks(2).map(|p| (p.first().copied().unwrap_or(0) << 4) | p.get(1).copied().unwrap_or(0)).collect()
    }

    /// Skip inline image data after `ID` (up to `EI`).
    pub fn skip_inline_image(&mut self) {
        let b = self.b;
        let mut j = self.i.saturating_add(1);
        while j + 2 <= b.len() {
            if b.get(j) == Some(&b'E')
                && b.get(j + 1) == Some(&b'I')
                && b.get(j.wrapping_sub(1)).is_some_and(|&c| is_ws(c))
                && b.get(j + 2).is_none_or(|&c| is_ws(c))
            {
                self.i = j + 2;
                return;
            }
            j += 1;
        }
        self.i = b.len();
    }
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn unescape_name(w: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(w.len());
    let mut i = 0;
    while let Some(&c) = w.get(i) {
        if c == b'#'
            && let (Some(h), Some(l)) = (w.get(i + 1).copied().and_then(hex_val), w.get(i + 2).copied().and_then(hex_val))
        {
            out.push((h << 4) | l);
            i += 3;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Numbers like `--5` or `1.2.3` that some writers emit.
fn lenient_real(s: &str) -> f64 {
    let neg = s.starts_with('-');
    let body: String = s.trim_start_matches(['+', '-']).chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let mut parts = body.splitn(3, '.');
    let a = parts.next().unwrap_or("");
    let b = parts.next().unwrap_or("");
    let v = format!("{}.{}", if a.is_empty() { "0" } else { a }, if b.is_empty() { "0" } else { b }).parse::<f64>().unwrap_or(0.0);
    if neg { -v } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_objects() {
        let mut l = Lex::new(b"<< /Type /Page /Kids [1 0 R 2 0 R] /N -1.5 /S (a\\(b\\)c\\101) /H <4142> >>", true);
        let Some(Obj::Dict(d)) = l.obj() else { panic!() };
        assert!(d.get("Type").is_name("Page"));
        assert_eq!(d.get("Kids"), &Obj::Array(vec![Obj::Ref(1), Obj::Ref(2)]));
        assert_eq!(d.get("N").num(), Some(-1.5));
        assert_eq!(d.get("S"), &Obj::Str(b"a(b)cA".to_vec()));
        assert_eq!(d.get("H"), &Obj::Str(b"AB".to_vec()));
    }

    #[test]
    fn garbage_never_loops_or_panics() {
        for src in [&b"<<<<<<[[[[(((("[..], b"]]]>>>>)))}}{{", b"/#zz#4 1 0 R R R", b"<< /A", b"(\\", b"<4", b"1.2.3 --4 +", b"[1 2"] {
            let mut l = Lex::new(src, true);
            let mut n = 0;
            while !l.done() && n < 1000 {
                let _ = l.obj();
                n += 1;
            }
            assert!(l.done());
        }
        let deep = vec![b'['; 100_000];
        let mut l = Lex::new(&deep, true);
        let _ = l.obj();
    }
}
