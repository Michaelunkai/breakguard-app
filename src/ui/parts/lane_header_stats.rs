//! slice lane_header_stats (owned by one team agent)
//! API: pub fn counters_text(lines:u64,tools:u64,errors:u64)->String
//! API: pub fn last_activity_text(now_ms:i64,last_ms:i64)->String
//! API: pub fn header_stats(ui:&mut egui::Ui,lines:u64,tools:u64,errors:u64,now_ms:i64,last_ms:i64)
#![allow(dead_code)]

use eframe::egui;

use crate::ui::logic::counts::human;

/// "1.2k lines  3 tools  0 errors"
pub fn counters_text(lines: u64, tools: u64, errors: u64) -> String {
    format!(
        "{} lines  {} tools  {} errors",
        human(lines),
        human(tools),
        human(errors)
    )
}

/// "just now" under 5 s, "Ns ago" under 60 s, "Nm ago" under 60 min,
/// "Nh ago" otherwise. `last_ms <= 0` means no activity yet.
pub fn last_activity_text(now_ms: i64, last_ms: i64) -> String {
    if last_ms <= 0 {
        return "idle".to_string();
    }
    let diff = now_ms.saturating_sub(last_ms).max(0);
    if diff < 5_000 {
        "just now".to_string()
    } else if diff < 60_000 {
        format!("{}s ago", diff / 1_000)
    } else if diff < 3_600_000 {
        format!("{}m ago", diff / 60_000)
    } else {
        format!("{}h ago", diff / 3_600_000)
    }
}

pub fn header_stats(
    ui: &mut egui::Ui,
    lines: u64,
    tools: u64,
    errors: u64,
    now_ms: i64,
    last_ms: i64,
) {
    ui.horizontal(|ui| {
        ui.label(counters_text(lines, tools, errors));
        ui.weak(last_activity_text(now_ms, last_ms));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_plain_and_shortened() {
        assert_eq!(counters_text(0, 0, 0), "0 lines  0 tools  0 errors");
        assert_eq!(counters_text(1200, 3, 0), "1.2k lines  3 tools  0 errors");
        assert_eq!(counters_text(999, 1, 2), "999 lines  1 tools  2 errors");
    }

    #[test]
    fn counters_millions_truncate_not_round() {
        assert_eq!(counters_text(1_999_999, 0, 0), "1.9M lines  0 tools  0 errors");
        assert_eq!(counters_text(0, 2_000_000_000, 0), "0 lines  2.0G tools  0 errors");
    }

    #[test]
    fn activity_idle_when_no_timestamp() {
        assert_eq!(last_activity_text(10_000, 0), "idle");
        assert_eq!(last_activity_text(10_000, -5), "idle");
    }

    #[test]
    fn activity_just_now_under_5s() {
        let now = 100_000;
        assert_eq!(last_activity_text(now, now), "just now");
        assert_eq!(last_activity_text(now, now - 4_999), "just now");
    }

    #[test]
    fn activity_seconds_branch_and_boundary_59_60() {
        let now = 1_000_000;
        assert_eq!(last_activity_text(now, now - 5_000), "5s ago");
        assert_eq!(last_activity_text(now, now - 12_000), "12s ago");
        assert_eq!(last_activity_text(now, now - 59_999), "59s ago");
        assert_eq!(last_activity_text(now, now - 60_000), "1m ago");
    }

    #[test]
    fn activity_minutes_branch_and_boundary_59_60() {
        let now = 10_000_000;
        assert_eq!(last_activity_text(now, now - 3 * 60_000), "3m ago");
        assert_eq!(last_activity_text(now, now - 3_599_999), "59m ago");
        assert_eq!(last_activity_text(now, now - 3_600_000), "1h ago");
    }

    #[test]
    fn activity_hours_branch() {
        let now = 200_000_000;
        assert_eq!(last_activity_text(now, now - 2 * 3_600_000), "2h ago");
        assert_eq!(last_activity_text(now, now - 30 * 3_600_000), "30h ago");
    }

    #[test]
    fn activity_future_timestamp_is_just_now() {
        assert_eq!(last_activity_text(1_000, 5_000), "just now");
    }
}
