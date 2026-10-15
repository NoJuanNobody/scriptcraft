//! Dual dialogue across page breaks.
//!
//! Both columns split at the same row. Each column that continues gets `(MORE)` under its last
//! line and its cue with `(CONT'D)` at the top of the next page; a column that already ended
//! gets nothing. A block taller than a page splits as many times as it needs; no line is ever
//! dropped.
//!
//! The repeated cue always stays inside its column: it sits where the cue does when it fits,
//! moves left (never past the column's left edge) when it doesn't, and when even the whole
//! column is too narrow the name goes on its own line(s) with `(CONT'D)` under it.

use scriptcraft_geom::{CHAR_W, LINE_H, chars_between};

use super::{Builder, Element, Item, LineKind, deco, inch, sentence_end, spec_in, wrap};

/// A column's rows: (item, line) pairs in reading order.
type Rows = Vec<(usize, usize)>;

fn rows_of(items: &[Item]) -> Rows {
    items.iter().enumerate().flat_map(|(a, it)| (0..it.lines.len()).map(move |k| (a, k))).collect()
}

/// Where a column's cue sits (points), for `(MORE)` and `(CONT'D)`.
fn cue_x(items: &[Item]) -> f32 {
    items.iter().find(|i| i.el == Element::Character).and_then(|i| i.cols).or_else(|| items.first().and_then(|i| i.cols)).map_or(inch(3.5), |c| c.0)
}

/// A column's left and right edges (points): the widest of its items.
fn col_bounds(items: &[Item]) -> (f32, f32) {
    let l = items.iter().filter_map(|i| i.cols.map(|c| c.0)).fold(f32::INFINITY, f32::min);
    let r = items.iter().filter_map(|i| i.cols.map(|c| c.1)).fold(f32::NEG_INFINITY, f32::max);
    if l.is_finite() && r.is_finite() && r > l { (l, r) } else { (inch(1.5), inch(4.3)) }
}

/// The lines of a continued cue `NAME (CONT'D)` for a column, as (x, text), each inside the
/// column (see the module docs).
pub(crate) fn contd_lines(items: &[Item], name: &str) -> Vec<(f32, String)> {
    let full = if name.contains("CONT'D") { name.to_string() } else { format!("{name} (CONT'D)") };
    let cue = cue_x(items);
    let (l, r) = col_bounds(items);
    let width = chars_between(l, r);
    let at = |text: &str| {
        let w = text.chars().count() as f32 * CHAR_W;
        (cue.min(r - w)).max(l)
    };
    if full.chars().count() <= width {
        return vec![(at(&full), full)];
    }
    let mut out: Vec<(f32, String)> = wrap(name, width).into_iter().filter_map(|(s, e)| name.get(s..e)).map(|t| (at(t), t.to_string())).collect();
    if !name.contains("CONT'D") {
        out.push((at("(CONT'D)"), "(CONT'D)".to_string()));
    }
    out
}

impl Builder<'_> {
    /// Place a dual-dialogue block (left column `a`, right column `b`).
    pub(crate) fn dual(&mut self, a: &[Item], b: &[Item]) {
        let before = a.first().map_or(1, |i| spec_in(self.script, i.el).before);
        let sides: [(&[Item], Rows); 2] = [(a, rows_of(a)), (b, rows_of(b))];
        let height = |from: usize| sides.iter().map(|(_, r)| r.len().saturating_sub(from)).max().unwrap_or(0);
        let mut from = 0;
        if self.row > 0 && self.row + before + height(0) > self.rows {
            let room = self.left().saturating_sub(before);
            if let Some(r) = self.dual_split(&sides, 0, room, false) {
                self.gap(before);
                self.dual_place(&sides, 0, r, true);
                self.dual_break(&sides, r);
                from = r;
            } else {
                self.new_page();
            }
        } else {
            self.gap(before);
        }
        // The rest, splitting again at the top of each page if it is still taller than the page.
        let mut guard = 0usize;
        while height(from) > 0 && guard < 100_000 {
            guard += 1;
            let remain = height(from);
            let room = self.left();
            if remain <= room {
                self.dual_place(&sides, from, from + remain, false);
                return;
            }
            let r = self.dual_split(&sides, from, room, true).unwrap_or(1).max(1);
            self.dual_place(&sides, from, from + r, true);
            self.dual_break(&sides, from + r);
            from += r;
        }
    }

    /// How many rows (from `from`) go on this page, or `None` when no good split exists here.
    /// `force` (at the top of a page) always finds one, so a block taller than a page still
    /// makes progress.
    fn dual_split(&self, sides: &[(&[Item], Rows); 2], from: usize, room: usize, force: bool) -> Option<usize> {
        let more = usize::from(self.script.settings.more_contd);
        let remain = sides.iter().map(|(_, r)| r.len().saturating_sub(from)).max().unwrap_or(0);
        let max = room.saturating_sub(more).min(remain.saturating_sub(1));
        if max == 0 {
            return force.then_some(1);
        }
        let row_el = |items: &[Item], rows: &Rows, i: usize| rows.get(i).and_then(|&(a, _)| items.get(a)).map(|it| it.el);
        let row_text = |items: &[Item], rows: &Rows, i: usize| {
            rows.get(i)
                .and_then(|&(a, k)| items.get(a).and_then(|it| it.lines.get(k).map(|&(s, e)| (it.para, s, e))))
                .and_then(|(p, s, e)| self.script.paras.get(p).and_then(|p| p.text.get(s..e)))
                .unwrap_or("")
        };
        // Every column that continues past row `from + r` must keep its cue with a line, not end
        // on a parenthetical, and leave at least one line for the next page.
        let valid = |r: usize| {
            sides.iter().all(|(items, rows)| {
                let end = from + r;
                if rows.len() <= end {
                    return true;
                }
                (from > 0 || r >= 2) && row_el(items, rows, end - 1) != Some(Element::Parenthetical)
            })
        };
        let good = |r: usize| {
            sides.iter().all(|(items, rows)| {
                let end = from + r;
                rows.len() <= end
                    || sentence_end(row_text(items, rows, end - 1))
                    || rows.get(end - 1).zip(rows.get(end)).is_some_and(|(x, y)| x.0 != y.0)
            })
        };
        (1..=max).rev().find(|&r| valid(r) && good(r)).or_else(|| (1..=max).rev().find(|&r| valid(r))).or_else(|| force.then_some(max))
    }

    /// Place rows `from..to` of both columns side by side; with `more`, a column that continues
    /// past `to` gets `(MORE)` under its last line.
    fn dual_place(&mut self, sides: &[(&[Item], Rows); 2], from: usize, to: usize, more: bool) {
        let top = self.row;
        let mut bottom = top;
        for (items, rows) in sides {
            self.row = top;
            for &(a, k) in rows.get(from.min(rows.len())..to.min(rows.len())).unwrap_or(&[]) {
                if let Some(it) = items.get(a) {
                    self.emit(it, k, false);
                }
            }
            if more && self.script.settings.more_contd && rows.len() > to {
                let y = self.y();
                self.cur.push(deco(LineKind::More, cue_x(items), y, "(MORE)"));
                self.row += 1;
            }
            bottom = bottom.max(self.row);
        }
        self.row = bottom;
    }

    /// A page break inside the block at row `at`: a new page with `NAME (CONT'D)` over each
    /// column that continues.
    fn dual_break(&mut self, sides: &[(&[Item], Rows); 2], at: usize) {
        self.new_page();
        if !self.script.settings.more_contd {
            return;
        }
        let y = self.y();
        let mut used = 0usize;
        for (items, rows) in sides {
            if rows.len() <= at {
                continue;
            }
            let Some(cue) = items.iter().find(|i| i.el == Element::Character) else { continue };
            let name = self.script.paras.get(cue.para).map_or(String::new(), |p| p.text.clone());
            let lines = contd_lines(items, &name);
            for (k, (x, text)) in lines.iter().enumerate() {
                self.cur.push(deco(LineKind::Contd, *x, y + k as f32 * LINE_H, text));
            }
            used = used.max(lines.len());
        }
        self.row += used;
    }
}
