//! slice light_polish (owned by one team agent)
//! API: pub struct LightPalette { bg, surface, surface_lo, border, text, text_dim, ok, warn, danger, accent,
//!      log_assistant, log_tool, log_result, log_user, log_error: (u8,u8,u8) }
//! API: pub const fn light() -> LightPalette
//! API: pub const CREATURE_ACCENTS: [Rgb; 12]
//! Light theme palette. Text/status/log colours pass WCAG AA 4.5:1 on surface and surface_lo;
//! border passes WCAG 1.4.11 non-text 3:1 on both surfaces.
#![allow(dead_code)]

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LightPalette {
    pub bg: Rgb,
    pub surface: Rgb,
    pub surface_lo: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub text_dim: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub danger: Rgb,
    pub accent: Rgb,
    pub log_assistant: Rgb,
    pub log_tool: Rgb,
    pub log_result: Rgb,
    pub log_user: Rgb,
    pub log_error: Rgb,
}

pub const fn light() -> LightPalette {
    LightPalette {
        bg: (244, 246, 249),
        surface: (255, 255, 255),
        surface_lo: (236, 240, 245),
        border: (120, 126, 140),
        text: (17, 24, 39),
        text_dim: (71, 84, 103),
        ok: (21, 110, 50),
        warn: (146, 64, 14),
        danger: (185, 28, 28),
        accent: (29, 78, 216),
        log_assistant: (17, 24, 39),
        log_tool: (29, 78, 216),
        log_result: (71, 84, 103),
        log_user: (146, 64, 14),
        log_error: (185, 28, 28),
    }
}

/// Creature accent colours for light lanes, one per creature (COUNT = 12).
pub const CREATURE_ACCENTS: [Rgb; 12] = [
    (29, 78, 216),
    (147, 30, 110),
    (21, 110, 50),
    (146, 64, 14),
    (185, 28, 28),
    (13, 100, 120),
    (88, 28, 160),
    (67, 56, 169),
    (75, 85, 99),
    (120, 60, 0),
    (4, 100, 95),
    (136, 20, 80),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::logic::contrast::{passes_aa, ratio};

    fn texts(p: &LightPalette) -> [(&'static str, Rgb); 11] {
        [
            ("text", p.text),
            ("text_dim", p.text_dim),
            ("ok", p.ok),
            ("warn", p.warn),
            ("danger", p.danger),
            ("accent", p.accent),
            ("log_assistant", p.log_assistant),
            ("log_tool", p.log_tool),
            ("log_result", p.log_result),
            ("log_user", p.log_user),
            ("log_error", p.log_error),
        ]
    }

    #[test]
    fn text_and_status_pass_aa_on_both_surfaces() {
        let p = light();
        for (name, c) in texts(&p) {
            for (sname, s) in [("surface", p.surface), ("surface_lo", p.surface_lo)] {
                let r = ratio(c, s);
                assert!(passes_aa(c, s), "{name} on {sname} = {r:.2}:1, need 4.5");
            }
        }
    }

    #[test]
    fn creature_accents_pass_aa_on_both_surfaces() {
        let p = light();
        assert_eq!(CREATURE_ACCENTS.len(), 12);
        for (i, c) in CREATURE_ACCENTS.iter().enumerate() {
            for (sname, s) in [("surface", p.surface), ("surface_lo", p.surface_lo)] {
                let r = ratio(*c, s);
                assert!(passes_aa(*c, s), "creature {i} on {sname} = {r:.2}:1, need 4.5");
            }
        }
    }

    #[test]
    fn creature_accents_are_distinct() {
        for i in 0..CREATURE_ACCENTS.len() {
            for j in i + 1..CREATURE_ACCENTS.len() {
                assert_ne!(CREATURE_ACCENTS[i], CREATURE_ACCENTS[j], "creatures {i} and {j} share a colour");
            }
        }
    }

    #[test]
    fn border_distinguishable_on_both_surfaces() {
        let p = light();
        for (sname, s) in [("surface", p.surface), ("surface_lo", p.surface_lo)] {
            let r = ratio(p.border, s);
            assert!(r >= 3.0, "border on {sname} = {r:.2}:1, need 3:1");
        }
    }

    #[test]
    fn surfaces_are_distinct_from_each_other_and_bg() {
        let p = light();
        assert_ne!(p.surface, p.surface_lo);
        assert_ne!(p.surface, p.bg);
        assert_ne!(p.surface_lo, p.bg);
    }

    #[test]
    fn text_is_the_strongest_text_colour() {
        let p = light();
        for s in [p.surface, p.surface_lo] {
            assert!(ratio(p.text, s) >= ratio(p.text_dim, s));
        }
    }

    #[test]
    fn status_colours_are_distinct() {
        let p = light();
        let st = [p.ok, p.warn, p.danger, p.accent];
        for i in 0..st.len() {
            for j in i + 1..st.len() {
                assert_ne!(st[i], st[j]);
            }
        }
    }

    #[test]
    fn light_is_const_callable() {
        const P: LightPalette = light();
        assert_eq!(P, light());
    }
}
