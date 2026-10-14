//! Compare two drafts, paragraph by paragraph.
//!
//! Paragraphs are aligned by their element and text (longest common subsequence). In each run
//! of differing paragraphs, removed and added paragraphs that are mostly the same words pair up
//! as **changed**, with a word-level diff. A removed paragraph whose exact text comes back
//! elsewhere is **moved**. What remains is **added** or **removed**. Duplicate paragraphs are
//! matched in order, one to one.

use scriptcraft_doc::track::lcs_pairs;
use scriptcraft_doc::{Element, Script};
use serde::Serialize;

/// Most cells for the paragraph and word tables (bigger inputs fall back to coarser diffs).
const CAP: usize = 16_000_000;

/// What happened to a paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffKind {
    Added,
    Removed,
    Changed,
    Moved,
}

/// A piece of a word-level diff.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    /// `same`, `added` or `removed`.
    pub op: &'static str,
    pub text: String,
}

/// One difference.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub kind: DiffKind,
    /// Paragraph in the older draft.
    pub old: Option<usize>,
    /// Paragraph in the newer draft.
    pub new: Option<usize>,
    pub element: Element,
    /// Changed: the element in the older draft, when it differs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_element: Option<Element>,
    /// The paragraph's text (the newer one, or the removed one).
    pub text: String,
    /// Changed: word-level pieces.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<Piece>,
    /// The scene it is in (heading text) in the draft it belongs to.
    pub scene: String,
}

/// The whole comparison.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub moved: usize,
    pub unchanged: usize,
    pub diffs: Vec<Diff>,
}

/// Word tokens: runs of non-space and runs of space, so joining them gives the text back.
fn tokens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev_space: Option<bool> = None;
    for (i, c) in s.char_indices() {
        let sp = c.is_whitespace();
        if prev_space.is_some_and(|p| p != sp) {
            out.push(s.get(start..i).unwrap_or(""));
            start = i;
        }
        prev_space = Some(sp);
    }
    if start < s.len() {
        out.push(s.get(start..).unwrap_or(""));
    }
    out
}

/// Word-level diff of two texts.
pub fn word_diff(a: &str, b: &str) -> Vec<Piece> {
    let (x, y) = (tokens(a), tokens(b));
    let pairs = lcs_pairs(&x, &y, |p, q| p == q, CAP);
    let mut out: Vec<Piece> = Vec::new();
    let mut push = |op: &'static str, t: &str| {
        if t.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(l) if l.op == op => l.text.push_str(t),
            _ => out.push(Piece { op, text: t.to_string() }),
        }
    };
    let (mut i, mut j) = (0, 0);
    for (pi, pj) in pairs.into_iter().chain(std::iter::once((x.len(), y.len()))) {
        while i < pi {
            push("removed", x.get(i).copied().unwrap_or(""));
            i += 1;
        }
        while j < pj {
            push("added", y.get(j).copied().unwrap_or(""));
            j += 1;
        }
        if let Some(t) = x.get(pi) {
            push("same", t);
        }
        i = pi + 1;
        j = pj + 1;
    }
    if out.is_empty() && a.is_empty() && !b.is_empty() {
        out.push(Piece { op: "added", text: b.to_string() });
    }
    out
}

/// How alike two texts are: shared words over all words (0.0–1.0).
fn similarity(a: &str, b: &str) -> f32 {
    let wa: Vec<String> = a.split_whitespace().map(str::to_lowercase).collect();
    let wb: Vec<String> = b.split_whitespace().map(str::to_lowercase).collect();
    if wa.is_empty() && wb.is_empty() {
        return 1.0;
    }
    let mut pool = wb.clone();
    let mut shared = 0;
    for w in &wa {
        if let Some(k) = pool.iter().position(|x| x == w) {
            pool.swap_remove(k);
            shared += 1;
        }
    }
    (2 * shared) as f32 / (wa.len() + wb.len()) as f32
}

fn scene_names(s: &Script) -> Vec<String> {
    let mut cur = String::new();
    s.paras
        .iter()
        .map(|p| {
            if p.element == Element::SceneHeading {
                cur = p.text.clone();
            }
            cur.clone()
        })
        .collect()
}

/// Compare an older draft with a newer one.
pub fn compare(old: &Script, new: &Script) -> Report {
    let (o, n) = (&old.paras, &new.paras);
    let key_eq = |a: &scriptcraft_doc::Para, b: &scriptcraft_doc::Para| a.element == b.element && a.text == b.text;
    let mut pre = 0;
    while let (Some(a), Some(b)) = (o.get(pre), n.get(pre)) {
        if !key_eq(a, b) {
            break;
        }
        pre += 1;
    }
    let mut suf = 0;
    while suf < o.len() - pre && suf < n.len() - pre {
        match (o.get(o.len() - 1 - suf), n.get(n.len() - 1 - suf)) {
            (Some(a), Some(b)) if key_eq(a, b) => suf += 1,
            _ => break,
        }
    }
    let om = o.get(pre..o.len() - suf).unwrap_or(&[]);
    let nm = n.get(pre..n.len() - suf).unwrap_or(&[]);
    let mut matched: Vec<(usize, usize)> = lcs_pairs(om, nm, key_eq, CAP).into_iter().map(|(i, j)| (i + pre, j + pre)).collect();
    let mut rep = Report { unchanged: pre + suf + matched.len(), ..Report::default() };
    let (os, ns) = (scene_names(old), scene_names(new));
    let scene = |v: &[String], i: usize| v.get(i).cloned().unwrap_or_default();

    // Runs of differences between matches.
    matched.push((o.len() - suf, n.len() - suf));
    let mut prev = (pre, pre);
    let mut removed: Vec<usize> = Vec::new();
    let mut added: Vec<usize> = Vec::new();
    let mut changed: Vec<(usize, usize)> = Vec::new();
    for &(i, j) in &matched {
        let run_o: Vec<usize> = (prev.0..i).collect();
        let run_n: Vec<usize> = (prev.1..j).collect();
        prev = (i + 1, j + 1);
        // Edited in place: as many paragraphs of the same elements on both sides.
        if !run_o.is_empty()
            && run_o.len() == run_n.len()
            && run_o.iter().zip(&run_n).all(|(&a, &b)| o.get(a).map(|p| p.element) == n.get(b).map(|p| p.element))
        {
            changed.extend(run_o.iter().copied().zip(run_n.iter().copied()));
            continue;
        }
        // Pair in order: each removed paragraph with the most alike added one after the last pair.
        let mut next_n = 0;
        for &a in &run_o {
            let pa = o.get(a);
            let best = run_n
                .iter()
                .enumerate()
                .skip(next_n)
                .filter_map(|(k, &b)| Some((k, b, similarity(&pa?.text, &n.get(b)?.text))))
                .filter(|(_, b, sim)| *sim >= 0.5 || (o.get(a).map(|x| &x.text) == n.get(*b).map(|x| &x.text)))
                .max_by(|x, y| x.2.total_cmp(&y.2));
            match best {
                Some((k, b, _)) => {
                    added.extend(run_n.get(next_n..k).unwrap_or(&[]));
                    changed.push((a, b));
                    next_n = k + 1;
                }
                None => removed.push(a),
            }
        }
        added.extend(run_n.get(next_n..).unwrap_or(&[]));
    }
    // Moves: a removed paragraph whose exact text was added elsewhere.
    let mut moved: Vec<(usize, usize)> = Vec::new();
    removed.retain(|&a| {
        let Some(pa) = o.get(a).filter(|p| !p.text.trim().is_empty()) else { return true };
        if let Some(k) = added.iter().position(|&b| n.get(b).is_some_and(|pb| key_eq(pa, pb))) {
            moved.push((a, added.remove(k)));
            return false;
        }
        true
    });
    for (a, b) in changed {
        let (Some(pa), Some(pb)) = (o.get(a), n.get(b)) else { continue };
        rep.diffs.push(Diff {
            kind: DiffKind::Changed,
            old: Some(a),
            new: Some(b),
            element: pb.element,
            old_element: (pa.element != pb.element).then_some(pa.element),
            text: pb.text.clone(),
            pieces: word_diff(&pa.text, &pb.text),
            scene: scene(&ns, b),
        });
    }
    for (a, b) in moved {
        let Some(pb) = n.get(b) else { continue };
        rep.diffs.push(Diff {
            kind: DiffKind::Moved,
            old: Some(a),
            new: Some(b),
            element: pb.element,
            old_element: None,
            text: pb.text.clone(),
            pieces: Vec::new(),
            scene: scene(&ns, b),
        });
    }
    for a in removed {
        let Some(pa) = o.get(a) else { continue };
        rep.diffs.push(Diff {
            kind: DiffKind::Removed,
            old: Some(a),
            new: None,
            element: pa.element,
            old_element: None,
            text: pa.text.clone(),
            pieces: Vec::new(),
            scene: scene(&os, a),
        });
    }
    for b in added {
        let Some(pb) = n.get(b) else { continue };
        rep.diffs.push(Diff {
            kind: DiffKind::Added,
            old: None,
            new: Some(b),
            element: pb.element,
            old_element: None,
            text: pb.text.clone(),
            pieces: Vec::new(),
            scene: scene(&ns, b),
        });
    }
    // Reading order of the newer draft (removed ones where they were).
    let order = |d: &Diff| -> (usize, usize) {
        match (d.new, d.old) {
            (Some(b), _) => (b, 1),
            (None, Some(a)) => {
                // Just after the newer paragraph that matched the one before it.
                let before = matched.iter().take_while(|(i, _)| *i < a).last().map_or(0, |(_, j)| j + 1);
                (before, 0)
            }
            _ => (usize::MAX, 0),
        }
    };
    rep.diffs.sort_by_key(order);
    for d in &rep.diffs {
        match d.kind {
            DiffKind::Added => rep.added += 1,
            DiffKind::Removed => rep.removed += 1,
            DiffKind::Changed => rep.changed += 1,
            DiffKind::Moved => rep.moved += 1,
        }
    }
    rep
}

/// A plain-text report (the CLI).
pub fn text_report(r: &Report) -> String {
    let mut out = format!("{} changed, {} added, {} removed, {} moved, {} unchanged\n", r.changed, r.added, r.removed, r.moved, r.unchanged);
    for d in &r.diffs {
        let tag = match d.kind {
            DiffKind::Added => "+",
            DiffKind::Removed => "-",
            DiffKind::Changed => "~",
            DiffKind::Moved => ">",
        };
        let at = match (d.old, d.new) {
            (Some(a), Some(b)) => format!("{}→{}", a + 1, b + 1),
            (Some(a), None) => format!("{}", a + 1),
            (None, Some(b)) => format!("{}", b + 1),
            _ => String::new(),
        };
        let body = if d.pieces.is_empty() {
            d.text.clone()
        } else {
            d.pieces
                .iter()
                .map(|p| match p.op {
                    "added" => format!("{{+{}+}}", p.text),
                    "removed" => format!("[-{}-]", p.text),
                    _ => p.text.clone(),
                })
                .collect()
        };
        let el = match d.old_element {
            Some(e) => format!("{} → {}", e.name(), d.element.name()),
            None => d.element.name().to_string(),
        };
        out.push_str(&format!("{tag} ¶{at} {el}: {body}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use scriptcraft_doc::{Element, Para, Script};

    use super::*;

    fn s(v: &[(Element, &str)]) -> Script {
        Script::from_paras(v.iter().map(|(e, t)| Para::new(*e, *t)).collect()).unwrap_or_default()
    }

    #[test]
    fn added_removed_changed() {
        let a = s(&[
            (Element::SceneHeading, "INT. A"),
            (Element::Action, "The rain falls on the roof."),
            (Element::Action, "Gone soon."),
            (Element::Action, "End."),
        ]);
        let b = s(&[
            (Element::SceneHeading, "INT. A"),
            (Element::Action, "The rain hammers on the tin roof."),
            (Element::Action, "Brand new line."),
            (Element::Action, "Another."),
            (Element::Action, "End."),
        ]);
        let r = compare(&a, &b);
        assert_eq!((r.changed, r.added, r.removed, r.moved, r.unchanged), (1, 2, 1, 0, 2), "{r:#?}");
        let c = r.diffs.iter().find(|d| d.kind == DiffKind::Changed).unwrap();
        assert!(c.pieces.iter().any(|p| p.op == "removed" && p.text.contains("falls")));
        assert!(c.pieces.iter().any(|p| p.op == "added" && p.text.contains("hammers")));
        let joined_new: String = c.pieces.iter().filter(|p| p.op != "removed").map(|p| p.text.as_str()).collect();
        assert_eq!(joined_new, "The rain hammers on the tin roof.");
        let joined_old: String = c.pieces.iter().filter(|p| p.op != "added").map(|p| p.text.as_str()).collect();
        assert_eq!(joined_old, "The rain falls on the roof.");
        assert_eq!(c.scene, "INT. A");
        // As many paragraphs on both sides, same elements: edited in place.
        let b2 = s(&[
            (Element::SceneHeading, "INT. A"),
            (Element::Action, "Totally different."),
            (Element::Action, "Also new."),
            (Element::Action, "End."),
        ]);
        let r = compare(&a, &b2);
        assert_eq!((r.changed, r.added, r.removed), (2, 0, 0));
    }

    #[test]
    fn moved_and_duplicates() {
        let a = s(&[
            (Element::Action, "One."),
            (Element::Action, "Two."),
            (Element::Action, "Three."),
            (Element::Action, "Dup."),
            (Element::Action, "Dup."),
        ]);
        let b = s(&[(Element::Action, "Three."), (Element::Action, "One."), (Element::Action, "Two."), (Element::Action, "Dup.")]);
        let r = compare(&a, &b);
        assert_eq!(r.moved, 1, "{r:#?}");
        assert_eq!(r.removed, 1, "one of the duplicates went");
        assert_eq!(r.added + r.changed, 0);
        // Same draft: nothing.
        let r = compare(&a, &a);
        assert!(r.diffs.is_empty() && r.unchanged == 5);
    }

    #[test]
    fn element_change_and_empty() {
        let a = s(&[(Element::Action, "hello there")]);
        let b = s(&[(Element::Dialogue, "hello there")]);
        let r = compare(&a, &b);
        assert_eq!(r.changed, 1);
        assert_eq!(r.diffs[0].old_element, Some(Element::Action));
        let r = compare(&Script::new(), &a);
        assert!(r.added >= 1);
        assert!(text_report(&r).contains("+ ¶"));
    }

    #[test]
    fn word_diff_edges() {
        assert!(word_diff("", "").is_empty());
        assert_eq!(word_diff("", "hi"), vec![Piece { op: "added", text: "hi".into() }]);
        let p = word_diff("café au lait", "café noir");
        let joined: String = p.iter().filter(|x| x.op != "removed").map(|x| x.text.as_str()).collect();
        assert_eq!(joined, "café noir");
    }

    #[test]
    fn structure_points_bookmarks_and_beats_are_not_differences() {
        use scriptcraft_doc::{Element, Para, Script, StructureKind, StructurePoint};
        let a = Script::from_paras(vec![
            Para::new(Element::SceneHeading, "INT. A - DAY"),
            Para::new(Element::Action, "One."),
            Para::new(Element::SceneHeading, "INT. B - DAY"),
            Para::new(Element::Action, "Two."),
        ])
        .unwrap_or_default();
        let mut b = a.clone();
        b.add_structure(StructurePoint::new(StructureKind::Act, "Act One", "", None), Some(0));
        b.add_structure(StructurePoint::new(StructureKind::Act, "Act Two", "", None), None);
        b.set_bookmark(3, Some("Mark")).ok();
        let id = b.board.add("Beat", "", None, 0.0, 0.0).unwrap_or(0);
        b.link_beat(id, Some(1)).ok();
        let r = compare(&a, &b);
        assert_eq!((r.added, r.removed, r.changed, r.moved), (0, 0, 0, 0));
        assert!(r.diffs.is_empty());
        // With a real edit as well: only that is reported.
        b.paras[3].text = "Two!".into();
        let r = compare(&a, &b);
        assert_eq!(r.changed, 1);
        let _ = text_report(&r);
    }
}
