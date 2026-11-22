#![allow(dead_code)]
//! New-line fade/slide animation state (std-only, pure logic).

pub const ANIM_MS: i64 = 220;
pub const BURST_LIMIT: usize = 20;
pub const BURST_OFF_MS: i64 = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anim {
    disabled_until_ms: i64,
}

impl Default for Anim {
    fn default() -> Self {
        Anim { disabled_until_ms: i64::MIN }
    }
}

impl Anim {
    /// Record `count_new` lines arriving at `now_ms`. More than 20 disables animation for 2000 ms.
    pub fn on_lines(&mut self, now_ms: i64, count_new: usize) {
        if count_new > BURST_LIMIT {
            let until = now_ms.saturating_add(BURST_OFF_MS);
            if until > self.disabled_until_ms {
                self.disabled_until_ms = until;
            }
        }
    }

    /// Animation progress 0.0..=1.0 over 220 ms; 1.0 when disabled or old.
    pub fn progress(&self, line_born_ms: i64, now_ms: i64) -> f32 {
        if now_ms < self.disabled_until_ms || line_born_ms < self.disabled_until_ms {
            return 1.0;
        }
        let dt = now_ms.saturating_sub(line_born_ms);
        if dt <= 0 {
            0.0
        } else if dt >= ANIM_MS {
            1.0
        } else {
            dt as f32 / ANIM_MS as f32
        }
    }

    /// True only while the newest line still animates (UI may stop repainting when false).
    pub fn active(&self, now_ms: i64, newest_born_ms: i64) -> bool {
        self.progress(newest_born_ms, now_ms) < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal() {
        let mut a = Anim::default();
        a.on_lines(1000, 3);
        assert_eq!(a.progress(1000, 1000), 0.0);
        assert!((a.progress(1000, 1110) - 0.5).abs() < 1e-6);
        assert_eq!(a.progress(1000, 1220), 1.0);
        assert!(a.active(1100, 1000));
    }

    #[test]
    fn exactly_20_not_burst() {
        let mut a = Anim::default();
        a.on_lines(1000, 20);
        assert!(a.progress(1000, 1000) < 1.0);
        a.on_lines(1000, 21);
        assert_eq!(a.progress(1000, 1000), 1.0);
    }

    #[test]
    fn burst_disables() {
        let mut a = Anim::default();
        a.on_lines(1000, 500);
        assert_eq!(a.progress(1000, 1000), 1.0);
        assert_eq!(a.progress(2500, 2500), 1.0);
        assert!(!a.active(1100, 1000));
        assert!(!a.active(2999, 2999));
    }

    #[test]
    fn recovery_after_2s() {
        let mut a = Anim::default();
        a.on_lines(1000, 100);
        a.on_lines(3000, 1);
        assert_eq!(a.progress(3000, 3000), 0.0);
        assert!(a.active(3100, 3000));
        assert!(!a.active(3220, 3000));
    }

    #[test]
    fn idle_false() {
        let a = Anim::default();
        assert!(!a.active(1_000_000, 0));
        assert!(!a.active(i64::MAX, i64::MIN));
        assert!(!a.active(5000, 1000));
    }

    #[test]
    fn burst_extends_not_shrinks() {
        let mut a = Anim::default();
        a.on_lines(1000, 50);
        a.on_lines(500, 50); // clock goes back: must not shorten
        assert_eq!(a.progress(2500, 2500), 1.0);
    }

    #[test]
    fn adversarial_extremes_and_clock_skew() {
        let mut a = Anim::default();
        a.on_lines(i64::MAX, usize::MAX);
        let _ = a.progress(i64::MIN, i64::MAX);
        let b = Anim::default();
        assert_eq!(b.progress(i64::MIN, i64::MAX), 1.0);
        assert_eq!(b.progress(i64::MAX, i64::MIN), 0.0);
        assert_eq!(b.progress(1000, 900), 0.0); // clock backwards
        let p = b.progress(0, 219);
        assert!(p > 0.0 && p < 1.0);
        let mut c = Anim::default();
        c.on_lines(0, 0); // zero lines: no effect
        assert!(c.progress(0, 0) < 1.0);
    }
}
