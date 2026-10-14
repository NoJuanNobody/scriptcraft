//! Content-stream interpretation: positioned glyphs (with bold/italic from the font) and thin
//! filled rectangles or horizontal strokes (underlines), in points from the page's top-left.

use std::collections::HashMap;

use super::file::{File, PageRef};
use super::font::Font;
use super::obj::{Dict, Lex, Obj};

/// Most glyphs taken from one page.
const MAX_GLYPHS: usize = 200_000;
/// Most operators run on one page (forms included).
const MAX_OPS: usize = 4_000_000;

/// A glyph on the page.
#[derive(Clone, Debug)]
pub struct Glyph {
    /// Left edge and baseline, points from the top-left.
    pub x: f64,
    pub y: f64,
    /// Advance width, points.
    pub w: f64,
    /// Font size on the page, points.
    pub size: f64,
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

/// What a page holds.
#[derive(Default)]
pub struct PageText {
    pub width: f64,
    pub height: f64,
    pub glyphs: Vec<Glyph>,
    /// Underline candidates: `(x0, x1, y)` in points from the top-left.
    pub rules: Vec<(f64, f64, f64)>,
}

type M = [f64; 6];
const ID: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn mul(a: &M, b: &M) -> M {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn apply(m: &M, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

#[derive(Clone)]
struct Gs {
    ctm: M,
    font: Option<usize>,
    size: f64,
    tc: f64,
    tw: f64,
    tz: f64,
    tl: f64,
    rise: f64,
}

struct Run<'a> {
    file: &'a File,
    fonts: Vec<Font>,
    /// Font cache by object identity (resource dict pointer + name).
    font_ids: HashMap<(usize, Vec<u8>), usize>,
    out: PageText,
    page_h: f64,
    page_x0: f64,
    ops: usize,
}

/// The text of one page.
pub fn page_text(f: &File, p: &PageRef) -> PageText {
    let [x0, y0, x1, y1] = p.media;
    let mut r = Run {
        file: f,
        fonts: Vec::new(),
        font_ids: HashMap::new(),
        out: PageText { width: x1 - x0, height: y1 - y0, ..PageText::default() },
        page_h: y1,
        page_x0: x0,
        ops: 0,
    };
    let _ = y0;
    let mut data = Vec::new();
    match f.at(&p.dict, "Contents") {
        Obj::Array(a) => {
            for c in a {
                if let Some(d) = f.decode(c) {
                    data.extend_from_slice(&d);
                    data.push(b'\n');
                }
            }
        }
        _ => {
            if let Some(d) = f.decode(p.dict.get("Contents")) {
                data = d;
            }
        }
    }
    let res = p.resources.as_ref().map(|o| f.get(o)).and_then(Obj::dict).cloned().unwrap_or_default();
    r.exec(&data, &res, ID, 0);
    r.out
}

impl Run<'_> {
    fn font_for(&mut self, res: &Dict, name: &[u8]) -> Option<usize> {
        let fonts = self.file.at(res, "Font").dict()?;
        let key = (fonts as *const Dict as usize, name.to_vec());
        if let Some(&i) = self.font_ids.get(&key) {
            return Some(i);
        }
        let fd = self.file.get(fonts.0.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v)?).dict()?;
        let font = Font::load(self.file, fd);
        self.fonts.push(font);
        let i = self.fonts.len() - 1;
        self.font_ids.insert(key, i);
        Some(i)
    }

    fn exec(&mut self, data: &[u8], res: &Dict, base: M, depth: usize) {
        if depth > 8 {
            return;
        }
        let mut l = Lex::new(data, false);
        let mut st: Vec<Obj> = Vec::new();
        let mut gs = Gs { ctm: base, font: None, size: 12.0, tc: 0.0, tw: 0.0, tz: 1.0, tl: 0.0, rise: 0.0 };
        let mut stack: Vec<Gs> = Vec::new();
        let (mut tm, mut tlm) = (ID, ID);
        // The current path: thin rectangles and horizontal segments (user space).
        let mut rects: Vec<(f64, f64, f64, f64)> = Vec::new();
        // Subpaths as point lists (user space): filled thin shapes are underlines too.
        let mut paths: Vec<Vec<(f64, f64)>> = Vec::new();
        while !l.done() {
            self.ops += 1;
            if self.ops > MAX_OPS {
                return;
            }
            let Some(o) = l.obj() else { continue };
            let Obj::Op(op) = o else {
                if st.len() < 64 {
                    st.push(o);
                }
                continue;
            };
            let n = |i: usize| st.get(i).and_then(Obj::num).unwrap_or(0.0);
            let six = || -> M { [n(0), n(1), n(2), n(3), n(4), n(5)] };
            match op.as_slice() {
                b"q" => {
                    if stack.len() < 64 {
                        stack.push(gs.clone());
                    }
                }
                b"Q" => {
                    if let Some(g) = stack.pop() {
                        gs = g;
                    }
                }
                b"cm" if st.len() >= 6 => gs.ctm = mul(&six(), &gs.ctm),
                b"BT" => {
                    tm = ID;
                    tlm = ID;
                }
                b"Tf" if st.len() >= 2 => {
                    if let Some(name) = st.first().and_then(Obj::name).map(<[u8]>::to_vec) {
                        gs.font = self.font_for(res, &name);
                    }
                    gs.size = n(1);
                }
                b"Tc" => gs.tc = n(0),
                b"Tw" => gs.tw = n(0),
                b"Tz" => gs.tz = n(0) / 100.0,
                b"TL" => gs.tl = n(0),
                b"Ts" => gs.rise = n(0),
                b"Td" | b"TD" if st.len() >= 2 => {
                    if op.as_slice() == b"TD" {
                        gs.tl = -n(1);
                    }
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, n(0), n(1)], &tlm);
                    tm = tlm;
                }
                b"Tm" if st.len() >= 6 => {
                    tlm = six();
                    tm = tlm;
                }
                b"T*" => {
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -gs.tl], &tlm);
                    tm = tlm;
                }
                b"Tj" => {
                    if let Some(Obj::Str(s)) = st.first() {
                        let s = s.clone();
                        self.show(&s, &gs, &mut tm);
                    }
                }
                b"'" | b"\"" => {
                    if op.as_slice() == b"\"" && st.len() >= 3 {
                        gs.tw = n(0);
                        gs.tc = n(1);
                    }
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -gs.tl], &tlm);
                    tm = tlm;
                    if let Some(Obj::Str(s)) = st.last() {
                        let s = s.clone();
                        self.show(&s, &gs, &mut tm);
                    }
                }
                b"TJ" => {
                    if let Some(Obj::Array(a)) = st.first() {
                        let a = a.clone();
                        for item in &a {
                            match item {
                                Obj::Str(s) => self.show(s, &gs, &mut tm),
                                other => {
                                    if let Some(adj) = other.num() {
                                        let tx = -adj / 1000.0 * gs.size * gs.tz;
                                        tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], &tm);
                                    }
                                }
                            }
                        }
                    }
                }
                b"re" if st.len() >= 4 => {
                    rects.push((n(0), n(1), n(2), n(3)));
                }
                b"m" if st.len() >= 2 => {
                    if paths.len() < 4096 {
                        paths.push(vec![(n(0), n(1))]);
                    }
                }
                b"l" if st.len() >= 2 => {
                    if let Some(p) = paths.last_mut()
                        && p.len() < 64
                    {
                        p.push((n(0), n(1)));
                    }
                }
                b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"S" | b"s" => {
                    let fill = !matches!(op.as_slice(), b"S" | b"s");
                    let mut shapes: Vec<Vec<(f64, f64)>> = std::mem::take(&mut paths);
                    shapes.extend(rects.drain(..).map(|(x, y, w, h)| vec![(x, y), (x + w, y + h)]));
                    for pts in shapes {
                        let dev: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| apply(&gs.ctm, x, y)).collect();
                        let (x0, x1) = dev.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
                        let (y0, y1) = dev.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
                        let thin = if fill { y1 - y0 < 2.5 } else { y1 - y0 < 0.5 };
                        if thin && x1 - x0 > 2.0 {
                            self.rule(x0, x1, (y0 + y1) / 2.0);
                        }
                    }
                }
                b"n" => {
                    rects.clear();
                    paths.clear();
                }
                b"Do" => {
                    if let Some(name) = st.first().and_then(Obj::name).map(<[u8]>::to_vec) {
                        self.form(res, &name, &gs, depth);
                    }
                }
                b"ID" => l.skip_inline_image(),
                _ => {}
            }
            st.clear();
        }
    }

    fn form(&mut self, res: &Dict, name: &[u8], gs: &Gs, depth: usize) {
        let file = self.file;
        let Some(xo) = file.at(res, "XObject").dict() else { return };
        let Some(obj) = xo.0.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v) else { return };
        let Some(d) = file.get(obj).dict() else { return };
        if !file.at(d, "Subtype").is_name("Form") {
            return;
        }
        let m: M = match file.at(d, "Matrix").array() {
            Some(a) if a.len() >= 6 => {
                let v = |i: usize| a.get(i).and_then(|x| file.get(x).num()).unwrap_or(0.0);
                [v(0), v(1), v(2), v(3), v(4), v(5)]
            }
            _ => ID,
        };
        let inner = match file.at(d, "Resources") {
            Obj::Dict(r) => r.clone(),
            _ => res.clone(),
        };
        if let Some(data) = file.decode(obj) {
            self.exec(&data, &inner, mul(&m, &gs.ctm), depth + 1);
        }
    }

    fn rule(&mut self, x0: f64, x1: f64, y: f64) {
        if self.out.rules.len() < MAX_GLYPHS {
            self.out.rules.push((x0 - self.page_x0, x1 - self.page_x0, self.page_h - y));
        }
    }

    fn show(&mut self, s: &[u8], gs: &Gs, tm: &mut M) {
        let Some(fi) = gs.font else { return };
        let Some(font) = self.fonts.get(fi) else { return };
        let codes = font.codes(s);
        for (code, len) in codes {
            let Some(font) = self.fonts.get(fi) else { return };
            let w0 = font.width(code);
            let trm = mul(&[gs.size * gs.tz, 0.0, 0.0, gs.size, 0.0, gs.rise], &mul(tm, &gs.ctm));
            let (x, y) = (trm[4], trm[5]);
            let space = if len == 1 && code == 32 { gs.tw } else { 0.0 };
            let tx = (w0 * gs.size + gs.tc + space) * gs.tz;
            let (ex, _) = apply(&mul(tm, &gs.ctm), tx, 0.0);
            let (sx, _) = apply(&mul(tm, &gs.ctm), 0.0, 0.0);
            let size = (trm[2] * trm[2] + trm[3] * trm[3]).sqrt();
            if self.out.glyphs.len() < MAX_GLYPHS && x.is_finite() && y.is_finite() && size.is_finite() {
                let text = font.text(code);
                self.out.glyphs.push(Glyph {
                    x: x - self.page_x0,
                    y: self.page_h - y,
                    w: (ex - sx).abs(),
                    size,
                    text,
                    bold: font.bold,
                    italic: font.italic,
                });
            }
            *tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], tm);
        }
    }
}
