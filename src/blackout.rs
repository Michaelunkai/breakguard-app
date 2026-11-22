//! Software blackout / night-tint overlay window and the detached hardware-off failsafe helper.
//! OWNER: Overlay. Never closes or kills anything except PIDs recorded in our own pidfiles.
#![allow(clippy::missing_safety_doc)]
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::engine::log_line;
use crate::sys;

#[path = "ui/parts/blackout_art.rs"]
mod art;

// ---------------------------------------------------------------- public API

/// Result of [`hardware_off`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HwResult {
    pub accepted: bool,
    pub mode_before: Option<u32>,
    pub answers: Option<u32>,
    pub helper_armed: bool,
}

/// Turn one monitor off via DDC/CI for `mins` minutes. The detached helper is armed FIRST;
/// if it cannot be armed, or the monitor rejects the command, nothing stays off
/// (`accepted == false`, caller falls back to [`blackout`]).
pub fn hardware_off(dev: &str, mins: u32) -> HwResult {
    let mode_before = sys::ddc_get_power(dev);
    let at = now_unix() + i64::from(mins) * 60;
    let armed = arm_helper(dev, at);
    if !armed {
        log_line(&format!("hardware off {dev} for {mins} min: helper could not be armed, not turning off"));
        return HwResult { accepted: false, mode_before, answers: None, helper_armed: false };
    }
    let accepted = sys::ddc_set_power(dev, 4);
    if !accepted {
        disarm_helper(dev);
        log_line(&format!("hardware off {dev} for {mins} min: off command rejected, helper disarmed, mode before {mode_before:?}"));
        return HwResult { accepted: false, mode_before, answers: None, helper_armed: false };
    }
    if !sys::is_mock() {
        std::thread::sleep(Duration::from_millis(400));
    }
    let answers = sys::ddc_get_power(dev);
    log_line(&format!(
        "hardware off {dev} for {mins} min: off command accepted, helper armed, mode before {mode_before:?}, monitor answers {answers:?}"
    ));
    HwResult { accepted: true, mode_before, answers, helper_armed: true }
}

/// Spawn a blackout overlay process for one monitor (`dev`) or `"all"`. true = spawned.
pub fn blackout(dev: &str, mins: u32) -> bool {
    blackout_secs(dev, mins.saturating_mul(60), 255, (0, 0, 0), &format!("{mins} min"))
}

/// Night mode: translucent click-through warm tint. `level` 0-100 = strength. Runs `secs` seconds.
pub fn night_tint(dev: &str, level: u8, secs: u32) -> bool {
    let alpha = (u32::from(level.min(100)) * 160 / 100).max(1) as u8;
    blackout_secs(dev, secs, alpha, (255, 120, 0), &format!("night tint level {level}"))
}

/// Turn the monitor back on now and stop our recorded helper/overlay for it.
pub fn cancel(dev: &str) {
    let dir = data_dir();
    let overlay_names = overlay_targets(dev);
    for d in overlay_names {
        let _ = std::fs::remove_file(pidfile(&dir, &d, "overlay"));
    }
    if dev != "all" {
        // DDC on FIRST; the helper stays armed unless the monitor accepted it
        let mut ok = false;
        for _ in 0..3 {
            ok = sys::ddc_set_power(dev, 1);
            if ok {
                break;
            }
            if !sys::is_mock() {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        if ok {
            sys::screens_on();
            // removing the helper pidfile makes the helper exit by itself (we never kill it)
            let _ = std::fs::remove_file(pidfile(&dir, dev, "helper"));
        }
        log_line(&format!(
            "turn on (cancel) {dev}: command {}{}",
            if ok { "accepted" } else { "not accepted" },
            if ok { "" } else { ", helper left armed" }
        ));
    }
}

/// `breakguard blackout --monitor <dev|all> --secs N [--alpha 0-255] [--tint R,G,B]`
pub fn run_overlay(args: &[String]) -> i32 {
    let o = match parse_overlay(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("blackout: {e}");
            return 2;
        }
    };
    let deadline = now_unix() + i64::from(o.secs);
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pf = pidfile(&dir, &o.monitor, "overlay");
    let wrote = std::fs::write(&pf, format!("{}\n", std::process::id())).is_ok();

    let code = if sys::is_mock() {
        sys::mock_log(&format!("overlay {} secs {} alpha {}", o.monitor, o.secs, o.alpha));
        if !o.label.is_empty() || !o.tip.is_empty() {
            sys::mock_log(&format!("overlay text {} | {}", o.label, o.tip));
        }
        // Mock mode draws too, but only on the allowed rect(s); never the primary one.
        // The mock monitor list is fake, so an explicit BREAKGUARD_ONLY_MONITOR rect is used as the window rect.
        let only = std::env::var("BREAKGUARD_ONLY_MONITOR").ok();
        let rects: Vec<(i32, i32, i32, i32)> = match only.as_deref().and_then(parse_rect) {
            Some(r) => vec![r],
            None => sys::monitors().iter().map(|m| (m.x, m.y, m.w, m.h)).collect(),
        };
        let allowed = mock_allowed_rects(&rects, only.as_deref());
        let targets: Vec<sys::MonitorInfo> = allowed
            .iter()
            .map(|&(x, y, w, h)| sys::MonitorInfo { name: o.monitor.clone(), x, y, w, h, primary: false, ddc_power: None })
            .collect();
        sys::mock_log(&format!("overlay drawn on {} of {} rect(s)", targets.len(), rects.len()));
        if targets.is_empty() {
            1
        } else {
            unsafe { overlay_windows(&targets, &o, deadline, &pf, wrote) }
        }
    } else {
        // per-monitor DPI so sys::monitors() rects are physical pixels
        unsafe { SetProcessDpiAwarenessContext(-4isize) };
        let mons = sys::monitors();
        let targets: Vec<sys::MonitorInfo> = if o.monitor == "all" {
            mons
        } else {
            mons.into_iter().filter(|m| m.name == o.monitor).collect()
        };
        if targets.is_empty() {
            log_line(&format!("blackout: monitor {} not found", o.monitor));
            1
        } else {
            unsafe { overlay_windows(&targets, &o, deadline, &pf, wrote) }
        }
    };
    if read_pid(&pf) == Some(std::process::id()) {
        let _ = std::fs::remove_file(&pf);
    }
    log_line("blackout ended");
    code
}

/// `breakguard helper-on --monitor <dev> --at <unix_secs>` : detached failsafe.
pub fn run_helper(args: &[String]) -> i32 {
    let (dev, at) = match parse_helper(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("helper-on: {e}");
            return 2;
        }
    };
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pf = pidfile(&dir, &dev, "helper");
    let wrote = std::fs::write(&pf, format!("{}\n{at}\n", std::process::id())).is_ok();
    let step = if sys::is_mock() { 50 } else { 250 };
    while now_unix() < at {
        if cancelled(&pf, wrote) {
            return 0; // cancelled: the canceller turned the monitor on itself
        }
        std::thread::sleep(Duration::from_millis(step));
    }
    let mock = sys::is_mock();
    let started = Instant::now();
    let mut tries = 0u32;
    let mut answer;
    loop {
        tries += 1;
        sys::ddc_set_power(&dev, 1);
        if tries >= 2 {
            sys::screens_on(); // wake + mouse nudge
        }
        std::thread::sleep(Duration::from_millis(if mock { 30 } else if tries <= 5 { 1500 } else { 3000 }));
        answer = sys::ddc_get_power(&dev);
        if answer == Some(1) || mock || (tries >= 5 && started.elapsed() > Duration::from_secs(60)) {
            break;
        }
    }
    if answer != Some(1) && !mock {
        log_line(&format!("turn on FAILED {dev}: monitor still answers {answer:?} after {tries} tries"));
    }
    if read_pid(&pf) == Some(std::process::id()) {
        let _ = std::fs::remove_file(&pf);
    }
    log_line(&format!("turn on (timer) {dev}: {tries} tries, monitor answers {answer:?}"));
    0
}

/// `breakguard off --monitor 1|2|both --minutes N [--mode hardware|blackout]`:
/// turn the chosen screen(s) off right now for N minutes (1-1440), then exit.
/// The detached helper (hardware) / overlay deadline (blackout) turns them back on.
pub fn run_off(args: &[String]) -> i32 {
    let (target, mins, mode) = match parse_off(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("off: {e}");
            return 2;
        }
    };
    let mode = mode.unwrap_or_else(|| crate::engine::Settings::load().off_mode);
    no_inherit_std();
    let mons = sys::monitors();
    let want: Vec<usize> = match target {
        crate::engine::BreakTarget::Mon1 => vec![0],
        crate::engine::BreakTarget::Mon2 => vec![1],
        crate::engine::BreakTarget::Both => (0..mons.len()).collect(),
    };
    if want.is_empty() || want.iter().any(|&i| i >= mons.len()) {
        eprintln!("off: monitor not found ({} monitor(s) present)", mons.len());
        return 1;
    }
    let mut failed = false;
    for i in want {
        let dev = &mons[i].name;
        let mut how = "";
        if mode == crate::engine::OffMode::Hardware && hardware_off(dev, mins).accepted {
            how = "hardware";
        } else if blackout(dev, mins) {
            how = "blackout";
        }
        if how.is_empty() {
            eprintln!("off: could not turn off monitor {}", i + 1);
            failed = true;
        } else {
            println!("off monitor {} {how} for {mins} min", i + 1);
            log_line(&format!("cli off monitor {} {how} for {mins} min", i + 1));
        }
    }
    i32::from(failed)
}

/// Detached helpers must not inherit our stdout/stderr pipes, or a caller capturing our output
/// would wait for the helper (up to the whole break) to exit.
fn no_inherit_std() {
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
    for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: plain Win32 calls on the process's own std handles; failure is harmless.
        unsafe {
            let h = GetStdHandle(id);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

fn parse_off(args: &[String]) -> Result<(crate::engine::BreakTarget, u32, Option<crate::engine::OffMode>), String> {
    let t = crate::engine::BreakTarget::from_key(flag(args, "--monitor").ok_or("missing --monitor (1|2|both)")?)
        .ok_or("bad --monitor (1|2|both)")?;
    let mins: u32 = flag(args, "--minutes").ok_or("missing --minutes")?.parse().map_err(|_| "bad --minutes")?;
    if !(1..=1440).contains(&mins) {
        return Err("bad --minutes (1-1440)".into());
    }
    let mode = match flag(args, "--mode") {
        None => None,
        Some("hardware") => Some(crate::engine::OffMode::Hardware),
        Some("blackout") => Some(crate::engine::OffMode::Blackout),
        Some(_) => return Err("bad --mode (hardware|blackout)".into()),
    };
    Ok((t, mins, mode))
}

// ------------------------------------------------------------ helpers (pure)

#[derive(Debug, PartialEq, Eq)]
struct OverlayArgs {
    monitor: String,
    secs: u32,
    alpha: u8,
    tint: (u8, u8, u8),
    /// Headline drawn on the overlay (e.g. "Break time"); empty = none.
    label: String,
    /// Tip line drawn under the countdown; empty = none.
    tip: String,
}

/// Text passed to the overlay: at most 200 chars, no control characters.
fn parse_text(args: &[String], name: &str) -> Result<String, String> {
    match flag(args, name) {
        None => Ok(String::new()),
        Some(t) if t.chars().count() > 200 || t.chars().any(|c| c.is_control()) => Err(format!("bad {name}")),
        Some(t) => Ok(t.to_string()),
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn parse_overlay(args: &[String]) -> Result<OverlayArgs, String> {
    let monitor = flag(args, "--monitor").ok_or("missing --monitor")?.to_string();
    if !valid_monitor(&monitor) {
        return Err("bad --monitor".into());
    }
    let secs: u32 = flag(args, "--secs").ok_or("missing --secs")?.parse().map_err(|_| "bad --secs")?;
    let alpha = match flag(args, "--alpha") {
        Some(a) => a.parse::<u8>().map_err(|_| "bad --alpha (0-255)")?,
        None => 255,
    };
    let tint = match flag(args, "--tint") {
        Some(t) => {
            let p: Vec<&str> = t.split(',').collect();
            if p.len() != 3 {
                return Err("bad --tint (R,G,B)".into());
            }
            let n = |s: &str| s.trim().parse::<u8>().map_err(|_| "bad --tint (R,G,B)".to_string());
            (n(p[0])?, n(p[1])?, n(p[2])?)
        }
        None => (0, 0, 0),
    };
    let label = parse_text(args, "--label")?;
    let tip = parse_text(args, "--tip")?;
    Ok(OverlayArgs { monitor, secs, alpha, tint, label, tip })
}

fn parse_helper(args: &[String]) -> Result<(String, i64), String> {
    let dev = flag(args, "--monitor").ok_or("missing --monitor")?.to_string();
    if !valid_monitor(&dev) {
        return Err("bad --monitor".into());
    }
    let at = flag(args, "--at").ok_or("missing --at")?.parse::<i64>().map_err(|_| "bad --at")?;
    Ok((dev, at))
}

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("BREAKGUARD_DATA_DIR") {
        return PathBuf::from(d);
    }
    match std::env::var_os("APPDATA") {
        Some(a) => PathBuf::from(a).join("BreakGuard"),
        None => std::env::temp_dir().join("BreakGuard"),
    }
}

fn sanitize(dev: &str) -> String {
    dev.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn pidfile(dir: &Path, dev: &str, kind: &str) -> PathBuf {
    dir.join(format!("pid-{}.{kind}", sanitize(dev)))
}

fn read_pid(p: &Path) -> Option<u32> {
    std::fs::read_to_string(p).ok()?.lines().next()?.trim().parse().ok()
}

/// Cancelled = we wrote our pidfile, and it has since vanished or names another PID.
/// If we could never write it, nothing can cancel us (fail toward turning on).
fn cancelled(p: &Path, wrote: bool) -> bool {
    wrote && read_pid(p) != Some(std::process::id())
}

fn valid_monitor(m: &str) -> bool {
    m == "all" || m.strip_prefix(r"\\.\DISPLAY").is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Mock mode may draw only on the second monitor (x >= 2560), or on the exact rect given by
/// `BREAKGUARD_ONLY_MONITOR="x,y,w,h"` when that is set and parses. Never the primary monitor.
fn parse_rect(s: &str) -> Option<(i32, i32, i32, i32)> {
    let p: Vec<i32> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    (p.len() == 4).then(|| (p[0], p[1], p[2], p[3]))
}

fn mock_allowed_rects(rects: &[(i32, i32, i32, i32)], only: Option<&str>) -> Vec<(i32, i32, i32, i32)> {
    let want = only.and_then(parse_rect);
    rects
        .iter()
        .copied()
        .filter(|r| match want {
            Some(w) => *r == w,
            None => r.0 >= 2560,
        })
        .collect()
}

/// Which overlay pidfile names cancel(dev) must stop: that monitor's and the "all" one.
fn overlay_targets(dev: &str) -> Vec<String> {
    if dev == "all" { vec!["all".into()] } else { vec![dev.to_string(), "all".into()] }
}

fn exe_path() -> Option<PathBuf> {
    std::env::var_os("BREAKGUARD_EXE").map(PathBuf::from).or_else(|| std::env::current_exe().ok())
}

fn spawn_detached(args: &[String]) -> Option<u32> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    let exe = exe_path()?;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP, optionally | CREATE_BREAKAWAY_FROM_JOB
    for flags in [0x0100_0208u32, 0x0000_0208u32] {
        let r = Command::new(&exe)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(flags)
            .spawn();
        if let Ok(c) = r {
            return Some(c.id());
        }
    }
    None
}

fn arm_helper(dev: &str, at: i64) -> bool {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pf = pidfile(&dir, dev, "helper");
    if std::fs::write(&pf, "0\n").is_err() {
        return false;
    }
    let args = vec!["helper-on".to_string(), "--monitor".into(), dev.into(), "--at".into(), at.to_string()];
    match spawn_detached(&args) {
        Some(pid) => {
            std::fs::write(&pf, format!("{pid}\n{at}\n")).is_ok()
        }
        None => {
            let _ = std::fs::remove_file(&pf);
            false
        }
    }
}

fn disarm_helper(dev: &str) {
    let _ = std::fs::remove_file(pidfile(&data_dir(), dev, "helper"));
}

/// Break overlay with a headline and a tip. `dim` = see-through click-through dimming, otherwise full black.
pub fn break_overlay(dev: &str, secs: u32, dim: bool, label: &str, tip: &str) -> bool {
    let (alpha, tint) = if dim { (200u8, (6u8, 10u8, 20u8)) } else { (255u8, (0u8, 0u8, 0u8)) };
    blackout_full(dev, secs, alpha, tint, &format!("break {secs} s"), label, tip)
}

fn blackout_secs(dev: &str, secs: u32, alpha: u8, tint: (u8, u8, u8), what: &str) -> bool {
    blackout_full(dev, secs, alpha, tint, what, "", "")
}

/// m:ss for the overlay countdown.
fn fmt_left(secs: i64) -> String {
    let s = secs.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

fn blackout_full(dev: &str, secs: u32, alpha: u8, tint: (u8, u8, u8), what: &str, text_label: &str, text_tip: &str) -> bool {
    let label = if dev == "all" {
        "all".to_string()
    } else {
        match sys::monitors().iter().position(|m| m.name == dev) {
            Some(i) => (i + 1).to_string(),
            None => dev.to_string(),
        }
    };
    let mut args = vec![
        "blackout".to_string(), "--monitor".into(), dev.into(), "--secs".into(), secs.to_string(),
        "--alpha".into(), alpha.to_string(), "--tint".into(), format!("{},{},{}", tint.0, tint.1, tint.2),
    ];
    for (flag, v) in [("--label", text_label), ("--tip", text_tip)] {
        let v: String = v.chars().filter(|c| !c.is_control()).take(200).collect();
        if !v.is_empty() {
            args.push(flag.into());
            args.push(v);
        }
    }
    let pid = spawn_detached(&args);
    if let Some(pid) = pid {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(pidfile(&dir, dev, "overlay"), format!("{pid}\n"));
    }
    log_line(&format!(
        "blackout screen {label} for {what}{}",
        if pid.is_some() { "" } else { ": could not start overlay" }
    ));
    pid.is_some()
}

// ------------------------------------------------------------ overlay window

static QUIT: AtomicBool = AtomicBool::new(false);
static CLICKS: Mutex<Vec<Instant>> = Mutex::new(Vec::new());
static CFG: OnceLock<(i64, Instant, bool, PathBuf, bool)> = OnceLock::new(); // total secs, start, clickthrough, pidfile

/// Text drawn on the overlay (only when a label or tip was given).
struct Paint {
    label: String,
    tip: String,
    bg: u32,
    deadline: i64,
    total_secs: i64,
}
static PAINT: OnceLock<Paint> = OnceLock::new();
static WINS: Mutex<Vec<isize>> = Mutex::new(Vec::new());
static LAST_LEFT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

unsafe fn draw_text(hdc: HDC, text: &str, rc: &mut RECT, px: i32, bold: bool, color: u32, wrap: bool) {
    if text.is_empty() {
        return;
    }
    let face = wide("Segoe UI");
    let font = CreateFontW(
        -px, 0, 0, 0, if bold { FW_BOLD as i32 } else { FW_NORMAL as i32 }, 0, 0, 0,
        DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32, 0, face.as_ptr(),
    );
    let old = SelectObject(hdc, font);
    SetTextColor(hdc, color);
    let w: Vec<u16> = text.encode_utf16().collect();
    let fmt = DT_CENTER | DT_NOPREFIX | if wrap { DT_WORDBREAK } else { DT_SINGLELINE | DT_VCENTER };
    DrawTextW(hdc, w.as_ptr(), w.len() as i32, rc, fmt);
    SelectObject(hdc, old);
    DeleteObject(font);
}

unsafe fn paint(hwnd: HWND, p: &Paint) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    let brush = CreateSolidBrush(p.bg);
    FillRect(hdc, &rc, brush);
    DeleteObject(brush);
    SetBkMode(hdc, TRANSPARENT as i32);
    let now = now_unix();
    let left = (p.deadline - now).max(0);
    let elapsed = (p.total_secs - left).max(0) as u64;
    let band = |top: i32, bottom: i32| RECT { left: rc.left + w / 8, right: rc.right - w / 8, top, bottom };
    let message = if p.tip.is_empty() { art::message_at(elapsed) } else { p.tip.as_str() };
    draw_text(hdc, &p.label, &mut band(h * 18 / 100, h * 30 / 100), (h / 22).clamp(24, 52), false, rgb(148, 163, 184), false);
    draw_text(hdc, &fmt_left(left), &mut band(h * 30 / 100, h * 52 / 100), (h / 7).clamp(72, 220), false, rgb(125, 211, 252), false);
    draw_text(hdc, art::pose_glyph(art::pose_for(elapsed)), &mut band(h * 52 / 100, h * 60 / 100), (h / 14).clamp(32, 76), false, rgb(71, 85, 105), false);
    draw_text(hdc, message, &mut band(h * 62 / 100, h * 78 / 100), (h / 30).clamp(22, 42), false, rgb(203, 213, 225), true);
    draw_text(hdc, "Press Esc or click three times to end early", &mut band(h * 90 / 100, h * 96 / 100), 16, false, rgb(100, 116, 139), false);
    EndPaint(hwnd, &ps);
}

extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn SetProcessDpiAwarenessContext(ctx: isize) -> BOOL;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if let Some((total, start, click_through, pf, wrote)) = CFG.get() {
                let esc = !*click_through
                    && start.elapsed() > Duration::from_millis(300)
                    && (GetAsyncKeyState(VK_ESCAPE as i32) as u16 & 0x8000) != 0;
                // exact length from our own start (a whole-second unix deadline could end up to 1 s early)
                if start.elapsed() >= Duration::from_secs(*total as u64) || esc || cancelled(pf, *wrote) {
                    QUIT.store(true, Ordering::SeqCst);
                    PostQuitMessage(0);
                } else {
                    if !*click_through {
                        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                    if let Some(p) = PAINT.get() {
                        let left = (p.deadline - now_unix()).max(0);
                        if LAST_LEFT.swap(left, Ordering::SeqCst) != left {
                            for h in WINS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
                                InvalidateRect(*h as HWND, std::ptr::null(), 0);
                            }
                        }
                    }
                }
            }
            0
        }
        WM_ERASEBKGND if PAINT.get().is_some() => 1,
        WM_PAINT if PAINT.get().is_some() => {
            if let Some(p) = PAINT.get() {
                paint(hwnd, p);
            }
            0
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            let mut c = CLICKS.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            c.retain(|t| now.duration_since(*t) < Duration::from_millis(1500));
            c.push(now);
            if c.len() >= 3 {
                QUIT.store(true, Ordering::SeqCst);
                PostQuitMessage(0);
            }
            0
        }
        WM_SETCURSOR => {
            SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_ARROW));
            1
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn overlay_windows(targets: &[sys::MonitorInfo], o: &OverlayArgs, deadline: i64, pf: &Path, wrote: bool) -> i32 {
    let click_through = o.alpha < 255;
    let _ = CFG.set((i64::from(o.secs), Instant::now(), click_through, pf.to_path_buf(), wrote));
    let _ = PAINT.set(Paint {
        label: o.label.clone(),
        tip: o.tip.clone(),
        bg: rgb(o.tint.0, o.tint.1, o.tint.2),
        deadline,
        total_secs: i64::from(o.secs),
    });
    let hinst = GetModuleHandleW(std::ptr::null());
    let cls = wide("BreakGuardOverlay");
    let brush = CreateSolidBrush(o.tint.0 as u32 | (o.tint.1 as u32) << 8 | (o.tint.2 as u32) << 16);
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: brush,
        lpszMenuName: std::ptr::null(),
        lpszClassName: cls.as_ptr(),
    };
    if RegisterClassW(&wc) == 0 {
        log_line("blackout: window class registration failed");
        return 1;
    }
    let mut ex = WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    if click_through {
        ex |= WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;
    }
    let title = wide("BreakGuard overlay");
    let mut wins: Vec<HWND> = Vec::new();
    for m in targets {
        let h = CreateWindowExW(ex, cls.as_ptr(), title.as_ptr(), WS_POPUP | WS_VISIBLE, m.x, m.y, m.w, m.h,
            std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null());
        if h.is_null() {
            continue;
        }
        if click_through {
            SetLayeredWindowAttributes(h, 0, o.alpha, LWA_ALPHA);
        }
        SetWindowPos(h, HWND_TOPMOST, m.x, m.y, m.w, m.h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        if !click_through && !sys::is_mock() {
            SetForegroundWindow(h);
        }
        WINS.lock().unwrap_or_else(|e| e.into_inner()).push(h as isize);
        wins.push(h);
    }
    if wins.is_empty() {
        log_line("blackout: could not create overlay window");
        return 1;
    }
    SetTimer(wins[0], 1, 100, None);
    let mut msg: MSG = std::mem::zeroed();
    while !QUIT.load(Ordering::SeqCst) && GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    KillTimer(wins[0], 1);
    for h in wins {
        DestroyWindow(h);
    }
    0
}

// ---------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn overlay_args_ok() {
        let o = parse_overlay(&s(&["--monitor", r"\\.\DISPLAY2", "--secs", "90", "--alpha", "120", "--tint", "255,120,0"])).unwrap();
        assert_eq!(
            o,
            OverlayArgs { monitor: r"\\.\DISPLAY2".into(), secs: 90, alpha: 120, tint: (255, 120, 0), label: String::new(), tip: String::new() }
        );
    }

    #[test]
    fn overlay_text_args() {
        let o = parse_overlay(&s(&["--monitor", "all", "--secs", "20", "--label", "Break time", "--tip", "Drink water & stretch"])).unwrap();
        assert_eq!((o.label.as_str(), o.tip.as_str()), ("Break time", "Drink water & stretch"));
        let long = "x".repeat(201);
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "2", "--tip", &long])).is_err());
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "2", "--label", "a\nb"])).is_err());
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "2", "--tip", "ok; calc.exe && echo $(x)"])).is_ok(), "text is data, never a command");
        let ok = "y".repeat(200);
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "2", "--tip", &ok])).is_ok());
        assert_eq!(fmt_left(0), "0:00");
        assert_eq!(fmt_left(75), "1:15");
        assert_eq!(fmt_left(3600), "60:00");
    }

    #[test]
    fn overlay_args_defaults_and_errors() {
        let o = parse_overlay(&s(&["--monitor", "all", "--secs", "2"])).unwrap();
        assert_eq!((o.alpha, o.tint), (255, (0, 0, 0)));
        assert!(parse_overlay(&s(&["--secs", "2"])).is_err());
        assert!(parse_overlay(&s(&["--monitor", "all"])).is_err());
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "x"])).is_err());
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "2", "--alpha", "300"])).is_err());
        assert!(parse_overlay(&s(&["--monitor", "all", "--secs", "2", "--tint", "1,2"])).is_err());
    }

    #[test]
    fn helper_args() {
        assert_eq!(parse_helper(&s(&["--monitor", r"\\.\DISPLAY5", "--at", "1700000000"])).unwrap(), (r"\\.\DISPLAY5".into(), 1_700_000_000));
        assert!(parse_helper(&s(&["--monitor", r"\\.\DISPLAY5"])).is_err());
        assert!(parse_helper(&s(&["--monitor", r"\\.\DISPLAY5", "--at", "abc"])).is_err());
    }

    #[test]
    fn deadline_math() {
        let d = Instant::now() + Duration::from_secs(60 * 2);
        assert!(d > Instant::now() + Duration::from_secs(119));
        assert_eq!(i64::from(3u32) * 60, 180);
    }

    #[test]
    fn pidfile_roundtrip() {
        let dir = std::env::temp_dir().join(format!("bg-pid-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = pidfile(&dir, r"\\.\DISPLAY5", "helper");
        assert!(p.file_name().unwrap().to_string_lossy().starts_with("pid-____DISPLAY5"));
        assert_eq!(read_pid(&p), None);
        std::fs::write(&p, "4242\n1700000000\n").unwrap();
        assert_eq!(read_pid(&p), Some(4242));
        assert!(cancelled(&p, true)); // file names pid 4242, not ours
        std::fs::remove_file(&p).unwrap();
        assert!(cancelled(&p, true));
        assert!(!cancelled(&p, false));
        assert!(valid_monitor("all") && valid_monitor(r"\\.\DISPLAYMOCK1") && !valid_monitor("D") && !valid_monitor(r"\\.\DISPLAY"));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn mock_filter_keeps_primary_off() {
        let primary = (0, 0, 2560, 1440);
        let second = (2560, 0, 2560, 1440);
        let rects = [primary, second];
        assert_eq!(mock_allowed_rects(&rects, None), vec![second]);
        assert_eq!(mock_allowed_rects(&rects, Some("2560,0,2560,1440")), vec![second]);
        assert_eq!(mock_allowed_rects(&rects, Some("0,0,2560,1440")), vec![primary]);
        assert!(mock_allowed_rects(&[primary], None).is_empty());
        assert!(mock_allowed_rects(&rects, Some("bogus")).len() == 1);
        assert!(mock_allowed_rects(&rects, Some("9,9,9,9")).is_empty());
    }

    #[test]
    fn overlay_targets_include_all() {
        assert_eq!(overlay_targets("X"), vec!["X".to_string(), "all".to_string()]);
        assert_eq!(overlay_targets("all"), vec!["all".to_string()]);
    }
}
