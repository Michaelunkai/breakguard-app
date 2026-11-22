#![windows_subsystem = "windows"]
// Stylistic clippy lints that the drawing code (creature painters take many geometry arguments) trips on purpose.
#![allow(
    clippy::too_many_arguments,
    clippy::wrong_self_convention,
    clippy::collapsible_if,
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::ptr_arg,
    clippy::unnecessary_lazy_evaluations,
    clippy::field_reassign_with_default
)]
//! breakguard - break reminders, timed screen-off, scheduled screen-off, timed sleep, night mode.
//! Never closes, kills or locks the user's programs.
mod agents;
mod blackout;
mod break_action;
mod break_exec;
mod engine;
mod logic;
mod logs;
mod sessions;
mod stats;
mod sys;
mod sysx;
mod tips;
mod tray;
mod ui;

use eframe::egui;
use std::path::PathBuf;

/// Title-bar / taskbar icon: raw RGBA from scripts/make-icon.py (shield + moon).
const WINDOW_ICON_SIZE: u32 = 48;
static WINDOW_ICON_RGBA: &[u8] = include_bytes!("../assets/window-icon-48.rgba");
const _: () = assert!(WINDOW_ICON_RGBA.len() == (WINDOW_ICON_SIZE * WINDOW_ICON_SIZE * 4) as usize);

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Directory for settings/log/window state: BREAKGUARD_DATA_DIR or %APPDATA%\BreakGuard.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("BREAKGUARD_DATA_DIR") {
        return PathBuf::from(d);
    }
    match std::env::var_os("APPDATA") {
        Some(a) => PathBuf::from(a).join("BreakGuard"),
        None => std::env::temp_dir().join("BreakGuard"),
    }
}

fn window_file() -> PathBuf {
    data_dir().join("window.ini")
}

fn load_window_size() -> [f32; 2] {
    let parsed = std::fs::read_to_string(window_file()).ok().and_then(|s| {
        let mut it = s.split_whitespace().map(|v| v.parse::<f32>().ok());
        Some([it.next()??, it.next()??])
    });
    match parsed {
        Some([w, h]) if (640.0..=4000.0).contains(&w) && (520.0..=3000.0).contains(&h) => [w, h],
        _ => [900.0, 640.0],
    }
}

pub fn save_window_size(w: f32, h: f32) {
    if w.is_finite() && h.is_finite() {
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(window_file(), format!("{} {}", w.round(), h.round()));
    }
}

/// Returns false when another instance already owns the mutex.
#[cfg(windows)]
fn single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    // Mock runs get a name per data dir so parallel test runs do not collide.
    let suffix: String = if sys::is_mock() {
        data_dir().to_string_lossy().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
    } else {
        String::new()
    };
    let name: Vec<u16> = format!("Local\\BreakGuardSingleInstance{suffix}").encode_utf16().chain(Some(0)).collect();
    // SAFETY: valid NUL-terminated name; the handle is intentionally kept for the process lifetime.
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        !(h.is_null() || GetLastError() == ERROR_ALREADY_EXISTS)
    }
}
#[cfg(not(windows))]
fn single_instance() -> bool {
    true
}

/// CLI subcommands print to the parent console even though this is a GUI-subsystem exe.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call; failure (no parent console) is fine.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}
#[cfg(not(windows))]
fn attach_console() {}

fn selftest() -> i32 {
    if !sys::is_mock() || std::env::var_os("BREAKGUARD_DATA_DIR").is_none() {
        println!("selftest refuses to run: set BREAKGUARD_DDC_MOCK=<file> and BREAKGUARD_DATA_DIR=<dir> (it would touch real monitors and settings)");
        return 2;
    }
    let mut fails: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool| {
        println!("{} {}", if ok { "ok  " } else { "FAIL" }, name);
        if !ok {
            fails.push(name.to_string());
        }
    };
    check("mock mode active", sys::is_mock());
    let mons = sys::monitors();
    check("two mock monitors", mons.len() == 2);
    check("exactly one primary", mons.iter().filter(|m| m.primary).count() == 1);
    if let Some(m) = mons.first() {
        check("ddc set accepted", sys::ddc_set_power(&m.name, 4));
        check("ddc get answers", sys::ddc_get_power(&m.name).is_some());
        sys::ddc_set_power(&m.name, 1);
    }
    let dd = data_dir();
    check("data dir is writable", std::fs::create_dir_all(&dd).is_ok());
    let s = engine::Settings { work_min: 33, ..Default::default() };
    check("settings save", s.save());
    check("settings reload", engine::Settings::load().work_min == 33);
    let mut e = engine::Engine::new(engine::Settings::default(), Box::new(engine::SystemClock));
    e.start_break_timer();
    check(
        "break timer starts",
        matches!(e.view().phase, engine::Phase::Working { .. }),
    );
    e.stop_break_timer();
    check("break timer stops", matches!(e.view().phase, engine::Phase::Off));
    let names: Vec<String> = mons.iter().map(|m| m.name.clone()).collect();
    let acts = e.off_now(names.clone(), 1);
    check(
        "off_now emits ScreenOff",
        acts.iter().any(|a| matches!(a, engine::Action::ScreenOff { .. })),
    );
    let acts = e.restore_all();
    check(
        "restore_all emits ScreenOn",
        acts.iter().any(|a| matches!(a, engine::Action::ScreenOn { .. })),
    );
    check("autostart off at start", !sysx::autostart_get());
    check("autostart on", sysx::autostart_set(true) && sysx::autostart_get());
    check("autostart off", sysx::autostart_set(false) && !sysx::autostart_get());
    check("mock idle readable", sysx::idle_secs() < 100_000);
    check("mock fullscreen readable", !sysx::fullscreen_busy() || std::env::var("BREAKGUARD_MOCK_BUSY").is_ok());
    for t in [sysx::Tone::Warn, sysx::Tone::BreakStart, sysx::Tone::BreakEnd, sysx::Tone::Welcome] {
        sysx::play_tone(t);
    }
    let log = std::env::var_os("BREAKGUARD_DDC_MOCK").and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    check(
        "all four tones logged",
        ["tone warn", "tone break-start", "tone break-end", "tone welcome"].iter().all(|t| log.contains(t)),
    );
    if fails.is_empty() {
        println!("SELFTEST OK");
        0
    } else {
        println!("SELFTEST FAILED: {}", fails.join(", "));
        1
    }
}

/// Screen position from `BREAKGUARD_WINDOW_POS="x,y"` (screen coords); anything else is ignored.
fn parse_window_pos(v: &str) -> Option<[f32; 2]> {
    let (x, y) = v.split_once(',')?;
    Some([x.trim().parse::<i32>().ok()? as f32, y.trim().parse::<i32>().ok()? as f32])
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if matches!(args.get(1).map(String::as_str), Some("blackout" | "break-actions" | "helper-on" | "helper-resume" | "pause-agents" | "off" | "selftest" | "--version" | "-V" | "--help" | "-h")) {
        attach_console();
    }
    match args.get(1).map(String::as_str) {
        Some("off") => std::process::exit(blackout::run_off(&args[2..])),
        Some("break-actions") => std::process::exit(break_exec::run_cli(&args[2..])),
        Some("blackout") => std::process::exit(blackout::run_overlay(&args[2..])),
        Some("helper-on") => std::process::exit(blackout::run_helper(&args[2..])),
        Some("helper-resume") => std::process::exit(agents::run_resume_helper(&args[2..])),
        Some("pause-agents") => std::process::exit(agents::run_pause_cli(&args[2..])),
        Some("selftest") => std::process::exit(selftest()),
        Some("--version") | Some("-V") => {
            { use std::io::Write; let _ = writeln!(std::io::stdout(), "breakguard {VERSION}"); }
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            use std::io::Write;
            let _ = writeln!(
                std::io::stdout(),
                "breakguard {VERSION}\nUsage: breakguard [--version | --help]\n  (no arguments)  start the app\n  off | blackout | helper-on | helper-resume | pause-agents | selftest  internal commands"
            );
            return Ok(());
        }
        _ => {}
    }
    if !single_instance() {
        eprintln!("BreakGuard is already running.");
        return Ok(());
    }
    let demo = args.iter().any(|a| a == "--demo-tour");
    let size = load_window_size();
    let pos = std::env::var("BREAKGUARD_WINDOW_POS").ok().and_then(|v| parse_window_pos(&v));
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_min_inner_size([640.0, 520.0]);
    if let Some(p) = pos {
        // Created hidden at the target spot; App shows it on the first frame so it never appears on another monitor.
        viewport = viewport.with_position(p).with_visible(false);
    }
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title("BreakGuard")
            .with_icon(egui::IconData {
                rgba: WINDOW_ICON_RGBA.to_vec(),
                width: WINDOW_ICON_SIZE,
                height: WINDOW_ICON_SIZE,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "BreakGuard",
        options,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, demo, pos)))),
    )
}

#[cfg(test)]
mod window_pos_tests {
    use super::parse_window_pos;

    #[test]
    fn parses_screen_position_and_rejects_junk() {
        assert_eq!(parse_window_pos("2560,0"), Some([2560.0, 0.0]));
        assert_eq!(parse_window_pos(" 2600 , 40 "), Some([2600.0, 40.0]));
        assert_eq!(parse_window_pos("-1920,0"), Some([-1920.0, 0.0]));
        assert_eq!(parse_window_pos("2560"), None);
        assert_eq!(parse_window_pos("a,b"), None);
        assert_eq!(parse_window_pos(""), None);
    }

    #[test]
    fn rejects_overflow_floats_and_extra_fields() {
        assert_eq!(parse_window_pos("99999999999,0"), None, "i32 overflow");
        assert_eq!(parse_window_pos("1.5,2"), None, "no floats");
        assert_eq!(parse_window_pos("1,2,3"), None, "x,y only");
        assert_eq!(parse_window_pos(",5"), None);
        assert_eq!(parse_window_pos("5,"), None);
        assert_eq!(parse_window_pos("+7,-0"), Some([7.0, 0.0]));
        assert_eq!(parse_window_pos("\t10\t,\t20\t"), Some([10.0, 20.0]));
    }
}
