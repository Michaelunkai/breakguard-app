#![windows_subsystem = "windows"]
mod sys;
mod diag;

use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use std::collections::BTreeMap;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ACCENT: Color32 = Color32::from_rgb(94, 224, 190);
const ACCENT_DIM: Color32 = Color32::from_rgb(28, 72, 66);
const WARN: Color32 = Color32::from_rgb(255, 196, 92);
const BAD: Color32 = Color32::from_rgb(255, 120, 120);
const BG: Color32 = Color32::from_rgb(11, 14, 20);
const SIDE: Color32 = Color32::from_rgb(8, 10, 15);
const CARD: Color32 = Color32::from_rgb(22, 28, 38);
const CARD_LINE: Color32 = Color32::from_rgb(44, 54, 70);
const FIELD: Color32 = Color32::from_rgb(33, 42, 56);
const MUTED: Color32 = Color32::from_rgb(196, 207, 222);
const TEXT: Color32 = Color32::from_rgb(238, 242, 247);
const DEFAULT_WATCH: &str = "claude.exe,codex.exe,node.exe,cursor.exe,code.exe,docker desktop.exe,ollama.exe,python.exe";

type Snap = BTreeMap<String, usize>;

fn snapshot(watch: &[String]) -> Snap {
    let out = Command::new("tasklist")
        .args(["/fo", "csv", "/nh"])
        .creation_flags(0x0800_0000)
        .output();
    let mut m = Snap::new();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let name = line.trim_start_matches('"').split('"').next().unwrap_or("").to_lowercase();
            if watch.iter().any(|w| *w == name) {
                *m.entry(name).or_insert(0) += 1;
            }
        }
    }
    m
}

fn fmt_snap(s: &Snap) -> String {
    if s.is_empty() {
        return "none of the watched programs are running".into();
    }
    s.iter().map(|(k, v)| format!("{} x{}", k.trim_end_matches(".exe"), v)).collect::<Vec<_>>().join("  ")
}

fn compare(before: &Snap, after: &Snap) -> (bool, String) {
    let gone: Vec<String> = before
        .iter()
        .filter(|(k, v)| after.get(*k).copied().unwrap_or(0) < **v)
        .map(|(k, v)| format!("{} ({} -> {})", k, v, after.get(k).copied().unwrap_or(0)))
        .collect();
    if gone.is_empty() {
        (true, "Every watched program that was running before is still listed. This only compares process names and counts, it does not show the programs are working.".into())
    } else {
        (false, format!("Fewer processes than before: {}. Check those programs.", gone.join(", ")))
    }
}

fn log_line(shared: &Arc<Mutex<Vec<String>>>, s: String) {
    let ts = {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        // local clock via offset is unavailable in std; show UTC-less elapsed marker
        format!("t+{:02}:{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
    };
    if let Ok(mut v) = shared.lock() {
        v.push(format!("{}  {}", ts, s.clone()));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("breakguard.log")) {
                let _ = writeln!(f, "{}", s);
            }
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Tab { Home, Breaks, Power, Screens, Watch, Log }

#[derive(PartialEq, Clone, Copy)]
enum Phase { Idle, Work, Brk }

struct Blackout { monitor: usize, until: Instant }

struct App {
    tab: Tab,
    // break timer
    work_min: u32,
    brk_min: u32,
    phase: Phase,
    phase_end: Instant,
    phase_total: Duration,
    focus_on_break: bool,
    before: Snap,
    verdict: Option<(bool, String)>,
    cycles: u32,
    // watch
    watch_text: String,
    live: Snap,
    last_poll: Instant,
    poll_rx: Option<std::sync::mpsc::Receiver<Snap>>,
    // sleep
    sleep_min: u32,
    sleep_confirm: Option<Instant>,
    sleep_running: Arc<AtomicBool>,
    wake_timers_ok: Option<bool>,
    // night
    night_on: Arc<AtomicBool>,
    night_end: Option<Instant>,
    night_hours: f32,
    night_screens_off: bool,
    night_started: Option<Instant>,
    // screens
    screens_min: u32,
    screens_off_until: Option<Instant>,
    monitors: Vec<sys::MonitorInfo>,
    blackouts: Vec<Blackout>,
    ddc_ack: bool,
    // keep awake during breaks
    keep_awake_flag: Arc<AtomicBool>,
    keep_awake: bool,
    log: Arc<Mutex<Vec<String>>>,
    post_resume: Arc<Mutex<Option<String>>>,
}

fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = Color32::from_rgb(9, 12, 17);
    v.faint_bg_color = CARD;
    v.override_text_color = None;
    v.widgets.noninteractive.bg_fill = CARD;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, CARD_LINE);
    v.widgets.inactive.bg_fill = FIELD;
    v.widgets.inactive.weak_bg_fill = FIELD;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, CARD_LINE);
    v.widgets.hovered.bg_fill = Color32::from_rgb(46, 58, 76);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(46, 58, 76);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.weak_bg_fill = ACCENT_DIM;
    v.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
    v.selection.bg_fill = ACCENT_DIM;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    let r = egui::CornerRadius::same(10);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    ctx.set_theme(egui::ThemePreference::Dark); ctx.set_visuals_of(egui::Theme::Dark, v.clone()); ctx.set_visuals_of(egui::Theme::Light, v);
    let mut s = (*ctx.style()).clone();
    s.spacing.item_spacing = Vec2::new(12.0, 12.0);
    s.spacing.button_padding = Vec2::new(16.0, 9.0);
    s.spacing.interact_size.y = 30.0;
    s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(30.0));
    s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
    s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
    s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(13.0));
    ctx.set_style_of(egui::Theme::Dark, s.clone()); ctx.set_style_of(egui::Theme::Light, s);
}

fn primary_btn(ui: &mut egui::Ui, text: &str, w: f32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(16.0).strong().color(Color32::from_rgb(6, 24, 20)))
        .fill(ACCENT).corner_radius(egui::CornerRadius::same(12)).min_size(Vec2::new(w, 42.0)))
}

fn ghost_btn(ui: &mut egui::Ui, text: &str, w: f32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(16.0).color(TEXT))
        .corner_radius(egui::CornerRadius::same(12)).min_size(Vec2::new(w, 42.0)))
}

fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.18)).stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.7)))
        .corner_radius(egui::CornerRadius::same(20)).inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| { ui.label(RichText::new(text).size(12.0).strong().color(color)); });
}

fn card_title(ui: &mut egui::Ui, title: &str, tag: Option<(&str, Color32)>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(20.0).strong().color(TEXT));
        if let Some((t, c)) = tag { chip(ui, t, c); }
    });
}

fn callout(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.10)).stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.45)))
        .corner_radius(egui::CornerRadius::same(10)).inner_margin(egui::Margin::same(12))
        .show(ui, |ui| { ui.set_width(ui.available_width()); ui.label(RichText::new(text).size(14.0).color(TEXT)); });
}

fn page_header(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(30.0).strong().color(TEXT));
    if !sub.is_empty() { ui.label(RichText::new(sub).size(14.0).color(MUTED)); }
    ui.add_space(6.0);
}

fn wake_timers_enabled() -> Option<bool> {
    let o = Command::new("powercfg")
        .args(["/q", "SCHEME_CURRENT", "SUB_SLEEP", "bd3b718a-0680-4d9d-8ab2-e1d2b4ac806d"])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&o.stdout).to_string();
    let ac = t.lines().find(|l| l.contains("Current AC Power Setting Index"))?;
    let v = ac.rsplit("0x").next()?.trim();
    let n = u32::from_str_radix(v, 16).ok()?;
    Some(n != 0)
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style(&cc.egui_ctx);
        Self {
            tab: Tab::Home,
            work_min: 50,
            brk_min: 10,
            phase: Phase::Idle,
            phase_end: Instant::now(),
            phase_total: Duration::from_secs(1),
            focus_on_break: true,
            before: Snap::new(),
            verdict: None,
            cycles: 0,
            watch_text: DEFAULT_WATCH.to_string(),
            live: Snap::new(),
            last_poll: Instant::now() - Duration::from_secs(60),
            poll_rx: None,
            sleep_min: 5,
            sleep_confirm: None,
            sleep_running: Arc::new(AtomicBool::new(false)),
            wake_timers_ok: wake_timers_enabled(),
            night_on: Arc::new(AtomicBool::new(false)),
            night_end: None,
            night_hours: 8.0,
            night_screens_off: true,
            night_started: None,
            screens_min: 5,
            screens_off_until: None,
            monitors: sys::monitors(),
            blackouts: vec![],
            ddc_ack: false,
            keep_awake_flag: Arc::new(AtomicBool::new(false)),
            keep_awake: false,
            log: Arc::new(Mutex::new(vec![])),
            post_resume: Arc::new(Mutex::new(None)),
        }
    }

    fn watch_list(&self) -> Vec<String> {
        self.watch_text.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()
    }

    fn log(&self, s: impl Into<String>) {
        log_line(&self.log, s.into());
    }

    fn poll_watch(&mut self) {
        if let Some(rx) = &self.poll_rx {
            if let Ok(s) = rx.try_recv() {
                self.live = s;
                self.poll_rx = None;
            }
        }
        if self.poll_rx.is_none() && self.last_poll.elapsed() > Duration::from_secs(6) {
            self.last_poll = Instant::now();
            let w = self.watch_list();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || { let _ = tx.send(snapshot(&w)); });
            self.poll_rx = Some(rx);
        }
    }

    fn start_phase(&mut self, p: Phase, mins: u32) {
        self.phase = p;
        self.phase_total = Duration::from_secs(mins as u64 * 60);
        self.phase_end = Instant::now() + self.phase_total;
    }

    fn tick_phase(&mut self, ctx: &egui::Context) {
        if self.phase == Phase::Idle || Instant::now() < self.phase_end {
            return;
        }
        match self.phase {
            Phase::Work => {
                self.before = snapshot(&self.watch_list());
                self.cycles += 1;
                self.log(format!("break {} start. running: {}", self.cycles, fmt_snap(&self.before)));
                self.verdict = None;
                self.start_phase(Phase::Brk, self.brk_min);
                sys::beep();
                if self.focus_on_break {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
                }
            }
            Phase::Brk => {
                let after = snapshot(&self.watch_list());
                let v = compare(&self.before, &after);
                self.log(format!("break {} end. {}", self.cycles, v.1));
                self.verdict = Some(v);
                self.start_phase(Phase::Work, self.work_min);
                sys::beep();
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
            }
            Phase::Idle => {}
        }
    }

    fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(egui::CornerRadius::same(16))
            .inner_margin(egui::Margin::same(22))
            .stroke(Stroke::new(1.0_f32, CARD_LINE))
            .show(ui, |ui| { ui.set_width(ui.available_width()); add(ui) })
            .inner
    }

    fn ring(ui: &mut egui::Ui, frac: f32, big: String, small: String, color: Color32) {
        let size = 250.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        let c = rect.center();
        let rad = size / 2.0 - 10.0;
        let p = ui.painter();
        p.circle_stroke(c, rad, Stroke::new(14.0_f32, Color32::from_rgb(33, 42, 56)));
        let n = 120;
        let steps = (n as f32 * frac.clamp(0.0, 1.0)) as usize;
        if steps > 1 {
            let pts: Vec<egui::Pos2> = (0..=steps)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + (i as f32 / n as f32) * std::f32::consts::TAU;
                    c + Vec2::new(a.cos(), a.sin()) * rad
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(14.0_f32, color)));
        }
        p.text(c - Vec2::new(0.0, 8.0), egui::Align2::CENTER_CENTER, big, egui::FontId::proportional(52.0), TEXT);
        p.text(c + Vec2::new(0.0, 34.0), egui::Align2::CENTER_CENTER, small, egui::FontId::proportional(15.0), MUTED);
    }

    fn ui_breaks(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Break timer", "Reminders only. It never closes, pauses or locks anything.");
        let left = self.phase_end.saturating_duration_since(Instant::now());
        let (frac, big, small, color) = match self.phase {
            Phase::Idle => (0.0, format!("{:02}:00", self.work_min), "ready".to_string(), ACCENT),
            Phase::Work => (
                1.0 - left.as_secs_f32() / self.phase_total.as_secs_f32().max(1.0),
                format!("{:02}:{:02}", left.as_secs() / 60, left.as_secs() % 60),
                "focus time".to_string(),
                ACCENT,
            ),
            Phase::Brk => (
                1.0 - left.as_secs_f32() / self.phase_total.as_secs_f32().max(1.0),
                format!("{:02}:{:02}", left.as_secs() / 60, left.as_secs() % 60),
                "BREAK - step away".to_string(),
                WARN,
            ),
        };
        let mut start = false;
        let mut stop = false;
        Self::card(ui, |ui| {
            ui.horizontal(|ui| {
                Self::ring(ui, frac, big, small, color);
                ui.vertical(|ui| {
                    ui.add_enabled_ui(self.phase == Phase::Idle, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Work minutes");
                            ui.add(egui::DragValue::new(&mut self.work_min).range(1..=240));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Break minutes");
                            ui.add(egui::DragValue::new(&mut self.brk_min).range(1..=60));
                        });
                    });
                    ui.checkbox(&mut self.focus_on_break, "Bring this window to the front when a break starts");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if self.phase == Phase::Idle {
                            if primary_btn(ui, "Start", 140.0).clicked() { start = true; }
                        } else if ghost_btn(ui, "Stop", 140.0).clicked() { stop = true; }
                    });
                    ui.label(RichText::new(format!("Breaks taken this session: {}", self.cycles)).color(MUTED));
                });
            });
        });
        if start {
            self.start_phase(Phase::Work, self.work_min);
            self.log(format!("timer started {}/{} min", self.work_min, self.brk_min));
        }
        if stop {
            self.phase = Phase::Idle;
            self.log("timer stopped");
        }
        if let Some((ok, msg)) = &self.verdict {
            ui.add_space(8.0);
            Self::card(ui, |ui| {
                ui.label(RichText::new(if *ok { "Last break check" } else { "Last break check: attention" }).strong().color(if *ok { ACCENT } else { BAD }));
                ui.label(msg.as_str());
            });
        }
        ui.add_space(8.0);
        Self::card(ui, |ui| {
            let was = self.keep_awake;
            ui.checkbox(&mut self.keep_awake, "Keep Windows awake while this app is open (optional, off by default)");
            callout(ui, "Prevents idle sleep only. Released when you untick it or close the app. Does not change your power plan.", MUTED);
            if was != self.keep_awake {
                if self.keep_awake {
                    self.keep_awake_flag.store(true, Ordering::SeqCst);
                    sys::hold_awake(self.keep_awake_flag.clone(), false);
                    self.log("keep-awake ON (user choice)");
                } else {
                    self.keep_awake_flag.store(false, Ordering::SeqCst);
                    self.log("keep-awake OFF");
                }
            }
        });
    }

    fn ui_power(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Sleep and Night mode", "Real sleep pauses everything. Night mode keeps the PC awake with the screens off.");
        // ---- SLEEP
        let mut do_sleep = false;
        Self::card(ui, |ui| {
            card_title(ui, "Sleep now, wake automatically", Some(("REAL SLEEP", WARN)));
            ui.label("Real Windows sleep (S3). Everything is frozen while asleep: downloads, installs, AI jobs and agents do nothing until the PC wakes, then they continue from memory. Network connections and remote sessions may drop and reconnect.");
            ui.horizontal(|ui| {
                ui.label("Wake after");
                ui.add(egui::DragValue::new(&mut self.sleep_min).range(1..=720).suffix(" min"));
            });
            match self.wake_timers_ok {
                Some(true) => { callout(ui, "Wake timers are enabled in the active power plan (this does not prove the PC will wake on time).", ACCENT); }
                Some(false) => { callout(ui, "Wake timers are DISABLED in your power plan, so an automatic wake is not possible. Button disabled. This app does not change your power plan.", BAD); }
                None => { callout(ui, "Could not read the wake-timer setting. Button disabled.", BAD); }
            }
            callout(ui, "The app sets a wake timer before sleeping, but waking on time depends on firmware, drivers and Windows and is not guaranteed or tested here. If the PC stays asleep, press a key or the power button. Remote access (DWService) is offline while it sleeps.", WARN);
            let busy = self.sleep_running.load(Ordering::SeqCst);
            let can = self.wake_timers_ok == Some(true) && !busy;
            ui.horizontal(|ui| {
                if let Some(t) = self.sleep_confirm {
                    let left = 8i64 - t.elapsed().as_secs() as i64;
                    if left <= 0 { do_sleep = true; self.sleep_confirm = None; }
                    else {
                        ui.label(RichText::new(format!("Sleeping in {}s...", left)).color(WARN).strong());
                        if ghost_btn(ui, "Cancel", 110.0).clicked() { self.sleep_confirm = None; self.log("sleep cancelled"); }
                    }
                } else if ui.add_enabled(can, egui::Button::new(RichText::new("Sleep, then wake me").size(16.0).strong().color(Color32::from_rgb(40, 22, 0))).fill(WARN).corner_radius(egui::CornerRadius::same(12)).min_size(Vec2::new(220.0, 42.0))).clicked() {
                    self.sleep_confirm = Some(Instant::now());
                }
            });
        });
        if do_sleep {
            let mins = self.sleep_min;
            let log = self.log.clone();
            let running = self.sleep_running.clone();
            let post = self.post_resume.clone();
            let w = self.watch_list();
            running.store(true, Ordering::SeqCst);
            let before = snapshot(&w);
            log_line(&log, format!("going to sleep for {} min. running: {}", mins, fmt_snap(&before)));
            std::thread::spawn(move || {
                let outcome = sys::sleep_and_resume(mins);
                // After a timer wake Windows may treat the wake as unattended: keep the PC and screens up for 2 minutes.
                let keep = Arc::new(AtomicBool::new(true));
                sys::hold_awake(keep.clone(), true);
                sys::screens_on();
                let after = snapshot(&w);
                let (ok, cmp) = compare(&before, &after);
                let msg = match outcome {
                    sys::SleepOutcome::Returned { secs, timer_fired } => format!(
                        "Back from sleep after about {} s ({}). {} {}",
                        secs,
                        if timer_fired { "the wake timer fired" } else { "woken by something else or timer not due yet" },
                        if ok { "OK." } else { "ATTENTION." },
                        cmp
                    ),
                    sys::SleepOutcome::Failed(e) => format!("Sleep did not happen: {}", e),
                };
                log_line(&log, msg.clone());
                if let Ok(mut p) = post.lock() { *p = Some(msg); }
                std::thread::sleep(Duration::from_secs(120));
                keep.store(false, Ordering::SeqCst);
                running.store(false, Ordering::SeqCst);
            });
        }
        if let Ok(p) = self.post_resume.lock() {
            if let Some(m) = p.as_ref() {
                ui.add_space(6.0);
                let m = m.clone();
                Self::card(ui, |ui| { card_title(ui, "Last sleep result", None); ui.label(m); });
            }
        }
        ui.add_space(8.0);
        // ---- NIGHT
        let on = self.night_on.load(Ordering::SeqCst);
        let mut toggle = false;
        Self::card(ui, |ui| {
            card_title(ui, "Night mode: stay awake, screens off", Some(("NOT HIBERNATION", ACCENT)));
            ui.label("The PC stays fully awake so everything in the background keeps running (installs, AI jobs, agents, downloads). The screens go dark to save power and heat. This is not hibernation and does not save the same power as sleep. Nothing is throttled or closed.");
            ui.add_enabled_ui(!on, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Run for");
                    ui.add(egui::DragValue::new(&mut self.night_hours).range(0.0..=24.0).speed(0.1).suffix(" h"));
                    ui.label(RichText::new("(0 = until you stop it)").color(MUTED));
                });
                ui.checkbox(&mut self.night_screens_off, "Turn the screens off while night mode runs");
            });
            callout(ui, "Only blocks idle sleep and screen timeout while on. Restored when you stop, when time runs out, or when you close the app. Does not edit the power plan.", MUTED);
            if on {
                let started = self.night_started.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                ui.label(RichText::new(format!("ON for {:02}:{:02}:{:02}", started / 3600, (started / 60) % 60, started % 60)).color(WARN).strong());
                if ghost_btn(ui, "Stop night mode", 210.0).clicked() { toggle = true; }
            } else if primary_btn(ui, "Start night mode", 210.0).clicked() { toggle = true; }
        });
        if toggle {
            if on {
                self.stop_night();
            } else {
                self.night_on.store(true, Ordering::SeqCst);
                sys::hold_awake(self.night_on.clone(), false);
                self.night_started = Some(Instant::now());
                self.night_end = if self.night_hours > 0.0 { Some(Instant::now() + Duration::from_secs_f32(self.night_hours * 3600.0)) } else { None };
                self.log(format!("night mode ON, {} h, screens off: {}", self.night_hours, self.night_screens_off));
                if self.night_screens_off {
                    sys::screens_off();
                }
            }
        }
    }

    fn stop_night(&mut self) {
        self.night_on.store(false, Ordering::SeqCst);
        self.night_end = None;
        sys::screens_on();
        self.log("night mode OFF, turn-on requested");
    }

    fn ui_screens(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Screens", "Turn screens off for a while, or one at a time.");
        let mut all_off = false;
        let mut all_on = false;
        Self::card(ui, |ui| {
            card_title(ui, "All screens off for a set time", Some(("UNTESTED", WARN)));
            ui.label("Asks Windows to switch every screen off, then sends a turn-on request when the time is up. Whether they actually come back on time depends on your monitors and drivers, which has not been tested. Moving the mouse or pressing a key turns them on early. Programs keep running.");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.screens_min).range(1..=720).suffix(" min"));
                if self.screens_off_until.is_none() {
                    if primary_btn(ui, "Screens off", 160.0).clicked() { all_off = true; }
                } else if ghost_btn(ui, "Turn on now", 160.0).clicked() { all_on = true; }
            });
            if let Some(t) = self.screens_off_until {
                let l = t.saturating_duration_since(Instant::now()).as_secs();
                ui.label(RichText::new(format!("Turn-on request in {:02}:{:02}", l / 60, l % 60)).color(WARN));
            }
        });
        if all_off {
            sys::screens_off();
            self.screens_off_until = Some(Instant::now() + Duration::from_secs(self.screens_min as u64 * 60));
            self.log(format!("all screens off for {} min", self.screens_min));
        }
        if all_on {
            sys::screens_on();
            self.screens_off_until = None;
            self.log("screens on (manual)");
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Individual screens").size(20.0).strong().color(TEXT));
            if ghost_btn(ui, "Rescan", 100.0).clicked() { self.monitors = sys::monitors(); }
        });
        let mons = self.monitors.clone();
        let mut new_blackout: Option<(usize, u32)> = None;
        let mut ddc_action: Option<(String, u32)> = None;
        for (i, m) in mons.iter().enumerate() {
            Self::card(ui, |ui| {
                card_title(ui, &format!("Screen {}  {}x{}", i + 1, m.w, m.h), if m.primary { Some(("MAIN", ACCENT)) } else { None });
                ui.label(RichText::new(format!("{}  at ({}, {})", m.name, m.x, m.y)).color(MUTED).size(13.0));
                let active = self.blackouts.iter().find(|b| b.monitor == i).map(|b| b.until);
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.screens_min).range(1..=720).suffix(" min"));
                    if let Some(u) = active {
                        let l = u.saturating_duration_since(Instant::now()).as_secs();
                        ui.label(RichText::new(format!("Blackout ends in {:02}:{:02}", l / 60, l % 60)).color(WARN));
                    } else if ghost_btn(ui, "Black out this screen", 210.0).clicked() {
                        new_blackout = Some((i, self.screens_min));
                    }
                });
                callout(ui, "Blackout puts a black window over this screen. It does NOT power the screen off or save power. It closes when the timer ends; click it or press Esc to close it sooner. Untested on this PC.", MUTED);
                match m.ddc_power {
                    Some(v) => {
                        ui.label(RichText::new(format!("Monitor power register (DDC/CI) was readable, current value {}", v)).color(ACCENT).size(12.0));
                        ui.checkbox(&mut self.ddc_ack, "I understand: this is untested, readable does not mean the monitor will wake again, and I may need its own power button");
                        ui.add_enabled_ui(self.ddc_ack, |ui| {
                            ui.horizontal(|ui| {
                                if ghost_btn(ui, "Power off this monitor (hardware)", 280.0).clicked() { ddc_action = Some((m.name.clone(), 4)); }
                                if ghost_btn(ui, "Power on", 120.0).clicked() { ddc_action = Some((m.name.clone(), 1)); }
                            });
                        });
                    }
                    None => {
                        callout(ui, "Hardware power control (DDC/CI) was not readable for this screen, so only Blackout is offered.", MUTED);
                    }
                }
            });
        }
        if let Some((i, mins)) = new_blackout {
            self.blackouts.push(Blackout { monitor: i, until: Instant::now() + Duration::from_secs(mins as u64 * 60) });
            self.log(format!("blackout screen {} for {} min", i + 1, mins));
        }
        if let Some((name, val)) = ddc_action {
            let ok = sys::ddc_set_power(&name, val);
            self.log(format!("DDC power {} on {}: {}", val, name, if ok { "sent" } else { "failed" }));
        }
    }

    fn ui_watch(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Watched programs", "Names only. It does not prove a program is working.");
        Self::card(ui, |ui| {
            ui.label("Process names to compare before and after each break or sleep (comma separated).");
            ui.add(egui::TextEdit::multiline(&mut self.watch_text).desired_width(f32::INFINITY).desired_rows(3));
            if ghost_btn(ui, "Reset to default", 170.0).clicked() { self.watch_text = DEFAULT_WATCH.into(); }
        });
        ui.add_space(8.0);
        Self::card(ui, |ui| {
            card_title(ui, "Running now", Some(("LIVE", ACCENT)));
            ui.label(fmt_snap(&self.live));
            callout(ui, "This lists processes by name. It does not prove a program is working or that a session is healthy.", MUTED);
        });
    }

    fn ui_log(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Activity log", "What BreakGuard did this session.");
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            if let Ok(v) = self.log.lock() {
                for l in v.iter() { ui.label(RichText::new(l.as_str()).monospace().size(13.0)); }
                if v.is_empty() { ui.label(RichText::new("Nothing yet.").color(MUTED)); }
            }
        });
    }

    fn overlays(&mut self, ctx: &egui::Context) {
        let mons = self.monitors.clone();
        let ppp = ctx.pixels_per_point();
        let mut close: Vec<usize> = vec![];
        for (k, b) in self.blackouts.iter().enumerate() {
            if Instant::now() >= b.until { close.push(k); continue; }
            let Some(m) = mons.get(b.monitor) else { close.push(k); continue; };
            let id = egui::ViewportId::from_hash_of(("blackout", b.monitor));
            let vb = egui::ViewportBuilder::default()
                .with_title("breakguard blackout")
                .with_decorations(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_position(egui::pos2(m.x as f32 / ppp, m.y as f32 / ppp))
                .with_inner_size(egui::vec2(m.w as f32 / ppp, m.h as f32 / ppp));
            let mut shut = false;
            ctx.show_viewport_immediate(id, vb, |ctx, _| {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::BLACK)).show(ctx, |ui| {
                    let r = ui.allocate_rect(ui.max_rect(), egui::Sense::click());
                    if r.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { shut = true; }
                });
                if ctx.input(|i| i.viewport().close_requested()) { shut = true; }
            });
            if shut { close.push(k); }
        }
        close.sort();
        close.dedup();
        for k in close.into_iter().rev() {
            self.blackouts.remove(k);
            self.log("blackout ended");
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(i) = diag::per_frame(ctx) { self.tab = [Tab::Home, Tab::Breaks, Tab::Power, Tab::Screens, Tab::Watch, Tab::Log][i]; }
        ctx.request_repaint_after(Duration::from_millis(500));
        self.poll_watch();
        self.tick_phase(ctx);
        if let Some(t) = self.screens_off_until {
            if Instant::now() >= t { sys::screens_on(); self.screens_off_until = None; self.log("screens on (timer)"); }
        }
        if self.night_on.load(Ordering::SeqCst) {
            if let Some(e) = self.night_end { if Instant::now() >= e { self.stop_night(); } }
        }
        egui::SidePanel::left("nav").exact_width(220.0).frame(egui::Frame::new().fill(SIDE).inner_margin(egui::Margin::symmetric(16, 20))).show(ctx, |ui| {
            ui.label(RichText::new("BreakGuard").size(26.0).strong().color(ACCENT));
            ui.label(RichText::new("rest. sleep. screens.").color(MUTED).size(13.0));
            ui.add_space(22.0);
            for (t, name) in [(Tab::Home, "Overview"), (Tab::Power, "Sleep and Night"), (Tab::Screens, "Screens off"), (Tab::Breaks, "Break timer"), (Tab::Watch, "Watch list"), (Tab::Log, "Log")] {
                let sel = self.tab == t;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), egui::Sense::click());
                let hov = resp.hovered();
                let bg = if sel { ACCENT_DIM } else if hov { Color32::from_rgb(20, 26, 36) } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, egui::CornerRadius::same(12), bg);
                if sel {
                    let bar = egui::Rect::from_min_size(rect.left_center() - Vec2::new(0.0, 12.0), Vec2::new(4.0, 24.0));
                    ui.painter().rect_filled(bar, egui::CornerRadius::same(2), ACCENT);
                }
                ui.painter().text(rect.left_center() + Vec2::new(18.0, 0.0), egui::Align2::LEFT_CENTER, name,
                    egui::FontId::proportional(17.0), if sel { Color32::WHITE } else { TEXT });
                if resp.clicked() { self.tab = t; }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(RichText::new("Never closes or pauses\nyour programs.").color(MUTED).size(13.0));
                if self.night_on.load(Ordering::SeqCst) { ui.label(RichText::new("NIGHT MODE ON").color(WARN).strong()); }
                if self.keep_awake { ui.label(RichText::new("keep-awake ON").color(WARN)); }
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(28))).show(ctx, |ui| {
            diag::apply_scroll(egui::ScrollArea::vertical()).show(ui, |ui| match self.tab {
                Tab::Home => { self.ui_power(ui); ui.add_space(14.0); self.ui_screens(ui); }
                Tab::Breaks => self.ui_breaks(ui),
                Tab::Power => self.ui_power(ui),
                Tab::Screens => self.ui_screens(ui),
                Tab::Watch => self.ui_watch(ui),
                Tab::Log => self.ui_log(ui),
            });
        });
        self.overlays(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.keep_awake_flag.store(false, Ordering::SeqCst);
        self.night_on.store(false, Ordering::SeqCst);
        if self.night_started.is_some() || self.screens_off_until.is_some() {
            sys::screens_on();
        }
    }
}

fn main() -> eframe::Result {
    diag::install_panic_hook();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BreakGuard")
            .with_inner_size([1120.0, 780.0])
            .with_min_inner_size([820.0, 600.0]),
        ..Default::default()
    };
    let r = eframe::run_native("BreakGuard", opts, Box::new(|cc| { diag::log_gl(cc); Ok(Box::new(App::new(cc))) }));
    if let Err(e) = &r { diag::log(&format!("run_native error: {e}")); }
    r
}
