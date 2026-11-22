#![allow(dead_code)]
//! Case-insensitive substring filter for log lines. std-only.

/// Lowercased chars of `s`, each tagged with the byte range of its source char.
fn fold(s: &str) -> Vec<(char, usize, usize)> {
    let mut v = Vec::new();
    for (i, c) in s.char_indices() {
        let e = i + c.len_utf8();
        for l in c.to_lowercase() {
            v.push((l, i, e));
        }
    }
    v
}

fn is_blank(q: &str) -> bool {
    q.chars().all(char::is_whitespace)
}

pub fn matches(line: &str, query: &str) -> bool {
    is_blank(query) || !match_ranges(line, query).is_empty()
}

/// Byte ranges (start, end) of non-overlapping matches in the original line.
/// Blank query yields no ranges.
pub fn match_ranges(line: &str, query: &str) -> Vec<(usize, usize)> {
    if is_blank(query) {
        return Vec::new();
    }
    let q: Vec<char> = fold(query).into_iter().map(|t| t.0).collect();
    let h = fold(line);
    let mut out = Vec::new();
    let mut i = 0;
    while i + q.len() <= h.len() {
        if h[i..i + q.len()].iter().map(|t| t.0).eq(q.iter().copied()) {
            let (s, e) = (h[i].1, h[i + q.len() - 1].2);
            out.push((s, e));
            // resume at first folded char starting at or after e
            while i < h.len() && h[i].1 < e {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

pub fn filter<'a>(lines: &'a [String], query: &str) -> Vec<&'a String> {
    lines.iter().filter(|l| matches(l, query)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sl(l: &str, q: &str) -> Vec<String> {
        match_ranges(l, q).iter().map(|&(a, b)| l[a..b].to_string()).collect()
    }

    #[test]
    fn ascii() {
        assert!(matches("Hello World", "WORLD"));
        assert!(!matches("Hello", "xyz"));
        assert_eq!(match_ranges("Hello World", "o w"), vec![(4, 7)]);
    }
    #[test]
    fn empty_and_whitespace_match_all() {
        assert!(matches("abc", ""));
        assert!(matches("abc", "  \t"));
        assert!(matches("", ""));
        assert!(match_ranges("abc", " ").is_empty());
    }
    #[test]
    fn hebrew() {
        let l = "שלום עולם שלום";
        assert!(matches(l, "עולם"));
        assert_eq!(match_ranges(l, "שלום"), vec![(0, 8), (18, 26)]);
        assert_eq!(sl(l, "שלום").len(), 2);
    }
    #[test]
    fn multiple() {
        assert_eq!(match_ranges("ab AB aB", "ab"), vec![(0, 2), (3, 5), (6, 8)]);
    }
    #[test]
    fn aa_in_aaaa() {
        assert_eq!(match_ranges("aaaa", "aa"), vec![(0, 2), (2, 4)]);
        assert_eq!(match_ranges("aaa", "aa"), vec![(0, 2)]);
    }
    #[test]
    fn unicode_case_and_boundaries() {
        assert_eq!(sl("ÉCOLE école", "é"), vec!["É", "é"]);
        assert_eq!(sl("😀a😀A", "a"), vec!["a", "A"]);
        assert_eq!(sl("Straße", "ß"), vec!["ß"]);
    }
    #[test]
    fn length_changing_lowercase() {
        // 'İ' lowercases to 2 chars; ranges must stay on original boundaries.
        let l = "xİy";
        for q in ["i", "i\u{307}", "İ", "y", "xi"] {
            for (a, b) in match_ranges(l, q) {
                assert!(l.is_char_boundary(a) && l.is_char_boundary(b));
                let _ = &l[a..b];
            }
        }
        assert_eq!(sl(l, "y"), vec!["y"]);
        assert_eq!(sl(l, "İ"), vec!["İ"]);
    }
    #[test]
    fn query_longer_than_line() {
        assert!(!matches("a", "abc"));
        assert!(match_ranges("", "a").is_empty());
    }
    #[test]
    fn filter_keeps_order() {
        let v: Vec<String> = ["Err a", "ok", "err b"].iter().map(|s| s.to_string()).collect();
        let r = filter(&v, "ERR");
        assert_eq!(r, vec![&v[0], &v[2]]);
        assert_eq!(filter(&v, "").len(), 3);
        assert!(filter(&[], "x").is_empty());
    }
    #[test]
    fn adversarial_no_panic() {
        let l = "a\u{0}\n\t😀עב\u{301}e\u{301}";
        for q in ["\u{301}", "😀", "ע", "e\u{301}", "\n", "\u{0}", "zzzzzzzzzzzzzzzzzzzzzz"] {
            for (a, b) in match_ranges(l, q) {
                assert!(a < b && l.is_char_boundary(a) && l.is_char_boundary(b));
                let _ = &l[a..b];
            }
        }
        let big = "a".repeat(5000);
        assert_eq!(match_ranges(&big, "aa").len(), 2500);
    }
}
