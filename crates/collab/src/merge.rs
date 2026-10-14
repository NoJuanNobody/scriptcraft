//! Three-way merge of scripts, and carrying positions through a change. See
//! `docs/collaboration.md`.

use scriptcraft_doc::{Beat, BeatBoard, Element, Para, Pos, Script, Style};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::diff::{Hunk, hunks, text_splices};

/// Merge `theirs` into `ours`, both edited from `base`. `ours` wins true conflicts (the host's
/// order); concurrent typing is kept from both.
pub fn merge3(base: &Script, ours: &Script, theirs: &Script) -> Script {
    if theirs == base || ours == theirs {
        return ours.clone();
    }
    if ours == base {
        return theirs.clone();
    }
    // Beats both sides added under the same new id: theirs get fresh ids (links follow).
    let renumbered;
    let theirs = match renumber_new_beats(base, ours, theirs) {
        Some(t) => {
            renumbered = t;
            &renumbered
        }
        None => theirs,
    };
    // Every Script field must be merged: a new field fails to compile here until it is.
    let Script { title: _, paras: _, settings: _, structure_tail: _, board: _ } = ours;
    let mut out = Script {
        title: merge_value(&base.title, &ours.title, &theirs.title),
        paras: Vec::new(),
        settings: merge_value(&base.settings, &ours.settings, &theirs.settings),
        structure_tail: merge_value(&base.structure_tail, &ours.structure_tail, &theirs.structure_tail),
        board: merge_board(&base.board, &ours.board, &theirs.board),
    };
    out.paras = merge_paras(&base.paras, &ours.paras, &theirs.paras, &out);
    let _ = out.normalize();
    out
}

/// Field-by-field merge through JSON: a field changed on one side takes that side; changed on
/// both, `ours`. Unknown future fields merge the same way.
fn merge_value<T: Serialize + DeserializeOwned + Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> T {
    if theirs == base {
        return ours.clone();
    }
    if ours == base {
        return theirs.clone();
    }
    let (Ok(b), Ok(o), Ok(t)) = (serde_json::to_value(base), serde_json::to_value(ours), serde_json::to_value(theirs)) else {
        return ours.clone();
    };
    serde_json::from_value(merge_json(&b, &o, &t)).unwrap_or_else(|_| ours.clone())
}

fn merge_json(b: &Value, o: &Value, t: &Value) -> Value {
    if t == b {
        return o.clone();
    }
    if o == b {
        return t.clone();
    }
    match (b, o, t) {
        (Value::Object(bm), Value::Object(om), Value::Object(tm)) => {
            let mut out = serde_json::Map::new();
            let null = Value::Null;
            let keys: std::collections::BTreeSet<&String> = bm.keys().chain(om.keys()).chain(tm.keys()).collect();
            for k in keys {
                let v = merge_json(bm.get(k).unwrap_or(&null), om.get(k).unwrap_or(&null), tm.get(k).unwrap_or(&null));
                if !v.is_null() || om.contains_key(k) {
                    out.insert(k.clone(), v);
                }
            }
            Value::Object(out)
        }
        _ => o.clone(),
    }
}

/// Merge beat boards beat by beat (by id): each beat merges field by field; one deleted on a
/// side the other left alone is deleted; new beats from both sides are kept (ours first).
fn merge_board(base: &BeatBoard, ours: &BeatBoard, theirs: &BeatBoard) -> BeatBoard {
    if theirs == base {
        return ours.clone();
    }
    if ours == base {
        return theirs.clone();
    }
    let mut beats = Vec::new();
    for o in &ours.beats {
        match (base.get(o.id), theirs.get(o.id)) {
            (Some(b), Some(t)) => beats.push(merge_value(b, o, t)),
            (Some(b), None) if b != o => beats.push(o.clone()), // they deleted it, we changed it: keep
            (Some(_), None) => {}
            (None, _) => beats.push(o.clone()),
        }
    }
    for t in &theirs.beats {
        let kept = beats.iter().any(|b: &Beat| b.id == t.id);
        match base.get(t.id) {
            None if !kept => beats.push(t.clone()),
            Some(b) if !kept && ours.get(t.id).is_none() && b != t => beats.push(t.clone()),
            _ => {}
        }
    }
    let mut out = BeatBoard { beats };
    out.normalize();
    out
}

/// `theirs` with its new beats renumbered where ours added a different beat under the same id
/// (both sides pick "highest id + 1"); `None` when nothing collides.
fn renumber_new_beats(base: &Script, ours: &Script, theirs: &Script) -> Option<Script> {
    let clash: Vec<u32> =
        theirs.board.beats.iter().filter(|t| base.board.get(t.id).is_none() && ours.board.get(t.id).is_some_and(|o| o != *t)).map(|t| t.id).collect();
    if clash.is_empty() {
        return None;
    }
    let mut next = ours.board.beats.iter().chain(&theirs.board.beats).map(|b| b.id).max().unwrap_or(0);
    let mut map = std::collections::HashMap::new();
    for id in clash {
        next = next.checked_add(1)?;
        map.insert(id, next);
    }
    let mut t = theirs.clone();
    for b in &mut t.board.beats {
        if let Some(n) = map.get(&b.id) {
            b.id = *n;
        }
    }
    for p in &mut t.paras {
        for id in &mut p.beats {
            if let Some(n) = map.get(id) {
                *id = *n;
            }
        }
    }
    Some(t)
}

/// Two hunks from different sides conflict when they touch the same base paragraphs (or are
/// insertions at the same point, or one inserts inside the other's range).
fn touches(s: usize, e: usize, ks: usize, ke: usize) -> bool {
    match (s == e, ks == ke) {
        (true, true) => s == ks,
        (true, false) => ks < s && s < ke,
        (false, true) => s < ks && ks < e,
        (false, false) => ks < e && s < ke,
    }
}

struct Group<'a> {
    s: usize,
    e: usize,
    ours: Vec<&'a Hunk>,
    theirs: Vec<&'a Hunk>,
}

/// `base[s..e]` with these hunks (all inside it) applied.
fn side(base: &[Para], s: usize, e: usize, hs: &[&Hunk]) -> Vec<Para> {
    let mut out = Vec::new();
    let mut i = s;
    for h in hs {
        out.extend_from_slice(base.get(i..h.at.max(i)).unwrap_or(&[]));
        out.extend(h.ins.iter().cloned());
        i = h.at + h.del;
    }
    out.extend_from_slice(base.get(i.min(e)..e).unwrap_or(&[]));
    out
}

fn merge_paras(base: &[Para], ours: &[Para], theirs: &[Para], ctx: &Script) -> Vec<Para> {
    let ho = hunks(base, ours);
    let ht = hunks(base, theirs);
    // All hunks in base order; ours first on ties.
    let mut all: Vec<(&Hunk, bool)> = ho.iter().map(|h| (h, true)).chain(ht.iter().map(|h| (h, false))).collect();
    all.sort_by_key(|(h, mine)| (h.at, h.at + h.del, !*mine));
    let mut groups: Vec<Group> = Vec::new();
    for (h, mine) in all {
        let (ks, ke) = (h.at, h.at + h.del);
        match groups.last_mut() {
            Some(g) if touches(g.s, g.e, ks, ke) => {
                g.s = g.s.min(ks);
                g.e = g.e.max(ke);
                if mine { g.ours.push(h) } else { g.theirs.push(h) }
            }
            _ => groups.push(Group { s: ks, e: ke, ours: if mine { vec![h] } else { vec![] }, theirs: if mine { vec![] } else { vec![h] } }),
        }
    }
    let mut out = Vec::with_capacity(ours.len().max(theirs.len()));
    let mut i = 0;
    for g in &groups {
        out.extend_from_slice(base.get(i..g.s.max(i)).unwrap_or(&[]));
        let o = side(base, g.s, g.e, &g.ours);
        let t = side(base, g.s, g.e, &g.theirs);
        if g.theirs.is_empty() {
            out.extend(o);
        } else if g.ours.is_empty() {
            out.extend(t);
        } else {
            out.extend(merge_region(base.get(g.s..g.e).unwrap_or(&[]), &o, &t, ctx));
        }
        i = g.e.max(i);
    }
    out.extend_from_slice(base.get(i..).unwrap_or(&[]));
    out
}

fn join(ps: &[Para]) -> String {
    ps.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n")
}

/// Linear offset in a region joined with `\n` → position.
fn pos_in(ps: &[Para], off: usize) -> Pos {
    let mut rest = off;
    for (i, p) in ps.iter().enumerate() {
        if rest <= p.text.len() {
            return Pos::new(i, rest);
        }
        rest -= p.text.len() + 1;
    }
    let last = ps.len().saturating_sub(1);
    Pos::new(last, ps.get(last).map_or(0, |p| p.text.len()))
}

/// The paragraph and style of the character at a linear offset of a region.
fn char_info(ps: &[Para], off: usize) -> (Element, Style) {
    let p = pos_in(ps, off);
    let para = ps.get(p.para);
    let style = para.and_then(|q| q.styles().get(p.off).copied()).unwrap_or(Style::PLAIN);
    (para.map_or(Element::Action, |q| q.element), style)
}

/// One edit to replay on `ours`, in ours' linear coordinates.
enum Op {
    Delete(usize, usize),
    /// Insert `theirs[ta..tb]` (linear) at a point.
    Insert(usize, usize, usize),
}

/// Both sides changed the same paragraphs: replay their character edits onto ours.
fn merge_region(base: &[Para], ours: &[Para], theirs: &[Para], ctx: &Script) -> Vec<Para> {
    if ours == theirs {
        return ours.to_vec();
    }
    if base.is_empty() {
        // Two insertions at the same point: keep both, ours first.
        let mut v = ours.to_vec();
        v.extend(theirs.iter().cloned());
        return v;
    }
    if ours.is_empty() || theirs.is_empty() {
        // A side deleted these paragraphs: the deletion wins.
        return Vec::new();
    }
    let (lb, lo, lt) = (join(base), join(ours), join(theirs));
    let so = text_splices(&lb, &lo);
    let st = text_splices(&lb, &lt);
    // Base offset → ours offset. `after` puts the point after an ours insertion there.
    let map = |x: usize, after: bool| -> usize {
        let mut delta: isize = 0;
        for sp in &so {
            if sp.a < x && x < sp.b {
                return sp.tb;
            }
            let before = sp.b < x || (sp.b == x && sp.a < sp.b) || (sp.a == x && sp.b == x && after);
            if !before {
                break;
            }
            delta += (sp.tb - sp.ta) as isize - (sp.b - sp.a) as isize;
        }
        (x as isize + delta).max(0) as usize
    };
    let mut ops: Vec<Op> = Vec::new();
    for t in &st {
        // Delete what is left of base[a..b] in ours (ours' own replacements are kept).
        let mut cur = t.a;
        for sp in so.iter().filter(|sp| sp.b > t.a && sp.a < t.b) {
            if sp.a > cur {
                ops.push(Op::Delete(map(cur, true), map(sp.a, false)));
            }
            cur = cur.max(sp.b);
        }
        if cur < t.b {
            ops.push(Op::Delete(map(cur, true), map(t.b, false)));
        }
        if t.tb > t.ta {
            ops.push(Op::Insert(map(t.a, true), t.ta, t.tb));
        }
    }
    // From the end; at one point, delete before inserting.
    ops.sort_by_key(|op| match *op {
        Op::Delete(x, y) => (std::cmp::Reverse(x), 0u8, std::cmp::Reverse(y)),
        Op::Insert(p, ..) => (std::cmp::Reverse(p), 1u8, std::cmp::Reverse(p)),
    });
    let mut work = Script { title: ctx.title.clone(), paras: ours.to_vec(), settings: ctx.settings.clone(), ..Script::default() };
    for op in ops {
        match op {
            Op::Delete(x, y) if y > x => {
                let (a, b) = (pos_in(&work.paras, x), pos_in(&work.paras, y));
                work.delete_range(a, b);
            }
            Op::Delete(..) => {}
            Op::Insert(p, ta, tb) => {
                let at = pos_in(&work.paras, p);
                insert_from(&mut work, at, theirs, &lt, ta, tb);
            }
        }
    }
    let mut merged = work.paras;
    if merged.len() == base.len() && ours.len() == base.len() && theirs.len() == base.len() {
        for (k, m) in merged.iter_mut().enumerate() {
            if let (Some(b), Some(o), Some(t)) = (base.get(k), ours.get(k), theirs.get(k)) {
                *m = merge_fields(b, o, t, m);
            }
        }
    }
    merged
}

/// Insert `theirs[ta..tb]` (linear, may contain paragraph breaks) at `at`, with their styles and,
/// for new paragraphs, their elements.
fn insert_from(work: &mut Script, at: Pos, theirs: &[Para], lt: &str, ta: usize, tb: usize) {
    let mut pos = at;
    let Some(text) = lt.get(ta..tb) else { return };
    let mut run = String::new();
    let mut run_style = Style::PLAIN;
    let flush = |work: &mut Script, pos: &mut Pos, run: &mut String, st: Style| {
        if !run.is_empty() {
            if let Ok(p) = work.insert_styled(*pos, run, st) {
                *pos = p;
            }
            run.clear();
        }
    };
    for (i, c) in text.char_indices() {
        let off = ta + i;
        if c == '\n' {
            flush(work, &mut pos, &mut run, run_style);
            let (el, _) = char_info(theirs, off + 1);
            if let Ok(p) = work.split(pos, el) {
                pos = p;
            }
            continue;
        }
        let (_, st) = char_info(theirs, off);
        if st != run_style && !run.is_empty() {
            flush(work, &mut pos, &mut run, run_style);
        }
        run_style = st;
        run.push(c);
    }
    flush(work, &mut pos, &mut run, run_style);
}

/// Text-bearing fields come from the replayed paragraph; the rest merge field by field. Range
/// data the other side changed is taken when the merged text is exactly theirs.
fn merge_fields(b: &Para, o: &Para, t: &Para, merged: &Para) -> Para {
    const TEXTUAL: [&str; 5] = ["text", "spans", "notes", "tagged", "revs"];
    let (Ok(mut bv), Ok(mut ov), Ok(mut tv), Ok(mv)) =
        (serde_json::to_value(b), serde_json::to_value(o), serde_json::to_value(t), serde_json::to_value(merged))
    else {
        return merged.clone();
    };
    for v in [&mut bv, &mut ov, &mut tv] {
        if let Some(m) = v.as_object_mut() {
            for k in TEXTUAL {
                m.remove(k);
            }
        }
    }
    let mut out = merge_json(&bv, &ov, &tv);
    if let (Some(om), Some(mm)) = (out.as_object_mut(), mv.as_object()) {
        for k in TEXTUAL {
            let take_theirs = merged.text == t.text && field(o, k) == field(b, k) && field(t, k) != field(b, k);
            let v = if take_theirs { field(t, k) } else { mm.get(k).cloned().unwrap_or(Value::Null) };
            if !v.is_null() {
                om.insert(k.to_string(), v);
            }
        }
    }
    serde_json::from_value(out).unwrap_or_else(|_| merged.clone())
}

fn field(p: &Para, k: &str) -> Value {
    serde_json::to_value(p).ok().and_then(|v| v.get(k).cloned()).unwrap_or(Value::Null)
}

/// Where `pos` in `old` ends up in `new` (text inserted before it pushes it along).
pub fn transform_pos(old: &Script, new: &Script, pos: Pos) -> Pos {
    let pos = old.clamp(pos);
    let mut shift: isize = 0;
    for h in hunks(&old.paras, &new.paras) {
        let end = h.at + h.del;
        if end <= pos.para {
            shift += h.ins.len() as isize - h.del as isize;
            continue;
        }
        if h.at > pos.para {
            break;
        }
        // The caret's paragraph changed: follow the characters.
        let start = (h.at as isize + shift).max(0) as usize;
        if h.ins.is_empty() {
            return new.clamp(Pos::new(start, 0));
        }
        let region_old = old.paras.get(h.at..end).unwrap_or(&[]);
        let lin: usize = region_old.iter().take(pos.para - h.at).map(|p| p.text.len() + 1).sum::<usize>() + pos.off;
        let mut x = lin as isize;
        for sp in text_splices(&join(region_old), &join(&h.ins)) {
            if sp.a < lin && sp.b <= lin {
                x += (sp.tb - sp.ta) as isize - (sp.b - sp.a) as isize;
            } else if sp.a < lin && lin < sp.b {
                x = sp.tb as isize;
                break;
            } else if !(sp.a == lin && sp.b == lin) {
                break;
            }
        }
        let p = pos_in(&h.ins, x.max(0) as usize);
        return new.clamp(Pos::new(start + p.para, p.off));
    }
    new.clamp(Pos::new((pos.para as isize + shift).max(0) as usize, pos.off))
}
