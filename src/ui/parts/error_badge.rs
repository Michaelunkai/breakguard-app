//! API: pub fn badge(p: &Painter, center: Pos2, count: u32)
//! API: pub fn badge_text(count: u32) -> &'static str  (pure: "" for 0, "1".."9", "9+" above 9)
//! API: pub const BADGE_TEXT: Color32 (text colour on DANGER fill, AA >= 4.5:1; ON_FILL, white fails AA)
#![allow(dead_code)]

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2};

use crate::ui::theme::{DANGER, ON_FILL};

/// Radius of the red disc, in points.
const RADIUS: f32 = 8.0;
const FONT_SIZE: f32 = 11.0;

/// Text colour on the DANGER fill. White is only ~2.8:1 there, so dark ON_FILL is used (~6.5:1).
pub const BADGE_TEXT: Color32 = ON_FILL;

/// Label for a count: empty at 0, the digit for 1..=9, "9+" above 9.
pub fn badge_text(count: u32) -> &'static str {
    match count {
        0 => "",
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        9 => "9",
        _ => "9+",
    }
}

/// Paints a red disc with the count centred on it. Nothing is painted at 0.
pub fn badge(p: &Painter, center: Pos2, count: u32) {
    let text = badge_text(count);
    if text.is_empty() {
        return;
    }
    p.circle_filled(center, RADIUS, DANGER);
    p.text(
        center,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(FONT_SIZE),
        BADGE_TEXT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::logic::palette::contrast;

    fn rgb(c: Color32) -> (u8, u8, u8) {
        (c.r(), c.g(), c.b())
    }

    #[test]
    fn zero_is_empty() {
        assert_eq!(badge_text(0), "");
    }

    #[test]
    fn one_to_nine_are_digits() {
        for n in 1..=9u32 {
            assert_eq!(badge_text(n), n.to_string());
        }
    }

    #[test]
    fn above_nine_is_nine_plus() {
        assert_eq!(badge_text(10), "9+");
        assert_eq!(badge_text(99), "9+");
        assert_eq!(badge_text(u32::MAX), "9+");
    }

    #[test]
    fn badge_text_contrast_on_danger_meets_aa() {
        let r = contrast(rgb(BADGE_TEXT), rgb(DANGER));
        assert!(r >= 4.5, "badge text on DANGER = {r:.3}");
    }

    #[test]
    fn white_on_danger_fails_aa() {
        // Documents why BADGE_TEXT is not white.
        let r = contrast((255, 255, 255), rgb(DANGER));
        assert!(r < 4.5, "white on DANGER unexpectedly {r:.3}");
    }
}
