//! slice lane_wrap_row (owned by one team agent)
//! API: pub fn row_height(text: &str, width_chars: usize, wrap: bool, expanded: bool, line_h: f32) -> f32
//! API: `pub fn wrap_lines(text: &str, width_chars: usize) -> Vec<String>`
//! API: pub fn truncated(text: &str, width_chars: usize) -> String
#![allow(dead_code)]

use crate::ui::logic::wrap;

/// Height of one lane text row in points.
/// wrap=true: every wrapped line is shown.
/// wrap=false: one line (ellipsised) unless `expanded`, then every '\n' line.
pub fn row_height(text: &str, width_chars: usize, wrap: bool, expanded: bool, line_h: f32) -> f32 {
    let lines = if wrap {
        wrap::wrap_lines(text, width_chars).len()
    } else if expanded {
        wrap::row_count(text, width_chars, false)
    } else {
        1
    };
    lines.max(1) as f32 * line_h
}

/// Word-aware wrap to `width_chars`; words longer than the width are hard-broken.
pub fn wrap_lines(text: &str, width_chars: usize) -> Vec<String> {
    wrap::wrap_lines(text, width_chars)
}

/// Single line cut to `width_chars` chars, ending with '…' when shortened.
pub fn truncated(text: &str, width_chars: usize) -> String {
    wrap::truncate_line(text, width_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn very_long_line_wraps_and_truncates() {
        let long = "word ".repeat(10_000);
        let lines = wrap_lines(&long, 20);
        assert!(lines.len() > 1000);
        assert!(lines.iter().all(|l| l.chars().count() <= 20));
        let t = truncated(&long, 20);
        assert_eq!(t.chars().count(), 20);
        assert!(t.ends_with('…'));
        assert_eq!(row_height(&long, 20, false, false, 14.0), 14.0);
        assert_eq!(row_height(&long, 20, true, false, 14.0), lines.len() as f32 * 14.0);
    }

    #[test]
    fn empty_text() {
        assert_eq!(wrap_lines("", 10), vec![String::new()]);
        assert_eq!(truncated("", 10), "");
        assert_eq!(row_height("", 10, true, false, 14.0), 14.0);
        assert_eq!(row_height("", 10, false, true, 14.0), 14.0);
    }

    #[test]
    fn width_one() {
        assert_eq!(truncated("abc", 1), "…");
        assert_eq!(truncated("a", 1), "a");
        assert_eq!(wrap_lines("abc", 1), vec!["a", "b", "c"]);
        assert_eq!(row_height("abc", 1, true, false, 10.0), 30.0);
        assert_eq!(row_height("abc", 1, false, false, 10.0), 10.0);
    }

    #[test]
    fn expanded_nowrap_counts_newlines() {
        assert_eq!(row_height("a\nb\nc", 10, false, true, 10.0), 30.0);
        assert_eq!(row_height("a\nb\nc", 10, false, false, 10.0), 10.0);
    }

    #[test]
    fn hebrew_never_splits_inside_char() {
        let he = "שלום עולם זה טקסט בעברית ארוך מאוד לבדיקה";
        for w in 1..=12 {
            let lines = wrap_lines(he, w);
            let joined: String = lines.join(" ");
            let orig: Vec<char> = he.chars().filter(|c| !c.is_whitespace()).collect();
            let got: Vec<char> = joined.chars().filter(|c| !c.is_whitespace()).collect();
            assert_eq!(orig, got, "width {w}");
            assert!(lines.iter().all(|l| l.chars().count() <= w));
            let t = truncated(he, w);
            assert!(t.chars().count() <= w);
            assert!(w == 0 || t.chars().count() == w);
            assert!(t.ends_with('…'));
        }
    }
}
