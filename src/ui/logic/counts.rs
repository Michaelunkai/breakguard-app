#![allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Assistant,
    ToolCall,
    ToolResult,
    User,
    Error,
}

pub struct Line {
    pub kind: Kind,
}

#[derive(Debug, PartialEq)]
pub struct Totals {
    pub lines: u64,
    pub tools: u64,
    pub errors: u64,
}

pub fn count(lines: &[Line]) -> Totals {
    let mut t = Totals { lines: lines.len() as u64, tools: 0, errors: 0 };
    for l in lines {
        match l.kind {
            Kind::ToolCall => t.tools += 1,
            Kind::Error => t.errors += 1,
            _ => {}
        }
    }
    t
}

/// Truncating (never rounds up, so no "1000.0k").
pub fn human(n: u64) -> String {
    const UNITS: [(u64, &str); 3] = [
        (1_000_000_000, "G"),
        (1_000_000, "M"),
        (1_000, "k"),
    ];
    for (div, suf) in UNITS {
        if n >= div {
            let tenths = n / (div / 10);
            return format!("{}.{}{}", tenths / 10, tenths % 10, suf);
        }
    }
    n.to_string()
}

pub fn header_text(t: &Totals) -> String {
    format!(
        "{} lines  {} tools  {} errors",
        human(t.lines),
        human(t.tools),
        human(t.errors)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<Line> {
        use Kind::*;
        [User, Assistant, ToolCall, ToolResult, ToolCall, Error, ToolCall, ToolResult, ToolCall, Assistant, User]
            .iter()
            .map(|&kind| Line { kind })
            .collect()
    }

    #[test]
    fn count_fixture() {
        let t = count(&fixture());
        assert_eq!(t, Totals { lines: 11, tools: 4, errors: 1 });
    }

    #[test]
    fn count_empty() {
        assert_eq!(count(&[]), Totals { lines: 0, tools: 0, errors: 0 });
    }

    #[test]
    fn count_all_errors() {
        let v: Vec<Line> = (0..5).map(|_| Line { kind: Kind::Error }).collect();
        assert_eq!(count(&v), Totals { lines: 5, tools: 0, errors: 5 });
    }

    #[test]
    fn human_spec() {
        assert_eq!(human(0), "0");
        assert_eq!(human(999), "999");
        assert_eq!(human(1000), "1.0k");
        assert_eq!(human(15300), "15.3k");
        assert_eq!(human(2_500_000), "2.5M");
    }

    #[test]
    fn human_boundaries() {
        assert_eq!(human(999_999), "999.9k");
        assert_eq!(human(1_000_000), "1.0M");
        assert_eq!(human(999_999_999), "999.9M");
        assert_eq!(human(1_000_000_000), "1.0G");
        assert_eq!(human(1050), "1.0k");
        assert_eq!(human(1099), "1.0k");
        assert_eq!(human(1100), "1.1k");
    }

    #[test]
    fn human_u64_max_no_panic() {
        assert_eq!(human(u64::MAX), "18446744073.7G");
    }

    #[test]
    fn header_fixture() {
        assert_eq!(header_text(&count(&fixture())), "11 lines  4 tools  1 errors");
    }

    #[test]
    fn header_large() {
        let t = Totals { lines: 15300, tools: 1000, errors: 2_500_000 };
        assert_eq!(header_text(&t), "15.3k lines  1.0k tools  2.5M errors");
    }
}
