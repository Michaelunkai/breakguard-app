#![allow(dead_code)]
//! Plain-English one-line summary of a run of tool calls (std-only, no egui).

/// Summarize tool calls as one line, e.g. `Ran 5 commands, read a file, 1 failed`.
/// Each item is `(tool name, is_error)`. Empty input gives `""`.
pub fn summarize(calls: &[(&str, bool)]) -> String {
    if calls.is_empty() {
        return String::new();
    }
    let (mut commands, mut reads, mut edits, mut creates) = (0usize, 0usize, 0usize, 0usize);
    let (mut searches, mut helpers, mut web, mut other, mut failed) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for &(name, is_error) in calls {
        match name {
            "Bash" | "PowerShell" => commands += 1,
            "Read" => reads += 1,
            "Edit" | "MultiEdit" | "NotebookEdit" => edits += 1,
            "Write" => creates += 1,
            "Grep" | "Glob" => searches += 1,
            "Agent" | "Task" => helpers += 1,
            "WebFetch" | "WebSearch" => web += 1,
            _ => other += 1,
        }
        if is_error {
            failed += 1;
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if commands > 0 {
        parts.push(phrase("ran", commands, "a command", "commands"));
    }
    if reads > 0 {
        parts.push(phrase("read", reads, "a file", "files"));
    }
    if edits > 0 {
        parts.push(phrase("edited", edits, "a file", "files"));
    }
    if creates > 0 {
        parts.push(phrase("created", creates, "a file", "files"));
    }
    if searches > 0 {
        parts.push(phrase("", searches, "a search", "searches"));
    }
    if helpers > 0 {
        parts.push(phrase("started", helpers, "a helper", "helpers"));
    }
    if web > 0 {
        parts.push(phrase("", web, "a web lookup", "web lookups"));
    }
    if other > 0 {
        parts.push(phrase("", other, "1 other action", "other actions"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    capitalize_first(&parts.join(", "))
}

/// `verb` + count phrase. Count 1 uses `one` (with article), otherwise `"{n} {many}"`.
fn phrase(verb: &str, n: usize, one: &str, many: &str) -> String {
    let body = if n == 1 { one.to_string() } else { format!("{n} {many}") };
    if verb.is_empty() {
        body
    } else {
        format!("{verb} {body}")
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::summarize;

    #[test]
    fn summary_line_counts_bash_and_read() {
        let mut calls = vec![("Bash", false); 5];
        calls.push(("Read", false));
        assert_eq!(summarize(&calls), "Ran 5 commands, read a file");
    }

    #[test]
    fn summary_line_counts_edits_capitalised_and_plural() {
        let calls = [("Edit", false), ("MultiEdit", false), ("Read", false), ("Read", false)];
        assert_eq!(summarize(&calls), "Read 2 files, edited 2 files");
    }

    #[test]
    fn summary_line_counts_failures_last() {
        let calls = [("Bash", true), ("Bash", false), ("Read", false), ("Read", false)];
        assert_eq!(summarize(&calls), "Ran 2 commands, read 2 files, 1 failed");
    }

    #[test]
    fn summary_line_counts_singular_and_mixed_kinds() {
        let calls = [
            ("Bash", false),
            ("Write", false),
            ("Grep", false),
            ("Glob", false),
            ("Agent", false),
            ("WebSearch", false),
        ];
        assert_eq!(
            summarize(&calls),
            "Ran a command, created a file, 2 searches, started a helper, a web lookup"
        );
    }

    #[test]
    fn summary_line_counts_unknown_tool_is_other() {
        let calls = [("mcp__server__tool", false), ("Frobnicate", true), ("Frobnicate", false)];
        assert_eq!(summarize(&calls), "3 other actions, 1 failed");
        assert_eq!(summarize(&[("Zap", false)]), "1 other action");
    }

    #[test]
    fn summary_line_counts_empty_is_empty() {
        assert_eq!(summarize(&[]), "");
    }
}
