//! Hostile input never panics; every importer answers with a script or an error.

use crate::{Kind, import};

/// A small deterministic generator (no extra dependencies).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(src: &[u8], r: &mut Rng) -> Vec<u8> {
    let mut v = src.to_vec();
    for _ in 0..1 + r.below(12) {
        if v.is_empty() {
            break;
        }
        let i = r.below(v.len());
        match r.below(4) {
            0 => v[i] = r.next() as u8,
            1 => {
                v.truncate(i);
            }
            2 => {
                let j = r.below(v.len());
                let chunk: Vec<u8> = v[i.min(j)..i.max(j)].to_vec();
                v.splice(i..i, chunk.into_iter().take(4096));
            }
            _ => {
                v.remove(i);
            }
        }
    }
    v
}

#[test]
fn hostile_input_never_panics() {
    let seeds: Vec<(Kind, Vec<u8>)> = vec![
        (Kind::Pdf, crate::pdf::tests::build(&[vec![(1.5, 1.2, "INT. A - DAY"), (3.7, 1.5, "NELL"), (2.5, 1.7, "Hi.")]])),
        (Kind::Docx, crate::docx::tests::build(&[("SceneHeading", 0, "", &["int. a - day"]), ("", 3168, "", &["NELL"])], &[("center", "T")])),
        (Kind::Celtx, b"<p class=\"sceneheading\">INT. A</p><p class=\"dialog\">x<b>y</b></p>".to_vec()),
    ];
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    for (kind, seed) in &seeds {
        assert!(import(seed, *kind).is_ok(), "{kind:?} seed imports");
        for _ in 0..400 {
            let m = mutate(seed, &mut r);
            let _ = import(&m, *kind);
        }
        for junk in [&b""[..], b"%PDF-", b"PK\x03\x04", b"PK\x05\x06\0\0\0\0\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\0\0", &[0xFF; 2000][..]] {
            let _ = import(junk, *kind);
        }
    }
    // Deeply nested or self-referencing PDF structures.
    let mut deep = b"%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj 2 0 obj << /Type /Pages /Kids [2 0 R 3 0 R] >> endobj 3 0 obj << /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /XObject << /X 5 0 R >> >> >> endobj 4 0 obj << /Length 9 >> stream\n/X Do q q\nendstream endobj 5 0 obj << /Subtype /Form /Resources << /XObject << /X 5 0 R >> >> /Length 5 >> stream\n/X Do\nendstream endobj\n".to_vec();
    deep.extend(std::iter::repeat_n(b'[', 50_000));
    let _ = import(&deep, Kind::Pdf);
}
