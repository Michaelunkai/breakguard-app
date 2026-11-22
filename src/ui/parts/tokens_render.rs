//! API: pub fn highlighted_job(text: &str, base: Color32, font: FontId, query: &str) -> egui::text::LayoutJob
//! Colours log-line tokens (paths, numbers, error words, code spans) and paints search matches with a highlight background.
//! std + egui only. Section byte ranges are contiguous and cover all of `text`.
#![allow(dead_code)]

use crate::ui::logic::filter::match_ranges;
use crate::ui::logic::token::{tokenize, Tk};
use crate::ui::theme;
use eframe::egui::text::{LayoutJob, LayoutSection, TextFormat};
use eframe::egui::{Color32, FontId};

pub const PATH_COLOR: Color32 = Color32::from_rgb(125, 211, 252);
pub const NUMBER_COLOR: Color32 = Color32::from_rgb(196, 181, 253);
pub const CODE_COLOR: Color32 = Color32::from_rgb(253, 186, 116);
pub const MATCH_BG: Color32 = Color32::from_rgba_premultiplied(90, 70, 10, 230);

fn kind_color(kind: Tk, base: Color32) -> Color32 {
    if theme::is_light() {
        // Pale dark-theme token colours are unreadable on white; these reach AA (4.5:1) on the light surface.
        return match kind {
            Tk::Plain => base,
            Tk::Path => Color32::from_rgb(3, 105, 161),
            Tk::Number => Color32::from_rgb(109, 40, 217),
            Tk::ErrorWord => Color32::from_rgb(185, 28, 28),
            Tk::Code => Color32::from_rgb(154, 52, 18),
        };
    }
    match kind {
        Tk::Plain => base,
        Tk::Path => PATH_COLOR,
        Tk::Number => NUMBER_COLOR,
        Tk::ErrorWord => theme::DANGER_HI,
        Tk::Code => CODE_COLOR,
    }
}

/// Build a LayoutJob for `text`: token kinds colour the text, `query` matches get `MATCH_BG`.
pub fn highlighted_job(text: &str, base: Color32, font: FontId, query: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.text = text.to_string();
    if text.is_empty() {
        return job;
    }

    // Token spans as (start, end, kind) in byte offsets; spans concatenate to `text`.
    let mut spans: Vec<(usize, usize, Tk)> = Vec::new();
    let mut pos = 0usize;
    for s in tokenize(text) {
        let end = pos + s.text.len();
        spans.push((pos, end, s.kind));
        pos = end;
    }
    let matches = match_ranges(text, query);

    // Cut points: span edges and match edges.
    let mut cuts: Vec<usize> = Vec::with_capacity(spans.len() * 2 + matches.len() * 2 + 2);
    cuts.push(0);
    cuts.push(text.len());
    for &(s, e, _) in &spans {
        cuts.push(s);
        cuts.push(e);
    }
    for &(s, e) in &matches {
        cuts.push(s);
        cuts.push(e);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut last: Option<(Color32, Option<Color32>)> = None;
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b || b > text.len() || !text.is_char_boundary(a) || !text.is_char_boundary(b) {
            continue;
        }
        let idx = spans.partition_point(|s| s.0 <= a);
        let kind = if idx == 0 { Tk::Plain } else { spans[idx - 1].2 };
        let bg = matches.iter().any(|&(s, e)| s <= a && a < e).then_some(MATCH_BG);
        let color = kind_color(kind, base);
        let fmt = (color, bg);
        if last == Some(fmt) {
            if let Some(sec) = job.sections.last_mut() {
                sec.byte_range.end = b;
            }
            continue;
        }
        last = Some(fmt);
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: a..b,
            format: TextFormat {
                font_id: font.clone(),
                color,
                background: bg.unwrap_or(Color32::TRANSPARENT),
                ..Default::default()
            },
        });
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> FontId {
        FontId::monospace(12.0)
    }

    fn covers(job: &LayoutJob) {
        let mut pos = 0usize;
        for s in &job.sections {
            assert_eq!(s.byte_range.start, pos, "gap or overlap at {}", pos);
            assert!(s.byte_range.end > s.byte_range.start, "empty section");
            assert!(job.text.is_char_boundary(s.byte_range.start));
            assert!(job.text.is_char_boundary(s.byte_range.end));
            pos = s.byte_range.end;
        }
        assert_eq!(pos, job.text.len(), "sections do not cover text");
    }

    #[test]
    fn sections_cover_text() {
        let cases = [
            "error: F:/x/y.rs:12:3 failed",
            "run `a b` now 1,234.5",
            "plain",
            "  spaces  ",
        ];
        for t in cases {
            for q in ["", "error", "x", "1", "zz"] {
                let job = highlighted_job(t, theme::TEXT, font(), q);
                assert_eq!(job.text, t);
                covers(&job);
            }
        }
    }

    #[test]
    fn empty_text_no_sections() {
        let job = highlighted_job("", theme::TEXT, font(), "err");
        assert!(job.text.is_empty());
        assert!(job.sections.is_empty());
    }

    #[test]
    fn unicode_no_panic_and_covers() {
        let t = "שגיאה: /tmp/קובץ.rs failed `עברית` İstanbul 😀 error";
        for q in ["", "שגיאה", "ERROR", "😀", "İ", "tmp"] {
            let job = highlighted_job(t, theme::TEXT, font(), q);
            covers(&job);
        }
    }

    #[test]
    fn match_gets_background() {
        let job = highlighted_job("abc needle xyz", theme::TEXT, font(), "needle");
        let hit = job
            .sections
            .iter()
            .find(|s| &job.text[s.byte_range.clone()] == "needle")
            .expect("match section");
        assert_eq!(hit.format.background, MATCH_BG);
        let other = job.sections.iter().find(|s| s.byte_range.start == 0).unwrap();
        assert_eq!(other.format.background, Color32::TRANSPARENT);
    }

    fn lin(c: u8) -> f64 {
        let v = c as f64 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    }

    fn luminance(c: Color32) -> f64 {
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn token_colours_meet_aa_on_surface_lo() {
        for c in [theme::TEXT, PATH_COLOR, NUMBER_COLOR, CODE_COLOR, theme::DANGER_HI] {
            assert!(contrast(c, theme::SURFACE_LO) >= 4.5, "{:?} below 4.5:1", c);
        }
    }

    #[test]
    fn token_kinds_coloured() {
        let job = highlighted_job("error 12 /a/b `c`", theme::TEXT, font(), "");
        let color_of = |w: &str| {
            job.sections
                .iter()
                .find(|s| &job.text[s.byte_range.clone()] == w)
                .map(|s| s.format.color)
                .unwrap()
        };
        assert_eq!(color_of("error"), theme::DANGER_HI);
        assert_eq!(color_of("12"), NUMBER_COLOR);
        assert_eq!(color_of("/a/b"), PATH_COLOR);
        assert_eq!(color_of("`c`"), CODE_COLOR);
    }
}
