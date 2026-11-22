//! slice fade_in (owned by one team agent)
//! API: pub struct LineAnim { born_ms: i64, burst: bool }
//! API: pub fn LineAnim::new(born_ms: i64, burst: bool) -> LineAnim
//! API: pub fn LineAnim::sample(&self, now_ms: i64) -> (f32 alpha, f32 dy)
//! API: pub fn row_anim(age_ms: i64, burst: bool) -> (f32 alpha, f32 dy)
#![allow(dead_code)]
use crate::ui::logic::{ease::{ease_out_cubic, Transition}, fade::Anim};

/// Slide distance in px at age 0.
pub const SLIDE_PX: f32 = 8.0;

/// Alpha and vertical offset for a line of `age_ms`. Burst (many lines in one frame)
/// disables the animation: fully opaque, no offset.
pub fn row_anim(age_ms: i64, burst: bool) -> (f32, f32) {
    if burst {
        return (1.0, 0.0);
    }
    let alpha = ease_out_cubic(Anim::default().progress(0, age_ms));
    let dy = Transition::new(0).offset_px(age_ms, SLIDE_PX);
    (alpha, dy)
}

/// A new line with its birth time and burst flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineAnim {
    pub born_ms: i64,
    pub burst: bool,
}

impl LineAnim {
    pub fn new(born_ms: i64, burst: bool) -> LineAnim {
        LineAnim { born_ms, burst }
    }

    /// (alpha, dy) at `now_ms`. Clock before birth gives age <= 0 (alpha 0, dy full).
    pub fn sample(&self, now_ms: i64) -> (f32, f32) {
        row_anim(now_ms.saturating_sub(self.born_ms), self.burst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_monotonic_nondecreasing() {
        let mut prev = -1.0f32;
        for age in -10..=400 {
            let (a, _) = row_anim(age, false);
            assert!(a >= prev, "alpha dropped at age {age}");
            assert!((0.0..=1.0).contains(&a));
            prev = a;
        }
    }

    #[test]
    fn dy_monotonic_nonincreasing() {
        let mut prev = f32::MAX;
        for age in -10..=400 {
            let (_, dy) = row_anim(age, false);
            assert!(dy <= prev, "dy rose at age {age}");
            assert!((0.0..=SLIDE_PX).contains(&dy));
            prev = dy;
        }
    }

    #[test]
    fn ends_at_one_and_zero_offset() {
        let (a, dy) = row_anim(1000, false);
        assert_eq!(a, 1.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn starts_hidden_and_offset() {
        let (a, dy) = row_anim(0, false);
        assert_eq!(a, 0.0);
        assert_eq!(dy, SLIDE_PX);
    }

    #[test]
    fn burst_disables() {
        for age in [-5, 0, 50, 220, 10_000] {
            assert_eq!(row_anim(age, true), (1.0, 0.0));
        }
    }

    #[test]
    fn alpha_uses_shared_ease_out() {
        // Linear fade is 110/220 = 0.5 at mid-age; ease-out cubic gives 0.875.
        let (a, _) = row_anim(110, false);
        assert!((a - 0.875).abs() < 1e-6, "alpha={a}");
    }

    #[test]
    fn line_anim_sample() {
        let l = LineAnim::new(1000, false);
        assert_eq!(l.sample(1000), (0.0, SLIDE_PX));
        assert_eq!(l.sample(1_000_000), (1.0, 0.0));
        assert_eq!(l.sample(900), (0.0, SLIDE_PX));
        let b = LineAnim::new(1000, true);
        assert_eq!(b.sample(1000), (1.0, 0.0));
    }

    #[test]
    fn extremes_no_panic() {
        let _ = row_anim(i64::MIN, false);
        let _ = row_anim(i64::MAX, false);
        let l = LineAnim::new(i64::MIN, false);
        let _ = l.sample(i64::MAX);
    }
}
