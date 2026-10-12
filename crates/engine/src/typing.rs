//! Final Draft-style element flow: what Enter and Tab do in each element.
//! The behaviour is documented in `docs/typing-parity.md` and pinned by `tests_typing.rs`.

use scriptcraft_doc::Element;

/// The element a new paragraph gets after Enter at the end of `e`.
pub const fn enter_next(e: Element) -> Element {
    match e {
        Element::SceneHeading => Element::Action,
        Element::Action => Element::Action,
        Element::Character => Element::Dialogue,
        Element::Parenthetical => Element::Dialogue,
        Element::Dialogue => Element::Action,
        Element::Transition => Element::SceneHeading,
        Element::Shot => Element::Action,
        Element::General => Element::General,
    }
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

/// Tab at the end of a non-empty paragraph starts a new paragraph of this element.
pub const fn tab_next(e: Element) -> Element {
    match e {
        Element::SceneHeading => Element::Action,
        Element::Action => Element::Character,
        Element::Character => Element::Parenthetical,
        Element::Parenthetical => Element::Dialogue,
        Element::Dialogue => Element::Parenthetical,
        Element::Transition => Element::SceneHeading,
        Element::Shot => Element::Action,
        Element::General => Element::Character,
    }
}

/// Tab in an empty paragraph (or with the caret inside the text) changes its element.
pub const fn tab_empty(e: Element) -> Element {
    match e {
        Element::Action => Element::Character,
        Element::Character => Element::Transition,
        Element::Transition => Element::SceneHeading,
        Element::SceneHeading => Element::Action,
        Element::Dialogue => Element::Parenthetical,
        Element::Parenthetical => Element::Dialogue,
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

/// Action typed as a scene-heading prefix becomes a Scene Heading (`int. ` → `INT. `).
pub fn looks_like_heading(text: &str) -> bool {
    let up = text.to_uppercase();
    ["INT. ", "EXT. ", "INT./EXT. ", "EXT./INT. ", "I/E. ", "I/E ", "EST. "].iter().any(|p| up.starts_with(p))
}
