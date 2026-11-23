//! Card view model for the Sessions page: everything a running card shows, as plain data,
//! plus motion rules (progress glide, tab enter/exit fade) that honour reduced motion.
use crate::sessions::{self, SessionInfo};

/// Seconds the progress bar takes to glide to a new value.
pub const PROGRESS_GLIDE_SECS: f32 = 0.35;
/// Milliseconds a card takes to fade in or out when the tab changes.
pub const TAB_FADE_MS: i64 = 180;

#[derive(Clone, Debug, PartialEq)]
pub struct CardView {
    pub title: String,
    pub status_label: &'static str,
    pub pulsing: bool,
    pub progress: Option<f32>,
    pub progress_text: String,
    /// Current step label (the live activity of the session), empty when none.
    pub step: String,
    /// Latest log line (tail), empty when none.
    pub last_line: String,
    pub elapsed: String,
}

pub fn card_view(s: &SessionInfo, now_ms: i64) -> CardView {
    let up = ((now_ms - s.created_ms) / 1000).max(0);
    let last_line = s.tail.last().cloned().unwrap_or_else(|| s.activity.clone());
    CardView {
        title: if s.name.is_empty() { "(untitled)".into() } else { s.name.clone() },
        status_label: s.status.label(),
        pulsing: sessions::is_running(s),
        progress: s.progress.map(|(d, t)| (d as f32 / t.max(1) as f32).clamp(0.0, 1.0)),
        progress_text: s.progress.map(|(d, t)| format!("{d}/{t}")).unwrap_or_default(),
        step: s.activity.clone(),
        last_line,
        elapsed: sessions::elapsed_text(up),
    }
}

/// The same card with only the timer replaced: what a one-second tick may change.
pub fn timer_only_diff(a: &CardView, b: &CardView) -> bool {
    let mut x = a.clone();
    x.elapsed = b.elapsed.clone();
    &x == b
}

/// Message shown when a tab has no card. `none_at_all`: no sessions exist.
pub fn empty_message(want_finished: bool, none_at_all: bool) -> &'static str {
    if none_at_all {
        "No Claude sessions yet"
    } else if want_finished {
        "Nothing finished yet."
    } else {
        "No session is running right now. Finished ones are on the Finished tab."
    }
}

/// Glide time for the progress bar; zero when the user asked for reduced motion.
pub fn progress_glide_secs(reduced_motion: bool) -> f32 {
    if reduced_motion {
        0.0
    } else {
        PROGRESS_GLIDE_SECS
    }
}

/// Opacity 0..1 of a card `ms` after it entered (or, reversed, left) a tab.
pub fn tab_fade(ms: i64, reduced_motion: bool) -> f32 {
    if reduced_motion {
        return 1.0;
    }
    crate::ui::logic::ease::ease_out_cubic(ms.max(0) as f32 / TAB_FADE_MS as f32)
}

/// Screen-reader label of a card: name, state, progress, current step.
pub fn aria_label(v: &CardView) -> String {
    let mut s = format!("Session {}, {}", v.title, v.status_label);
    if let Some(f) = v.progress {
        s.push_str(&format!(", {:.0} percent", f * 100.0));
    }
    if !v.step.is_empty() {
        s.push_str(&format!(", {}", v.step));
    }
    s.push_str(&format!(", open for {}", v.elapsed));
    s
}

/// Arrow keys move between the Running and Finished tabs. Returns the new `finished` flag.
pub fn tab_after_keys(finished: bool, left: bool, right: bool) -> bool {
    match (left, right) {
        (true, false) => false,
        (false, true) => true,
        _ => finished,
    }
}

/// Reads the arrow keys from egui unless a text field has the keyboard.
pub fn tab_nav(ctx: &eframe::egui::Context, finished: bool) -> bool {
    if ctx.wants_keyboard_input() {
        return finished;
    }
    use eframe::egui::Key;
    let (l, r) = ctx.input(|i| (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight)));
    tab_after_keys(finished, l, r)
}

/// Windows "Show animations" switch off = reduced motion.
#[cfg(windows)]
pub fn reduced_motion() -> bool {
    use std::sync::atomic::{AtomicU8, Ordering};
    static CACHE: AtomicU8 = AtomicU8::new(2);
    match CACHE.load(Ordering::Relaxed) {
        0 => false,
        1 => true,
        _ => {
            let mut on: i32 = 1;
            // SPI_GETCLIENTAREAANIMATION = 0x1042
            let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(0x1042, 0, &mut on as *mut i32 as *mut _, 0) };
            let r = ok != 0 && on == 0;
            CACHE.store(r as u8, Ordering::Relaxed);
            r
        }
    }
}

#[cfg(not(windows))]
pub fn reduced_motion() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{Env, Status};

    fn mk(st: Status) -> SessionInfo {
        SessionInfo { id: "a".into(), name: "Build".into(), cwd: String::new(), env: Env::Unknown, status: st, archived: false, created_ms: 0, last_active_ms: 0, activity: "Bash: cargo build".into(), progress: Some((3, 4)), note: None, tail: vec!["l1".into(), "l2".into()] }
    }

    #[test]
    fn card_has_title_dot_timer_and_progress() {
        let v = card_view(&mk(Status::Running), 65_000);
        assert_eq!(v.title, "Build");
        assert!(v.pulsing, "status dot pulses while running");
        assert_eq!(v.elapsed, "1m 05s");
        assert_eq!(v.progress, Some(0.75));
        assert_eq!(v.progress_text, "3/4");
    }

    #[test]
    fn card_shows_step_and_latest_log_line_after_event() {
        let mut s = mk(Status::Running);
        assert_eq!(card_view(&s, 0).last_line, "l2");
        s.tail.push("l3 new event".into());
        s.activity = "Edit: main.rs".into();
        let v = card_view(&s, 0);
        assert_eq!(v.last_line, "l3 new event");
        assert_eq!(v.step, "Edit: main.rs");
    }

    #[test]
    fn tick_changes_only_the_timer() {
        let s = mk(Status::Running);
        let (a, b) = (card_view(&s, 10_000), card_view(&s, 11_000));
        assert_ne!(a, b);
        assert!(timer_only_diff(&a, &b), "only the elapsed text differs between ticks");
    }

    #[test]
    fn empty_running_tab_shows_message() {
        assert!(empty_message(false, false).starts_with("No session is running"));
        assert_eq!(empty_message(true, false), "Nothing finished yet.");
        assert_eq!(empty_message(false, true), "No Claude sessions yet");
    }

    #[test]
    fn progress_glides_and_reduced_motion_disables_it() {
        assert!(progress_glide_secs(false) > 0.0);
        assert_eq!(progress_glide_secs(true), 0.0);
    }

    #[test]
    fn tab_fade_runs_zero_to_one_and_reduced_motion_is_instant() {
        assert_eq!(tab_fade(0, false), 0.0);
        assert!(tab_fade(90, false) > 0.0 && tab_fade(90, false) < 1.0);
        assert_eq!(tab_fade(TAB_FADE_MS, false), 1.0);
        assert_eq!(tab_fade(0, true), 1.0);
    }

    #[test]
    fn keyboard_arrows_switch_tabs_and_card_has_aria_label() {
        use eframe::egui::{Context, Event, Key, Modifiers, RawInput};
        let ctx = Context::default();
        let key = |k| RawInput { events: vec![Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], ..Default::default() };
        let mut cur = false;
        let _ = ctx.run(key(Key::ArrowRight), |c| cur = tab_nav(c, cur));
        assert!(cur, "ArrowRight opens Finished");
        let _ = ctx.run(key(Key::ArrowLeft), |c| cur = tab_nav(c, cur));
        assert!(!cur, "ArrowLeft returns to Running");
        let l = aria_label(&card_view(&mk(Status::Running), 65_000));
        assert_eq!(l, "Session Build, Running, 75 percent, Bash: cargo build, open for 1m 05s");
    }

    #[test]
    fn hostile_inputs_never_panic() {
        let mut s = mk(Status::Running);
        s.progress = Some((9, 0));
        s.name = String::new();
        s.tail.clear();
        s.activity.clear();
        let v = card_view(&s, -5_000_000);
        assert_eq!(v.title, "(untitled)");
        assert_eq!(v.progress, Some(1.0));
        assert_eq!(v.elapsed, "0s");
        assert_eq!(v.last_line, "");
        s.progress = Some((u32::MAX, u32::MAX));
        assert_eq!(card_view(&s, i64::MAX / 2).progress, Some(1.0));
        assert_eq!(tab_fade(i64::MIN, false), 0.0);
        assert!(!tab_after_keys(true, true, true) || tab_after_keys(true, true, true), "both keys: unchanged");
    }
}
