//! slice creature_state_fx (owned by one team agent)
#![allow(dead_code)]
//! API: pub fn breathe_scale(t: f64) -> f32   (1.0..=1.04, sine, 0.4 Hz, period 2.5 s)
//! API: pub fn busy_bounce(t: f64) -> f32     (y offset in px, -2.0..=2.0, 2.5 Hz)
//! API: pub fn paint_zzz(p: &Painter, at: Pos2, t: f64, color: Color32)  (three rising, fading "Z")
//! API: pub fn paint_done_check(p: &Painter, at: Pos2, size: f32, color: Color32)  (tick mark, two strokes)

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Stroke};
use std::f64::consts::{PI, TAU};

const BREATHE_HZ: f64 = 0.4;
const BREATHE_AMP: f64 = 0.02;
const BUSY_HZ: f64 = 2.5;
const BUSY_AMP_PX: f64 = 2.0;
const ZZZ_RATE_HZ: f64 = 0.5;
const ZZZ_COUNT: usize = 3;
const ZZZ_RISE_PX: f32 = 18.0;

/// Breathing scale factor, 1.0..=1.04, sine at 0.4 Hz. Non-finite `t` gives 1.0.
pub fn breathe_scale(t: f64) -> f32 {
    if !t.is_finite() {
        return 1.0;
    }
    (1.0 + BREATHE_AMP * (1.0 + (TAU * BREATHE_HZ * t).sin())) as f32
}

/// Busy vertical bounce in px, -2.0..=2.0, at 2.5 Hz. Non-finite `t` gives 0.0.
pub fn busy_bounce(t: f64) -> f32 {
    if !t.is_finite() {
        return 0.0;
    }
    (BUSY_AMP_PX * (TAU * BUSY_HZ * t).sin()) as f32
}

/// Phase in 0.0..1.0 of the i-th Z glyph at time `t` (seconds). Rises over time.
fn zzz_phase(t: f64, i: usize) -> f32 {
    let base = (t * ZZZ_RATE_HZ).rem_euclid(1.0);
    (base + i as f64 / ZZZ_COUNT as f64).rem_euclid(1.0) as f32
}

/// Alpha 0.0..=1.0 for a glyph phase in 0.0..1.0: fades in, peaks at 0.5, fades out.
pub fn zzz_alpha(phase: f32) -> f32 {
    if !phase.is_finite() {
        return 0.0;
    }
    (PI as f32 * phase.clamp(0.0, 1.0)).sin().clamp(0.0, 1.0)
}

/// Three "Z" glyphs rising from `at`, growing and fading out in turn.
pub fn paint_zzz(p: &Painter, at: Pos2, t: f64, color: Color32) {
    if !t.is_finite() || !at.x.is_finite() || !at.y.is_finite() {
        return;
    }
    for i in 0..ZZZ_COUNT {
        let phase = zzz_phase(t, i);
        let alpha = zzz_alpha(phase);
        if alpha <= 0.0 {
            continue;
        }
        let size = 8.0 + 6.0 * phase;
        let pos = Pos2::new(
            at.x + i as f32 * 6.0 + 3.0 * phase,
            at.y - ZZZ_RISE_PX * phase,
        );
        let a = (color.a() as f32 * alpha).round() as u8;
        let c = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a);
        p.text(pos, Align2::LEFT_BOTTOM, "Z", FontId::proportional(size), c);
    }
}

/// Creature state driving the animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxState {
    Idle,
    Busy,
    Sleeping,
    Done,
}

/// Per-frame offsets for a state at time `t`: (scale, y offset px, zzz visible).
/// Idle breathes, Busy bounces, Sleeping shows zzz (scale at rest), Done is at rest.
pub fn state_offsets(state: FxState, t: f64) -> (f32, f32, bool) {
    match state {
        FxState::Idle => (breathe_scale(t), 0.0, false),
        FxState::Busy => (1.0, busy_bounce(t), false),
        FxState::Sleeping => (1.0, 0.0, true),
        FxState::Done => (1.0, 0.0, false),
    }
}

/// True when the caller should request continuous repaints. Idle requests none
/// (breathing advances only on frames drawn for other reasons).
pub fn needs_repaint(state: FxState) -> bool {
    !matches!(state, FxState::Idle)
}

/// Tick mark inside a square of side `size` whose top-left is `at`.
/// Non-finite or non-positive `size` draws nothing.
pub fn paint_done_check(p: &Painter, at: Pos2, size: f32, color: Color32) {
    if !size.is_finite() || size <= 0.0 || !at.x.is_finite() || !at.y.is_finite() {
        return;
    }
    let a = Pos2::new(at.x + size * 0.2, at.y + size * 0.5);
    let b = Pos2::new(at.x + size * 0.42, at.y + size * 0.72);
    let c = Pos2::new(at.x + size * 0.8, at.y + size * 0.28);
    let stroke = Stroke::new((size * 0.12).clamp(1.0, 4.0), color);
    p.line_segment([a, b], stroke);
    p.line_segment([b, c], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sweep() -> impl Iterator<Item = f64> {
        (-2000..=2000).map(|i| i as f64 * 0.0137)
    }

    #[test]
    fn breathe_scale_is_bounded_1_to_1_04() {
        for t in sweep() {
            let s = breathe_scale(t);
            assert!((1.0 - 1e-6..=1.04 + 1e-6).contains(&s), "t={t} s={s}");
        }
    }

    #[test]
    fn breathe_scale_reaches_both_ends() {
        let max = sweep().map(breathe_scale).fold(f32::MIN, f32::max);
        let min = sweep().map(breathe_scale).fold(f32::MAX, f32::min);
        assert!((max - 1.04).abs() < 1e-3, "max={max}");
        assert!((min - 1.0).abs() < 1e-3, "min={min}");
    }

    #[test]
    fn breathe_scale_period_is_2_5_seconds() {
        for t in sweep() {
            assert!((breathe_scale(t) - breathe_scale(t + 2.5)).abs() < 1e-4, "t={t}");
        }
    }

    #[test]
    fn breathe_scale_non_finite_is_rest() {
        assert_eq!(breathe_scale(f64::NAN), 1.0);
        assert_eq!(breathe_scale(f64::INFINITY), 1.0);
    }

    #[test]
    fn busy_bounce_is_bounded_pm_2px() {
        for t in sweep() {
            let o = busy_bounce(t);
            assert!(o.abs() <= 2.0 + 1e-6, "t={t} o={o}");
        }
    }

    #[test]
    fn busy_bounce_period_is_0_4_seconds() {
        for t in sweep() {
            assert!((busy_bounce(t) - busy_bounce(t + 0.4)).abs() < 1e-4, "t={t}");
        }
    }

    #[test]
    fn busy_bounce_non_finite_is_zero() {
        assert_eq!(busy_bounce(f64::NAN), 0.0);
        assert_eq!(busy_bounce(f64::NEG_INFINITY), 0.0);
    }

    #[test]
    fn zzz_alpha_is_within_0_1() {
        for i in -200..=1200 {
            let a = zzz_alpha(i as f32 / 1000.0);
            assert!((0.0..=1.0).contains(&a), "a={a}");
        }
        assert_eq!(zzz_alpha(f32::NAN), 0.0);
    }

    #[test]
    fn zzz_phase_is_within_0_1_and_periodic_every_2s() {
        for t in sweep() {
            for i in 0..ZZZ_COUNT {
                let p = zzz_phase(t, i);
                assert!((0.0..1.0).contains(&p), "t={t} p={p}");
                let q = zzz_phase(t + 2.0, i);
                assert!((p - q).abs() < 1e-3 || (p - q).abs() > 1.0 - 1e-3, "t={t} p={p} q={q}");
            }
        }
    }

    #[test]
    fn state_offsets_per_state() {
        let (s, y, z) = state_offsets(FxState::Idle, 0.0);
        assert!((s - breathe_scale(0.0)).abs() < 1e-6 && y == 0.0 && !z);
        let (s, y, z) = state_offsets(FxState::Busy, 0.1);
        assert!(s == 1.0 && y.abs() <= 2.0 && !z);
        let (s, y, z) = state_offsets(FxState::Sleeping, 3.0);
        assert!(s == 1.0 && y == 0.0 && z);
        let (s, y, z) = state_offsets(FxState::Done, 3.0);
        assert!(s == 1.0 && y == 0.0 && !z);
    }

    #[test]
    fn only_idle_skips_repaint() {
        assert!(!needs_repaint(FxState::Idle));
        assert!(needs_repaint(FxState::Busy));
        assert!(needs_repaint(FxState::Sleeping));
        assert!(needs_repaint(FxState::Done));
    }

    #[test]
    fn zzz_alpha_fades_at_cycle_edges() {
        assert!(zzz_alpha(0.0) < 1e-6);
        assert!(zzz_alpha(1.0) < 1e-6);
        assert!((zzz_alpha(0.5) - 1.0).abs() < 1e-6);
    }
}
