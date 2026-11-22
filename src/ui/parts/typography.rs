//! Type scale (std-only, no egui dependency so it can be unit-tested standalone).
//! Steps: Display, Heading, Title, Body, Small, Mono.
//! Sizes in points. Body is the minimum readable size (13 pt).
#![allow(dead_code)]

/// Type scale steps, largest to smallest (Mono sits at body size).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Display,
    Heading,
    Title,
    Body,
    Small,
    Mono,
}

/// Every scale step, in declaration order.
pub const ALL: [Scale; 6] = [
    Scale::Display,
    Scale::Heading,
    Scale::Title,
    Scale::Body,
    Scale::Small,
    Scale::Mono,
];

/// Minimum allowed body text size in points.
pub const MIN_BODY_PT: f32 = 13.0;

/// Font size in points, indexed by `Scale as usize`.
pub const SIZES: [f32; 6] = [34.0, 24.0, 18.0, 13.0, 11.0, 13.0];

/// Line height in points, indexed by `Scale as usize` (about 1.4x size).
pub const LINE_HEIGHTS: [f32; 6] = [48.0, 34.0, 26.0, 18.0, 16.0, 18.0];

/// Bundled font family names (egui default fonts, in fallback order).
pub const PROPORTIONAL_FAMILIES: [&str; 3] = ["Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"];
pub const MONO_FAMILIES: [&str; 3] = ["Hack-Regular", "NotoEmoji-Regular", "emoji-icon-font"];

/// Font size in points for a scale step.
pub fn size(s: Scale) -> f32 {
    SIZES[s as usize]
}

/// Line height in points for a scale step.
pub fn line_height(s: Scale) -> f32 {
    LINE_HEIGHTS[s as usize]
}

/// Bundled font family names for a scale step (monospace for Mono).
pub fn families(s: Scale) -> &'static [&'static str] {
    match s {
        Scale::Mono => &MONO_FAMILIES,
        _ => &PROPORTIONAL_FAMILIES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_monotonic_non_increasing_down_the_scale() {
        for pair in ALL.windows(2) {
            if pair[1] == Scale::Mono {
                continue;
            }
            assert!(
                size(pair[0]) > size(pair[1]),
                "size({:?}) must be > size({:?})",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn body_meets_min_13pt() {
        assert!(size(Scale::Body) >= MIN_BODY_PT);
        assert!(size(Scale::Mono) >= MIN_BODY_PT);
    }

    #[test]
    fn line_height_ratio_within_band() {
        for s in ALL {
            let ratio = line_height(s) / size(s);
            assert!((1.35..=1.5).contains(&ratio), "{s:?} ratio {ratio}");
        }
    }

    #[test]
    fn families_non_empty_and_mono_uses_mono_list() {
        for s in ALL {
            assert!(!families(s).is_empty());
        }
        assert_eq!(families(Scale::Mono)[0], "Hack-Regular");
        assert_eq!(families(Scale::Body)[0], "Ubuntu-Light");
    }
}
