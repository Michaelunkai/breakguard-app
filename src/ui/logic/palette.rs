#![allow(dead_code)]
//! Palette: dark/light themes, one log color per line kind, WCAG contrast helpers. std-only.

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub bg: Rgb,
    pub surface: Rgb,
    pub surface_hi: Rgb,
    pub surface_lo: Rgb,
    pub text: Rgb,
    pub text_dim: Rgb,
    pub accent: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub danger: Rgb,
}

pub fn dark() -> Theme {
    Theme {
        bg: (9, 12, 22),
        surface: (16, 22, 36),
        surface_hi: (26, 35, 55),
        surface_lo: (12, 17, 29),
        text: (232, 238, 247),
        text_dim: (148, 163, 184),
        accent: (45, 212, 191),
        ok: (74, 222, 128),
        warn: (251, 191, 36),
        danger: (248, 113, 113),
    }
}

pub fn light() -> Theme {
    Theme {
        bg: (248, 250, 252),
        surface: (255, 255, 255),
        surface_hi: (234, 239, 246),
        surface_lo: (241, 244, 249),
        text: (15, 23, 42),
        text_dim: (71, 85, 105),
        accent: (15, 118, 110),
        ok: (21, 128, 61),
        warn: (160, 72, 8),
        danger: (185, 28, 28),
    }
}

/// Theme switch value. Text form is the stored/config string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
}

impl ThemeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeKind::Dark => "dark",
            ThemeKind::Light => "light",
        }
    }

    pub fn parse(s: &str) -> Option<ThemeKind> {
        match s {
            "dark" => Some(ThemeKind::Dark),
            "light" => Some(ThemeKind::Light),
            _ => None,
        }
    }

    pub fn theme(self) -> Theme {
        match self {
            ThemeKind::Dark => dark(),
            ThemeKind::Light => light(),
        }
    }
}

/// Log line kinds. Every kind has exactly one color per theme (see `line_rgb`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    User,
    Assistant,
    Tool,
    Result,
    Error,
    System,
}

/// All line kinds, for iteration in tests and pages.
pub const LINE_KINDS: [LineKind; 6] = [
    LineKind::User,
    LineKind::Assistant,
    LineKind::Tool,
    LineKind::Result,
    LineKind::Error,
    LineKind::System,
];

/// The single log color for a line kind in a theme.
pub fn line_rgb(kind: LineKind, theme: ThemeKind) -> Rgb {
    match (theme, kind) {
        (ThemeKind::Dark, LineKind::User) => (125, 211, 252),
        (ThemeKind::Dark, LineKind::Assistant) => (232, 238, 247),
        (ThemeKind::Dark, LineKind::Tool) => (196, 181, 253),
        (ThemeKind::Dark, LineKind::Result) => (148, 163, 184),
        (ThemeKind::Dark, LineKind::Error) => (252, 165, 165),
        (ThemeKind::Dark, LineKind::System) => (251, 191, 36),
        (ThemeKind::Light, LineKind::User) => (3, 105, 161),
        (ThemeKind::Light, LineKind::Assistant) => (15, 23, 42),
        (ThemeKind::Light, LineKind::Tool) => (109, 40, 217),
        (ThemeKind::Light, LineKind::Result) => (71, 85, 105),
        (ThemeKind::Light, LineKind::Error) => (185, 28, 28),
        (ThemeKind::Light, LineKind::System) => (146, 64, 14),
    }
}

/// Focus ring color per theme: amber on dark, dark amber on light (non-text UI part, needs 3:1 on each surface).
pub fn focus_ring(theme: ThemeKind) -> Rgb {
    match theme {
        ThemeKind::Dark => (255, 176, 0),
        ThemeKind::Light => (146, 64, 14),
    }
}
/// Focus ring stroke width in logical pixels.
pub const FOCUS_RING_WIDTH: f32 = 2.0;

fn lin(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
}

/// WCAG relative luminance, 0.0..=1.0.
pub fn luminance(c: Rgb) -> f64 {
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

/// WCAG contrast ratio, 1.0..=21.0, symmetric.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Every surface a log line can sit on in a theme.
pub fn surfaces(theme: ThemeKind) -> [Rgb; 4] {
    let t = theme.theme();
    [t.bg, t.surface, t.surface_hi, t.surface_lo]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_exact_values() {
        let d = dark();
        assert_eq!(d.bg, (9, 12, 22));
        assert_eq!(d.surface, (16, 22, 36));
        assert_eq!(d.text, (232, 238, 247));
        assert_eq!(d.text_dim, (148, 163, 184));
        assert_eq!(d.accent, (45, 212, 191));
    }

    #[test]
    fn wcag_known_values() {
        assert!((contrast((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 1e-9);
        assert!((contrast((255, 255, 255), (255, 255, 255)) - 1.0).abs() < 1e-12);
        let r = contrast((119, 119, 119), (255, 255, 255));
        assert!((r - 4.478).abs() < 0.01, "{r}");
        assert!(luminance((0, 0, 0)).abs() < 1e-12);
        assert!((luminance((255, 255, 255)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn contrast_symmetric_and_bounded() {
        let pts = [0u8, 1, 17, 128, 200, 255];
        for &a in &pts { for &b in &pts { for &c in &pts {
            let x = (a, b, c);
            let y = (c, a, b);
            let r = contrast(x, y);
            assert_eq!(r, contrast(y, x));
            assert!((1.0..=21.0 + 1e-9).contains(&r));
        }}}
    }

    #[test]
    fn adversarial_low_contrast_detected() {
        assert!(contrast((119, 119, 119), (130, 130, 130)) < 4.5);
        assert!(contrast((119, 119, 119), (255, 255, 255)) < 4.5);
    }

    #[test]
    fn themes_distinct_and_light_is_light() {
        assert_ne!(dark(), light());
        assert!(luminance(light().bg) > 0.5);
        assert!(luminance(dark().bg) < 0.05);
        assert!(luminance(light().text) < luminance(light().bg));
        assert!(luminance(dark().text) > luminance(dark().bg));
    }

    #[test]
    fn theme_kind_text_roundtrip() {
        for k in [ThemeKind::Dark, ThemeKind::Light] {
            assert_eq!(ThemeKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(ThemeKind::parse("Light"), None);
        assert_eq!(ThemeKind::parse(""), None);
        assert_eq!(ThemeKind::Light.theme(), light());
        assert_eq!(ThemeKind::Dark.theme(), dark());
    }

    #[test]
    fn line_kinds_all_listed_once() {
        assert_eq!(LINE_KINDS.len(), 6);
        for (i, a) in LINE_KINDS.iter().enumerate() {
            for b in &LINE_KINDS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn focus_ring_meets_3_to_1_on_every_surface() {
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            for bg in surfaces(theme) {
                let r = contrast(focus_ring(theme), bg);
                assert!(r >= 3.0, "{theme:?} focus ring on {bg:?} = {r:.3}, need 3.0");
            }
        }
    }

    #[test]
    fn luminance_monotonic_in_each_channel() {
        for c in 0..255u8 {
            assert!(luminance((c + 1, 0, 0)) > luminance((c, 0, 0)));
            assert!(luminance((0, c + 1, 0)) > luminance((0, c, 0)));
            assert!(luminance((0, 0, c + 1)) > luminance((0, 0, c)));
        }
    }
}
