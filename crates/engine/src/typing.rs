//! Final Draft-style element flow: what Enter and Tab do in each element. Enter and Tab
//! targets are per-script element settings (`ElementFormat::default_for` has the defaults).
//! The behaviour is documented in `docs/typing-parity.md` and pinned by `tests_typing.rs`.

use scriptcraft_doc::{Element, Script, Sluglines};

/// The element a new paragraph gets after Enter at the end of `e` (this script's settings).
pub fn enter_next(s: &Script, e: Element) -> Element {
    s.settings.elements.get(e).enter
}

/// Tab at the end of a non-empty paragraph starts a new paragraph of this element.
pub fn tab_next(s: &Script, e: Element) -> Element {
    s.settings.elements.get(e).tab
}

/// Tab in an empty paragraph (or with the caret inside the text) changes its element.
pub fn tab_empty(s: &Script, e: Element) -> Element {
    s.settings.elements.get(e).tab_empty
}

/// Enter in an empty paragraph changes its element in place (no new paragraph).
pub const fn enter_empty(e: Element) -> Element {
    match e {
        Element::Action => Element::SceneHeading,
        Element::SceneHeading => Element::Action,
        Element::Character => Element::Action,
        Element::Parenthetical => Element::Dialogue,
        Element::Dialogue => Element::Action,
        Element::Transition => Element::SceneHeading,
        Element::Shot => Element::Action,
        Element::General => Element::Action,
    }
}

/// Shift+Tab changes the element backwards.
pub const fn shift_tab(e: Element) -> Element {
    match e {
        Element::Character => Element::Action,
        Element::Transition => Element::Character,
        Element::SceneHeading => Element::Transition,
        Element::Parenthetical => Element::Dialogue,
        Element::Dialogue => Element::Character,
        Element::Action => Element::SceneHeading,
        Element::Shot => Element::Action,
        Element::General => Element::Action,
    }
}

/// Action typed as one of the user's scene-heading prefixes becomes a Scene Heading (`int. ` →
/// `INT. `), but only while the paragraph is no longer than the prefix and its space.
pub fn looks_like_heading(words: &Sluglines, text: &str) -> bool {
    let longest = words.prefixes().first().map_or(0, |(w, _)| w.trim().chars().count());
    text.chars().take(longest + 2).count() <= longest + 1 && words.starts_heading(text)
}
