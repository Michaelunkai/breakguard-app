//! slice error_line (owned by one team agent)
//! API: pub fn paint_error_bar(p: &Painter, row: Rect)
//! API: pub const ERROR_TEXT: Color32
//! API: pub const ERROR_TINT: Color32 (opaque faint red row tint, used by paint_error_bar)
#![allow(dead_code)]

use eframe::egui::{pos2, vec2, Color32, Painter, Rect, Shape, Stroke};

use crate::ui::theme::{DANGER, ON_FILL};

/// Text colour for error rows. Meets 4.5:1 on `SURFACE_LO` and on `ERROR_TINT`.
pub const ERROR_TEXT: Color32 = Color32::from_rgb(252, 165, 165);

/// Opaque faint red tint: DANGER at ~10% over SURFACE_LO.
pub const ERROR_TINT: Color32 = Color32::from_rgb(36, 27, 37);

const ACCENT_W: f32 = 3.0;

/// Paints the error row: faint red tint, left red accent bar, warning glyph.
pub fn paint_error_bar(p: &Painter, row: Rect) {
    // Light theme: a pale red band so the dark-red text and grey labels stay readable (the dark tint is near-black).
    let tint = if crate::ui::theme::is_light() { Color32::from_rgb(254, 226, 226) } else { ERROR_TINT };
    p.rect_filled(row, 0.0, tint);
    p.rect_filled(
        Rect::from_min_max(row.min, pos2(row.min.x + ACCENT_W, row.max.y)),
        0.0,
        DANGER,
    );

    let h = (row.height() * 0.5).max(0.0);
    let cx = row.min.x + ACCENT_W + h * 0.9;
    let cy = row.center().y;
    let half_w = h * 0.5;
    let triangle = vec![
        pos2(cx, cy - h * 0.5),
        pos2(cx + half_w, cy + h * 0.5),
        pos2(cx - half_w, cy + h * 0.5),
    ];
    p.add(Shape::convex_polygon(triangle, DANGER, Stroke::NONE));

    let bar_w = (h * 0.12).max(0.5);
    p.rect_filled(
        Rect::from_center_size(pos2(cx, cy - h * 0.06), vec2(bar_w, h * 0.42)),
        0.0,
        ON_FILL,
    );
    p.circle_filled(pos2(cx, cy + h * 0.26), (h * 0.07).max(0.5), ON_FILL);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::SURFACE_LO;

    fn channel(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(c: Color32) -> f64 {
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn error_text_on_surface_lo_meets_aa() {
        let r = contrast(ERROR_TEXT, SURFACE_LO);
        assert!(r >= 4.5, "ERROR_TEXT on SURFACE_LO ratio {r:.2} < 4.5");
    }

    #[test]
    fn error_text_on_tint_meets_aa() {
        let r = contrast(ERROR_TEXT, ERROR_TINT);
        assert!(r >= 4.5, "ERROR_TEXT on ERROR_TINT ratio {r:.2} < 4.5");
    }

    #[test]
    fn luminance_known_values() {
        assert!(luminance(Color32::BLACK).abs() < 1e-9);
        assert!((luminance(Color32::WHITE) - 1.0).abs() < 1e-3);
    }
}
