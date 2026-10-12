//! Colour tokens for the light and dark themes (our own palette).

use egui::{Color32, Visuals};

#[derive(Clone, Copy)]
pub struct Tokens {
    pub dark: bool,
    /// Menu bar, toolbar, status bar.
    pub chrome: Color32,
    pub chrome_line: Color32,
    /// Sidebars.
    pub panel: Color32,
    /// The desk behind the pages.
    pub desk: Color32,
    pub paper: Color32,
    pub paper_edge: Color32,
    pub ink: Color32,
    /// Page numbers, (MORE), scene numbers.
    pub ink_soft: Color32,
    pub text: Color32,
    pub text_soft: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub caret: Color32,
    pub card: Color32,
    pub card_band: [Color32; 4],
}

impl Tokens {
    pub fn get(dark: bool) -> Tokens {
        if dark {
            Tokens {
                dark,
                chrome: Color32::from_rgb(0x1d, 0x1f, 0x24),
                chrome_line: Color32::from_rgb(0x30, 0x33, 0x3a),
                panel: Color32::from_rgb(0x22, 0x25, 0x2b),
                desk: Color32::from_rgb(0x14, 0x16, 0x1a),
                paper: Color32::from_rgb(0x26, 0x28, 0x2e),
                paper_edge: Color32::from_rgb(0x3a, 0x3d, 0x45),
                ink: Color32::from_rgb(0xe4, 0xe2, 0xdc),
                ink_soft: Color32::from_rgb(0x9a, 0x9d, 0xa6),
                text: Color32::from_rgb(0xe6, 0xe7, 0xea),
                text_soft: Color32::from_rgb(0x96, 0x9a, 0xa4),
                accent: Color32::from_rgb(0x6f, 0x8c, 0xff),
                selection: Color32::from_rgba_unmultiplied(0x6f, 0x8c, 0xff, 80),
                caret: Color32::from_rgb(0x9d, 0xb2, 0xff),
                card: Color32::from_rgb(0x2b, 0x2e, 0x35),
                card_band: [
                    Color32::from_rgb(0x55, 0x6c, 0xd6),
                    Color32::from_rgb(0x3f, 0x9c, 0x8a),
                    Color32::from_rgb(0xc0, 0x84, 0x3c),
                    Color32::from_rgb(0xa8, 0x5c, 0x9a),
                ],
            }
        } else {
            Tokens {
                dark,
                chrome: Color32::from_rgb(0xf3, 0xf3, 0xf5),
                chrome_line: Color32::from_rgb(0xd8, 0xd9, 0xde),
                panel: Color32::from_rgb(0xfa, 0xfa, 0xfb),
                desk: Color32::from_rgb(0xdc, 0xde, 0xe3),
                paper: Color32::WHITE,
                paper_edge: Color32::from_rgb(0xc9, 0xcc, 0xd3),
                ink: Color32::from_rgb(0x16, 0x16, 0x18),
                ink_soft: Color32::from_rgb(0x70, 0x73, 0x7c),
                text: Color32::from_rgb(0x1e, 0x20, 0x26),
                text_soft: Color32::from_rgb(0x6b, 0x6f, 0x7a),
                accent: Color32::from_rgb(0x3b, 0x5b, 0xdb),
                selection: Color32::from_rgba_unmultiplied(0x3b, 0x5b, 0xdb, 56),
                caret: Color32::from_rgb(0x2b, 0x47, 0xb5),
                card: Color32::WHITE,
                card_band: [
                    Color32::from_rgb(0x4c, 0x6e, 0xf5),
                    Color32::from_rgb(0x2f, 0x9e, 0x86),
                    Color32::from_rgb(0xe0, 0x8e, 0x2b),
                    Color32::from_rgb(0xb5, 0x4f, 0xa6),
                ],
            }
        }
    }

    /// egui visuals in our colours.
    pub fn visuals(&self) -> Visuals {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = self.chrome;
        v.window_fill = self.panel;
        v.extreme_bg_color = if self.dark { Color32::from_rgb(0x17, 0x19, 0x1d) } else { Color32::WHITE };
        v.selection.bg_fill = self.accent;
        v.selection.stroke.color = Color32::WHITE;
        v.hyperlink_color = self.accent;
        v.widgets.noninteractive.bg_stroke.color = self.chrome_line;
        v.override_text_color = Some(self.text);
        v
    }
}
