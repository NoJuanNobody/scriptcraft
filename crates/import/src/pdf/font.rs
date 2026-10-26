//! Fonts as far as text extraction needs them: how to split a string into codes, the Unicode text
//! of each code (ToUnicode CMap, else the simple-font encoding), glyph widths, bold/italic.

use std::collections::HashMap;

use super::file::File;
use super::obj::{Dict, Lex, Obj};

/// Most CMap entries kept.
const MAX_MAP: usize = 1 << 20;

pub struct Font {
    /// Code lengths in bytes from the codespace ranges (Type0), or 1 for simple fonts.
    spaces: Vec<(u32, u32, usize)>,
    two_byte: bool,
    to_unicode: HashMap<u32, String>,
    simple: Option<[char; 256]>,
    widths: HashMap<u32, f64>,
    default_width: f64,
    /// Glyph space to text space (1/1000, or a Type 3 font matrix).
    scale: f64,
    pub bold: bool,
    pub italic: bool,
}

impl Font {
    pub fn load(f: &File, d: &Dict) -> Font {
        let sub = f.at(d, "Subtype").name().unwrap_or(b"").to_vec();
        let base = String::from_utf8_lossy(f.at(d, "BaseFont").name().unwrap_or(b"")).to_lowercase();
        let type0 = sub == b"Type0";
        let mut font = Font {
            spaces: Vec::new(),
            two_byte: type0,
            to_unicode: HashMap::new(),
            simple: None,
            widths: HashMap::new(),
            default_width: if base.contains("courier") { 600.0 } else { 0.0 },
            scale: 0.001,
            bold: base.contains("bold") || base.contains("black") || base.contains("heavy") || base.contains("semibold"),
            italic: base.contains("italic") || base.contains("oblique"),
        };
        if let Some(data) = f.decode(d.get("ToUnicode")) {
            parse_cmap(&data, &mut font);
        }
        let desc_holder;
        let desc: Option<&Dict> = if type0 {
            let cid = f.at(d, "DescendantFonts").array().and_then(|a| a.first()).map(|x| f.get(x));
            if let Some(cd) = cid.and_then(Obj::dict) {
                font.default_width = f.at(cd, "DW").num().unwrap_or(1000.0);
                cid_widths(f, f.at(cd, "W"), &mut font.widths);
                desc_holder = f.at(cd, "FontDescriptor").dict().cloned();
                desc_holder.as_ref()
            } else {
                None
            }
        } else {
            font.simple = Some(simple_encoding(f, f.at(d, "Encoding"), &base));
            let first = f.at(d, "FirstChar").int().unwrap_or(0).clamp(0, 255) as u32;
            if let Some(w) = f.at(d, "Widths").array() {
                for (i, x) in w.iter().take(256).enumerate() {
                    if let Some(v) = f.get(x).num() {
                        font.widths.insert(first + i as u32, v);
                    }
                }
            }
            if sub == b"Type3"
                && let Some(m) = f.at(d, "FontMatrix").array().and_then(|a| a.first()).and_then(|x| f.get(x).num())
            {
                font.scale = m.abs().min(1.0);
            }
            desc_holder = f.at(d, "FontDescriptor").dict().cloned();
            desc_holder.as_ref()
        };
        if let Some(desc) = desc {
            let flags = f.at(desc, "Flags").int().unwrap_or(0);
            font.italic |= flags & (1 << 6) != 0 || f.at(desc, "ItalicAngle").num().is_some_and(|a| a.abs() > 1.0);
            font.bold |= f.at(desc, "FontWeight").num().is_some_and(|w| w >= 600.0);
            if font.default_width == 0.0 {
                font.default_width = f.at(desc, "MissingWidth").num().unwrap_or(0.0);
            }
        }
        if font.default_width == 0.0 {
            font.default_width = 500.0;
        }
        font
    }

    /// Split a string operand into codes.
    pub fn codes(&self, s: &[u8]) -> Vec<(u32, usize)> {
        let mut out = Vec::with_capacity(s.len());
        let mut i = 0;
        while i < s.len() {
            let mut len = if self.two_byte { 2 } else { 1 };
            if !self.spaces.is_empty() {
                for n in 1..=4usize {
                    let code = code_of(s, i, n);
                    if self.spaces.iter().any(|&(lo, hi, l)| l == n && code >= lo && code <= hi) {
                        len = n;
                        break;
                    }
                }
            }
            let len = len.min(s.len() - i).max(1);
            out.push((code_of(s, i, len), len));
            i += len;
        }
        out
    }

    /// The text of a code.
    pub fn text(&self, code: u32) -> String {
        if let Some(t) = self.to_unicode.get(&code) {
            return t.clone();
        }
        if let Some(tab) = &self.simple {
            return tab.get(code as usize).map(|c| c.to_string()).unwrap_or_default();
        }
        // A CID font without a ToUnicode map: nothing reliable to say.
        String::new()
    }

    /// The advance of a code in text space (before the font size).
    pub fn width(&self, code: u32) -> f64 {
        self.widths.get(&code).copied().unwrap_or(self.default_width) * self.scale
    }
}

fn code_of(s: &[u8], i: usize, n: usize) -> u32 {
    s.get(i..(i + n).min(s.len())).unwrap_or(&[]).iter().fold(0u32, |acc, &b| (acc << 8) | u32::from(b))
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> =
        b.chunks(2).map(|c| (u16::from(c.first().copied().unwrap_or(0)) << 8) | u16::from(c.get(1).copied().unwrap_or(0))).collect();
    String::from_utf16_lossy(&units).replace('\u{FFFD}', "")
}

fn parse_cmap(data: &[u8], font: &mut Font) {
    let mut l = Lex::new(data, false);
    let mut stack: Vec<Obj> = Vec::new();
    let mut guard = 0usize;
    while !l.done() && guard < 4_000_000 {
        guard += 1;
        let Some(o) = l.obj() else { continue };
        let Obj::Op(op) = &o else {
            stack.push(o);
            if stack.len() > 4096 {
                stack.clear();
            }
            continue;
        };
        match op.as_slice() {
            b"endcodespacerange" => {
                for p in stack.chunks(2) {
                    if let (Some(Obj::Str(lo)), Some(Obj::Str(hi))) = (p.first(), p.get(1)) {
                        font.spaces.push((code_of(lo, 0, lo.len()), code_of(hi, 0, hi.len()), lo.len().clamp(1, 4)));
                    }
                }
            }
            b"endbfchar" => {
                for p in stack.chunks(2) {
                    if let (Some(Obj::Str(src)), Some(Obj::Str(dst))) = (p.first(), p.get(1))
                        && font.to_unicode.len() < MAX_MAP
                    {
                        font.to_unicode.insert(code_of(src, 0, src.len()), utf16(dst));
                    }
                }
            }
            b"endbfrange" => {
                for p in stack.chunks(3) {
                    let (Some(Obj::Str(lo)), Some(Obj::Str(hi)), Some(dst)) = (p.first(), p.get(1), p.get(2)) else { continue };
                    let (lo, hi) = (code_of(lo, 0, lo.len()), code_of(hi, 0, hi.len()));
                    if hi < lo || hi - lo > 65_535 {
                        continue;
                    }
                    for (k, code) in (lo..=hi).enumerate() {
                        if font.to_unicode.len() >= MAX_MAP {
                            break;
                        }
                        let t = match dst {
                            Obj::Str(d) => {
                                // Increment the last UTF-16 unit.
                                let mut d = d.clone();
                                let n = d.len();
                                if n >= 2 {
                                    let last = (u16::from(d.get(n - 2).copied().unwrap_or(0)) << 8 | u16::from(d.get(n - 1).copied().unwrap_or(0)))
                                        .wrapping_add(k as u16);
                                    if let Some(s) = d.get_mut(n - 2..n) {
                                        s.copy_from_slice(&last.to_be_bytes());
                                    }
                                } else if let Some(b) = d.last_mut() {
                                    *b = b.wrapping_add(k as u8);
                                }
                                utf16(&d)
                            }
                            Obj::Array(a) => match a.get(k) {
                                Some(Obj::Str(d)) => utf16(d),
                                _ => continue,
                            },
                            _ => continue,
                        };
                        font.to_unicode.insert(code, t);
                    }
                }
            }
            _ => {}
        }
        stack.clear();
    }
}

fn cid_widths(f: &File, w: &Obj, out: &mut HashMap<u32, f64>) {
    let Some(a) = w.array() else { return };
    let mut i = 0;
    while i < a.len() && out.len() < MAX_MAP {
        let Some(c) = a.get(i).and_then(|x| f.get(x).int()) else { break };
        match a.get(i + 1).map(|x| f.get(x)) {
            Some(Obj::Array(ws)) => {
                for (k, x) in ws.iter().enumerate().take(65_536) {
                    if let Some(v) = f.get(x).num() {
                        out.insert((c.max(0) as u32).saturating_add(k as u32), v);
                    }
                }
                i += 2;
            }
            Some(last) => {
                let (Some(last), Some(v)) = (last.int(), a.get(i + 2).and_then(|x| f.get(x).num())) else { break };
                if last >= c && last - c <= 65_535 {
                    for code in c.max(0)..=last.max(0) {
                        out.insert(code as u32, v);
                    }
                }
                i += 3;
            }
            None => break,
        }
    }
}

/// The 256-entry table of a simple font.
fn simple_encoding(f: &File, enc: &Obj, base_font: &str) -> [char; 256] {
    let mut tab = if base_font.contains("symbol") || base_font.contains("dingbat") { latin1() } else { win_ansi() };
    let apply_base = |name: &[u8], tab: &mut [char; 256]| match name {
        b"MacRomanEncoding" => *tab = mac_roman(),
        b"StandardEncoding" => {
            *tab = win_ansi();
            tab[0x27] = '’';
            tab[0x60] = '‘';
        }
        _ => {}
    };
    match enc {
        Obj::Name(n) => apply_base(n, &mut tab),
        Obj::Dict(d) => {
            if let Some(n) = f.at(d, "BaseEncoding").name() {
                apply_base(n, &mut tab);
            }
            if let Some(diff) = f.at(d, "Differences").array() {
                let mut code = 0usize;
                for x in diff {
                    match f.get(x) {
                        Obj::Int(c) => code = (*c).clamp(0, 255) as usize,
                        Obj::Name(n) => {
                            if let (Some(slot), Some(c)) = (tab.get_mut(code), glyph_char(n)) {
                                *slot = c;
                            }
                            code += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    tab
}

fn latin1() -> [char; 256] {
    let mut t = ['\0'; 256];
    for (i, c) in t.iter_mut().enumerate() {
        *c = char::from(i as u8);
    }
    t
}

fn win_ansi() -> [char; 256] {
    let mut t = latin1();
    const HIGH: [char; 32] = [
        '€', '\0', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\0', 'Ž', '\0', '\0', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š',
        '›', 'œ', '\0', 'ž', 'Ÿ',
    ];
    for (i, c) in HIGH.iter().enumerate() {
        if let Some(s) = t.get_mut(0x80 + i) {
            *s = *c;
        }
    }
    t
}

fn mac_roman() -> [char; 256] {
    let mut t = latin1();
    const HIGH: &str =
        "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»… ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{F8FF}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";
    for (i, c) in HIGH.chars().enumerate() {
        if let Some(s) = t.get_mut(0x80 + i) {
            *s = c;
        }
    }
    t
}

/// The character of a common glyph name.
fn glyph_char(n: &[u8]) -> Option<char> {
    let s = std::str::from_utf8(n).ok()?;
    if s.chars().count() == 1 {
        return s.chars().next();
    }
    if let Some(h) = s.strip_prefix("uni").or_else(|| s.strip_prefix('u')).filter(|h| (4..=6).contains(&h.len())) {
        return u32::from_str_radix(h.get(..4.max(h.len().min(6)))?, 16).ok().and_then(char::from_u32);
    }
    const NAMES: [(&str, char); 64] = [
        ("space", ' '),
        ("exclam", '!'),
        ("quotedbl", '"'),
        ("numbersign", '#'),
        ("dollar", '$'),
        ("percent", '%'),
        ("ampersand", '&'),
        ("quotesingle", '\''),
        ("quoteright", '’'),
        ("quoteleft", '‘'),
        ("parenleft", '('),
        ("parenright", ')'),
        ("asterisk", '*'),
        ("plus", '+'),
        ("comma", ','),
        ("hyphen", '-'),
        ("minus", '-'),
        ("period", '.'),
        ("slash", '/'),
        ("zero", '0'),
        ("one", '1'),
        ("two", '2'),
        ("three", '3'),
        ("four", '4'),
        ("five", '5'),
        ("six", '6'),
        ("seven", '7'),
        ("eight", '8'),
        ("nine", '9'),
        ("colon", ':'),
        ("semicolon", ';'),
        ("less", '<'),
        ("equal", '='),
        ("greater", '>'),
        ("question", '?'),
        ("at", '@'),
        ("bracketleft", '['),
        ("backslash", '\\'),
        ("bracketright", ']'),
        ("asciicircum", '^'),
        ("underscore", '_'),
        ("grave", '`'),
        ("braceleft", '{'),
        ("bar", '|'),
        ("braceright", '}'),
        ("asciitilde", '~'),
        ("endash", '–'),
        ("emdash", '—'),
        ("quotedblleft", '“'),
        ("quotedblright", '”'),
        ("quotesinglbase", '‚'),
        ("quotedblbase", '„'),
        ("ellipsis", '…'),
        ("bullet", '•'),
        ("eacute", 'é'),
        ("egrave", 'è'),
        ("ecircumflex", 'ê'),
        ("aacute", 'á'),
        ("agrave", 'à'),
        ("ntilde", 'ñ'),
        ("ccedilla", 'ç'),
        ("oacute", 'ó'),
        ("uacute", 'ú'),
        ("iacute", 'í'),
    ];
    NAMES.iter().find(|(k, _)| *k == s).map(|(_, c)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Font {
        Font {
            spaces: Vec::new(),
            two_byte: true,
            to_unicode: HashMap::new(),
            simple: None,
            widths: HashMap::new(),
            default_width: 600.0,
            scale: 0.001,
            bold: false,
            italic: false,
        }
    }

    #[test]
    fn cmap_ranges_and_chars() {
        let mut f = blank();
        parse_cmap(b"1 begincodespacerange <0000> <FFFF> endcodespacerange 2 beginbfchar <0001> <004E> <0002> <00490048> endbfchar 1 beginbfrange <0010> <0012> <0061> endbfrange 1 beginbfrange <0020> <0021> [<0041> <0042>] endbfrange", &mut f);
        assert_eq!(f.text(1), "N");
        assert_eq!(f.text(2), "IH");
        assert_eq!(f.text(0x12), "c");
        assert_eq!(f.text(0x21), "B");
        assert_eq!(f.codes(b"\x00\x01\x00\x02"), vec![(1, 2), (2, 2)]);
    }

    #[test]
    fn glyph_names() {
        assert_eq!(glyph_char(b"quoteright"), Some('’'));
        assert_eq!(glyph_char(b"uni0041"), Some('A'));
        assert_eq!(glyph_char(b"x"), Some('x'));
    }
}
