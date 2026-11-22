#![allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct L {
    pub ts_ms: i64,
    pub tag: String,
    pub text: String,
}

fn flatten(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(' ');
            }
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

fn hms(ts_ms: i64) -> String {
    let secs = ts_ms.div_euclid(1000).rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

pub fn copy_payload(lines: &[L], with_time: bool, range: Option<(usize, usize)>) -> String {
    let (start, end) = match range {
        Some((s, e)) => (s, e),
        None => (0, usize::MAX),
    };
    if lines.is_empty() || start > end || start >= lines.len() {
        return String::new();
    }
    let end = end.min(lines.len() - 1);
    lines[start..=end]
        .iter()
        .map(|l| {
            let body = format!("{}: {}", flatten(&l.tag), flatten(&l.text));
            if with_time {
                format!("[{}] {}", hms(l.ts_ms), body)
            } else {
                body
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(ts: i64, tag: &str, text: &str) -> L {
        L { ts_ms: ts, tag: tag.into(), text: text.into() }
    }
    fn sample() -> Vec<L> {
        vec![l(0, "a", "one"), l(3_661_000, "b", "two"), l(86_399_999, "c", "three")]
    }

    #[test]
    fn full_with_time() {
        assert_eq!(
            copy_payload(&sample(), true, None),
            "[00:00:00] a: one\n[01:01:01] b: two\n[23:59:59] c: three"
        );
    }
    #[test]
    fn full_no_time() {
        assert_eq!(copy_payload(&sample(), false, None), "a: one\nb: two\nc: three");
    }
    #[test]
    fn range_inclusive() {
        assert_eq!(copy_payload(&sample(), false, Some((1, 1))), "b: two");
        assert_eq!(copy_payload(&sample(), false, Some((0, 1))), "a: one\nb: two");
    }
    #[test]
    fn clamp_end() {
        assert_eq!(copy_payload(&sample(), false, Some((1, 99))), "b: two\nc: three");
        assert_eq!(copy_payload(&sample(), false, Some((2, usize::MAX))), "c: three");
    }
    #[test]
    fn empty_cases() {
        assert_eq!(copy_payload(&[], true, None), "");
        assert_eq!(copy_payload(&[], true, Some((0, 0))), "");
        assert_eq!(copy_payload(&sample(), true, Some((2, 1))), "");
        assert_eq!(copy_payload(&sample(), true, Some((3, 5))), "");
        assert_eq!(copy_payload(&sample(), true, Some((usize::MAX, usize::MAX))), "");
    }
    #[test]
    fn multiline_text() {
        let v = vec![l(0, "t", "x\ny\r\nz\rw\n\n")];
        assert_eq!(copy_payload(&v, false, None), "t: x y z w  ");
    }
    #[test]
    fn newline_in_tag_flattened() {
        let v = vec![l(0, "t\nu", "x")];
        assert_eq!(copy_payload(&v, false, None), "t u: x");
    }
    #[test]
    fn no_trailing_newline_and_single_line_count() {
        let out = copy_payload(&sample(), true, None);
        assert!(!out.ends_with('\n'));
        assert_eq!(out.lines().count(), 3);
    }
    #[test]
    fn negative_and_huge_ts_no_panic() {
        assert_eq!(copy_payload(&[l(-1, "a", "b")], true, None), "[23:59:59] a: b");
        assert_eq!(copy_payload(&[l(i64::MIN, "a", "b")], true, None).len(), "[00:00:00] a: b".len());
        assert_eq!(copy_payload(&[l(i64::MAX, "a", "b")], true, None).len(), "[00:00:00] a: b".len());
    }
    #[test]
    fn empty_text_and_unicode() {
        let v = vec![l(1000, "é", ""), l(0, "日", "ü\n")];
        assert_eq!(copy_payload(&v, true, None), "[00:00:01] é: \n[00:00:00] 日: ü ");
    }
}
