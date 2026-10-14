//! Element settings: where each element sits on the page and how Enter and Tab move between
//! elements (Final Draft's Elements dialog). Defaults are the industry indents and the flow in
//! `docs/typing-parity.md`.

use serde::{Deserialize, Serialize};

use crate::Element;

/// Horizontal alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    pub fn name(self) -> &'static str {
        match self {
            Align::Left => "Left",
            Align::Center => "Center",
            Align::Right => "Right",
        }
    }
    pub fn parse(s: &str) -> Option<Align> {
        match s.to_ascii_lowercase().as_str() {
            "left" | "full" => Some(Align::Left),
            "center" | "centre" => Some(Align::Center),
            "right" => Some(Align::Right),
            _ => None,
        }
    }
}

/// One element's settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementFormat {
    /// Left edge, inches from the paper's left edge.
    pub left: f32,
    /// Right edge, inches from the paper's left edge.
    pub right: f32,
    /// Blank lines before the element.
    pub before: u8,
    pub align: Align,
    /// Typed in capitals.
    pub uppercase: bool,
    /// The element Enter starts after this one.
    pub enter: Element,
    /// The element Tab starts at the end of a non-empty paragraph.
    pub tab: Element,
    /// The element an empty paragraph (or one with the caret mid-text) becomes on Tab.
    pub tab_empty: Element,
    /// Line spacing: 1 = single, 2 = a blank line between lines (multi-camera dialogue).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub spacing: u8,
    /// Each paragraph of this element starts a new page (multi-camera scenes, act breaks).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub new_page: bool,
}

fn one() -> u8 {
    1
}
fn is_one(v: &u8) -> bool {
    *v == 1
}

/// Most line spacing kept.
pub const MAX_SPACING: u8 = 3;

/// Limits for hostile values.
pub const MIN_INCH: f32 = 0.0;
pub const MAX_INCH: f32 = 8.5;
pub const MAX_BEFORE: u8 = 6;

impl ElementFormat {
    /// The defaults for an element.
    pub const fn default_for(e: Element) -> ElementFormat {
        use Element::*;
        let (left, right, before, align) = match e {
            SceneHeading => (1.5, 7.5, 1, Align::Left),
            Action => (1.5, 7.5, 1, Align::Left),
            Character => (3.7, 7.5, 1, Align::Left),
            Parenthetical => (3.1, 5.6, 0, Align::Left),
            Dialogue => (2.5, 6.0, 0, Align::Left),
            Transition => (4.0, 7.5, 1, Align::Right),
            Shot => (1.5, 7.5, 1, Align::Left),
            General => (1.5, 7.5, 0, Align::Left),
        };
        let (enter, tab, tab_empty) = match e {
            SceneHeading => (Action, Action, Action),
            Action => (Action, Character, Character),
            Character => (Dialogue, Parenthetical, Transition),
            Parenthetical => (Dialogue, Dialogue, Dialogue),
            Dialogue => (Action, Parenthetical, Parenthetical),
            Transition => (SceneHeading, SceneHeading, SceneHeading),
            Shot => (Action, Action, Action),
            General => (General, Character, Action),
        };
        ElementFormat { left, right, before, align, uppercase: e.uppercase(), enter, tab, tab_empty, spacing: 1, new_page: false }
    }

    /// Clamp to sane values (hostile files and parameters).
    pub fn sanitized(mut self) -> ElementFormat {
        let fin = |v: f32, d: f32| if v.is_finite() { v } else { d };
        self.left = fin(self.left, 1.5).clamp(MIN_INCH, MAX_INCH - 0.5);
        self.right = fin(self.right, 7.5).clamp(self.left + 0.5, MAX_INCH);
        self.before = self.before.min(MAX_BEFORE);
        self.spacing = self.spacing.clamp(1, MAX_SPACING);
        self
    }
}

/// Settings for all eight elements.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementFormats(pub [ElementFormat; 8]);

impl Default for ElementFormats {
    fn default() -> Self {
        ElementFormats(Element::ALL.map(ElementFormat::default_for))
    }
}

impl ElementFormats {
    pub fn get(&self, e: Element) -> ElementFormat {
        self.0.get(e.index()).copied().unwrap_or(ElementFormat::default_for(e))
    }
    pub fn set(&mut self, e: Element, f: ElementFormat) {
        if let Some(slot) = self.0.get_mut(e.index()) {
            *slot = f.sanitized();
        }
    }
    pub fn is_default(&self) -> bool {
        *self == ElementFormats::default()
    }
}
