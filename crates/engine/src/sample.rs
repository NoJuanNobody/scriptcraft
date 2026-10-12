//! The built-in sample screenplay: an original short scene written for ScriptCraft
//! (dedicated to the public domain, CC0 1.0; see ATTRIBUTION.md).

use scriptcraft_doc::{Element, Script};

/// The sample as Fountain.
pub const FOUNTAIN: &str = include_str!("sample.fountain");

/// The sample screenplay.
pub fn script() -> Script {
    let mut s = scriptcraft_fountain::parse(FOUNTAIN);
    // Fountain has no Shot element: the sample's one shot is set here.
    for p in &mut s.paras {
        if p.element == Element::Action && p.text == "CLOSE ON THE BAROMETER" {
            p.element = Element::Shot;
        }
    }
    s
}
