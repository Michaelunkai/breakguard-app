//! WCAG AA contrast tests for every log line kind on every surface, dark and light (Contrast).
#![allow(dead_code)]

#[cfg(test)]
mod tests {
    use crate::ui::logic::palette::{contrast, line_rgb, surfaces, ThemeKind, LINE_KINDS};

    #[test]
    fn every_line_kind_passes_aa_text_on_every_surface() {
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            for kind in LINE_KINDS {
                let fg = line_rgb(kind, theme);
                for bg in surfaces(theme) {
                    let r = contrast(fg, bg);
                    assert!(r >= 4.5, "{theme:?} {kind:?} {fg:?} on {bg:?} = {r:.3}, need 4.5");
                }
            }
        }
    }

    #[test]
    fn every_line_kind_has_distinct_color_per_theme() {
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            for (i, a) in LINE_KINDS.iter().enumerate() {
                for b in &LINE_KINDS[i + 1..] {
                    assert_ne!(line_rgb(*a, theme), line_rgb(*b, theme), "{theme:?} {a:?} == {b:?}");
                }
            }
        }
    }
}
