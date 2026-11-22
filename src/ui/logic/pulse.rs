#![allow(dead_code)]
//! Pulse: std-only pure timing logic for the lane log page.

const PULSE_PERIOD_MS: i64 = 1400;
const BLINK_HALF_MS: i64 = 530;
const MIN_REPAINT_MS: u64 = 33;
const MAX_REPAINT_MS: u64 = 530;

/// Smooth sine in 0.35..=1.0, period 1400 ms, starts at minimum. Idle: 0.35.
pub fn pulse(now_ms: i64, running: bool) -> f32 {
    if !running {
        return 0.35;
    }
    let phase = now_ms.rem_euclid(PULSE_PERIOD_MS) as f64 / PULSE_PERIOD_MS as f64;
    let s = (phase * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2).sin();
    ((0.675 + 0.325 * s) as f32).clamp(0.35, 1.0)
}

/// Running: 530 ms on, 530 ms off, on first. Idle: false.
pub fn cursor_visible(now_ms: i64, running: bool) -> bool {
    running && now_ms.rem_euclid(2 * BLINK_HALF_MS) < BLINK_HALF_MS
}

/// None when idle (no repaint needed). `any_anim` (fade etc.) asks for the
/// 33 ms frame floor; running alone waits for the next cursor toggle.
/// Always clamped to 33..=530.
pub fn next_repaint_ms(now_ms: i64, running: bool, any_anim: bool) -> Option<u64> {
    if !running && !any_anim {
        return None;
    }
    if any_anim || !running {
        return Some(MIN_REPAINT_MS);
    }
    let to_toggle = (BLINK_HALF_MS - now_ms.rem_euclid(BLINK_HALF_MS)) as u64;
    Some(to_toggle.clamp(MIN_REPAINT_MS, MAX_REPAINT_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_repeats() {
        for t in [0i64, 1, 350, 700, 1399, 123_456] {
            assert!((pulse(t, true) - pulse(t + 1400, true)).abs() < 1e-5);
            assert!((pulse(t, true) - pulse(t - 1400, true)).abs() < 1e-5);
        }
        assert!((pulse(0, true) - 0.35).abs() < 1e-5);
        assert!((pulse(700, true) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn bounds_sweep_and_idle_constant() {
        for t in (-5000..5000).step_by(7) {
            let p = pulse(t, true);
            assert!((0.35..=1.0).contains(&p), "t={t} p={p}");
            assert_eq!(pulse(t, false), 0.35);
        }
    }

    #[test]
    fn negative_and_extreme_time_safe() {
        for t in [i64::MIN, i64::MIN + 1, -1, i64::MAX, i64::MAX - 1] {
            let p = pulse(t, true);
            assert!((0.35..=1.0).contains(&p) && p.is_finite());
            let _ = cursor_visible(t, true);
            for a in [false, true] {
                if let Some(ms) = next_repaint_ms(t, true, a) {
                    assert!((33..=530).contains(&ms));
                }
            }
        }
        assert!(!cursor_visible(-1, true));
        assert!(cursor_visible(-531, true));
    }

    #[test]
    fn cursor_blink_530() {
        assert!(cursor_visible(0, true));
        assert!(cursor_visible(529, true));
        assert!(!cursor_visible(530, true));
        assert!(!cursor_visible(1059, true));
        assert!(cursor_visible(1060, true));
        for t in [-3000i64, 0, 1, 530, 99999] {
            assert!(!cursor_visible(t, false));
        }
    }

    #[test]
    fn none_when_idle() {
        for t in [i64::MIN, -1, 0, 1, 530, i64::MAX] {
            assert_eq!(next_repaint_ms(t, false, false), None);
        }
    }

    #[test]
    fn repaint_clamped_and_some_when_active() {
        for t in -2000..2000 {
            for (r, a) in [(true, false), (true, true), (false, true)] {
                let ms = next_repaint_ms(t, r, a).expect("active");
                assert!((33..=530).contains(&ms), "t={t} ms={ms}");
            }
        }
        assert_eq!(next_repaint_ms(0, true, false), Some(530));
        assert_eq!(next_repaint_ms(520, true, false), Some(33)); // 10 -> clamp up
        assert_eq!(next_repaint_ms(0, false, true), Some(33));
    }

    #[test]
    fn repaint_lands_on_toggle() {
        for t in 0..3000i64 {
            if let Some(ms) = next_repaint_ms(t, true, false) {
                let before = cursor_visible(t, true);
                // at exact wait, unless clamped up, state has flipped
                if ms as i64 == 530 - t.rem_euclid(530) {
                    assert_ne!(before, cursor_visible(t + ms as i64, true));
                }
            }
        }
    }
}
