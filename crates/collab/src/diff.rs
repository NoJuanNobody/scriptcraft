//! Diffs: whole paragraphs (patches between scripts) and characters (inside merged regions).

use std::hash::{DefaultHasher, Hash, Hasher};

use scriptcraft_doc::{MAX_PARAS, Para, Script, Settings, TitlePage};
use serde::{Deserialize, Serialize};

/// Above this many cells a diff replaces the whole changed middle instead of running LCS.
const LCS_CELLS: usize = 4_000_000;

/// `base[at..at + del]` is replaced by `ins`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hunk {
    pub at: usize,
    pub del: usize,
    #[serde(default)]
    pub ins: Vec<Para>,
}

/// The difference between two scripts: paragraph hunks (ascending, non-overlapping, against the
/// base) and the title page and settings when they changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    #[serde(default)]
    pub hunks: Vec<Hunk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Box<TitlePage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Box<Settings>>,
}

impl Patch {
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty() && self.title.is_none() && self.settings.is_none()
    }
}

/// A range of `a` replaced by a range of `b` (indices into the compared sequences).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Edit {
    pub a0: usize,
    pub a1: usize,
    pub b0: usize,
    pub b1: usize,
}

/// The edits turning `a` into `b`: common start and end trimmed, LCS on the middle when it is
/// small enough, else one replacement.
pub(crate) fn edits<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Edit> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (ar, br) = (a.get(pre..).unwrap_or(&[]), b.get(pre..).unwrap_or(&[]));
    let suf = ar.iter().rev().zip(br.iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (ar.get(..ar.len() - suf).unwrap_or(&[]), br.get(..br.len() - suf).unwrap_or(&[]));
    if am.is_empty() && bm.is_empty() {
        return Vec::new();
    }
    let whole = || vec![Edit { a0: pre, a1: pre + am.len(), b0: pre, b1: pre + bm.len() }];
    if am.is_empty() || bm.is_empty() || am.len().saturating_mul(bm.len()) > LCS_CELLS {
        return whole();
    }
    // LCS lengths from the end: t[i][j] = LCS(am[i..], bm[j..]).
    let (n, m) = (am.len(), bm.len());
    let w = m + 1;
    let mut t = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let v = if am.get(i) == bm.get(j) {
                t.get((i + 1) * w + j + 1).copied().unwrap_or(0) + 1
            } else {
                t.get((i + 1) * w + j).copied().unwrap_or(0).max(t.get(i * w + j + 1).copied().unwrap_or(0))
            };
            if let Some(c) = t.get_mut(i * w + j) {
                *c = v;
            }
        }
    }
    let at = |i: usize, j: usize| t.get(i * w + j).copied().unwrap_or(0);
    let mut out: Vec<Edit> = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut push = |ai: usize, aj: usize, bi: usize, bj: usize| {
        let e = Edit { a0: pre + ai, a1: pre + aj, b0: pre + bi, b1: pre + bj };
        match out.last_mut() {
            Some(l) if l.a1 == e.a0 && l.b1 == e.b0 => {
                l.a1 = e.a1;
                l.b1 = e.b1;
            }
            _ => out.push(e),
        }
    };
    while i < n || j < m {
        if i < n && j < m && am.get(i) == bm.get(j) {
            i += 1;
            j += 1;
        } else if j < m && (i == n || at(i, j + 1) >= at(i + 1, j)) {
            push(i, i, j, j + 1);
            j += 1;
        } else {
            push(i, i + 1, j, j);
            i += 1;
        }
    }
    out
}

fn para_hash(p: &Para) -> u64 {
    let mut h = DefaultHasher::new();
    serde_json::to_string(p).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// Paragraph hunks turning `a` into `b`.
pub fn hunks(a: &[Para], b: &[Para]) -> Vec<Hunk> {
    // Trim with real equality first (cheap), hash only the middle.
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a.get(pre..).unwrap_or(&[]).iter().rev().zip(b.get(pre..).unwrap_or(&[]).iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (a.get(pre..a.len() - suf).unwrap_or(&[]), b.get(pre..b.len() - suf).unwrap_or(&[]));
    let (ha, hb): (Vec<u64>, Vec<u64>) = (am.iter().map(para_hash).collect(), bm.iter().map(para_hash).collect());
    edits(&ha, &hb)
        .into_iter()
        .map(|e| Hunk { at: pre + e.a0, del: e.a1 - e.a0, ins: bm.get(e.b0..e.b1).map(<[Para]>::to_vec).unwrap_or_default() })
        .collect()
}

/// The patch turning `a` into `b`.
pub fn diff(a: &Script, b: &Script) -> Patch {
    Patch {
        hunks: hunks(&a.paras, &b.paras),
        title: (a.title != b.title).then(|| Box::new(b.title.clone())),
        settings: (a.settings != b.settings).then(|| Box::new(b.settings.clone())),
    }
}

/// Why a patch can't be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchError {
    /// Hunks out of order, overlapping or past the end.
    Range,
    /// The result would be too large.
    TooLarge,
}

/// Apply `p` to `base` (hostile input: validated; the result is normalized).
pub fn apply(base: &Script, p: &Patch) -> Result<Script, PatchError> {
    let mut paras = Vec::with_capacity(base.paras.len());
    let mut i = 0usize;
    for h in &p.hunks {
        let end = h.at.checked_add(h.del).ok_or(PatchError::Range)?;
        if h.at < i || end > base.paras.len() {
            return Err(PatchError::Range);
        }
        paras.extend_from_slice(base.paras.get(i..h.at).ok_or(PatchError::Range)?);
        if paras.len() + h.ins.len() > MAX_PARAS {
            return Err(PatchError::TooLarge);
        }
        paras.extend(h.ins.iter().cloned());
        i = end;
    }
    paras.extend_from_slice(base.paras.get(i..).unwrap_or(&[]));
    if paras.len() > MAX_PARAS {
        return Err(PatchError::TooLarge);
    }
    let mut s = Script {
        title: p.title.as_deref().cloned().unwrap_or_else(|| base.title.clone()),
        paras,
        settings: p.settings.as_deref().cloned().unwrap_or_else(|| base.settings.clone()),
    };
    s.normalize().map_err(|_| PatchError::TooLarge)?;
    Ok(s)
}

/// A text splice: `base[a..b]` (bytes) replaced by `target[ta..tb]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Splice {
    pub a: usize,
    pub b: usize,
    pub ta: usize,
    pub tb: usize,
}

/// Character diff of two strings, as byte-offset splices.
pub(crate) fn text_splices(base: &str, target: &str) -> Vec<Splice> {
    let (bc, tc): (Vec<char>, Vec<char>) = (base.chars().collect(), target.chars().collect());
    let boff = byte_offsets(base);
    let toff = byte_offsets(target);
    let at = |v: &[usize], i: usize| v.get(i).copied().unwrap_or(0);
    edits(&bc, &tc).into_iter().map(|e| Splice { a: at(&boff, e.a0), b: at(&boff, e.a1), ta: at(&toff, e.b0), tb: at(&toff, e.b1) }).collect()
}

/// Byte offset of every char index (and the end).
fn byte_offsets(s: &str) -> Vec<usize> {
    let mut v: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
    v.push(s.len());
    v
}
