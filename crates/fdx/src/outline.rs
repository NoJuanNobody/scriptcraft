//! Structure points (acts, sequences, sections) in FDX.
//!
//! Final Draft keeps outline elements in the script itself: hidden paragraphs in `<Content>` of
//! type `Outline 1` (top level), `Outline 2`, `Outline 3`…, each followed by its description in
//! `Summary` paragraphs, with `<ElementSettings Type="Outline 1">` describing how they look
//! (`Behavior PaginateAs="Outline"`, hidden text). That is what we write: acts as `Outline 1`,
//! sequences as `Outline 2`, sections as `Outline 3` (deeper levels read as sections), just
//! before the scene they open. The point's colour has no known FDX form and uses a
//! ScriptCraft-only `ScriptCraftColor` attribute.

use scriptcraft_doc::{CardColor, MAX_STRUCTURE, Para, StructureKind, StructurePoint};

use crate::{Kind, Open, color_attr, esc};

/// Outline paragraphs read so far.
#[derive(Default)]
pub(crate) struct Read {
    /// (the number of script paragraphs read before it, the point).
    pub points: Vec<(usize, StructurePoint)>,
    /// The last point may still take `Summary` paragraphs.
    open: bool,
}

/// `Outline 2` → 2.
fn outline_level(ty: &str) -> Option<usize> {
    let rest = ty.trim().strip_prefix("Outline")?.trim();
    if rest.is_empty() {
        return Some(1);
    }
    rest.parse::<usize>().ok().filter(|n| *n >= 1)
}

impl Read {
    /// Take a finished body paragraph if it belongs to the outline (an `Outline N` paragraph, or
    /// a `Summary` paragraph right after one); `false` hands it to the script.
    pub fn take(&mut self, o: &Open, paras_read: usize) -> bool {
        if o.kind != Kind::Body {
            return false;
        }
        let ty = o.ty.as_deref().unwrap_or("");
        if let Some(level) = outline_level(ty) {
            if self.points.len() < MAX_STRUCTURE {
                self.points.push((paras_read, StructurePoint::new(StructureKind::from_level(level), &o.text, "", o.color)));
            }
            self.open = true;
            return true;
        }
        if self.open && ty.trim().eq_ignore_ascii_case("Summary") {
            if let Some((_, sp)) = self.points.last_mut() {
                let t = o.text.trim();
                if !t.is_empty() {
                    if !sp.summary.is_empty() {
                        sp.summary.push(' ');
                    }
                    sp.summary.push_str(t);
                    sp.clean();
                }
            }
            return true;
        }
        self.open = false;
        false
    }

    /// Seat the points before the paragraphs they preceded (normalizing then moves each onto the
    /// next scene heading); returns the points after the last paragraph.
    pub fn apply(self, paras: &mut [Para]) -> Vec<StructurePoint> {
        let mut tail = Vec::new();
        for (at, sp) in self.points {
            match paras.get_mut(at) {
                Some(p) => p.structure.push(sp),
                None => tail.push(sp),
            }
        }
        tail
    }
}

/// Write points as `Outline N` paragraphs with their `Summary`.
pub(crate) fn write_points(out: &mut String, indent: &str, points: &[StructurePoint]) {
    for sp in points {
        let color = sp.color.map(|c: CardColor| format!(" ScriptCraftColor=\"{}\"", color_attr(c))).unwrap_or_default();
        out.push_str(&format!(
            "{indent}<Paragraph Type=\"Outline {}\"{color}>\n{indent}  <Text>{}</Text>\n{indent}</Paragraph>\n",
            sp.kind.level(),
            esc(&sp.title)
        ));
        if !sp.summary.trim().is_empty() {
            out.push_str(&format!("{indent}<Paragraph Type=\"Summary\">\n{indent}  <Text>{}</Text>\n{indent}</Paragraph>\n", esc(sp.summary.trim())));
        }
    }
}

/// `<ElementSettings>` for the outline levels (hidden, paginated as outline).
pub(crate) fn write_settings(out: &mut String) {
    for (level, size, left) in [(1, 16, 0.88), (2, 14, 1.12), (3, 12, 1.38)] {
        out.push_str(&format!(
            "  <ElementSettings Type=\"Outline {level}\">\n    <FontSpec Font=\"Courier Prime\" Size=\"{size}\" Style=\"Bold+HiddenText\"/>\n    <ParagraphSpec Alignment=\"Left\" FirstIndent=\"0.00\" Leading=\"Regular\" LeftIndent=\"{left:.2}\" RightIndent=\"7.50\" SpaceBefore=\"{size}\" Spacing=\"1\" StartsNewPage=\"No\"/>\n    <Behavior PaginateAs=\"Outline\" ReturnKey=\"Summary\"/>\n  </ElementSettings>\n"
        ));
    }
}
