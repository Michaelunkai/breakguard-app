#![allow(dead_code)]
// Spacing: row-density and display prefs for the lane log page. std-only.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Density {
    Comfortable,
    Dense,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Prefs {
    pub density: Density,
    pub wrap: bool,
    /// 0 off, 1 relative, 2 absolute
    pub time_mode: u8,
    pub font: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { density: Density::Comfortable, wrap: false, time_mode: 0, font: 13.0 }
    }
}

const FONT_MIN: f32 = 10.0;
const FONT_MAX: f32 = 22.0;

fn clean_font(f: f32) -> f32 {
    if f.is_finite() { f.clamp(FONT_MIN, FONT_MAX) } else { 13.0 }
}

/// Row height in px: font*1.45 (comfortable) or font*1.2 (dense), rounded to .5, min 12.
pub fn row_height(p: &Prefs) -> f32 {
    let mult = match p.density {
        Density::Comfortable => 1.45,
        Density::Dense => 1.2,
    };
    let f = if p.font.is_finite() { p.font } else { 13.0 };
    let h = ((f * mult) * 2.0).round() / 2.0;
    h.max(12.0)
}

impl Prefs {
    pub fn to_text(&self) -> String {
        let d = match self.density {
            Density::Comfortable => "comfortable",
            Density::Dense => "dense",
        };
        format!(
            "density={}\nwrap={}\ntime_mode={}\nfont={}\n",
            d,
            self.wrap,
            self.time_mode,
            clean_font(self.font)
        )
    }
}

/// Tolerant parser: unknown keys, junk lines, CRLF, bad values all ignored/defaulted.
pub fn from_text(s: &str) -> Prefs {
    let mut p = Prefs::default();
    for line in s.lines() {
        let line = line.trim();
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim()),
            None => continue,
        };
        match k.as_str() {
            "density" => match v.to_ascii_lowercase().as_str() {
                "dense" => p.density = Density::Dense,
                "comfortable" => p.density = Density::Comfortable,
                _ => {}
            },
            "wrap" => match v.to_ascii_lowercase().as_str() {
                "true" | "1" => p.wrap = true,
                "false" | "0" => p.wrap = false,
                _ => {}
            },
            "time_mode" => {
                p.time_mode = match v.parse::<u32>() {
                    Ok(n) if n <= 2 => n as u8,
                    _ => 0,
                }
            }
            "font" => {
                if let Ok(f) = v.parse::<f32>() {
                    p.font = clean_font(f);
                }
            }
            _ => {}
        }
    }
    p
}

impl Prefs {
    pub fn from_text(s: &str) -> Prefs {
        from_text(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let p = Prefs::default();
        assert_eq!(p, Prefs { density: Density::Comfortable, wrap: false, time_mode: 0, font: 13.0 });
    }

    #[test]
    fn height_comfortable() {
        assert_eq!(row_height(&Prefs::default()), 19.0); // 18.85 -> 19.0
    }

    #[test]
    fn height_dense() {
        let p = Prefs { density: Density::Dense, ..Prefs::default() };
        assert_eq!(row_height(&p), 15.5); // 15.6 -> 15.5
    }

    #[test]
    fn height_min_12() {
        let p = Prefs { density: Density::Dense, font: 1.0, ..Prefs::default() };
        assert_eq!(row_height(&p), 12.0);
        let p = Prefs { font: -5.0, ..Prefs::default() };
        assert_eq!(row_height(&p), 12.0);
    }

    #[test]
    fn height_nan_inf() {
        let p = Prefs { font: f32::NAN, ..Prefs::default() };
        assert_eq!(row_height(&p), 19.0);
        let p = Prefs { font: f32::INFINITY, ..Prefs::default() };
        assert_eq!(row_height(&p), 19.0);
    }

    #[test]
    fn height_half_steps() {
        for f in [10.0f32, 10.3, 14.7, 22.0] {
            for d in [Density::Comfortable, Density::Dense] {
                let h = row_height(&Prefs { density: d, font: f, ..Prefs::default() });
                assert_eq!((h * 2.0).fract(), 0.0);
                assert!(h >= 12.0);
            }
        }
    }

    #[test]
    fn roundtrip_both_densities() {
        for d in [Density::Comfortable, Density::Dense] {
            for w in [false, true] {
                for t in 0..=2u8 {
                    for f in [10.0f32, 13.0, 15.5, 22.0] {
                        let p = Prefs { density: d, wrap: w, time_mode: t, font: f };
                        assert_eq!(from_text(&p.to_text()), p);
                        assert_eq!(Prefs::from_text(&p.to_text()), p);
                    }
                }
            }
        }
    }

    #[test]
    fn roundtrip_default_text() {
        assert_eq!(Prefs::default().to_text(), "density=comfortable\nwrap=false\ntime_mode=0\nfont=13\n");
    }

    #[test]
    fn garbage_gives_default() {
        for s in ["", "\u{0}\u{1}\u{2}", "=====", "no equals here", "\n\n\n", "💥=💥\n=\n==\n", "density", "font=", "密度=dense"] {
            assert_eq!(from_text(s), Prefs::default(), "input {:?}", s);
        }
    }

    #[test]
    fn crlf_and_whitespace() {
        let p = from_text("density=dense\r\nwrap=true\r\ntime_mode=2\r\nfont=16\r\n");
        assert_eq!(p, Prefs { density: Density::Dense, wrap: true, time_mode: 2, font: 16.0 });
        let p = from_text("  density = dense  \r\n\twrap\t=\ttrue\r\n");
        assert_eq!(p.density, Density::Dense);
        assert!(p.wrap);
    }

    #[test]
    fn unknown_keys_ignored() {
        let p = from_text("zzz=1\ndensity=dense\nfoo=bar=baz\nwrap=true\n");
        assert_eq!(p.density, Density::Dense);
        assert!(p.wrap);
    }

    #[test]
    fn font_clamped() {
        assert_eq!(from_text("font=3").font, 10.0);
        assert_eq!(from_text("font=999").font, 22.0);
        assert_eq!(from_text("font=-4").font, 10.0);
        assert_eq!(from_text("font=1e30").font, 22.0);
        assert_eq!(from_text("font=NaN").font, 13.0);
        assert_eq!(from_text("font=inf").font, 13.0);
        assert_eq!(from_text("font=abc").font, 13.0);
    }

    #[test]
    fn time_mode_out_of_range() {
        assert_eq!(from_text("time_mode=3").time_mode, 0);
        assert_eq!(from_text("time_mode=255").time_mode, 0);
        assert_eq!(from_text("time_mode=256").time_mode, 0);
        assert_eq!(from_text("time_mode=99999999999999999999").time_mode, 0);
        assert_eq!(from_text("time_mode=-1").time_mode, 0);
        assert_eq!(from_text("time_mode=1").time_mode, 1);
        assert_eq!(from_text("time_mode=x").time_mode, 0);
    }

    #[test]
    fn bad_value_keeps_default_density_wrap() {
        let p = from_text("density=huge\nwrap=maybe\n");
        assert_eq!(p.density, Density::Comfortable);
        assert!(!p.wrap);
    }

    #[test]
    fn last_key_wins() {
        assert_eq!(from_text("time_mode=1\ntime_mode=2\n").time_mode, 2);
    }

    #[test]
    fn to_text_sanitizes_bad_font() {
        let p = Prefs { font: f32::NAN, ..Prefs::default() };
        assert_eq!(from_text(&p.to_text()).font, 13.0);
        let p = Prefs { font: 500.0, ..Prefs::default() };
        assert_eq!(from_text(&p.to_text()).font, 22.0);
    }

    #[test]
    fn huge_input_no_panic() {
        let s = "x=1\n".repeat(100_000) + "density=dense";
        assert_eq!(from_text(&s).density, Density::Dense);
    }
}
