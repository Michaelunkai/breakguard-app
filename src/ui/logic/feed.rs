//! Live feed model: turns raw log lines into transcript-style items (prose, grouped tool runs,
//! chips, user bubbles). Pure logic, no egui, so every rule is unit tested.
use crate::logs::{LineKind, LogLine};

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub tool: String,
    pub desc: String,
    pub cmd: String,
    /// First 5 output lines of the folded-in result.
    pub out: Vec<String>,
    pub failed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChipKind {
    Ticked,
    Added,
    JobStarted,
    JobEnded,
    Forecast,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    User { ts_ms: i64, text: String },
    Prose { ts_ms: i64, text: String },
    Group { ts_ms: i64, calls: Vec<Call> },
    Chip { ts_ms: i64, kind: ChipKind, text: String },
    Error { ts_ms: i64, text: String },
    Divider,
    Thinking { secs: i64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProseStyle {
    pub label_col: bool,
    pub wrap: bool,
    pub mono: bool,
}

/// Claude prose: full width, wrapped, proportional, no "claude" label column.
pub fn prose_style() -> ProseStyle {
    ProseStyle { label_col: false, wrap: true, mono: false }
}

/// Hook / system text that must never reach the feed.
pub fn is_noise(text: &str) -> bool {
    let t = text.trim_start();
    t.contains("system-reminder")
        || t.contains("PostToolUse hook")
        || t.contains("PreToolUse hook")
        || t.contains("SessionStart hook")
        || t.contains("UserPromptSubmit hook")
        || t.contains("Stop hook")
        || t.contains("SubagentStop hook")
        || t.starts_with("<command-")
        || t.starts_with("<local-command")
        || t.starts_with("<task-notification")
        || t.starts_with("<ccd")
        || t.starts_with("Base directory for this skill")
        || t.starts_with("[token-savior")
}

pub fn chip_kind(text: &str) -> Option<ChipKind> {
    let t = text.trim_start();
    let t = t.strip_prefix("RALP ").unwrap_or(t);
    let up = t.to_uppercase();
    [("TICKED", ChipKind::Ticked), ("ADDED", ChipKind::Added), ("JOB STARTED", ChipKind::JobStarted), ("JOB ENDED", ChipKind::JobEnded), ("FORECAST", ChipKind::Forecast)]
        .iter()
        .find(|(p, _)| up.starts_with(p))
        .map(|(_, k)| *k)
}

/// Human description only: drop a duplicated text after the `·` separator.
pub fn dedup_desc(plain: &str, raw: &str) -> String {
    let first = plain.split('\u{b7}').next().unwrap_or("").trim();
    if first.is_empty() {
        raw.trim().to_string()
    } else {
        first.to_string()
    }
}

/// "Ran 2 commands, a search, 1 failed" (sentence built by `group::summarize`).
pub fn group_label(calls: &[Call]) -> String {
    let items: Vec<(&str, bool)> = calls.iter().map(|c| (c.tool.as_str(), c.failed)).collect();
    crate::ui::logic::group::summarize(&items)
}

pub fn build(lines: &[LogLine]) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut seen_user = false;
    for l in lines {
        if is_noise(&l.text) {
            continue;
        }
        match l.kind {
            LineKind::User => {
                if seen_user {
                    items.push(Item::Divider);
                }
                seen_user = true;
                items.push(Item::User { ts_ms: l.ts_ms, text: l.text.clone() });
            }
            LineKind::Assistant => match chip_kind(&l.text) {
                Some(kind) => items.push(Item::Chip { ts_ms: l.ts_ms, kind, text: l.text.trim().to_string() }),
                None => items.push(Item::Prose { ts_ms: l.ts_ms, text: l.text.clone() }),
            },
            LineKind::ToolCall => {
                let raw = crate::ui::pages::lanes::row_text(l).to_string();
                let call = Call { tool: l.tool.clone(), desc: dedup_desc(&l.plain, &raw), cmd: raw, out: Vec::new(), failed: false };
                match items.last_mut() {
                    Some(Item::Group { calls, .. }) => calls.push(call),
                    _ => items.push(Item::Group { ts_ms: l.ts_ms, calls: vec![call] }),
                }
            }
            LineKind::ToolResult => {
                if let Some(Item::Group { calls, .. }) = items.last_mut() {
                    if let Some(c) = calls.last_mut() {
                        if l.plain.is_empty() {
                            c.out = l.text.lines().take(5).map(|s| s.to_string()).collect();
                        } else {
                            c.out = vec![l.plain.clone()];
                            c.failed = l.plain.starts_with("It failed");
                        }
                    }
                }
            }
            LineKind::Error => match items.last_mut() {
                Some(Item::Group { calls, .. }) => {
                    if let Some(c) = calls.last_mut() {
                        c.failed = true;
                    }
                }
                _ => items.push(Item::Error { ts_ms: l.ts_ms, text: l.text.clone() }),
            },
        }
    }
    items
}

/// Keep only errors: standalone error items and groups that hold a failed call.
pub fn errors_only(items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Error { .. } => Some(i.clone()),
            Item::Group { ts_ms, calls } => {
                let f: Vec<Call> = calls.iter().filter(|c| c.failed).cloned().collect();
                if f.is_empty() { None } else { Some(Item::Group { ts_ms: *ts_ms, calls: f }) }
            }
            _ => None,
        })
        .collect()
}

/// "Thinking 7m 23s" while a session is mid-turn and silent (>= 3 s since last line).
pub fn thinking_row(running: bool, now_ms: i64, last_ts_ms: i64) -> Option<Item> {
    if !running || last_ts_ms <= 0 || now_ms - last_ts_ms < 3000 {
        return None;
    }
    Some(Item::Thinking { secs: (now_ms - last_ts_ms) / 1000 })
}

pub fn thinking_text(secs: i64, now_ms: i64) -> String {
    let dots = ".".repeat(1 + ((now_ms / 400) % 3) as usize);
    let t = if secs >= 60 { format!("{}m {}s", secs / 60, secs % 60) } else { format!("{secs}s") };
    format!("Thinking {t}{dots}")
}

/// Time column: repeated identical stamps become blank.
pub fn collapse_times(stamps: &[String]) -> Vec<String> {
    let mut prev: Option<&String> = None;
    stamps
        .iter()
        .map(|s| {
            let out = if prev == Some(s) { String::new() } else { s.clone() };
            prev = Some(s);
            out
        })
        .collect()
}

/// Lane subtitle: first sentence of the latest claude line.
pub fn now_doing(lines: &[LogLine]) -> String {
    let Some(l) = lines.iter().rev().find(|l| l.kind == LineKind::Assistant && !is_noise(&l.text) && !l.text.trim().is_empty()) else {
        return String::new();
    };
    let t = l.text.trim();
    let b = t.as_bytes();
    let end = (0..b.len()).find(|&i| matches!(b[i], b'.' | b'!' | b'?') && b.get(i + 1).map_or(true, |n| *n == b' ')).map(|i| i + 1).unwrap_or(t.len());
    t[..end].chars().take(90).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpanKind {
    Plain,
    Code,
    Bold,
    Path,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub kind: SpanKind,
}

fn is_path(w: &str) -> bool {
    let w = w.trim_matches(|c: char| ",;:()\"'".contains(c));
    if w.len() < 3 {
        return false;
    }
    ((w.contains('/') || w.contains('\\')) && w.chars().any(|c| c.is_alphanumeric())) || [".rs", ".md", ".json", ".toml", ".mjs", ".ps1", ".py"].iter().any(|e| w.ends_with(e))
}

fn push_span(out: &mut Vec<Span>, t: &str, k: SpanKind) {
    if t.is_empty() {
        return;
    }
    if k == SpanKind::Plain {
        if let Some(last) = out.last_mut() {
            if last.kind == SpanKind::Plain {
                last.text.push_str(t);
                return;
            }
        }
    }
    out.push(Span { text: t.to_string(), kind: k });
}

fn plain_with_paths(out: &mut Vec<Span>, s: &str) {
    for w in s.split_inclusive(' ') {
        let core = w.trim_end();
        if is_path(core) {
            push_span(out, core, SpanKind::Path);
            push_span(out, &w[core.len()..], SpanKind::Plain);
        } else {
            push_span(out, w, SpanKind::Plain);
        }
    }
}

/// Markdown-lite: `inline code`, **bold**, file paths.
pub fn md_spans(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let next = match (rest.find('`'), rest.find("**")) {
            (Some(a), Some(b)) => Some(if a <= b { (a, true) } else { (b, false) }),
            (Some(a), None) => Some((a, true)),
            (None, Some(b)) => Some((b, false)),
            _ => None,
        };
        let Some((pos, is_tick)) = next else {
            plain_with_paths(&mut out, rest);
            break;
        };
        let mark = if is_tick { "`" } else { "**" };
        let after = &rest[pos + mark.len()..];
        match after.find(mark) {
            Some(end) => {
                plain_with_paths(&mut out, &rest[..pos]);
                push_span(&mut out, &after[..end], if is_tick { SpanKind::Code } else { SpanKind::Bold });
                rest = &after[end + mark.len()..];
            }
            None => {
                plain_with_paths(&mut out, rest);
                break;
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Font {
    Mono,
    Proportional,
}

/// Mono only for code, paths and commands.
pub fn font_for(k: SpanKind) -> Font {
    match k {
        SpanKind::Code | SpanKind::Path => Font::Mono,
        _ => Font::Proportional,
    }
}

/// (done, total) boxes from a ralp CHECKLIST.md.
pub fn ralp_progress(checklist: &str) -> (usize, usize) {
    let on = ["[", "x", "]"].concat();
    let on_up = on.to_uppercase();
    let off = "[ ]";
    let mut done = 0;
    let mut total = 0;
    for l in checklist.lines() {
        let t = l.trim_start().trim_start_matches(|c| c == '-' || c == '*').trim_start();
        if t.starts_with(&on) || t.starts_with(&on_up) {
            done += 1;
            total += 1;
        } else if t.starts_with(off) {
            total += 1;
        }
    }
    (done, total)
}

/// Pill shown when the user scrolled up and lines arrived below.
pub fn new_lines_pill(scrolled_up: bool, new_lines: usize) -> Option<String> {
    if scrolled_up && new_lines > 0 {
        Some(format!("{new_lines} new line{}", if new_lines == 1 { "" } else { "s" }))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ln(kind: LineKind, tool: &str, text: &str, plain: &str, ts: i64) -> LogLine {
        LogLine { ts_ms: ts, kind, tool: tool.into(), text: text.into(), plain: plain.into() }
    }
    fn call(tool: &str, failed: bool) -> Call {
        Call { tool: tool.into(), desc: "d".into(), cmd: "c".into(), out: vec![], failed }
    }

    #[test]
    fn live_prose_full_width() {
        let s = prose_style();
        assert!(!s.label_col && !s.mono);
        let it = build(&[ln(LineKind::Assistant, "", "Fixed the bug.", "", 1)]);
        assert!(matches!(&it[0], Item::Prose { text, .. } if text == "Fixed the bug."));
    }

    #[test]
    fn live_groups_tool_runs() {
        let it = build(&[
            ln(LineKind::ToolCall, "Bash", "Bash: ls", "Lists files", 1),
            ln(LineKind::ToolResult, "", "a\nb", "", 2),
            ln(LineKind::ToolCall, "Bash", "Bash: pwd", "Prints dir", 3),
            ln(LineKind::ToolCall, "Grep", "Grep: x", "Searches", 4),
        ]);
        assert_eq!(it.len(), 1);
        let Item::Group { calls, .. } = &it[0] else { panic!() };
        assert_eq!(group_label(calls), "Ran 2 commands, a search");
    }

    #[test]
    fn live_hides_result_rows() {
        let it = build(&[ln(LineKind::ToolCall, "Bash", "Bash: ls", "Lists", 1), ln(LineKind::ToolResult, "", "one\ntwo", "", 2)]);
        assert_eq!(it.len(), 1);
        let Item::Group { calls, .. } = &it[0] else { panic!() };
        assert_eq!(calls[0].out, vec!["one", "two"]);
    }

    #[test]
    fn live_failed_tag() {
        let it = build(&[ln(LineKind::ToolCall, "Bash", "Bash: x", "Runs", 1), ln(LineKind::Error, "", "boom", "", 2)]);
        let Item::Group { calls, .. } = &it[0] else { panic!() };
        assert!(calls[0].failed);
        assert_eq!(group_label(&[call("Bash", true), call("Bash", false)]), "Ran 2 commands, 1 failed");
        assert_eq!(group_label(&[call("Read", false), call("Edit", false), call("Agent", false)]), "Read a file, edited a file, started a helper");
    }

    #[test]
    fn live_dedup_description() {
        assert_eq!(dedup_desc("Reads a.rs  \u{b7}  a.rs", "a.rs"), "Reads a.rs");
        assert_eq!(dedup_desc("", "ls -la"), "ls -la");
    }

    #[test]
    fn live_thinking_row() {
        assert_eq!(thinking_row(true, 443_000, 0), None);
        assert_eq!(thinking_row(false, 443_000, 1000), None);
        assert_eq!(thinking_row(true, 2000, 1000), None);
        assert_eq!(thinking_row(true, 444_000, 1000), Some(Item::Thinking { secs: 443 }));
        assert_eq!(thinking_text(443, 0), "Thinking 7m 23s.");
        assert_ne!(thinking_text(5, 0), thinking_text(5, 400));
    }

    #[test]
    fn live_prose_wraps() {
        assert!(prose_style().wrap);
    }

    #[test]
    fn live_time_collapse() {
        let v = collapse_times(&["10:01".into(), "10:01".into(), "10:02".into(), "10:02".into(), "10:01".into()]);
        assert_eq!(v, vec!["10:01", "", "10:02", "", "10:01"]);
    }

    #[test]
    fn live_header_now_doing() {
        let l = [ln(LineKind::Assistant, "", "Old one. more", "", 1), ln(LineKind::Assistant, "", "Writing feed.rs now. Then tests.", "", 2), ln(LineKind::ToolCall, "Bash", "Bash: x", "", 3)];
        assert_eq!(now_doing(&l), "Writing feed.rs now.");
        assert_eq!(now_doing(&[]), "");
    }

    #[test]
    fn live_group_expand_detail() {
        let lines: Vec<String> = (1..=8).map(|i| format!("l{i}")).collect();
        let it = build(&[ln(LineKind::ToolCall, "Bash", "Bash: cargo test", "Runs tests", 1), ln(LineKind::ToolResult, "", &lines.join("\n"), "", 2)]);
        let Item::Group { calls, .. } = &it[0] else { panic!() };
        assert_eq!(calls[0].cmd, "cargo test");
        assert_eq!(calls[0].out.len(), 5);
        assert_eq!(calls[0].out[4], "l5");
    }

    #[test]
    fn live_markdown_lite() {
        let s = md_spans("Run `cargo test` on **src/ui/logic/feed.rs** ok");
        assert!(s.iter().any(|x| x.kind == SpanKind::Code && x.text == "cargo test"));
        assert!(s.iter().any(|x| x.kind == SpanKind::Bold && x.text == "src/ui/logic/feed.rs"));
        let p = md_spans("edit src/main.rs today");
        assert!(p.iter().any(|x| x.kind == SpanKind::Path && x.text == "src/main.rs"));
        assert_eq!(md_spans("plain words")[0].kind, SpanKind::Plain);
    }

    #[test]
    fn live_user_bubble() {
        let it = build(&[ln(LineKind::User, "", "do it", "", 1), ln(LineKind::Assistant, "", "ok.", "", 2)]);
        assert!(matches!(&it[0], Item::User { text, .. } if text == "do it"));
        assert!(matches!(&it[1], Item::Prose { .. }));
    }

    #[test]
    fn live_ralp_chips() {
        assert_eq!(chip_kind("TICKED box 3"), Some(ChipKind::Ticked));
        assert_eq!(chip_kind("RALP JOB ENDED lbtest"), Some(ChipKind::JobEnded));
        assert_eq!(chip_kind("JOB STARTED x"), Some(ChipKind::JobStarted));
        assert_eq!(chip_kind("ADDED box 12"), Some(ChipKind::Added));
        assert_eq!(chip_kind("FORECAST 00:49"), Some(ChipKind::Forecast));
        assert_eq!(chip_kind("hello"), None);
        let it = build(&[ln(LineKind::Assistant, "", "TICKED box 3", "", 1)]);
        assert!(matches!(it[0], Item::Chip { kind: ChipKind::Ticked, .. }));
    }

    #[test]
    fn live_filters_hook_noise() {
        let it = build(&[
            ln(LineKind::Assistant, "", "PostToolUse hook additional context: x", "", 1),
            ln(LineKind::User, "", "<system-reminder>be good</system-reminder>", "", 2),
            ln(LineKind::Assistant, "", "real text", "", 3),
        ]);
        assert_eq!(it.len(), 1);
    }

    #[test]
    fn live_filters_stop_hook_task_notification_skill_dump() {
        let it = build(&[
            ln(LineKind::User, "", "Stop hook feedback:\n[node x]: RALP", "", 1),
            ln(LineKind::User, "", "<task-notification><task-id>1</task-id></task-notification>", "", 2),
            ln(LineKind::User, "", "Base directory for this skill: C:\\x", "", 3),
            ln(LineKind::User, "", "please fix the bug", "", 4),
        ]);
        assert_eq!(it.len(), 1);
        assert!(matches!(&it[0], Item::User { text, .. } if text == "please fix the bug"));
    }

    #[test]
    fn live_real_transcript_lines_hide_stop_hook_box() {
        let raw = [
            r#"{"type":"user","timestamp":"2026-11-23T10:00:00Z","message":{"role":"user","content":"fix it"}}"#,
            r#"{"type":"assistant","timestamp":"2026-11-23T10:00:01Z","message":{"content":[{"type":"text","text":"on it"}]}}"#,
            r#"{"type":"user","timestamp":"2026-11-23T10:00:02Z","message":{"role":"user","content":"Stop hook feedback:\nRALP: keep working"}}"#,
            r#"{"type":"user","timestamp":"2026-11-23T10:00:03Z","message":{"role":"user","content":[{"type":"text","text":"<task-notification>done</task-notification>"}]}}"#,
        ];
        let lines: Vec<LogLine> = raw.iter().flat_map(|l| crate::logs::parse_record(l)).collect();
        let it = build(&lines);
        assert!(it.iter().all(|i| !matches!(i, Item::User { text, .. } if text.contains("hook") || text.contains("task-notification"))), "{it:?}");
        assert!(it.iter().any(|i| matches!(i, Item::User { text, .. } if text == "fix it")));
    }

    #[test]
    fn live_error_filter() {
        let it = build(&[
            ln(LineKind::Assistant, "", "hi", "", 1),
            ln(LineKind::ToolCall, "Bash", "Bash: a", "A", 2),
            ln(LineKind::ToolCall, "Bash", "Bash: b", "B", 3),
            ln(LineKind::Error, "", "bad", "", 4),
        ]);
        let e = errors_only(&it);
        assert_eq!(e.len(), 1);
        let Item::Group { calls, .. } = &e[0] else { panic!("{e:?}") };
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].cmd, "b");
        let orphan = build(&[ln(LineKind::Error, "", "orphan", "", 5)]);
        assert_eq!(errors_only(&orphan).len(), 1);
    }

    #[test]
    fn live_new_lines_pill() {
        assert_eq!(new_lines_pill(true, 3), Some("3 new lines".into()));
        assert_eq!(new_lines_pill(true, 1), Some("1 new line".into()));
        assert_eq!(new_lines_pill(false, 3), None);
        assert_eq!(new_lines_pill(true, 0), None);
    }

    #[test]
    fn live_font_roles() {
        assert_eq!(font_for(SpanKind::Code), Font::Mono);
        assert_eq!(font_for(SpanKind::Path), Font::Mono);
        assert_eq!(font_for(SpanKind::Plain), Font::Proportional);
        assert_eq!(font_for(SpanKind::Bold), Font::Proportional);
    }

    #[test]
    fn live_ralp_progress() {
        let t = format!("# list\n- [{}] one\n- [ ] two\n[{}] three\nnote\n- [ ] four\n", 'x', 'x');
        assert_eq!(ralp_progress(&t), (2, 4));
        assert_eq!(ralp_progress(""), (0, 0));
    }

    #[test]
    fn live_turn_spacing() {
        let it = build(&[
            ln(LineKind::User, "", "a", "", 1),
            ln(LineKind::Assistant, "", "r1", "", 2),
            ln(LineKind::User, "", "b", "", 3),
            ln(LineKind::Assistant, "", "r2", "", 4),
        ]);
        assert_eq!(it.iter().filter(|i| **i == Item::Divider).count(), 1);
        assert!(matches!(it[2], Item::Divider));
    }

    #[test]
    fn feed_append_does_not_reflow_other_lanes() {
        use crate::logs::SessionLog;
        use crate::ui::logic::scroll::FollowState;
        let mut a = SessionLog::new("a", "A");
        let mut b = SessionLog::new("b", "B");
        for i in 0..5 {
            b.push(LogLine::new(LineKind::Assistant, &format!("b{i}")));
        }
        let b_items = |log: &SessionLog| build(&log.iter().cloned().collect::<Vec<_>>());
        let b_rows_before = b_items(&b);
        let (b_total_before, b_held_before, b_dropped_before) = (b.total_lines, b.iter().count(), b.dropped);
        let mut a_follow = FollowState::default();
        a_follow.on_scroll(100.0, 1000.0, true);
        assert!(!a_follow.follow);
        for i in 0..50 {
            a.push(LogLine::new(LineKind::Assistant, &format!("a{i}")));
        }
        assert_eq!(a.total_lines, 50);
        assert_eq!(b_items(&b), b_rows_before);
        assert_eq!((b.total_lines, b.iter().count(), b.dropped), (b_total_before, b_held_before, b_dropped_before));
        assert!(!a_follow.follow);
        assert_eq!(new_lines_pill(a_follow.show_jump_button(), a.total_lines as usize), Some("50 new lines".to_string()));
        a_follow.jump_to_live();
        assert!(a_follow.follow);
        assert_eq!(new_lines_pill(a_follow.show_jump_button(), a.total_lines as usize), None);
    }

    #[test]
    fn feed_failed_result_renders_plain_sentence() {
        let mut res = ln(LineKind::ToolResult, "", "boom\nmore", "It failed: boom", 2);
        res.plain = "It failed: boom".into();
        let it = build(&[ln(LineKind::ToolCall, "Bash", "Bash: x", "Runs", 1), res]);
        let Item::Group { calls, .. } = &it[0] else { panic!() };
        assert!(calls[0].failed);
        assert_eq!(calls[0].out, vec!["It failed: boom"]);
    }

    #[test]
    fn feed_groups_use_summarize() {
        let calls = [call("Bash", false), call("Bash", true), call("Grep", false), call("Write", false), call("WebFetch", false)];
        let items: Vec<(&str, bool)> = calls.iter().map(|c| (c.tool.as_str(), c.failed)).collect();
        assert_eq!(group_label(&calls), crate::ui::logic::group::summarize(&items));
        assert_eq!(group_label(&calls), "Ran 2 commands, created a file, a search, a web lookup, 1 failed");
    }
}
