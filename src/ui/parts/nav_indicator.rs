//! API: pub fn indicator_y(ctx: &eframe::egui::Context, id: &str, target_y: f32) -> f32
//! API: pub fn paint_indicator(p: &eframe::egui::Painter, nav_rect: eframe::egui::Rect, y: f32, h: f32, accent: eframe::egui::Color32)
//! API: pub fn ease_out_cubic(t: f32) -> f32
//! slice nav_indicator (owned by one team agent)
#![allow(dead_code)]

use eframe::egui::{Color32, Context, CornerRadius, Id, Painter, Rect, pos2, vec2};

/// Animation duration for the nav indicator slide, in seconds.
const SLIDE_SECS: f32 = 0.18;
/// Width of the solid accent bar, in points.
const BAR_W: f32 = 3.0;
/// Glow layers: (outset in points, alpha). Outer layers are fainter.
const GLOW: [(f32, u8); 3] = [(6.0, 14), (4.0, 24), (2.0, 40)];

/// Shared ease-out curve from logic/ease.rs (the project-wide easing helper).
#[allow(unused_imports)] // re-exported for callers and tests
pub use crate::ui::logic::ease::ease_out_cubic;

/// True once the animated value has reached its target (within 0.01 pt).
/// Callers request a repaint only while this is false, so no frames are spent after settle.
pub fn is_settled(value: f32, target_y: f32) -> bool {
    (value - target_y).abs() < 0.01
}

/// Smoothly animated Y for the indicator, sliding toward `target_y` over 0.18 s.
pub fn indicator_y(ctx: &Context, id: &str, target_y: f32) -> f32 {
    ctx.animate_value_with_time(Id::new(id), target_y, SLIDE_SECS)
}

/// Paints an accent bar on the left edge of `nav_rect` at `y` with height `h`,
/// surrounded by a soft glow.
pub fn paint_indicator(p: &Painter, nav_rect: Rect, y: f32, h: f32, accent: Color32) {
    if !(h > 0.0) || !y.is_finite() || !h.is_finite() {
        return;
    }
    let [r, g, b, _] = accent.to_array();
    let x0 = nav_rect.left();
    for (outset, alpha) in GLOW {
        let rect = Rect::from_min_size(
            pos2(x0 - outset * 0.5, y - outset),
            vec2(BAR_W + outset, h + outset * 2.0),
        );
        let radius = CornerRadius::same((outset + BAR_W * 0.5).min(255.0) as u8);
        p.rect_filled(rect, radius, Color32::from_rgba_unmultiplied(r, g, b, alpha));
    }
    let bar = Rect::from_min_size(pos2(x0, y), vec2(BAR_W, h));
    p.rect_filled(bar, CornerRadius::same(2), accent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_endpoints() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ease_is_monotonic() {
        let mut prev = ease_out_cubic(0.0);
        for i in 1..=1000 {
            let v = ease_out_cubic(i as f32 / 1000.0);
            assert!(v >= prev, "not monotonic at step {i}");
            prev = v;
        }
    }

    #[test]
    fn ease_clamps_out_of_range() {
        assert_eq!(ease_out_cubic(-1.0), 0.0);
        assert!((ease_out_cubic(2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn settle_detection() {
        assert!(is_settled(120.0, 120.0));
        assert!(is_settled(120.005, 120.0));
        assert!(!is_settled(119.9, 120.0));
        assert!(!is_settled(0.0, 120.0));
    }

    #[test]
    fn ease_is_ease_out() {
        // Ease-out: front-loaded, so midpoint is past half.
        assert!(ease_out_cubic(0.5) > 0.5);
    }

    fn pass_at(ctx: &Context, time: f64) {
        let mut raw = eframe::egui::RawInput::default();
        raw.time = Some(time);
        ctx.begin_pass(raw);
    }

    #[test]
    fn indicator_y_eases_then_converges() {
        let ctx = Context::default();
        pass_at(&ctx, 0.0);
        assert_eq!(indicator_y(&ctx, "nav_test", 0.0), 0.0);
        let _ = ctx.end_pass();

        // Target jumps to 120; a short time later the value is mid-slide.
        pass_at(&ctx, 0.05);
        let _ = indicator_y(&ctx, "nav_test", 120.0); // slide starts here (value still 0)
        let _ = ctx.end_pass();
        pass_at(&ctx, 0.10);
        let mid = indicator_y(&ctx, "nav_test", 120.0);
        let _ = ctx.end_pass();
        assert!(mid > 0.0 && mid < 120.0, "mid = {mid}");

        // Well past the 0.18 s slide the value has settled on the target.
        pass_at(&ctx, 1.0);
        let settled = indicator_y(&ctx, "nav_test", 120.0);
        let _ = ctx.end_pass();
        assert!((settled - 120.0).abs() < 1e-3, "settled = {settled}");
    }

    #[test]
    fn paint_indicator_draws_only_for_valid_input() {
        let nav = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 400.0));
        let rect_count = |y: f32, h: f32| {
            let ctx = Context::default();
            ctx.begin_pass(eframe::egui::RawInput::default());
            let p = Painter::new(
                ctx.clone(),
                eframe::egui::LayerId::background(),
                Rect::EVERYTHING,
            );
            paint_indicator(&p, nav, y, h, Color32::RED);
            ctx.end_pass()
                .shapes
                .iter()
                .filter(|c| matches!(c.shape, eframe::egui::Shape::Rect(_)))
                .count()
        };
        // Glow layers plus the solid bar.
        assert_eq!(rect_count(10.0, 20.0), GLOW.len() + 1);
        assert_eq!(rect_count(10.0, 0.0), 0);
        assert_eq!(rect_count(10.0, -5.0), 0);
        assert_eq!(rect_count(f32::NAN, 20.0), 0);
        assert_eq!(rect_count(10.0, f32::INFINITY), 0);
    }
}
