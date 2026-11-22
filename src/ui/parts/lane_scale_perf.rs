//! Lane scaling: off-screen lanes are not drawn (a same-height placeholder keeps the scroll bar stable)
//! and every lane keeps a minimum height, so 20+ sessions stay smooth.
#![allow(dead_code)]

/// Smallest lane body height in points (about 3 rows plus the header).
pub const MIN_LANE_H: f32 = 120.0;
/// Lanes within this many points of the viewport are still drawn, so scrolling never shows a blank lane.
pub const OVERSCAN: f32 = 240.0;

pub fn clamp_lane_h(h: f32) -> f32 {
    if h.is_finite() { h.max(MIN_LANE_H) } else { MIN_LANE_H }
}

/// True when a lane spanning `top .. top + h` touches the viewport `clip_min .. clip_max` (plus overscan).
pub fn is_visible(top: f32, h: f32, clip_min: f32, clip_max: f32) -> bool {
    top + h >= clip_min - OVERSCAN && top <= clip_max + OVERSCAN
}

/// Height reserved for an undrawn lane: the last measured height, else an estimate from the lane state.
pub fn placeholder_h(last_measured: f32, collapsed: bool, view_h: f32) -> f32 {
    if last_measured > 1.0 {
        last_measured
    } else if collapsed {
        56.0
    } else {
        clamp_lane_h(view_h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_height_is_enforced_and_nan_is_safe() {
        assert_eq!(clamp_lane_h(10.0), MIN_LANE_H);
        assert_eq!(clamp_lane_h(300.0), 300.0);
        assert_eq!(clamp_lane_h(f32::NAN), MIN_LANE_H);
    }

    #[test]
    fn visible_range_with_overscan() {
        assert!(is_visible(100.0, 200.0, 0.0, 800.0));
        assert!(is_visible(-300.0, 200.0, 0.0, 800.0), "just above, within overscan");
        assert!(!is_visible(-900.0, 200.0, 0.0, 800.0), "far above");
        assert!(!is_visible(2000.0, 200.0, 0.0, 800.0), "far below");
        assert!(is_visible(1000.0, 200.0, 0.0, 800.0), "just below, within overscan");
    }

    #[test]
    fn only_a_few_of_many_lanes_are_drawn() {
        let drawn = (0..40).filter(|i| is_visible(*i as f32 * 228.0, 220.0, 0.0, 900.0)).count();
        assert!(drawn <= 7, "drawn {drawn}");
        assert!(drawn >= 4);
    }

    #[test]
    fn placeholder_prefers_measured_height() {
        assert_eq!(placeholder_h(250.0, false, 220.0), 250.0);
        assert_eq!(placeholder_h(0.0, true, 220.0), 56.0);
        assert_eq!(placeholder_h(0.0, false, 30.0), MIN_LANE_H);
    }
}
