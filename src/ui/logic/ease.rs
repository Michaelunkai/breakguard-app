#![allow(dead_code)]
//! Ease: std-only slide transition easing for UI.

pub const DURATION_MS: i64 = 180;

/// Cubic ease-out, clamped to 0..1. NaN -> 1.
pub fn ease_out_cubic(t: f32) -> f32 {
    if t.is_nan() {
        return 1.0;
    }
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    1.0 - u * u * u
}

/// Cubic ease-in-out, clamped to 0..1. NaN -> 1.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t.is_nan() {
        return 1.0;
    }
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

/// Critically damped spring settle (k = 6), normalised so t=1 -> exactly 1.
/// Monotonic, no overshoot. Clamped to 0..1. NaN -> 1.
pub fn spring_settle(t: f32) -> f32 {
    if t.is_nan() {
        return 1.0;
    }
    const K: f32 = 6.0;
    let t = t.clamp(0.0, 1.0);
    let f = |x: f32| 1.0 - (1.0 + K * x) * (-K * x).exp();
    f(t) / f(1.0)
}

/// Max frame step for 60 fps: 1000/60 ms. Negative/NaN -> 0; large -> cap.
pub fn clamp_step_ms(dt_ms: f32) -> f32 {
    const MAX_STEP: f32 = 1000.0 / 60.0;
    if dt_ms.is_nan() {
        return 0.0;
    }
    dt_ms.clamp(0.0, MAX_STEP)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub start_ms: i64,
    pub dur_ms: i64,
}

impl Transition {
    pub fn new(now_ms: i64) -> Transition {
        Transition { start_ms: now_ms, dur_ms: DURATION_MS }
    }

    /// Eased progress 0..1. Clock before start -> 0. Zero/negative dur -> 1.
    pub fn progress(&self, now_ms: i64) -> f32 {
        if self.dur_ms <= 0 {
            return 1.0;
        }
        let el = now_ms.saturating_sub(self.start_ms).clamp(0, self.dur_ms);
        ease_out_cubic(el as f32 / self.dur_ms as f32)
    }

    /// True only while animating; false before start (clock backwards) is
    /// treated as running at progress 0 only if within the same window.
    pub fn is_running(&self, now_ms: i64) -> bool {
        if self.dur_ms <= 0 {
            return false;
        }
        now_ms.saturating_sub(self.start_ms) < self.dur_ms
    }

    /// Remaining slide offset in px: distance * (1 - progress).
    pub fn offset_px(&self, now_ms: i64, distance: f32) -> f32 {
        if !distance.is_finite() {
            return 0.0;
        }
        distance * (1.0 - self.progress(now_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
    }

    #[test]
    fn clamp_and_nan() {
        assert_eq!(ease_out_cubic(-5.0), 0.0);
        assert_eq!(ease_out_cubic(5.0), 1.0);
        assert_eq!(ease_out_cubic(f32::NAN), 1.0);
        assert_eq!(ease_out_cubic(f32::INFINITY), 1.0);
        assert_eq!(ease_out_cubic(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn monotonic() {
        let mut prev = -1.0;
        for i in 0..=1000 {
            let v = ease_out_cubic(i as f32 / 1000.0);
            assert!(v >= prev);
            prev = v;
        }
        let tr = Transition::new(1000);
        let mut prev = -1.0;
        for ms in 1000..=1200 {
            let p = tr.progress(ms);
            assert!(p >= prev);
            prev = p;
        }
    }

    #[test]
    fn in_out_and_spring_endpoints_monotonic() {
        for f in [ease_in_out_cubic as fn(f32) -> f32, spring_settle] {
            assert_eq!(f(0.0), 0.0);
            assert_eq!(f(1.0), 1.0);
            assert_eq!(f(-3.0), 0.0);
            assert_eq!(f(3.0), 1.0);
            assert_eq!(f(f32::NAN), 1.0);
            let mut prev = -1.0;
            for i in 0..=1000 {
                let v = f(i as f32 / 1000.0);
                assert!(!v.is_nan());
                assert!(v >= prev);
                prev = v;
            }
        }
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn step_clamp() {
        assert_eq!(clamp_step_ms(0.0), 0.0);
        assert_eq!(clamp_step_ms(-4.0), 0.0);
        assert_eq!(clamp_step_ms(f32::NAN), 0.0);
        assert!((clamp_step_ms(500.0) - 1000.0 / 60.0).abs() < 1e-4);
        assert_eq!(clamp_step_ms(f32::INFINITY), 1000.0 / 60.0);
        assert_eq!(clamp_step_ms(5.0), 5.0);
    }

    #[test]
    fn new_has_180() {
        let tr = Transition::new(42);
        assert_eq!((tr.start_ms, tr.dur_ms), (42, 180));
    }

    #[test]
    fn progress_endpoints() {
        let tr = Transition::new(1000);
        assert_eq!(tr.progress(1000), 0.0);
        assert_eq!(tr.progress(1180), 1.0);
        assert_eq!(tr.progress(99999), 1.0);
        let mid = tr.progress(1090);
        assert!(mid > 0.5 && mid < 1.0);
    }

    #[test]
    fn clock_backwards() {
        let tr = Transition::new(1000);
        assert_eq!(tr.progress(500), 0.0);
        assert_eq!(tr.progress(i64::MIN), 0.0);
        assert_eq!(tr.offset_px(i64::MIN, 50.0), 50.0);
        assert!(tr.is_running(500));
        assert!(tr.is_running(i64::MIN));
    }

    #[test]
    fn after_end_not_running() {
        let tr = Transition::new(1000);
        assert!(tr.is_running(1000));
        assert!(tr.is_running(1179));
        assert!(!tr.is_running(1180));
        assert!(!tr.is_running(i64::MAX));
    }

    #[test]
    fn offset() {
        let tr = Transition::new(0);
        assert_eq!(tr.offset_px(0, 100.0), 100.0);
        assert_eq!(tr.offset_px(180, 100.0), 0.0);
        assert_eq!(tr.offset_px(500, -40.0), 0.0);
        let a = tr.offset_px(50, 100.0);
        let b = tr.offset_px(100, 100.0);
        assert!(a > b && b > 0.0);
    }

    #[test]
    fn offset_non_finite_distance_is_zero() {
        let tr = Transition::new(0);
        assert_eq!(tr.offset_px(0, f32::NAN), 0.0);
        assert_eq!(tr.offset_px(0, f32::INFINITY), 0.0);
        assert_eq!(tr.offset_px(0, f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn overflow_and_bad_dur() {
        let tr = Transition::new(i64::MIN);
        let _ = tr.progress(i64::MAX);
        assert!(!tr.is_running(i64::MAX));
        let z = Transition { start_ms: 0, dur_ms: 0 };
        assert_eq!(z.progress(0), 1.0);
        assert!(!z.is_running(0));
        let n = Transition { start_ms: 0, dur_ms: -5 };
        assert_eq!(n.progress(3), 1.0);
        assert!(!n.is_running(3));
    }
}
