#![allow(dead_code)]
//! Lane retention: opacity of finished lanes and when to remove them. std-only.

pub const KEEP_MS: i64 = 300_000;
pub const FADE_MS: i64 = 30_000;
const FINISHED_OPACITY: f32 = 0.55;

/// Running (None) = 1.0. Finished: 0.55 while elapsed <= keep_ms, then linear to 0.0
/// over fade_ms, 0.0 after. Clock backwards (now < finished) = 0.55.
/// Negative keep/fade are treated as 0; fade 0 = instant drop after keep.
pub fn lane_opacity(finished_at_ms: Option<i64>, now_ms: i64, keep_ms: i64, fade_ms: i64) -> f32 {
    let Some(t) = finished_at_ms else { return 1.0 };
    let keep = keep_ms.max(0);
    let fade = fade_ms.max(0);
    let elapsed = now_ms.saturating_sub(t);
    if elapsed <= keep {
        return FINISHED_OPACITY;
    }
    let into = elapsed - keep; // > 0, no overflow: elapsed > keep >= 0
    if into >= fade {
        return 0.0;
    }
    let frac = (fade - into) as f64 / fade as f64; // exact difference: no 1.0 rounding for huge fade
    (FINISHED_OPACITY as f64 * frac) as f32
}

/// True only when a finished lane has fully faded (opacity 0).
pub fn should_remove(finished_at_ms: Option<i64>, now_ms: i64, keep_ms: i64, fade_ms: i64) -> bool {
    finished_at_ms.is_some() && lane_opacity(finished_at_ms, now_ms, keep_ms, fade_ms) == 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    const T: i64 = 1_000_000;
    fn op(now: i64) -> f32 { lane_opacity(Some(T), now, KEEP_MS, FADE_MS) }
    fn rm(now: i64) -> bool { should_remove(Some(T), now, KEEP_MS, FADE_MS) }
    fn near(a: f32, b: f32) -> bool { (a - b).abs() < 1e-6 }

    #[test] fn consts() { assert_eq!(KEEP_MS, 300_000); assert_eq!(FADE_MS, 30_000); }
    #[test] fn running_is_one() {
        assert_eq!(lane_opacity(None, 0, KEEP_MS, FADE_MS), 1.0);
        assert_eq!(lane_opacity(None, i64::MAX, KEEP_MS, FADE_MS), 1.0);
        assert!(!should_remove(None, i64::MAX, KEEP_MS, FADE_MS));
    }
    #[test] fn just_finished() { assert!(near(op(T), 0.55)); assert!(!rm(T)); }
    #[test] fn during_keep() { assert!(near(op(T + 1), 0.55)); assert!(near(op(T + KEEP_MS - 1), 0.55)); }
    #[test] fn keep_boundary() { assert!(near(op(T + KEEP_MS), 0.55)); assert!(!rm(T + KEEP_MS)); }
    #[test] fn fade_start() { let o = op(T + KEEP_MS + 1); assert!(o < 0.55 && o > 0.54); }
    #[test] fn fade_midpoint() { assert!(near(op(T + KEEP_MS + FADE_MS / 2), 0.275)); assert!(!rm(T + KEEP_MS + FADE_MS / 2)); }
    #[test] fn fade_end_minus_one() { let o = op(T + KEEP_MS + FADE_MS - 1); assert!(o > 0.0 && o < 0.001); assert!(!rm(T + KEEP_MS + FADE_MS - 1)); }
    #[test] fn fade_end() { assert_eq!(op(T + KEEP_MS + FADE_MS), 0.0); assert!(rm(T + KEEP_MS + FADE_MS)); }
    #[test] fn after_end() { assert_eq!(op(T + KEEP_MS + FADE_MS + 1), 0.0); assert!(rm(T + KEEP_MS + FADE_MS + 1)); }
    #[test] fn monotonic() {
        let mut prev = 1.0f32;
        let mut n = T;
        while n < T + KEEP_MS + FADE_MS + 100 { let o = op(n); assert!(o <= prev); assert!((0.0..=0.55 + 1e-6).contains(&o)); prev = o; n += 997; }
    }
    #[test] fn clock_backwards() { assert!(near(op(T - 1), 0.55)); assert!(near(op(T - 10_000_000), 0.55)); assert!(!rm(T - 1)); }
    #[test] fn extreme_clock() {
        assert!(near(lane_opacity(Some(i64::MAX), i64::MIN, KEEP_MS, FADE_MS), 0.55));
        assert_eq!(lane_opacity(Some(i64::MIN), i64::MAX, KEEP_MS, FADE_MS), 0.0);
        assert!(should_remove(Some(i64::MIN), i64::MAX, KEEP_MS, FADE_MS));
        assert!(near(lane_opacity(Some(0), 0, i64::MAX, i64::MAX), 0.55));
        assert!(near(lane_opacity(Some(0), i64::MAX, i64::MAX, i64::MAX), 0.55));
        assert_eq!(lane_opacity(Some(0), i64::MAX, 0, 0), 0.0);
    }
    #[test] fn zero_fade() {
        assert!(near(lane_opacity(Some(0), 10, 10, 0), 0.55));
        assert!(!should_remove(Some(0), 10, 10, 0));
        assert_eq!(lane_opacity(Some(0), 11, 10, 0), 0.0);
        assert!(should_remove(Some(0), 11, 10, 0));
    }
    #[test] fn zero_keep() {
        assert!(near(lane_opacity(Some(0), 0, 0, 100), 0.55));
        assert!(near(lane_opacity(Some(0), 50, 0, 100), 0.275));
        assert!(should_remove(Some(0), 100, 0, 100));
    }
    #[test] fn negative_params() {
        assert!(near(lane_opacity(Some(0), 0, -5, -5), 0.55));
        assert_eq!(lane_opacity(Some(0), 1, -5, -5), 0.0);
        assert!(should_remove(Some(0), 1, i64::MIN, i64::MIN));
    }
    #[test] fn remove_iff_zero() {
        for now in (T - 5)..(T + KEEP_MS + FADE_MS + 5) {
            if now % 1013 != 0 && now != T + KEEP_MS + FADE_MS - 1 && now != T + KEEP_MS + FADE_MS { continue; }
            assert_eq!(rm(now), op(now) == 0.0, "now={now}");
        }
    }
}
