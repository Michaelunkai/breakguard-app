#![allow(dead_code)]
//! Pure text wrapping helpers (std-only). Widths are counted in chars.

/// Control chars -> spaces, trim, cut to `max_chars` chars ending in '…' when too long.
pub fn truncate_line(text: &str, max_chars: usize) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = cleaned.trim();
    if t.chars().count() <= max_chars {
        return t.to_string();
    }
    match max_chars {
        0 => String::new(),
        1 => "…".to_string(),
        n => {
            let mut s: String = t.chars().take(n - 1).collect();
            s.push('…');
            s
        }
    }
}

/// Word wrap. '\n' starts a new paragraph. Never returns an empty vec.
/// width 0 -> whole text as one line.
pub fn wrap_lines(text: &str, width_chars: usize) -> Vec<String> {
    if width_chars == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for para in text.split('\n') {
        let para = para.strip_suffix('\r').unwrap_or(para);
        let mut cur = String::new();
        let mut cur_len = 0usize;
        for word in para.split_whitespace() {
            let mut chars: Vec<char> = word.chars().collect();
            // break over-long words
            while chars.len() > width_chars {
                if cur_len > 0 {
                    out.push(std::mem::take(&mut cur));
                    cur_len = 0;
                }
                out.push(chars.drain(..width_chars).collect());
            }
            if chars.is_empty() {
                continue;
            }
            let wl = chars.len();
            if cur_len == 0 {
                cur = chars.into_iter().collect();
                cur_len = wl;
            } else if cur_len + 1 + wl <= width_chars {
                cur.push(' ');
                cur.extend(chars);
                cur_len += 1 + wl;
            } else {
                out.push(std::mem::take(&mut cur));
                cur = chars.into_iter().collect();
                cur_len = wl;
            }
        }
        if cur_len > 0 || out.is_empty() || para.trim().is_empty() {
            out.push(cur);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Rows needed. wrap=false: one row per '\n' line. Always >= 1.
pub fn row_count(text: &str, width: usize, wrap: bool) -> usize {
    if wrap {
        wrap_lines(text, width).len()
    } else {
        text.split('\n').count().max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunc_basic() {
        assert_eq!(truncate_line("hello", 10), "hello");
        assert_eq!(truncate_line("hello", 5), "hello");
        assert_eq!(truncate_line("hello world", 5), "hell…");
        assert_eq!(truncate_line("  hi \t\n there ", 20), "hi    there");
    }
    #[test]
    fn trunc_small_max() {
        assert_eq!(truncate_line("abc", 0), "");
        assert_eq!(truncate_line("abc", 1), "…");
        assert_eq!(truncate_line("a", 1), "a");
        assert_eq!(truncate_line("abc", 2), "a…");
        assert_eq!(truncate_line("", 0), "");
    }
    #[test]
    fn trunc_unicode() {
        assert_eq!(truncate_line("שלום עולם", 4), "שלו…");
        assert_eq!(truncate_line("日本語テキスト", 3), "日本…");
        assert_eq!(truncate_line("😀😀😀😀", 3).chars().count(), 3);
        assert_eq!(truncate_line("e\u{301}e\u{301}e\u{301}", 3).chars().count(), 3);
    }
    #[test]
    fn trunc_long() {
        let s = "x".repeat(1_000_000);
        let r = truncate_line(&s, 50);
        assert_eq!(r.chars().count(), 50);
        assert!(r.ends_with('…'));
        assert_eq!(truncate_line(&s, usize::MAX).len(), 1_000_000);
    }
    #[test]
    fn trunc_controls_only() {
        assert_eq!(truncate_line("\n\t\r\u{0}", 5), "");
        assert_eq!(truncate_line("a\u{7}b", 5), "a b");
    }
    #[test]
    fn wrap_basic() {
        assert_eq!(wrap_lines("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
        assert_eq!(wrap_lines("aaa bbb", 7), vec!["aaa bbb"]);
        assert_eq!(wrap_lines("aaa bbb", 6), vec!["aaa", "bbb"]);
        assert_eq!(wrap_lines("a  b", 10), vec!["a b"]);
    }
    #[test]
    fn wrap_exact_fit() {
        assert_eq!(wrap_lines("abcd", 4), vec!["abcd"]);
        assert_eq!(wrap_lines("ab cd", 5), vec!["ab cd"]);
        assert_eq!(wrap_lines("ab cd", 4), vec!["ab", "cd"]);
    }
    #[test]
    fn wrap_long_word() {
        assert_eq!(wrap_lines("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap_lines("x abcdefg y", 3), vec!["x", "abc", "def", "g y"]);
        let r = wrap_lines(&"z".repeat(100_000), 7);
        assert_eq!(r.len(), 14286);
        assert!(r.iter().all(|l| l.chars().count() <= 7));
    }
    #[test]
    fn wrap_width_zero_and_one() {
        assert_eq!(wrap_lines("a b c", 0), vec!["a b c"]);
        assert_eq!(wrap_lines("", 0), vec![""]);
        assert_eq!(wrap_lines("ab c", 1), vec!["a", "b", "c"]);
    }
    #[test]
    fn wrap_never_empty() {
        for t in ["", " ", "\n", "   \n  ", "a", "\r\n"] {
            for w in [0, 1, 2, 80] {
                assert!(!wrap_lines(t, w).is_empty(), "{:?} {}", t, w);
            }
        }
    }
    #[test]
    fn wrap_newlines() {
        assert_eq!(wrap_lines("a\nb", 10), vec!["a", "b"]);
        assert_eq!(wrap_lines("a\n\nb", 10), vec!["a", "", "b"]);
        assert_eq!(wrap_lines("a\r\nb", 10), vec!["a", "b"]);
        assert_eq!(wrap_lines("a\n", 10), vec!["a", ""]);
    }
    #[test]
    fn wrap_hebrew() {
        assert_eq!(wrap_lines("שלום עולם יפה", 9), vec!["שלום עולם", "יפה"]);
        assert_eq!(wrap_lines("שלומשלום", 3), vec!["שלו", "משל", "ום"]);
    }
    #[test]
    fn wrap_invariants_adversarial() {
        let texts = ["a b  c\td", "😀 😀😀 😀😀😀", "אבג דהו  זחט\nיכל", "  lead trail  ", "x"];
        for t in texts {
            for w in 1..12 {
                for l in wrap_lines(t, w) {
                    assert!(l.chars().count() <= w, "{:?} w{} {:?}", t, w, l);
                    assert_eq!(l, l.trim());
                }
            }
        }
    }
    #[test]
    fn rows() {
        assert_eq!(row_count("", 10, true), 1);
        assert_eq!(row_count("", 10, false), 1);
        assert_eq!(row_count("aaa bbb ccc", 7, true), 2);
        assert_eq!(row_count("aaa bbb ccc", 7, false), 1);
        assert_eq!(row_count("a\nb\nc", 10, false), 3);
        assert_eq!(row_count("a\nb\nc", 10, true), 3);
        assert_eq!(row_count("abcdefghij", 4, true), 3);
        assert_eq!(row_count("abc", 0, true), 1);
        assert_eq!(row_count("abcd", 4, true), 1);
    }
}
