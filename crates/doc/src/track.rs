//! Track changes: who inserted and deleted what, kept until someone accepts or rejects it.
//!
//! This is separate from revision marks (production revisions with coloured pages). While
//! track changes is on, the engine runs [`follow`] after each command with the user's name.
//! It compares the script before and after the command and records:
//!
//! - **Insert**: new text, a byte range of the paragraph.
//! - **Break**: the paragraph break before this paragraph was inserted (Enter, pasted paragraphs).
//! - **Delete**: removed text, kept beside the paragraph at the place it was. A `\n` in it is a
//!   removed paragraph break, and `elements` holds the element of each paragraph that break
//!   started, so rejecting it restores the paragraphs as they were.
//! - **Element**: the paragraph's element changed; `elements[0]` is the old one.
//!
//! The script's text is always the current text, with insertions in and deletions out. So
//! pagination, PDF, print, reports and exports show the script as it would read if every
//! pending change were accepted. Deleted text is shown struck through in the Changes window and
//! marked in the page margin.
//!
//! Deleting text that is itself a pending insertion just removes it: nothing was there before.
//! The same author's deletions next to each other merge into one change.
//!
//! [`follow`] always keeps the existing changes on the text they belong to, also while tracking
//! is off. It aligns paragraphs (longest common subsequence of their text), then finds the
//! changed span inside each run of differing paragraphs.

use serde::{Deserialize, Serialize};

use crate::{Element, Pos, Script};

/// Most changes kept on one paragraph.
pub const MAX_CHANGES: usize = 2000;
/// Longest author name kept.
pub const MAX_AUTHOR: usize = 40;

/// What a tracked change is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Insert,
    Delete,
    Break,
    Element,
}

impl ChangeKind {
    pub fn id(self) -> &'static str {
        match self {
            ChangeKind::Insert => "insert",
            ChangeKind::Delete => "delete",
            ChangeKind::Break => "break",
            ChangeKind::Element => "element",
        }
    }
    pub fn parse(s: &str) -> Option<ChangeKind> {
        [ChangeKind::Insert, ChangeKind::Delete, ChangeKind::Break, ChangeKind::Element].into_iter().find(|k| k.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// One tracked change on a paragraph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub kind: ChangeKind,
    /// Insert: the inserted byte range. Delete: `start == end`, where the text was.
    #[serde(default)]
    pub start: usize,
    #[serde(default)]
    pub end: usize,
    /// Delete: the removed text (`\n` = a removed paragraph break).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Delete: the element of each paragraph a `\n` in `text` started. Element: the old element.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
    pub author: String,
}

impl Change {
    fn new(kind: ChangeKind, start: usize, end: usize, author: &str) -> Self {
        Change { kind, start, end, text: String::new(), elements: Vec::new(), author: author.to_string() }
    }
}

/// An author name made safe: one line, at most 40 characters; empty becomes `Writer`.
pub fn clean_author(a: &str) -> String {
    let a: String = a.chars().filter(|c| !c.is_control()).take(MAX_AUTHOR).collect::<String>().trim().to_string();
    if a.is_empty() { "Writer".into() } else { a }
}

/// Track-changes settings of a script.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackSettings {
    /// Record changes.
    pub on: bool,
}

impl TrackSettings {
    pub fn is_default(&self) -> bool {
        !self.on
    }
}

/// A change and where it is, for lists and commands.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRef {
    pub para: usize,
    /// Index into the paragraph's changes.
    pub index: usize,
    pub kind: ChangeKind,
    pub author: String,
    pub start: usize,
    pub end: usize,
    /// Insert: the inserted text. Delete: the deleted text. Element: `Action to Dialogue`.
    pub text: String,
}

// ---------------------------------------------------------------------------------------------
// Joined coordinates: the script as one string, paragraphs separated by '\n'.

struct Joined {
    text: String,
    /// Start of each paragraph.
    starts: Vec<usize>,
}

impl Joined {
    fn of(s: &Script) -> Joined {
        let mut text = String::with_capacity(s.paras.iter().map(|p| p.text.len() + 1).sum());
        let mut starts = Vec::with_capacity(s.paras.len());
        for (i, p) in s.paras.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            starts.push(text.len());
            text.push_str(&p.text);
        }
        Joined { text, starts }
    }
    fn start(&self, i: usize) -> usize {
        self.starts.get(i).copied().unwrap_or(self.text.len() + 1)
    }
    fn end(&self, i: usize, s: &Script) -> usize {
        self.start(i) + s.paras.get(i).map_or(0, |p| p.text.len())
    }
    /// The paragraph and offset of a joined position (a separator is the end of the paragraph
    /// before it).
    fn pos(&self, x: usize) -> Pos {
        let i = self.starts.partition_point(|&s| s <= x).saturating_sub(1);
        Pos::new(i, x - self.start(i).min(x))
    }
}

/// One edit: old joined range `a..b` became new range `c..d`.
#[derive(Clone, Copy, Debug)]
struct Edit {
    a: usize,
    b: usize,
    c: usize,
    d: usize,
}

/// Common prefix and suffix (bytes, on char boundaries, not overlapping).
fn common(a: &str, b: &str) -> (usize, usize) {
    let mut p = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    while p > 0 && !(a.is_char_boundary(p) && b.is_char_boundary(p)) {
        p -= 1;
    }
    let max = a.len().min(b.len()) - p;
    let mut s = a.bytes().rev().zip(b.bytes().rev()).take(max).take_while(|(x, y)| x == y).count();
    while s > 0 && !(a.is_char_boundary(a.len() - s) && b.is_char_boundary(b.len() - s)) {
        s -= 1;
    }
    (p, s)
}

/// Longest common subsequence of two sequences: matched index pairs in order. Large inputs (over
/// `cap` cells) get no matches (the caller treats everything as changed).
pub fn lcs_pairs<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool, cap: usize) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n.saturating_mul(m) > cap {
        return Vec::new();
    }
    // Lengths table, (n+1) × (m+1), filled from the end.
    let w = m + 1;
    let mut t = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let v = match (a.get(i), b.get(j)) {
                (Some(x), Some(y)) if eq(x, y) => t.get((i + 1) * w + j + 1).copied().unwrap_or(0) + 1,
                _ => t.get((i + 1) * w + j).copied().unwrap_or(0).max(t.get(i * w + j + 1).copied().unwrap_or(0)),
            };
            if let Some(c) = t.get_mut(i * w + j) {
                *c = v;
            }
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    let at = |i: usize, j: usize| t.get(i * w + j).copied().unwrap_or(0);
    while i < n && j < m {
        match (a.get(i), b.get(j)) {
            (Some(x), Some(y)) if eq(x, y) => {
                out.push((i, j));
                i += 1;
                j += 1;
            }
            _ if at(i + 1, j) >= at(i, j + 1) => i += 1,
            _ => j += 1,
        }
    }
    out
}

/// Paragraph alignment of two scripts: (old, new) index pairs that stay the same paragraph
/// (equal text, in order), plus 1:1 pairs inside runs of the same length (edited in place) and
/// the first paragraphs of other runs.
fn align(old: &Script, new: &Script) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let (o, n) = (&old.paras, &new.paras);
    let mut pre = 0;
    while pre < o.len() && pre < n.len() && o.get(pre).map(|p| &p.text) == n.get(pre).map(|p| &p.text) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < o.len() - pre && suf < n.len() - pre && o.get(o.len() - 1 - suf).map(|p| &p.text) == n.get(n.len() - 1 - suf).map(|p| &p.text) {
        suf += 1;
    }
    let mut same: Vec<(usize, usize)> = (0..pre).map(|i| (i, i)).collect();
    let om = o.get(pre..o.len() - suf).unwrap_or(&[]);
    let nm = n.get(pre..n.len() - suf).unwrap_or(&[]);
    same.extend(lcs_pairs(om, nm, |x, y| x.text == y.text, 4_000_000).into_iter().map(|(i, j)| (i + pre, j + pre)));
    same.extend((0..suf).map(|k| (o.len() - suf + k, n.len() - suf + k)));
    // Runs between matches; equal-length runs pair up one to one.
    let mut paired = Vec::new();
    let mut prev = (0usize, 0usize);
    let mut bounds = same.clone();
    bounds.push((o.len(), n.len()));
    for &(i, j) in &bounds {
        let (oa, na) = prev;
        if i - oa == j - na {
            paired.extend((0..i - oa).map(|k| (oa + k, na + k)));
        } else if i > oa && j > na {
            // Joins and splits keep the first paragraph (and its element).
            paired.push((oa, na));
        }
        prev = (i + 1, j + 1);
    }
    (same, paired)
}

/// The edits between two scripts in joined coordinates, in order.
fn edits(old: &Script, new: &Script, jo: &Joined, jn: &Joined, same: &[(usize, usize)]) -> Vec<Edit> {
    let mut out = Vec::new();
    let mut bounds = same.to_vec();
    bounds.push((old.paras.len(), new.paras.len()));
    let mut prev = (0usize, 0usize);
    for &(i, j) in &bounds {
        let (oa, na) = prev;
        prev = (i + 1, j + 1);
        if oa == i && na == j {
            continue;
        }
        // The run's region, separators included (see the module docs).
        let region = |jj: &Joined, s: &Script, a: usize, b: usize| -> (usize, usize) {
            if a > 0 {
                let l = jj.start(a) - 1;
                let r = if b > a { jj.end(b - 1, s) } else { l };
                (l, r)
            } else {
                let r = if b < s.paras.len() { jj.start(b) } else { jj.text.len() };
                (0, r)
            }
        };
        let (l1, r1) = region(jo, old, oa, i);
        let (l2, r2) = region(jn, new, na, j);
        let (x, y) = (jo.text.get(l1..r1).unwrap_or(""), jn.text.get(l2..r2).unwrap_or(""));
        if x == y {
            continue;
        }
        let (p, s) = common(x, y);
        out.push(Edit { a: l1 + p, b: r1 - s, c: l2 + p, d: r2 - s });
    }
    out
}

/// Where an old position goes. `after`: at a pure insertion, land after the inserted text
/// (range starts) instead of before it (range ends, deletion points).
fn map_point(edits: &[Edit], x: usize, after: bool) -> usize {
    let mut delta: isize = 0;
    for e in edits {
        if x < e.a {
            break;
        }
        if x == e.a {
            return if e.a == e.b && after { e.d } else { e.c };
        }
        if x < e.b {
            return if after { e.d } else { e.c };
        }
        delta = e.d as isize - e.b as isize;
    }
    (x as isize + delta).max(0) as usize
}

/// The parts of old range `x..y` that survive, mapped to the new text.
fn map_range(edits: &[Edit], x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut pieces = vec![(x, y)];
    for e in edits.iter().filter(|e| e.a < e.b) {
        let mut next = Vec::new();
        for (a, b) in pieces {
            if b <= e.a || a >= e.b {
                next.push((a, b));
                continue;
            }
            if a < e.a {
                next.push((a, e.a));
            }
            if b > e.b {
                next.push((e.b, b));
            }
        }
        pieces = next;
    }
    pieces.into_iter().filter(|(a, b)| b > a).map(|(a, b)| (map_point(edits, a, true), map_point(edits, b, false))).filter(|(a, b)| b > a).collect()
}

/// Whether any paragraph has a tracked change.
pub fn has_changes(s: &Script) -> bool {
    s.paras.iter().any(|p| !p.changes.is_empty())
}

/// After a command changed `old` into `new`: keep the existing changes on their text and, when
/// `author` is given (track changes on), record what this command inserted and deleted.
pub fn follow(old: &Script, new: &mut Script, author: Option<&str>) {
    if author.is_none() && !has_changes(old) {
        if has_changes(new) {
            // Text pasted with changes, files merged…: no history to follow; keep them valid.
            fix_all(new);
        }
        return;
    }
    let jo = Joined::of(old);
    let jn = Joined::of(new);
    let elements_same = old.paras.len() == new.paras.len() && old.paras.iter().zip(&new.paras).all(|(a, b)| a.element == b.element);
    if jo.text == jn.text && elements_same {
        // Nothing moved (a command that only changed marks, settings…).
        return;
    }
    let author = author.map(clean_author);
    let (same, paired) = align(old, new);
    let ed = edits(old, new, &jo, &jn, &same);

    // 1. Existing changes, mapped. (new paragraph, change, old joined position for ghosts)
    let mut out: Vec<Vec<Change>> = vec![Vec::new(); new.paras.len()];
    let mut ghosts: Vec<(usize, Change, usize)> = Vec::new();
    let new_of_old: std::collections::HashMap<usize, usize> = same.iter().chain(paired.iter()).copied().collect();
    let mut inserted_old: Vec<(usize, usize)> = Vec::new();
    for (i, p) in old.paras.iter().enumerate() {
        let base = jo.start(i);
        for c in &p.changes {
            match c.kind {
                ChangeKind::Insert => {
                    inserted_old.push((base + c.start, base + c.end));
                    for (a, b) in map_range(&ed, base + c.start, base + c.end) {
                        push_range(&mut out, &jn, new, a, b, &c.author);
                    }
                }
                ChangeKind::Break => {
                    if i > 0 {
                        inserted_old.push((base - 1, base));
                        for (a, b) in map_range(&ed, base - 1, base) {
                            push_range(&mut out, &jn, new, a, b, &c.author);
                        }
                    }
                }
                ChangeKind::Delete => {
                    let x = base + c.start;
                    ghosts.push((map_point(&ed, x, false), c.clone(), x));
                }
                ChangeKind::Element => {
                    if let Some(&ni) = new_of_old.get(&i)
                        && let Some(slot) = out.get_mut(ni)
                    {
                        slot.push(c.clone());
                    }
                }
            }
        }
    }

    // 2. This command's changes.
    if let Some(author) = author.as_deref() {
        for e in &ed {
            if e.d > e.c {
                push_range(&mut out, &jn, new, e.c, e.d, author);
            }
            if e.b > e.a {
                // Deleted text, minus what was a pending insertion.
                let mut text = String::new();
                let mut elements = Vec::new();
                let mut k = e.a;
                while k < e.b {
                    let ch = jo.text.get(k..).and_then(|t| t.chars().next());
                    let Some(ch) = ch else { break };
                    let w = ch.len_utf8();
                    let was_inserted = inserted_old.iter().any(|&(a, b)| a <= k && k < b);
                    if !was_inserted {
                        text.push(ch);
                        if ch == '\n' {
                            let starts = jo.pos(k + 1).para;
                            elements.push(old.paras.get(starts).map_or(Element::Action, |p| p.element));
                        }
                    }
                    k += w;
                }
                if !text.is_empty() {
                    let mut c = Change::new(ChangeKind::Delete, 0, 0, author);
                    c.text = text;
                    c.elements = elements;
                    ghosts.push((e.c, c, usize::MAX));
                    // Merge with the same author's deletions at the same place, in reading order.
                    let here: Vec<usize> =
                        ghosts.iter().enumerate().filter(|(_, (at, g, _))| *at == e.c && g.author == author).map(|(k, _)| k).collect();
                    if here.len() > 1 {
                        let mut parts: Vec<(usize, Change)> = Vec::new();
                        for k in here.iter().rev() {
                            if *k < ghosts.len() {
                                let (_, g, ox) = ghosts.remove(*k);
                                // Before the deleted range, the new deletion, then after it.
                                let rank = if ox == usize::MAX {
                                    1
                                } else if ox <= e.a {
                                    0
                                } else {
                                    2
                                };
                                parts.push((rank, g));
                            }
                        }
                        parts.sort_by_key(|(r, _)| *r);
                        let mut merged = Change::new(ChangeKind::Delete, 0, 0, author);
                        for (_, g) in parts {
                            merged.text.push_str(&g.text);
                            merged.elements.extend(g.elements);
                        }
                        ghosts.push((e.c, merged, e.a));
                    }
                }
            }
        }
        // Element changes on paragraphs that stayed.
        for &(i, j) in same.iter().chain(paired.iter()) {
            let (Some(op), Some(np)) = (old.paras.get(i), new.paras.get(j)) else { continue };
            if op.element == np.element {
                continue;
            }
            let Some(slot) = out.get_mut(j) else { continue };
            match slot.iter().position(|c| c.kind == ChangeKind::Element) {
                Some(k) => {
                    if slot.get(k).and_then(|c| c.elements.first()) == Some(&np.element) {
                        slot.remove(k);
                    }
                }
                None => {
                    let mut c = Change::new(ChangeKind::Element, 0, 0, author);
                    c.elements = vec![op.element];
                    slot.push(c);
                }
            }
        }
    }
    for (at, mut g, _) in ghosts {
        let p = jn.pos(at);
        g.start = p.off;
        g.end = p.off;
        if let Some(slot) = out.get_mut(p.para) {
            slot.push(g);
        }
    }
    for (p, v) in new.paras.iter_mut().zip(out) {
        p.changes = v;
    }
    fix_all(new);
}

/// Add Insert ranges (and Break marks for separators) for the new joined range `a..b`.
fn push_range(out: &mut [Vec<Change>], jn: &Joined, new: &Script, a: usize, b: usize, author: &str) {
    let mut x = a;
    while x < b {
        let p = jn.pos(x);
        let start = jn.start(p.para);
        let end = jn.end(p.para, new);
        if x == end && x < b {
            // A separator: the next paragraph's break was inserted.
            if let Some(slot) = out.get_mut(p.para + 1) {
                slot.push(Change::new(ChangeKind::Break, 0, 0, author));
            }
            x += 1;
            continue;
        }
        let stop = b.min(end);
        if let Some(slot) = out.get_mut(p.para) {
            slot.push(Change::new(ChangeKind::Insert, x - start, stop - start, author));
        }
        x = stop;
    }
}

/// Keep every paragraph's changes valid.
pub fn fix_all(s: &mut Script) {
    for (i, p) in s.paras.iter_mut().enumerate() {
        if p.changes.is_empty() {
            continue;
        }
        let v = std::mem::take(&mut p.changes);
        p.changes = fix(&p.text, v, i > 0);
    }
}

/// Changes made valid for `text`: in range, on char boundaries, Insert ranges sorted and the
/// same author's adjacent ones joined, at most one Break and one Element.
pub fn fix(text: &str, v: Vec<Change>, can_break: bool) -> Vec<Change> {
    let clamp = |x: usize, up: bool| {
        let mut x = x.min(text.len());
        while !text.is_char_boundary(x) {
            if up { x += 1 } else { x -= 1 }
        }
        x
    };
    let mut ins: Vec<Change> = Vec::new();
    let mut other: Vec<Change> = Vec::new();
    let (mut brk, mut el) = (false, false);
    for mut c in v {
        c.author = clean_author(&c.author);
        match c.kind {
            ChangeKind::Insert => {
                c.start = clamp(c.start, false);
                c.end = clamp(c.end, true).max(c.start);
                c.text.clear();
                c.elements.clear();
                if c.end > c.start {
                    ins.push(c);
                }
            }
            ChangeKind::Delete => {
                c.start = clamp(c.start, false);
                c.end = c.start;
                c.text = c.text.chars().filter(|ch| *ch == '\n' || !ch.is_control()).take(crate::MAX_PARA_BYTES).collect();
                let breaks = c.text.matches('\n').count();
                c.elements.truncate(breaks);
                while c.elements.len() < breaks {
                    c.elements.push(Element::Action);
                }
                if !c.text.is_empty() {
                    other.push(c);
                }
            }
            ChangeKind::Break => {
                if can_break && !brk {
                    brk = true;
                    other.push(Change { start: 0, end: 0, text: String::new(), elements: Vec::new(), ..c });
                }
            }
            ChangeKind::Element => {
                if !el && !c.elements.is_empty() {
                    el = true;
                    c.elements.truncate(1);
                    other.push(Change { start: 0, end: 0, text: String::new(), ..c });
                }
            }
        }
    }
    ins.sort_by_key(|c| (c.start, c.end));
    let mut merged: Vec<Change> = Vec::new();
    for c in ins {
        match merged.last_mut() {
            Some(l) if c.start <= l.end && l.author == c.author => l.end = l.end.max(c.end),
            Some(l) if c.start < l.end => {
                // Overlap of different authors (files only): keep the first.
                let mut c = c;
                c.start = l.end;
                if c.end > c.start {
                    merged.push(c);
                }
            }
            _ => merged.push(c),
        }
    }
    merged.extend(other);
    merged.sort_by_key(|c| (c.start, c.kind != ChangeKind::Break, c.kind == ChangeKind::Insert));
    merged.truncate(MAX_CHANGES);
    merged
}

/// Every change in reading order.
pub fn list(s: &Script) -> Vec<ChangeRef> {
    let mut out = Vec::new();
    for (i, p) in s.paras.iter().enumerate() {
        for (k, c) in p.changes.iter().enumerate() {
            let text = match c.kind {
                ChangeKind::Insert => p.text.get(c.start..c.end).unwrap_or("").to_string(),
                ChangeKind::Delete => c.text.clone(),
                ChangeKind::Break => "¶".into(),
                ChangeKind::Element => format!("{} to {}", c.elements.first().map_or("?", |e| e.name()), p.element.name()),
            };
            out.push(ChangeRef { para: i, index: k, kind: c.kind, author: c.author.clone(), start: c.start, end: c.end, text });
        }
    }
    out
}

/// Authors with pending changes.
pub fn authors(s: &Script) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for p in &s.paras {
        for c in &p.changes {
            if !v.contains(&c.author) {
                v.push(c.author.clone());
            }
        }
    }
    v
}

/// Accept change `index` of paragraph `para`: it becomes ordinary text (or is forgotten).
pub fn accept(s: &mut Script, para: usize, index: usize) -> bool {
    match s.paras.get_mut(para) {
        Some(p) if index < p.changes.len() => {
            p.changes.remove(index);
            true
        }
        _ => false,
    }
}

/// Reject change `index` of paragraph `para`: undo what it did, keeping every other change on
/// its text. Returns where the caret should go.
pub fn reject(s: &mut Script, para: usize, index: usize) -> Option<Pos> {
    let c = s.paras.get(para)?.changes.get(index)?.clone();
    let mut old = s.clone();
    if let Some(p) = old.paras.get_mut(para) {
        p.changes.remove(index);
    }
    let mut new = old.clone();
    let caret = match c.kind {
        ChangeKind::Insert => new.delete_range(Pos::new(para, c.start), Pos::new(para, c.end)),
        ChangeKind::Break => {
            let prev = para.checked_sub(1)?;
            let end = new.paras.get(prev)?.text.len();
            new.delete_range(Pos::new(prev, end), Pos::new(para, 0))
        }
        ChangeKind::Element => {
            let e = *c.elements.first()?;
            new.set_element(para, e).ok()?;
            Pos::new(para, 0)
        }
        ChangeKind::Delete => {
            let at = new.clamp(Pos::new(para, c.start));
            restore(&mut new, at, &c.text, &c.elements)?
        }
    };
    follow(&old, &mut new, None);
    *s = new;
    Some(caret)
}

/// Put deleted text back at `at` as it was: no capitals added, a `\n` starts a paragraph of
/// the recorded element, and the rest of the paragraph (with its styles and marks) follows the
/// last piece. Returns the position after the restored text.
fn restore(s: &mut Script, at: Pos, text: &str, elements: &[Element]) -> Option<Pos> {
    if text.matches('\n').count() + s.paras.len() > crate::MAX_PARAS {
        return None;
    }
    let p = s.paras.get_mut(at.para)?;
    let mut styles = p.styles();
    let tail_styles = styles.split_off(at.off.min(styles.len()));
    let tail = p.text.split_off(at.off.min(p.text.len()));
    let mut moved = Some(p.marks_split(at.off));
    let chunks: Vec<&str> = text.split('\n').collect();
    let last = chunks.len() - 1;
    let mut cur = at.para;
    let mut end = at;
    for (k, chunk) in chunks.iter().enumerate() {
        let chunk = crate::sanitize(chunk);
        if k > 0 {
            let e = elements.get(k - 1).copied().unwrap_or(Element::Action);
            s.paras.insert(cur + 1, crate::Para::new(e, ""));
            cur += 1;
        }
        let q = s.paras.get_mut(cur)?;
        let mut st = q.styles();
        let mut t = std::mem::take(&mut q.text);
        st.extend(std::iter::repeat_n(crate::Style::PLAIN, chunk.len()));
        t.push_str(&chunk);
        end = Pos::new(cur, t.len());
        if k == last {
            let off = t.len();
            st.extend(tail_styles.iter().copied());
            t.push_str(&tail);
            q.set_text_styled(&t, &st, false);
            if let Some(m) = moved.take() {
                q.marks_append(shift(m, off));
            }
        } else {
            q.set_text_styled(&t, &st, false);
        }
    }
    Some(end)
}

/// Marks moved by a split, shifted to start at `by`.
fn shift(mut m: crate::annot::Marks, by: usize) -> crate::annot::Marks {
    for n in &mut m.notes {
        n.start += by;
        n.end += by;
    }
    for t in &mut m.tagged {
        t.start += by;
        t.end += by;
    }
    m
}

/// Accept or reject every change (of `author`, when given). Returns how many.
pub fn resolve_all(s: &mut Script, accept_them: bool, author: Option<&str>) -> usize {
    let mut n = 0;
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 1_000_000 {
            break;
        }
        let found = s.paras.iter().enumerate().find_map(|(i, p)| p.changes.iter().position(|c| author.is_none_or(|a| c.author == a)).map(|k| (i, k)));
        let Some((i, k)) = found else { break };
        let ok = if accept_them { accept(s, i, k) } else { reject(s, i, k).is_some() || accept(s, i, k) };
        if !ok {
            break;
        }
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Para;

    fn script(lines: &[(Element, &str)]) -> Script {
        Script::from_paras(lines.iter().map(|(e, t)| Para::new(*e, *t)).collect()).unwrap_or_default()
    }

    fn tracked(old: &Script, edit: impl FnOnce(&mut Script), who: &str) -> Script {
        let mut new = old.clone();
        edit(&mut new);
        follow(old, &mut new, Some(who));
        new
    }

    #[test]
    fn insert_and_delete_in_a_paragraph() {
        let s = script(&[(Element::Action, "The cat sat.")]);
        let s2 = tracked(
            &s,
            |x| {
                let _ = x.insert_text(Pos::new(0, 4), "big ");
            },
            "Ana",
        );
        assert_eq!(s2.paras[0].changes, vec![Change::new(ChangeKind::Insert, 4, 8, "Ana")]);
        // Bo deletes "cat " ; Ana's insertion stays.
        let s3 = tracked(
            &s2,
            |x| {
                let _ = x.delete_range(Pos::new(0, 8), Pos::new(0, 12));
            },
            "Bo",
        );
        assert_eq!(s3.paras[0].text, "The big sat.");
        let l = list(&s3);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].kind, l[0].text.as_str(), l[0].author.as_str()), (ChangeKind::Insert, "big ", "Ana"));
        assert_eq!((l[1].kind, l[1].text.as_str(), l[1].start), (ChangeKind::Delete, "cat ", 8));
        // Reject the deletion: the text is back, Ana's insertion still marked.
        let mut s4 = s3.clone();
        reject(&mut s4, 0, 1).unwrap();
        assert_eq!(s4.paras[0].text, "The big cat sat.");
        assert_eq!(list(&s4).len(), 1);
        // Reject the insertion too: the original.
        reject(&mut s4, 0, 0).unwrap();
        assert_eq!(s4.paras[0].text, "The cat sat.");
        assert!(!has_changes(&s4));
    }

    #[test]
    fn deleting_your_insertion_leaves_nothing_and_backspaces_merge() {
        let s = script(&[(Element::Action, "abcdef")]);
        let s2 = tracked(
            &s,
            |x| {
                let _ = x.insert_text(Pos::new(0, 3), "XYZ");
            },
            "A",
        );
        let s3 = tracked(
            &s2,
            |x| {
                let _ = x.delete_range(Pos::new(0, 4), Pos::new(0, 6));
            },
            "A",
        );
        assert_eq!(s3.paras[0].text, "abcXdef");
        assert_eq!(list(&s3).len(), 1);
        // Backspace "c", then "b": one deletion "bc".
        let s4 = tracked(
            &s3,
            |x| {
                let _ = x.delete_range(Pos::new(0, 2), Pos::new(0, 3));
            },
            "A",
        );
        let s5 = tracked(
            &s4,
            |x| {
                let _ = x.delete_range(Pos::new(0, 1), Pos::new(0, 2));
            },
            "A",
        );
        let dels: Vec<String> = list(&s5).into_iter().filter(|c| c.kind == ChangeKind::Delete).map(|c| c.text).collect();
        assert_eq!(dels, ["bc"]);
        // Deleting the pending insertion next to it adds nothing.
        let s6 = tracked(
            &s5,
            |x| {
                let _ = x.delete_range(Pos::new(0, 1), Pos::new(0, 2));
            },
            "A",
        );
        assert_eq!(s6.paras[0].text, "adef");
        let dels: Vec<String> = list(&s6).into_iter().filter(|c| c.kind == ChangeKind::Delete).map(|c| c.text).collect();
        assert_eq!(dels, ["bc"], "{:?}", list(&s6));
    }

    #[test]
    fn paragraph_breaks_and_joins() {
        let s = script(&[(Element::Action, "One two"), (Element::Character, "ANA"), (Element::Dialogue, "Hi.")]);
        // Enter after "One".
        let s2 = tracked(
            &s,
            |x| {
                let _ = x.split(Pos::new(0, 3), Element::Action);
            },
            "A",
        );
        assert_eq!(s2.paras.len(), 4);
        assert_eq!(list(&s2).iter().map(|c| c.kind).collect::<Vec<_>>(), [ChangeKind::Break]);
        let mut r = s2.clone();
        reject(&mut r, 1, 0).unwrap();
        assert_eq!(r.paras[0].text, "One two");
        // Join Character into the line before: a deleted break that restores a Character.
        let s3 = tracked(
            &s,
            |x| {
                let _ = x.delete_range(Pos::new(0, 7), Pos::new(1, 0));
            },
            "B",
        );
        assert_eq!(s3.paras.len(), 2);
        let l = list(&s3);
        assert_eq!((l[0].kind, l[0].text.as_str()), (ChangeKind::Delete, "\n"));
        let mut r = s3.clone();
        reject(&mut r, 0, 0).unwrap();
        assert_eq!(r.paras, s.paras);
        // Delete whole paragraphs and bring them back.
        let s4 = tracked(
            &s,
            |x| {
                let _ = x.delete_range(Pos::new(0, 7), Pos::new(2, 3));
            },
            "B",
        );
        assert_eq!(s4.paras.len(), 1);
        let mut r = s4.clone();
        assert_eq!(resolve_all(&mut r, false, None), 1);
        assert_eq!(r.paras, s.paras);
    }

    #[test]
    fn element_changes_and_authors() {
        let s = script(&[(Element::Action, "hello"), (Element::Action, "world")]);
        let s2 = tracked(
            &s,
            |x| {
                let _ = x.set_element(1, Element::Dialogue);
            },
            "A",
        );
        let l = list(&s2);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "Action to Dialogue");
        let s3 = tracked(
            &s2,
            |x| {
                let _ = x.insert_text(Pos::new(0, 5), "!");
            },
            "B",
        );
        assert_eq!(authors(&s3), ["B", "A"]);
        let mut r = s3.clone();
        assert_eq!(resolve_all(&mut r, false, Some("A")), 1);
        assert_eq!(r.paras[1].element, Element::Action);
        assert_eq!(authors(&r), ["B"]);
        assert_eq!(resolve_all(&mut r, true, None), 1);
        assert!(!has_changes(&r) && r.paras[0].text == "hello!");
        // Changing it back removes the change.
        let s4 = tracked(
            &s2,
            |x| {
                let _ = x.set_element(1, Element::Action);
            },
            "A",
        );
        assert!(!has_changes(&s4));
    }

    #[test]
    fn changes_follow_untracked_edits() {
        let s = script(&[(Element::Action, "abc"), (Element::Action, "def")]);
        let s2 = tracked(
            &s,
            |x| {
                let _ = x.insert_text(Pos::new(1, 1), "XX");
            },
            "A",
        );
        let mut s3 = s2.clone();
        s3.insert_text(Pos::new(1, 0), "__").unwrap();
        s3.paras.insert(0, Para::new(Element::Action, "new first"));
        follow(&s2, &mut s3, None);
        let l = list(&s3);
        assert_eq!((l.len(), l[0].para, l[0].text.as_str()), (1, 2, "XX"));
    }

    #[test]
    fn moved_scene_is_one_delete_and_one_insert() {
        let s = script(&[(Element::SceneHeading, "INT. A"), (Element::Action, "a"), (Element::SceneHeading, "INT. B"), (Element::Action, "b")]);
        let s2 = tracked(
            &s,
            |x| {
                let _ = x.move_scene(1, 0);
            },
            "A",
        );
        let kinds: Vec<ChangeKind> = list(&s2).into_iter().map(|c| c.kind).collect();
        assert!(kinds.contains(&ChangeKind::Delete) && kinds.contains(&ChangeKind::Insert), "{kinds:?}");
        let mut r = s2.clone();
        resolve_all(&mut r, false, None);
        assert_eq!(r.paras, s.paras);
    }

    #[test]
    fn hostile_changes_are_fixed() {
        let mut s = script(&[(Element::Action, "héllo")]);
        s.paras[0].changes = vec![
            Change { kind: ChangeKind::Insert, start: 2, end: 999, text: "x".into(), elements: vec![Element::Action], author: "\u{7}".into() },
            Change { kind: ChangeKind::Delete, start: 2, end: 5, text: "a\nb\nc".into(), elements: vec![], author: "B".into() },
            Change { kind: ChangeKind::Break, start: 3, end: 3, text: String::new(), elements: vec![], author: "B".into() },
            Change { kind: ChangeKind::Element, start: 0, end: 0, text: String::new(), elements: vec![], author: "B".into() },
        ];
        fix_all(&mut s);
        let c = &s.paras[0].changes;
        assert!(c.iter().all(|c| s.paras[0].text.is_char_boundary(c.start) && c.end <= s.paras[0].text.len()));
        assert!(c.iter().any(|c| c.kind == ChangeKind::Delete && c.elements.len() == 2));
        assert!(!c.iter().any(|c| matches!(c.kind, ChangeKind::Break | ChangeKind::Element)));
        assert!(c.iter().any(|c| c.kind == ChangeKind::Insert && c.author == "Writer"));
        let mut r = s.clone();
        resolve_all(&mut r, false, None);
        assert!(!has_changes(&r));
        assert!(reject(&mut r, 9, 9).is_none());
        assert!(!accept(&mut r, 0, 99));
    }

    #[test]
    fn lcs_basics() {
        let a = [1, 2, 3, 4];
        let b = [2, 4, 5];
        assert_eq!(lcs_pairs(&a, &b, |x, y| x == y, 100), vec![(1, 0), (3, 1)]);
        assert!(lcs_pairs(&a, &b, |x, y| x == y, 2).is_empty());
    }
}
