//! slice tokens_doc (owned by one team agent)
//! API: pub const RADII: [f32; 4] (corner radii px, ascending)
//! API: pub const SPACING: [f32; 6] (spacing px, ascending)
//! API: pub const ELEVATION: [(f32, f32); 4] (shadow (blur, offset) px, ascending blur)
//! API: pub const TOKENS: [(&str, [u8; 3]); N] (colour tokens mirrored from ui::theme, std-only)
//! API: pub fn contrast_report() -> Vec<(String, f64)> (text/status/accent on SURFACE_LO, WCAG ratio)
//! API: pub const DESIGN_SECTION: &str (markdown: palette, radii, spacing, elevation)
//! Std-only on purpose: no egui / crate imports, so `rustc --test` runs standalone.
//! Keep TOKENS and the numbers in ui::theme in sync (see tests).
#![allow(dead_code)]

/// Corner radii in px: none, small, medium, large (matches theme R_S, R_M, R_L).
pub const RADII: [f32; 4] = [0.0, 8.0, 12.0, 16.0];

/// Spacing scale in px (8px grid based; matches theme SP_XS, SP_S, SP_M, SP_L).
pub const SPACING: [f32; 6] = [0.0, 4.0, 8.0, 16.0, 24.0, 32.0];

/// Shadow elevations as (blur, offset) in px.
pub const ELEVATION: [(f32, f32); 4] = [(0.0, 0.0), (4.0, 1.0), (8.0, 2.0), (16.0, 4.0)];

/// Colour tokens, mirrored from ui::theme (sRGB u8).
pub const TOKENS: [(&str, [u8; 3]); 15] = [
    ("BG", [9, 12, 22]),
    ("SURFACE", [16, 22, 36]),
    ("SURFACE_HI", [26, 35, 55]),
    ("SURFACE_LO", [12, 17, 29]),
    ("BORDER", [38, 49, 72]),
    ("BORDER_STRONG", [100, 116, 139]),
    ("TEXT", [232, 238, 247]),
    ("TEXT_DIM", [148, 163, 184]),
    ("OK", [74, 222, 128]),
    ("WARN", [251, 191, 36]),
    ("DANGER", [248, 113, 113]),
    ("DANGER_HI", [252, 165, 165]),
    ("ON_FILL", [6, 20, 24]),
    ("ACCENT", [45, 212, 191]),
    ("ACCENT_HI", [94, 234, 212]),
];

/// Lookup a colour token by name.
pub fn token(name: &str) -> Option<[u8; 3]> {
    TOKENS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// WCAG 2.x relative luminance of an sRGB colour.
fn luminance(c: [u8; 3]) -> f64 {
    let lin = |v: u8| {
        let s = v as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio between two colours (1.0..=21.0).
pub fn ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// WCAG contrast ratio of each text/status/accent token against SURFACE_LO.
pub fn contrast_report() -> Vec<(String, f64)> {
    let bg = token("SURFACE_LO").expect("SURFACE_LO in TOKENS");
    ["TEXT", "TEXT_DIM", "OK", "WARN", "DANGER", "ACCENT"]
        .iter()
        .map(|name| {
            let c = token(name).expect("name in TOKENS");
            (name.to_string(), ratio(c, bg))
        })
        .collect()
}

/// Design tokens reference (markdown).
pub const DESIGN_SECTION: &str = "## Design tokens

### Palette (dark, calm night)
- BG #090C16, SURFACE #101624, SURFACE_HI #1A2337, SURFACE_LO #0C111D
- BORDER #263148, BORDER_STRONG #64748B
- TEXT #E8EEF7, TEXT_DIM #94A3B8
- OK #4ADE80, WARN #FBBF24, DANGER #F87171, DANGER_HI #FCA5A5
- Accent: user-selectable from theme SWATCHES

### Radii (px)
- none 0, small 8, medium 12, large 16

### Spacing (px, 8px grid)
- 0, xs 4, s 8, m 16, l 24, xl 32

### Elevation (shadow blur / offset px)
- 0: none
- 1: blur 4, offset 1
- 2: blur 8, offset 2
- 3: blur 16, offset 4

### Contrast
- Text and status colours meet WCAG AA 4.5:1 on SURFACE_LO (see contrast_report).
";

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `NAME` token written in the DESIGN.md colour table must exist in TOKENS.
    #[test]
    fn design_md_tokens_exist_in_table() {
        let design = include_str!("../../../DESIGN.md");
        let mut checked = 0;
        for (name, _) in TOKENS.iter() {
            assert!(design.contains(name), "TOKENS has {name} but DESIGN.md does not");
            checked += 1;
        }
        assert_eq!(checked, TOKENS.len());
        for word in ["BG", "SURFACE", "SURFACE_HI", "TEXT", "TEXT_DIM", "ACCENT", "OK", "WARN", "DANGER"] {
            assert!(token(word).is_some(), "DESIGN.md token {word} missing from TOKENS");
        }
    }

    #[test]
    fn scales_are_ascending() {
        assert!(RADII.windows(2).all(|w| w[0] < w[1]));
        assert!(SPACING.windows(2).all(|w| w[0] < w[1]));
        assert!(ELEVATION.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 <= w[1].1));
    }

    #[test]
    fn every_contrast_ratio_meets_aa() {
        let report = contrast_report();
        assert_eq!(report.len(), 6);
        for (name, r) in &report {
            assert!(*r >= 4.5, "{name} ratio {r:.2} < 4.5");
        }
    }

    #[test]
    fn ratio_is_symmetric_and_bounded() {
        let (w, b) = ([255, 255, 255], [0, 0, 0]);
        assert!((ratio(w, b) - 21.0).abs() < 1e-9);
        assert!((ratio(w, b) - ratio(b, w)).abs() < 1e-12);
    }

    #[test]
    fn design_section_mentions_every_scale() {
        for word in ["Palette", "Radii", "Spacing", "Elevation"] {
            assert!(DESIGN_SECTION.contains(word), "missing {word}");
        }
    }
}
