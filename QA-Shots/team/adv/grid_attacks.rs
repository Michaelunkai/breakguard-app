// AdversarialQA scratch attacks on src/ui/logic/grid.rs (read-only include via #[path]).
#[path = "F:/study/projects/breakguard/src/ui/logic/grid.rs"]
#[allow(dead_code)]
mod grid;
use grid::*;

fn hit(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

// A1: n=0 yields 0 columns, lane_rects(0) must not panic
#[test]
fn a01_zero_lanes() {
    assert_eq!(columns_for(0, 1000.0, 420.0, 8.0), 0);
    assert!(lane_rects(0, 1000.0, 8.0, &[]).is_empty());
}

// A2: degenerate widths (0, -1, NaN, inf, -inf) -> 1 column, never panic
#[test]
fn a02_degenerate_width() {
    for w in [0.0f32, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MIN_POSITIVE] {
        assert_eq!(columns_for(5, w, 420.0, 8.0), 1, "w={w}");
    }
}

// A3: huge width 1e30 with 30 sessions -> clamps to 30, rects finite and disjoint
#[test]
fn a03_huge_width_30_sessions() {
    let c = columns_for(30, 1e30, 420.0, 8.0);
    assert_eq!(c, 30);
    let hs = vec![100.0f32; 30];
    let r = lane_rects(c, 1e30, 8.0, &hs);
    for &(x, y, w, h) in &r {
        assert!(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite());
    }
    for i in 0..30 { for j in i + 1..30 { assert!(!hit(r[i], r[j]), "{i} vs {j}"); } }
}

// A4: min_w 0 / negative / NaN -> treated as 1.0, no div-by-zero, c in 1..=n
#[test]
fn a04_bad_min_width() {
    for m in [0.0f32, -5.0, f32::NAN, f32::INFINITY] {
        let c = columns_for(30, 1300.0, m, 8.0);
        assert!((1..=30).contains(&c), "m={m} c={c}");
    }
}

// A5: gap NaN / inf / negative -> no panic, c in range, rects inside avail_w
#[test]
fn a05_bad_gap_rects_inside() {
    for g in [f32::NAN, f32::INFINITY, -8.0, 1e30] {
        let c = columns_for(6, 1300.0, 420.0, g);
        assert!((1..=6).contains(&c), "g={g} c={c}");
        let r = lane_rects(c, 1300.0, g, &vec![50.0; 6]);
        for &(x, _, w, _) in &r {
            assert!(x.is_finite() && w.is_finite() && w >= 0.0, "g={g} x={x} w={w}");
        }
    }
}

// A6: columns_for must never return a count whose min-width footprint exceeds avail_w (float boundary sweep)
#[test]
fn a06_fit_contract_float_sweep() {
    let mut w = 0.0f32;
    while w < 14000.0 {
        for n in 1..=30usize {
            let c = columns_for(n, w, 420.0, 8.0);
            if c > 1 {
                assert!(c as f32 * 420.0 + (c - 1) as f32 * 8.0 <= w + 0.5, "w={w} n={n} c={c}");
            }
        }
        w += 0.37;
    }
}

// A7: heights with NaN/neg/inf entries -> rects have finite non-negative h; y never decreases; (inf y is a finding)
#[test]
fn a07_bad_heights() {
    let hs = [f32::NAN, -5.0, f32::INFINITY, 0.0, 80.0, f32::NEG_INFINITY];
    let r = lane_rects(2, 900.0, 8.0, &hs);
    assert_eq!(r.len(), 6);
    for &(_, y, _, h) in &r {
        assert!(h.is_finite() && h >= 0.0, "h={h}");
        assert!(y.is_finite(), "y={y} (non-finite row offset)");
    }
}

// A8: f32::MAX heights in two rows -> row y overflows to inf (expected: finite or clamped)
#[test]
fn a08_max_heights_overflow() {
    let r = lane_rects(1, 900.0, 8.0, &[f32::MAX, f32::MAX, f32::MAX]);
    for &(_, y, _, _) in &r {
        assert!(y.is_finite(), "y={y} overflow to non-finite");
    }
}

// A9: avail_w smaller than total gaps -> col_w 0, x finite, no panic
#[test]
fn a09_gaps_exceed_width() {
    let r = lane_rects(30, 10.0, 8.0, &vec![40.0; 30]);
    assert!(r.iter().all(|&(x, _, w, _)| x.is_finite() && w >= 0.0));
}

// A10: cols = usize::MAX and cols = 0 -> no panic, no OOM
#[test]
fn a10_extreme_cols() {
    let _ = lane_rects(usize::MAX, 1300.0, 8.0, &[10.0, 20.0]);
    let r = lane_rects(0, 1300.0, 8.0, &[10.0, 20.0]);
    assert_eq!(r.len(), 2);
}

// A11: 30 lanes uneven heights, 3 cols: rect count matches and row y is shared by row members
#[test]
fn a11_thirty_lanes_rows_aligned() {
    let hs: Vec<f32> = (0..30).map(|i| 50.0 + (i % 7) as f32 * 13.0).collect();
    let r = lane_rects(3, 1300.0, 8.0, &hs);
    assert_eq!(r.len(), 30);
    for row in 0..10 {
        let ys: Vec<f32> = (0..3).map(|k| r[row * 3 + k].1).collect();
        assert!(ys.iter().all(|&y| (y - ys[0]).abs() < 1e-4), "row {row} ys={ys:?}");
    }
}

// A12: lane A (row 0) grows: lanes in OTHER columns of row 0 keep x and width; y of later rows shifts (design check)
#[test]
fn a12_grow_lane_isolation() {
    let mut hs = vec![100.0f32; 4];
    let before = lane_rects(2, 1300.0, 8.0, &hs);
    hs[0] = 400.0;
    let after = lane_rects(2, 1300.0, 8.0, &hs);
    assert_eq!(before[1], after[1], "sibling in same row must not change");
    assert_eq!(before[0].0, after[0].0);
}

// A13: tiny avail (subnormal) and width exactly equal to one column -> 1 col
#[test]
fn a13_boundary_exact_one_col() {
    assert_eq!(columns_for(2, 420.0, 420.0, 8.0), 1);
    assert_eq!(columns_for(2, 848.0, 420.0, 8.0), 2);
    assert_eq!(columns_for(2, 847.999, 420.0, 8.0), 1);
    assert_eq!(columns_for(2, f32::MIN_POSITIVE, 420.0, 8.0), 1);
}

// A14: visible_range on flat heights from row layout must not panic with NaN scroll and 1e9 viewport
#[test]
fn a14_visible_range_extreme() {
    let h = vec![100.0f32; 30];
    let r = visible_range(f32::NAN, 1e9, &h, 8.0);
    assert!(r.end <= 30);
    let r2 = visible_range(-1e30, f32::INFINITY, &h, 8.0);
    assert!(r2.end <= 30 && r2.start <= r2.end);
}

// A15: 30 sessions, width sweep 1..3000 step 1: lane_rects never overlaps and stays in [0, avail_w]
#[test]
fn a15_thirty_sessions_width_sweep() {
    let hs = vec![60.0f32; 30];
    let mut w = 1.0f32;
    while w <= 3000.0 {
        let c = columns_for(30, w, 420.0, 8.0);
        let r = lane_rects(c, w, 8.0, &hs);
        for &(x, _, cw, _) in &r {
            assert!(x >= 0.0 && x + cw <= w + 0.01, "w={w} c={c} x={x} cw={cw}");
        }
        for i in 0..30 { for j in i + 1..30 { assert!(!hit(r[i], r[j]), "w={w} {i} vs {j}"); } }
        w += 1.0;
    }
}
