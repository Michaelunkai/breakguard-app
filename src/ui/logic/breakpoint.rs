#![allow(dead_code)]
//! Width breakpoints for lane rows. std-only.

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Width {
    Narrow,
    Normal,
    Wide,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Parts {
    pub show_sparkline: bool,
    pub show_counters: bool,
    pub show_uptime: bool,
    pub show_state_text: bool,
    pub tag_w: f32,
}

/// <640 Narrow, <1000 Normal, else Wide (a full header row needs about 650 points). NaN -> Normal.
pub fn classify(w: f32) -> Width {
    if w.is_nan() {
        Width::Normal
    } else if w < 640.0 {
        Width::Narrow
    } else if w < 1000.0 {
        Width::Normal
    } else {
        Width::Wide
    }
}

pub fn parts(w: Width) -> Parts {
    match w {
        Width::Narrow => Parts {
            show_sparkline: false,
            show_counters: false,
            show_uptime: false,
            show_state_text: false,
            tag_w: 0.0,
        },
        Width::Normal => Parts {
            show_sparkline: false,
            show_counters: true,
            show_uptime: true,
            show_state_text: true,
            tag_w: 72.0,
        },
        Width::Wide => Parts {
            show_sparkline: true,
            show_counters: true,
            show_uptime: true,
            show_state_text: true,
            tag_w: 96.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        assert_eq!(classify(639.0), Width::Narrow);
        assert_eq!(classify(639.999), Width::Narrow);
        assert_eq!(classify(640.0), Width::Normal);
        assert_eq!(classify(999.0), Width::Normal);
        assert_eq!(classify(999.999), Width::Normal);
        assert_eq!(classify(1000.0), Width::Wide);
    }

    #[test]
    fn nan_inf_neg() {
        assert_eq!(classify(f32::NAN), Width::Normal);
        assert_eq!(classify(-f32::NAN), Width::Normal);
        assert_eq!(classify(f32::INFINITY), Width::Wide);
        assert_eq!(classify(f32::NEG_INFINITY), Width::Narrow);
        assert_eq!(classify(-1.0), Width::Narrow);
        assert_eq!(classify(0.0), Width::Narrow);
        assert_eq!(classify(-0.0), Width::Narrow);
        assert_eq!(classify(f32::MAX), Width::Wide);
        assert_eq!(classify(f32::MIN_POSITIVE), Width::Narrow);
    }

    #[test]
    fn monotonic_classify() {
        let mut prev = 0;
        let mut w = -10.0f32;
        while w < 2000.0 {
            let r = match classify(w) {
                Width::Narrow => 0,
                Width::Normal => 1,
                Width::Wide => 2,
            };
            assert!(r >= prev, "regress at {}", w);
            prev = r;
            w += 0.5;
        }
    }

    #[test]
    fn narrow_minimal() {
        let p = parts(Width::Narrow);
        assert!(!p.show_sparkline && !p.show_counters && !p.show_uptime && !p.show_state_text);
    }

    #[test]
    fn normal_parts() {
        let p = parts(Width::Normal);
        assert!(p.show_state_text && p.show_uptime && p.show_counters && !p.show_sparkline);
    }

    #[test]
    fn wide_all() {
        let p = parts(Width::Wide);
        assert!(p.show_sparkline && p.show_counters && p.show_uptime && p.show_state_text);
    }

    fn count(p: &Parts) -> [bool; 4] {
        [p.show_sparkline, p.show_counters, p.show_uptime, p.show_state_text]
    }

    #[test]
    fn wider_never_shows_less() {
        let order = [Width::Narrow, Width::Normal, Width::Wide];
        for i in 0..order.len() - 1 {
            let a = count(&parts(order[i]));
            let b = count(&parts(order[i + 1]));
            for k in 0..4 {
                assert!(!a[k] || b[k], "feature {} lost going wider", k);
            }
            assert!(parts(order[i]).tag_w <= parts(order[i + 1]).tag_w);
        }
    }

    #[test]
    fn tag_w_finite_nonneg() {
        for w in [Width::Narrow, Width::Normal, Width::Wide] {
            let t = parts(w).tag_w;
            assert!(t.is_finite() && t >= 0.0);
        }
    }

    #[test]
    fn nan_parts_equal_normal() {
        assert_eq!(parts(classify(f32::NAN)), parts(Width::Normal));
    }
}
