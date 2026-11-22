#![allow(dead_code)]
use std::ops::Range;

fn nz(x: f32) -> f32 {
    if x.is_finite() && x > 0.0 { x } else { 0.0 }
}

/// Lane heights: fit viewport (default_h, or shrunk equally, never below min_h);
/// otherwise every lane is min_h and the page scrolls. NaN/negative inputs clamp to 0.
pub fn lane_heights(n: usize, viewport_h: f32, min_h: f32, default_h: f32, gap: f32) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    let min_h = nz(min_h);
    let default_h = if default_h.is_finite() { default_h.max(min_h) } else { min_h };
    let gaps = nz(gap) * (n - 1) as f32;
    let per = (nz(viewport_h) - gaps) / n as f32;
    let h = if per.is_finite() { per.clamp(min_h, default_h) } else { min_h };
    vec![h; n]
}

/// Finite, non-negative lane height capped at 1e6 pt so row offsets can never overflow to inf.
fn clamp_h(x: f32) -> f32 {
    nz(x).min(1.0e6)
}

/// Side-by-side columns for n lanes: as many as fit at min_w each (with gap), at least 1, at most n.
pub fn columns_for(n: usize, avail_w: f32, min_w: f32, gap: f32) -> usize {
    if n == 0 {
        return 0;
    }
    let (w, m, g) = (nz(avail_w), nz(min_w).max(1.0), nz(gap));
    let fit = ((w + g) / (m + g)).floor();
    let fit = if fit.is_finite() && fit >= 1.0 { fit as usize } else { 1 };
    fit.clamp(1, n)
}

/// Lane rects (x, y, w, h) relative to the page origin: `cols` equal columns separated by `gap`,
/// rows wrap left to right, each row as tall as its tallest lane. Rects never intersect.
pub fn lane_rects(cols: usize, avail_w: f32, gap: f32, heights: &[f32]) -> Vec<(f32, f32, f32, f32)> {
    let cols = cols.max(1);
    let g = nz(gap);
    let col_w = ((nz(avail_w) - g * (cols - 1) as f32) / cols as f32).max(0.0);
    let mut out = Vec::with_capacity(heights.len());
    let mut y = 0.0f32;
    for row in heights.chunks(cols) {
        let rh = row.iter().fold(0.0f32, |a, &h| a.max(clamp_h(h)));
        for (i, &h) in row.iter().enumerate() {
            out.push((i as f32 * (col_w + g), y, col_w, clamp_h(h)));
        }
        y += rh + g;
    }
    out
}

#[cfg(test)]
mod side_by_side_tests {
    use super::*;

    fn hit(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    }

    #[test]
    fn grid_columns_for_widths() {
        for n in 1..=6usize {
            for w in (400..=3000).step_by(100) {
                let c = columns_for(n, w as f32, 420.0, 8.0);
                assert!(c >= 1 && c <= n, "n={n} w={w} c={c}");
                if c > 1 {
                    assert!(c as f32 * 420.0 + (c - 1) as f32 * 8.0 <= w as f32 + 0.01);
                }
            }
        }
        assert_eq!(columns_for(0, 1000.0, 420.0, 8.0), 0);
        assert_eq!(columns_for(2, 1200.0, 420.0, 8.0), 2);
        assert_eq!(columns_for(2, 700.0, 420.0, 8.0), 1);
        assert_eq!(columns_for(3, f32::NAN, 420.0, 8.0), 1);
    }

    #[test]
    fn lane_rects_never_overlap() {
        for n in 1..=8usize {
            for w in [400.0f32, 900.0, 1300.0, 2400.0] {
                let cols = columns_for(n, w, 420.0, 8.0);
                let hs: Vec<f32> = (0..n).map(|i| 100.0 + 37.0 * i as f32).collect();
                let r = lane_rects(cols, w, 8.0, &hs);
                assert_eq!(r.len(), n);
                for i in 0..n {
                    for j in i + 1..n {
                        assert!(!hit(r[i], r[j]), "n={n} w={w} {i} vs {j}");
                    }
                }
            }
        }
    }

    #[test]
    fn lane_rects_huge_heights_stay_finite() {
        let r = lane_rects(1, 900.0, 8.0, &[f32::MAX; 3]);
        assert_eq!(r.len(), 3);
        for &(x, y, w, h) in &r {
            assert!(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite(), "{:?}", (x, y, w, h));
        }
    }

    #[test]
    fn lane_rects_inside_bounds() {
        let r = lane_rects(3, 1300.0, 10.0, &[100.0, 120.0, 90.0, 80.0, 70.0]);
        for &(x, _, w, _) in &r {
            assert!(x >= 0.0 && x + w <= 1300.0 + 0.01);
        }
        assert!((r[1].0 - (r[0].0 + r[0].2) - 10.0).abs() < 0.01);
        assert!((r[3].1 - (120.0 + 10.0)).abs() < 0.01);
    }
}

/// Lanes intersecting [scroll_top, scroll_top+viewport_h). Contiguous range; empty if none.
pub fn visible_range(scroll_top: f32, viewport_h: f32, heights: &[f32], gap: f32) -> Range<usize> {
    let gap = nz(gap);
    let top = if scroll_top.is_finite() { scroll_top } else { 0.0 };
    let bottom = top + nz(viewport_h);
    let mut start = None;
    let mut end = 0;
    let mut y = 0.0f32;
    for (i, &h) in heights.iter().enumerate() {
        let h = nz(h);
        let (a, b) = (y, y + h);
        if a >= bottom {
            break;
        }
        if b > top && h > 0.0 {
            if start.is_none() {
                start = Some(i);
            }
            end = i + 1;
        }
        y = b + gap;
    }
    match start {
        Some(s) => s..end,
        None => 0..0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_lanes() {
        assert!(lane_heights(0, 500.0, 50.0, 100.0, 4.0).is_empty());
    }
    #[test]
    fn one_lane_default() {
        assert_eq!(lane_heights(1, 500.0, 50.0, 100.0, 4.0), vec![100.0]);
    }
    #[test]
    fn three_default_fit() {
        assert_eq!(lane_heights(3, 500.0, 50.0, 100.0, 4.0), vec![100.0; 3]);
    }
    #[test]
    fn three_shrunk() {
        let h = lane_heights(3, 200.0, 20.0, 100.0, 10.0);
        assert_eq!(h, vec![60.0; 3]); // (200-20)/3
    }
    #[test]
    fn twenty_scroll() {
        assert_eq!(lane_heights(20, 600.0, 40.0, 100.0, 4.0), vec![40.0; 20]);
    }
    #[test]
    fn hundred_scroll() {
        let h = lane_heights(100, 600.0, 40.0, 100.0, 4.0);
        assert_eq!(h.len(), 100);
        assert!(h.iter().all(|&x| x == 40.0));
    }
    #[test]
    fn exact_fit_boundary() {
        assert_eq!(lane_heights(2, 104.0, 10.0, 50.0, 4.0), vec![50.0; 2]);
    }
    #[test]
    fn adversarial_inputs() {
        let nan = f32::NAN;
        for v in [nan, -5.0, f32::INFINITY, f32::NEG_INFINITY, 0.0] {
            for g in [nan, -1.0, f32::INFINITY, 0.0] {
                let h = lane_heights(5, v, 30.0, 80.0, g);
                assert_eq!(h.len(), 5);
                assert!(h.iter().all(|x| x.is_finite() && *x >= 30.0 && *x <= 80.0));
            }
        }
        assert_eq!(lane_heights(2, 100.0, nan, nan, nan), vec![0.0, 0.0]);
        assert_eq!(lane_heights(2, 100.0, -3.0, 50.0, 0.0), vec![50.0, 50.0]);
        // default below min -> min wins
        assert_eq!(lane_heights(2, 1000.0, 60.0, 10.0, 0.0), vec![60.0, 60.0]);
        // huge n no overflow/panic
        assert_eq!(lane_heights(1_000_000, 600.0, 1.0, 100.0, 1.0).len(), 1_000_000);
    }

    // 5 lanes of 100, gap 10: tops 0,110,220,330,440
    fn five() -> Vec<f32> {
        vec![100.0; 5]
    }
    #[test]
    fn range_top() {
        assert_eq!(visible_range(0.0, 150.0, &five(), 10.0), 0..2);
    }
    #[test]
    fn range_edges_exclusive() {
        // viewport ends exactly at lane1 top (110): lane1 excluded
        assert_eq!(visible_range(0.0, 110.0, &five(), 10.0), 0..1);
        // viewport starts exactly at lane0 bottom (100): lane0 excluded
        assert_eq!(visible_range(100.0, 50.0, &five(), 10.0), 1..2);
        // in the gap only
        assert_eq!(visible_range(100.0, 10.0, &five(), 10.0), 0..0);
    }
    #[test]
    fn range_middle_and_end() {
        assert_eq!(visible_range(250.0, 100.0, &five(), 10.0), 2..4);
        assert_eq!(visible_range(10_000.0, 100.0, &five(), 10.0), 0..0);
        assert_eq!(visible_range(500.0, 500.0, &five(), 10.0), 4..5);
    }
    #[test]
    fn range_negative_scroll() {
        assert_eq!(visible_range(-50.0, 100.0, &five(), 10.0), 0..1);
        assert_eq!(visible_range(-500.0, 100.0, &five(), 10.0), 0..0);
    }
    #[test]
    fn range_empty_and_degenerate() {
        assert_eq!(visible_range(0.0, 100.0, &[], 10.0), 0..0);
        assert_eq!(visible_range(0.0, 0.0, &five(), 10.0), 0..0);
        assert_eq!(visible_range(0.0, -10.0, &five(), 10.0), 0..0);
        assert_eq!(visible_range(f32::NAN, 150.0, &five(), 10.0), 0..2);
        assert_eq!(visible_range(0.0, f32::NAN, &five(), 10.0), 0..0);
        assert_eq!(visible_range(0.0, 150.0, &five(), f32::NAN), 0..2);
        assert_eq!(visible_range(0.0, f32::INFINITY, &five(), 10.0), 0..0);
    }
    #[test]
    fn range_hundred_lanes_scrolling() {
        let h = lane_heights(100, 600.0, 40.0, 100.0, 4.0); // pitch 44
        for s in [0.0, 43.0, 44.0, 2200.0, 4359.0, 4400.0] {
            let r = visible_range(s, 600.0, &h, 4.0);
            assert!(r.end <= 100 && r.start <= r.end);
            // brute-force check
            for i in 0..100 {
                let a = i as f32 * 44.0;
                let hit = a < s + 600.0 && a + 40.0 > s;
                assert_eq!(r.contains(&i), hit, "s={s} i={i}");
            }
        }
        // virtualization: far fewer than 100 rendered
        assert!(visible_range(2200.0, 600.0, &h, 4.0).len() <= 15);
    }
    #[test]
    fn range_twenty_lanes_end() {
        let h = lane_heights(20, 600.0, 40.0, 100.0, 4.0);
        let total = 20.0 * 40.0 + 19.0 * 4.0;
        assert_eq!(visible_range(total - 1.0, 600.0, &h, 4.0), 19..20);
        assert_eq!(visible_range(total, 600.0, &h, 4.0), 0..0);
    }
    #[test]
    fn range_bad_heights() {
        let h = [f32::NAN, -5.0, 100.0, 0.0, 100.0];
        assert_eq!(visible_range(0.0, 1000.0, &h, 0.0), 2..5);
    }
    #[test]
    fn range_single_lane() {
        let h = lane_heights(1, 500.0, 50.0, 100.0, 4.0);
        assert_eq!(visible_range(0.0, 500.0, &h, 4.0), 0..1);
        assert_eq!(visible_range(99.0, 500.0, &h, 4.0), 0..1);
        assert_eq!(visible_range(100.0, 500.0, &h, 4.0), 0..0);
    }
}
