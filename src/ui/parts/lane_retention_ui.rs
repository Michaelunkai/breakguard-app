//! slice lane_retention_ui (owned by one team agent)
//! API: `pub fn lane_opacity(finished_at_ms: Option<i64>, now_ms: i64, keep_min: u32) -> f32`
//! API: `pub fn should_remove(finished_at_ms: Option<i64>, now_ms: i64, keep_min: u32) -> bool`
#![allow(dead_code)]

use crate::ui::logic::retain;

const MIN_MS: i64 = 60_000;
const FADE_TAIL_MS: i64 = 20_000;

/// Splits the N-minute window into (hold, fade): 0.55 for `hold` ms, then fade over the last 20 s.
/// keep_min = 0 gives (0, 0): instant drop.
fn windows(keep_min: u32) -> (i64, i64) {
    let keep = keep_min as i64 * MIN_MS;
    let fade = keep.min(FADE_TAIL_MS);
    (keep - fade, fade)
}

/// Running (None) = 1.0. Finished: 0.55 until the last 20 s of the N-minute window,
/// then linear to 0.0 at N minutes.
pub fn lane_opacity(finished_at_ms: Option<i64>, now_ms: i64, keep_min: u32) -> f32 {
    let (hold, fade) = windows(keep_min);
    retain::lane_opacity(finished_at_ms, now_ms, hold, fade)
}

/// True only when a finished lane has reached opacity 0 (N minutes after finish).
pub fn should_remove(finished_at_ms: Option<i64>, now_ms: i64, keep_min: u32) -> bool {
    let (hold, fade) = windows(keep_min);
    retain::should_remove(finished_at_ms, now_ms, hold, fade)
}

#[cfg(test)]
mod tests {
    use super::*;
    const T: i64 = 1_000_000;
    const N: u32 = 5;
    const N_MS: i64 = 5 * MIN_MS;
    fn op(now: i64) -> f32 { lane_opacity(Some(T), now, N) }
    fn rm(now: i64) -> bool { should_remove(Some(T), now, N) }
    fn near(a: f32, b: f32) -> bool { (a - b).abs() < 1e-5 }

    #[test] fn running_is_one() {
        assert_eq!(lane_opacity(None, T, N), 1.0);
        assert_eq!(lane_opacity(None, i64::MAX, N), 1.0);
        assert!(!should_remove(None, i64::MAX, N));
    }
    #[test] fn just_finished() { assert!(near(op(T), 0.55)); assert!(!rm(T)); }
    #[test] fn n_minus_one_min() {
        assert!(near(op(T + 4 * MIN_MS), 0.55));
        assert!(!rm(T + 4 * MIN_MS));
    }
    #[test] fn fade_window_start() {
        // last 20 s starts at N min - 20 s = 4 min 40 s
        assert!(near(op(T + N_MS - FADE_TAIL_MS), 0.55));
        assert!(!rm(T + N_MS - FADE_TAIL_MS));
        let o = op(T + N_MS - FADE_TAIL_MS + 1);
        assert!(o < 0.55 && o > 0.54);
    }
    #[test] fn fade_midpoint() {
        assert!(near(op(T + N_MS - 10_000), 0.275));
        assert!(!rm(T + N_MS - 10_000));
    }
    #[test] fn n_minus_1ms() {
        let o = op(T + N_MS - 1);
        assert!(o > 0.0 && o < 0.001);
        assert!(!rm(T + N_MS - 1));
    }
    #[test] fn n_min_exact() {
        assert_eq!(op(T + N_MS), 0.0);
        assert!(rm(T + N_MS));
    }
    #[test] fn n_plus_one_ms() {
        assert_eq!(op(T + N_MS + 1), 0.0);
        assert!(rm(T + N_MS + 1));
    }
    #[test] fn monotonic_and_bounded() {
        let mut prev = 1.0f32;
        let mut n = T;
        while n < T + N_MS + 1_000 {
            let o = op(n);
            assert!(o <= prev && (0.0..=0.55 + 1e-6).contains(&o), "now={n}");
            prev = o;
            n += 997;
        }
    }
    #[test] fn clock_backwards_stays_dimmed() {
        assert!(near(op(T - 1), 0.55));
        assert!(!rm(T - 1));
    }
    #[test] fn keep_zero_is_instant() {
        assert!(near(lane_opacity(Some(T), T, 0), 0.55));
        assert_eq!(lane_opacity(Some(T), T + 1, 0), 0.0);
        assert!(should_remove(Some(T), T + 1, 0));
    }
    #[test] fn keep_one_min_fades_last_20s() {
        assert!(near(lane_opacity(Some(T), T + 40_000, 1), 0.55));
        assert!(near(lane_opacity(Some(T), T + 50_000, 1), 0.275));
        assert_eq!(lane_opacity(Some(T), T + 60_000, 1), 0.0);
        assert!(should_remove(Some(T), T + 60_000, 1));
    }
    #[test] fn huge_keep_no_overflow() {
        assert!(near(lane_opacity(Some(0), 1_000, u32::MAX), 0.55));
        assert!(!should_remove(Some(0), 1_000, u32::MAX));
    }
    #[test] fn extreme_clock() {
        assert!(near(lane_opacity(Some(i64::MAX), i64::MIN, N), 0.55));
        assert!(should_remove(Some(i64::MIN), i64::MAX, N));
    }
}
