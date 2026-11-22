//! API: pub fn night_scene(p: &Painter, rect: Rect, on: bool, t: f64)
//! on=true: moon with twinkling stars; on=false: sunrise over horizon. Theme-token colours only.
#![allow(dead_code)]
use crate::ui::theme;
use eframe::egui::{pos2, Painter, Rect, Stroke};
use std::f32::consts::TAU;

const STARS: [(f32, f32, f32); 7] = [
    (0.12, 0.18, 1.3),
    (0.30, 0.10, 1.0),
    (0.46, 0.30, 1.6),
    (0.62, 0.14, 1.1),
    (0.20, 0.42, 0.9),
    (0.80, 0.38, 1.2),
    (0.90, 0.12, 1.0),
];

/// Draws a night/dawn scene into `rect`. `t` is seconds (any f64; non-finite treated as 0).
pub fn night_scene(p: &Painter, rect: Rect, on: bool, t: f64) {
    let t = if t.is_finite() { t as f32 } else { 0.0 };
    let w = rect.width().max(0.0);
    let h = rect.height().max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    p.rect_filled(rect, 0.0, theme::SURFACE_LO);
    if on {
        draw_moon(p, rect, t);
    } else {
        draw_sunrise(p, rect, t);
    }
}

fn draw_moon(p: &Painter, rect: Rect, t: f32) {
    let w = rect.width();
    let h = rect.height();
    let center = pos2(rect.left() + w * 0.78, rect.top() + h * 0.42);
    let r = h.min(w) * 0.22;
    // Stars twinkle: alpha oscillates per star, phase offset by index.
    for (i, (fx, fy, size)) in STARS.iter().enumerate() {
        let phase = i as f32 * 1.7;
        let k = 0.5 + 0.5 * (t * 2.2 + phase).sin();
        let pos = pos2(rect.left() + w * fx, rect.top() + h * fy);
        p.circle_filled(pos, size * 1.2, theme::accent().gamma_multiply(0.35 + 0.65 * k));
    }
    // Moon disc with a soft halo ring.
    p.circle_filled(center, r * 1.35, theme::accent().gamma_multiply(0.12));
    p.circle_filled(center, r, theme::accent_hi());
    // Crescent shadow: offset disc in sky colour.
    let (dx, dy) = crescent_offset(r);
    p.circle_filled(center + eframe_vec(dx, dy), r * 0.9, theme::SURFACE_LO);
}

fn draw_sunrise(p: &Painter, rect: Rect, t: f32) {
    let w = rect.width();
    let h = rect.height();
    let horizon = rect.top() + h * 0.72;
    // Rise progress ping-pongs slowly between 0.35 and 1.0 for a living dawn.
    let raw = 0.5 + 0.5 * (t * 0.5).sin();
    let rise = 0.35 + 0.65 * ease_out(raw);
    let r = h.min(w) * 0.26;
    let cy = horizon - r * 0.4 - (1.0 - rise) * r * 0.9;
    let center = pos2(rect.left() + w * 0.5, cy);
    // Rays radiating from sun, clipped to sky by drawing only above horizon.
    let stroke = Stroke::new(1.5_f32, theme::WARN.gamma_multiply(0.35 * rise));
    for i in 0..7 {
        let a = std::f32::consts::PI + (i as f32 + 0.5) * (TAU / 2.0) / 7.0;
        let inner = center + eframe_vec(a.cos(), a.sin()) * (r * 1.15);
        let outer = center + eframe_vec(a.cos(), a.sin()) * (r * 1.6);
        if outer.y < horizon {
            p.line_segment([inner, outer], stroke);
        }
    }
    p.circle_filled(center, r, theme::WARN.gamma_multiply(0.55 + 0.45 * rise));
    // Horizon line and ground band.
    let ground = Rect::from_min_max(pos2(rect.left(), horizon), rect.max);
    p.rect_filled(ground, 0.0, theme::SURFACE_HI);
    p.line_segment(
        [pos2(rect.left(), horizon), pos2(rect.right(), horizon)],
        Stroke::new(1.0_f32, theme::BORDER),
    );
}

/// Star count drawn in the moon scene.
pub fn star_count() -> usize {
    STARS.len()
}

/// Offset (dx, dy) of the shadow disc that cuts the moon into a crescent. Scales with radius `r`.
pub fn crescent_offset(r: f32) -> (f32, f32) {
    (r * 0.35, -r * 0.1)
}

fn eframe_vec(x: f32, y: f32) -> eframe::egui::Vec2 {
    eframe::egui::vec2(x, y)
}

fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, LayerId};

    fn painter(ctx: &Context, rect: Rect) -> Painter {
        Painter::new(ctx.clone(), LayerId::background(), rect)
    }

    #[test]
    fn moon_branch_no_panic() {
        let ctx = Context::default();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(240.0, 120.0));
        let p = painter(&ctx, rect);
        for t in [0.0_f64, 1.3, 1e6, -4.2] {
            night_scene(&p, rect, true, t);
        }
    }

    #[test]
    fn sunrise_branch_no_panic() {
        let ctx = Context::default();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(240.0, 120.0));
        let p = painter(&ctx, rect);
        for t in [0.0_f64, 2.7, 1e6, -4.2] {
            night_scene(&p, rect, false, t);
        }
    }

    #[test]
    fn degenerate_inputs_no_panic() {
        let ctx = Context::default();
        let empty = Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(0.0, 0.0));
        let p = painter(&ctx, empty);
        night_scene(&p, empty, true, 0.0);
        night_scene(&p, empty, false, 0.0);
        let rect = Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(100.0, 50.0));
        let p = painter(&ctx, rect);
        night_scene(&p, rect, true, f64::NAN);
        night_scene(&p, rect, false, f64::INFINITY);
    }

    #[test]
    fn star_count_matches_table() {
        assert_eq!(star_count(), 7);
        assert!(STARS
            .iter()
            .all(|(fx, fy, _)| (0.0..=1.0).contains(fx) && (0.0..=1.0).contains(fy)));
    }

    #[test]
    fn crescent_offset_geometry() {
        assert_eq!(crescent_offset(0.0), (0.0, 0.0));
        let (dx, dy) = crescent_offset(10.0);
        assert!(dx > 0.0 && dy < 0.0, "shadow must sit up-right of moon");
        assert!((dx.hypot(dy)) < 10.0, "offset smaller than radius keeps a crescent");
        let (dx2, dy2) = crescent_offset(20.0);
        assert!((dx2 - 2.0 * dx).abs() < 1e-5 && (dy2 - 2.0 * dy).abs() < 1e-5);
    }

    #[test]
    fn ease_out_bounds() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(-3.0), 0.0);
        assert_eq!(ease_out(9.0), 1.0);
    }
}
