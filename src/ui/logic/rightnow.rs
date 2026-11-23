//! "Right now" sentence for a lane: one plain-English line that says what the session is doing this
//! very moment, with how long it has been at it. Pure: same input, same sentence. std-only.

use crate::logs::{LineKind, LogLine, LogState};

fn secs(now_ms: i64, ts_ms: i64) -> i64 {
    if ts_ms <= 0 { 0 } else { (now_ms.saturating_sub(ts_ms) / 1000).max(0) }
}

fn dur(s: i64) -> String {
    if s < 60 { format!("{s}s") } else if s < 3600 { format!("{}m {:02}s", s / 60, s % 60) } else { format!("{}h {:02}m", s / 3600, (s % 3600) / 60) }
}

/// Plain verb phrase of a tool call, lower-cased start: "Reads the file a.rs" -> "reading the file a.rs".
fn ing(plain: &str) -> String {
    const MAP: [(&str, &str); 13] = [
        ("Runs ", "running "), ("Reads ", "reading "), ("Edits ", "editing "), ("Writes ", "writing "),
        ("Searches ", "searching "), ("Looks ", "looking "), ("Opens ", "opening "), ("Starts ", "starting "),
        ("Updates ", "updating "), ("Uses ", "using "), ("Asks ", "asking "), ("Looks up", "looking up"), ("Lists ", "listing "),
    ];
    for (a, b) in MAP {
        if let Some(rest) = plain.strip_prefix(a) {
            return format!("{b}{rest}");
        }
    }
    plain.to_string()
}

/// A session that wrote its final answer is done: after this many seconds with the answer as the
/// last line it leaves Live. A started step (tool call, your message) may stay quiet this long.
pub const ANSWER_DONE_SECS: i64 = 8;
pub const STEP_QUIET_SECS: i64 = 600;

/// True while the session is really working. False once its process is gone, or it ended its turn
/// (answer written, nothing since) - then it belongs on the Finished tab.
pub fn is_working(state: LogState, lines: &[LogLine], now_ms: i64) -> bool {
    if state == LogState::Finished {
        return false;
    }
    let Some(last) = lines.last() else { return state == LogState::Running };
    let age = secs(now_ms, last.ts_ms);
    match last.kind {
        LineKind::Assistant => age < ANSWER_DONE_SECS,
        _ => age < STEP_QUIET_SECS,
    }
}

/// What the session is doing now. `lines` = its newest lines (oldest first).
pub fn right_now(state: LogState, lines: &[LogLine], now_ms: i64) -> String {
    if state == LogState::Finished {
        return "Finished. This session has stopped.".to_string();
    }
    if !is_working(state, lines, now_ms) && !lines.is_empty() {
        return format!("Done. Finished its work {} ago and is waiting for you.", dur(secs(now_ms, lines.last().map_or(0, |l| l.ts_ms))));
    }
    let Some(last) = lines.last() else {
        return "Just started, nothing to show yet.".to_string();
    };
    let age = secs(now_ms, last.ts_ms);
    if state == LogState::Idle {
        return format!("Waiting. Nothing new for {}. It may be waiting for you.", dur(age));
    }
    let tool_plain = |l: &LogLine| if l.plain.is_empty() { l.text.clone() } else { l.plain.clone() };
    match last.kind {
        LineKind::ToolCall => format!("Now: {} (for {})", ing(&tool_plain(last)), dur(age)),
        LineKind::ToolResult => {
            // the call that this result answers
            let call = lines.iter().rev().find(|l| l.kind == LineKind::ToolCall).map(|l| ing(&tool_plain(l)));
            match call {
                Some(c) => format!("Just finished {c}. Deciding the next step ({} ago).", dur(age)),
                None => format!("A step just finished. Deciding the next step ({} ago).", dur(age)),
            }
        }
        LineKind::Error => format!("Last step failed {} ago. It is trying another way.", dur(age)),
        LineKind::User => format!("Got a new message {} ago. Reading it and thinking.", dur(age)),
        LineKind::Assistant => format!("Now: writing its answer ({} ago).", dur(age)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const T: i64 = 1_000_000;
    fn l(kind: LineKind, plain: &str, ts: i64) -> LogLine {
        LogLine { ts_ms: ts, kind, tool: String::new(), text: "x".into(), plain: plain.into() }
    }

    #[test] fn running_call() {
        let v = [l(LineKind::ToolCall, "Reads the file main.rs", T)];
        assert_eq!(right_now(LogState::Running, &v, T + 4000), "Now: reading the file main.rs (for 4s)");
    }
    #[test] fn running_command() {
        let v = [l(LineKind::ToolCall, "Runs a command: cargo test", T)];
        assert_eq!(right_now(LogState::Running, &v, T + 75_000), "Now: running a command: cargo test (for 1m 15s)");
    }
    #[test] fn result_names_the_call() {
        let v = [l(LineKind::ToolCall, "Searches the code for \"x\"", T), l(LineKind::ToolResult, "Done, got 3 lines back", T + 1000)];
        assert_eq!(right_now(LogState::Running, &v, T + 3000), "Just finished searching the code for \"x\". Deciding the next step (2s ago).");
    }
    #[test] fn error_line() {
        let v = [l(LineKind::Error, "It failed", T)];
        assert_eq!(right_now(LogState::Running, &v, T + 9000), "Last step failed 9s ago. It is trying another way.");
    }
    #[test] fn idle_says_waiting() {
        let v = [l(LineKind::ToolCall, "Reads the file a", T)];
        assert_eq!(right_now(LogState::Idle, &v, T + 125_000), "Waiting. Nothing new for 2m 05s. It may be waiting for you.");
    }
    #[test] fn answer_written_means_done() {
        let v = [l(LineKind::Assistant, "", T)];
        assert!(is_working(LogState::Running, &v, T + 7_000));
        assert!(!is_working(LogState::Running, &v, T + 8_000));
        assert_eq!(right_now(LogState::Idle, &v, T + 125_000), "Done. Finished its work 2m 05s ago and is waiting for you.");
    }
    #[test] fn long_step_stays_working() {
        let v = [l(LineKind::ToolCall, "Runs a command", T)];
        assert!(is_working(LogState::Running, &v, T + 599_000));
        assert!(!is_working(LogState::Running, &v, T + 600_000));
        assert!(!is_working(LogState::Finished, &v, T));
    }
    #[test] fn finished_says_stopped() { assert_eq!(right_now(LogState::Finished, &[], T), "Finished. This session has stopped."); }
    #[test] fn empty_running() { assert_eq!(right_now(LogState::Running, &[], T), "Just started, nothing to show yet."); }
    #[test] fn assistant_and_user() {
        assert_eq!(right_now(LogState::Running, &[l(LineKind::Assistant, "", T)], T), "Now: writing its answer (0s ago).");
        assert_eq!(right_now(LogState::Running, &[l(LineKind::User, "", T)], T + 2000), "Got a new message 2s ago. Reading it and thinking.");
    }
    #[test] fn clock_backwards_and_unknown_ts() {
        assert_eq!(right_now(LogState::Running, &[l(LineKind::ToolCall, "Reads the file a", T)], T - 5000), "Now: reading the file a (for 0s)");
        assert_eq!(right_now(LogState::Running, &[l(LineKind::ToolCall, "Reads the file a", 0)], T), "Now: reading the file a (for 0s)");
    }
}
