//! Pure UI-side logic: messages, titles, hints, presets, CSV. No egui, no Win32, fully unit tested.
use crate::engine::{Phase, Settings};
use crate::stats::DayStats;

/// Greeting under the dashboard timer, chosen from today's progress against the daily goal.
pub fn greeting(taken: u32, goal: u32, streak: u32) -> String {
    let goal = goal.max(1);
    let mut s = if taken == 0 {
        "No breaks yet today. Your eyes will thank you for the first one.".to_string()
    } else if taken * 2 < goal {
        format!("Good start: {taken} of {goal} breaks done.")
    } else if taken < goal {
        format!("Over halfway: {taken} of {goal} breaks. Keep going!")
    } else if taken == goal {
        format!("Goal reached! All {goal} breaks taken today.")
    } else {
        format!("Above your goal: {taken} breaks today. Great care for your eyes!")
    };
    if streak >= 2 {
        s.push_str(&format!(" {streak}-day streak."));
    }
    s
}

/// Fraction of the daily goal reached (0..=1).
pub fn goal_fraction(taken: u32, goal: u32) -> f32 {
    (taken as f32 / goal.max(1) as f32).clamp(0.0, 1.0)
}

fn mmss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Window title with the live countdown.
pub fn window_title(phase: Phase, paused: bool) -> String {
    match phase {
        Phase::Off => "BreakGuard".to_string(),
        Phase::Working { left } if paused => format!("BreakGuard - paused at {}", mmss(left)),
        Phase::Working { left } => format!("BreakGuard - {} until break", mmss(left)),
        Phase::Warning { left } => format!("BreakGuard - break in {left} s"),
        Phase::Break { left } => format!("BreakGuard - on break, {} left", mmss(left)),
    }
}

/// Advice from the last days' counters; `None` when there is too little data or all is fine.
pub fn coach_hint(taken: u32, skipped: u32, snoozed: u32) -> Option<&'static str> {
    let total = taken + skipped + snoozed;
    if total < 5 {
        return None;
    }
    if skipped * 100 >= total * 40 {
        Some("You skip many breaks. Try a shorter work period so breaks feel less disruptive.")
    } else if snoozed * 100 >= total * 40 {
        Some("You snooze often. Try a longer snooze length so a break can wait for a good moment.")
    } else {
        None
    }
}

/// Break presets: (name, work minutes, break seconds).
pub const PRESETS: [(&str, u32, u32); 4] = [
    ("20-20-20", 20, 20),
    ("Pomodoro 25/5", 25, 300),
    ("Focus 50/10", 50, 600),
    ("Deep 90/20", 90, 1200),
];

/// Index of the preset matching the settings, `None` = custom.
pub fn preset_index(s: &Settings) -> Option<usize> {
    PRESETS.iter().position(|p| p.1 == s.work_min && p.2 == s.break_secs)
}

/// Days since 1970-01-01 to (year, month, day).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Stats as CSV text (header + one row per day, oldest first).
pub fn stats_csv(days: &[DayStats]) -> String {
    let mut v: Vec<&DayStats> = days.iter().collect();
    v.sort_by_key(|d| d.day);
    let mut out = String::from("date,breaks_taken,breaks_skipped,breaks_snoozed,rest_seconds\r\n");
    for d in v {
        let (y, m, dd) = civil(d.day as i64);
        out.push_str(&format!("{y:04}-{m:02}-{dd:02},{},{},{},{}\r\n", d.taken, d.skipped, d.snoozed, d.rest_secs));
    }
    out
}

/// Factory defaults, keeping what is not a preference: first-run flag and chosen monitors.
pub fn reset_settings(cur: &Settings) -> Settings {
    Settings { onboarded: true, selected: cur.selected.clone(), ..Settings::default() }
}

/// First-run card: apply preset `idx` (None keeps the current numbers), enable the timer, mark done.
pub fn apply_onboarding(s: &mut Settings, idx: Option<usize>) {
    if let Some(p) = idx.and_then(|i| PRESETS.get(i)) {
        s.work_min = p.1;
        s.break_secs = p.2;
    }
    s.break_enabled = true;
    s.onboarded = true;
}

/// UI zoom percent, clamped.
pub fn clamp_scale(p: u32) -> u32 {
    p.clamp(90, 150)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_for_zero_half_goal_over() {
        assert!(greeting(0, 8, 0).starts_with("No breaks yet"));
        assert!(greeting(3, 8, 0).starts_with("Good start: 3 of 8"));
        assert!(greeting(4, 8, 0).starts_with("Over halfway: 4 of 8"));
        assert!(greeting(8, 8, 0).starts_with("Goal reached"));
        assert!(greeting(10, 8, 0).starts_with("Above your goal: 10"));
        assert!(greeting(8, 8, 5).ends_with("5-day streak."));
        assert!(!greeting(8, 8, 1).contains("streak"));
        assert_eq!(goal_fraction(4, 8), 0.5);
        assert_eq!(goal_fraction(20, 8), 1.0);
        assert_eq!(goal_fraction(1, 0), 1.0);
    }

    #[test]
    fn title_shows_countdown() {
        assert_eq!(window_title(Phase::Off, false), "BreakGuard");
        assert_eq!(window_title(Phase::Working { left: 754 }, false), "BreakGuard - 12:34 until break");
        assert_eq!(window_title(Phase::Working { left: 754 }, true), "BreakGuard - paused at 12:34");
        assert_eq!(window_title(Phase::Warning { left: 9 }, false), "BreakGuard - break in 9 s");
        assert_eq!(window_title(Phase::Break { left: 65 }, false), "BreakGuard - on break, 1:05 left");
    }

    #[test]
    fn coach_hint_thresholds() {
        assert_eq!(coach_hint(2, 2, 0), None, "too little data");
        assert_eq!(coach_hint(8, 1, 1), None, "healthy");
        assert!(coach_hint(5, 5, 0).unwrap().contains("shorter work period"));
        assert!(coach_hint(6, 3, 0).is_none(), "29% skipped is fine");
        assert!(coach_hint(6, 4, 0).is_some(), "exactly 40% skipped");
        assert!(coach_hint(5, 0, 5).unwrap().contains("longer snooze"));
        assert!(coach_hint(2, 5, 5).unwrap().contains("shorter work period"), "skips win over snoozes");
    }

    #[test]
    fn presets_match_settings() {
        let mut s = Settings::default();
        assert_eq!(preset_index(&s), Some(0));
        s.work_min = 25;
        s.break_secs = 300;
        assert_eq!(preset_index(&s), Some(1));
        s.work_min = 33;
        assert_eq!(preset_index(&s), None);
    }

    #[test]
    fn csv_format() {
        let days = [
            DayStats { day: 20_453, taken: 6, skipped: 1, snoozed: 2, rest_secs: 360 },
            DayStats { day: 20_452, taken: 0, skipped: 0, snoozed: 0, rest_secs: 0 },
        ];
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_453), (2025, 12, 31));
        let t = stats_csv(&days);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "date,breaks_taken,breaks_skipped,breaks_snoozed,rest_seconds");
        assert_eq!(lines[1], "2025-12-30,0,0,0,0", "oldest first");
        assert_eq!(lines[2], "2025-12-31,6,1,2,360");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn reset_restores_defaults_but_keeps_first_run_and_monitors() {
        let mut s = Settings { work_min: 77, sound: false, accent: 3, ..Settings::default() };
        s.selected = vec!["A".into()];
        let r = reset_settings(&s);
        assert_eq!(r.work_min, Settings::default().work_min);
        assert!(r.sound && r.accent == 0);
        assert!(r.onboarded);
        assert_eq!(r.selected, vec!["A".to_string()]);
    }

    #[test]
    fn onboarding_click_path() {
        let mut s = Settings::default();
        assert!(!s.onboarded && !s.break_enabled);
        apply_onboarding(&mut s, Some(1));
        assert_eq!((s.work_min, s.break_secs), (25, 300));
        assert!(s.onboarded && s.break_enabled);
        let mut t = Settings { work_min: 33, ..Settings::default() };
        apply_onboarding(&mut t, None);
        assert_eq!(t.work_min, 33, "no preset keeps the numbers");
        assert!(t.onboarded && t.break_enabled);
    }

    #[test]
    fn scale_clamps() {
        assert_eq!((clamp_scale(10), clamp_scale(125), clamp_scale(900)), (90, 125, 150));
    }
}
