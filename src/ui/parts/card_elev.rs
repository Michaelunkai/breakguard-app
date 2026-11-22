//! API: pub fn elevated_frame(level: u8) -> egui::Frame
//! API: pub fn elevation_shadow(level: u8) -> egui::epaint::Shadow
//! slice card_elev (owned by one team agent)
#![allow(dead_code)]

use crate::ui::theme;
use eframe::egui::{self, epaint::Shadow, Color32, CornerRadius, Stroke};

/// Highest supported elevation level. Higher levels clamp to this.
const MAX_LEVEL: u8 = 3;

fn clamp_level(level: u8) -> u8 {
    level.min(MAX_LEVEL)
}

/// Soft drop shadow for an elevation level. Level 0 is flat (no shadow).
/// Blur and y-offset grow strictly with the level.
pub fn elevation_shadow(level: u8) -> Shadow {
    match clamp_level(level) {
        0 => Shadow { offset: [0, 0], blur: 0, spread: 0, color: Color32::TRANSPARENT },
        1 => Shadow { offset: [0, 1], blur: 4, spread: 0, color: Color32::from_black_alpha(60) },
        2 => Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(90) },
        _ => Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(120) },
    }
}

/// Card surface for an elevation level: fill steps from SURFACE toward SURFACE_HI,
/// 1px BORDER stroke, theme::R_M corners, and the elevation shadow.
pub fn elevated_frame(level: u8) -> egui::Frame {
    let lvl = clamp_level(level);
    let p = theme::pal();
    let fill = theme::mix(p.surface, p.surface_hi, f32::from(lvl) / f32::from(MAX_LEVEL));
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(theme::R_M))
        .shadow(elevation_shadow(lvl))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_blur_and_offset_monotonic() {
        for l in 0..MAX_LEVEL {
            let a = elevation_shadow(l);
            let b = elevation_shadow(l + 1);
            assert!(b.blur > a.blur, "blur not increasing at level {l}");
            assert!(b.offset[1] > a.offset[1], "offset not increasing at level {l}");
        }
    }

    #[test]
    fn shadow_clamps_above_max_level() {
        assert_eq!(elevation_shadow(9).blur, elevation_shadow(MAX_LEVEL).blur);
        assert_eq!(elevated_frame(200).fill, elevated_frame(MAX_LEVEL).fill);
    }

    #[test]
    fn border_distinct_from_fill_every_level() {
        for l in 0..=MAX_LEVEL {
            let f = elevated_frame(l);
            assert_ne!(f.stroke.color, f.fill, "border equals fill at level {l}");
            assert_eq!(f.stroke.color, theme::BORDER);
            assert!((f.stroke.width - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn frame_uses_radius_and_shadow() {
        let f = elevated_frame(2);
        assert_eq!(f.corner_radius, CornerRadius::same(theme::R_M));
        assert_eq!(f.shadow.blur, elevation_shadow(2).blur);
    }
}
