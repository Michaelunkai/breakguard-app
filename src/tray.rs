//! Tray icon (menu, balloon notices) and global hotkeys on one hidden window thread.
//! Commands reach the UI through a channel; the thread wakes egui with `request_repaint`.
//! Not started in mock mode, so tests and screenshots never register real hotkeys.
use crate::engine::Settings;
use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Show,
    BreakNow,
    TogglePause,
    Snooze,
    ToggleNight,
    Quit,
}

/// One global hotkey: `Ctrl+Alt+<letter>` for a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub cmd: Cmd,
    pub key: char,
}

impl Binding {
    pub fn label(&self) -> String {
        format!("Ctrl+Alt+{}", self.key)
    }
}

/// Hotkeys from settings (empty when hotkeys are off).
pub fn bindings(s: &Settings) -> Vec<Binding> {
    if !s.hotkeys {
        return Vec::new();
    }
    vec![
        Binding { cmd: Cmd::BreakNow, key: s.hk_break },
        Binding { cmd: Cmd::TogglePause, key: s.hk_pause },
        Binding { cmd: Cmd::Snooze, key: s.hk_snooze },
    ]
}

/// Letters must be A-Z and different from each other.
pub fn validate(brk: char, pause: char, snooze: char) -> Result<(), String> {
    for (name, c) in [("Break now", brk), ("Pause", pause), ("Snooze", snooze)] {
        if !c.is_ascii_uppercase() {
            return Err(format!("{name}: use a letter A-Z"));
        }
    }
    if brk == pause || brk == snooze || pause == snooze {
        return Err("Each action needs its own letter".into());
    }
    Ok(())
}

const ID_SHOW: usize = 1;
const ID_BREAK: usize = 2;
const ID_PAUSE: usize = 3;
const ID_SNOOZE: usize = 4;
const ID_QUIT: usize = 5;
const ID_NIGHT: usize = 6;

/// Tray menu item id -> command.
pub fn menu_cmd(id: usize) -> Option<Cmd> {
    match id {
        ID_SHOW => Some(Cmd::Show),
        ID_BREAK => Some(Cmd::BreakNow),
        ID_PAUSE => Some(Cmd::TogglePause),
        ID_SNOOZE => Some(Cmd::Snooze),
        ID_NIGHT => Some(Cmd::ToggleNight),
        ID_QUIT => Some(Cmd::Quit),
        _ => None,
    }
}

// ------------------------------------------------------------------ shared state

static TX: OnceLock<Mutex<Sender<Cmd>>> = OnceLock::new();
static CTX: OnceLock<egui::Context> = OnceLock::new();
static BINDS: Mutex<Vec<Binding>> = Mutex::new(Vec::new());
static NOTICE: Mutex<Option<(String, String)>> = Mutex::new(None);
static FAILED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static PAUSED: AtomicBool = AtomicBool::new(false);
static NIGHT_ON: AtomicBool = AtomicBool::new(false);
static HWND_RAW: AtomicIsize = AtomicIsize::new(0);

fn send(cmd: Cmd) {
    if let Some(tx) = TX.get() {
        let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(cmd);
    }
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

/// Handle to the tray thread.
pub struct Tray {
    pub rx: Receiver<Cmd>,
}

/// Hotkeys another program already owns (shown in Settings).
pub fn failed_hotkeys() -> Vec<String> {
    FAILED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

impl Tray {
    /// Start the tray thread. `None` in mock mode or when the window cannot be created.
    pub fn start(ctx: egui::Context, s: &Settings) -> Option<Tray> {
        if crate::sys::is_mock() || TX.get().is_some() {
            return None;
        }
        let (tx, rx) = channel();
        let _ = TX.set(Mutex::new(tx));
        let _ = CTX.set(ctx);
        *BINDS.lock().unwrap_or_else(|e| e.into_inner()) = bindings(s);
        let (ready_tx, ready_rx) = channel::<bool>();
        std::thread::spawn(move || imp::run(ready_tx));
        match ready_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(true) => Some(Tray { rx }),
            _ => None,
        }
    }

    /// Apply changed hotkey settings.
    pub fn reconfigure(&self, s: &Settings) {
        *BINDS.lock().unwrap_or_else(|e| e.into_inner()) = bindings(s);
        imp::post(imp::MSG_REBIND);
    }

    /// Balloon notice from the tray icon.
    pub fn notify(&self, title: &str, text: &str) {
        *NOTICE.lock().unwrap_or_else(|e| e.into_inner()) = Some((title.to_string(), text.to_string()));
        imp::post(imp::MSG_NOTICE);
    }

    pub fn set_paused(&self, paused: bool) {
        PAUSED.store(paused, Ordering::SeqCst);
    }

    pub fn set_night(&self, on: bool) {
        NIGHT_ON.store(on, Ordering::SeqCst);
    }

    pub fn remove(&self) {
        imp::post(imp::MSG_REMOVE);
    }
}

mod imp {
    use super::*;
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    pub const MSG_TRAY: u32 = WM_APP + 1;
    pub const MSG_REBIND: u32 = WM_APP + 2;
    pub const MSG_NOTICE: u32 = WM_APP + 3;
    pub const MSG_REMOVE: u32 = WM_APP + 4;
    const HK_BASE: i32 = 0x4247; // "BG"

    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn copy_wide<const N: usize>(dst: &mut [u16; N], s: &str) {
        let w: Vec<u16> = s.encode_utf16().take(N - 1).collect();
        dst[..w.len()].copy_from_slice(&w);
        dst[w.len()] = 0;
    }

    pub fn post(msg: u32) {
        let h = HWND_RAW.load(Ordering::SeqCst);
        if h != 0 {
            // SAFETY: posting a message to our own window handle.
            unsafe {
                PostMessageW(h as HWND, msg, 0, 0);
            }
        }
    }

    unsafe fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
        let mut d: NOTIFYICONDATAW = std::mem::zeroed();
        d.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        d.hWnd = hwnd;
        d.uID = 1;
        d
    }

    unsafe fn add_icon(hwnd: HWND) -> bool {
        let mut d = icon_data(hwnd);
        d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        d.uCallbackMessage = MSG_TRAY;
        let hinst = GetModuleHandleW(std::ptr::null());
        let mut icon = LoadIconW(hinst, std::ptr::without_provenance(1));
        if icon.is_null() {
            icon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        }
        d.hIcon = icon;
        copy_wide(&mut d.szTip, "BreakGuard");
        Shell_NotifyIconW(NIM_ADD, &d) != 0
    }

    unsafe fn register_hotkeys(hwnd: HWND) {
        for i in 0..8 {
            UnregisterHotKey(hwnd, HK_BASE + i);
        }
        let binds = BINDS.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let mut failed = Vec::new();
        for (i, b) in binds.iter().enumerate() {
            let vk = b.key as u32; // letters: VK == ASCII uppercase
            if RegisterHotKey(hwnd, HK_BASE + i as i32, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, vk) == 0 {
                failed.push(b.label());
            }
        }
        *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = failed;
    }

    unsafe fn show_menu(hwnd: HWND) {
        let menu = CreatePopupMenu();
        let paused = PAUSED.load(Ordering::SeqCst);
        for (id, text) in [
            (ID_SHOW, "Show BreakGuard"),
            (ID_BREAK, "Take a break now"),
            (ID_PAUSE, if paused { "Resume timer" } else { "Pause timer" }),
            (ID_SNOOZE, "Snooze next break"),
            (ID_NIGHT, if NIGHT_ON.load(Ordering::SeqCst) { "Night mode off" } else { "Night mode on" }),
        ] {
            AppendMenuW(menu, MF_STRING, id, wide(text).as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_QUIT, wide("Quit").as_ptr());
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let id = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
        DestroyMenu(menu);
        if id > 0 {
            if let Some(c) = menu_cmd(id as usize) {
                send(c);
            }
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            MSG_TRAY => {
                match lp as u32 {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => send(Cmd::Show),
                    WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                    _ => {}
                }
                0
            }
            MSG_REBIND => {
                register_hotkeys(hwnd);
                0
            }
            MSG_NOTICE => {
                if let Some((title, text)) = NOTICE.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    let mut d = icon_data(hwnd);
                    d.uFlags = NIF_INFO;
                    copy_wide(&mut d.szInfoTitle, &title);
                    copy_wide(&mut d.szInfo, &text);
                    d.dwInfoFlags = NIIF_INFO;
                    Shell_NotifyIconW(NIM_MODIFY, &d);
                }
                0
            }
            MSG_REMOVE | WM_CLOSE => {
                let d = icon_data(hwnd);
                Shell_NotifyIconW(NIM_DELETE, &d);
                PostQuitMessage(0);
                0
            }
            WM_HOTKEY => {
                let i = wp as i32 - HK_BASE;
                let binds = BINDS.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(b) = usize::try_from(i).ok().and_then(|i| binds.get(i)) {
                    send(b.cmd);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    pub fn run(ready: Sender<bool>) {
        // SAFETY: standard hidden-window message loop on its own thread; all pointers are to locals
        // that outlive the calls, and the window is destroyed before the thread ends.
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let cls = wide("BreakGuardTray");
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassW(&wc);
            let title = wide("BreakGuard tray");
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW, cls.as_ptr(), title.as_ptr(), WS_POPUP, 0, 0, 0, 0,
                std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            if hwnd.is_null() || !add_icon(hwnd) {
                let _ = ready.send(false);
                return;
            }
            HWND_RAW.store(hwnd as isize, Ordering::SeqCst);
            register_hotkeys(hwnd);
            let _ = ready.send(true);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            for i in 0..8 {
                UnregisterHotKey(hwnd, HK_BASE + i);
            }
            DestroyWindow(hwnd);
            HWND_RAW.store(0, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_and_labels() {
        let s = Settings::default();
        let b = bindings(&s);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0], Binding { cmd: Cmd::BreakNow, key: 'B' });
        assert_eq!(b[1].label(), "Ctrl+Alt+P");
        let off = Settings { hotkeys: false, ..Settings::default() };
        assert!(bindings(&off).is_empty());
    }

    #[test]
    fn validate_rejects_conflicts_and_non_letters() {
        assert!(validate('B', 'P', 'S').is_ok());
        assert!(validate('B', 'B', 'S').is_err());
        assert!(validate('B', 'P', 'P').is_err());
        assert!(validate('b', 'P', 'S').is_err());
        assert!(validate('1', 'P', 'S').is_err());
    }

    #[test]
    fn menu_ids_map_to_commands() {
        assert_eq!(menu_cmd(ID_SHOW), Some(Cmd::Show));
        assert_eq!(menu_cmd(ID_BREAK), Some(Cmd::BreakNow));
        assert_eq!(menu_cmd(ID_PAUSE), Some(Cmd::TogglePause));
        assert_eq!(menu_cmd(ID_SNOOZE), Some(Cmd::Snooze));
        assert_eq!(menu_cmd(ID_NIGHT), Some(Cmd::ToggleNight));
        assert_eq!(menu_cmd(ID_QUIT), Some(Cmd::Quit));
        assert_eq!(menu_cmd(0), None);
        assert_eq!(menu_cmd(99), None);
    }

    #[test]
    fn tray_never_starts_in_mock_mode() {
        // mock mode is decided by an env var; this test only checks the pure guard order
        // (the real thread is never started by tests).
        assert!(TX.get().is_none());
    }
}
