//! Thin Windows wrappers. Everything here is explicit and user-triggered.
#![allow(clippy::missing_safety_doc)]
use std::ffi::c_void;
use windows_sys::Win32::Devices::Display::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Power::*;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// powrprof.dll is loaded at run time (no import library needed to link).
unsafe fn SetSuspendState(hibernate: u8, force: u8, wake_events: u8) -> u8 {
    extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(m: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let m = LoadLibraryA(b"powrprof.dll\0".as_ptr());
    if m.is_null() { return 0; }
    let f = GetProcAddress(m, b"SetSuspendState\0".as_ptr());
    if f.is_null() { return 0; }
    let f: extern "system" fn(u8, u8, u8) -> u8 = std::mem::transmute(f);
    f(hibernate, force, wake_events)
}

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

unsafe extern "system" fn enum_cb(h: HMONITOR, _dc: HDC, _r: *mut RECT, data: LPARAM) -> BOOL {
    let v = &mut *(data as *mut Vec<MonitorInfo>);
    let mut mi: MONITORINFOEXW = std::mem::zeroed();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) != 0 {
        let name = String::from_utf16_lossy(&mi.szDevice)
            .trim_end_matches('\0')
            .to_string();
        let r = mi.monitorInfo.rcMonitor;
        let mut ddc = None;
        let mut n: u32 = 0;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(h, &mut n) != 0 && n > 0 {
            let mut arr: Vec<PHYSICAL_MONITOR> = vec![std::mem::zeroed(); n as usize];
            if GetPhysicalMonitorsFromHMONITOR(h, n, arr.as_mut_ptr()) != 0 {
                let mut cur = 0u32;
                let mut max = 0u32;
                let mut ty = 0;
                // read-only probe of VCP 0xD6 (power mode)
                if GetVCPFeatureAndVCPFeatureReply(arr[0].hPhysicalMonitor, 0xD6, &mut ty, &mut cur, &mut max) != 0 {
                    ddc = Some(cur);
                }
                DestroyPhysicalMonitors(n, arr.as_ptr());
            }
        }
        v.push(MonitorInfo {
            name,
            x: r.left,
            y: r.top,
            w: r.right - r.left,
            h: r.bottom - r.top,
            primary: mi.monitorInfo.dwFlags & 1 != 0,
            ddc_power: ddc,
        });
    }
    1
}

pub fn monitors() -> Vec<MonitorInfo> {
    let mut v: Vec<MonitorInfo> = Vec::new();
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(enum_cb), &mut v as *mut _ as LPARAM);
    }
    v.sort_by_key(|m| (m.x, m.y));
    v
}

/// Set VCP 0xD6 on the monitor whose device name matches. 1 = on, 4 = off.
/// Only called from an explicit, confirmed button. Untested on real hardware.
pub fn ddc_set_power(device: &str, value: u32) -> bool {
    struct Ctx { dev: String, val: u32, ok: bool }
    unsafe extern "system" fn cb(h: HMONITOR, _d: HDC, _r: *mut RECT, data: LPARAM) -> BOOL {
        let c = &mut *(data as *mut Ctx);
        let mut mi: MONITORINFOEXW = std::mem::zeroed();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) != 0 {
            let name = String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string();
            if name == c.dev {
                let mut n = 0u32;
                if GetNumberOfPhysicalMonitorsFromHMONITOR(h, &mut n) != 0 && n > 0 {
                    let mut arr: Vec<PHYSICAL_MONITOR> = vec![std::mem::zeroed(); n as usize];
                    if GetPhysicalMonitorsFromHMONITOR(h, n, arr.as_mut_ptr()) != 0 {
                        c.ok = SetVCPFeature(arr[0].hPhysicalMonitor, 0xD6, c.val) != 0;
                        DestroyPhysicalMonitors(n, arr.as_ptr());
                    }
                }
            }
        }
        1
    }
    let mut c = Ctx { dev: device.to_string(), val: value, ok: false };
    unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(cb), &mut c as *mut _ as LPARAM); }
    c.ok
}

pub fn screens_off() {
    unsafe { SendMessageW(0xffff as HWND, WM_SYSCOMMAND, 0xF170, 2); }
}

/// Turn screens back on: monitor-power message, a tiny mouse nudge, and a one-shot display request.
pub fn screens_on() {
    unsafe {
        SendMessageW(0xffff as HWND, WM_SYSCOMMAND, 0xF170, -1);
        mouse_event(MOUSEEVENTF_MOVE, 1, 0, 0, 0);
        mouse_event(MOUSEEVENTF_MOVE, -1, 0, 0, 0);
        SetThreadExecutionState(ES_DISPLAY_REQUIRED);
    }
}

pub fn beep() {
    unsafe { MessageBeep(0x40); }
}

/// Holds "system required" (and optionally "display required") for as long as `keep` is true.
/// The execution state is per-thread, so a dedicated thread owns it and clears it on exit.
pub fn hold_awake(keep: std::sync::Arc<std::sync::atomic::AtomicBool>, display: bool) {
    std::thread::spawn(move || {
        let flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED | if display { ES_DISPLAY_REQUIRED } else { 0 };
        unsafe { SetThreadExecutionState(flags); }
        while keep.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
    });
}

pub enum SleepOutcome {
    /// SetSuspendState returned (system resumed or request refused). Seconds spent before returning.
    Returned { secs: u64, timer_fired: bool },
    Failed(String),
}

/// Arms a resume-capable waitable timer for `minutes`, then suspends (S3 sleep, not hibernate).
/// Blocks until the system resumes. Never kills or throttles anything.
pub fn sleep_and_resume(minutes: u32) -> SleepOutcome {
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
        let r = SetSuspendState(0, 0, 0);
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
