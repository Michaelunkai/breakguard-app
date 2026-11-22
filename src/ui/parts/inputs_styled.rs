//! slice inputs_styled (owned by one team agent)
//! API: pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response
//! API: `pub fn slider_styled(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>) -> Response`
//! API: pub fn text_input(ui: &mut Ui, text: &mut String, hint: &str) -> Response
//! API: pub fn number_input(ui: &mut Ui, value: &mut i32, min: i32, max: i32) -> Response
//! API: pub fn clamp_i32(v: i32, min: i32, max: i32) -> i32
//! API: pub fn toggle_knob_x(t: f32, off_x: f32, on_x: f32) -> f32
#![allow(dead_code)]

use std::ops::RangeInclusive;

use eframe::egui::{self, Color32, Response, Sense, Stroke, Ui, Vec2, WidgetInfo, WidgetType};

use crate::ui::theme;

const TOGGLE_SIZE: Vec2 = Vec2::new(44.0, 24.0);
const TOGGLE_ANIM_SECS: f32 = 0.15;
const SLIDER_HEIGHT: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const KNOB_RADIUS: f32 = 8.0;

/// Pure clamp for integer input. Swaps nothing: caller must pass min <= max.
pub fn clamp_i32(v: i32, min: i32, max: i32) -> i32 {
    v.max(min).min(max)
}

/// Accessibility info for a custom control: role, state and the caller's name.
fn control_info(kind: WidgetType, state: bool, label: &str) -> WidgetInfo {
    WidgetInfo::selected(kind, true, state, label)
}

/// Pure linear interpolation of knob x between off and on positions.
/// `t` is clamped to [0, 1], so t=0 gives `off_x` and t=1 gives `on_x`.
pub fn toggle_knob_x(t: f32, off_x: f32, on_x: f32) -> f32 {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    off_x + (on_x - off_x) * t
}

/// Animated pill toggle. Flips `on` on click.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(TOGGLE_SIZE, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let state = *on;
    response.widget_info(|| control_info(WidgetType::Checkbox, state, label));
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, *on, TOGGLE_ANIM_SECS);
    let track_fill = lerp_color(theme::SURFACE_HI, theme::accent(), t);
    let track_stroke = if response.hovered() {
        Stroke::new(1.0_f32, theme::BORDER_STRONG)
    } else {
        Stroke::new(1.0_f32, theme::BORDER)
    };
    let radius = rect.height() / 2.0;
    let painter = ui.painter();
    painter.rect(rect, radius, track_fill, track_stroke, egui::StrokeKind::Inside);
    let off_x = rect.left() + radius;
    let on_x = rect.right() - radius;
    let knob_x = toggle_knob_x(t, off_x, on_x);
    let knob_center = egui::pos2(knob_x, rect.center().y);
    painter.circle_filled(knob_center, KNOB_RADIUS, theme::TEXT);
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            radius + 2.0,
            Stroke::new(2.0_f32, focus_ring_color()),
            egui::StrokeKind::Outside,
        );
    }
    response
}

/// Horizontal slider drawn with theme tokens. Clamps value into `range`.
pub fn slider_styled(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, label: &str) -> Response {
    let lo = *range.start();
    let hi = *range.end();
    let width = ui.available_width().max(60.0);
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, SLIDER_HEIGHT), Sense::click_and_drag());
    response.widget_info(|| control_info(WidgetType::Slider, true, label));
    let before = *value;
    if (response.dragged() || response.clicked()) && hi > lo {
        if let Some(pos) = response.interact_pointer_pos() {
            let span = rect.width().max(1.0);
            let frac = ((pos.x - rect.left()) / span).clamp(0.0, 1.0);
            *value = lo + (hi - lo) * frac;
        }
    }
    *value = clamp_f32(*value, lo, hi);
    if *value != before {
        response.mark_changed();
    }
    let frac = if hi > lo {
        ((*value - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.center().y - TRACK_HEIGHT / 2.0),
        egui::pos2(rect.right(), rect.center().y + TRACK_HEIGHT / 2.0),
    );
    let filled_x = rect.left() + rect.width() * frac;
    let filled = egui::Rect::from_min_max(track.min, egui::pos2(filled_x, track.max.y));
    let painter = ui.painter();
    painter.rect_filled(track, TRACK_HEIGHT / 2.0, theme::SURFACE_HI);
    painter.rect_filled(filled, TRACK_HEIGHT / 2.0, theme::accent());
    let knob = egui::pos2(filled_x, rect.center().y);
    let knob_color = if response.hovered() || response.dragged() {
        theme::accent_hi()
    } else {
        theme::TEXT
    };
    painter.circle_filled(knob, KNOB_RADIUS, knob_color);
    response
}

/// Single-line text field with theme-coloured hint text.
pub fn text_input(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    let edit = egui::TextEdit::singleline(text)
        .hint_text(egui::RichText::new(hint).color(theme::pal().text_dim))
        .text_color(theme::TEXT)
        .desired_width(f32::INFINITY);
    ui.add(edit)
}

/// Integer field clamped to [min, max]. Out-of-range input is clamped and flagged changed.
pub fn number_input(ui: &mut Ui, value: &mut i32, min: i32, max: i32) -> Response {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let before = *value;
    let mut response = ui.add(
        egui::DragValue::new(value)
            .range(lo..=hi)
            .speed(1.0)
            .max_decimals(0),
    );
    *value = clamp_i32(*value, lo, hi);
    if *value != before {
        response.mark_changed();
    }
    response
}

/// Focus ring colour: the active accent swatch, drawn 2px outside focused controls.
pub fn focus_ring_color() -> Color32 {
    theme::accent()
}

/// WCAG relative luminance of an sRGB colour.
pub fn relative_luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio between two colours, in [1, 21].
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_nan() {
        return lo;
    }
    v.max(lo).min(hi)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_info_carries_name_and_state() {
        let info = control_info(WidgetType::Checkbox, true, "Night mode");
        assert_eq!(info.label.as_deref(), Some("Night mode"));
        assert_eq!(info.selected, Some(true));
        assert!(!control_info(WidgetType::Slider, true, "Work for").label.unwrap_or_default().is_empty());
    }

    #[test]
    fn clamp_i32_bounds() {
        assert_eq!(clamp_i32(5, 0, 10), 5);
        assert_eq!(clamp_i32(-3, 0, 10), 0);
        assert_eq!(clamp_i32(99, 0, 10), 10);
        assert_eq!(clamp_i32(i32::MIN, -1, 1), -1);
        assert_eq!(clamp_i32(i32::MAX, 0, i32::MAX), i32::MAX);
    }

    #[test]
    fn toggle_anim_endpoints() {
        assert_eq!(toggle_knob_x(0.0, 10.0, 34.0), 10.0);
        assert_eq!(toggle_knob_x(1.0, 10.0, 34.0), 34.0);
        assert_eq!(toggle_knob_x(0.5, 10.0, 34.0), 22.0);
    }

    #[test]
    fn toggle_anim_clamps_out_of_range_and_nan() {
        assert_eq!(toggle_knob_x(-2.0, 10.0, 34.0), 10.0);
        assert_eq!(toggle_knob_x(3.0, 10.0, 34.0), 34.0);
        assert_eq!(toggle_knob_x(f32::NAN, 10.0, 34.0), 10.0);
    }

    #[test]
    fn clamp_f32_bounds() {
        assert_eq!(clamp_f32(0.5, 0.0, 1.0), 0.5);
        assert_eq!(clamp_f32(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(clamp_f32(2.0, 0.0, 1.0), 1.0);
        assert_eq!(clamp_f32(f32::NAN, 0.0, 1.0), 0.0);
    }

    fn run_frame(mut body: impl FnMut(&mut Ui)) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| body(ui));
        });
    }

    #[test]
    fn number_input_clamps_headless() {
        let mut value = 50;
        run_frame(|ui| {
            number_input(ui, &mut value, 0, 10);
        });
        assert_eq!(value, 10);

        let mut low = -7;
        run_frame(|ui| {
            number_input(ui, &mut low, 0, 10);
        });
        assert_eq!(low, 0);
    }

    #[test]
    fn slider_styled_stays_in_range_headless() {
        let mut value = 5.0_f32;
        run_frame(|ui| {
            slider_styled(ui, &mut value, 0.0..=1.0, "test slider");
        });
        assert_eq!(value, 1.0);

        let mut nan = f32::NAN;
        run_frame(|ui| {
            slider_styled(ui, &mut nan, 0.0..=1.0, "test slider");
        });
        assert!((0.0..=1.0).contains(&nan));
    }

    #[test]
    fn focus_ring_contrast_at_least_3_on_surface() {
        let surface = theme::SURFACE;
        for (name, c) in theme::SWATCHES {
            let r = contrast_ratio(c, surface);
            assert!(r >= 3.0, "{name} focus ring contrast {r} < 3");
        }
        assert!(contrast_ratio(Color32::WHITE, Color32::BLACK) > 20.0);
        assert_eq!(contrast_ratio(surface, surface), 1.0);
    }

    #[test]
    fn lerp_color_endpoints() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(200, 100, 50);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
    }
}
