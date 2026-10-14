//! Toolbar icons, drawn in code (original; see ATTRIBUTION.md).

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    New,
    Open,
    Save,
    Pdf,
    Page,
    Cards,
    Reports,
    Outline,
    StoryMap,
    Navigator,
    Find,
    Moon,
    Sun,
    Dual,
    Numbers,
    Title,
    Undo,
    Redo,
    Beats,
    Split,
}

/// Draw `icon` in `r` with colour `c`.
pub fn paint(ui: &Ui, icon: Icon, r: Rect, c: Color32) {
    let p = ui.painter();
    let s = Stroke::new(1.4, c);
    let cx = r.center();
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let page = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
    let lines = |x0: f32, x1: f32, ys: &[f32]| {
        for y in ys {
            p.line_segment([at(x0, *y), at(x1, *y)], Stroke::new(1.2, c));
        }
    };
    match icon {
        Icon::New => {
            p.rect_stroke(page(0.25, 0.15, 0.75, 0.85), 1.5, s, StrokeKind::Middle);
            p.line_segment([at(0.5, 0.35), at(0.5, 0.65)], s);
            p.line_segment([at(0.35, 0.5), at(0.65, 0.5)], s);
        }
        Icon::Open => {
            let pts = vec![at(0.15, 0.3), at(0.4, 0.3), at(0.48, 0.38), at(0.85, 0.38), at(0.85, 0.78), at(0.15, 0.78)];
            p.add(egui::Shape::closed_line(pts, s));
        }
        Icon::Save => {
            p.rect_stroke(page(0.2, 0.2, 0.8, 0.8), 2.0, s, StrokeKind::Middle);
            p.rect_stroke(page(0.33, 0.2, 0.67, 0.42), 0.0, s, StrokeKind::Middle);
            p.rect_filled(page(0.35, 0.58, 0.65, 0.8), 1.0, c);
        }
        Icon::Pdf => {
            p.rect_stroke(page(0.25, 0.15, 0.75, 0.85), 1.5, s, StrokeKind::Middle);
            p.line_segment([at(0.5, 0.3), at(0.5, 0.62)], s);
            p.line_segment([at(0.38, 0.52), at(0.5, 0.64)], s);
            p.line_segment([at(0.62, 0.52), at(0.5, 0.64)], s);
            lines(0.36, 0.64, &[0.74]);
        }
        Icon::Page | Icon::Title => {
            p.rect_stroke(page(0.25, 0.12, 0.75, 0.88), 1.5, s, StrokeKind::Middle);
            if icon == Icon::Page {
                lines(0.34, 0.66, &[0.3, 0.42]);
                lines(0.44, 0.6, &[0.56]);
                lines(0.38, 0.62, &[0.68]);
            } else {
                lines(0.38, 0.62, &[0.42]);
                lines(0.42, 0.58, &[0.52]);
            }
        }
        Icon::Cards => {
            for (x, y) in [(0.14, 0.2), (0.53, 0.2), (0.14, 0.56), (0.53, 0.56)] {
                p.rect_stroke(Rect::from_min_size(at(x, y), vec2(0.33 * r.width(), 0.26 * r.height())), 1.5, s, StrokeKind::Middle);
            }
        }
        Icon::Reports => {
            for (i, h) in [0.35, 0.6, 0.45].iter().enumerate() {
                let x = 0.22 + i as f32 * 0.22;
                p.rect_filled(page(x, 0.82 - h, x + 0.14, 0.82), 1.0, c);
            }
        }
        Icon::Outline => {
            // Indented rows: an act, then its scenes.
            for (x, y) in [(0.18, 0.26), (0.32, 0.44), (0.32, 0.6), (0.32, 0.76)] {
                p.circle_filled(at(x, y), 1.6, c);
                p.line_segment([at(x + 0.08, y), at(0.84, y)], Stroke::new(if x < 0.2 { 1.8 } else { 1.2 }, c));
            }
        }
        Icon::StoryMap => {
            // Bands over a row of scene blocks.
            p.rect_filled(page(0.12, 0.24, 0.52, 0.34), 1.0, c);
            p.rect_filled(page(0.55, 0.24, 0.88, 0.34), 1.0, c.gamma_multiply(0.6));
            for (x0, x1) in [(0.12, 0.3), (0.33, 0.45), (0.48, 0.7), (0.73, 0.88)] {
                p.rect_stroke(page(x0, 0.44, x1, 0.66), 1.0, Stroke::new(1.2, c), StrokeKind::Middle);
            }
            p.line_segment([at(0.12, 0.78), at(0.88, 0.78)], Stroke::new(1.0, c));
        }
        Icon::Navigator => {
            p.rect_stroke(page(0.15, 0.2, 0.85, 0.8), 2.0, s, StrokeKind::Middle);
            p.line_segment([at(0.4, 0.2), at(0.4, 0.8)], s);
            lines(0.2, 0.34, &[0.36, 0.5, 0.64]);
        }
        Icon::Find => {
            p.circle_stroke(at(0.44, 0.42), 0.2 * r.height(), s);
            p.line_segment([at(0.58, 0.58), at(0.8, 0.8)], Stroke::new(2.0, c));
        }
        Icon::Moon => {
            p.circle_filled(cx, 0.3 * r.height(), c);
            let bg = ui.visuals().panel_fill;
            p.circle_filled(cx + vec2(0.14 * r.width(), -0.1 * r.height()), 0.26 * r.height(), bg);
        }
        Icon::Sun => {
            p.circle_filled(cx, 0.16 * r.height(), c);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([cx + d * 0.26 * r.height(), cx + d * 0.36 * r.height()], s);
            }
        }
        Icon::Dual => {
            lines(0.15, 0.45, &[0.3, 0.45, 0.6, 0.75]);
            lines(0.55, 0.85, &[0.3, 0.45, 0.6, 0.75]);
        }
        Icon::Numbers => {
            p.text(at(0.5, 0.52), egui::Align2::CENTER_CENTER, "#", egui::FontId::proportional(0.62 * r.height()), c);
        }
        Icon::Beats => {
            // Loose cards joined by a thread.
            p.line_segment([at(0.3, 0.32), at(0.68, 0.66)], Stroke::new(1.0, c));
            for (x, y, w, h) in [(0.1, 0.18, 0.38, 0.28), (0.5, 0.54, 0.38, 0.28)] {
                p.rect_stroke(Rect::from_min_size(at(x, y), vec2(w * r.width(), h * r.height())), 1.5, s, StrokeKind::Middle);
            }
            p.circle_filled(at(0.74, 0.27), 0.07 * r.height(), c);
        }
        Icon::Split => {
            p.rect_stroke(page(0.14, 0.2, 0.86, 0.8), 2.0, s, StrokeKind::Middle);
            p.line_segment([at(0.5, 0.2), at(0.5, 0.8)], s);
            lines(0.22, 0.42, &[0.36, 0.5, 0.64]);
            lines(0.58, 0.78, &[0.36, 0.5]);
        }
        Icon::Undo | Icon::Redo => {
            let flip = if icon == Icon::Redo { -1.0 } else { 1.0 };
            let q = |x: f32, y: f32| at(0.5 + (x - 0.5) * flip, y);
            let pts: Vec<Pos2> = (0..=12)
                .map(|k| {
                    let a = std::f32::consts::PI * (1.0 + k as f32 / 12.0);
                    q(0.52 - 0.24 * a.cos(), 0.58 + 0.22 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([q(0.28, 0.58), q(0.2, 0.44)], s);
            p.line_segment([q(0.28, 0.58), q(0.42, 0.5)], s);
        }
    }
}

/// A toolbar button with an icon; `on` draws it selected.
pub fn button(ui: &mut Ui, icon: Icon, tip: &str, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 26.0), Sense::click());
    let v = ui.visuals();
    let fill = if on {
        v.selection.bg_fill.gamma_multiply(0.22)
    } else if resp.hovered() {
        v.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, 5.0, fill);
    let c = if on { v.selection.bg_fill } else { v.text_color() };
    paint(ui, icon, r.shrink2(vec2(6.0, 4.0)), c);
    resp.on_hover_text(tip)
}
