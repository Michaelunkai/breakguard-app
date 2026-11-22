//! Thin Windows wrappers. Everything here is explicit and user-triggered.
//!
//! Mock mode: when env `BREAKGUARD_DDC_MOCK=<file>` is set (non-empty), NO real
//! monitor / DDC / power / sleep call is ever made. Two fake monitors are
//! reported and every action appends one line to the mock file.
#![allow(clippy::missing_safety_doc)]
#![allow(dead_code)]
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Write;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use windows_sys::Win32::Devices::Display::*;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::Power::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// VCP code 0xD6 = power mode (1 on, 4 off).
const VCP_POWER: u8 = 0xD6;
/// Extra attempts after the first one.
const DDC_RETRIES: u32 = 2;
const DDC_RETRY_GAP: Duration = Duration::from_millis(300);
/// One attempt (all physical monitors of one display) may not block longer than this.
const DDC_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);
const ENUM_TIMEOUT: Duration = Duration::from_secs(8);

static SET_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub const MOCK_DEV1: &str = r"\\.\DISPLAYMOCK1";
pub const MOCK_DEV2: &str = r"\\.\DISPLAYMOCK2";

// ---------------------------------------------------------------- mock

fn mock_path() -> Option<String> {
    match std::env::var("BREAKGUARD_DDC_MOCK") {
        Ok(p) if !p.trim().is_empty() => Some(p),
        _ => None,
    }
}

/// True when BREAKGUARD_DDC_MOCK is set: no real hardware is touched.
pub fn is_mock() -> bool {
    mock_path().is_some()
}

/// Append one line to the mock file. No-op when not in mock mode. Never panics.
pub fn mock_log(line: &str) {
    if let Some(p) = mock_path() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            // one write_all call: writeln! splits into two writes and interleaves across threads
            let _ = f.write_all(format!("{}
", line).as_bytes());
        }
    }
}

fn mock_power() -> &'static Mutex<HashMap<String, u32>> {
    static M: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn mock_power_of(dev: &str) -> Option<u32> {
    if dev != MOCK_DEV1 && dev != MOCK_DEV2 {
        return None;
    }
    let m = mock_power().lock().unwrap_or_else(|e| e.into_inner());
    Some(m.get(dev).copied().unwrap_or(1))
}

fn mock_monitors() -> Vec<MonitorInfo> {
    vec![
        MonitorInfo { name: MOCK_DEV1.into(), x: 0, y: 0, w: 1920, h: 1080, primary: true, ddc_power: mock_power_of(MOCK_DEV1) },
        MonitorInfo { name: MOCK_DEV2.into(), x: 1920, y: 0, w: 1920, h: 1080, primary: false, ddc_power: mock_power_of(MOCK_DEV2) },
    ]
}

// ---------------------------------------------------------------- helpers

/// DDC/CI calls can take several seconds: NEVER call the DDC functions on the UI thread.
/// Run `f` on a helper thread; None if it does not finish within `t` (thread is abandoned).
fn run_timeout<T: Send + 'static>(t: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(t).ok()
}

/// powrprof.dll is loaded at run time (no import library needed to link).
unsafe fn set_suspend_state(hibernate: u8, force: u8, wake_events: u8) -> u8 {
    extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(m: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let m = LoadLibraryA(c"powrprof.dll".as_ptr().cast());
    if m.is_null() {
        return 0;
    }
    let f = GetProcAddress(m, c"SetSuspendState".as_ptr().cast());
    if f.is_null() {
        return 0;
    }
    // SAFETY: SetSuspendState has exactly this signature (BOOLEAN x3 -> BOOLEAN).
    let f: extern "system" fn(u8, u8, u8) -> u8 = std::mem::transmute(f);
    f(hibernate, force, wake_events)
}

/// Accepts `\\.\DISPLAY<n>` and the mock names only.
fn valid_device(d: &str) -> bool {
    if d == MOCK_DEV1 || d == MOCK_DEV2 {
        return true;
    }
    match d.strip_prefix(r"\\.\DISPLAY") {
        Some(n) => !n.is_empty() && n.len() <= 4 && n.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

// ---------------------------------------------------------------- monitors

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
    /// None = DDC/CI power control not readable. Some(v) = VCP 0xD6 current value.
    pub ddc_power: Option<u32>,
}

/// (HMONITOR as usize, device name, rect, primary)
type RawMon = (usize, String, RECT, bool);

unsafe extern "system" fn enum_cb(h: HMONITOR, _dc: HDC, _r: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: data is the &mut Vec<RawMon> passed by enum_raw, alive for the call.
    let v = &mut *(data as *mut Vec<RawMon>);
    let mut mi: MONITORINFOEXW = std::mem::zeroed();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) != 0 {
        let name = String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string();
        v.push((h as usize, name, mi.monitorInfo.rcMonitor, mi.monitorInfo.dwFlags & 1 != 0));
    }
    1
}

fn enum_raw() -> Vec<RawMon> {
    let mut v: Vec<RawMon> = Vec::new();
    // SAFETY: callback only dereferences `v`, which outlives the synchronous call.
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(enum_cb), &mut v as *mut _ as LPARAM);
    }
    v
}

/// Run `f` for every physical monitor behind `hmon` (handles destroyed afterwards).
fn for_physical<T>(hmon: usize, mut f: impl FnMut(isize) -> Option<T>) -> Option<T> {
    // SAFETY: standard Dxva2 physical-monitor sequence; array is destroyed on every path.
    unsafe {
        let h = hmon as HMONITOR;
        let mut n: u32 = 0;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(h, &mut n) == 0 || n == 0 {
            return None;
        }
        let mut arr: Vec<PHYSICAL_MONITOR> = vec![std::mem::zeroed(); n as usize];
        if GetPhysicalMonitorsFromHMONITOR(h, n, arr.as_mut_ptr()) == 0 {
            return None;
        }
        let mut res = None;
        for p in arr.iter() {
            res = f(p.hPhysicalMonitor as isize);
            if res.is_some() {
                break;
            }
        }
        DestroyPhysicalMonitors(n, arr.as_ptr());
        res
    }
}

fn read_vcp_power(phys: isize) -> Option<u32> {
    let (mut cur, mut max, mut ty) = (0u32, 0u32, 0);
    // SAFETY: phys is a live physical-monitor handle; out pointers are valid locals.
    let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(phys as HANDLE, VCP_POWER, &mut ty, &mut cur, &mut max) };
    if ok != 0 { Some(cur) } else { None }
}

/// All monitors, sorted left-to-right. Real mode probes DDC power read-only.
pub fn monitors() -> Vec<MonitorInfo> {
    if is_mock() {
        return mock_monitors();
    }
    let build = |probe: bool| -> Vec<MonitorInfo> {
        let mut v: Vec<MonitorInfo> = enum_raw()
            .into_iter()
            .map(|(h, name, r, primary)| MonitorInfo {
                ddc_power: if probe { for_physical(h, read_vcp_power) } else { None },
                name,
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
                primary,
            })
            .collect();
        v.sort_by_key(|m| (m.x, m.y));
        v
    };
    // DDC probing can hang on bad monitors/cables: fall back to a probe-free list.
    run_timeout(ENUM_TIMEOUT, move || build(true)).unwrap_or_else(|| build(false))
}

/// Read VCP 0xD6 of the monitor with this device name. None = unsupported / unreadable.
/// Retried; never blocks longer than ~3 attempts x 5 s.
pub fn ddc_get_power(device: &str) -> Option<u32> {
    if !valid_device(device) {
        return None;
    }
    if is_mock() {
        let r = mock_power_of(device);
        mock_log(&format!("ddc get {} -> {:?}", device, r));
        return r;
    }
    for attempt in 0..=DDC_RETRIES {
        let dev = device.to_string();
        let r = run_timeout(DDC_ATTEMPT_TIMEOUT, move || {
            enum_raw().into_iter().find(|m| m.1 == dev).and_then(|m| for_physical(m.0, read_vcp_power))
        })
        .flatten();
        if r.is_some() {
            return r;
        }
        if attempt < DDC_RETRIES {
            std::thread::sleep(DDC_RETRY_GAP);
        }
    }
    None
}

/// Set VCP 0xD6 on the monitor whose device name matches. 1 = on, 4 = off.
/// Retried 2x with 300 ms gap; each attempt is time-limited. Callers should still use a thread.
pub fn ddc_set_power(device: &str, value: u32) -> bool {
    if (value != 1 && value != 4) || !valid_device(device) {
        mock_log(&format!("ddc set {} {} -> rejected", device, value));
        return false;
    }
    if is_mock() {
        let known = mock_power_of(device).is_some();
        if known {
            mock_power().lock().unwrap_or_else(|e| e.into_inner()).insert(device.to_string(), value);
        }
        mock_log(&format!("ddc set {} {} -> {}", device, value, if known { "ok" } else { "fail" }));
        return known;
    }
    for attempt in 0..=DDC_RETRIES {
        let dev = device.to_string();
        // Newer set call (retry or later on/off) invalidates abandoned older threads.
        let generation = SET_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let ok = run_timeout(DDC_ATTEMPT_TIMEOUT, move || {
            let Some(m) = enum_raw().into_iter().find(|m| m.1 == dev) else { return false };
            for_physical(m.0, |p| {
                if SET_GEN.load(std::sync::atomic::Ordering::SeqCst) != generation {
                    return None; // stale: a newer command exists
                }
                // SAFETY: p is a live physical-monitor handle for the duration of the call.
                if unsafe { SetVCPFeature(p as HANDLE, VCP_POWER, value) } != 0 { Some(true) } else { None }
            })
            .unwrap_or(false)
        })
        .unwrap_or(false);
        if ok {
            return true;
        }
        if attempt < DDC_RETRIES {
            std::thread::sleep(DDC_RETRY_GAP);
        }
    }
    false
}

// ---------------------------------------------------------------- screens / power

/// Broadcast SC_MONITORPOWER with a 1 s per-window timeout so a hung window cannot block us.
unsafe fn monitor_power_msg(lparam: isize) {
    let mut res: usize = 0;
    SendMessageTimeoutW(0xffff as HWND, WM_SYSCOMMAND, 0xF170, lparam, SMTO_ABORTIFHUNG, 1000, &mut res);
}

pub fn screens_off() {
    if is_mock() {
        mock_log("screens_off");
        return;
    }
    // SAFETY: broadcast SC_MONITORPOWER(off); no pointers involved.
    unsafe {
        monitor_power_msg(2);
    }
}

/// Turn screens back on: monitor-power message, a tiny mouse nudge, and a one-shot display request.
pub fn screens_on() {
    if is_mock() {
        mock_log("screens_on");
        return;
    }
    // SAFETY: plain value-only Win32 calls.
    unsafe {
        monitor_power_msg(-1);
        mouse_event(MOUSEEVENTF_MOVE, 1, 0, 0, 0);
        mouse_event(MOUSEEVENTF_MOVE, -1, 0, 0, 0);
        SetThreadExecutionState(ES_DISPLAY_REQUIRED);
    }
}

pub fn beep() {
    if is_mock() {
        mock_log("beep");
        return;
    }
    // SAFETY: MB_ICONASTERISK-style system sound, value-only.
    unsafe {
        MessageBeep(0x40);
    }
}

/// Holds "system required" (and optionally "display required") for as long as `keep` is true.
/// The execution state is per-thread, so a dedicated thread owns it and clears it on exit.
pub fn hold_awake(keep: std::sync::Arc<std::sync::atomic::AtomicBool>, display: bool) {
    if is_mock() {
        mock_log(&format!("hold_awake display={}", display));
        return;
    }
    std::thread::spawn(move || {
        let flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED | if display { ES_DISPLAY_REQUIRED } else { 0 };
        // SAFETY: value-only; state is cleared on this same thread below.
        unsafe {
            SetThreadExecutionState(flags);
        }
        while keep.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(500));
        }
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    });
}

#[derive(Debug)]
pub enum SleepOutcome {
    /// SetSuspendState returned (system resumed or request refused). Seconds spent before returning.
    Returned { secs: u64, timer_fired: bool },
    Failed(String),
}

/// Arms a resume-capable waitable timer for `minutes`, then suspends (S3 sleep, not hibernate).
/// Blocks until the system resumes. Never kills or throttles anything.
pub fn sleep_and_resume(minutes: u32) -> SleepOutcome {
    let minutes = minutes.clamp(1, 1440);
    if is_mock() {
        mock_log(&format!("sleep_and_resume {} min", minutes));
        return SleepOutcome::Returned { secs: 0, timer_fired: true };
    }
    // SAFETY: timer handle is created, used and closed in this function only.
    unsafe {
        let timer = CreateWaitableTimerW(std::ptr::null(), 1, std::ptr::null());
        if timer.is_null() {
            return SleepOutcome::Failed("could not create wake timer".into());
        }
        let due: i64 = -((minutes as i64) * 60 * 10_000_000);
        if SetWaitableTimer(timer, &due, 0, None, std::ptr::null::<c_void>(), 1) == 0 {
            CloseHandle(timer);
            return SleepOutcome::Failed("could not arm wake timer (wake timers may be disabled)".into());
        }
        let t0 = std::time::Instant::now();
        let r = set_suspend_state(0, 0, 0);
        let secs = t0.elapsed().as_secs();
        if r == 0 {
            CloseHandle(timer);
            return SleepOutcome::Failed("Windows refused to sleep".into());
        }
        let fired = WaitForSingleObject(timer, 0) == WAIT_OBJECT_0;
        CloseHandle(timer);
        SleepOutcome::Returned { secs, timer_fired: fired }
    }
}

// ---------------------------------------------------------------- time

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    /// 0 = Sunday .. 6 = Saturday
    pub dow: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    dow: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    ms: u16,
}

/// Current local wall-clock time (works in mock mode too: time is not hardware).
pub fn local_now() -> LocalTime {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(p: *mut SystemTime);
    }
    let mut t = SystemTime::default();
    // SAFETY: t is a valid SYSTEMTIME-layout out buffer.
    unsafe {
        GetLocalTime(&mut t);
    }
    LocalTime { year: t.year, month: t.month, day: t.day, dow: t.dow, hour: t.hour, minute: t.minute, second: t.second }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Set the mock env exactly once, to a unique temp file.
    fn init() -> String {
        static F: OnceLock<String> = OnceLock::new();
        F.get_or_init(|| {
            let p = std::env::temp_dir().join(format!("bg_sys_mock_{}.log", std::process::id()));
            let _ = std::fs::remove_file(&p);
            let s = p.to_string_lossy().to_string();
            std::env::set_var("BREAKGUARD_DDC_MOCK", &s);
            s
        })
        .clone()
    }

    #[test]
    fn mock_monitors_fixed() {
        init();
        assert!(is_mock());
        let m = monitors();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].name, MOCK_DEV1);
        assert!(m[0].primary && !m[1].primary);
        assert_eq!((m[0].w, m[0].h), (1920, 1080));
        assert_eq!(m[1].x, 1920);
    }

    #[test]
    fn mock_ddc_roundtrip_and_unknown() {
        let f = init();
        assert!(ddc_set_power(MOCK_DEV2, 4));
        assert_eq!(ddc_get_power(MOCK_DEV2), Some(4));
        assert!(ddc_set_power(MOCK_DEV2, 1));
        assert_eq!(ddc_get_power(MOCK_DEV2), Some(1));
        assert!(!ddc_set_power(r"\\.\DISPLAY99", 4));
        assert_eq!(ddc_get_power(r"\\.\DISPLAY99"), None);
        let log = std::fs::read_to_string(f).unwrap();
        assert!(log.contains(&format!("ddc set {} 4 -> ok", MOCK_DEV2)));
        assert!(log.contains(r"ddc set \\.\DISPLAY99 4 -> fail"));
    }

    #[test]
    fn mock_actions_only_log() {
        let f = init();
        screens_off();
        screens_on();
        beep();
        assert!(matches!(sleep_and_resume(3), SleepOutcome::Returned { timer_fired: true, .. }));
        mock_log("custom line");
        let log = std::fs::read_to_string(f).unwrap();
        for s in ["screens_off", "screens_on", "beep", "sleep_and_resume 3 min", "custom line"] {
            assert!(log.lines().any(|l| l == s), "missing {s}; log={log:?}");
        }
    }

    #[test]
    fn local_now_in_range() {
        let t = local_now();
        assert!(t.year >= 2024 && (1..=12).contains(&t.month) && (1..=31).contains(&t.day));
        assert!(t.dow <= 6 && t.hour < 24 && t.minute < 60 && t.second < 61);
    }

    #[test]
    fn timeout_helper() {
        assert_eq!(run_timeout(Duration::from_secs(1), || 7), Some(7));
        assert_eq!(run_timeout(Duration::from_millis(50), || { std::thread::sleep(Duration::from_millis(500)); 1 }), None);
    }
}
