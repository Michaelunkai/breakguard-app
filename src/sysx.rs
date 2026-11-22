//! Extra Win32 sensors and switches: idle time, fullscreen detection, autostart, tones.
//! Every function has a mock path (BREAKGUARD_DDC_MOCK) so tests never touch the machine.
use crate::sys::{is_mock, mock_log};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Seconds since the last keyboard or mouse input (mock: `BREAKGUARD_MOCK_IDLE`, default 0).
pub fn idle_secs() -> u32 {
    if is_mock() {
        return std::env::var("BREAKGUARD_MOCK_IDLE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    }
    // SAFETY: LASTINPUTINFO is a plain struct with cbSize set; GetTickCount takes no arguments.
    unsafe {
        let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut li) == 0 {
            return 0;
        }
        windows_sys::Win32::System::SystemInformation::GetTickCount().wrapping_sub(li.dwTime) / 1000
    }
}

/// True while a fullscreen app, a Direct3D fullscreen game or presentation mode is active
/// (mock: `BREAKGUARD_MOCK_BUSY=1`).
pub fn fullscreen_busy() -> bool {
    if is_mock() {
        return std::env::var("BREAKGUARD_MOCK_BUSY").map(|v| v == "1").unwrap_or(false);
    }
    let mut state: i32 = 0;
    // SAFETY: out-pointer to a local i32.
    let hr = unsafe { windows_sys::Win32::UI::Shell::SHQueryUserNotificationState(&mut state) };
    // 2 = busy (fullscreen), 3 = Direct3D fullscreen, 4 = presentation mode
    hr >= 0 && matches!(state, 2..=4)
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "BreakGuard";

fn mock_autostart_file() -> std::path::PathBuf {
    crate::engine::data_dir().join("autostart.mock")
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Whether BreakGuard starts with Windows (HKCU Run value; mock: a marker file in the data dir).
pub fn autostart_get() -> bool {
    if is_mock() {
        return mock_autostart_file().exists();
    }
    use windows_sys::Win32::System::Registry::*;
    // SAFETY: valid NUL-terminated names, handle closed before return.
    unsafe {
        let mut h: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide_z(RUN_KEY).as_ptr(), 0, KEY_QUERY_VALUE, &mut h) != 0 {
            return false;
        }
        let r = RegQueryValueExW(
            h,
            wide_z(RUN_NAME).as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        RegCloseKey(h);
        r == 0
    }
}

/// Add or remove the Run value. Returns true when the requested state is in place.
pub fn autostart_set(on: bool) -> bool {
    if is_mock() {
        let p = mock_autostart_file();
        mock_log(&format!("autostart {}", if on { "on" } else { "off" }));
        if on {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            return std::fs::write(&p, b"1").is_ok();
        }
        let _ = std::fs::remove_file(&p);
        return !p.exists();
    }
    use windows_sys::Win32::System::Registry::*;
    // SAFETY: valid NUL-terminated names and byte length of the UTF-16 value incl. terminator.
    unsafe {
        let mut h: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide_z(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE | KEY_QUERY_VALUE, &mut h) != 0 {
            return false;
        }
        let ok = if on {
            let Ok(exe) = std::env::current_exe() else {
                RegCloseKey(h);
                return false;
            };
            let v = wide_z(&format!("\"{}\" --autostart", exe.display()));
            RegSetValueExW(h, wide_z(RUN_NAME).as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32) == 0
        } else {
            let r = RegDeleteValueW(h, wide_z(RUN_NAME).as_ptr());
            r == 0 || r == 2 // 2 = already absent
        };
        RegCloseKey(h);
        ok
    }
}

/// Short, distinct sounds for each moment of the break cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Warn,
    BreakStart,
    BreakEnd,
    Welcome,
}

impl Tone {
    pub fn name(self) -> &'static str {
        match self {
            Tone::Warn => "warn",
            Tone::BreakStart => "break-start",
            Tone::BreakEnd => "break-end",
            Tone::Welcome => "welcome",
        }
    }
    /// (frequency Hz, milliseconds) steps.
    pub fn notes(self) -> &'static [(u32, u32)] {
        match self {
            Tone::Warn => &[(880, 120), (880, 120)],
            Tone::BreakStart => &[(660, 160), (523, 160), (392, 240)],
            Tone::BreakEnd => &[(392, 140), (523, 140), (659, 200)],
            Tone::Welcome => &[(523, 110), (659, 110), (784, 180)],
        }
    }
}

/// Play a tone (blocks for under a second: call from a worker thread). Mock: logs `tone <name>`.
pub fn play_tone(t: Tone) {
    if is_mock() {
        mock_log(&format!("tone {}", t.name()));
        return;
    }
    for (f, ms) in t.notes() {
        // SAFETY: plain value arguments.
        unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::Beep(*f, *ms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tones_are_distinct_and_short() {
        let all = [Tone::Warn, Tone::BreakStart, Tone::BreakEnd, Tone::Welcome];
        for t in all {
            let total: u32 = t.notes().iter().map(|n| n.1).sum();
            assert!(total <= 600, "{} too long: {total} ms", t.name());
            assert!(!t.notes().is_empty());
        }
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.notes(), b.notes());
                assert_ne!(a.name(), b.name());
            }
        }
    }

    #[test]
    fn run_key_is_hkcu_run() {
        assert!(RUN_KEY.ends_with("\\Run"));
        assert_eq!(RUN_NAME, "BreakGuard");
    }
}
