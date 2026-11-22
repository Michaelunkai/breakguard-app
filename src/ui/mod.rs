//! screens. OWNER: Frontend Engineer.
pub mod creatures;
pub mod logic;
pub mod icons;
pub mod pages;
pub mod parts;
pub mod theme;
pub mod widgets;

use crate::engine::{self, Action, Engine, EngineView, OffMode, Phase};
use crate::sys::{self, MonitorInfo};
use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Dashboard,
    Breaks,
    Stats,
    Sessions,
    Monitors,
    Schedule,
    Night,
    Log,
    Settings,
    Debug,
}

pub const NAV: [(Page, &str, icons::Icon); 9] = [
    (Page::Dashboard, "Dashboard", icons::Icon::Home),
    (Page::Breaks, "Breaks", icons::Icon::Cup),
    (Page::Stats, "Stats", icons::Icon::Chart),
    (Page::Sessions, "Sessions", icons::Icon::Sessions),
    (Page::Monitors, "Monitors", icons::Icon::Monitor),
    (Page::Schedule, "Schedule", icons::Icon::Calendar),
    (Page::Night, "Night mode", icons::Icon::Moon),
    (Page::Log, "Log", icons::Icon::List),
    (Page::Settings, "Settings", icons::Icon::Gear),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ok,
    Warn,
    Err,
    Info,
}

/// Night overlay lifetime; the app re-issues it so a crash cannot leave it up for long.
const NIGHT_SECS: u32 = 900;

/// What to put on screen for a break.
#[derive(Debug, PartialEq, Eq)]
pub struct BreakScreen {
    pub secs: u32,
    pub dim: bool,
    pub label: String,
    pub tip: String,
}

/// Pure plan for the break screen: `None` for Notify style or when there is no screen to cover.
pub fn plan_break_screen(style: engine::BreakStyle, has_targets: bool, secs: u32, long: bool, tip: &str) -> Option<BreakScreen> {
    if style == engine::BreakStyle::Notify || !has_targets {
        return None;
    }
    Some(BreakScreen {
        // one extra second so the overlay never ends before the break does
        secs: secs + 1,
        dim: style == engine::BreakStyle::Dim,
        label: if long { "Long break" } else { "Break time" }.to_string(),
        tip: tip.to_string(),
    })
}


#[cfg(test)]
mod tests {
    #[test]
    fn nav_has_nine_unique_pages_for_ctrl_1_to_9() {
        assert_eq!(super::NAV.len(), 9);
        for (i, (p, _, _)) in super::NAV.iter().enumerate() {
            assert!(super::NAV.iter().skip(i + 1).all(|(q, _, _)| q != p), "duplicate page");
        }
    }

    use super::*;
    use engine::BreakStyle::*;

    #[test]
    fn break_screen_plan_per_style() {
        assert_eq!(plan_break_screen(Notify, true, 20, false, "t"), None);
        assert_eq!(plan_break_screen(Blackout, false, 20, false, "t"), None);
        let b = plan_break_screen(Blackout, true, 20, false, "Drink").unwrap();
        assert_eq!((b.secs, b.dim, b.label.as_str(), b.tip.as_str()), (21, false, "Break time", "Drink"));
        let d = plan_break_screen(Dim, true, 300, true, "").unwrap();
        assert_eq!((d.secs, d.dim, d.label.as_str()), (301, true, "Long break"));
    }
}

/// Work done off the UI thread.
/// Sidebar row: selected fill, hover tint, icon, label and an optional error count pill.
/// The accent bar is drawn separately by `nav_indicator` so it can slide between rows.
fn nav_row(ui: &mut egui::Ui, icon: icons::Icon, label: &str, sel: bool, badge: u64) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, sel, label));
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let inner = rect.shrink2(egui::vec2(theme::SP_S, 2.0));
    let pal = theme::pal();
    let fill = if sel { theme::mix(pal.surface, theme::accent(), 0.16) } else { theme::mix(pal.bg, pal.surface_hi, t) };
    let p = ui.painter();
    p.rect_filled(inner, theme::rad(theme::R_S), fill);
    let col = if sel { theme::accent_hi() } else { theme::mix(pal.text_dim, pal.text, t) };
    icons::paint(p, icon, egui::pos2(inner.left() + 22.0, inner.center().y), 17.0, col);
    let text_col = if sel { pal.text } else { col };
    p.text(egui::pos2(inner.left() + 42.0, inner.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.5), text_col);
    if badge > 0 {
        let txt = logic::badge::label(badge);
        let w = if txt.len() > 1 { 26.0 } else { 18.0 };
        let pill = egui::Rect::from_center_size(egui::pos2(inner.right() - 14.0, inner.center().y), egui::vec2(w, 16.0));
        let danger = if theme::is_light() { theme::LIGHT_DANGER } else { theme::DANGER_HI };
        p.rect_filled(pill, theme::rad(8), danger);
        p.text(pill.center(), egui::Align2::CENTER_CENTER, txt, egui::FontId::proportional(11.0), pal.bg);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Cumulative count of failed-action lines in the activity log. Reads only bytes added since
/// the last scan. Starts at EOF so historical errors do not show as new. Truncation resets.
fn scan_log_errors(path: &std::path::Path, off: &mut u64, total: &mut u64) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return };
    let Ok(len) = f.metadata().map(|m| m.len()) else { return };
    if len < *off {
        *off = 0;
    }
    if len == *off || f.seek(SeekFrom::Start(*off)).is_err() {
        return;
    }
    let mut buf = Vec::new();
    if f.take(len - *off).read_to_end(&mut buf).is_err() {
        return;
    }
    // Consume only complete lines; a partial trailing line is picked up next scan.
    let Some(end) = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else { return };
    let text = String::from_utf8_lossy(&buf[..end]);
    *total += text.lines().filter(|l| pages::log::is_error_line(l)).count() as u64;
    *off += end as u64;
}

/// Toast kind mapping from the app's toast kinds to the toasts API kinds.
fn toast_kind(k: Kind) -> parts::toasts::ToastKind {
    match k {
        Kind::Ok => parts::toasts::ToastKind::Success,
        Kind::Warn => parts::toasts::ToastKind::Warn,
        Kind::Err => parts::toasts::ToastKind::Error,
        Kind::Info => parts::toasts::ToastKind::Info,
    }
}

enum Job {
    Off { targets: Vec<(String, usize)>, mins: u32, hardware: bool },
    On { targets: Vec<String> },
    Refresh,
    Night { level: u8, devs: Vec<String> },
    Tone(crate::sysx::Tone),
    /// Break overlay on the break screens: headline, tip and live countdown.
    Break { targets: Vec<String>, secs: u32, dim: bool, label: String, tip: String },
    Quit { devs: Vec<String> },
}

enum Done {
    Toast(Kind, String),
    Monitors(Vec<MonitorInfo>),
    SleepEnded(String),
    Quit,
}

struct Toast {
    text: String,
    kind: Kind,
    until: Instant,
    born: Instant,
}

pub struct App {
    pub engine: Engine,
    pub page: Page,
    /// Live per-session log model; its thread only polls while the Log page is open.
    pub log_hub: std::sync::Arc<std::sync::Mutex<crate::logs::LogHub>>,
    pub log_wanted: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub monitors: Vec<MonitorInfo>,
    pub monitors_loaded: bool,
    /// Monitor choice of the Breaks page quick-start card.
    pub quick_target: engine::BreakTarget,
    /// Quick-start card: also pause agents for this break.
    pub quick_pause: bool,
    /// Agents BreakGuard holds suspended right now.
    pub pauser: crate::agents::Pauser,
    /// Running agents (refreshed every 2 s while the Breaks page is open).
    pub agents: Vec<crate::agents::Agent>,
    pub agents_at: Option<Instant>,
    /// The current pause was started by a manual screen-off, not a break.
    pub pause_from_off: bool,
    pub sleeping: bool,
    pub confirm_sleep: bool,
    pub confirm_quit: bool,
    pub log_cache: (Instant, String),
    pub sched_form: pages::schedule::Form,
    /// Manual screen-off "start in" delay in seconds (0 = now); not saved, so a stray click never delays.
    pub off_delay: u32,
    jobs: Sender<Job>,
    urgent: Sender<Job>,
    night_sent: Option<(u8, Instant)>,
    done: Receiver<Done>,
    done_tx: Sender<Done>,
    toasts: Vec<Toast>,
    dirty_since: Option<Instant>,
    last_refresh: Instant,
    refresh_pending: bool,
    break_targets: Vec<String>,
    quitting: Option<Instant>,
    allow_close: bool,
    demo: Option<Instant>,
    last_size: Option<egui::Vec2>,
    env_checked: Instant,
    tray: Option<crate::tray::Tray>,
    /// Window hidden to the tray.
    hidden: bool,
    /// First frame: apply "start minimized" once.
    first_frame: bool,
    /// BREAKGUARD_WINDOW_POS target; the window is created hidden and shown here on the first frame.
    window_pos: Option<[f32; 2]>,
    /// Frames left in which our window is forced to the BREAKGUARD_WINDOW_POS target (physical pixels).
    pos_wait: u8,
    start_hidden: bool,
    pub sessions_rx: Option<Receiver<crate::sessions::Snapshot>>,
    pub sessions_snapshot: Option<crate::sessions::Snapshot>,
    pub sess_query: String,
    pub sess_chips: std::collections::HashSet<crate::sessions::Status>,
    pub sess_pinned: std::collections::HashSet<String>,
    pub sess_show_archived: bool,
    pub sess_notify: bool,
    pub tip_draft: String,
    pub confirm_reset_stats: bool,
    pub confirm_reset_settings: bool,
    /// Log page tab: 0 live lanes, 1 activity, 2 sessions.
    pub log_tab: usize,
    pub lanes: pages::lanes::LaneUi,
    last_title: String,
    /// Page shown last frame; a change restarts the content fade.
    last_page: Page,
    page_changed: Instant,
    /// Error-line badge state: byte offset scanned in the log, cumulative error count, last scan time.
    log_off: u64,
    err_total: u64,
    err_scan: Instant,
    badges: logic::badge::Badges,
    /// BREAKGUARD_DEMO_TOASTS=1 (screenshots only): start time of the four demo toasts.
    demo_toasts: Option<Instant>,
}

/// Move this process's visible top-level window to physical pixel (x, y) when it is not already there.
fn force_window_pos(x: i32, y: i32) {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, SetWindowPos, GW_OWNER, SWP_NOACTIVATE, SWP_NOSIZE,
        SWP_NOZORDER,
    };
    struct Found {
        pid: u32,
        hwnd: HWND,
    }
    unsafe extern "system" fn each(h: HWND, l: LPARAM) -> BOOL {
        let f = &mut *(l as *mut Found);
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, &mut pid);
        if pid == f.pid && IsWindowVisible(h) != 0 && GetWindow(h, GW_OWNER).is_null() {
            f.hwnd = h;
            return 0;
        }
        1
    }
    unsafe {
        let mut f = Found { pid: std::process::id(), hwnd: std::ptr::null_mut() };
        EnumWindows(Some(each), &mut f as *mut Found as LPARAM);
        if f.hwnd.is_null() {
            return;
        }
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(f.hwnd, &mut r) != 0 && ((r.left - x).abs() > 3 || (r.top - y).abs() > 3) {
            SetWindowPos(f.hwnd, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

fn worker(rx: Receiver<Job>, tx: Sender<Done>) {
    let mut night_on: Vec<String> = Vec::new();
    for job in rx {
        match job {
            Job::Off { targets, mins, hardware } => {
                for (dev, idx) in targets {
                    let mut used_hw = false;
                    if hardware {
                        let r = crate::blackout::hardware_off(&dev, mins);
                        engine::log_line(&format!(
                            "hardware off {dev} for {mins} min: {}, {}, mode before {:?}, monitor answers {:?}",
                            if r.accepted { "off command accepted" } else { "off command rejected" },
                            if r.helper_armed { "helper armed" } else { "no helper" },
                            r.mode_before,
                            r.answers
                        ));
                        used_hw = r.accepted;
                        if !r.accepted {
                            let _ = tx.send(Done::Toast(
                                Kind::Warn,
                                format!("Monitor {idx} refused hardware off; using blackout instead"),
                            ));
                        }
                    }
                    if !used_hw {
                        if crate::blackout::blackout(&dev, mins) {
                            engine::log_line(&format!("blackout screen {idx} for {mins} min"));
                            let _ = tx.send(Done::Toast(Kind::Ok, format!("Screen {idx} off for {mins} min")));
                        } else {
                            let _ = tx.send(Done::Toast(Kind::Err, format!("Could not turn off screen {idx}")));
                        }
                    } else {
                        let _ = tx.send(Done::Toast(Kind::Ok, format!("Screen {idx} off for {mins} min")));
                    }
                }
            }
            Job::On { targets } => {
                for dev in targets {
                    crate::blackout::cancel(&dev);
                }
                engine::log_line("blackout ended");
                let _ = tx.send(Done::Toast(Kind::Ok, "Screens back on".into()));
            }
            Job::Refresh => {
                let _ = tx.send(Done::Monitors(sys::monitors()));
            }
            Job::Night { level, devs } => {
                if sys::is_mock() {
                    sys::mock_log(&format!("night overlay level {level}"));
                } else if level > 0 {
                    for d in std::mem::take(&mut night_on) {
                        crate::blackout::cancel(&d);
                    }
                    for d in &devs {
                        if !crate::blackout::night_tint(d, level, NIGHT_SECS) {
                            let _ = tx.send(Done::Toast(Kind::Err, "Could not start night mode".into()));
                            break;
                        }
                    }
                    night_on = devs;
                } else {
                    for d in std::mem::take(&mut night_on) {
                        crate::blackout::cancel(&d);
                    }
                }
            }
            Job::Tone(t) => crate::sysx::play_tone(t),
            Job::Break { targets, secs, dim, label, tip } => {
                for dev in targets {
                    if !crate::blackout::break_overlay(&dev, secs, dim, &label, &tip) {
                        let _ = tx.send(Done::Toast(Kind::Err, "Could not start the break screen".into()));
                    }
                }
            }
            Job::Quit { devs } => {
                for d in std::mem::take(&mut night_on).into_iter().chain(devs) {
                    crate::blackout::cancel(&d);
                }
                let _ = tx.send(Done::Quit);
            }
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, demo: bool, window_pos: Option<[f32; 2]>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        let settings = engine::Settings::load();
        let log_wanted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let woke = crate::agents::recover_on_start(); // BreakGuard died while agents were paused: wake them
        if woke > 0 {
            engine::log_line(&format!("resumed {woke} agent process(es) left suspended by an earlier run"));
        }
        theme::install_fonts(&cc.egui_ctx);
        theme::set_accent(settings.accent);
        theme::set_light(settings.light_theme);
        theme::apply(&cc.egui_ctx);
        cc.egui_ctx.set_zoom_factor(settings.ui_scale.clamp(90, 150) as f32 / 100.0);
        let engine = Engine::new(settings, Box::new(engine::SystemClock));
        let (jobs, jobs_rx) = channel();
        let (done_tx, done) = channel();
        let tx2 = done_tx.clone();
        std::thread::spawn(move || worker(jobs_rx, tx2));
        let (urgent, urgent_rx) = channel();
        let tx3 = done_tx.clone();
        std::thread::spawn(move || worker(urgent_rx, tx3));
        let _ = jobs.send(Job::Refresh);
        let mut app = App {
            engine,
            // BREAKGUARD_START_PAGE=1..9 opens that page, "gallery" the widget gallery (screenshots, QA).
            page: match std::env::var("BREAKGUARD_START_PAGE").ok().as_deref() {
                Some("gallery") => Page::Debug,
                v => v.and_then(|v| v.parse::<usize>().ok()).and_then(|n| NAV.get(n.wrapping_sub(1))).map_or(Page::Dashboard, |n| n.0),
            },
            monitors: Vec::new(),
            monitors_loaded: false,
            quick_target: engine::BreakTarget::Both,
            quick_pause: false,
            pauser: crate::agents::Pauser::default(),
            agents: Vec::new(),
            agents_at: None,
            pause_from_off: false,
            sleeping: false,
            confirm_sleep: false,
            confirm_quit: false,
            log_cache: (Instant::now() - Duration::from_secs(60), String::new()),
            sched_form: Default::default(),
            off_delay: 0,
            jobs,
            urgent,
            night_sent: None,
            done,
            done_tx,
            toasts: Vec::new(),
            dirty_since: None,
            last_refresh: Instant::now(),
            refresh_pending: true,
            break_targets: Vec::new(),
            quitting: None,
            allow_close: false,
            demo: demo.then(Instant::now),
            last_size: None,
            env_checked: Instant::now() - Duration::from_secs(60),
            tray: None,
            hidden: false,
            first_frame: true,
            window_pos,
            pos_wait: 0,
            start_hidden: std::env::args().any(|a| a == "--autostart" || a == "--minimized"),
            sessions_rx: Some(crate::sessions::spawn(cc.egui_ctx.clone())),
            log_hub: crate::logs::spawn_hub(cc.egui_ctx.clone(), log_wanted.clone()),
            log_wanted,
            sessions_snapshot: None,
            sess_query: String::new(),
            sess_chips: Default::default(),
            sess_pinned: Default::default(),
            sess_show_archived: false,
            sess_notify: true,
            tip_draft: String::new(),
            confirm_reset_stats: false,
            confirm_reset_settings: false,
            log_tab: if std::env::var("BREAKGUARD_LOG_TAB").is_ok() { 2 } else { 0 },
            lanes: Default::default(),
            last_title: String::new(),
            last_page: Page::Dashboard,
            page_changed: Instant::now(),
            log_off: std::fs::metadata(pages::log::log_path()).map(|m| m.len()).unwrap_or(0),
            err_total: 0,
            err_scan: Instant::now() - Duration::from_secs(60),
            badges: logic::badge::Badges::default(),
            demo_toasts: std::env::var("BREAKGUARD_DEMO_TOASTS").is_ok_and(|v| v == "1").then(Instant::now),
        };
        app.engine.set_stats(crate::stats::Stats::load());
        app.tray = crate::tray::Tray::start(cc.egui_ctx.clone(), app.engine.settings());
        app.start_hidden |= app.engine.settings().start_minimized;
        if app.engine.settings().night_enabled {
            app.engine.apply_settings_changed();
        }
        // Screenshots/QA only: BREAKGUARD_START_BREAK=1 opens straight into a running break.
        if std::env::var("BREAKGUARD_START_BREAK").is_ok() {
            app.engine.start_break_now();
        }
        app
    }

    pub fn toast(&mut self, kind: Kind, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), kind, until: Instant::now() + Duration::from_secs(4), born: Instant::now() });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// First-run card: choose a preset (or keep the numbers) and start the timer.
    pub fn finish_onboarding(&mut self, preset: Option<usize>) {
        crate::logic::apply_onboarding(self.engine.settings_mut(), preset);
        self.engine.apply_settings_changed();
        self.engine.start_break_timer();
        self.mark_dirty();
        self.toast(Kind::Ok, "Timer started. Take care of your eyes!");
    }

    pub fn toggle_pause(&mut self) {
        if self.engine.is_paused() {
            self.engine.resume();
            self.toast(Kind::Ok, "Timer resumed");
        } else {
            self.engine.pause();
            self.toast(Kind::Info, "Timer paused");
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_since.get_or_insert_with(Instant::now);
    }

    pub fn view(&self) -> EngineView {
        self.engine.view()
    }

    pub fn is_selected(&self, name: &str) -> bool {
        self.engine.settings().selected.iter().any(|s| s == name)
    }

    pub fn toggle_selected(&mut self, name: &str) {
        let s = &mut self.engine.settings_mut().selected;
        if let Some(p) = s.iter().position(|x| x == name) {
            s.remove(p);
        } else {
            s.push(name.to_string());
        }
        self.mark_dirty();
    }

    /// Selected monitors that still exist; falls back to the primary one.
    pub fn selected_or_primary(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .monitors
            .iter()
            .filter(|m| self.is_selected(&m.name))
            .map(|m| m.name.clone())
            .collect();
        if v.is_empty() {
            v.extend(self.monitors.iter().filter(|m| m.primary).map(|m| m.name.clone()));
        }
        if v.is_empty() {
            v.extend(self.monitors.first().map(|m| m.name.clone()));
        }
        v
    }

    /// Monitor device names for a break target (Mon1/Mon2 = screen number, Both = all).
    pub fn target_devs(&self, t: engine::BreakTarget) -> Vec<String> {
        let n = |i: usize| self.monitors.get(i).map(|m| m.name.clone());
        match t {
            engine::BreakTarget::Mon1 => n(0).into_iter().collect(),
            engine::BreakTarget::Mon2 => n(1).into_iter().collect(),
            engine::BreakTarget::Both => self.monitors.iter().map(|m| m.name.clone()).collect(),
        }
    }

    pub fn screen_number(&self, name: &str) -> usize {
        self.monitors.iter().position(|m| m.name == name).map(|p| p + 1).unwrap_or(1)
    }

    /// Manual screen-off for exactly `secs` seconds (1 s .. 24 h).
    pub fn off_now_secs(&mut self, targets: Vec<String>, secs: u32) {
        if targets.is_empty() {
            self.toast(Kind::Warn, "No monitor to turn off");
            return;
        }
        let acts = self.engine.off_now_secs(targets, secs);
        self.dispatch(acts);
    }

    /// The remembered manual screen-off length in seconds.
    pub fn off_secs(&self) -> u32 {
        self.engine.settings().off_secs
    }

    /// Exact-length editor bound to the remembered screen-off length (saved when changed).
    pub fn off_len_edit(&mut self, ui: &mut eframe::egui::Ui, id: &str) {
        let mut v = self.off_secs();
        if pages::duration_edit(ui, id, &mut v) {
            self.engine.settings_mut().off_secs = v;
            self.mark_dirty();
        }
    }

    /// "Start in" editor bound to the manual screen-off delay.
    pub fn off_delay_edit(&mut self, ui: &mut eframe::egui::Ui, id: &str) {
        let mut v = self.off_delay;
        if pages::delay_edit(ui, id, &mut v) {
            self.off_delay = v;
        }
    }

    /// Turn `targets` off for the chosen length, starting after the chosen delay (now when 0).
    pub fn off_chosen(&mut self, targets: Vec<String>) {
        let secs = self.off_secs();
        self.off_after_delay(targets, secs);
    }

    /// Turn `targets` off for `secs`, starting after the chosen delay (now when 0).
    pub fn off_after_delay(&mut self, targets: Vec<String>, secs: u32) {
        if targets.is_empty() {
            self.toast(Kind::Warn, "No monitor to turn off");
            return;
        }
        let delay = self.off_delay;
        if delay == 0 {
            self.off_now_secs(targets, secs);
        } else {
            let acts = self.engine.off_in_secs(targets, delay, secs);
            self.dispatch(acts);
            self.toast(Kind::Ok, format!("Screens turn off in {} for {}", engine::fmt_duration(delay), engine::fmt_duration(secs)));
        }
    }

    /// Button text for `off_chosen`: "Turn off for 1 min" or "Turn off in 10 min for 1 h".
    pub fn off_chosen_label(&self) -> String {
        let len = engine::fmt_duration(self.off_secs());
        if self.off_delay == 0 { format!("Turn off for {len}") } else { format!("Turn off in {} for {len}", engine::fmt_duration(self.off_delay)) }
    }

    /// Waiting (delayed) screen-offs with a Cancel button each.
    pub fn delayed_off_rows(&mut self, ui: &mut eframe::egui::Ui) {
        let rows = self.engine.view().delayed_off;
        let mut cancel = None;
        for (i, (targets, at, secs)) in rows.iter().enumerate() {
            ui.horizontal(|ui| {
                let names: Vec<String> = targets.iter().map(|t| format!("screen {}", self.screen_number(t))).collect();
                widgets::status_pill(ui, &format!("{} off at {} for {}", names.join(", "), pages::fmt_hm(*at), engine::fmt_duration(*secs)), widgets::PillKind::Info);
                if widgets::ghost_button(ui, "Cancel").on_hover_text("Forget this waiting screen-off").clicked() {
                    cancel = Some(i);
                }
            });
        }
        if let Some(i) = cancel {
            self.engine.cancel_delayed(i);
        }
    }

    pub fn turn_on(&mut self, targets: Vec<String>) {
        let _ = self.urgent.send(Job::On { targets });
    }

    /// Re-read the running agents (cheap: a few small files plus liveness checks).
    pub fn refresh_agents(&mut self, force: bool) {
        if !force && self.agents_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        self.agents = crate::agents::discover(&crate::sessions::roots(), &crate::agents::process_alive);
        self.agents_at = Some(Instant::now());
    }

    /// Pause the chosen agents (all when none are chosen).
    pub fn pause_agents_now(&mut self) -> usize {
        self.refresh_agents(true);
        let sel = self.engine.settings().pause_sel.clone();
        let list: Vec<crate::agents::Agent> = self.agents.iter().filter(|a| sel.is_empty() || sel.contains(&a.session_id)).cloned().collect();
        self.pauser.pause(&list)
    }

    pub fn resume_agents_now(&mut self) {
        let n = self.pauser.resume_all();
        self.pause_from_off = false;
        if n > 0 {
            self.toast(Kind::Ok, format!("Resumed {n} agent(s)"));
        }
    }

    /// Per-agent Break / Continue.
    pub fn toggle_agent(&mut self, a: &crate::agents::Agent) {
        if self.pauser.is_paused(&a.session_id) {
            self.pauser.resume_session(&a.session_id);
        } else {
            self.pauser.pause(std::slice::from_ref(a));
        }
    }

    /// End one running screen-off now.
    pub fn end_off(&mut self, targets: Vec<String>) {
        let acts = self.engine.end_off(&targets);
        if acts.is_empty() {
            self.turn_on(targets);
        } else {
            self.dispatch(acts);
        }
    }

    /// Add `secs` to one running screen-off.
    pub fn extend_off(&mut self, targets: Vec<String>, secs: u32) {
        let acts = self.engine.extend_off(&targets, secs);
        self.dispatch(acts);
        self.toast(Kind::Info, format!("Screen-off extended by {} min", secs / 60));
    }

    pub fn restore_all(&mut self) {
        let acts = self.engine.restore_all();
        self.dispatch(acts);
    }

    pub fn start_sleep(&mut self) {
        if self.sleeping {
            return;
        }
        self.sleeping = true;
        self.engine.sleep_now();
        self.toast(Kind::Info, "Going to sleep");
    }

    pub fn dispatch(&mut self, acts: Vec<Action>) {
        for a in acts {
            match a {
                Action::ScreenOff { targets, mins } => self.run_off(targets, mins, false),
                Action::ScreenOn { targets } => {
                    let _ = self.urgent.send(Job::On { targets });
                    if self.pause_from_off && self.engine.view().active_off.is_empty() {
                        self.resume_agents_now();
                    }
                }
                Action::Warn { secs_left } => {
                    self.tone(crate::sysx::Tone::Warn);
                    self.toast(Kind::Warn, format!("Break in {secs_left} s"));
                    if self.hidden {
                        self.balloon("Break soon", &format!("Break in {secs_left} s"));
                    }
                }
                Action::BreakStart { secs } => {
                    if self.engine.settings().pause_agents {
                        let n = self.pause_agents_now();
                        if n > 0 {
                            self.toast(Kind::Info, format!("Paused {n} agent(s) for the break"));
                        }
                    }
                    let targets = self.selected_or_primary();
                    self.break_targets = targets.clone();
                    let (long, cycle) = {
                        let v = self.engine.view();
                        (v.break_is_long, v.cycle)
                    };
                    let tip = if self.engine.settings().tips {
                        crate::tips::pick(&self.engine.settings().custom_tips, cycle, long)
                    } else {
                        String::new()
                    };
                    crate::break_exec::start(crate::break_exec::configured(), secs);
                    self.tone(crate::sysx::Tone::BreakStart);
                    self.show_break_screen(secs, long, &tip);
                    if self.hidden || self.engine.settings().break_style == engine::BreakStyle::Notify {
                        self.balloon(if long { "Long break" } else { "Break time" }, if tip.is_empty() { "Rest your eyes" } else { &tip });
                    }
                    let head = if long { "Long break" } else { "Break time" };
                    self.toast(Kind::Info, if tip.is_empty() { head.to_string() } else { format!("{head}: {tip}") });
                }
                Action::BreakEnd => {
                    if self.pauser.any_paused() && !self.pause_from_off {
                        self.resume_agents_now();
                    }
                    let t = std::mem::take(&mut self.break_targets);
                    if !t.is_empty() && self.engine.settings().break_style != engine::BreakStyle::Notify {
                        let _ = self.urgent.send(Job::On { targets: t });
                    }
                    crate::break_exec::end();
                    self.tone(crate::sysx::Tone::BreakEnd);
                    self.toast(Kind::Ok, "Break over");
                }
                Action::SleepPc { mins } => {
                    self.sleeping = true;
                    let tx = self.done_tx.clone();
                    std::thread::spawn(move || {
                        let msg = match sys::sleep_and_resume(mins) {
                            sys::SleepOutcome::Returned { secs, timer_fired } => {
                                engine::log_line(&format!(
                                    "sleep for {mins} min: back after {secs} s, timer fired {timer_fired}"
                                ));
                                "PC woke up".to_string()
                            }
                            sys::SleepOutcome::Failed(e) => format!("Sleep failed: {e}"),
                        };
                        let _ = tx.send(Done::SleepEnded(msg));
                    });
                }
                Action::NightSet { level } => {
                    self.night_sent = (level > 0).then(|| (level, Instant::now()));
                    let devs = self.monitors.iter().map(|m| m.name.clone()).collect();
                    let _ = self.jobs.send(Job::Night { level, devs });
                }
                Action::Micro(k) => {
                    self.tone(crate::sysx::Tone::Warn);
                    self.toast(Kind::Info, engine::micro_text(k));
                    if self.hidden {
                        self.balloon("BreakGuard", engine::micro_text(k));
                    }
                }
                Action::Notify(s) => {
                    if s.starts_with("Away detected") {
                        self.tone(crate::sysx::Tone::Welcome);
                    }
                    self.toast(Kind::Info, s)
                }
            }
        }
    }

    /// Tray balloon notice (no-op without a tray).
    fn balloon(&self, title: &str, text: &str) {
        if let Some(t) = &self.tray {
            t.notify(title, text);
        }
    }

    /// Run commands coming from the tray menu and global hotkeys.
    fn handle_tray(&mut self, ctx: &egui::Context) {
        use crate::tray::Cmd;
        let Some(tray) = &self.tray else { return };
        tray.set_paused(self.engine.is_paused());
        tray.set_night(self.engine.view().night_active);
        let cmds: Vec<Cmd> = tray.rx.try_iter().collect();
        for c in cmds {
            match c {
                Cmd::Show => {
                    self.hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Cmd::BreakNow => {
                    self.engine.start_break_now();
                    self.toast(Kind::Info, "Starting a break");
                }
                Cmd::TogglePause => {
                    if self.engine.is_paused() {
                        self.engine.resume();
                        self.toast(Kind::Ok, "Timer resumed");
                    } else {
                        self.engine.pause();
                        self.toast(Kind::Info, "Timer paused");
                    }
                }
                Cmd::ToggleNight => {
                    self.engine.night_toggle();
                    self.mark_dirty();
                    let on = self.engine.settings().night_force == 1;
                    self.toast(Kind::Info, if on { "Night mode on" } else { "Night mode off" });
                }
                Cmd::Snooze => {
                    self.engine.snooze_default();
                    let m = self.engine.settings().snooze_mins;
                    self.toast(Kind::Info, format!("Snoozed for {m} min"));
                }
                Cmd::Quit => self.begin_quit(),
            }
        }
    }

    /// Play a tone on the worker thread when sound is on.
    fn tone(&mut self, t: crate::sysx::Tone) {
        if self.engine.settings().sound {
            let _ = self.jobs.send(Job::Tone(t));
        }
    }

    /// Put the break screen up for the style in settings (Notify = nothing on screen).
    fn show_break_screen(&mut self, secs: u32, long: bool, tip: &str) {
        let style = self.engine.settings().break_style;
        if let Some(p) = plan_break_screen(style, !self.break_targets.is_empty(), secs, long, tip) {
            let _ = self.jobs.send(Job::Break {
                targets: self.break_targets.clone(),
                secs: p.secs,
                dim: p.dim,
                label: p.label,
                tip: p.tip,
            });
        }
    }

    /// Add `secs` to the running break and redraw the break screen with the new end.
    pub fn extend_break(&mut self, secs: u32) {
        self.engine.extend_break(secs);
        let (left, long, cycle) = match self.engine.view() {
            v @ EngineView { phase: Phase::Break { .. }, .. } => {
                let l = if let Phase::Break { left } = v.phase { left } else { 0 };
                (l, v.break_is_long, v.cycle)
            }
            _ => return,
        };
        let tip = if self.engine.settings().tips {
            crate::tips::pick(&self.engine.settings().custom_tips, cycle, long)
        } else {
            String::new()
        };
        self.show_break_screen(left, long, &tip);
        self.toast(Kind::Info, format!("Break extended by {secs} s"));
    }

    fn run_off(&mut self, names: Vec<String>, mins: u32, is_break: bool) {
        let targets: Vec<(String, usize)> =
            names.into_iter().map(|n| { let i = self.screen_number(&n); (n, i) }).collect();
        let hardware = self.engine.settings().off_mode == OffMode::Hardware && !is_break;
        let _ = self.jobs.send(Job::Off { targets, mins, hardware });
    }

    fn needs_quit_confirm(&self) -> bool {
        let v = self.engine.view();
        !v.active_off.is_empty() || !matches!(v.phase, Phase::Off) || self.sleeping
    }

    fn begin_quit(&mut self) {
        if self.quitting.is_some() {
            return;
        }
        self.restore_all();
        self.pauser.resume_all();
        if let Some(t) = &self.tray {
            t.remove();
        }
        let _ = self.jobs.send(Job::Night { level: 0, devs: Vec::new() });
        let devs = self.monitors.iter().map(|m| m.name.clone()).collect();
        let _ = self.urgent.send(Job::Quit { devs });
        self.quitting = Some(Instant::now());
        self.save_now();
    }

    fn save_now(&mut self) {
        self.dirty_since = None;
        self.engine.apply_settings_changed();
        if let Some(t) = &self.tray {
            t.reconfigure(self.engine.settings());
        }
        if !self.engine.settings().save() {
            self.toast(Kind::Err, "Could not save settings");
        }
    }

    fn drain(&mut self) {
        while let Ok(d) = self.done.try_recv() {
            match d {
                Done::Toast(k, t) => self.toast(k, t),
                Done::Monitors(m) => {
                    self.monitors = m;
                    self.monitors_loaded = true;
                    self.refresh_pending = false;
                }
                Done::SleepEnded(t) => {
                    self.sleeping = false;
                    self.toast(Kind::Info, t);
                }
                Done::Quit => self.allow_close = true,
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();
        self.log_wanted.store(self.page == Page::Log, std::sync::atomic::Ordering::Relaxed);
        if self.env_checked.elapsed() >= Duration::from_secs(1) {
            self.env_checked = Instant::now();
            let busy = self.engine.settings().dnd_fullscreen && crate::sysx::fullscreen_busy();
            self.engine.set_env(crate::sysx::idle_secs(), busy);
        }
        let acts = self.engine.tick();
        self.dispatch(acts);
        if self.engine.stats().is_dirty() {
            self.engine.stats_mut().save();
        }
        self.handle_tray(ctx);
        if self.first_frame {
            self.first_frame = false;
            if let Some(p) = self.window_pos {
                // Move first, then show: the window never appears on the primary monitor.
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(p[0], p[1])));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                self.pos_wait = 40;
            }
            if self.start_hidden && !self.demo.is_some() {
                if self.tray.is_some() {
                    self.hidden = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
        } else if self.pos_wait > 0 {
            // BREAKGUARD_WINDOW_POS (test hook): in some launches the logical position is applied as physical
            // pixels, Windows then clamps the straddling window back into the primary monitor. For 2 s, place
            // our own window in physical pixels whenever it is not at the target.
            self.pos_wait -= 1;
            if let Some(p) = self.window_pos {
                let ppp = ctx.pixels_per_point();
                force_window_pos((p[0] * ppp).round() as i32, (p[1] * ppp).round() as i32);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }

        if !self.refresh_pending && self.last_refresh.elapsed() > Duration::from_secs(6) {
            self.refresh_pending = true;
            self.last_refresh = Instant::now();
            let _ = self.jobs.send(Job::Refresh);
        }
        if let Some(t) = self.dirty_since {
            if t.elapsed() > Duration::from_millis(700) {
                self.save_now();
            }
        }
        if let Some((lvl, at)) = self.night_sent {
            if at.elapsed() > Duration::from_secs(NIGHT_SECS as u64 - 60) {
                self.dispatch(vec![Action::NightSet { level: lvl }]);
            }
        }
        self.toasts.retain(|t| t.until > Instant::now());

        // close handling
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.engine.settings().close_to_tray && self.tray.is_some() && !self.quitting.is_some() {
                self.hidden = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.balloon("BreakGuard is still running", "Click the tray icon to open it. Right-click for the menu.");
            } else if self.needs_quit_confirm() {
                self.confirm_quit = true;
            } else {
                self.begin_quit();
            }
        }
        if let Some(t) = self.quitting {
            if self.allow_close || t.elapsed() > Duration::from_secs(5) {
                if let Some(s) = self.last_size {
                    crate::save_window_size(s.x, s.y);
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
            self.last_size = Some(r.size());
        }

        // demo tour: cycle pages for 6 s, then exit
        if let Some(start) = self.demo {
            let el = start.elapsed().as_secs_f32();
            let n = NAV.len();
            self.page = NAV[((el / 6.0 * n as f32) as usize).min(n - 1)].0;
            if el >= 6.0 {
                self.demo = None;
                self.begin_quit();
            }
        }

        // keyboard: Ctrl+1..9 pages, Ctrl+B break now, Ctrl+P pause, Ctrl+Shift+D gallery
        let (mut want_break, mut want_pause) = (false, false);
        ctx.input(|i| {
            let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5,
                        egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9];
            if i.modifiers.command {
                for (k, (p, _, _)) in keys.iter().zip(NAV.iter()) {
                    if i.key_pressed(*k) {
                        self.page = *p;
                    }
                }
                if i.key_pressed(egui::Key::B) {
                    want_break = true;
                }
                if i.key_pressed(egui::Key::P) {
                    want_pause = true;
                }
                if i.modifiers.shift && i.key_pressed(egui::Key::D) {
                    self.page = Page::Debug;
                }
            }
        });

        if want_break {
            self.engine.start_break_now();
            self.toast(Kind::Info, "Starting a break");
        }
        if want_pause {
            self.toggle_pause();
        }
        self.poll_sessions();
        let title = crate::logic::window_title(self.engine.view().phase, self.engine.is_paused());
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if self.err_scan.elapsed() > Duration::from_secs(2) {
            self.err_scan = Instant::now();
            scan_log_errors(&pages::log::log_path(), &mut self.log_off, &mut self.err_total);
        }
        if self.page == Page::Log {
            self.badges.mark_seen("log", self.err_total);
        }
        if self.page != self.last_page {
            self.last_page = self.page;
            self.page_changed = Instant::now();
        }

        egui::SidePanel::left("nav").resizable(false).exact_width(196.0).show(ctx, |ui| {
            ui.add_space(theme::SP_M);
            ui.horizontal(|ui| {
                ui.add_space(theme::SP_S);
                let (r, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
                let brand = theme::link_accent(theme::is_light());
                ui.painter().circle_filled(r.center(), 11.0, theme::mix(theme::pal().surface_hi, brand, 0.35));
                icons::paint(ui.painter(), icons::Icon::Moon, r.center(), 15.0, brand);
                ui.label(egui::RichText::new("BreakGuard").size(20.0).strong().color(theme::pal().text));
            });
            ui.add_space(theme::SP_M);
            let mut sel_rect: Option<egui::Rect> = None;
            // Keep ~130 px for the timer and Break now button below; the nav scrolls when the window is short.
            let nav_h = (ui.available_height() - 130.0).max(80.0);
            egui::ScrollArea::vertical().id_salt("nav_scroll").max_height(nav_h).auto_shrink([false, true]).show(ui, |ui| {
                for (p, label, icon) in NAV {
                    let sel = self.page == p;
                    let badge = if p == Page::Log { self.badges.unseen("log", self.err_total) } else { 0 };
                    let resp = nav_row(ui, icon, label, sel, badge);
                    if sel {
                        sel_rect = Some(resp.rect);
                    }
                    if resp.clicked() {
                        self.page = p;
                    }
                }
            });
            if let Some(r) = sel_rect {
                let target = r.top() + 8.0;
                let h = r.height() - 16.0;
                let y = parts::nav_indicator::indicator_y(ctx, "nav_indicator", target);
                if !parts::nav_indicator::is_settled(y, target) {
                    ctx.request_repaint();
                }
                parts::nav_indicator::paint_indicator(ui.painter(), r, y, h, theme::accent());
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(theme::SP_S);
                ui.horizontal(|ui| {
                    ui.add_space(theme::SP_M);
                    ui.label(egui::RichText::new(format!("v{}", crate::VERSION)).size(12.0).color(theme::pal().text_dim));
                });
                ui.add_space(theme::SP_XS);
                ui.horizontal(|ui| {
                    ui.add_space(theme::SP_S);
                    if widgets::primary_button(ui, "Break now").on_hover_text("Start a break right now (Ctrl+B)").clicked() {
                        self.engine.start_break_now();
                    }
                });
                // every tab: schedule a screen-off "x from now, for y" without leaving the page
                ui.horizontal(|ui| {
                    ui.add_space(theme::SP_S);
                    let pop = ui.make_persistent_id("break_in_popup");
                    let btn = widgets::ghost_button(ui, "Break in...").on_hover_text("Start a break X from now for Y: pick the start time and the length");
                    static BREAKIN_DEMO: std::sync::Once = std::sync::Once::new();
                    if crate::sys::is_mock() && std::env::var_os("BREAKGUARD_BREAKIN_DEMO").is_some() {
                        BREAKIN_DEMO.call_once(|| ui.memory_mut(|m| m.open_popup(pop)));
                    }
                    if btn.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(pop));
                    }
                    egui::popup::popup_above_or_below_widget(ui, pop, &btn, egui::AboveOrBelow::Above, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                        ui.set_min_width(360.0);
                        ui.label(egui::RichText::new("Start in (from now)").color(theme::pal().text_dim));
                        self.off_delay_edit(ui, "side-delay");
                        ui.label(egui::RichText::new("Break length").color(theme::pal().text_dim));
                        self.off_len_edit(ui, "side");
                        if widgets::primary_button(ui, &self.off_chosen_label().replace("Turn off", "Start break")).on_hover_text("Turn your chosen screens off after the Start in time, for the chosen length").clicked() {
                            let t = self.selected_or_primary();
                            self.off_chosen(t);
                            ui.memory_mut(|m| m.close_popup());
                        }
                        self.delayed_off_rows(ui);
                    });
                });
                ui.add_space(theme::SP_S);
                let v = self.engine.view();
                let (txt, col) = match v.phase {
                    Phase::Off => ("--:--".to_string(), theme::pal().text_dim),
                    Phase::Working { left } => (pages::fmt_mmss(left), if v.paused { theme::pal().text_dim } else { theme::link_accent(theme::is_light()) }),
                    Phase::Warning { left } => (pages::fmt_mmss(left), if theme::is_light() { theme::LIGHT_WARN } else { theme::WARN }),
                    Phase::Break { left } => (pages::fmt_mmss(left), if theme::is_light() { theme::LIGHT_OK } else { theme::OK }),
                };
                let sub = match v.phase {
                    Phase::Off => "timer off",
                    Phase::Working { .. } if v.paused => "paused",
                    Phase::Working { .. } => "until break",
                    Phase::Warning { .. } => "break soon",
                    Phase::Break { .. } => "on break",
                };
                // One row (time + caption) keeps the sidebar short so it never overlaps the nav on small windows.
                ui.horizontal(|ui| {
                    ui.add_space(theme::SP_M);
                    ui.label(egui::RichText::new(txt).font(theme::mono(22.0)).color(col));
                    ui.label(egui::RichText::new(sub).size(12.0).color(theme::pal().text_dim));
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let v = self.engine.view();
            ui.horizontal(|ui| {
                let phase = match v.phase {
                    Phase::Off => "Break timer off".to_string(),
                    Phase::Working { left } => format!("Working, break in {}", pages::fmt_mmss(left)),
                    Phase::Warning { left } => format!("Break starts in {} s", left),
                    Phase::Break { left } => format!("On break, {} left", pages::fmt_mmss(left)),
                };
                let kind = match v.phase {
                    Phase::Off => widgets::PillKind::Neutral,
                    Phase::Working { .. } => widgets::PillKind::Info,
                    Phase::Warning { .. } => widgets::PillKind::Warn,
                    Phase::Break { .. } => widgets::PillKind::Ok,
                };
                widgets::status_pill(ui, &phase, kind);
                if v.paused {
                    widgets::status_pill(ui, "Paused", widgets::PillKind::Neutral);
                }
                let off = v.active_off.len();
                widgets::status_pill(ui, &format!("{off} screen(s) off"), if off > 0 { widgets::PillKind::Warn } else { widgets::PillKind::Neutral });
                if v.night_active {
                    widgets::status_pill(ui, "Night mode on", widgets::PillKind::Info);
                }
                if let Some(sn) = &self.sessions_snapshot {
                    let run = sn.live;   // same discovery as the Live tab lanes, so both counts agree
                    if run > 0 {
                        widgets::status_pill(ui, &format!("{run} Claude running"), widgets::PillKind::Info);
                    }
                }
                if sys::is_mock() {
                    widgets::status_pill(ui, "MOCK MODE", widgets::PillKind::Warn);
                }
            });
        });

        // Page change: content eases in from 12 px below over 180 ms (ease_out_cubic, same as slide transitions).
        let fade_t = logic::ease::ease_out_cubic(self.page_changed.elapsed().as_secs_f32() / (logic::ease::DURATION_MS as f32 / 1000.0));
        let fade_off = (1.0 - fade_t) * 12.0;
        if fade_t < 1.0 {
            ctx.request_repaint();
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(fade_off);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.page {
                Page::Dashboard => pages::dashboard::show(self, ui),
                Page::Breaks => pages::breaks::show(self, ui),
                Page::Stats => pages::stats::show(self, ui),
                Page::Sessions => pages::sessions::show(self, ui),
                Page::Monitors => pages::monitors::show(self, ui),
                Page::Schedule => pages::schedule::show(self, ui),
                Page::Night => pages::night::show(self, ui),
                Page::Log => pages::log::show(self, ui),
                Page::Settings => pages::settings::show(self, ui),
                Page::Debug => widgets::gallery(ui),
            });
        });

        self.dialogs(ctx);
        self.show_toasts(ctx);
        if let Some(t) = self.demo_toasts {
            parts::toasts::demo(ctx, t.elapsed().as_millis() as i64);
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

impl App {
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_sleep {
            let mins = self.engine.settings().sleep_min;
            let mut close = false;
            egui::Window::new("Put PC to sleep?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("The PC sleeps now and wakes in {mins} min. Your programs keep running."));
                    ui.horizontal(|ui| {
                        if widgets::danger_button(ui, "Sleep now").clicked() {
                            self.start_sleep();
                            close = true;
                        }
                        if widgets::ghost_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.confirm_sleep = false;
            }
        }
        if self.confirm_quit {
            let mut close = false;
            egui::Window::new("Quit BreakGuard?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Screens are off or timers are running. Quitting turns every screen back on and stops the timers.");
                    ui.horizontal(|ui| {
                        if widgets::danger_button(ui, "Quit").clicked() {
                            close = true;
                            self.begin_quit();
                        }
                        if widgets::primary_button(ui, "Stay").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.confirm_quit = false;
            }
        }
    }

    fn show_toasts(&self, ctx: &egui::Context) {
        let live = self.toasts.len().saturating_sub(parts::toasts::STACK_LIMIT);
        for (i, t) in self.toasts.iter().skip(live).enumerate() {
            let age_ms = t.born.elapsed().as_millis() as i64;
            parts::toasts::toast(ctx, &i.to_string(), i, toast_kind(t.kind), &t.text, age_ms);
            if age_ms < parts::toasts::SLIDE_MS {
                ctx.request_repaint();
            }
        }
    }

    /// Take the newest scanner snapshot; notify on finished / needs-input changes.
    fn poll_sessions(&mut self) {
        let Some(rx) = &self.sessions_rx else { return };
        let mut newest = None;
        while let Ok(s) = rx.try_recv() {
            newest = Some(s);
        }
        let Some(new) = newest else { return };
        if let Some(old) = &self.sessions_snapshot {
            if self.sess_notify && new.ok && old.ok {
                for ev in crate::sessions::diff(&old.sessions, &new.sessions) {
                    let (title, text) = match ev {
                        crate::sessions::Event::Finished(_, n) => ("Claude session finished", n),
                        crate::sessions::Event::NeedsInput(_, n) => ("Claude session needs input", n),
                    };
                    self.toast(Kind::Info, format!("{title}: {text}"));
                    self.balloon(title, &text);
                }
            }
        }
        self.sessions_snapshot = Some(new);
    }
}

#[cfg(test)]
mod window_force_tests {
    /// The test process owns no visible top-level window: forcing a position must find nothing and not panic.
    #[test]
    fn force_window_pos_without_a_window_is_a_noop() {
        super::force_window_pos(3941, 101);
        super::force_window_pos(-5000, -5000);
    }
}
