//! A PDF file's objects, collected by scanning for `n g obj` (so damaged or missing cross-reference
//! tables don't matter), plus objects inside object streams; stream filters; the page tree.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use super::obj::{Dict, Lex, NULL, Obj, is_delim, is_ws};

/// Largest decoded stream.
const MAX_STREAM: usize = 64 << 20;
/// Most pages read.
pub const MAX_PAGES: usize = 5_000;

/// All objects of a file.
pub struct File {
    objs: HashMap<u32, Obj>,
    trailer_root: Option<u32>,
}

/// One page: its dictionary with inherited resources and media box.
pub struct PageRef {
    pub dict: Dict,
    pub resources: Option<Obj>,
    /// `[x0, y0, x1, y1]` in points.
    pub media: [f64; 4],
}

impl File {
    pub fn parse(b: &[u8]) -> Result<File, String> {
        if !b.windows(5).take(1024).any(|w| w == b"%PDF-") {
            return Err("not a PDF file".into());
        }
        let mut f = File { objs: HashMap::new(), trailer_root: None };
        f.scan(b);
        if f.objs.is_empty() {
            return Err("no readable objects in the PDF".into());
        }
        f.object_streams();
        f.trailer_root = trailer_root(b, &f);
        let encrypted = f.objs.values().any(|o| {
            o.dict().is_some_and(|d| {
                (d.get("Filter").is_name("Standard") || d.get("Type").is_name("XRef"))
                    && !matches!(d.get("O"), Obj::Null)
                    && !matches!(d.get("U"), Obj::Null)
            })
        }) || find(b, b"/Encrypt").is_some();
        if encrypted {
            return Err("this PDF is encrypted; ScriptCraft can't import it (print it to a new PDF first)".into());
        }
        Ok(f)
    }

    /// Follow references.
    pub fn get<'a>(&'a self, mut o: &'a Obj) -> &'a Obj {
        for _ in 0..16 {
            match o {
                Obj::Ref(n) => o = self.objs.get(n).unwrap_or(&NULL),
                _ => return o,
            }
        }
        &NULL
    }

    /// A dictionary entry, resolved.
    pub fn at<'a>(&'a self, d: &'a Dict, k: &str) -> &'a Obj {
        self.get(d.get(k))
    }

    fn scan(&mut self, b: &[u8]) {
        let mut i = 0;
        while let Some(off) = find(b.get(i..).unwrap_or(&[]), b"obj") {
            let at = i + off;
            i = at + 3;
            if b.get(at + 3).is_some_and(|&c| !is_ws(c) && !is_delim(c)) {
                continue;
            }
            let Some(num) = header_number(b, at) else { continue };
            let mut l = Lex::new(b, true);
            l.i = at + 3;
            let Some(o) = l.obj() else { continue };
            l.skip_ws();
            let o = if l.at_keyword(b"stream") {
                let Obj::Dict(d) = o else { continue };
                let mut s = l.i + 6;
                if b.get(s) == Some(&b'\r') {
                    s += 1;
                }
                if b.get(s) == Some(&b'\n') {
                    s += 1;
                }
                let declared = d.get("Length").int().and_then(|n| usize::try_from(n).ok());
                let end = declared
                    .filter(|&n| {
                        let e = s.saturating_add(n);
                        let mut k = Lex::new(b, false);
                        k.i = e;
                        k.skip_ws();
                        e <= b.len() && k.at_keyword(b"endstream")
                    })
                    .map(|n| s + n)
                    .or_else(|| {
                        let e = s + find(b.get(s..).unwrap_or(&[]), b"endstream")?;
                        let mut e2 = e;
                        if e2 > s && b.get(e2 - 1) == Some(&b'\n') {
                            e2 -= 1;
                        }
                        if e2 > s && b.get(e2 - 1) == Some(&b'\r') {
                            e2 -= 1;
                        }
                        Some(e2)
                    })
                    .unwrap_or(b.len());
                let data = b.get(s..end.max(s)).unwrap_or(&[]).to_vec();
                i = end.max(i);
                Obj::Stream(d, data)
            } else {
                i = l.i.max(i);
                o
            };
            self.objs.insert(num, o);
        }
    }

    fn object_streams(&mut self) {
        let streams: Vec<Obj> = self.objs.values().filter(|o| matches!(o, Obj::Stream(d, _) if d.get("Type").is_name("ObjStm"))).cloned().collect();
        for s in streams {
            let Obj::Stream(d, _) = &s else { continue };
            let Some(data) = self.decode(&s) else { continue };
            let n = d.get("N").int().unwrap_or(0).clamp(0, 1 << 20) as usize;
            let first = d.get("First").int().unwrap_or(0).max(0) as usize;
            let mut head = Lex::new(data.get(..first.min(data.len())).unwrap_or(&[]), false);
            for _ in 0..n {
                let (Some(num), Some(off)) = (head.obj().and_then(|o| o.int()), head.obj().and_then(|o| o.int())) else { break };
                let (Ok(num), Ok(off)) = (u32::try_from(num), usize::try_from(off)) else { continue };
                let mut l = Lex::new(&data, true);
                l.i = first.saturating_add(off);
                if let Some(o) = l.obj() {
                    self.objs.entry(num).or_insert(o);
                }
            }
        }
    }

    /// The decoded bytes of a stream (filters applied), or `None` for an unsupported filter.
    pub fn decode(&self, o: &Obj) -> Option<Vec<u8>> {
        let Obj::Stream(d, raw) = self.get(o) else { return None };
        let filters: Vec<Vec<u8>> = match self.at(d, "Filter") {
            Obj::Name(n) => vec![n.clone()],
            Obj::Array(a) => a.iter().filter_map(|f| self.get(f).name().map(<[u8]>::to_vec)).collect(),
            _ => Vec::new(),
        };
        let parms = self.at(d, "DecodeParms");
        let mut data = raw.clone();
        for (k, f) in filters.iter().enumerate() {
            let p = match parms {
                Obj::Array(a) => a.get(k).map_or(&NULL, |x| self.get(x)),
                other => other,
            };
            data = match f.as_slice() {
                b"FlateDecode" | b"Fl" => predict(inflate(&data)?, p.dict()),
                b"ASCIIHexDecode" | b"AHx" => ascii_hex(&data),
                b"ASCII85Decode" | b"A85" => ascii85(&data),
                _ => return None,
            };
        }
        Some(data)
    }

    /// The pages, in order.
    pub fn pages(&self) -> Vec<PageRef> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let root = self
            .trailer_root
            .and_then(|r| self.objs.get(&r))
            .or_else(|| self.objs.values().find(|o| o.dict().is_some_and(|d| d.get("Type").is_name("Catalog"))));
        if let Some(pages) = root.and_then(|c| c.dict()).map(|c| c.get("Pages")) {
            self.walk(pages, None, [0.0, 0.0, 612.0, 792.0], &mut out, &mut seen, 0);
        }
        if out.is_empty() {
            // No usable page tree: every page object in number order.
            let mut nums: Vec<&u32> =
                self.objs.iter().filter(|(_, o)| o.dict().is_some_and(|d| d.get("Type").is_name("Page"))).map(|(n, _)| n).collect();
            nums.sort();
            for n in nums.into_iter().take(MAX_PAGES) {
                if let Some(d) = self.objs.get(n).and_then(Obj::dict) {
                    out.push(PageRef {
                        dict: d.clone(),
                        resources: Some(self.at(d, "Resources").clone()),
                        media: self.media(d).unwrap_or([0.0, 0.0, 612.0, 792.0]),
                    });
                }
            }
        }
        out
    }

    fn media(&self, d: &Dict) -> Option<[f64; 4]> {
        let a = self.at(d, "MediaBox").array()?;
        let v: Vec<f64> = a.iter().filter_map(|x| self.get(x).num()).collect();
        let (x0, y0, x1, y1) = (*v.first()?, *v.get(1)?, *v.get(2)?, *v.get(3)?);
        let r = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
        (r[2] - r[0] > 1.0 && r[3] - r[1] > 1.0).then_some(r)
    }

    fn walk(&self, node: &Obj, res: Option<&Obj>, media: [f64; 4], out: &mut Vec<PageRef>, seen: &mut HashSet<u32>, depth: usize) {
        if depth > 64 || out.len() >= MAX_PAGES {
            return;
        }
        if let Obj::Ref(n) = node
            && !seen.insert(*n)
        {
            return;
        }
        let Some(d) = self.get(node).dict() else { return };
        let res = match self.at(d, "Resources") {
            Obj::Null => res,
            r => Some(r),
        };
        let media = self.media(d).unwrap_or(media);
        match self.at(d, "Kids") {
            Obj::Array(kids) if !d.get("Type").is_name("Page") => {
                for k in kids {
                    self.walk(k, res, media, out, seen, depth + 1);
                }
            }
            _ => out.push(PageRef { dict: d.clone(), resources: res.cloned(), media }),
        }
    }
}

/// The object number before `obj` at `at` (`… 12 0 obj`).
fn header_number(b: &[u8], at: usize) -> Option<u32> {
    let mut j = at;
    let back_ws = |j: &mut usize| {
        while *j > 0 && b.get(*j - 1).is_some_and(|&c| is_ws(c)) {
            *j -= 1;
        }
    };
    let back_digits = |j: &mut usize| -> Option<usize> {
        let end = *j;
        while *j > 0 && b.get(*j - 1).is_some_and(u8::is_ascii_digit) {
            *j -= 1;
        }
        (end > *j && end - *j <= 10).then_some(end)
    };
    back_ws(&mut j);
    back_digits(&mut j)?;
    if j == at || !b.get(j.saturating_sub(1)).is_some_and(|&c| is_ws(c)) {
        return None;
    }
    back_ws(&mut j);
    let end = back_digits(&mut j)?;
    if j > 0 && b.get(j - 1).is_some_and(|&c| !is_ws(c) && !is_delim(c)) {
        return None;
    }
    std::str::from_utf8(b.get(j..end)?).ok()?.parse().ok()
}

fn trailer_root(b: &[u8], f: &File) -> Option<u32> {
    // The last `trailer` dictionary, else the last cross-reference stream.
    if let Some(at) = rfind(b, b"trailer") {
        let mut l = Lex::new(b, true);
        l.i = at + 7;
        if let Some(Obj::Dict(d)) = l.obj()
            && let Obj::Ref(r) = d.get("Root")
        {
            return Some(*r);
        }
    }
    f.objs.iter().filter(|(_, o)| o.dict().is_some_and(|d| d.get("Type").is_name("XRef"))).max_by_key(|(n, _)| **n).and_then(|(_, o)| {
        match o.dict()?.get("Root") {
            Obj::Ref(r) => Some(*r),
            _ => None,
        }
    })
}

pub fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    let first = *n.first()?;
    let mut i = 0;
    while i + n.len() <= h.len() {
        let p = h.get(i..).and_then(|s| s.iter().position(|&c| c == first))?;
        i += p;
        if h.get(i..i + n.len()) == Some(n) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    if h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).rev().find(|&i| h.get(i..i + n.len()) == Some(n))
}

fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let r = flate2::read::ZlibDecoder::new(data).take(MAX_STREAM as u64).read_to_end(&mut out);
    if r.is_err() && out.is_empty() {
        out.clear();
        let r2 = flate2::read::DeflateDecoder::new(data).take(MAX_STREAM as u64).read_to_end(&mut out);
        if r2.is_err() && out.is_empty() {
            return None;
        }
    }
    Some(out)
}

/// PNG predictors (`/Predictor` 10–15); TIFF predictor 2 is left as is.
fn predict(data: Vec<u8>, parms: Option<&Dict>) -> Vec<u8> {
    let Some(p) = parms else { return data };
    let pred = p.get("Predictor").int().unwrap_or(1);
    if pred < 10 {
        return data;
    }
    let colors = p.get("Colors").int().unwrap_or(1).clamp(1, 4) as usize;
    let bpc = p.get("BitsPerComponent").int().unwrap_or(8).clamp(1, 16) as usize;
    let cols = p.get("Columns").int().unwrap_or(1).clamp(1, 1 << 16) as usize;
    let bpp = (colors * bpc).div_ceil(8).max(1);
    let row = (colors * bpc * cols).div_ceil(8);
    let mut out = Vec::with_capacity(data.len());
    let mut prev = vec![0u8; row];
    for chunk in data.chunks(row + 1) {
        let Some((&kind, line)) = chunk.split_first() else { break };
        let mut cur = vec![0u8; row];
        for (i, &x) in line.iter().enumerate() {
            let a = if i >= bpp { cur.get(i - bpp).copied().unwrap_or(0) } else { 0 };
            let b = prev.get(i).copied().unwrap_or(0);
            let c = if i >= bpp { prev.get(i - bpp).copied().unwrap_or(0) } else { 0 };
            let v = match kind {
                1 => x.wrapping_add(a),
                2 => x.wrapping_add(b),
                3 => x.wrapping_add(((u16::from(a) + u16::from(b)) / 2) as u8),
                4 => {
                    let p = i16::from(a) + i16::from(b) - i16::from(c);
                    let (pa, pb, pc) = ((p - i16::from(a)).abs(), (p - i16::from(b)).abs(), (p - i16::from(c)).abs());
                    x.wrapping_add(if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    })
                }
                _ => x,
            };
            if let Some(slot) = cur.get_mut(i) {
                *slot = v;
            }
        }
        out.extend_from_slice(cur.get(..line.len().min(row)).unwrap_or(&[]));
        prev = cur;
    }
    out
}

fn ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut l = Lex::new(b"", false);
    let mut v = Vec::with_capacity(data.len() + 2);
    v.push(b'<');
    v.extend(data.iter().take_while(|&&c| c != b'>'));
    v.push(b'>');
    l.b = &v;
    match l.obj() {
        Some(Obj::Str(s)) => s,
        _ => Vec::new(),
    }
}

fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut group: Vec<u32> = Vec::with_capacity(5);
    let body = data.strip_prefix(b"<~").unwrap_or(data);
    for &c in body {
        match c {
            b'~' => break,
            b'z' if group.is_empty() => out.extend_from_slice(&[0; 4]),
            b'!'..=b'u' => {
                group.push(u32::from(c - b'!'));
                if group.len() == 5 {
                    let v = group.iter().fold(0u64, |acc, &d| acc * 85 + u64::from(d));
                    out.extend_from_slice(&(v as u32).to_be_bytes());
                    group.clear();
                }
            }
            _ => {}
        }
    }
    if group.len() > 1 {
        let n = group.len();
        while group.len() < 5 {
            group.push(84);
        }
        let v = group.iter().fold(0u64, |acc, &d| acc * 85 + u64::from(d));
        out.extend_from_slice((v as u32).to_be_bytes().get(..n - 1).unwrap_or(&[]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_decode() {
        assert_eq!(ascii_hex(b"414 2>"), b"AB");
        assert_eq!(ascii85(b"<~87cURD]i,\"Ebo80~>"), b"Hello World!");
    }

    #[test]
    fn object_numbers() {
        let b = b"x\n12 0 obj << >> endobj";
        let at = find(b, b"obj").unwrap();
        assert_eq!(header_number(b, at), Some(12));
        assert_eq!(header_number(b"a12 0 obj", 6), None);
    }
}
