#![allow(dead_code)]
//! Tag: tool-name chip (icon + short label) for the lane log.

const MAX: usize = 8;

fn short(s: &str) -> String {
    if s.chars().count() <= MAX {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX - 1).collect();
    out.push('…');
    out
}

pub fn chip(tool: &str) -> (&'static str, String) {
    let t = tool.trim();
    if t.is_empty() {
        return ("gear", "tool".to_string());
    }
    if let Some(rest) = t.strip_prefix("mcp__") {
        let last = rest.rsplit("__").next().unwrap_or("");
        let name = if last.is_empty() { "tool" } else { last };
        return ("gear", short(name));
    }
    let icon = match t {
        "Bash" => "terminal",
        "Read" => "file",
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "pencil",
        "Grep" | "Glob" => "search",
        "WebFetch" | "WebSearch" => "globe",
        "TodoWrite" => "list",
        "Agent" | "Skill" => "robot",
        _ if t.starts_with("Task") => "list",
        _ => "gear",
    };
    (icon, short(t))
}

pub fn error_glyph() -> &'static str {
    "⚠"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ic(t: &str) -> &'static str {
        chip(t).0
    }

    #[test]
    fn mappings() {
        assert_eq!(chip("Bash"), ("terminal", "Bash".to_string()));
        assert_eq!(ic("Read"), "file");
        for t in ["Edit", "Write", "MultiEdit", "NotebookEdit"] {
            assert_eq!(ic(t), "pencil", "{t}");
        }
        assert_eq!(ic("Grep"), "search");
        assert_eq!(ic("Glob"), "search");
        assert_eq!(ic("WebFetch"), "globe");
        assert_eq!(ic("WebSearch"), "globe");
        for t in ["TodoWrite", "Task", "TaskCreate", "TaskUpdate", "TaskOutput"] {
            assert_eq!(ic(t), "list", "{t}");
        }
        assert_eq!(ic("Agent"), "robot");
        assert_eq!(ic("Skill"), "robot");
    }

    #[test]
    fn mcp() {
        assert_eq!(chip("mcp__srv__run"), ("gear", "run".to_string()));
        assert_eq!(chip("mcp__a__b__c"), ("gear", "c".to_string()));
        assert_eq!(chip("mcp__srv__"), ("gear", "tool".to_string()));
        assert_eq!(chip("mcp__"), ("gear", "tool".to_string()));
        assert_eq!(chip("mcp__srv__Bash").0, "gear");
        assert_eq!(chip("mcp__s__averylongtoolname"), ("gear", "averylo…".to_string()));
    }

    #[test]
    fn unknown_and_empty() {
        assert_eq!(chip("Frobnicate"), ("gear", "Frobnic…".to_string()));
        assert_eq!(chip(""), ("gear", "tool".to_string()));
        assert_eq!(chip("   "), ("gear", "tool".to_string()));
        assert_eq!(ic("bash"), "gear"); // case sensitive
        assert_eq!(ic("mcp_single"), "gear");
    }

    #[test]
    fn length_boundary() {
        assert_eq!(chip("12345678").1, "12345678");
        assert_eq!(chip("123456789").1, "1234567…");
        assert_eq!(chip("NotebookEdit"), ("pencil", "Noteboo…".to_string()));
        assert_eq!(chip("MultiEdit"), ("pencil", "MultiEd…".to_string()));
    }

    #[test]
    fn unicode() {
        let (_, n) = chip("ツールツールツールツール");
        assert_eq!(n, "ツールツールツ…");
        assert_eq!(chip("日本語").1, "日本語");
        let (_, n) = chip("😀😀😀😀😀😀😀😀😀");
        assert_eq!(n.chars().count(), 8);
        assert!(n.ends_with('…'));
        assert_eq!(chip("mcp__s__é").1, "é");
    }

    #[test]
    fn adversarial() {
        for t in ["\0", "\n", "Task\u{0}", "mcp__mcp__mcp__", "____", "a\u{301}a\u{301}a\u{301}a\u{301}a\u{301}"] {
            let (i, n) = chip(t);
            assert!(!i.is_empty());
            assert!(n.chars().count() <= MAX, "{t:?}");
        }
        assert_eq!(chip("Taskfoo").0, "list");
        assert_eq!(chip("Tas").0, "gear");
    }

    #[test]
    fn glyph() {
        assert_eq!(error_glyph(), "⚠");
        assert!(!error_glyph().is_empty());
    }
}
