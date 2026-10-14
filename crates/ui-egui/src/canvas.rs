//! The page view: paginated script on paper, caret, selection, typing and SmartType.

use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use scriptcraft_doc::{Element, Pos, word_left, word_right};
use scriptcraft_engine::smarttype::{Suggest, suggest};
use scriptcraft_geom::{CHAR_W, FONT_SIZE, LINE_H};
use scriptcraft_layout::{Layout, Line, LineKind};
use serde_json::json;

use crate::ScriptApp;

/// Gap between pages, points.
const GAP: f32 = 28.0;

pub struct CanvasState {
    /// Screen points per page point.
    pub zoom: f32,
    pub page_rects: Vec<Rect>,
    pub canvas_rect: Option<Rect>,
    pub scroll_to_caret: bool,
    /// Index of the highlighted SmartType item, and the list it belongs to.
    pub suggest_idx: usize,
    suggest_items: Vec<String>,
    pub suggest_shown: bool,
    pub caret_screen: Option<Rect>,
    /// Misspellings for a layout (by pointer) and dictionary size: (para, start, end).
    spell_cache: Option<(u64, usize, std::sync::Arc<Vec<(usize, usize, usize)>>)>,
    /// When the user last typed, clicked or moved the caret (the caret shows solid, then blinks).
    pub last_activity: f64,
    /// Blink the caret (off for screenshots and tests).
    pub blink: bool,
    /// The element list opened by Enter in an empty paragraph: the highlighted row.
    pub picker: Option<usize>,
    /// The misspelled word a context menu was opened on, with its suggestions.
    pub spell_target: Option<(usize, usize, usize, Vec<String>)>,
    /// Which split-screen pane this canvas belongs to (0 or 1; see `split.rs`).
    pub pane: usize,
}

impl Default for CanvasState {
    fn default() -> Self {
        CanvasState {
            zoom: 1.0,
            page_rects: Vec::new(),
            canvas_rect: None,
            scroll_to_caret: false,
            suggest_idx: 0,
            suggest_items: Vec::new(),
            suggest_shown: false,
            caret_screen: None,
            spell_cache: None,
            spell_target: None,
            picker: None,
            last_activity: 0.0,
            blink: true,
            pane: 0,
        }
    }
}

impl CanvasState {
    /// The misspellings the page view last underlined: (paragraph, start, end).
    pub fn underlined(&self) -> &[(usize, usize, usize)] {
        self.spell_cache.as_ref().map_or(&[], |(_, _, v)| v.as_slice())
    }
}

pub fn canvas_id() -> Id {
    Id::new("scriptcraft-page-view")
}

/// The page view's widget id in split-screen pane `pane` (pane 0 is [`canvas_id`]).
pub fn canvas_id_for(pane: usize) -> Id {
    if pane == 0 { canvas_id() } else { canvas_id().with(pane) }
}

fn courier(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("courier".into()))
}

/// The Courier Prime face for a style.
pub fn courier_styled(size: f32, st: scriptcraft_doc::Style) -> FontId {
    let fam = match (st.bold, st.italic) {
        (false, false) => "courier",
        (true, false) => "courier-bold",
        (false, true) => "courier-italic",
        (true, true) => "courier-bolditalic",
    };
    FontId::new(size, FontFamily::Name(fam.into()))
}

/// SmartType suggestions the popup would show now.
fn current_suggest(app: &ScriptApp) -> Option<Suggest> {
    let s = &app.session;
    if !s.sel.is_caret() || s.suggest_dismissed {
        return None;
    }
    let p = s.script.para(s.sel.focus.para)?;
    if p.text.trim().is_empty() {
        return None;
    }
    suggest(&s.script, s.sel.focus)
}

/// Misspellings of the current script, recomputed only when the script or dictionaries change.
/// Keyed by the session's revision (not the layout's address, which a new layout can reuse).
fn misspellings(app: &mut ScriptApp) -> std::sync::Arc<Vec<(usize, usize, usize)>> {
    if !app.spelling {
        return std::sync::Arc::new(Vec::new());
    }
    let id = app.session.revision();
    let dict = app.session.ignored_words.len() + app.session.user_words.len();
    if let Some((i, d, v)) = &app.canvas.spell_cache
        && *i == id
        && *d == dict
    {
        return v.clone();
    }
    let v = std::sync::Arc::new(scriptcraft_engine::cmd::spell::misspellings(&app.session, None));
    app.canvas.spell_cache = Some((id, dict, v.clone()));
    v
}

/// The shortcut string for a key press (`Mod+Shift+Z`).
pub fn shortcut(key: Key, m: Modifiers) -> String {
    let mut s = String::new();
    if m.command {
        s.push_str("Mod+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    if m.shift {
        s.push_str("Shift+");
    }
    s.push_str(match key {
        Key::Equals | Key::Plus => "=",
        Key::Minus => "-",
        k => k.name(),
    });
    s
}

/// Keyboard input while the page has focus.
fn handle_keys(app: &mut ScriptApp, ui: &Ui) {
    let events = ui.input(|i| i.events.clone());
    let mut sugg = current_suggest(app);
    for ev in events {
        match ev {
            egui::Event::Key { key, pressed: true, modifiers: m, .. } if app.canvas.picker.is_some() => {
                picker_key(app, key, m);
                sugg = current_suggest(app);
            }
            egui::Event::Text(t) => {
                if let Some(e) = app.canvas.picker.take()
                    && t.len() == 1
                    && let Some(k) = t.chars().next().and_then(|c| c.to_digit(10)).filter(|k| (1..=8).contains(k))
                {
                    let _ = e;
                    if let Some(el) = Element::ALL.get(k as usize - 1) {
                        app.run("element.set", json!({"element": el.id()}));
                    }
                    continue;
                }
                if !t.chars().all(char::is_control) {
                    app.run("text.insert", json!({"text": t}));
                }
            }
            egui::Event::Ime(egui::ImeEvent::Commit(t)) => app.run("text.insert", json!({"text": t})),
            egui::Event::Copy => {
                if let Ok(v) = app.execute("edit.copy", json!({})) {
                    ui.ctx().copy_text(v["text"].as_str().unwrap_or("").to_string());
                }
            }
            egui::Event::Cut => {
                if let Ok(v) = app.execute("edit.cut", json!({})) {
                    ui.ctx().copy_text(v["text"].as_str().unwrap_or("").to_string());
                }
            }
            egui::Event::Paste(t) => app.run("edit.paste", json!({"text": t})),
            egui::Event::Key { key, pressed: true, modifiers: m, .. } => {
                // SmartType popup keys.
                if let Some(sg) = &sugg {
                    let n = sg.items.len();
                    match key {
                        Key::ArrowDown if n > 0 => {
                            app.canvas.suggest_idx = (app.canvas.suggest_idx + 1) % n;
                            continue;
                        }
                        Key::ArrowUp if n > 0 => {
                            app.canvas.suggest_idx = (app.canvas.suggest_idx + n - 1) % n;
                            continue;
                        }
                        Key::Enter if !m.shift => {
                            let cue = app.session.element() == Element::Character;
                            let idx = app.canvas.suggest_idx.min(n.saturating_sub(1));
                            app.run("smarttype.accept", json!({"index": idx}));
                            if cue {
                                app.run("text.enter", json!({}));
                            }
                            sugg = current_suggest(app);
                            continue;
                        }
                        Key::Escape => {
                            app.session.suggest_dismissed = true;
                            sugg = None;
                            continue;
                        }
                        _ => {}
                    }
                }
                key_command(app, key, m);
                sugg = current_suggest(app);
            }
            _ => {}
        }
    }
}

/// Keys while the element list is open.
fn picker_key(app: &mut ScriptApp, key: Key, m: Modifiers) {
    let n = Element::ALL.len();
    let Some(i) = app.canvas.picker else { return };
    match key {
        Key::ArrowDown => app.canvas.picker = Some((i + 1) % n),
        Key::ArrowUp => app.canvas.picker = Some((i + n - 1) % n),
        Key::Enter | Key::Tab => {
            app.canvas.picker = None;
            if let Some(e) = Element::ALL.get(i) {
                app.run("element.set", json!({"element": e.id()}));
            }
        }
        Key::Escape => app.canvas.picker = None,
        _ => {
            app.canvas.picker = None;
            key_command(app, key, m);
        }
    }
}

/// Whether the caret's paragraph is empty for element flow.
fn caret_para_blank(app: &ScriptApp) -> bool {
    let s = &app.session;
    s.sel.is_caret()
        && s.script.para(s.sel.focus.para).is_some_and(|p| p.text.trim().is_empty() || (p.element == Element::Parenthetical && p.text.trim() == "()"))
}

/// Map a key to a command.
pub fn key_command(app: &mut ScriptApp, key: Key, m: Modifiers) {
    if key == Key::Enter && !m.command && !m.shift && app.enter_picker && caret_para_blank(app) {
        let cur = app.session.element();
        let target = scriptcraft_engine::typing::enter_empty(cur);
        app.canvas.picker = Element::ALL.iter().position(|e| *e == target);
        return;
    }
    let ext = json!({"extend": m.shift});
    let mac = cfg!(target_os = "macos");
    let plain = !m.command && !m.alt;
    let (id, params) = match key {
        Key::Enter if !m.command => ("text.enter", json!({})),
        Key::Tab if m.shift => ("text.shiftTab", json!({})),
        Key::Tab => ("text.tab", json!({})),
        Key::Backspace if m.alt || m.command => {
            let id = if m.command && mac { "caret.home" } else { "caret.wordLeft" };
            if app.session.sel.is_caret() {
                app.run(id, json!({"extend": true}));
            }
            ("text.backspace", json!({}))
        }
        Key::Backspace => ("text.backspace", json!({})),
        Key::Delete => ("text.delete", json!({})),
        Key::ArrowLeft if m.command && mac => ("caret.home", ext),
        Key::ArrowRight if m.command && mac => ("caret.end", ext),
        Key::ArrowLeft if m.alt || m.command => ("caret.wordLeft", ext),
        Key::ArrowRight if m.alt || m.command => ("caret.wordRight", ext),
        Key::ArrowLeft => ("caret.left", ext),
        Key::ArrowRight => ("caret.right", ext),
        Key::ArrowUp if m.command => ("caret.docStart", ext),
        Key::ArrowDown if m.command => ("caret.docEnd", ext),
        Key::ArrowUp => ("caret.up", ext),
        Key::ArrowDown => ("caret.down", ext),
        Key::Home if m.command => ("caret.docStart", ext),
        Key::End if m.command => ("caret.docEnd", ext),
        Key::Home => ("caret.home", ext),
        Key::End => ("caret.end", ext),
        Key::Escape => {
            if app.find.is_some() {
                app.find = None;
            }
            return;
        }
        _ if plain => return,
        _ => {
            crate::chrome::dispatch_shortcut(app, key, m);
            return;
        }
    };
    app.run(id, params);
}

/// Screen rect of a line's columns `[c0, c1)` on page rect `pr`.
fn cols_rect(pr: Rect, z: f32, l: &Line, c0: usize, c1: usize) -> Rect {
    let x0 = pr.left() + (l.x + c0 as f32 * CHAR_W) * z;
    let x1 = pr.left() + (l.x + c1 as f32 * CHAR_W) * z;
    Rect::from_min_max(pos2(x0, pr.top() + l.y * z), pos2(x1, pr.top() + (l.y + LINE_H) * z))
}

/// Selected column range of a body line, if any.
fn selected_cols(l: &Line, a: Pos, b: Pos) -> Option<(usize, usize)> {
    let para = l.para?;
    if para < a.para || para > b.para || a == b {
        return None;
    }
    let from = if para == a.para { a.off.max(l.start) } else { l.start };
    let to = if para == b.para { b.off.min(l.end) } else { l.end };
    let eol = para < b.para && to == l.end;
    if from > to || (from == to && !eol) {
        return None;
    }
    let col = |off: usize| l.text.get(..off.saturating_sub(l.start).min(l.text.len())).map_or(0, |t| t.chars().count());
    Some((col(from), col(to) + usize::from(eol)))
}

/// The paper colour of page `i`: tinted with its revision set's colour when coloured revision
/// pages are on.
fn page_color(app: &ScriptApp, l: &Layout, i: usize, paper: Color32, dark: bool) -> Color32 {
    let r = &app.session.script.settings.revision;
    let Some(set) = l.page_rev.get(i).copied().flatten().filter(|_| r.color_pages).and_then(|lvl| r.set(lvl)) else { return paper };
    let [cr, cg, cb] = set.color;
    let c = Color32::from_rgb(cr, cg, cb);
    if dark { paper.lerp_to_gamma(c, 0.22) } else { c }
}

fn paint_page(ui: &Ui, app: &ScriptApp, l: &Layout, i: usize, pr: Rect, bad: &[(usize, usize, usize)]) {
    let t = app.tokens();
    let z = app.canvas.zoom;
    let p = ui.painter();
    let shadow = egui::epaint::Shadow { offset: [0, 3], blur: 14, spread: 0, color: Color32::from_black_alpha(if t.dark { 120 } else { 46 }) };
    p.add(shadow.as_shape(pr, CornerRadius::same(2)));
    p.rect_filled(pr, 2.0, page_color(app, l, i, t.paper, t.dark));
    p.rect_stroke(pr, 2.0, Stroke::new(0.6, t.paper_edge), StrokeKind::Inside);
    let Some(page) = l.pages.get(i) else { return };
    let (a, b) = app.session.sel.ordered();
    let font = courier(FONT_SIZE * z);
    for line in &page.lines {
        if line.kind == LineKind::Body
            && let Some((c0, c1)) = selected_cols(line, a, b)
        {
            p.rect_filled(cols_rect(pr, z, line, c0, c1), 0.0, t.selection);
        }
        crate::annotations::paint_behind(app, p, line, pr, z);
        crate::annotations::paint_margin(app, p, line, pr, z);
        crate::outline::paint_marker(app, p, line, pr, z);
        let color = match line.kind {
            LineKind::Header | LineKind::Footer | LineKind::More | LineKind::Contd | LineKind::SceneNumber => t.ink_soft,
            _ => t.ink,
        };
        for (col, text, st) in line.segments() {
            let x = line.x + col as f32 * CHAR_W;
            let pos = pr.min + vec2(x * z, line.y * z + 0.5 * z);
            p.text(pos, Align2::LEFT_TOP, text, courier_styled(FONT_SIZE * z, st), color);
            if st.underline {
                let y = pr.top() + (line.baseline() + 1.8) * z;
                let x1 = x + text.chars().count() as f32 * CHAR_W;
                p.line_segment([pos2(pr.left() + x * z, y), pos2(pr.left() + x1 * z, y)], Stroke::new(0.7 * z, color));
            }
        }
        // Spelling: a wavy red line under misspelled words.
        if let Some(para) = line.para {
            for &(_, a, b) in bad.iter().filter(|m| m.0 == para && m.2 > line.start && m.1 < line.end) {
                let col = |off: usize| line.text.get(..off.clamp(line.start, line.end) - line.start).map_or(0, |t| t.chars().count());
                let (x0, x1) = (pr.left() + (line.x + col(a) as f32 * CHAR_W) * z, pr.left() + (line.x + col(b) as f32 * CHAR_W) * z);
                let y = pr.top() + (line.baseline() + 2.6) * z;
                let step = 2.0 * z.max(0.6);
                let mut pts = Vec::new();
                let mut x = x0;
                let mut up = true;
                while x <= x1 {
                    pts.push(pos2(x, if up { y - 0.9 * z } else { y + 0.9 * z }));
                    up = !up;
                    x += step;
                }
                if pts.len() > 1 {
                    p.add(egui::Shape::line(pts, Stroke::new(0.9 * z.max(0.8), Color32::from_rgb(0xe0, 0x3b, 0x3b))));
                }
            }
        }
        if !line.suffix.is_empty() {
            let x = line.x + line.text.chars().count() as f32 * CHAR_W;
            p.text(pr.min + vec2(x * z, line.y * z + 0.5 * z), Align2::LEFT_TOP, &line.suffix, font.clone(), t.ink_soft);
        }
    }
    if page.title_page && !app.session.speed {
        p.text(pr.left_bottom() + vec2(10.0, -8.0), Align2::LEFT_BOTTOM, "Title page", FontId::proportional(10.0), t.ink_soft.gamma_multiply(0.7));
    }
}

/// The page under a screen point (nearest by vertical distance) and the point in page points.
fn page_point(rects: &[Rect], z: f32, at: Pos2) -> Option<(usize, f32, f32)> {
    let (i, r) = rects.iter().enumerate().min_by(|(_, a), (_, b)| {
        let d = |r: &Rect| if r.y_range().contains(at.y) { 0.0 } else { (r.center().y - at.y).abs() };
        d(a).total_cmp(&d(b))
    })?;
    Some((i, (at.x - r.left()) / z, (at.y - r.top()) / z))
}

pub fn show(app: &mut ScriptApp, ui: &mut Ui) {
    let id = canvas_id_for(app.canvas.pane);
    // Keep the page focused unless another widget (find bar, dialogs) has the keyboard.
    let focused_other = ui.memory(|m| m.focused().is_some_and(|f| f != id));
    if !focused_other {
        ui.memory_mut(|m| m.request_focus(id));
    }
    ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
    if ui.memory(|m| m.has_focus(id)) {
        handle_keys(app, ui);
    }
    let now = ui.input(|i| i.time);
    if ui.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Key { pressed: true, .. } | egui::Event::PointerButton { pressed: true, .. }))
    }) {
        app.canvas.last_activity = now;
    }
    // Ctrl/Cmd + wheel or a pinch zooms the pages.
    let zd = ui.input(|i| i.zoom_delta());
    if (zd - 1.0).abs() > 1e-4 && ui.rect_contains_pointer(ui.max_rect()) {
        app.canvas.zoom = (app.canvas.zoom * zd).clamp(0.4, 3.0);
    }
    let layout = app.session.view_layout();
    let z = app.canvas.zoom;
    let pw = layout.page_width() * z;
    let n = layout.pages.len();
    let heights: Vec<f32> = (0..n).map(|i| layout.page_height(i) * z).collect();
    let total_h = heights.iter().map(|h| h + GAP).sum::<f32>() + GAP;
    let scroll_to_caret = std::mem::take(&mut app.canvas.scroll_to_caret);
    egui::ScrollArea::both().id_salt("pages").auto_shrink([false, false]).show(ui, |ui| {
        let avail = ui.available_width().max(pw + 2.0 * GAP);
        let (rect, _) = ui.allocate_exact_size(vec2(avail, total_h), Sense::hover());
        let x0 = rect.left() + ((avail - pw) / 2.0).round();
        let mut y = rect.top() + GAP;
        let rects: Vec<Rect> = heights
            .iter()
            .map(|h| {
                let r = Rect::from_min_size(pos2(x0, y), vec2(pw, *h));
                y += h + GAP;
                r
            })
            .collect();
        let clip = ui.clip_rect();
        let bad = misspellings(app);
        for (i, pr) in rects.iter().enumerate() {
            if pr.intersects(clip) {
                paint_page(ui, app, &layout, i, *pr, &bad);
            }
        }
        // Caret.
        let caret = layout.caret(app.session.sel.focus).and_then(|c| {
            let pr = rects.get(c.page)?;
            Some(Rect::from_min_size(pr.min + vec2(c.x * z, c.y * z), vec2(1.6 * z.max(1.0), c.h * z)))
        });
        app.canvas.caret_screen = caret;
        if let Some(cr) = caret {
            let t = app.tokens();
            // Solid for a moment after activity, then a 1.06 s blink.
            let since = now - app.canvas.last_activity;
            let on = !app.canvas.blink || since < 0.5 || (since - 0.5) % 1.06 < 0.53;
            if on {
                ui.painter().rect_filled(Rect::from_min_size(cr.min - vec2(0.5, 0.0), vec2(1.8, cr.height())), 0.0, t.caret);
            }
            if app.canvas.blink {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
            }
            if scroll_to_caret {
                // A jump off screen (scene, find, page) centres the caret; typing scrolls minimally.
                let far = !clip.expand(40.0).contains_rect(cr);
                ui.scroll_to_rect(cr.expand2(vec2(40.0, 60.0)), far.then_some(egui::Align::Center));
            }
        }
        // Pointer.
        let resp = ui.interact(rect, id, Sense::click_and_drag());
        // Right-click: put the caret there and remember a misspelled word under it.
        if resp.secondary_clicked()
            && let Some(at) = resp.interact_pointer_pos()
            && let Some((pi, x, y)) = page_point(&rects, z, at)
            && let Some(pos) = layout.hit(pi, x, y)
        {
            let inside = app.session.sel.ordered();
            if !(inside.0 <= pos && pos <= inside.1 && inside.0 != inside.1) {
                app.run("caret.set", json!({"para": pos.para, "off": pos.off}));
            }
            app.canvas.spell_target = bad.iter().find(|m| m.0 == pos.para && m.1 <= pos.off && pos.off <= m.2).map(|&(pa, a, b)| {
                let w = app.session.script.para(pa).and_then(|p| p.text.get(a..b)).unwrap_or("").to_string();
                (pa, a, b, scriptcraft_spell::english().suggest(&w, 6))
            });
            app.canvas.scroll_to_caret = false;
        }
        resp.context_menu(|ui| context_menu(app, ui));
        if let Some(at) = resp.interact_pointer_pos()
            && let Some((pi, x, y)) = page_point(&rects, z, at)
            && let Some(pos) = layout.hit(pi, x, y)
        {
            let shift = ui.input(|i| i.modifiers.shift);
            if resp.double_clicked() {
                let text = app.session.script.para(pos.para).map_or(String::new(), |p| p.text.clone());
                let b = word_right(&text, pos.off);
                let a = word_left(&text, b);
                app.run("select.range", json!({"anchor": {"para": pos.para, "off": a}, "focus": {"para": pos.para, "off": b}}));
            } else if resp.triple_clicked() {
                app.run("select.para", json!({"para": pos.para}));
            } else if resp.drag_started() || resp.clicked() {
                if !resp.double_clicked() {
                    app.run("caret.set", json!({"para": pos.para, "off": pos.off, "extend": shift}));
                }
                resp.request_focus();
            } else if resp.dragged() {
                app.run("caret.set", json!({"para": pos.para, "off": pos.off, "extend": true}));
            }
            app.canvas.scroll_to_caret = false;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        app.canvas.page_rects = rects;
        app.canvas.canvas_rect = Some(clip);
    });
    // With two page panes, only the one with the keyboard shows the pop-ups.
    if app.canvas.pane == 0 || ui.memory(|m| m.has_focus(id)) {
        smarttype_popup(app, ui);
        element_picker(app, ui);
    }
}

fn element_picker(app: &mut ScriptApp, ui: &mut Ui) {
    let (Some(sel), Some(cr)) = (app.canvas.picker, app.canvas.caret_screen) else { return };
    if !caret_para_blank(app) {
        app.canvas.picker = None;
        return;
    }
    let t = app.tokens();
    let mut chosen = None;
    egui::Area::new(Id::new("element-picker")).order(egui::Order::Foreground).fixed_pos(pos2(cr.left(), cr.bottom() + 3.0)).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_min_width(200.0);
            ui.label(egui::RichText::new("Change element").size(10.0).color(t.text_soft));
            for (i, e) in Element::ALL.iter().enumerate() {
                let label = format!("{}  {}", i + 1, e.name());
                if ui.add(egui::Button::selectable(i == sel, label).min_size(vec2(190.0, 0.0))).clicked() {
                    chosen = Some(*e);
                }
            }
            ui.label(egui::RichText::new("Arrows or 1–8 · Return chooses · Esc cancels").size(10.0).color(t.text_soft));
        });
    });
    if let Some(e) = chosen {
        app.canvas.picker = None;
        app.run("element.set", json!({"element": e.id()}));
    }
}

fn context_menu(app: &mut ScriptApp, ui: &mut Ui) {
    if let Some((para, a, b, sugg)) = app.canvas.spell_target.clone() {
        if sugg.is_empty() {
            ui.label(egui::RichText::new("No suggestions").italics());
        }
        for w in sugg {
            if ui.button(egui::RichText::new(&w).strong()).clicked() {
                app.run("select.range", json!({"anchor": {"para": para, "off": a}, "focus": {"para": para, "off": b}}));
                app.run("text.insert", json!({"text": w}));
                ui.close();
            }
        }
        ui.separator();
        let word = app.session.script.para(para).and_then(|p| p.text.get(a..b)).unwrap_or("").to_string();
        if ui.button("Ignore Spelling").clicked() {
            app.run("spell.ignore", json!({"word": word}));
            ui.close();
        }
        if ui.button("Add to Dictionary").clicked() {
            app.run("spell.addWord", json!({"word": word}));
            ui.close();
        }
        ui.separator();
    }
    crate::annotations::context_items(app, ui);
    for (label, id) in [("Cut", "edit.cut"), ("Copy", "edit.copy")] {
        if ui.add_enabled(!app.session.sel.is_caret(), egui::Button::new(label)).clicked() {
            if let Ok(v) = app.execute(id, json!({})) {
                ui.ctx().copy_text(v["text"].as_str().unwrap_or("").to_string());
            }
            ui.close();
        }
    }
    if ui.button("Select All").clicked() {
        app.run("select.all", json!({}));
        ui.close();
    }
}

fn smarttype_popup(app: &mut ScriptApp, ui: &mut Ui) {
    let sugg = current_suggest(app);
    app.canvas.suggest_shown = false;
    let (Some(sg), Some(cr)) = (sugg, app.canvas.caret_screen) else {
        app.canvas.suggest_items.clear();
        return;
    };
    if sg.items != app.canvas.suggest_items {
        app.canvas.suggest_idx = 0;
        app.canvas.suggest_items = sg.items.clone();
    }
    let t = app.tokens();
    let z = app.canvas.zoom;
    // Align the list with the start of the text being completed.
    let back = app.session.script.para(sg.para).and_then(|p| p.text.get(sg.from..)).map_or(0, |s| s.chars().count());
    let at = pos2(cr.left() - back as f32 * CHAR_W * z, cr.bottom() + 3.0);
    app.canvas.suggest_shown = true;
    let mut clicked = None;
    egui::Area::new(Id::new("smarttype")).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_min_width(180.0);
            ui.label(egui::RichText::new("SmartType").size(10.0).color(t.text_soft));
            for (i, item) in sg.items.iter().enumerate() {
                let sel = i == app.canvas.suggest_idx;
                let text = egui::RichText::new(item.trim_end()).font(courier(13.0));
                if ui.add(egui::Button::selectable(sel, text).min_size(vec2(170.0, 0.0))).clicked() {
                    clicked = Some(i);
                }
            }
            ui.label(egui::RichText::new("↩ accept · esc dismiss").size(10.0).color(t.text_soft));
        });
    });
    if let Some(i) = clicked {
        let cue = app.session.element() == Element::Character;
        app.run("smarttype.accept", json!({"index": i}));
        if cue {
            app.run("text.enter", json!({}));
        }
    }
}
