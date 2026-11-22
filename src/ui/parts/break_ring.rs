//! slice break_ring (owned by one team agent)
//! API: pub fn countdown_ring(ui: &mut Ui, size: f32, fraction_left: f32, label: &str)  (square ring, arc = eased fraction, label centred)
//! API: pub fn monitor_card(ui: &mut Ui, name: &str, state: &str, ok: bool)  (card: name, status dot OK/DANGER, state text)
//! API: pub fn clamp_fraction(f: f32) -> f32  (pure: NaN -> 0, else clamped to 0..=1)
//! API: pub fn ring_fraction(f: f32) -> f32  (pure: clamp_fraction then ease_out_cubic; what the arc draws)
//! API: pub fn status_color(ok: bool) -> Color32  (pure: OK when ok, DANGER otherwise)
#![allow(dead_code)]

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Ui, Vec2,
};

use crate::ui::logic::{ease::ease_out_cubic, ring};
use crate::ui::theme;

const ARC_SEGMENTS: usize = 64;
const STROKE_W: f32 = 6.0;
const DOT_R: f32 = 4.0;

/// Clamp a fraction to 0..=1. NaN -> 0 (empty ring).
pub fn clamp_fraction(f: f32) -> f32 {
    if f.is_nan() {
        return 0.0;
    }
    f.clamp(0.0, 1.0)
}

/// Fraction the arc draws: clamped, then eased out for a smooth finish.
pub fn ring_fraction(f: f32) -> f32 {
    ease_out_cubic(clamp_fraction(f))
}

/// Status colour for a monitor: OK when healthy, DANGER otherwise.
pub fn status_color(ok: bool) -> Color32 {
    if ok {
        theme::OK
    } else {
        theme::DANGER
    }
}

/// Circular countdown ring of `size` px square. `fraction_left` is clamped (NaN -> empty).
pub fn countdown_ring(ui: &mut Ui, size: f32, fraction_left: f32, label: &str) {
    let side = if size.is_finite() { size.max(0.0) } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    if side <= STROKE_W * 2.0 {
        return;
    }
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = side * 0.5 - STROKE_W * 0.5;

    painter.circle_stroke(center, radius, Stroke::new(STROKE_W, theme::pal().border));

    let pts = ring::arc_points(
        (center.x, center.y),
        radius,
        ring_fraction(fraction_left),
        ARC_SEGMENTS,
    );
    if pts.len() >= 2 {
        let line: Vec<Pos2> = pts.into_iter().map(|(x, y)| Pos2::new(x, y)).collect();
        painter.add(Shape::line(line, Stroke::new(STROKE_W, theme::accent())));
    }

    painter.text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional((side * 0.2).max(10.0)),
        theme::pal().text,
    );
}

/// Monitor status card: name, coloured status dot, state text.
pub fn monitor_card(ui: &mut Ui, name: &str, state: &str, ok: bool) {
    crate::ui::widgets::card(ui, "", |ui| {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(DOT_R * 2.0), Sense::hover());
            ui.painter()
                .circle_filled(dot.center(), DOT_R, status_color(ok));
            ui.label(egui::RichText::new(name).strong().color(theme::pal().text));
        });
        ui.label(egui::RichText::new(state).size(12.0).color(theme::pal().text_dim));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_bounds() {
        assert_eq!(clamp_fraction(-0.5), 0.0);
        assert_eq!(clamp_fraction(0.0), 0.0);
        assert_eq!(clamp_fraction(0.4), 0.4);
        assert_eq!(clamp_fraction(1.0), 1.0);
        assert_eq!(clamp_fraction(7.0), 1.0);
        assert_eq!(clamp_fraction(f32::INFINITY), 1.0);
        assert_eq!(clamp_fraction(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn clamp_nan_is_zero() {
        assert_eq!(clamp_fraction(f32::NAN), 0.0);
        assert_eq!(ring_fraction(f32::NAN), 0.0);
    }

    #[test]
    fn ring_fraction_in_range_and_monotonic() {
        let mut prev = -1.0_f32;
        for i in -20..=120 {
            let v = ring_fraction(i as f32 / 100.0);
            assert!((0.0..=1.0).contains(&v));
            assert!(v >= prev);
            prev = v;
        }
        assert_eq!(ring_fraction(0.0), 0.0);
        assert_eq!(ring_fraction(1.0), 1.0);
    }

    #[test]
    fn status_colors() {
        assert_eq!(status_color(true), theme::OK);
        assert_eq!(status_color(false), theme::DANGER);
    }

    #[test]
    fn nan_arc_is_empty() {
        assert!(ring::arc_points((0.0, 0.0), 10.0, ring_fraction(f32::NAN), 16).is_empty());
    }
}
