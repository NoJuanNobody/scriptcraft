//! ScriptCraft L0: units and page geometry.
//!
//! Screenplays are set in 12-point Courier on US Letter paper: ten characters per inch and six
//! lines per inch. Everything here is in PDF points (1/72 inch) measured from the page's top-left.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Serialize};

/// Points per inch.
pub const PT_PER_IN: f32 = 72.0;
/// US Letter width, points.
pub const PAGE_W: f32 = 8.5 * PT_PER_IN;
/// US Letter height, points.
pub const PAGE_H: f32 = 11.0 * PT_PER_IN;
/// The type size: 12 point.
pub const FONT_SIZE: f32 = 12.0;
/// One line: 6 lines per inch.
pub const LINE_H: f32 = PT_PER_IN / 6.0;
/// One Courier character: 10 characters per inch (0.6 em at 12 pt).
pub const CHAR_W: f32 = PT_PER_IN / 10.0;
/// Top margin of the body text (1").
pub const TOP: f32 = PT_PER_IN;
/// Bottom margin of the body text (1").
pub const BOTTOM: f32 = PT_PER_IN;
/// Baseline offset inside a 12-point line box (Courier's ascent at 12 pt, rounded).
pub const BASELINE: f32 = 9.5;

/// Inches to points.
pub const fn inch(v: f32) -> f32 {
    v * PT_PER_IN
}

/// Lines of body text per page (54 with 1" margins).
pub const fn body_lines() -> usize {
    ((PAGE_H - TOP - BOTTOM) / LINE_H) as usize
}

/// How many monospace characters fit between two x positions (points), at least 1.
pub fn chars_between(left: f32, right: f32) -> usize {
    let w = ((right - left) / CHAR_W).floor();
    if w.is_finite() && w >= 1.0 { (w as usize).min(10_000) } else { 1 }
}

/// A rectangle in points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_page_metrics() {
        assert_eq!(PAGE_W, 612.0);
        assert_eq!(PAGE_H, 792.0);
        assert_eq!(body_lines(), 54);
        assert_eq!(chars_between(inch(1.5), inch(7.5)), 60);
        assert_eq!(chars_between(inch(2.5), inch(6.0)), 35);
        assert_eq!(chars_between(5.0, 1.0), 1);
        assert_eq!(chars_between(0.0, f32::NAN), 1);
    }
}
