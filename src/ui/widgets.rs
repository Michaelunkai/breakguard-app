//! shared widgets. OWNER: UI/UX Designer. See DESIGN.md.
use super::theme::*;
use crate::sys::MonitorInfo;
use eframe::egui::{
    self, epaint::Shadow, pos2, vec2, Align2, Color32, Id, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Ui, WidgetInfo, WidgetType,
};

/// Semantic colour kinds for pills and toasts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PillKind {
    Ok,
    Warn,
    Danger,
    Info,
    Neutral,
}

impl PillKind {
    pub fn color(self) -> Color32 {
        match self {
            PillKind::Ok => if is_light() { LIGHT_OK } else { OK },
            PillKind::Warn => if is_light() { LIGHT_WARN } else { WARN },
            PillKind::Danger => if is_light() { LIGHT_DANGER } else { DANGER },
            PillKind::Info => link_accent(is_light()),
            PillKind::Neutral => pal().text_dim,
        }
    }
}

/// Visual state of a monitor tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileState {
    Idle,
    Selected,
    Off,
    Blackout,
    Unsupported,
}

fn focus_ring(ui: &Ui, rect: Rect, radius: u8) {
    let (r, g, b) = if pal().bg.r() > 128 { (146, 64, 14) } else { (255, 176, 0) };
    ui.painter().rect_stroke(rect.expand(2.0), rad(radius + 2), Stroke::new(2.0_f32, Color32::from_rgb(r, g, b)), StrokeKind::Outside);
}

// ---------------------------------------------------------------- card

/// Titled surface panel.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    super::parts::card_elev::elevated_frame(1)
        .inner_margin(egui::Margin::same(SP_M as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(RichText::new(title.to_uppercase()).size(11.5).color(pal().text_dim).strong());
                ui.add_space(SP_XS);
            }
            add(ui)
        })
        .inner
}

/// Lane row frame: surface_lo fill, 1px border, accent border when `selected`.
pub fn lane_frame(selected: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(pal().surface_lo)
        .stroke(Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { accent() } else { pal().border }))
        .corner_radius(rad(R_M))
        .inner_margin(egui::Margin { left: 16, right: 16, top: 10, bottom: 8 })
}

/// Status banner: tinted fill, 1px tinted stroke, small radius. Used for warnings and notices.
pub fn banner<R>(ui: &mut Ui, color: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(mix(pal().surface, color, 0.14))
        .stroke(Stroke::new(1.0_f32, color))
        .corner_radius(rad(R_S))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn section_header(ui: &mut Ui, text: &str) {
    ui.add_space(SP_S);
    ui.label(RichText::new(text).heading().color(pal().text));
    ui.add_space(SP_XS);
}

// ---------------------------------------------------------------- buttons

struct BtnStyle {
    fill: Color32,
    fill_hi: Color32,
    fill_active: Color32,
    text: Color32,
    stroke: Color32,
}

fn button(ui: &mut Ui, text: &str, st: BtnStyle) -> Response {
    let enabled = ui.is_enabled();
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(14.5), st.text);
    let size = vec2((galley.size().x + 2.0 * SP_L).max(88.0), 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, ANIM);
    let p_ = pal();
    let (fill, stroke, txt) = if !enabled {
        (mix(p_.surface, st.fill, 0.18), mix(p_.surface, st.stroke, 0.3), p_.text_dim)
    } else if resp.is_pointer_button_down_on() {
        (st.fill_active, st.stroke, st.text)
    } else {
        (mix(st.fill, st.fill_hi, t), st.stroke, st.text)
    };
    let p = ui.painter();
    p.rect_filled(rect, rad(R_S), fill);
    p.rect_stroke(rect, rad(R_S), Stroke::new(1.0_f32, stroke), StrokeKind::Inside);
    let pos = rect.center() - galley.size() / 2.0;
    p.galley(pos, galley, txt);
    if resp.has_focus() && enabled {
        focus_ring(ui, rect, R_S);
    }
    if enabled {
        resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text, BtnStyle { fill: accent(), fill_hi: accent_hi(), fill_active: accent_lo(), text: ON_FILL, stroke: accent() })
}

pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text, BtnStyle { fill: DANGER, fill_hi: DANGER_HI, fill_active: mix(DANGER, pal().bg, 0.3), text: ON_FILL, stroke: DANGER })
}

pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    let p = pal();
    button(ui, text, BtnStyle { fill: p.surface, fill_hi: p.surface_hi, fill_active: mix(p.surface_hi, accent(), 0.2), text: p.text, stroke: p.border_strong })
}

/// Compact square icon button with a tooltip. Used for toolbar and per-lane actions.
pub fn icon_button(ui: &mut Ui, icon: super::icons::Icon, tooltip: &str) -> Response {
    let enabled = ui.is_enabled();
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, tooltip));
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, ANIM);
    let fill = if !enabled { pal().surface_lo } else if resp.is_pointer_button_down_on() { mix(pal().surface_hi, accent(), 0.3) } else { mix(pal().surface, pal().surface_hi, t) };
    let col = if enabled { mix(pal().text_dim, pal().text, t) } else { pal().border_strong };
    let p = ui.painter();
    p.rect_filled(rect, rad(R_S), fill);
    super::icons::paint(p, icon, rect.center(), 16.0, col);
    if resp.has_focus() && enabled {
        focus_ring(ui, rect, R_S);
    }
    resp.on_hover_text(tooltip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- checkbox

/// Checkbox: empty dark rounded square when off, accent fill + check mark when on.
pub fn checkbox(ui: &mut Ui, value: &mut bool, text: impl Into<egui::WidgetText>) -> Response {
    ui.scope(|ui| {
        let on = *value;
        let w = &mut ui.visuals_mut().widgets;
        let fill = if on { mix(pal().surface_hi, accent(), 0.6) } else { pal().surface };
        for s in [&mut w.inactive, &mut w.hovered, &mut w.active] {
            s.bg_fill = if on { mix(fill, accent_hi(), 0.0) } else { fill };
            s.weak_bg_fill = s.bg_fill;
            s.corner_radius = rad(5);
        }
        w.inactive.bg_stroke = Stroke::new(1.5_f32, if on { accent() } else { pal().border_strong });
        ui.checkbox(value, text)
    })
    .inner
}

/// Radio option: dark ring when off, accent dot when on (same fill logic as `checkbox`).
pub fn radio<T: PartialEq>(ui: &mut Ui, value: &mut T, option: T, text: &str) -> Response {
    ui.scope(|ui| {
        let on = *value == option;
        let w = &mut ui.visuals_mut().widgets;
        for s in [&mut w.inactive, &mut w.hovered, &mut w.active] {
            s.bg_fill = if on { mix(pal().surface_hi, accent(), 0.6) } else { pal().surface };
            s.weak_bg_fill = s.bg_fill;
        }
        w.inactive.bg_stroke = Stroke::new(1.5_f32, if on { accent() } else { pal().border_strong });
        ui.radio_value(value, option, text)
    })
    .inner
}

// ---------------------------------------------------------------- nav item

/// Sidebar entry: active = tinted fill + accent bar on the left, hover = lighter fill.
#[allow(dead_code)] // plain variant; sidebar uses nav_item_icon
pub fn nav_item(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, text));
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), ANIM);
    let p = ui.painter();
    let inner = rect.shrink2(vec2(SP_S, 2.0));
    let fill = if selected { mix(pal().surface, accent(), 0.16) } else { mix(pal().bg, pal().surface_hi, t) };
    p.rect_filled(inner, rad(R_S), fill);
    if selected {
        let bar = Rect::from_min_size(pos2(inner.left(), inner.top() + 8.0), vec2(3.0, inner.height() - 16.0));
        p.rect_filled(bar, rad(2), accent());
    }
    let col = if selected { pal().text } else { mix(pal().text_dim, pal().text, t) };
    let font = egui::FontId::proportional(14.5);
    p.text(pos2(inner.left() + SP_M + 4.0, inner.center().y), Align2::LEFT_CENTER, text, font, col);
    if resp.has_focus() {
        focus_ring(ui, inner, R_S);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- chip

/// Toggle chip. Selected = teal outline + tinted fill + check mark.
pub fn chip(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let enabled = ui.is_enabled();
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(13.5), pal().text);
    let pad = if selected { 34.0 } else { 24.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + pad, 30.0), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, selected, text));
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, ANIM);
    let base = if selected { mix(pal().surface, accent(), 0.2) } else { pal().surface_hi };
    let fill = mix(base, accent(), t * 0.12);
    let (stroke, txt) = if !enabled {
        (pal().border, pal().text_dim)
    } else if selected {
        (accent(), pal().text)
    } else {
        (pal().border_strong, pal().text)
    };
    let p = ui.painter();
    p.rect_filled(rect, rad(15), fill);
    p.rect_stroke(rect, rad(15), Stroke::new(1.0_f32, stroke), StrokeKind::Inside);
    let mut x = rect.left() + 12.0;
    if selected {
        let c = pos2(x + 4.0, rect.center().y);
        p.add(Shape::line(vec![c + vec2(-3.5, 0.0), c + vec2(-1.0, 3.0), c + vec2(4.0, -3.5)], Stroke::new(2.0_f32, if enabled { accent_hi() } else { pal().text_dim })));
        x += 14.0;
    }
    p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, txt);
    if resp.has_focus() && enabled {
        focus_ring(ui, rect, 15);
    }
    resp
}

// ---------------------------------------------------------------- pill

/// Small status label with dot. Not interactive.
pub fn status_pill(ui: &mut Ui, text: &str, kind: PillKind) -> Response {
    let c = kind.color();
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.5), c);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 30.0, 24.0), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    let p = ui.painter();
    p.rect_filled(rect, rad(12), mix(pal().surface, c, 0.14));
    p.rect_stroke(rect, rad(12), Stroke::new(1.0_f32, mix(pal().surface, c, 0.45)), StrokeKind::Inside);
    p.circle_filled(pos2(rect.left() + 11.0, rect.center().y), 3.5, c);
    p.galley(pos2(rect.left() + 21.0, rect.center().y - galley.size().y / 2.0), galley, c);
    resp
}

// ---------------------------------------------------------------- ring

/// Hero countdown ring. `frac` = remaining 0..1. `color` = arc colour (accent() work, WARN break...).
pub fn countdown_ring(ui: &mut Ui, frac: f32, big: &str, small: &str, color: Color32) -> Response {
    let avail = ui.available_width().min(ui.available_height().max(180.0));
    let size = avail.clamp(180.0, 260.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, format!("{big} {small}")));
    let frac = ui.ctx().animate_value_with_time(resp.id, frac.clamp(0.0, 1.0), ANIM);
    let c = rect.center();
    let w = 12.0;
    let r = size / 2.0 - w;
    let p = ui.painter();
    p.circle_filled(c, r + w / 2.0 + 2.0, pal().surface);
    if frac > 0.002 {
        glow_circle(p, c, r + w / 2.0 + 2.0, color, 4);
    }
    p.circle_stroke(c, r, Stroke::new(w, mix(pal().surface_hi, pal().border_strong, 0.35)));
    if frac > 0.002 {
        let start = -std::f32::consts::FRAC_PI_2;
        let n = ((frac * 120.0) as usize).max(2);
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = start + frac * std::f32::consts::TAU * i as f32 / n as f32;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect();
        p.add(Shape::line(pts.clone(), Stroke::new(w, color)));
        for pt in [pts[0], pts[n]] {
            p.circle_filled(pt, w / 2.0, color);
        }
    }
    p.text(c - vec2(0.0, 6.0), Align2::CENTER_CENTER, big, mono(size * 0.2), pal().text);
    p.text(c + vec2(0.0, size * 0.13), Align2::CENTER_CENTER, small, egui::FontId::proportional(13.5), pal().text_dim);
    resp
}

// ---------------------------------------------------------------- toggle / segmented / stat / slider / hero

/// iOS-style switch with a label to the right.
pub fn toggle(ui: &mut Ui, value: &mut bool, text: &str) -> Response {
    let enabled = ui.is_enabled();
    let txt_col = if enabled { pal().text } else { pal().text_dim };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(14.5), txt_col);
    let (sw, sh) = (40.0_f32, 22.0_f32);
    let size = vec2(sw + SP_S + galley.size().x, sh.max(galley.size().y).max(26.0));
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() && enabled {
        *value = !*value;
        resp.mark_changed();
    }
    let on = *value;
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, text));
    let t = ui.ctx().animate_bool_with_time(resp.id, on, ANIM);
    let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() && enabled, ANIM);
    let track = Rect::from_min_size(pos2(rect.left(), rect.center().y - sh / 2.0), vec2(sw, sh));
    let off_fill = mix(pal().surface_lo, pal().border_strong, 0.35 + 0.15 * hov);
    let fill = mix(off_fill, accent(), t);
    let p = ui.painter();
    p.rect_filled(track, rad(11), fill);
    p.rect_stroke(track, rad(11), Stroke::new(1.5_f32, mix(pal().border_strong, accent_hi(), t)), StrokeKind::Inside);
    let kx = track.left() + 11.0 + (sw - 22.0) * t;
    p.circle_filled(pos2(kx, track.center().y), 8.0, mix(pal().text_dim, ON_FILL, t));
    if on {
        let c = pos2(kx, track.center().y);
        p.add(Shape::line(vec![c + vec2(-3.0, 0.0), c + vec2(-0.8, 2.4), c + vec2(3.2, -2.4)], Stroke::new(1.8_f32, accent())));
    }
    p.galley(pos2(rect.left() + sw + SP_S, rect.center().y - galley.size().y / 2.0), galley, txt_col);
    if resp.has_focus() && enabled {
        focus_ring(ui, track, 11);
    }
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Segmented control: one row of mutually exclusive options. Returns true when the selection changed.
pub fn segmented(ui: &mut Ui, value: &mut usize, options: &[&str]) -> bool {
    let mut changed = false;
    let font = egui::FontId::proportional(13.5);
    let galleys: Vec<_> = options.iter().map(|o| ui.painter().layout_no_wrap((*o).to_owned(), font.clone(), pal().text)).collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 28.0).collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(vec2(total + 6.0, 34.0), Sense::hover());
    ui.painter().rect_filled(rect, rad(R_S + 2), pal().surface_lo);
    ui.painter().rect_stroke(rect, rad(R_S + 2), Stroke::new(1.0_f32, pal().border_strong), StrokeKind::Inside);
    let mut x = rect.left() + 3.0;
    for (i, g) in galleys.into_iter().enumerate() {
        let seg = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(widths[i], 28.0));
        let resp = ui.interact(seg, ui.id().with(("seg", i)), Sense::click());
        resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, *value == i, options[i]));
        let sel = *value == i;
        let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), ANIM);
        let p = ui.painter();
        if sel {
            p.rect_filled(seg, rad(R_S), mix(pal().surface_hi, accent(), 0.30));
            p.rect_stroke(seg, rad(R_S), Stroke::new(1.0_f32, accent()), StrokeKind::Inside);
        } else if t > 0.0 {
            p.rect_filled(seg, rad(R_S), mix(pal().surface_lo, pal().surface_hi, t));
        }
        p.galley(pos2(seg.center().x - g.size().x / 2.0, seg.center().y - g.size().y / 2.0), g, if sel { pal().text } else { mix(pal().text_dim, pal().text, t) });
        if resp.has_focus() {
            focus_ring(ui, seg, R_S);
        }
        if resp.clicked() && !sel {
            *value = i;
            changed = true;
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        x += widths[i];
    }
    changed
}

/// Number tile: icon, big value, small caption.
pub fn stat_tile(ui: &mut Ui, icon: super::icons::Icon, value: &str, label: &str, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(132.0, 84.0), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, format!("{label} {value}")));
    let p = ui.painter();
    p.rect_filled(rect, rad(R_M), pal().surface_hi);
    p.rect_stroke(rect, rad(R_M), Stroke::new(1.0_f32, pal().border), StrokeKind::Inside);
    let c = pos2(rect.left() + 24.0, rect.top() + 26.0);
    p.circle_filled(c, 14.0, mix(pal().surface_hi, color, 0.18));
    super::icons::paint(p, icon, c, 18.0, color);
    p.text(pos2(rect.left() + 46.0, rect.top() + 26.0), Align2::LEFT_CENTER, value, num(22.0), pal().text);
    p.text(pos2(rect.left() + 14.0, rect.bottom() - 16.0), Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), pal().text_dim);
    resp
}

/// Label on the left, slider on the right, optional help line below (also the tooltip).
pub fn slider_row(ui: &mut Ui, label: &str, value: &mut u32, range: std::ops::RangeInclusive<u32>, suffix: &str, help: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 24.0], egui::Label::new(label));
        let r = ui.add(egui::Slider::new(value, range).suffix(suffix));
        changed = r.changed();
        if !help.is_empty() {
            r.on_hover_text(help);
        }
    });
    if !help.is_empty() {
        ui.label(RichText::new(help).size(12.0).color(pal().text_dim));
    }
    changed
}

/// Hero surface: tinted panel with an accent glow line on top.
pub fn hero_card<R>(ui: &mut Ui, tint: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let outer = super::parts::card_elev::elevated_frame(2)
        .fill(mix(pal().surface, tint, 0.07))
        .stroke(Stroke::new(1.0_f32, mix(pal().border, tint, 0.5)))
        .corner_radius(rad(R_L))
        .inner_margin(egui::Margin::same(SP_L as i8))
        .shadow(Shadow { offset: [0, 8], blur: 24, spread: 0, color: with_alpha(tint, 26) })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
    let r = outer.response.rect;
    ui.painter().line_segment([pos2(r.left() + 24.0, r.top() + 1.0), pos2(r.right() - 24.0, r.top() + 1.0)], Stroke::new(2.0_f32, with_alpha(tint, 150)));
    outer.inner
}

/// Toggle with a plain-language help line below (also shown as tooltip). True when changed.
pub fn setting_toggle(ui: &mut Ui, value: &mut bool, label: &str, help: &str) -> bool {
    let r = toggle(ui, value, label).on_hover_text(help);
    if !help.is_empty() {
        ui.label(RichText::new(help).size(12.0).color(pal().text_dim));
    }
    ui.add_space(SP_XS);
    r.changed()
}

/// Hour : minute picker (two drag boxes, zero padded). True when changed.
pub fn time_picker(ui: &mut Ui, label: &str, hm: &mut (u8, u8)) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 24.0], egui::Label::new(label));
        let (mut h, mut m) = (hm.0 as u32, hm.1 as u32);
        let fmt = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{:02}", v as u32);
        changed |= ui.add(egui::DragValue::new(&mut h).range(0..=23).custom_formatter(fmt)).on_hover_text("Hour (0-23). Drag or type.").changed();
        ui.label(RichText::new(":").strong());
        changed |= ui.add(egui::DragValue::new(&mut m).range(0..=59).custom_formatter(fmt)).on_hover_text("Minute (0-59). Drag or type.").changed();
        *hm = (h as u8, m as u8);
    });
    changed
}

/// Row of accent colour swatches; returns the newly picked index.
pub fn swatches(ui: &mut Ui, current: u8) -> Option<u8> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for (i, (name, col)) in SWATCHES.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
            resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, current as usize == i, *name));
            let p = ui.painter();
            p.circle_filled(rect.center(), 12.0, *col);
            if current as usize == i {
                p.circle_stroke(rect.center(), 16.0, Stroke::new(2.0_f32, pal().text));
                p.add(Shape::line(vec![rect.center() + vec2(-4.5, 0.0), rect.center() + vec2(-1.2, 3.5), rect.center() + vec2(4.5, -3.5)], Stroke::new(2.2_f32, ON_FILL)));
            }
            if resp.on_hover_text(*name).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(i as u8);
            }
        }
    });
    picked
}

/// Slim progress bar for goals and sessions.
pub fn progress_bar(ui: &mut Ui, frac: f32, color: Color32, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, format!("{:.0} percent", frac * 100.0)));
    let p = ui.painter();
    let r = rad((height / 2.0) as u8);
    p.rect_filled(rect, r, pal().surface_lo);
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        p.rect_filled(Rect::from_min_size(rect.min, vec2((rect.width() * f).max(height), height)), r, color);
    }
    resp
}

/// Pulsing status dot (running sessions, live indicators).
pub fn pulse_dot(ui: &mut Ui, color: Color32, pulsing: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let c = rect.center();
    let t = (ui.input(|i| i.time) as f32 * 2.0).sin() * 0.5 + 0.5;
    let p = ui.painter();
    if pulsing {
        p.circle_filled(c, 4.0 + 3.5 * t, with_alpha(color, (70.0 * (1.0 - t)) as u8 + 10));
    }
    p.circle_filled(c, 4.0, color);
    if pulsing {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
    }
    resp
}

// ---------------------------------------------------------------- monitor tile

/// Selectable monitor card with a drawn screen icon.
pub fn monitor_tile(ui: &mut Ui, m: &MonitorInfo, selected: bool, state: TileState) -> Response {
    let enabled = ui.is_enabled() && state != TileState::Unsupported;
    let (rect, resp) = ui.allocate_exact_size(vec2(176.0, 124.0), if enabled { Sense::click() } else { Sense::hover() });
    let label = format!("{} {}x{}", m.name, m.w, m.h);
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, selected, &label));
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && enabled, ANIM);
    let sel = selected || state == TileState::Selected;
    let fill = mix(if sel { mix(pal().surface, accent(), 0.12) } else { pal().surface }, pal().surface_hi, t);
    let border = if sel { accent() } else if state == TileState::Unsupported { pal().border } else { pal().border_strong };
    let p = ui.painter();
    p.rect_filled(rect, rad(R_M), fill);
    p.rect_stroke(rect, rad(R_M), Stroke::new(if sel { 2.0_f32 } else { 1.0 }, border), StrokeKind::Inside);

    // screen icon
    let (screen_col, screen_fill) = match state {
        TileState::Off => (pal().text_dim, Color32::from_rgb(4, 6, 10)),
        TileState::Blackout => (WARN, Color32::from_rgb(4, 6, 10)),
        TileState::Unsupported => (pal().border, pal().bg),
        _ => (if sel { accent() } else { pal().text_dim }, mix(pal().surface_hi, accent(), 0.15)),
    };
    let aspect = if m.h > 0 { (m.w as f32 / m.h as f32).clamp(1.0, 2.4) } else { 1.78 };
    let ih = 44.0;
    let iw = (ih * aspect).min(110.0);
    let icon = Rect::from_center_size(pos2(rect.center().x, rect.top() + 38.0), vec2(iw, ih));
    p.rect_filled(icon, rad(4), screen_fill);
    p.rect_stroke(icon, rad(4), Stroke::new(2.0_f32, screen_col), StrokeKind::Inside);
    p.line_segment([pos2(icon.center().x, icon.bottom() + 1.0), pos2(icon.center().x, icon.bottom() + 6.0)], Stroke::new(2.0_f32, screen_col));
    p.line_segment([pos2(icon.center().x - 9.0, icon.bottom() + 6.5), pos2(icon.center().x + 9.0, icon.bottom() + 6.5)], Stroke::new(2.0_f32, screen_col));
    match state {
        TileState::Off => {
            let c = icon.center();
            p.circle_stroke(c, 7.0, Stroke::new(1.8_f32, pal().text_dim));
            p.line_segment([c + vec2(0.0, -9.0), c + vec2(0.0, -2.0)], Stroke::new(1.8_f32, pal().text_dim));
        }
        TileState::Blackout => {
            p.text(icon.center(), Align2::CENTER_CENTER, "z z", egui::FontId::monospace(14.0), WARN);
        }
        TileState::Unsupported => {
            let c = icon.center();
            p.line_segment([c + vec2(-7.0, -7.0), c + vec2(7.0, 7.0)], Stroke::new(2.0_f32, pal().text_dim));
            p.line_segment([c + vec2(-7.0, 7.0), c + vec2(7.0, -7.0)], Stroke::new(2.0_f32, pal().text_dim));
        }
        _ => {}
    }

    let title = if m.primary { format!("{} (main)", short_name(&m.name)) } else { short_name(&m.name) };
    p.text(pos2(rect.center().x, rect.bottom() - 36.0), Align2::CENTER_CENTER, title, egui::FontId::proportional(13.5), if enabled { pal().text } else { pal().text_dim });
    let status = match state {
        TileState::Off => ("Off".to_string(), pal().text_dim),
        TileState::Blackout => ("Blackout".to_string(), WARN),
        TileState::Unsupported => ("No hardware off".to_string(), pal().text_dim),
        _ => (format!("{}x{}", m.w, m.h), pal().text_dim),
    };
    p.text(pos2(rect.center().x, rect.bottom() - 16.0), Align2::CENTER_CENTER, status.0, egui::FontId::proportional(12.0), status.1);
    if sel {
        // check badge, top-right
        let c = pos2(rect.right() - 14.0, rect.top() + 14.0);
        p.circle_filled(c, 8.0, accent());
        p.add(Shape::line(vec![c + vec2(-3.5, 0.0), c + vec2(-1.0, 3.0), c + vec2(3.5, -3.0)], Stroke::new(2.0_f32, ON_FILL)));
    }
    if resp.has_focus() && enabled {
        focus_ring(ui, rect, R_M);
    }
    resp
}

/// "\\.\DISPLAY5" -> "Display 5"; mock names too.
fn short_name(dev: &str) -> String {
    let t = dev.trim_start_matches('\\').trim_start_matches('.').trim_start_matches('\\');
    let up = t.to_uppercase();
    if let Some(n) = up.strip_prefix("DISPLAYMOCK") {
        format!("Mock {n}")
    } else if let Some(n) = up.strip_prefix("DISPLAY") {
        format!("Display {n}")
    } else {
        t.to_string()
    }
}

// ---------------------------------------------------------------- toast

/// Bottom-centre message. Call every frame while it should be visible; the caller owns the lifetime.
pub fn toast(ctx: &egui::Context, text: &str, kind: PillKind) {
    let c = kind.color();
    egui::Area::new(Id::new("bg_toast"))
        .order(egui::Order::Tooltip)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -SP_L))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(pal().surface_hi)
                .stroke(Stroke::new(1.5_f32, c))
                .corner_radius(rad(R_M))
                .inner_margin(egui::Margin::symmetric(SP_M as i8, 10))
                .shadow(ctx.style().visuals.popup_shadow)
                .show(ui, |ui| {
                    ui.label(RichText::new(text).color(pal().text));
                });
        });
}

// ---------------------------------------------------------------- gallery

/// All 12 creatures at three sizes (one row each) plus the four poses of every creature.
fn creature_sheet(ui: &mut Ui) {
    use super::creatures::{self, Pose};
    let t = ui.input(|i| i.time);
    card(ui, "Creatures", |ui| {
        ui.columns(2, |cols| {
            for k in 0..creatures::COUNT {
                cols[k / 6].horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 1.0;
                    ui.add_sized([62.0, 20.0], egui::Label::new(RichText::new(creatures::NAMES[k]).color(name_colour(creatures::ACCENTS[k]))));
                    for sz in [24.0f32, 40.0, 56.0] {
                        let (r, _) = ui.allocate_exact_size(vec2(sz + 4.0, 60.0), Sense::hover());
                        creatures::draw(ui.painter(), k, r.center(), sz, Pose::Idle, t);
                    }
                    for pose in [Pose::Working, Pose::Sleeping, Pose::Done] {
                        let (r, _) = ui.allocate_exact_size(vec2(38.0, 60.0), Sense::hover());
                        creatures::draw(ui.painter(), k, r.center(), 32.0, pose, t);
                    }
                });
            }
        });
    });
}

/// Creature name colour readable on the current theme: on a light card the accent is pulled toward the text colour.
fn name_colour(accent: Color32) -> Color32 {
    if pal().bg.r() > 128 {
        mix(accent, pal().text, 0.55)
    } else {
        accent
    }
}

/// Every widget in every state. Hidden debug page (Ctrl+Shift+D).
pub fn gallery(ui: &mut Ui) {
    section_header(ui, "Design gallery");
    creature_sheet(ui);
    ui.add_space(SP_S);
    card(ui, "Buttons", |ui| {
        ui.horizontal_wrapped(|ui| {
            primary_button(ui, "Primary");
            danger_button(ui, "Danger");
            ghost_button(ui, "Ghost");
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(false, |ui| {
                primary_button(ui, "Primary off");
                danger_button(ui, "Danger off");
                ghost_button(ui, "Ghost off");
            });
        });
        ui.label(RichText::new("Tab to focus: teal ring. Hover/press animate.").color(pal().text_dim).size(12.0));
    });
    ui.add_space(SP_S);
    card(ui, "Toggle, segmented, slider, stat tiles", |ui| {
        let (mut a, mut b, mut c, mut v) = (true, false, 1usize, 25u32);
        toggle(ui, &mut a, "Toggle on");
        toggle(ui, &mut b, "Toggle off");
        segmented(ui, &mut c, &["Blackout", "Dim", "Notify"]);
        slider_row(ui, "Work for", &mut v, 5..=90, " min", "How long you work before a break");
        ui.horizontal_wrapped(|ui| {
            use super::icons::Icon;
            stat_tile(ui, Icon::Cup, "6", "Breaks today", accent());
            stat_tile(ui, Icon::Flame, "4 d", "Streak", WARN);
            stat_tile(ui, Icon::Target, "75%", "Goal", OK);
        });
        progress_bar(ui, 0.6, accent(), 8.0);
    });
    ui.add_space(SP_S);
    hero_card(ui, accent(), |ui| {
        ui.label(RichText::new("Hero card").heading());
    });
    ui.add_space(SP_S);
    card(ui, "Chips and pills", |ui| {
        ui.horizontal_wrapped(|ui| {
            chip(ui, "Idle chip", false);
            chip(ui, "Selected chip", true);
            ui.add_enabled_ui(false, |ui| chip(ui, "Disabled", false));
        });
        ui.horizontal_wrapped(|ui| {
            status_pill(ui, "Working", PillKind::Info);
            status_pill(ui, "Healthy", PillKind::Ok);
            status_pill(ui, "Break soon", PillKind::Warn);
            status_pill(ui, "Screen off", PillKind::Danger);
            status_pill(ui, "Idle", PillKind::Neutral);
        });
    });
    ui.add_space(SP_S);
    card(ui, "Countdown ring", |ui| {
        ui.horizontal_wrapped(|ui| {
            countdown_ring(ui, 0.7, "14:32", "until break", accent());
            countdown_ring(ui, 0.3, "00:12", "break", WARN);
        });
    });
    ui.add_space(SP_S);
    card(ui, "Monitor tiles", |ui| {
        let m = |n: &str, primary: bool| MonitorInfo { name: n.into(), x: 0, y: 0, w: 1920, h: 1080, primary, ddc_power: Some(1) };
        ui.horizontal_wrapped(|ui| {
            monitor_tile(ui, &m("\\\\.\\DISPLAY1", true), false, TileState::Idle);
            monitor_tile(ui, &m("\\\\.\\DISPLAY2", false), true, TileState::Selected);
            monitor_tile(ui, &m("\\\\.\\DISPLAY3", false), false, TileState::Off);
            monitor_tile(ui, &m("\\\\.\\DISPLAY4", false), false, TileState::Blackout);
            monitor_tile(ui, &m("\\\\.\\DISPLAY5", false), false, TileState::Unsupported);
        });
    });
    ui.add_space(SP_S);
    toast(ui.ctx(), "Toast: settings saved", PillKind::Ok);
}
