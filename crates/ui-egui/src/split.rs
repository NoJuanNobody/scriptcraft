//! Split screen: the main area as one pane, or two panes (side by side or stacked) showing views
//! of the same script. Each pane has its own view and scroll position (and zoom for the page
//! view); the caret and selection are the script's, shared by both. The pane last clicked is the
//! focused one: View-menu choices, zoom and jumps to the caret go to it.
//!
//! Every view is drawn through [`show_view`], so a new view needs one match arm there (and a
//! [`crate::View`] variant) to work in either pane.

use egui::{Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::canvas::CanvasState;
use crate::{ScriptApp, View};

/// How the main area is split.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitMode {
    Off,
    /// Two panes side by side (a vertical divider).
    SideBySide,
    /// Two panes one above the other (a horizontal divider).
    Stacked,
}

impl SplitMode {
    pub fn id(self) -> &'static str {
        match self {
            SplitMode::Off => "off",
            SplitMode::SideBySide => "sideBySide",
            SplitMode::Stacked => "stacked",
        }
    }
    pub fn parse(s: &str) -> Option<SplitMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "single" => Some(SplitMode::Off),
            "side" | "sidebyside" | "side-by-side" | "vertical" | "columns" => Some(SplitMode::SideBySide),
            "stacked" | "horizontal" | "rows" | "top-bottom" => Some(SplitMode::Stacked),
            _ => None,
        }
    }
}

/// The split state: the second pane's view and page canvas, which pane has focus, the divider.
pub struct Split {
    pub mode: SplitMode,
    /// The second pane's view (the first pane's is [`ScriptApp::view`]).
    pub second: View,
    /// The focused pane: 0 or 1.
    pub focus: usize,
    /// Where the divider is, as a fraction of the main area.
    pub frac: f32,
    /// The second pane's page canvas (scroll, zoom, page rectangles).
    pub canvas: CanvasState,
    /// The panes' screen rectangles last drawn.
    pub rects: [Option<Rect>; 2],
}

impl Default for Split {
    fn default() -> Self {
        let mut canvas = CanvasState::default();
        canvas.pane = 1;
        Split { mode: SplitMode::Off, second: View::Cards, focus: 0, frac: 0.5, canvas, rects: [None, None] }
    }
}

impl Split {
    pub fn on(&self) -> bool {
        self.mode != SplitMode::Off
    }
}

/// The view of pane `i`.
pub fn pane_view(app: &ScriptApp, i: usize) -> View {
    if i == 1 && app.split.on() { app.split.second } else { app.view }
}

/// The view of the focused pane.
pub fn active_view(app: &ScriptApp) -> View {
    pane_view(app, if app.split.on() { app.split.focus } else { 0 })
}

/// Show `v` in the focused pane.
pub fn set_view(app: &mut ScriptApp, v: View) {
    if app.split.on() && app.split.focus == 1 {
        app.split.second = v;
    } else {
        app.view = v;
    }
}

/// After a jump to a place in the script (navigator, cards, reports, notes, beats): make sure a
/// page view shows it, scrolled to the caret. With two panes and one of them a page view, that
/// pane follows; otherwise the focused pane becomes the page view.
pub fn show_page(app: &mut ScriptApp) {
    if app.split.on() {
        let page = [0, 1]
            .into_iter()
            .find(|i| pane_view(app, *i) == View::Page && *i == app.split.focus)
            .or_else(|| [0, 1].into_iter().find(|i| pane_view(app, *i) == View::Page));
        match page {
            Some(1) => app.split.canvas.scroll_to_caret = true,
            Some(_) => app.canvas.scroll_to_caret = true,
            None => {
                set_view(app, View::Page);
                follow_caret(app);
            }
        }
    } else {
        app.view = View::Page;
        app.canvas.scroll_to_caret = true;
    }
}

/// After a command moved the caret: scroll the page pane that should follow it.
pub fn follow_caret(app: &mut ScriptApp) {
    if app.split.on() && app.split.second == View::Page && (app.split.focus == 1 || app.view != View::Page) {
        app.split.canvas.scroll_to_caret = true;
    } else {
        app.canvas.scroll_to_caret = true;
    }
}

/// Whether a page view has the keyboard (its own keys then go to it, not the global shortcuts).
pub fn page_has_keys(app: &ScriptApp, ctx: &egui::Context) -> bool {
    (0..2).any(|i| (i == 0 || app.split.on()) && pane_view(app, i) == View::Page && ctx.memory(|m| m.has_focus(crate::canvas::canvas_id_for(i))))
}

/// Run `f` with the focused pane's page canvas in [`ScriptApp::canvas`] (zoom commands).
pub fn with_focused_canvas<R>(app: &mut ScriptApp, f: impl FnOnce(&mut ScriptApp) -> R) -> R {
    let swap = app.split.on() && app.split.focus == 1 && app.split.second == View::Page;
    if swap {
        std::mem::swap(&mut app.canvas, &mut app.split.canvas);
    }
    let r = f(app);
    if swap {
        std::mem::swap(&mut app.canvas, &mut app.split.canvas);
    }
    r
}

/// Draw one view. The one place views are dispatched: add new views here.
pub fn show_view(app: &mut ScriptApp, ui: &mut Ui, view: View) {
    match view {
        View::Page => crate::canvas::show(app, ui),
        View::Cards => crate::cards::show(app, ui),
        View::Reports => crate::reports::show(app, ui),
        View::Outline => crate::outline::show(app, ui),
        View::BeatBoard => crate::beatboard::show(app, ui),
    }
}

/// Draw pane `i` of the main area in `ui`.
fn pane(app: &mut ScriptApp, ui: &mut Ui, i: usize) {
    let view = pane_view(app, i);
    ui.push_id(("pane", i), |ui| {
        if i == 1 && view == View::Page {
            std::mem::swap(&mut app.canvas, &mut app.split.canvas);
            show_view(app, ui, view);
            std::mem::swap(&mut app.canvas, &mut app.split.canvas);
        } else {
            show_view(app, ui, view);
        }
    });
}

/// The main area: one pane, or two with a draggable divider.
pub fn central(app: &mut ScriptApp, ui: &mut Ui) {
    if !app.split.on() {
        app.split.rects = [Some(ui.max_rect()), None];
        pane(app, ui, 0);
        return;
    }
    let full = ui.max_rect();
    let side = app.split.mode == SplitMode::SideBySide;
    let len = if side { full.width() } else { full.height() };
    let gap = 6.0;
    let min = 160.0_f32.min(len * 0.3);
    let cut = (len * app.split.frac).clamp(min, (len - min).max(min));
    let (a, d, b) = if side {
        (
            Rect::from_min_max(full.min, pos2(full.left() + cut - gap / 2.0, full.bottom())),
            Rect::from_min_max(pos2(full.left() + cut - gap / 2.0, full.top()), pos2(full.left() + cut + gap / 2.0, full.bottom())),
            Rect::from_min_max(pos2(full.left() + cut + gap / 2.0, full.top()), full.max),
        )
    } else {
        (
            Rect::from_min_max(full.min, pos2(full.right(), full.top() + cut - gap / 2.0)),
            Rect::from_min_max(pos2(full.left(), full.top() + cut - gap / 2.0), pos2(full.right(), full.top() + cut + gap / 2.0)),
            Rect::from_min_max(pos2(full.left(), full.top() + cut + gap / 2.0), full.max),
        )
    };
    app.split.rects = [Some(a), Some(b)];
    // The pane pressed in becomes the focused one.
    if let Some(p) = ui.input(|inp| if inp.pointer.any_pressed() { inp.pointer.interact_pos() } else { None }) {
        if a.contains(p) {
            app.split.focus = 0;
        } else if b.contains(p) {
            app.split.focus = 1;
        }
    }
    for (i, r) in [a, b].into_iter().enumerate() {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(r);
        pane(app, &mut child, i);
    }
    // The divider.
    let t = app.tokens();
    let resp = ui.interact(d, ui.id().with("split-divider"), Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(if side { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let at = if side { p.x - full.left() } else { p.y - full.top() };
        app.split.frac = (at / len.max(1.0)).clamp(0.15, 0.85);
    }
    let p = ui.painter();
    p.rect_filled(d, 0.0, t.chrome);
    let mid = d.center();
    let grip = if side { vec2(2.0, 28.0) } else { vec2(28.0, 2.0) };
    p.rect_filled(Rect::from_center_size(mid, grip), 1.0, t.text_soft.gamma_multiply(if resp.hovered() { 1.0 } else { 0.5 }));
    // The focused pane gets a thin accent outline.
    if let Some(r) = app.split.rects.get(app.split.focus).copied().flatten() {
        p.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(2.0, t.accent.gamma_multiply(0.7)), StrokeKind::Inside);
    }
}
