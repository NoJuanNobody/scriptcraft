//! Element types.

use serde::{Deserialize, Serialize};

/// A screenplay element type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Element {
    SceneHeading,
    Action,
    Character,
    Parenthetical,
    Dialogue,
    Transition,
    Shot,
    General,
}

impl Element {
    /// Every element, in menu order.
    pub const ALL: [Element; 8] = [
        Element::SceneHeading,
        Element::Action,
        Element::Character,
        Element::Parenthetical,
        Element::Dialogue,
        Element::Transition,
        Element::Shot,
        Element::General,
    ];

    /// The display name (also the FDX `Type`).
    pub const fn name(self) -> &'static str {
        match self {
            Element::SceneHeading => "Scene Heading",
            Element::Action => "Action",
            Element::Character => "Character",
            Element::Parenthetical => "Parenthetical",
            Element::Dialogue => "Dialogue",
            Element::Transition => "Transition",
            Element::Shot => "Shot",
            Element::General => "General",
        }
    }

    /// The stable id used by commands (`sceneHeading`).
    pub const fn id(self) -> &'static str {
        match self {
            Element::SceneHeading => "sceneHeading",
            Element::Action => "action",
            Element::Character => "character",
            Element::Parenthetical => "parenthetical",
            Element::Dialogue => "dialogue",
            Element::Transition => "transition",
            Element::Shot => "shot",
            Element::General => "general",
        }
    }

    /// Parse a name or id, forgiving case, spaces and common aliases (`slug`, `paren`, `cue`).
    pub fn parse(s: &str) -> Option<Element> {
        let k: String = s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        Some(match k.as_str() {
            "sceneheading" | "scene" | "heading" | "slug" | "slugline" => Element::SceneHeading,
            "action" | "description" => Element::Action,
            "character" | "cue" | "char" => Element::Character,
            "parenthetical" | "paren" | "wryly" => Element::Parenthetical,
            "dialogue" | "dialog" => Element::Dialogue,
            "transition" => Element::Transition,
            "shot" => Element::Shot,
            "general" => Element::General,
            _ => return None,
        })
    }

    /// Typed in capitals (Scene Heading, Character, Transition, Shot).
    pub const fn uppercase(self) -> bool {
        matches!(self, Element::SceneHeading | Element::Character | Element::Transition | Element::Shot)
    }

    /// Part of a dialogue block (Character, Parenthetical, Dialogue).
    pub const fn in_dialogue(self) -> bool {
        matches!(self, Element::Character | Element::Parenthetical | Element::Dialogue)
    }
}

impl std::fmt::Display for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
