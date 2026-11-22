#![allow(dead_code)]
//! Ring: pure logic for a countdown ring (std only).

use std::f64::consts::PI;

/// Fraction of time left, clamped to 0..=1. total<=0 -> 0.
pub fn fraction_left(remaining_s: i64, total_s: i64) -> f32 {
    if total_s <= 0 || remaining_s <= 0 {
        return 0.0;
    }
    if remaining_s >= total_s {
        return 1.0;
    }
    (remaining_s as f64 / total_s as f64) as f32
}

const MAX_SEGMENTS: usize = 1024;

/// Points of an arc from 12 o'clock, clockwise (screen y down).
/// fraction<=0 or NaN, or non-finite radius -> empty. fraction>=1 -> closed full circle.
/// Result has clamp(segments, 1, 1024)+1 points (min 2).
pub fn arc_points(center: (f32, f32), radius: f32, fraction: f32, segments: usize) -> Vec<(f32, f32)> {
    if fraction.is_nan() || fraction <= 0.0 || !radius.is_finite() {
        return Vec::new();
    }
    let f = fraction.min(1.0) as f64;
    let n = segments.clamp(1, MAX_SEGMENTS);
    let (cx, cy, r) = (center.0 as f64, center.1 as f64, radius as f64);
    let mut pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let a = -PI / 2.0 + 2.0 * PI * f * (i as f64 / n as f64);
            ((cx + r * a.cos()) as f32, (cy + r * a.sin()) as f32)
        })
        .collect();
    if f >= 1.0 {
        let first = pts[0];
        *pts.last_mut().unwrap() = first;
    }
    pts
}

/// Format seconds as MM:SS. Negative -> "00:00". Minutes may exceed 59.
pub fn mmss(s: i64) -> String {
    let s = s.max(0);
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn fraction_basic() {
        assert_eq!(fraction_left(30, 60), 0.5);
        assert_eq!(fraction_left(0, 60), 0.0);
        assert_eq!(fraction_left(60, 60), 1.0);
    }

    #[test]
    fn fraction_edges() {
        assert_eq!(fraction_left(10, 0), 0.0);
        assert_eq!(fraction_left(10, -5), 0.0);
        assert_eq!(fraction_left(100, 60), 1.0);
        assert_eq!(fraction_left(-5, 60), 0.0);
        assert_eq!(fraction_left(i64::MAX, i64::MAX), 1.0);
        assert_eq!(fraction_left(i64::MIN, i64::MIN), 0.0);
        assert_eq!(fraction_left(i64::MIN, i64::MAX), 0.0);
        let f = fraction_left(i64::MAX - 1, i64::MAX);
        assert!((0.0..=1.0).contains(&f));
    }

    #[test]
    fn arc_empty() {
        assert!(arc_points((0.0, 0.0), 10.0, 0.0, 16).is_empty());
        assert!(arc_points((0.0, 0.0), 10.0, -1.0, 16).is_empty());
        assert!(arc_points((0.0, 0.0), 10.0, f32::NAN, 16).is_empty());
    }

    #[test]
    fn arc_starts_at_top() {
        let p = arc_points((5.0, 7.0), 10.0, 0.5, 8);
        assert!(close(p[0], (5.0, -3.0)));
    }

    #[test]
    fn arc_quarter_turn() {
        let p = arc_points((0.0, 0.0), 10.0, 0.25, 4);
        assert_eq!(p.len(), 5);
        assert!(close(p[0], (0.0, -10.0)));
        assert!(close(*p.last().unwrap(), (10.0, 0.0)));
    }

    #[test]
    fn arc_half_and_three_quarter() {
        let p = arc_points((0.0, 0.0), 10.0, 0.5, 8);
        assert!(close(*p.last().unwrap(), (0.0, 10.0)));
        let p = arc_points((0.0, 0.0), 10.0, 0.75, 12);
        assert!(close(*p.last().unwrap(), (-10.0, 0.0)));
    }

    #[test]
    fn arc_full_closed() {
        let p = arc_points((100.0, 100.0), 1000.0, 1.0, 64);
        assert_eq!(p.len(), 65);
        assert!(close(p[0], *p.last().unwrap()));
        let p = arc_points((0.0, 0.0), 10.0, 7.0, 8);
        assert!(close(p[0], *p.last().unwrap()));
    }

    #[test]
    fn arc_min_points() {
        assert_eq!(arc_points((0.0, 0.0), 1.0, 0.5, 0).len(), 2);
        assert_eq!(arc_points((0.0, 0.0), 1.0, 0.5, 1).len(), 2);
        assert_eq!(arc_points((0.0, 0.0), 1.0, 1.0, 0).len(), 2);
    }

    #[test]
    fn arc_all_on_circle() {
        for (x, y) in arc_points((3.0, 4.0), 25.0, 0.37, 50) {
            let d = ((x - 3.0).powi(2) + (y - 4.0).powi(2)).sqrt();
            assert!((d - 25.0).abs() < 1e-3);
        }
    }

    #[test]
    fn arc_zero_radius_and_inf() {
        let p = arc_points((1.0, 2.0), 0.0, 1.0, 4);
        assert!(p.iter().all(|&q| close(q, (1.0, 2.0))));
        assert_eq!(arc_points((0.0, 0.0), 1.0, f32::INFINITY, 4).len(), 5);
        assert!(arc_points((0.0, 0.0), 1.0, f32::NEG_INFINITY, 4).is_empty());
        assert_eq!(arc_points((0.0, 0.0), 1.0, 0.5, 100_000).len(), 1025);
        assert_eq!(arc_points((0.0, 0.0), 1.0, 0.5, usize::MAX).len(), 1025);
    }

    #[test]
    fn arc_nan_radius_is_empty() {
        assert!(arc_points((0.0, 0.0), f32::NAN, 1.0, 16).is_empty());
        assert!(arc_points((0.0, 0.0), f32::INFINITY, 1.0, 16).is_empty());
    }

    #[test]
    fn mmss_boundaries() {
        assert_eq!(mmss(0), "00:00");
        assert_eq!(mmss(-1), "00:00");
        assert_eq!(mmss(i64::MIN), "00:00");
        assert_eq!(mmss(59), "00:59");
        assert_eq!(mmss(60), "01:00");
        assert_eq!(mmss(3599), "59:59");
        assert_eq!(mmss(3600), "60:00");
        assert_eq!(mmss(6000), "100:00");
        assert_eq!(mmss(i64::MAX), format!("{}:07", i64::MAX / 60));
    }
}
