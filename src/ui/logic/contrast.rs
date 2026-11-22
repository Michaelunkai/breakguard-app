#![allow(dead_code)]
//! WCAG 2.x contrast helpers. std only.

type Rgb = (u8, u8, u8);

fn channel(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.03928 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance, 0.0 (black) to 1.0 (white).
pub fn luminance(rgb: Rgb) -> f64 {
    0.2126 * channel(rgb.0) + 0.7152 * channel(rgb.1) + 0.0722 * channel(rgb.2)
}

/// Contrast ratio, 1.0..=21.0, symmetric.
pub fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// WCAG AA for normal text: ratio >= 4.5.
pub fn passes_aa(fg: Rgb, bg: Rgb) -> bool {
    ratio(fg, bg) >= 4.5
}

#[cfg(test)]
mod tests {
    use super::*;
    const W: Rgb = (255, 255, 255);
    const K: Rgb = (0, 0, 0);

    #[test]
    fn black_white() {
        assert!((ratio(K, W) - 21.0).abs() < 0.01);
    }
    #[test]
    fn identical() {
        assert_eq!(ratio((12, 200, 7), (12, 200, 7)), 1.0);
        assert_eq!(ratio(W, W), 1.0);
    }
    #[test]
    fn symmetric() {
        let (a, b) = ((10, 20, 30), (200, 180, 90));
        assert_eq!(ratio(a, b), ratio(b, a));
    }
    #[test]
    fn grey_767676_passes() {
        let r = ratio((0x76, 0x76, 0x76), W);
        assert!((r - 4.54).abs() < 0.01, "{r}");
        assert!(passes_aa((0x76, 0x76, 0x76), W));
    }
    #[test]
    fn grey_777777_fails() {
        assert!(!passes_aa((0x77, 0x77, 0x77), W));
    }
    #[test]
    fn luminance_bounds() {
        assert_eq!(luminance(K), 0.0);
        assert!((luminance(W) - 1.0).abs() < 1e-9);
    }
    #[test]
    fn ratio_in_range_exhaustive_grid() {
        let v = [0u8, 1, 10, 11, 127, 128, 254, 255];
        for &r in &v {
            for &g in &v {
                for &b in &v {
                    for &x in &v {
                        let q = ratio((r, g, b), (x, x, x));
                        assert!((1.0..=21.0 + 1e-9).contains(&q) && q.is_finite());
                    }
                }
            }
        }
    }
    #[test]
    fn symmetric_pass_both_ways_on_extremes() {
        assert!(passes_aa(K, W) && passes_aa(W, K));
        assert!(!passes_aa(K, K));
    }
    #[test]
    fn threshold_step_near_linearization_knee() {
        // 10 -> 0.0392 (linear branch), 11 -> 0.0431 (power branch): monotonic
        assert!(luminance((10, 10, 10)) < luminance((11, 11, 11)));
    }
}
