//! Courier Prime, a Courier-metric screenplay typeface by Alan Dague-Greene for Quote-Unquote
//! Apps, licensed under the SIL Open Font License 1.1 (see `fonts/OFL.txt` and ATTRIBUTION.md).
//! Every glyph advances 0.6 em, so 12-point text sets at exactly ten characters per inch.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

/// Courier Prime Regular.
pub const REGULAR: &[u8] = include_bytes!("../fonts/CourierPrime-Regular.ttf");
/// Courier Prime Bold.
pub const BOLD: &[u8] = include_bytes!("../fonts/CourierPrime-Bold.ttf");
/// Courier Prime Italic.
pub const ITALIC: &[u8] = include_bytes!("../fonts/CourierPrime-Italic.ttf");
/// Courier Prime Bold Italic.
pub const BOLD_ITALIC: &[u8] = include_bytes!("../fonts/CourierPrime-BoldItalic.ttf");
/// The font's licence text.
pub const LICENSE: &str = include_str!("../fonts/OFL.txt");
/// The family name.
pub const FAMILY: &str = "Courier Prime";

#[cfg(test)]
mod tests {
    #[test]
    fn fonts_are_truetype() {
        for f in [super::REGULAR, super::BOLD, super::ITALIC, super::BOLD_ITALIC] {
            assert_eq!(f.get(..4), Some(&[0, 1, 0, 0][..]));
        }
        assert!(super::LICENSE.contains("SIL Open Font License"));
    }
}
