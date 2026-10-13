//! Locked pages and omitted scenes.
//!
//! When pages are locked, each locked page remembers the paragraph it started with
//! (`Para::page_lock`, the labels of that page and of any pages its text ran onto). Laying out
//! again, every such paragraph starts a new page, so earlier pages keep their breaks:
//! - text that no longer fits flows onto A/B pages after the last locked label (`12A`, `12B`);
//! - a page whose text shrank takes the numbers of the pages it replaces (`12-13`);
//! - pages added before the first locked page are `A1`, `B1`…
//!
//! Locking a layout again keeps its A/B labels as the new locked pages.

use scriptcraft_doc::{Script, expand_page_label, next_label};

/// What an omitted scene's heading prints.
pub const OMITTED: &str = "OMITTED";

/// Most pages one locked segment may claim (hostile files).
const MAX_LABELS: usize = 1000;

/// The locked labels that start at paragraph `para`, if it carries any.
pub(crate) fn anchor(script: &Script, para: Option<usize>) -> Option<Vec<String>> {
    let p = script.paras.get(para?)?;
    (!p.page_lock.is_empty()).then(|| p.page_lock.iter().take(MAX_LABELS).cloned().collect())
}

/// The number a label counts as for gaps: `12` → 12, `12A` → 12, `12-14` → 14; `A1` has none.
fn last_number(label: &str) -> Option<u32> {
    let tail = label.rsplit('-').next().unwrap_or(label).trim();
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() { None } else { digits.parse().ok() }
}

fn first_number(label: &str) -> Option<u32> {
    let digits: String = label.trim().chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() { None } else { digits.parse().ok() }
}

/// Page labels for `n` pages, given where each locked segment starts (page index) and its
/// labels.
pub(crate) fn assign(n: usize, segments: &[(usize, Vec<String>)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(n);
    let Some(first) = segments.first() else {
        return (1..=n).map(|k| k.to_string()).collect();
    };
    // Pages before the first locked page: A1, B1… before its label.
    let base = first.1.first().cloned().unwrap_or_else(|| "1".into());
    let mut lead = format!("A{base}");
    for _ in 0..first.0.min(n) {
        out.push(lead.clone());
        lead = next_label(&lead);
    }
    // Each segment's page range; empty segments hand their labels to the one before.
    let mut segs: Vec<(usize, usize, Vec<String>)> = Vec::new();
    for (k, (start, labels)) in segments.iter().enumerate() {
        let start = (*start).min(n);
        let end = segments.get(k + 1).map_or(n, |s| s.0).clamp(start, n);
        if end == start
            && let Some(prev) = segs.last_mut()
        {
            prev.2.extend(labels.iter().cloned());
            continue;
        }
        segs.push((start, end, labels.clone()));
    }
    // Numbers missing between segments (a locked page's first paragraph was deleted) belong to
    // the segment before the gap.
    for k in 0..segs.len().saturating_sub(1) {
        let a = segs.get(k).and_then(|s| s.2.last()).and_then(|l| last_number(l));
        let b = segs.get(k + 1).and_then(|s| s.2.first()).and_then(|l| first_number(l));
        if let (Some(a), Some(b)) = (a, b)
            && b > a + 1
            && let Some(s) = segs.get_mut(k)
        {
            s.2.extend((a + 1..b).take(MAX_LABELS).map(|x| x.to_string()));
        }
    }
    for (start, end, labels) in segs {
        let m = end - start;
        if m == 0 {
            continue;
        }
        let labels: Vec<String> = labels.iter().flat_map(|l| expand_page_label(l)).take(MAX_LABELS).collect();
        if labels.is_empty() {
            for _ in 0..m {
                let l = out.last().map_or_else(|| "1".into(), |l| next_label(l));
                out.push(l);
            }
            continue;
        }
        if m >= labels.len() {
            let mut last = labels.last().cloned().unwrap_or_default();
            let extra = m - labels.len();
            out.extend(labels);
            for _ in 0..extra {
                last = next_label(&last);
                out.push(last.clone());
            }
        } else {
            out.extend(labels.iter().take(m - 1).cloned());
            let from = labels.get(m - 1).cloned().unwrap_or_default();
            let to = labels.last().cloned().unwrap_or_default();
            out.push(combine(&from, &to));
        }
    }
    out.resize(n, String::new());
    out
}

/// One page standing for several: `12` and `13` → `12-13`.
fn combine(from: &str, to: &str) -> String {
    if from == to { from.to_string() } else { format!("{from}-{to}") }
}

#[cfg(test)]
mod tests {
    use super::assign;

    fn seg(start: usize, labels: &[&str]) -> (usize, Vec<String>) {
        (start, labels.iter().map(|l| l.to_string()).collect())
    }

    #[test]
    fn labels_for_locked_segments() {
        // Unchanged: every page keeps its number.
        assert_eq!(assign(3, &[seg(0, &["1"]), seg(1, &["2"]), seg(2, &["3"])]), ["1", "2", "3"]);
        // A segment that grew by two pages: A/B pages after its last label.
        assert_eq!(assign(5, &[seg(0, &["1"]), seg(1, &["2"]), seg(4, &["3"])]), ["1", "2", "2A", "2B", "3"]);
        // A segment that held two pages and now fits on one: a combined page.
        assert_eq!(assign(2, &[seg(0, &["1", "2"]), seg(1, &["3"])]), ["1-2", "3"]);
        // A locked page whose first paragraph was deleted: its number joins the page before.
        assert_eq!(assign(2, &[seg(0, &["1"]), seg(1, &["3"])]), ["1-2", "3"]);
        assert_eq!(assign(3, &[seg(0, &["1"]), seg(2, &["3"])]), ["1", "2", "3"]);
        // A segment that produced no page hands its labels to the page before.
        assert_eq!(assign(2, &[seg(0, &["1"]), seg(1, &["2"]), seg(1, &["3"])]), ["1-2", "3"]);
        // Pages before the first locked page.
        assert_eq!(assign(3, &[seg(2, &["1"])]), ["A1", "B1", "1"]);
        // Re-locked A pages grow further: 2A → 2B.
        assert_eq!(assign(3, &[seg(0, &["2", "2A"])]), ["2", "2A", "2B"]);
        // No locks at all.
        assert_eq!(assign(2, &[]), ["1", "2"]);
        // Never more or fewer labels than pages.
        assert_eq!(assign(0, &[seg(0, &["1"])]).len(), 0);
        assert_eq!(assign(4, &[seg(9, &["1"])]).len(), 4);
    }
}
