#![allow(dead_code)]
pub const MIN_LANE_H: f32 = 66.0;
pub const MAX_LANE_H: f32 = 1600.0;
const DEFAULT_LANE_H: f32 = 220.0;

/// Clamp to [MIN_LANE_H, MAX_LANE_H]; NaN -> 220.0.
pub fn clamp_height(h: f32) -> f32 {
    if h.is_nan() {
        return DEFAULT_LANE_H;
    }
    h.clamp(MIN_LANE_H, MAX_LANE_H)
}

/// start + drag_dy, clamped. NaN start -> default base; NaN dy -> ignored.
pub fn drag_height(start: f32, drag_dy: f32) -> f32 {
    let s = if start.is_nan() { DEFAULT_LANE_H } else { start };
    let d = if drag_dy.is_nan() { 0.0 } else { drag_dy };
    // inf + -inf = NaN -> default via clamp_height
    clamp_height(s + d)
}

fn main() {
    println!("{}", clamp_height(100.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds() {
        assert_eq!(clamp_height(66.0), 66.0);
        assert_eq!(clamp_height(1600.0), 1600.0);
        assert_eq!(clamp_height(65.999), 66.0);
        assert_eq!(clamp_height(1600.1), 1600.0);
        assert_eq!(clamp_height(300.5), 300.5);
    }
    #[test]
    fn nan() {
        assert_eq!(clamp_height(f32::NAN), 220.0);
        assert_eq!(drag_height(f32::NAN, 10.0), 230.0);
        assert_eq!(drag_height(100.0, f32::NAN), 100.0);
        assert_eq!(drag_height(f32::NAN, f32::NAN), 220.0);
    }
    #[test]
    fn inf() {
        assert_eq!(clamp_height(f32::INFINITY), 1600.0);
        assert_eq!(clamp_height(f32::NEG_INFINITY), 66.0);
        assert_eq!(drag_height(100.0, f32::INFINITY), 1600.0);
        assert_eq!(drag_height(100.0, f32::NEG_INFINITY), 66.0);
        assert_eq!(drag_height(f32::INFINITY, f32::NEG_INFINITY), 220.0);
    }
    #[test]
    fn negative_zero_huge() {
        assert_eq!(clamp_height(-5.0), 66.0);
        assert_eq!(clamp_height(0.0), 66.0);
        assert_eq!(clamp_height(-0.0), 66.0);
        assert_eq!(clamp_height(f32::MAX), 1600.0);
        assert_eq!(clamp_height(f32::MIN), 66.0);
        assert_eq!(drag_height(0.0, 0.0), 66.0);
        assert_eq!(drag_height(f32::MAX, f32::MAX), 1600.0);
    }
    #[test]
    fn drag() {
        assert_eq!(drag_height(200.0, 50.0), 250.0);
        assert_eq!(drag_height(200.0, -50.0), 150.0);
        assert_eq!(drag_height(200.0, -500.0), 66.0);
        assert_eq!(drag_height(1300.0, 500.0), 1600.0);
        assert_eq!(drag_height(200.0, 0.0), 200.0);
    }
    #[test]
    fn result_always_in_range() {
        let vals = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1e30, 1e30, 66.0, 1600.0, 1.5];
        for &a in &vals {
            for &b in &vals {
                let r = drag_height(a, b);
                assert!((MIN_LANE_H..=MAX_LANE_H).contains(&r), "{a} {b} -> {r}");
            }
        }
    }
}
