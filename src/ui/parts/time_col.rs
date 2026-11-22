//! slice time_col (owned by one team agent)
//! API: pub fn stamp(mode: u8, now_ms: i64, ts_ms: i64) -> String   (0 hidden => "", 1 relative, 2 absolute, ts_ms <= 0 => "--")
//! API: pub fn column_width(mode: u8, char_w: f32) -> f32            (0 => 0.0, 1 => 10 chars, 2 => 8 chars, other => 0.0)
#![allow(dead_code)]

use crate::ui::logic::clock;

/// Timestamp text for a table cell. `mode` 0 hides the column, 1 is relative, 2 is absolute.
/// Any other mode is treated as hidden. A non-positive `ts_ms` renders as "--".
pub fn stamp(mode: u8, now_ms: i64, ts_ms: i64) -> String {
    match mode {
        1 | 2 if ts_ms <= 0 => "--".to_string(),
        1 => clock::relative(now_ms, ts_ms),
        2 => clock::absolute(ts_ms),
        _ => String::new(),
    }
}

/// Column width in pixels for the given mode, based on the widest text it can show.
pub fn column_width(mode: u8, char_w: f32) -> f32 {
    match mode {
        1 => 10.0 * char_w,
        2 => 8.0 * char_w,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 10_000_000_000;

    #[test]
    fn mode0_hidden_is_empty_even_with_zero_ts() {
        assert_eq!(stamp(0, NOW, NOW - 5_000), "");
        assert_eq!(stamp(0, NOW, 0), "");
        assert_eq!(stamp(0, NOW, -1), "");
    }

    #[test]
    fn unknown_mode_is_empty() {
        assert_eq!(stamp(3, NOW, NOW - 5_000), "");
        assert_eq!(stamp(255, NOW, 0), "");
    }

    #[test]
    fn mode1_relative_delegates_to_clock() {
        assert_eq!(stamp(1, NOW, NOW - 5_000), "5s ago");
        assert_eq!(stamp(1, NOW, NOW - 1_000), "just now");
        assert_eq!(stamp(1, NOW, NOW - 180_000), "3 min ago");
        assert_eq!(stamp(1, NOW, NOW - 7_200_000), "2 h ago");
    }

    #[test]
    fn mode2_absolute_delegates_to_clock() {
        assert_eq!(stamp(2, NOW, 1_700_000_000_000), "22:13:20");
        assert_eq!(stamp(2, NOW, 3_661_000), "01:01:01");
    }

    #[test]
    fn zero_and_negative_ts_show_dashes_in_modes_1_and_2() {
        for mode in [1u8, 2u8] {
            assert_eq!(stamp(mode, NOW, 0), "--");
            assert_eq!(stamp(mode, NOW, -1), "--");
            assert_eq!(stamp(mode, NOW, i64::MIN), "--");
        }
    }

    #[test]
    fn no_panic_on_extremes() {
        assert_eq!(stamp(1, i64::MAX, 1), clock::relative(i64::MAX, 1));
        assert_eq!(stamp(2, NOW, i64::MAX).len(), 8);
    }

    #[test]
    fn column_width_per_mode() {
        assert_eq!(column_width(0, 7.0), 0.0);
        assert_eq!(column_width(1, 7.0), 70.0);
        assert_eq!(column_width(2, 7.0), 56.0);
        assert_eq!(column_width(9, 7.0), 0.0);
    }

    #[test]
    fn column_width_fits_widest_output() {
        // widest realistic relative text is "59 min ago" (10 chars); absolute is "HH:MM:SS" (8 chars)
        assert!(stamp(1, NOW, NOW - 3_599_999).chars().count() <= 10);
        assert!(stamp(2, NOW, 86_399_999).chars().count() <= 8);
    }
}
