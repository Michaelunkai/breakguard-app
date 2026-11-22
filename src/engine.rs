//! engine: pure logic, no Win32, no egui. OWNER: Engine Engineer.
//! Settings persistence, injectable clock, break-timer state machine,
//! schedules, night window, timed sleep request, activity log.
#![allow(dead_code)]

use crate::stats::Stats;
use std::io::Write;
use std::path::PathBuf;

pub const MAX_SCHEDS: usize = 16;
const RESUME_GAP_SECS: i64 = 10;
const SCHED_LATE_SECS: i64 = 60;
const LOG_MAX_BYTES: u64 = 256 * 1024;

// ---------------------------------------------------------------- paths / log

pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("BREAKGUARD_DATA_DIR") {
        return PathBuf::from(d);
    }
    match std::env::var_os("APPDATA") {
        Some(a) => PathBuf::from(a).join("BreakGuard"),
        None => PathBuf::from("."),
    }
}

fn log_path() -> PathBuf {
    if std::env::var_os("BREAKGUARD_DATA_DIR").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("breakguard.log");
                let ok = std::fs::OpenOptions::new().create(true).append(true).open(&p).is_ok();
                if ok {
                    return p;
                }
            }
        }
    }
    data_dir().join("breakguard.log")
}

/// Append a timestamped line to breakguard.log; rotates at 256 KB (keeps .1).
pub fn log_line(s: &str) {
    let t = crate::sys::local_now();
    let line = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}  {}\r\n",
        t.year, t.month, t.day, t.hour, t.minute, t.second, s
    );
    log_to(&log_path(), &line, LOG_MAX_BYTES);
}

fn log_to(path: &std::path::Path, line: &str, max: u64) {
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Ok(md) = std::fs::metadata(path) {
        if md.len() >= max {
            let old = path.with_extension("log.1");
            let _ = std::fs::remove_file(&old);
            let _ = std::fs::rename(path, &old);
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

// ------------------------------------------------------------------- settings

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffMode {
    Hardware,
    Blackout,
}

/// What a break looks like on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakStyle {
    /// Selected screens go fully black with a tip and countdown.
    Blackout,
    /// Selected screens dim, clicks pass through.
    Dim,
    /// Only a toast and a sound; nothing on screen changes.
    Notify,
}

impl BreakStyle {
    pub fn key(self) -> &'static str {
        match self {
            BreakStyle::Blackout => "blackout",
            BreakStyle::Dim => "dim",
            BreakStyle::Notify => "notify",
        }
    }
    pub fn from_key(k: &str) -> Option<BreakStyle> {
        match k {
            "blackout" => Some(BreakStyle::Blackout),
            "dim" => Some(BreakStyle::Dim),
            "notify" => Some(BreakStyle::Notify),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sched {
    pub id: u32,
    pub h: u8,
    pub m: u8,
    pub mins: u32,
    pub targets: Vec<String>,
    pub daily: bool,
}

/// Which monitors a break preset turns off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakTarget {
    Mon1,
    Mon2,
    Both,
}

impl BreakTarget {
    pub fn key(self) -> &'static str {
        match self {
            BreakTarget::Mon1 => "1",
            BreakTarget::Mon2 => "2",
            BreakTarget::Both => "both",
        }
    }
    pub fn from_key(k: &str) -> Option<BreakTarget> {
        match k {
            "1" => Some(BreakTarget::Mon1),
            "2" => Some(BreakTarget::Mon2),
            "both" => Some(BreakTarget::Both),
            _ => None,
        }
    }
}

pub const MAX_PRESETS: usize = 24;

/// A named, user-editable break: how long, which monitors, whether running agents pause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakPreset {
    pub id: u32,
    pub name: String,
    pub mins: u32,
    pub target: BreakTarget,
    pub pause_agents: bool,
}

/// Built-in presets used until the user edits the list.
pub fn default_presets() -> Vec<BreakPreset> {
    let p = |id, name: &str, mins, target, pause_agents| BreakPreset { id, name: name.into(), mins, target, pause_agents };
    vec![
        p(1, "Eyes 20-20-20", 1, BreakTarget::Both, false),
        p(2, "Quick", 5, BreakTarget::Both, true),
        p(3, "Walk", 15, BreakTarget::Both, true),
        p(4, "Meal", 30, BreakTarget::Both, true),
    ]
}

/// Append a new preset with a fresh unique id; returns its index, or None at the cap.
pub fn preset_add(list: &mut Vec<BreakPreset>) -> Option<usize> {
    if list.len() >= MAX_PRESETS {
        return None;
    }
    let id = list.iter().map(|p| p.id).max().unwrap_or(0) + 1;
    list.push(BreakPreset { id, name: format!("Break {id}"), mins: 10, target: BreakTarget::Both, pause_agents: false });
    Some(list.len() - 1)
}

/// Delete the preset with this id; true when something was removed.
pub fn preset_delete(list: &mut Vec<BreakPreset>, id: u32) -> bool {
    let n = list.len();
    list.retain(|p| p.id != id);
    list.len() != n
}

/// Longest manual screen-off: 24 hours (the hardware failsafe helper accepts up to 1440 min).
pub const MAX_OFF_SECS: u32 = 24 * 3600;

/// Longest work stretch before a break: 24 hours, any whole minute from 1 up.
pub const MAX_WORK_MIN: u32 = 24 * 60;

/// Parse a screen-off length typed by the user. Accepts "90" (minutes), "45s", "20m", "1h", "1h30m",
/// "1h 30m 15s", "2.5h", "1:30" (h:mm) and "1:30:15" (h:mm:ss). Returns seconds in 1..=MAX_OFF_SECS.
pub fn parse_duration(text: &str) -> Option<u32> {
    let t = text.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let secs: f64 = if t.contains(':') {
        let parts: Vec<&str> = t.split(':').map(str::trim).collect();
        if parts.len() > 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
            return None;
        }
        let n: Vec<f64> = parts.iter().map(|p| p.parse().unwrap_or(0.0)).collect();
        match n.as_slice() {
            [h, m] if *m < 60.0 => h * 3600.0 + m * 60.0,
            [h, m, s] if *m < 60.0 && *s < 60.0 => h * 3600.0 + m * 60.0 + s,
            _ => return None,
        }
    } else if let Ok(m) = t.parse::<f64>() {
        m * 60.0
    } else {
        let (mut total, mut num, mut any) = (0.0f64, String::new(), false);
        let mut chars = t.chars().peekable();
        while let Some(c) = chars.next() {
            if c.is_ascii_digit() || c == '.' {
                num.push(c);
                continue;
            }
            if c == ' ' || c == ',' {
                continue;
            }
            let mut unit = c.to_string();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphabetic() { unit.push(n); chars.next(); } else { break; }
            }
            let mult = match unit.as_str() {
                "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
                "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
                "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
                _ => return None,
            };
            let v: f64 = num.parse().ok()?;
            total += v * mult;
            num.clear();
            any = true;
        }
        if !num.is_empty() || !any {
            return None;
        }
        total
    };
    let secs = secs.round();
    if !secs.is_finite() || secs < 1.0 || secs > MAX_OFF_SECS as f64 {
        return None;
    }
    Some(secs as u32)
}

/// Human text for a screen-off length: "45 s", "15 min", "1 h 30 min", "1 h 0 min 5 s".
pub fn fmt_duration(secs: u32) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    let mut out = Vec::new();
    if h > 0 { out.push(format!("{h} h")); }
    if m > 0 || (h > 0 && s > 0) { out.push(format!("{m} min")); }
    if s > 0 || out.is_empty() { out.push(format!("{s} s")); }
    out.join(" ")
}

/// Longest short break the settings accept (4 h): any minute count up to this is valid.
pub const BREAK_MAX_SECS: u32 = 4 * 3600;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Named break types (quick start list).
    pub presets: Vec<BreakPreset>,
    pub work_min: u32,
    pub break_secs: u32,
    pub warn_secs: u32,
    pub break_enabled: bool,
    pub off_mode: OffMode,
    pub night_enabled: bool,
    /// Manual night override that outlives the schedule: 0 follow schedule, 1 forced on, 2 forced off.
    pub night_force: u8,
    pub night_start: (u8, u8),
    pub night_end: (u8, u8),
    pub night_level: u8,
    pub selected: Vec<String>,
    /// Suspend Claude Code agents while a break runs.
    pub pause_agents: bool,
    /// Session ids chosen for pausing; empty = every running agent.
    pub pause_sel: Vec<String>,
    pub sleep_min: u32,
    /// Length of a manual screen-off in seconds (1 s .. MAX_OFF_SECS); remembered between runs.
    pub off_secs: u32,
    pub scheds: Vec<Sched>,
    pub break_style: BreakStyle,
    /// Every Nth break is a long one; 0 = never.
    pub long_break_every: u32,
    pub long_break_secs: u32,
    /// false = strict: a running break cannot be skipped.
    pub allow_skip: bool,
    /// true = work timer starts by itself after a break; false = waits paused until you resume.
    pub auto_resume: bool,
    pub snooze_mins: u32,
    /// Being away from keyboard for a whole break counts as a break.
    pub idle_credit: bool,
    /// Freeze the work timer after this many idle seconds; 0 = off.
    pub idle_pause_secs: u32,
    pub hours_enabled: bool,
    pub hours_start: (u8, u8),
    pub hours_end: (u8, u8),
    /// Hold breaks while a fullscreen app / presentation is active.
    pub dnd_fullscreen: bool,
    pub sound: bool,
    pub tips: bool,
    pub custom_tips: Vec<String>,
    pub autostart: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub hotkeys: bool,
    pub hk_break: char,
    pub hk_pause: char,
    pub hk_snooze: char,
    pub daily_goal: u32,
    pub onboarded: bool,
    /// Accent swatch index (0..ACCENTS).
    pub accent: u8,
    /// UI zoom in percent.
    pub ui_scale: u32,
    /// Micro reminders in minutes; 0 = off.
    pub micro_water: u32,
    pub micro_stand: u32,
    pub micro_posture: u32,
    /// Break-start actions chosen in the dashboard picker (`break_action::actions_to_text`); empty = none set.
    pub break_actions: String,
    /// Monitor names the picker's monitor actions apply to, comma separated; empty = by action kind.
    pub break_monitors: String,
    /// Light palette instead of the dark one.
    pub light_theme: bool,
}

pub const ACCENTS: u8 = 6;
pub const MAX_TIPS: usize = 20;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            presets: default_presets(),
            break_style: BreakStyle::Blackout,
            long_break_every: 4,
            long_break_secs: 300,
            allow_skip: true,
            auto_resume: true,
            snooze_mins: 5,
            idle_credit: true,
            idle_pause_secs: 120,
            hours_enabled: false,
            hours_start: (9, 0),
            hours_end: (18, 0),
            dnd_fullscreen: true,
            sound: true,
            tips: true,
            custom_tips: Vec::new(),
            autostart: false,
            start_minimized: false,
            close_to_tray: false,
            hotkeys: true,
            hk_break: 'B',
            hk_pause: 'P',
            hk_snooze: 'S',
            daily_goal: 8,
            onboarded: false,
            accent: 0,
            ui_scale: 100,
            micro_water: 0,
            micro_stand: 0,
            micro_posture: 0,
            break_actions: String::new(),
            break_monitors: String::new(),
            light_theme: false,
            work_min: 20,
            break_secs: 20,
            warn_secs: 10,
            break_enabled: false,
            off_mode: OffMode::Hardware,
            night_enabled: false,
            night_force: 0,
            night_start: (22, 0),
            night_end: (6, 0),
            night_level: 40,
            selected: Vec::new(),
            pause_agents: false,
            pause_sel: Vec::new(),
            sleep_min: 30,
            off_secs: 15 * 60,
            scheds: Vec::new(),
        }
    }
}

fn clamp_hm(hm: (u8, u8)) -> (u8, u8) {
    (hm.0.min(23), hm.1.min(59))
}

/// Tip text: single line, no control chars, at most 120 chars.
pub fn clean_tip(t: &str) -> String {
    t.chars().filter(|c| !c.is_control() && *c != '|').take(120).collect::<String>().trim().to_string()
}

/// Hotkey letter: A-Z only, otherwise the default.
fn clean_key(c: char, default: char) -> char {
    let u = c.to_ascii_uppercase();
    if u.is_ascii_uppercase() {
        u
    } else {
        default
    }
}

fn clamp_micro(m: u32) -> u32 {
    if m == 0 {
        0
    } else {
        m.clamp(5, 480)
    }
}

impl Settings {
    pub fn clamp(&mut self) {
        self.work_min = self.work_min.clamp(1, MAX_WORK_MIN);
        self.break_secs = self.break_secs.clamp(5, BREAK_MAX_SECS);
        self.warn_secs = self.warn_secs.min(120);
        self.long_break_every = self.long_break_every.min(20);
        self.long_break_secs = self.long_break_secs.clamp(5, BREAK_MAX_SECS);
        self.snooze_mins = self.snooze_mins.clamp(1, 120);
        self.idle_pause_secs = if self.idle_pause_secs == 0 { 0 } else { self.idle_pause_secs.clamp(15, 3600) };
        self.hours_start = clamp_hm(self.hours_start);
        self.hours_end = clamp_hm(self.hours_end);
        self.custom_tips = self
            .custom_tips
            .iter()
            .map(|t| clean_tip(t))
            .filter(|t| !t.is_empty())
            .take(MAX_TIPS)
            .collect();
        self.hk_break = clean_key(self.hk_break, 'B');
        self.hk_pause = clean_key(self.hk_pause, 'P');
        self.hk_snooze = clean_key(self.hk_snooze, 'S');
        self.daily_goal = self.daily_goal.clamp(1, 50);
        self.accent = self.accent.min(ACCENTS - 1);
        self.ui_scale = self.ui_scale.clamp(90, 150);
        self.micro_water = clamp_micro(self.micro_water);
        self.micro_stand = clamp_micro(self.micro_stand);
        self.micro_posture = clamp_micro(self.micro_posture);
        self.night_start = clamp_hm(self.night_start);
        self.night_end = clamp_hm(self.night_end);
        self.night_level = self.night_level.min(100);
        self.night_force = self.night_force.min(2);
        self.sleep_min = self.sleep_min.clamp(1, 720);
        self.off_secs = self.off_secs.clamp(1, MAX_OFF_SECS);
        let mut seen = Vec::new();
        self.selected.retain(|x| if seen.contains(x) { false } else { seen.push(x.clone()); true });
        self.selected.truncate(32);
        self.pause_sel.truncate(64);
        self.presets.truncate(MAX_PRESETS);
        for p in &mut self.presets {
            p.mins = p.mins.clamp(1, 720);
            p.name = clean_tip(&p.name).chars().take(24).collect();
            if p.name.is_empty() {
                p.name = format!("Break {}", p.id);
            }
        }
        let mut pseen = Vec::new();
        self.presets.retain(|p| if pseen.contains(&p.id) { false } else { pseen.push(p.id); true });
        self.scheds.truncate(MAX_SCHEDS);
        for s in &mut self.scheds {
            s.h = s.h.min(23);
            s.m = s.m.min(59);
            s.mins = s.mins.clamp(1, 720);
            s.targets.truncate(32);
        }
    }

    pub fn path() -> PathBuf {
        data_dir().join("settings.ini")
    }

    pub fn load() -> Settings {
        use std::io::Read;
        let Ok(f) = std::fs::File::open(Self::path()) else { return Settings::default() };
        let mut b = Vec::new();
        if f.take(64 * 1024).read_to_end(&mut b).is_err() {
            return Settings::default();
        }
        if looks_corrupt(&b) {
            // Keep the unreadable file for the user, then fall back to defaults.
            let _ = std::fs::copy(Self::path(), Self::path().with_extension("ini.bad"));
            return Settings::default();
        }
        Self::parse(&String::from_utf8_lossy(&b))
    }

    /// Atomic write (temp file + rename). Returns false on failure.
    pub fn save(&self) -> bool {
        let path = Self::path();
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let tmp = path.with_extension("ini.tmp");
        if std::fs::write(&tmp, self.serialize()).is_err() {
            return false;
        }
        if std::fs::rename(&tmp, &path).is_ok() {
            return true;
        }
        let _ = std::fs::remove_file(&tmp);
        false
    }

    pub fn serialize(&self) -> String {
        let mut o = String::new();
        let mut kv = |k: &str, v: String| {
            o.push_str(k);
            o.push('=');
            o.push_str(&v);
            o.push('\n');
        };
        kv("work_min", self.work_min.to_string());
        kv("break_secs", self.break_secs.to_string());
        kv("warn_secs", self.warn_secs.to_string());
        kv("break_enabled", (self.break_enabled as u8).to_string());
        kv("off_mode", if self.off_mode == OffMode::Hardware { "hardware" } else { "blackout" }.into());
        kv("night_enabled", (self.night_enabled as u8).to_string());
        kv("night_force", self.night_force.to_string());
        kv("night_start", format!("{:02}:{:02}", self.night_start.0, self.night_start.1));
        kv("night_end", format!("{:02}:{:02}", self.night_end.0, self.night_end.1));
        kv("night_level", self.night_level.to_string());
        kv("selected", self.selected.join("|"));
        kv("pause_agents", (self.pause_agents as u8).to_string());
        kv("pause_sel", self.pause_sel.join("|"));
        kv("sleep_min", self.sleep_min.to_string());
        kv("off_secs", self.off_secs.to_string());
        kv("break_style", self.break_style.key().into());
        kv("long_break_every", self.long_break_every.to_string());
        kv("long_break_secs", self.long_break_secs.to_string());
        kv("allow_skip", (self.allow_skip as u8).to_string());
        kv("auto_resume", (self.auto_resume as u8).to_string());
        kv("snooze_mins", self.snooze_mins.to_string());
        kv("idle_credit", (self.idle_credit as u8).to_string());
        kv("idle_pause_secs", self.idle_pause_secs.to_string());
        kv("hours_enabled", (self.hours_enabled as u8).to_string());
        kv("hours_start", format!("{:02}:{:02}", self.hours_start.0, self.hours_start.1));
        kv("hours_end", format!("{:02}:{:02}", self.hours_end.0, self.hours_end.1));
        kv("dnd_fullscreen", (self.dnd_fullscreen as u8).to_string());
        kv("sound", (self.sound as u8).to_string());
        kv("tips", (self.tips as u8).to_string());
        for t in &self.custom_tips {
            kv("tip", t.clone());
        }
        kv("autostart", (self.autostart as u8).to_string());
        kv("start_minimized", (self.start_minimized as u8).to_string());
        kv("close_to_tray", (self.close_to_tray as u8).to_string());
        kv("hotkeys", (self.hotkeys as u8).to_string());
        kv("hk_break", self.hk_break.to_string());
        kv("hk_pause", self.hk_pause.to_string());
        kv("hk_snooze", self.hk_snooze.to_string());
        kv("daily_goal", self.daily_goal.to_string());
        kv("onboarded", (self.onboarded as u8).to_string());
        kv("accent", self.accent.to_string());
        kv("ui_scale", self.ui_scale.to_string());
        kv("micro_water", self.micro_water.to_string());
        kv("micro_stand", self.micro_stand.to_string());
        kv("micro_posture", self.micro_posture.to_string());
        kv("break_actions", self.break_actions.clone());
        kv("break_monitors", self.break_monitors.clone());
        kv("light_theme", (self.light_theme as u8).to_string());
        kv("presets_set", "1".into());
        for p in &self.presets {
            kv("preset", format!("{},{},{},{},{}", p.id, p.mins, p.target.key(), p.pause_agents as u8, p.name));
        }
        for s in &self.scheds {
            kv(
                "sched",
                format!("{},{},{},{},{},{}", s.id, s.h, s.m, s.mins, s.daily as u8, s.targets.join("|")),
            );
        }
        o
    }

    /// Tolerant parser: unknown/garbled lines ignored, result always clamped.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        let mut saw_onboarded = false;
        let mut saw_known = false;
        let mut presets_seen = false;
        for line in text.lines() {
            let line = line.trim().trim_start_matches('\u{feff}');
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            saw_known |= match k {
                "work_min" | "break_secs" => v.parse::<u32>().is_ok(),
                "break_enabled" | "selected" => true,
                _ => false,
            };
            match k {
                "work_min" => set_num(&mut s.work_min, v),
                "break_secs" => set_num(&mut s.break_secs, v),
                "warn_secs" => set_num(&mut s.warn_secs, v),
                "break_enabled" => set_bool(&mut s.break_enabled, v),
                "off_mode" => match v {
                    "hardware" => s.off_mode = OffMode::Hardware,
                    "blackout" => s.off_mode = OffMode::Blackout,
                    _ => {}
                },
                "night_enabled" => set_bool(&mut s.night_enabled, v),
                "night_force" => set_num(&mut s.night_force, v),
                "night_start" => {
                    if let Some(hm) = parse_hm(v) {
                        s.night_start = hm
                    }
                }
                "night_end" => {
                    if let Some(hm) = parse_hm(v) {
                        s.night_end = hm
                    }
                }
                "night_level" => set_num(&mut s.night_level, v),
                "selected" => s.selected = split_list(v),
                "pause_agents" => set_bool(&mut s.pause_agents, v),
                "pause_sel" => s.pause_sel = split_list(v),
                "sleep_min" => set_num(&mut s.sleep_min, v),
                "off_secs" => set_num(&mut s.off_secs, v),
                "break_style" => {
                    if let Some(b) = BreakStyle::from_key(v) {
                        s.break_style = b
                    }
                }
                "long_break_every" => set_num(&mut s.long_break_every, v),
                "long_break_secs" => set_num(&mut s.long_break_secs, v),
                "allow_skip" => set_bool(&mut s.allow_skip, v),
                "auto_resume" => set_bool(&mut s.auto_resume, v),
                "snooze_mins" => set_num(&mut s.snooze_mins, v),
                "idle_credit" => set_bool(&mut s.idle_credit, v),
                "idle_pause_secs" => set_num(&mut s.idle_pause_secs, v),
                "hours_enabled" => set_bool(&mut s.hours_enabled, v),
                "hours_start" => {
                    if let Some(hm) = parse_hm(v) {
                        s.hours_start = hm
                    }
                }
                "hours_end" => {
                    if let Some(hm) = parse_hm(v) {
                        s.hours_end = hm
                    }
                }
                "dnd_fullscreen" => set_bool(&mut s.dnd_fullscreen, v),
                "sound" => set_bool(&mut s.sound, v),
                "tips" => set_bool(&mut s.tips, v),
                "tip" => s.custom_tips.push(v.to_string()),
                "autostart" => set_bool(&mut s.autostart, v),
                "start_minimized" => set_bool(&mut s.start_minimized, v),
                "close_to_tray" => set_bool(&mut s.close_to_tray, v),
                "hotkeys" => set_bool(&mut s.hotkeys, v),
                "hk_break" => set_key(&mut s.hk_break, v),
                "hk_pause" => set_key(&mut s.hk_pause, v),
                "hk_snooze" => set_key(&mut s.hk_snooze, v),
                "daily_goal" => set_num(&mut s.daily_goal, v),
                "onboarded" => {
                    set_bool(&mut s.onboarded, v);
                    saw_onboarded = true;
                }
                "accent" => set_num(&mut s.accent, v),
                "ui_scale" => set_num(&mut s.ui_scale, v),
                "micro_water" => set_num(&mut s.micro_water, v),
                "micro_stand" => set_num(&mut s.micro_stand, v),
                "micro_posture" => set_num(&mut s.micro_posture, v),
                "break_actions" => s.break_actions = v.trim().to_string(),
                "break_monitors" => s.break_monitors = v.trim().to_string(),
                "light_theme" => set_bool(&mut s.light_theme, v),
                "presets_set" => {
                    if !presets_seen {
                        s.presets.clear();
                        presets_seen = true;
                    }
                }
                "preset" => {
                    if !presets_seen {
                        s.presets.clear();
                        presets_seen = true;
                    }
                    if let Some(p) = parse_preset(v) {
                        s.presets.push(p)
                    }
                }
                "sched" => {
                    if let Some(sc) = parse_sched(v) {
                        s.scheds.push(sc)
                    }
                }
                _ => {}
            }
        }
        // a settings file from before onboarding existed means an existing user: skip the welcome
        if !saw_onboarded && saw_known {
            s.onboarded = true;
        }
        // drop duplicate ids
        let mut seen = Vec::new();
        s.scheds.retain(|c| {
            if seen.contains(&c.id) {
                false
            } else {
                seen.push(c.id);
                true
            }
        });
        s.clamp();
        s
    }
}

fn set_num<T: std::str::FromStr>(dst: &mut T, v: &str) {
    if let Ok(n) = v.parse::<T>() {
        *dst = n;
    }
}
fn set_bool(dst: &mut bool, v: &str) {
    match v {
        "1" | "true" => *dst = true,
        "0" | "false" => *dst = false,
        _ => {}
    }
}
fn set_key(dst: &mut char, v: &str) {
    if let Some(c) = v.chars().next() {
        *dst = c;
    }
}
fn split_list(v: &str) -> Vec<String> {
    v.split('|').filter(|x| !x.is_empty()).map(String::from).collect()
}
fn parse_hm(v: &str) -> Option<(u8, u8)> {
    let (h, m) = v.split_once(':')?;
    let (h, m) = (h.trim().parse::<u8>().ok()?, m.trim().parse::<u8>().ok()?);
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}
fn parse_preset(v: &str) -> Option<BreakPreset> {
    let mut p = v.splitn(5, ',');
    let id = p.next()?.trim().parse().ok()?;
    let mins = p.next()?.trim().parse().ok()?;
    let target = BreakTarget::from_key(p.next()?.trim())?;
    let pause_agents = p.next()?.trim() == "1";
    let name = p.next()?.trim().to_string();
    Some(BreakPreset { id, name, mins, target, pause_agents })
}
fn parse_sched(v: &str) -> Option<Sched> {
    let mut p = v.splitn(6, ',');
    let id = p.next()?.trim().parse().ok()?;
    let h: u8 = p.next()?.trim().parse().ok()?;
    let m: u8 = p.next()?.trim().parse().ok()?;
    let mins = p.next()?.trim().parse().ok()?;
    let daily = p.next()?.trim() == "1";
    let targets = split_list(p.next().unwrap_or(""));
    if h > 23 || m > 59 {
        return None;
    }
    Some(Sched { id, h, m, mins, targets, daily })
}

// ---------------------------------------------------------------------- clock

pub trait Clock {
    fn now_unix(&self) -> i64;
    fn secs_since_midnight(&self) -> u32;
    /// Milliseconds since the epoch; timed screen-offs end on this so a length in seconds is exact.
    fn now_ms(&self) -> i64 {
        self.now_unix() * 1000
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
    fn secs_since_midnight(&self) -> u32 {
        let t = crate::sys::local_now();
        t.hour as u32 * 3600 + t.minute as u32 * 60 + t.second as u32
    }
}

// --------------------------------------------------------------------- engine

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    ScreenOff { targets: Vec<String>, mins: u32 },
    ScreenOn { targets: Vec<String> },
    Warn { secs_left: u32 },
    BreakStart { secs: u32 },
    BreakEnd,
    SleepPc { mins: u32 },
    NightSet { level: u8 },
    Notify(String),
    /// Micro reminder: 0 water, 1 stand up, 2 posture.
    Micro(u8),
}

/// Text of a micro reminder.
pub fn micro_text(kind: u8) -> &'static str {
    match kind {
        0 => "Time for some water",
        1 => "Stand up and move for a minute",
        _ => "Check your posture: shoulders down, back straight",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Off,
    Working { left: u32 },
    Warning { left: u32 },
    Break { left: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineView {
    pub phase: Phase,
    pub next_sched: Option<(u32, i64)>,
    pub night_active: bool,
    /// Timer frozen by the user (or by idle / do-not-disturb); `Phase` keeps the frozen remaining time.
    pub paused: bool,
    /// Self-freeze reason (away, fullscreen app, off hours).
    pub hold: Hold,
    /// The running (or last) break is a long one.
    pub break_is_long: bool,
    /// Length in seconds of the running break (for progress rings).
    pub break_total: u32,
    /// Completed breaks since the timer started (drives long breaks).
    pub cycle: u32,
    /// Currently active screen-offs: (targets, end unix time).
    pub active_off: Vec<(Vec<String>, i64)>,
    /// Screen-offs waiting for their start: (targets, start unix time, length in seconds).
    pub delayed_off: Vec<(Vec<String>, i64, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ph {
    Off,
    Working,
    Break,
}

struct ActiveOff {
    sched_id: u32, // 0 = manual
    targets: Vec<String>,
    end: i64,    // whole seconds (view, caps)
    end_ms: i64, // exact end: tick turns the screens back on at this millisecond
    mins: u32,
}

/// A manual screen-off that starts later ("in 10 min, for 1 h").
struct DelayedOff {
    targets: Vec<String>,
    start_ms: i64,
    secs: u32,
}

pub struct Engine {
    settings: Settings,
    clock: Box<dyn Clock>,
    delayed: Vec<DelayedOff>,
    ph: Ph,
    deadline: i64, // end of work period or of break
    warned: bool,
    last_now: Option<i64>,
    next_fire: Vec<(u32, i64)>,
    active: Vec<ActiveOff>,
    night_level: u8,
    pending: Vec<Action>,
    /// Seconds that were left when the user paused; None = running.
    paused_left: Option<i64>,
    /// work_min the running work period was planned with (live re-plan on edit).
    work_min_applied: u32,
    cycle: u32,
    cur_break: (u32, bool),
    stats: Stats,
    /// Seconds without keyboard/mouse input, fed by the UI each frame.
    idle_secs: u32,
    /// A fullscreen app / presentation is active.
    busy: bool,
    held_since: Option<i64>,
    work_start: i64,
    idle_credited: bool,
    /// Next due time (unix s) of the water / stand / posture reminders; 0 = not armed.
    micro_due: [i64; 3],
}

/// Why the work countdown is frozen by itself (not by the user).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    None,
    /// Away from the keyboard.
    Idle,
    /// Fullscreen app or presentation running.
    Busy,
    /// Outside the configured working hours.
    OffHours,
}

/// `now` (seconds since midnight) inside the window `a..b`, wrapping over midnight.
fn in_window(now: u32, a: u32, b: u32) -> bool {
    if a == b {
        false
    } else if a < b {
        now >= a && now < b
    } else {
        now >= a || now < b
    }
}

impl Engine {
    pub fn new(mut settings: Settings, clock: Box<dyn Clock>) -> Engine {
        settings.clamp();
        let mut e = Engine {
            settings,
            clock,
            delayed: Vec::new(),
            ph: Ph::Off,
            deadline: 0,
            warned: false,
            last_now: None,
            next_fire: Vec::new(),
            active: Vec::new(),
            night_level: 0,
            pending: Vec::new(),
            paused_left: None,
            work_min_applied: 0,
            cycle: 0,
            cur_break: (0, false),
            stats: Stats::default(),
            idle_secs: 0,
            busy: false,
            held_since: None,
            work_start: 0,
            idle_credited: false,
            micro_due: [0; 3],
        };
        e.work_min_applied = e.settings.work_min;
        e.recompute_sched();
        if e.settings.break_enabled {
            e.start_break_timer();
        }
        e
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Call after editing via settings_mut(): re-clamps and reschedules.
    pub fn apply_settings_changed(&mut self) {
        self.settings.clamp();
        self.recompute_sched();
        let new = self.settings.work_min as i64 * 60;
        let delta = new - self.work_min_applied as i64 * 60;
        self.work_min_applied = self.settings.work_min;
        if self.ph == Ph::Working {
            let now = self.clock.now_unix();
            if let Some(left) = self.paused_left.as_mut() {
                *left = (*left + delta).clamp(0, new);
            } else {
                self.deadline = (self.deadline + delta).clamp(now, now + new);
            }
        }
    }

    fn now(&self) -> i64 {
        self.clock.now_unix()
    }

    fn day_start(&self, now: i64) -> i64 {
        now - self.clock.secs_since_midnight() as i64
    }

    fn fire_time(&self, s: &Sched, now: i64) -> i64 {
        let t = self.day_start(now) + s.h as i64 * 3600 + s.m as i64 * 60;
        if t <= now {
            t + 86400
        } else {
            t
        }
    }

    fn recompute_sched(&mut self) {
        let now = self.now();
        let nf: Vec<(u32, i64)> = self
            .settings
            .scheds
            .iter()
            .map(|s| (s.id, self.fire_time(s, now)))
            .collect();
        self.next_fire = nf;
    }

    // ---- break timer
    pub fn start_break_timer(&mut self) {
        self.settings.break_enabled = true;
        self.paused_left = None;
        self.held_since = None;
        self.work_start = self.now();
        self.ph = Ph::Working;
        self.deadline = self.now() + self.settings.work_min as i64 * 60;
        self.work_min_applied = self.settings.work_min;
        self.warned = false;
    }

    pub fn stop_break_timer(&mut self) {
        self.settings.break_enabled = false;
        self.paused_left = None;
        if self.ph == Ph::Break {
            self.pending.push(Action::BreakEnd);
        }
        self.ph = Ph::Off;
    }

    /// End a running break (or restart the work period when working/warning).
    /// A running break cannot be skipped in strict mode (`allow_skip == false`); returns whether it did anything.
    pub fn skip_break(&mut self) -> bool {
        self.paused_left = None;
        match self.ph {
            Ph::Break => {
                if !self.settings.allow_skip {
                    return false;
                }
                let day = self.today();
                self.stats.record_skipped(day);
                self.pending.push(Action::BreakEnd);
                self.restart_work();
                true
            }
            Ph::Working => {
                self.restart_work();
                true
            }
            Ph::Off => false,
        }
    }

    /// Postpone the next break by `mins` minutes from now.
    pub fn snooze(&mut self, mins: u32) {
        if self.ph == Ph::Break {
            self.pending.push(Action::BreakEnd);
        }
        if self.ph != Ph::Off {
            let day = self.today();
            self.stats.record_snoozed(day);
            self.paused_left = None;
            self.ph = Ph::Working;
            self.deadline = self.now() + mins.clamp(1, 240) as i64 * 60;
            self.warned = false;
        }
    }

    /// Snooze by the length chosen in settings.
    pub fn snooze_default(&mut self) {
        let m = self.settings.snooze_mins;
        self.snooze(m);
    }

    /// Freeze the countdown (work or break) until `resume`.
    pub fn pause(&mut self) {
        if self.ph != Ph::Off && self.paused_left.is_none() {
            self.paused_left = Some((self.deadline - self.now()).max(0));
        }
    }

    pub fn resume(&mut self) {
        if let Some(left) = self.paused_left.take() {
            self.deadline = self.now() + left;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused_left.is_some()
    }

    /// Add `secs` to the running break (max one hour from now).
    pub fn extend_break(&mut self, secs: u32) {
        if self.ph == Ph::Break {
            let cap = self.now() + 3600;
            self.deadline = (self.deadline + secs as i64).min(cap);
            self.cur_break.0 = self.cur_break.0.saturating_add(secs).min(3600);
            if let Some(l) = self.paused_left.as_mut() {
                *l = (*l + secs as i64).min(3600);
            }
        }
    }

    /// Begin a break right now (also starts the timer when it was off). Ignored during a break.
    pub fn start_break_now(&mut self) {
        if self.ph == Ph::Break {
            return;
        }
        self.settings.break_enabled = true;
        self.paused_left = None;
        let now = self.now();
        let mut out = Vec::new();
        self.begin_break(now, &mut out);
        self.pending.extend(out);
    }

    fn restart_work(&mut self) {
        self.ph = Ph::Working;
        self.deadline = self.now() + self.settings.work_min as i64 * 60;
        self.work_min_applied = self.settings.work_min;
        self.work_start = self.now();
        self.held_since = None;
        self.warned = false;
    }

    /// Feed the sensors: seconds since last input and whether a fullscreen app is running.
    pub fn set_env(&mut self, idle_secs: u32, busy: bool) {
        self.idle_secs = idle_secs;
        self.busy = busy;
    }

    /// The reason the work countdown is frozen right now, if any.
    pub fn hold(&self) -> Hold {
        if self.ph != Ph::Working || self.paused_left.is_some() {
            return Hold::None;
        }
        self.hold_reason()
    }

    fn hold_reason(&self) -> Hold {
        let s = &self.settings;
        if s.dnd_fullscreen && self.busy {
            return Hold::Busy;
        }
        if s.hours_enabled {
            let now = self.clock.secs_since_midnight();
            let a = s.hours_start.0 as u32 * 3600 + s.hours_start.1 as u32 * 60;
            let b = s.hours_end.0 as u32 * 3600 + s.hours_end.1 as u32 * 60;
            if !in_window(now, a, b) {
                return Hold::OffHours;
            }
        }
        if s.idle_pause_secs > 0 && self.idle_secs >= s.idle_pause_secs {
            return Hold::Idle;
        }
        Hold::None
    }

    /// Local day number (days since 1970-01-01 at local midnight).
    pub fn today(&self) -> u32 {
        let now = self.now();
        let start = self.day_start(now);
        ((start + 43200).div_euclid(86400)).max(0) as u32
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }
    pub fn stats_mut(&mut self) -> &mut Stats {
        &mut self.stats
    }
    pub fn set_stats(&mut self, s: Stats) {
        self.stats = s;
    }

    // ---- schedules / manual offs
    pub fn add_sched(&mut self, mut s: Sched) -> Result<u32, String> {
        if self.settings.scheds.len() >= MAX_SCHEDS {
            return Err(format!("at most {} schedules", MAX_SCHEDS));
        }
        s.h = s.h.min(23);
        s.m = s.m.min(59);
        s.mins = s.mins.clamp(1, 720);
        if self
            .settings
            .scheds
            .iter()
            .any(|o| o.h == s.h && o.m == s.m && o.targets == s.targets)
        {
            return Err("duplicate schedule".into());
        }
        let id = self.settings.scheds.iter().map(|o| o.id).max().unwrap_or(0) + 1;
        s.id = id;
        let ft = self.fire_time(&s, self.now());
        self.settings.scheds.push(s);
        self.next_fire.push((id, ft));
        Ok(id)
    }

    /// Remove a schedule; returns ScreenOn if it was mid-off.
    pub fn cancel_sched(&mut self, id: u32) -> Vec<Action> {
        self.settings.scheds.retain(|s| s.id != id);
        self.next_fire.retain(|(i, _)| *i != id);
        let mut out = Vec::new();
        let mut keep = Vec::new();
        let mut gone = Vec::new();
        for a in self.active.drain(..) {
            if a.sched_id == id {
                gone.push(a.targets);
            } else {
                keep.push(a);
            }
        }
        self.active = keep;
        for t in gone {
            if let Some(x) = self.on_action(t) {
                out.push(x);
            }
        }
        out
    }

    /// Manual timed screen-off; ScreenOn is emitted by tick() at the end.
    pub fn off_now(&mut self, targets: Vec<String>, mins: u32) -> Vec<Action> {
        self.off_now_secs(targets, mins.saturating_mul(60))
    }

    /// Manual timed screen-off for exactly `secs` seconds (1 s .. MAX_OFF_SECS). The screens come back
    /// at that second (tick emits ScreenOn); the hardware failsafe gets the length rounded up to a minute.
    pub fn off_now_secs(&mut self, targets: Vec<String>, secs: u32) -> Vec<Action> {
        let secs = secs.clamp(1, MAX_OFF_SECS);
        let end_ms = self.clock.now_ms() + secs as i64 * 1000;
        let end = (end_ms + 999).div_euclid(1000);
        let mins = secs.div_ceil(60);
        self.active.push(ActiveOff { sched_id: 0, targets: targets.clone(), end, end_ms, mins });
        vec![Action::ScreenOff { targets, mins }]
    }

    /// Screen-off that starts `delay_secs` from now and lasts `secs` (0 delay = start now).
    /// Delay is capped at 24 h like the length; a waiting off is lost when the app quits.
    pub fn off_in_secs(&mut self, targets: Vec<String>, delay_secs: u32, secs: u32) -> Vec<Action> {
        if delay_secs == 0 {
            return self.off_now_secs(targets, secs);
        }
        let start_ms = self.clock.now_ms() + delay_secs.min(MAX_OFF_SECS) as i64 * 1000;
        self.delayed.push(DelayedOff { targets, start_ms, secs: secs.clamp(1, MAX_OFF_SECS) });
        Vec::new()
    }

    /// Drop the waiting screen-off at `idx` (index into `view().delayed_off`).
    pub fn cancel_delayed(&mut self, idx: usize) {
        if idx < self.delayed.len() {
            self.delayed.remove(idx);
        }
    }

    /// End the manual screen-off holding exactly these targets now; ScreenOn unless another off still covers them.
    pub fn end_off(&mut self, targets: &[String]) -> Vec<Action> {
        let mut gone = Vec::new();
        self.active.retain(|a| if a.targets == targets { gone.push(a.targets.clone()); false } else { true });
        gone.into_iter().filter_map(|t| self.on_action(t)).collect()
    }

    /// Add `secs` to the screen-off holding these targets (at most 24 h from now) and re-issue the
    /// off so the hardware failsafe deadline moves with it.
    pub fn extend_off(&mut self, targets: &[String], secs: u32) -> Vec<Action> {
        let now = self.now();
        let mut out = Vec::new();
        for a in self.active.iter_mut().filter(|a| a.targets == targets) {
            a.end_ms = (a.end_ms + secs as i64 * 1000).min(self.clock.now_ms() + MAX_OFF_SECS as i64 * 1000);
            a.end = (a.end_ms + 999).div_euclid(1000);
            a.mins = (((a.end - now).max(60) + 59) / 60) as u32;
            out.push(Action::ScreenOff { targets: a.targets.clone(), mins: a.mins });
        }
        out
    }

    /// ScreenOn for everything currently off (call before quitting).
    pub fn restore_all(&mut self) -> Vec<Action> {
        self.active
            .drain(..)
            .map(|a| Action::ScreenOn { targets: a.targets })
            .collect()
    }

    pub fn sleep_now(&mut self) {
        self.pending.push(Action::SleepPc { mins: self.settings.sleep_min });
    }

    // ---- tick
    pub fn tick(&mut self) -> Vec<Action> {
        let now = self.now();
        let mut out: Vec<Action> = std::mem::take(&mut self.pending);

        let prev = self.last_now.unwrap_or(now);
        let gap = now - prev;
        let resumed = gap > RESUME_GAP_SECS;
        let back = now < prev;
        self.last_now = Some(now);
        if back {
            // clock moved backwards: re-anchor everything to the new time
            self.recompute_sched();
            if self.ph == Ph::Working || self.ph == Ph::Break {
                let cap = now + if self.ph == Ph::Break {
                    self.cur_break.0 as i64
                } else {
                    self.settings.work_min as i64 * 60
                };
                if self.deadline > cap {
                    self.deadline = cap;
                }
            }
            for a in &mut self.active {
                let cap = now + a.mins as i64 * 60;
                if a.end > cap {
                    a.end = cap;
                    a.end_ms = cap * 1000;
                }
            }
        }

        self.tick_breaks(now, resumed, &mut out);
        self.tick_micro(now, gap, &mut out);
        self.tick_scheds(now, &mut out);
        self.tick_night(&mut out);
        out
    }

    /// Independent hydrate / stand / posture reminders: each fires once per interval while the
    /// timer is working and not paused; time spent paused or on a break does not count.
    fn tick_micro(&mut self, now: i64, gap: i64, out: &mut Vec<Action>) {
        let iv = [self.settings.micro_water, self.settings.micro_stand, self.settings.micro_posture];
        let live = self.ph == Ph::Working && self.paused_left.is_none();
        for (i, mins) in iv.into_iter().enumerate() {
            if mins == 0 {
                self.micro_due[i] = 0;
            } else if self.micro_due[i] == 0 {
                self.micro_due[i] = now + mins as i64 * 60;
            } else if !live {
                self.micro_due[i] += gap.max(0);
            } else if now >= self.micro_due[i] {
                out.push(Action::Micro(i as u8));
                self.micro_due[i] = now + mins as i64 * 60;
            }
        }
    }

    fn tick_breaks(&mut self, now: i64, resumed: bool, out: &mut Vec<Action>) {
        if self.paused_left.is_some() {
            return;
        }
        if self.idle_secs < 5 {
            self.idle_credited = false;
        }
        // Being away for a whole break counts as a break (needs a minute of real work first).
        if self.ph == Ph::Working
            && self.settings.idle_credit
            && !self.idle_credited
            && !resumed
            && self.idle_secs >= self.settings.break_secs
            && now - self.work_start >= 60
        {
            self.idle_credited = true;
            let day = self.today();
            self.stats.record_taken(day, self.idle_secs.min(3600));
            self.cycle += 1;
            self.restart_work();
            out.push(Action::Notify("Away detected: counted as a break".into()));
            return;
        }
        // Self-freezing work countdown (away, fullscreen app, outside working hours).
        let held = self.ph == Ph::Working && self.hold_reason() != Hold::None;
        match (self.held_since, held) {
            (None, true) => self.held_since = Some(now),
            (Some(t), false) => {
                self.deadline += (now - t).max(0);
                self.held_since = None;
            }
            _ => {}
        }
        if held {
            return;
        }
        match self.ph {
            Ph::Off => {}
            Ph::Working => {
                if now >= self.deadline {
                    if resumed {
                        // stale break after sleep/resume: do not fire
                        self.restart_work();
                    } else {
                        self.begin_break(now, out);
                    }
                } else if !self.warned
                    && self.settings.warn_secs > 0
                    && !resumed
                    && self.deadline - now <= self.settings.warn_secs as i64
                {
                    self.warned = true;
                    out.push(Action::Warn { secs_left: (self.deadline - now) as u32 });
                }
            }
            Ph::Break => {
                if now >= self.deadline {
                    let day = self.today();
                    self.stats.record_taken(day, self.cur_break.0);
                    self.cycle += 1;
                    out.push(Action::BreakEnd);
                    self.restart_work();
                    if !self.settings.auto_resume {
                        self.pause();
                    }
                }
            }
        }
    }

    fn begin_break(&mut self, now: i64, out: &mut Vec<Action>) {
        let every = self.settings.long_break_every;
        let long = every > 0 && (self.cycle + 1).is_multiple_of(every);
        let secs = if long { self.settings.long_break_secs } else { self.settings.break_secs };
        self.cur_break = (secs, long);
        self.ph = Ph::Break;
        self.deadline = now + secs as i64;
        out.push(Action::BreakStart { secs });
    }

    fn tick_scheds(&mut self, now: i64, out: &mut Vec<Action>) {
        // ends first
        let now_ms = self.clock.now_ms();
        let mut i = 0;
        while i < self.active.len() {
            if now_ms >= self.active[i].end_ms {
                let a = self.active.remove(i);
                if let Some(x) = self.on_action(a.targets) {
                    out.push(x);
                }
            } else {
                i += 1;
            }
        }
        // delayed manual offs whose start has come
        let mut j = 0;
        while j < self.delayed.len() {
            if now_ms >= self.delayed[j].start_ms {
                let d = self.delayed.remove(j);
                out.extend(self.off_now_secs(d.targets, d.secs));
            } else {
                j += 1;
            }
        }
        // starts
        let mut fired: Vec<u32> = Vec::new();
        for (id, t) in &self.next_fire {
            if now >= *t {
                fired.push(*id);
            }
        }
        for id in fired {
            let Some(s) = self.settings.scheds.iter().find(|s| s.id == id).cloned() else { continue };
            let t = self.next_fire.iter().find(|(i, _)| *i == id).map(|x| x.1).unwrap_or(now);
            let late = now - t;
            // late > 60 s (e.g. PC was asleep): skip this occurrence
            if late > SCHED_LATE_SECS {
                log_line(&format!("schedule {:02}:{:02} skipped, {} s late", s.h, s.m, late));
            } else {
                let end = now + s.mins as i64 * 60;
                self.active.push(ActiveOff { sched_id: id, targets: s.targets.clone(), end, end_ms: end * 1000, mins: s.mins });
                out.push(Action::ScreenOff { targets: s.targets.clone(), mins: s.mins });
            }
            if s.daily {
                let nt = self.fire_time(&s, now);
                if let Some(e) = self.next_fire.iter_mut().find(|(i, _)| *i == id) {
                    e.1 = nt;
                }
            } else {
                self.settings.scheds.retain(|x| x.id != id);
                self.next_fire.retain(|(i, _)| *i != id);
            }
        }
    }

    /// ScreenOn for `targets`, minus any still held off by another active entry.
    fn on_action(&self, targets: Vec<String>) -> Option<Action> {
        if self.active.iter().any(|a| a.targets.is_empty()) && !targets.is_empty() {
            return None;
        }
        if targets.is_empty() {
            return Some(Action::ScreenOn { targets });
        }
        let left: Vec<String> = targets
            .into_iter()
            .filter(|t| !self.active.iter().any(|a| a.targets.contains(t)))
            .collect();
        if left.is_empty() {
            None
        } else {
            Some(Action::ScreenOn { targets: left })
        }
    }

    /// Flip the night overlay right now (until toggled again or `night_auto`).
    pub fn night_toggle(&mut self) {
        self.settings.night_force = if self.night_level > 0 { 2 } else { 1 };
    }

    /// Go back to following the From / Until schedule.
    pub fn night_auto(&mut self) {
        self.settings.night_force = 0;
    }

    fn night_window(&self) -> bool {
        let s = &self.settings;
        match s.night_force {
            1 => return true,
            2 => return false,
            _ => {}
        }
        if !s.night_enabled {
            return false;
        }
        let now = self.clock.secs_since_midnight();
        let a = s.night_start.0 as u32 * 3600 + s.night_start.1 as u32 * 60;
        let b = s.night_end.0 as u32 * 3600 + s.night_end.1 as u32 * 60;
        in_window(now, a, b)
    }

    fn tick_night(&mut self, out: &mut Vec<Action>) {
        let want = if self.night_window() { self.settings.night_level } else { 0 };
        if want != self.night_level {
            self.night_level = want;
            out.push(Action::NightSet { level: want });
        }
    }

    pub fn view(&self) -> EngineView {
        let now = self.now();
        let left = match (self.paused_left, self.held_since) {
            (Some(l), _) => l.max(0) as u32,
            (None, Some(t)) => (self.deadline - t).max(0) as u32,
            (None, None) => (self.deadline - now).max(0) as u32,
        };
        let phase = match self.ph {
            Ph::Off => Phase::Off,
            Ph::Working if self.warned => Phase::Warning { left },
            Ph::Working => Phase::Working { left },
            Ph::Break => Phase::Break { left },
        };
        EngineView {
            phase,
            next_sched: self.next_fire.iter().min_by_key(|(_, t)| *t).copied(),
            night_active: self.night_level > 0,
            paused: self.paused_left.is_some(),
            hold: self.hold(),
            break_is_long: self.cur_break.1,
            break_total: if self.ph == Ph::Break { self.cur_break.0 } else { self.settings.break_secs },
            cycle: self.cycle,
            active_off: self.active.iter().map(|a| (a.targets.clone(), a.end)).collect(),
            delayed_off: self.delayed.iter().map(|d| (d.targets.clone(), (d.start_ms + 999).div_euclid(1000), d.secs)).collect(),
        }
    }
}

/// Non-empty settings bytes that are binary or have no `key=value` line.
fn looks_corrupt(b: &[u8]) -> bool {
    if b.iter().all(|c| c.is_ascii_whitespace()) {
        return false;
    }
    b.contains(&0) || !String::from_utf8_lossy(b).lines().any(|l| l.contains('='))
}

// ---------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_settings_detected_and_parse_gives_defaults() {
        assert!(looks_corrupt(&[0, 159, 146, 150, 0]));
        assert!(looks_corrupt(b"garbage without equals"));
        assert!(!looks_corrupt(b""));
        assert!(!looks_corrupt(b"work_min=33\n"));
        let d = Settings::default();
        assert_eq!(Settings::parse("\u{0}\u{1}garbage").work_min, d.work_min);
        assert_eq!(Settings::parse("").work_min, d.work_min);
    }
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Clone)]
    struct Fake(Rc<Cell<i64>>);
    impl Clock for Fake {
        fn now_unix(&self) -> i64 {
            self.0.get()
        }
        fn secs_since_midnight(&self) -> u32 {
            self.0.get().rem_euclid(86400) as u32
        }
    }

    const DAY: i64 = 1_700_000_000 - 1_700_000_000 % 86400; // a midnight (UTC==local in tests)

    fn mk(s: Settings, t: i64) -> (Engine, Rc<Cell<i64>>) {
        let c = Rc::new(Cell::new(t));
        (Engine::new(s, Box::new(Fake(c.clone()))), c)
    }
    fn adv(e: &mut Engine, c: &Rc<Cell<i64>>, secs: i64) -> Vec<Action> {
        let mut all = Vec::new();
        for _ in 0..secs {
            c.set(c.get() + 1);
            all.extend(e.tick());
        }
        all
    }
    fn brk() -> Settings {
        Settings { work_min: 1, break_secs: 5, warn_secs: 10, break_enabled: true, ..Settings::default() }
    }
    fn sc(h: u8, m: u8, mins: u32, daily: bool) -> Sched {
        Sched { id: 0, h, m, mins, targets: vec!["A".into()], daily }
    }

    #[test]
    fn phases_cycle() {
        let (mut e, c) = mk(brk(), DAY + 100);
        assert_eq!(e.view().phase, Phase::Working { left: 60 });
        let a = adv(&mut e, &c, 49);
        assert!(a.is_empty());
        let a = adv(&mut e, &c, 1);
        assert_eq!(a, vec![Action::Warn { secs_left: 10 }]);
        assert_eq!(e.view().phase, Phase::Warning { left: 10 });
        let a = adv(&mut e, &c, 10);
        assert_eq!(a, vec![Action::BreakStart { secs: 5 }]);
        assert_eq!(e.view().phase, Phase::Break { left: 5 });
        let a = adv(&mut e, &c, 5);
        assert_eq!(a, vec![Action::BreakEnd]);
        assert_eq!(e.view().phase, Phase::Working { left: 60 });
    }

    fn micro_count(a: &[Action], k: u8) -> usize {
        a.iter().filter(|x| **x == Action::Micro(k)).count()
    }

    #[test]
    fn micro_reminders_fire_once_per_interval() {
        let mut s = Settings { work_min: 240, micro_water: 5, micro_stand: 10, ..Settings::default() };
        s.break_enabled = true;
        let (mut e, c) = mk(s, DAY);
        let a = adv(&mut e, &c, 5 * 60);
        assert!(a.is_empty());
        let a = adv(&mut e, &c, 1);
        assert_eq!(micro_count(&a, 0), 1);
        let a = adv(&mut e, &c, 5 * 60);
        assert_eq!((micro_count(&a, 0), micro_count(&a, 1)), (1, 1), "water again, stand at 10 min");
        assert_eq!(micro_count(&a, 2), 0, "posture is off");
    }

    #[test]
    fn micro_reminders_respect_pause() {
        let mut s = Settings { work_min: 240, micro_water: 5, ..Settings::default() };
        s.break_enabled = true;
        let (mut e, c) = mk(s, DAY);
        adv(&mut e, &c, 200);
        e.pause();
        let a = adv(&mut e, &c, 3600);
        assert_eq!(micro_count(&a, 0), 0, "no reminder while paused");
        e.resume();
        let a = adv(&mut e, &c, 99);
        assert_eq!(micro_count(&a, 0), 0, "paused time does not count");
        let a = adv(&mut e, &c, 2);
        assert_eq!(micro_count(&a, 0), 1);
    }

    #[test]
    fn warn_only_once() {
        let (mut e, c) = mk(brk(), DAY);
        let a = adv(&mut e, &c, 59);
        assert_eq!(a.iter().filter(|x| matches!(x, Action::Warn { .. })).count(), 1);
    }

    #[test]
    fn no_warn_when_zero() {
        let mut s = brk();
        s.warn_secs = 0;
        let (mut e, c) = mk(s, DAY);
        let a = adv(&mut e, &c, 60);
        assert_eq!(a, vec![Action::BreakStart { secs: 5 }]);
    }

    #[test]
    fn disabled_does_nothing() {
        let mut s = brk();
        s.break_enabled = false;
        let (mut e, c) = mk(s, DAY);
        assert_eq!(e.view().phase, Phase::Off);
        assert!(adv(&mut e, &c, 200).is_empty());
    }

    #[test]
    fn stop_timer() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 30);
        e.stop_break_timer();
        assert_eq!(e.view().phase, Phase::Off);
        assert!(adv(&mut e, &c, 100).is_empty());
        assert!(!e.settings().break_enabled);
    }

    #[test]
    fn stop_during_break_emits_end() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 61);
        assert!(matches!(e.view().phase, Phase::Break { .. }));
        e.stop_break_timer();
        assert_eq!(adv(&mut e, &c, 1), vec![Action::BreakEnd]);
    }

    #[test]
    fn skip_break_ends_and_restarts() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 61);
        e.skip_break();
        let a = adv(&mut e, &c, 1);
        assert_eq!(a, vec![Action::BreakEnd]);
        assert_eq!(e.view().phase, Phase::Working { left: 59 });
    }

    #[test]
    fn skip_while_working_restarts_period() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 55);
        e.skip_break();
        assert_eq!(e.view().phase, Phase::Working { left: 60 });
    }

    #[test]
    fn snooze_postpones() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 55);
        e.snooze(5);
        assert_eq!(e.view().phase, Phase::Working { left: 300 });
        let a = adv(&mut e, &c, 100);
        assert!(a.is_empty());
    }

    #[test]
    fn snooze_during_break_ends_it() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 61);
        e.snooze(2);
        assert_eq!(adv(&mut e, &c, 1), vec![Action::BreakEnd]);
        assert!(matches!(e.view().phase, Phase::Working { .. }));
    }

    #[test]
    fn resume_gap_skips_stale_break() {
        let (mut e, c) = mk(brk(), DAY);
        e.tick();
        c.set(c.get() + 3600);
        let a = e.tick();
        assert!(a.is_empty(), "{:?}", a);
        assert_eq!(e.view().phase, Phase::Working { left: 60 });
    }

    #[test]
    fn resume_gap_ends_expired_break() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 61);
        c.set(c.get() + 3600);
        let a = e.tick();
        assert_eq!(a, vec![Action::BreakEnd]);
    }

    #[test]
    fn sched_once_fires_and_ends() {
        let (mut e, c) = mk(Settings::default(), DAY + 3600 * 10 - 5);
        let id = e.add_sched(sc(10, 0, 1, false)).unwrap();
        assert_eq!(id, 1);
        let a = adv(&mut e, &c, 5);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["A".into()], mins: 1 }]);
        assert!(e.settings().scheds.is_empty());
        assert_eq!(e.view().active_off.len(), 1);
        let a = adv(&mut e, &c, 59);
        assert!(a.is_empty());
        let a = adv(&mut e, &c, 1);
        assert_eq!(a, vec![Action::ScreenOn { targets: vec!["A".into()] }]);
        assert!(adv(&mut e, &c, 86400).is_empty());
    }

    #[test]
    fn sched_daily_repeats() {
        let (mut e, c) = mk(Settings::default(), DAY + 3600 * 10 - 1);
        e.add_sched(sc(10, 0, 1, true)).unwrap();
        let a = adv(&mut e, &c, 86400 + 200);
        let offs = a.iter().filter(|x| matches!(x, Action::ScreenOff { .. })).count();
        let ons = a.iter().filter(|x| matches!(x, Action::ScreenOn { .. })).count();
        assert_eq!((offs, ons), (2, 2));
        assert_eq!(e.settings().scheds.len(), 1);
    }

    #[test]
    fn sched_across_midnight() {
        let (mut e, c) = mk(Settings::default(), DAY + 86400 - 10);
        e.add_sched(sc(0, 0, 1, true)).unwrap();
        let a = adv(&mut e, &c, 10);
        assert_eq!(a.len(), 1);
        assert!(matches!(a[0], Action::ScreenOff { .. }));
    }

    #[test]
    fn sched_late_after_resume_skipped() {
        let (mut e, c) = mk(Settings::default(), DAY + 9 * 3600);
        e.add_sched(sc(10, 0, 5, true)).unwrap();
        e.tick();
        c.set(DAY + 10 * 3600 + 120); // woke 2 min late
        assert!(e.tick().is_empty());
        let nx = e.view().next_sched.unwrap().1;
        assert_eq!(nx, DAY + 86400 + 10 * 3600);
    }

    #[test]
    fn sched_within_60s_after_resume_fires() {
        let (mut e, c) = mk(Settings::default(), DAY + 9 * 3600);
        e.add_sched(sc(10, 0, 5, true)).unwrap();
        e.tick();
        c.set(DAY + 10 * 3600 + 30);
        let a = e.tick();
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn sched_duplicates_rejected() {
        let (mut e, _c) = mk(Settings::default(), DAY);
        e.add_sched(sc(10, 0, 5, true)).unwrap();
        assert!(e.add_sched(sc(10, 0, 9, false)).is_err());
        assert!(e.add_sched(sc(10, 1, 5, true)).is_ok());
    }

    #[test]
    fn sched_max_16() {
        let (mut e, _c) = mk(Settings::default(), DAY);
        for i in 0..16 {
            e.add_sched(sc(i as u8, 0, 5, true)).unwrap();
        }
        assert!(e.add_sched(sc(20, 0, 5, true)).is_err());
    }

    #[test]
    fn cancel_active_sched_turns_on() {
        let (mut e, c) = mk(Settings::default(), DAY + 3600 * 10 - 1);
        let id = e.add_sched(sc(10, 0, 5, true)).unwrap();
        adv(&mut e, &c, 1);
        let a = e.cancel_sched(id);
        assert_eq!(a, vec![Action::ScreenOn { targets: vec!["A".into()] }]);
        assert!(e.view().next_sched.is_none());
        assert!(adv(&mut e, &c, 600).is_empty());
    }

    #[test]
    fn clock_back_reanchors() {
        let (mut e, c) = mk(brk(), DAY + 5000);
        e.add_sched(sc(2, 0, 1, true)).unwrap();
        adv(&mut e, &c, 30);
        c.set(DAY + 100); // jump back
        e.tick();
        assert_eq!(e.view().next_sched.unwrap().1, DAY + 7200);
        assert!(matches!(e.view().phase, Phase::Working { left } if left <= 60));
    }

    #[test]
    fn off_now_and_restore() {
        let (mut e, c) = mk(Settings::default(), DAY);
        let a = e.off_now(vec!["X".into()], 2);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["X".into()], mins: 2 }]);
        let a = adv(&mut e, &c, 120);
        assert_eq!(a, vec![Action::ScreenOn { targets: vec!["X".into()] }]);
        e.off_now(vec!["Y".into()], 2);
        assert_eq!(e.restore_all().len(), 1);
        assert!(e.view().active_off.is_empty());
    }

    #[test]
    fn off_in_starts_later_for_chosen_length_and_cancels() {
        let (mut e, c) = mk(Settings::default(), DAY);
        // delay 600 s, length 90 s: nothing now, view shows the waiting off
        assert!(e.off_in_secs(vec!["X".into()], 600, 90).is_empty());
        assert_eq!(e.view().delayed_off, vec![(vec!["X".to_string()], DAY + 600, 90)]);
        assert!(e.view().active_off.is_empty());
        assert!(adv(&mut e, &c, 599).is_empty());
        // at +600 s the screen goes off for exactly 90 s
        assert_eq!(adv(&mut e, &c, 1), vec![Action::ScreenOff { targets: vec!["X".into()], mins: 2 }]);
        assert!(e.view().delayed_off.is_empty());
        assert_eq!(e.view().active_off[0].1, c.get() + 90);
        assert_eq!(adv(&mut e, &c, 90), vec![Action::ScreenOn { targets: vec!["X".into()] }]);
        // delay 0 = immediate; a cancelled waiting off never fires
        assert_eq!(e.off_in_secs(vec!["Y".into()], 0, 60), vec![Action::ScreenOff { targets: vec!["Y".into()], mins: 1 }]);
        e.restore_all();
        e.off_in_secs(vec!["Z".into()], 30, 60);
        e.cancel_delayed(0);
        assert!(e.view().delayed_off.is_empty());
        assert!(adv(&mut e, &c, 31).is_empty());
    }

    #[test]
    fn off_now_exact_seconds_and_long() {
        // 90 s: helper gets 2 min, screens come back at exactly 90 s, not before
        let (mut e, c) = mk(Settings::default(), DAY);
        let a = e.off_now_secs(vec!["X".into()], 90);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["X".into()], mins: 2 }]);
        assert!(adv(&mut e, &c, 89).is_empty());
        assert_eq!(adv(&mut e, &c, 1), vec![Action::ScreenOn { targets: vec!["X".into()] }]);
        // 3 h 20 min (past the old 15 min default and the old 10 h cap)
        let a = e.off_now_secs(vec!["Y".into()], 12_000);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["Y".into()], mins: 200 }]);
        assert_eq!(e.view().active_off[0].1, c.get() + 12_000);
        // over 24 h is capped at 24 h
        e.restore_all();
        e.off_now_secs(vec!["Z".into()], 999_999);
        assert_eq!(e.view().active_off[0].1, c.get() + MAX_OFF_SECS as i64);
    }

    #[derive(Clone)]
    struct FakeMs(Rc<Cell<i64>>);
    impl Clock for FakeMs {
        fn now_unix(&self) -> i64 {
            self.0.get().div_euclid(1000)
        }
        fn secs_since_midnight(&self) -> u32 {
            self.now_unix().rem_euclid(86400) as u32
        }
        fn now_ms(&self) -> i64 {
            self.0.get()
        }
    }

    #[test]
    fn off_secs_end_at_the_exact_millisecond() {
        // started 900 ms into a second: 1 s must not end at the next whole second (100 ms later)
        let c = Rc::new(Cell::new(DAY * 1000 + 900));
        let mut e = Engine::new(Settings::default(), Box::new(FakeMs(c.clone())));
        e.off_now_secs(vec!["X".into()], 1);
        e.tick();
        c.set(DAY * 1000 + 1000);
        assert!(e.tick().is_empty(), "ended 900 ms early");
        c.set(DAY * 1000 + 1899);
        assert!(e.tick().is_empty());
        c.set(DAY * 1000 + 1900);
        assert_eq!(e.tick(), vec![Action::ScreenOn { targets: vec!["X".into()] }]);
    }

    #[test]
    fn duration_text_parses() {
        for (t, v) in [
            ("90", 5400), ("45s", 45), ("20m", 1200), ("1h", 3600), ("1h30m", 5400), ("1h 30m 15s", 5415),
            ("2.5h", 9000), ("1:30", 5400), ("0:00:45", 45), ("1:30:15", 5415), ("7 min", 420),
            ("2 hours 5 minutes", 7500), ("24h", 86400), (" 15 ", 900), ("1.5", 90),
        ] {
            assert_eq!(parse_duration(t), Some(v), "{t}");
        }
        for t in ["", "0", "0s", "abc", "25h", "1:60", "1:2:3:4", "5x", "h", "-5", "1h30", "1:-1"] {
            assert_eq!(parse_duration(t), None, "{t}");
        }
        assert_eq!(fmt_duration(45), "45 s");
        assert_eq!(fmt_duration(900), "15 min");
        assert_eq!(fmt_duration(5400), "1 h 30 min");
        assert_eq!(fmt_duration(3605), "1 h 0 min 5 s");
        assert_eq!(fmt_duration(7200), "2 h");
    }

    #[test]
    fn off_secs_saved_and_clamped() {
        let mut s = Settings::default();
        assert_eq!(s.off_secs, 900);
        s.off_secs = 5415;
        let back = Settings::parse(&s.serialize());
        assert_eq!(back.off_secs, 5415);
        assert_eq!(Settings::parse("off_secs=0\n").off_secs, 1);
        assert_eq!(Settings::parse("off_secs=999999\n").off_secs, MAX_OFF_SECS);
    }

    #[test]
    fn end_and_extend_off() {
        let (mut e, c) = mk(Settings::default(), DAY);
        e.off_now(vec!["X".into()], 2);
        let a = e.extend_off(&["X".to_string()], 300);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["X".into()], mins: 7 }]);
        assert_eq!(e.view().active_off[0].1, DAY + 420);
        assert!(adv(&mut e, &c, 120).is_empty(), "not restored at the old deadline");
        assert!(e.extend_off(&["Z".to_string()], 300).is_empty());
        let a = e.end_off(&["X".to_string()]);
        assert_eq!(a, vec![Action::ScreenOn { targets: vec!["X".into()] }]);
        assert!(e.view().active_off.is_empty());
        assert!(e.end_off(&["X".to_string()]).is_empty());
        assert!(adv(&mut e, &c, 600).is_empty(), "no second ScreenOn");
        e.off_now(vec!["Y".into()], 700);
        e.extend_off(&["Y".to_string()], 99999);
        assert_eq!(e.view().active_off[0].1, c.get() + MAX_OFF_SECS as i64, "extend capped at 24 h from now");
    }

    #[test]
    fn break_custom_three_min_off_restores_at_deadline() {
        let (mut e, c) = mk(Settings::default(), DAY);
        let a = e.off_now(vec!["Screen 1".into()], 3);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["Screen 1".into()], mins: 3 }]);
        assert!(adv(&mut e, &c, 179).is_empty(), "still off before 3 min");
        let a = adv(&mut e, &c, 1);
        assert_eq!(a, vec![Action::ScreenOn { targets: vec!["Screen 1".into()] }]);
    }

    #[test]
    fn monitor_selection_off_touches_only_chosen_screen() {
        let (mut e, _c) = mk(Settings::default(), DAY);
        let a = e.off_now(vec!["Screen 2".into()], 3);
        assert_eq!(a, vec![Action::ScreenOff { targets: vec!["Screen 2".into()], mins: 3 }]);
        let names: Vec<String> = e.view().active_off.iter().flat_map(|(t, _)| t.clone()).collect();
        assert_eq!(names, vec!["Screen 2".to_string()]);
    }

    #[test]
    fn overlapping_offs_keep_target_off() {
        let (mut e, c) = mk(Settings::default(), DAY);
        e.off_now(vec!["A".into()], 1);
        e.off_now(vec!["A".into(), "B".into()], 2);
        let a = adv(&mut e, &c, 60);
        assert_eq!(a, vec![]);
        let a = adv(&mut e, &c, 60);
        assert_eq!(a, vec![Action::ScreenOn { targets: vec!["A".into(), "B".into()] }]);
    }

    #[test]
    fn clock_back_caps_active_off() {
        let (mut e, c) = mk(Settings::default(), DAY + 7200);
        e.off_now(vec!["A".into()], 5);
        e.tick();
        c.set(DAY + 100);
        e.tick();
        assert!(e.view().active_off[0].1 <= DAY + 100 + 300);
    }

    #[test]
    fn sleep_action() {
        let mut s = Settings::default();
        s.sleep_min = 45;
        let (mut e, _c) = mk(s, DAY);
        e.sleep_now();
        assert_eq!(e.tick(), vec![Action::SleepPc { mins: 45 }]);
        assert!(e.tick().is_empty());
    }

    #[test]
    fn night_wraps_midnight() {
        let mut s = Settings::default();
        s.night_enabled = true;
        s.night_level = 60;
        let (mut e, c) = mk(s, DAY + 22 * 3600 - 2);
        assert!(e.tick().is_empty());
        let a = adv(&mut e, &c, 2);
        assert_eq!(a, vec![Action::NightSet { level: 60 }]);
        assert!(e.view().night_active);
        c.set(DAY + 86400 + 6 * 3600 - 1);
        e.tick();
        let a = adv(&mut e, &c, 1);
        assert_eq!(a, vec![Action::NightSet { level: 0 }]);
        assert!(!e.view().night_active);
    }

    #[test]
    fn night_manual_toggle_overrides_schedule_and_persists() {
        let (mut e, c) = mk(Settings::default(), DAY + 12 * 3600); // noon, schedule off
        assert!(e.tick().is_empty());
        e.night_toggle();
        assert_eq!(e.tick(), vec![Action::NightSet { level: 40 }]);
        assert!(e.view().night_active);
        let saved = e.settings().serialize();
        assert!(saved.contains("night_force=1"), "{saved}");
        assert_eq!(Settings::parse(&saved).night_force, 1);
        e.night_toggle();
        assert_eq!(e.tick(), vec![Action::NightSet { level: 0 }]);
        assert_eq!(e.settings().night_force, 2);
        e.night_auto();
        assert!(e.tick().is_empty(), "back on schedule at noon: still off");
        let mut s = Settings::default();
        s.night_enabled = true;
        let (mut e2, _c2) = mk(s, DAY + 23 * 3600);
        assert_eq!(e2.tick(), vec![Action::NightSet { level: 40 }]);
        e2.night_toggle();
        assert_eq!(e2.tick(), vec![Action::NightSet { level: 0 }], "forced off inside the night window");
        let _ = c;
    }

    #[test]
    fn night_disabled_clears() {
        let mut s = Settings::default();
        s.night_enabled = true;
        let (mut e, _c) = mk(s, DAY + 23 * 3600);
        assert_eq!(e.tick(), vec![Action::NightSet { level: 40 }]);
        e.settings_mut().night_enabled = false;
        e.apply_settings_changed();
        assert_eq!(e.tick(), vec![Action::NightSet { level: 0 }]);
    }

    #[test]
    fn settings_roundtrip() {
        let mut s = Settings::default();
        s.work_min = 45;
        s.break_enabled = true;
        s.off_mode = OffMode::Blackout;
        s.selected = vec![r"\\.\DISPLAY5".into(), r"\\.\DISPLAY2".into()];
        s.night_start = (21, 30);
        s.scheds.push(Sched { id: 7, h: 3, m: 15, mins: 90, targets: vec![r"\\.\DISPLAY5".into()], daily: true });
        let p = Settings::parse(&s.serialize());
        assert_eq!(p, s);
    }

    #[test]
    fn presets_default_roundtrip_edit_and_clamp() {
        let d = Settings::default();
        assert_eq!(d.presets.len(), 4);
        assert_eq!(Settings::parse(&d.serialize()).presets, d.presets);
        let mut s = Settings::default();
        s.presets = vec![
            BreakPreset { id: 9, name: "Lunch, long".into(), mins: 45, target: BreakTarget::Mon2, pause_agents: true },
            BreakPreset { id: 9, name: "dup".into(), mins: 5, target: BreakTarget::Mon1, pause_agents: false },
        ];
        let p = Settings::parse(&s.serialize());
        assert_eq!(p.presets.len(), 1, "duplicate id dropped");
        assert_eq!(p.presets[0].name, "Lunch, long");
        assert_eq!(p.presets[0].target, BreakTarget::Mon2);
        assert!(p.presets[0].pause_agents);
        s.presets.clear();
        assert!(Settings::parse(&s.serialize()).presets.is_empty(), "deleted presets stay deleted");
        let c = Settings::parse("work_min=20\npreset=1,99999,both,1,Big\npreset=2,5,zzz,1,Bad\npreset=3,x\n");
        assert_eq!(c.presets.len(), 1);
        assert_eq!(c.presets[0].mins, 720);
    }

    #[test]
    fn presets_add_edit_delete_persist() {
        let mut s = Settings::default();
        let i = preset_add(&mut s.presets).unwrap();
        assert_eq!(s.presets[i].id, 5);
        s.presets[i].name = "Stretch".into();
        s.presets[i].mins = 7;
        s.presets[i].target = BreakTarget::Mon1;
        s.presets[i].pause_agents = true;
        assert!(preset_delete(&mut s.presets, 2));
        assert!(!preset_delete(&mut s.presets, 2));
        let p = Settings::parse(&s.serialize());
        assert_eq!(p.presets.len(), 4);
        assert!(p.presets.iter().all(|x| x.id != 2));
        let n = p.presets.iter().find(|x| x.name == "Stretch").unwrap();
        assert_eq!((n.mins, n.target, n.pause_agents), (7, BreakTarget::Mon1, true));
        while preset_add(&mut s.presets).is_some() {}
        assert_eq!(s.presets.len(), MAX_PRESETS);
    }

    #[test]
    fn settings_corrupt_is_default() {
        let s = Settings::parse("\u{0}\u{1}garbage\n===\nwork_min=abc\nsched=1,2\nnight_start=99:99\n");
        assert_eq!(s, Settings::default());
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn settings_clamped() {
        let s = Settings::parse("work_min=0\nbreak_secs=99999\nwarn_secs=500\nsleep_min=0\nnight_level=250\nsched=1,5,5,9999,1,A\n");
        assert_eq!(s.work_min, 1);
        assert_eq!(s.break_secs, BREAK_MAX_SECS);
        assert_eq!(s.warn_secs, 120);
        assert_eq!(s.sleep_min, 1);
        assert_eq!(s.night_level, 100);
        assert_eq!(s.scheds[0].mins, 720);
    }

    #[test]
    fn work_minutes_any_length_up_to_a_day() {
        for m in [1u32, 7, 60, 61, 90, 241, 600, 1440] {
            let mut s = Settings::parse(&format!("work_min={m}\n"));
            s.clamp();
            assert_eq!(s.work_min, m, "work {m} min must be kept");
        }
        let mut s = Settings::parse("work_min=5000\n");
        s.clamp();
        assert_eq!(s.work_min, MAX_WORK_MIN);
    }

    #[test]
    fn work_min_any_length() {
        for m in [1u32, 1000, MAX_WORK_MIN] {
            let mut s = Settings::parse(&format!("work_min={m}\n"));
            s.clamp();
            assert_eq!(s.work_min, m, "work {m} min must be kept after clamp");
            let mut back = Settings::parse(&s.serialize());
            back.clamp();
            assert_eq!(back.work_min, m, "work {m} min must survive save/load");
        }
        assert_eq!(MAX_WORK_MIN, 1440);
        let mut s = Settings::parse("work_min=1441\n");
        s.clamp();
        assert_eq!(s.work_min, MAX_WORK_MIN);
    }

    #[test]
    fn long_break_any_length() {
        for secs in [7200u32, 14400] {
            let mut s = Settings::parse(&format!("long_break_secs={secs}\n"));
            s.clamp();
            assert_eq!(s.long_break_secs, secs, "long break {secs} s must be kept");
        }
    }

    #[test]
    fn break_actions_persist_roundtrip() {
        let mut s = Settings::default();
        s.break_actions = "sleep_secondary:0;mute:60".into();
        s.break_monitors = "\\\\.\\DISPLAY2".into();
        let back = Settings::parse(&s.serialize());
        assert_eq!(back.break_actions, s.break_actions);
        assert_eq!(back.break_monitors, s.break_monitors);
    }


    #[test]
    fn live_dashboard_work_break_values_drive_scheduler() {
        // app running with defaults (20 min / 20 s); user edits dashboard to work=60 break=7 min
        let (mut e, c) = mk(Settings { break_enabled: true, ..Settings::default() }, DAY);
        adv(&mut e, &c, 1);
        e.settings_mut().work_min = 60;
        e.settings_mut().break_secs = 7 * 60;
        e.apply_settings_changed();
        // no restart: next break is 60 min out
        assert!(matches!(e.view().phase, Phase::Working { left } if (3599..=3600).contains(&left)), "{:?}", e.view().phase);
        assert_eq!(e.view().break_total, 420);
        let w = e.settings().warn_secs as i64;
        let a = adv(&mut e, &c, 3599 - w);
        assert!(!a.iter().any(|x| matches!(x, Action::BreakStart { .. })), "no break before 60 min");
        let a = adv(&mut e, &c, w + 1);
        assert!(a.contains(&Action::BreakStart { secs: 420 }), "{:?}", a);
        assert!(matches!(e.view().phase, Phase::Break { left } if (419..=420).contains(&left)));
        println!("LIVE: next break at 60 min, length 7 min (420 s)");
    }

    #[test]
    fn work_len_live_replan() {
        let (mut e, c) = mk(Settings { work_min: 20, break_enabled: true, ..Settings::default() }, DAY);
        adv(&mut e, &c, 300); // 5 min elapsed, 15 left
        e.settings_mut().work_min = 30;
        e.apply_settings_changed();
        assert_eq!(e.work_min_applied, 30);
        // no restart: elapsed kept, remaining grows by 10 min -> 25 min
        assert!(matches!(e.view().phase, Phase::Working { left } if (1499..=1500).contains(&left)), "{:?}", e.view().phase);
        e.settings_mut().work_min = 10;
        e.apply_settings_changed();
        assert_eq!(e.work_min_applied, 10);
        assert!(matches!(e.view().phase, Phase::Working { left } if (299..=300).contains(&left)), "{:?}", e.view().phase);
    }

    fn long_work() -> Settings {
        Settings { work_min: 5, break_secs: 5, warn_secs: 10, break_enabled: true, ..Settings::default() }
    }

    #[test]
    fn idle_credit_counts_a_break_once_per_absence() {
        let (mut e, c) = mk(long_work(), DAY);
        adv(&mut e, &c, 70);
        e.set_env(10, false);
        let a = adv(&mut e, &c, 1);
        assert_eq!(a, vec![Action::Notify("Away detected: counted as a break".into())]);
        assert_eq!(e.stats().day(e.today()).taken, 1);
        assert!(matches!(e.view().phase, Phase::Working { left } if left >= 299));
        // still away: no second credit
        assert!(adv(&mut e, &c, 30).is_empty());
        assert_eq!(e.stats().day(e.today()).taken, 1);
        // back to work for 70 s, away again: second credit
        e.set_env(0, false);
        adv(&mut e, &c, 70);
        e.set_env(10, false);
        adv(&mut e, &c, 1);
        assert_eq!(e.stats().day(e.today()).taken, 2);
    }

    #[test]
    fn idle_credit_needs_real_work_and_the_setting() {
        let (mut e, c) = mk(long_work(), DAY);
        adv(&mut e, &c, 10);
        e.set_env(10, false);
        assert!(adv(&mut e, &c, 3).is_empty(), "less than a minute of work, no credit");
        let mut s = long_work();
        s.idle_credit = false;
        let (mut e, c) = mk(s, DAY);
        adv(&mut e, &c, 100);
        e.set_env(50, false);
        assert!(adv(&mut e, &c, 3).is_empty());
        assert_eq!(e.stats().total_taken(), 0);
    }

    #[test]
    fn idle_pause_freezes_work_timer() {
        let mut s = long_work();
        s.break_secs = 600;
        s.idle_pause_secs = 30;
        let (mut e, c) = mk(s, DAY);
        adv(&mut e, &c, 100);
        e.set_env(40, false);
        adv(&mut e, &c, 1);
        assert_eq!(e.view().hold, Hold::Idle);
        let frozen = e.view().phase;
        assert!(adv(&mut e, &c, 200).is_empty());
        assert_eq!(e.view().phase, frozen);
        e.set_env(0, false);
        adv(&mut e, &c, 1);
        assert_eq!(e.view().hold, Hold::None);
        assert!(matches!(e.view().phase, Phase::Working { left } if (198..=200).contains(&left)), "{:?}", e.view().phase);
    }

    #[test]
    fn fullscreen_dnd_holds_break_until_released() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 30);
        e.set_env(0, true);
        assert!(adv(&mut e, &c, 300).is_empty());
        assert_eq!(e.view().hold, Hold::Busy);
        e.set_env(0, false);
        let a = adv(&mut e, &c, 40);
        assert!(a.contains(&Action::BreakStart { secs: 5 }), "{a:?}");
        // setting off: fullscreen no longer holds
        let mut s = brk();
        s.dnd_fullscreen = false;
        let (mut e, c) = mk(s, DAY);
        e.set_env(0, true);
        assert!(adv(&mut e, &c, 61).contains(&Action::BreakStart { secs: 5 }));
    }

    #[test]
    fn off_hours_hold_breaks() {
        let mut s = brk();
        s.hours_enabled = true; // 09:00-18:00
        let (mut e, c) = mk(s, DAY + 20 * 3600);
        assert!(adv(&mut e, &c, 200).is_empty());
        assert_eq!(e.view().hold, Hold::OffHours);
        c.set(DAY + 10 * 3600);
        e.tick();
        assert_eq!(e.view().hold, Hold::None);
        assert!(adv(&mut e, &c, 70).contains(&Action::BreakStart { secs: 5 }));
    }

    #[test]
    fn pause_freezes_and_resume_continues() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 20);
        e.pause();
        assert!(e.is_paused() && e.view().paused);
        assert!(adv(&mut e, &c, 500).is_empty(), "no warn or break while paused");
        assert_eq!(e.view().phase, Phase::Working { left: 40 });
        e.resume();
        assert_eq!(e.view().phase, Phase::Working { left: 40 });
        let a = adv(&mut e, &c, 30);
        assert_eq!(a, vec![Action::Warn { secs_left: 10 }]);
        // pause is a no-op when the timer is off
        e.stop_break_timer();
        e.pause();
        assert!(!e.is_paused());
    }

    #[test]
    fn extend_break_adds_and_caps() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 61);
        assert_eq!(e.view().phase, Phase::Break { left: 4 });
        e.extend_break(60);
        assert_eq!(e.view().phase, Phase::Break { left: 64 });
        assert_eq!(e.view().break_total, 65);
        e.extend_break(99999);
        assert_eq!(e.view().phase, Phase::Break { left: 3600 });
        // outside a break it does nothing
        e.skip_break();
        let before = e.view().phase;
        e.extend_break(60);
        assert_eq!(e.view().phase, before);
    }

    #[test]
    fn start_break_now_works_from_working_and_off() {
        let (mut e, c) = mk(brk(), DAY);
        adv(&mut e, &c, 5);
        e.start_break_now();
        assert_eq!(e.tick(), vec![Action::BreakStart { secs: 5 }]);
        assert_eq!(e.view().phase, Phase::Break { left: 5 });
        e.start_break_now(); // ignored during a break
        assert!(e.tick().is_empty());
        e.stop_break_timer();
        e.tick();
        e.start_break_now();
        assert_eq!(e.tick(), vec![Action::BreakStart { secs: 5 }]);
        assert!(e.settings().break_enabled);
    }

    #[test]
    fn long_break_every_third() {
        let mut s = brk();
        s.long_break_every = 3;
        s.long_break_secs = 9;
        let (mut e, c) = mk(s, DAY);
        let mut lens = Vec::new();
        for _ in 0..4 {
            let a = adv(&mut e, &c, 70 + 10);
            for x in a {
                if let Action::BreakStart { secs } = x {
                    lens.push(secs);
                }
            }
        }
        assert_eq!(lens, vec![5, 5, 9, 5]);
        assert_eq!(e.view().cycle, 4);
    }

    #[test]
    fn strict_mode_blocks_skip_during_break() {
        let mut s = brk();
        s.allow_skip = false;
        let (mut e, c) = mk(s, DAY);
        adv(&mut e, &c, 61);
        assert!(!e.skip_break());
        assert!(matches!(e.view().phase, Phase::Break { .. }));
        assert_eq!(e.stats().day(e.today()).skipped, 0);
        // skipping a work period is still allowed
        e.settings_mut().allow_skip = true;
        assert!(e.skip_break());
        assert_eq!(e.stats().day(e.today()).skipped, 1);
    }

    #[test]
    fn snooze_default_uses_setting() {
        let mut s = brk();
        s.snooze_mins = 15;
        let (mut e, _c) = mk(s, DAY);
        e.snooze_default();
        assert_eq!(e.view().phase, Phase::Working { left: 900 });
    }

    #[test]
    fn stats_count_taken_skipped_snoozed() {
        let (mut e, c) = mk(brk(), DAY + 1000);
        adv(&mut e, &c, 61 + 5);
        let d = e.stats().day(e.today());
        assert_eq!((d.taken, d.rest_secs), (1, 5));
        adv(&mut e, &c, 61);
        e.skip_break();
        e.snooze(2);
        let d = e.stats().day(e.today());
        assert_eq!((d.taken, d.skipped, d.snoozed), (1, 1, 1));
        assert!(e.stats_mut().take_dirty());
    }

    #[test]
    fn new_settings_roundtrip_and_clamp() {
        let mut s = Settings::default();
        s.break_style = BreakStyle::Dim;
        s.long_break_every = 3;
        s.long_break_secs = 420;
        s.allow_skip = false;
        s.snooze_mins = 15;
        s.idle_pause_secs = 90;
        s.hours_enabled = true;
        s.hours_start = (8, 30);
        s.sound = false;
        s.custom_tips = vec!["Drink water".into(), "Look outside".into()];
        s.hk_break = 'K';
        s.accent = 3;
        s.ui_scale = 125;
        s.micro_water = 45;
        s.onboarded = true;
        assert_eq!(Settings::parse(&s.serialize()), s);
        let c = Settings::parse(
            "work_min=20\nlong_break_every=99\nsnooze_mins=0\nidle_pause_secs=3\ndaily_goal=0\naccent=99\nui_scale=500\nmicro_water=2\nhk_break=!\ntip=a|b\ntip=\n",
        );
        assert_eq!(c.long_break_every, 20);
        assert_eq!(c.snooze_mins, 1);
        assert_eq!(c.idle_pause_secs, 15);
        assert_eq!(c.daily_goal, 1);
        assert_eq!(c.accent, ACCENTS - 1);
        assert_eq!(c.ui_scale, 150);
        assert_eq!(c.micro_water, 5);
        assert_eq!(c.hk_break, 'B');
        assert_eq!(c.custom_tips, vec!["ab".to_string()]);
    }

    #[test]
    fn onboarding_skipped_for_existing_users() {
        assert!(Settings::parse("work_min=25\n").onboarded);
        assert!(!Settings::parse("").onboarded);
        assert!(!Settings::parse("onboarded=0\nwork_min=25\n").onboarded);
    }

    #[test]
    fn settings_save_load_files() {
        let dir = std::env::temp_dir().join(format!("bg_engine_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("BREAKGUARD_DATA_DIR", &dir);
        let mut s = Settings::default();
        s.work_min = 33;
        assert!(s.save());
        assert_eq!(Settings::load().work_min, 33);
        std::fs::write(Settings::path(), b"\xff\xfe\x00junk").unwrap();
        assert_eq!(Settings::load(), Settings::default());
        std::fs::remove_file(Settings::path()).unwrap();
        assert_eq!(Settings::load(), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_rotates() {
        let dir = std::env::temp_dir().join(format!("bg_log_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("breakguard.log");
        for i in 0..20 {
            log_to(&p, &format!("line {} xxxxxxxxxx\r\n", i), 100);
        }
        assert!(p.exists());
        assert!(dir.join("breakguard.log.1").exists());
        assert!(std::fs::metadata(&p).unwrap().len() < 200);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod settings_roundtrip_tests {
    use super::*;

    #[test]
    fn light_theme_roundtrips_and_defaults_off() {
        assert!(!Settings::default().light_theme);
        let on = Settings { light_theme: true, ..Settings::default() };
        assert!(Settings::parse(&on.serialize()).light_theme);
        let off = Settings { light_theme: false, ..Settings::default() };
        assert!(!Settings::parse(&off.serialize()).light_theme);
    }

    #[test]
    fn old_file_without_light_theme_loads_dark() {
        let s = Settings::parse("accent=2\nui_scale=110\n");
        assert!(!s.light_theme);
        assert_eq!(s.accent, 2);
    }

    #[test]
    fn hero_settings_roundtrip_all_keys() {
        let s = Settings {
            work_min: 50,
            break_secs: 90,
            long_break_every: 3,
            long_break_secs: 600,
            warn_secs: 30,
            allow_skip: false,
            sound: false,
            break_actions: "dim,mute".into(),
            ..Settings::default()
        };
        let text = s.serialize();
        for k in ["work_min=50", "break_secs=90", "long_break_every=3", "long_break_secs=600", "warn_secs=30", "allow_skip=0", "sound=0"] {
            assert!(text.lines().any(|l| l == k), "missing {k}");
        }
        let p = Settings::parse(&text);
        assert_eq!(p, s);
        assert_eq!(p.break_actions, "dim,mute");
    }

    #[test]
    fn hero_extra_options_roundtrip() {
        let s = Settings { allow_skip: false, sound: false, auto_resume: false, long_break_every: 7, ..Settings::default() };
        let text = s.serialize();
        for k in ["allow_skip=0", "sound=0", "auto_resume=0", "long_break_every=7"] {
            assert!(text.lines().any(|l| l == k), "missing {k}");
        }
        let p = Settings::parse(&text);
        assert!(!p.allow_skip && !p.sound && !p.auto_resume && p.long_break_every == 7);
        assert!(Settings::parse("").auto_resume);
    }

    #[test]
    fn hero_warn_secs_roundtrip() {
        let chips = crate::ui::parts::timer_setup::warn_chips();
        for w in [10u32, 30, 60] {
            assert!(chips.contains(&w), "chip {w} missing");
        }
        for w in chips {
            let s = Settings { warn_secs: w, ..Settings::default() };
            let text = s.serialize();
            assert!(text.lines().any(|l| l == format!("warn_secs={w}")));
            assert_eq!(Settings::parse(&text).warn_secs, w);
        }
        assert_eq!(Settings::parse("warn_secs=500
").warn_secs, 120);
    }

    #[test]
    fn bad_light_theme_value_keeps_default() {
        assert!(!Settings::parse("light_theme=maybe\n").light_theme);
    }
}
